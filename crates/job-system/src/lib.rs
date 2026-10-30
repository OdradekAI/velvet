use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct JobId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Interrupted,
    Cancelled,
    Failed,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct JobSnapshot {
    pub id: JobId,
    pub kind: String,
    pub status: JobStatus,
    pub progress: u8,
    pub message: Option<String>,
    pub attempt: u32,
    pub retryable: bool,
    pub duration_ms: u64,
    pub will_fail: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobSpec {
    pub kind: String,
    pub duration_ms: u64,
    pub will_fail: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobError {
    Store(String),
    NotFound(JobId),
    NotRetryable(JobId),
}

pub trait JobStateStore: Send + Sync {
    fn load(&self) -> Result<Vec<JobSnapshot>, String>;
    fn save(&self, jobs: &[JobSnapshot]) -> Result<(), String>;
}

#[derive(Clone, Default)]
pub struct InMemoryJobStateStore {
    state: Arc<Mutex<Vec<JobSnapshot>>>,
}

impl InMemoryJobStateStore {
    pub fn snapshots(&self) -> Vec<JobSnapshot> {
        self.state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default()
    }
}

impl JobStateStore for InMemoryJobStateStore {
    fn load(&self) -> Result<Vec<JobSnapshot>, String> {
        Ok(self.snapshots())
    }

    fn save(&self, jobs: &[JobSnapshot]) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "job store lock poisoned".to_string())?;
        *state = jobs.to_vec();
        Ok(())
    }
}

struct Inner {
    jobs: Mutex<BTreeMap<JobId, JobSnapshot>>,
    next_id: Mutex<u64>,
    store: Arc<dyn JobStateStore>,
    events: Mutex<Vec<JobSnapshot>>,
}

/// Rust-owned asynchronous work coordinator. The first slice deliberately
/// uses a small deterministic worker so the host can exercise cancellation,
/// retry and restart recovery without giving UI code a second state owner.
#[derive(Clone)]
pub struct JobSystem {
    inner: Arc<Inner>,
}

impl Default for JobSystem {
    fn default() -> Self {
        Self::with_store(Arc::new(InMemoryJobStateStore::default()))
    }
}

impl JobSystem {
    pub fn with_store(store: Arc<dyn JobStateStore>) -> Self {
        let loaded = store.load().unwrap_or_default();
        let mut jobs = BTreeMap::new();
        let mut next_id = 1;
        for mut snapshot in loaded {
            if matches!(snapshot.status, JobStatus::Queued | JobStatus::Running) {
                snapshot.status = JobStatus::Interrupted;
                snapshot.message = Some("Interrupted by application restart".into());
                snapshot.retryable = true;
            }
            if let Some(number) = snapshot
                .id
                .0
                .strip_prefix("job-")
                .and_then(|value| value.parse::<u64>().ok())
            {
                next_id = next_id.max(number.saturating_add(1));
            }
            jobs.insert(snapshot.id.clone(), snapshot);
        }
        let system = Self {
            inner: Arc::new(Inner {
                jobs: Mutex::new(jobs),
                next_id: Mutex::new(next_id),
                store,
                events: Mutex::new(Vec::new()),
            }),
        };
        let _ = system.persist();
        system
    }

    pub fn submit(&self, spec: JobSpec) -> Result<JobSnapshot, JobError> {
        let id = {
            let mut next_id = self
                .inner
                .next_id
                .lock()
                .map_err(|_| JobError::Store("job id lock poisoned".into()))?;
            let id = JobId(format!("job-{next_id:06}"));
            *next_id = next_id.saturating_add(1);
            id
        };
        let snapshot = JobSnapshot {
            id: id.clone(),
            kind: spec.kind,
            status: JobStatus::Queued,
            progress: 0,
            message: None,
            attempt: 1,
            retryable: true,
            duration_ms: spec.duration_ms.clamp(200, 30_000),
            will_fail: spec.will_fail,
        };
        self.insert(snapshot.clone())?;
        self.spawn_worker(id);
        Ok(snapshot)
    }

    pub fn cancel(&self, id: &JobId) -> Result<JobSnapshot, JobError> {
        self.update(id, |snapshot| {
            if matches!(snapshot.status, JobStatus::Queued | JobStatus::Running) {
                snapshot.status = JobStatus::Cancelled;
                snapshot.message = Some("Cancelled by user".into());
                snapshot.retryable = true;
            }
        })
    }

    pub fn retry(&self, id: &JobId) -> Result<JobSnapshot, JobError> {
        let snapshot = self.update(id, |snapshot| {
            if !matches!(
                snapshot.status,
                JobStatus::Failed | JobStatus::Cancelled | JobStatus::Interrupted
            ) {
                return;
            }
            snapshot.attempt = snapshot.attempt.saturating_add(1);
            snapshot.status = JobStatus::Queued;
            snapshot.progress = 0;
            snapshot.message = None;
            snapshot.retryable = true;
        })?;
        if snapshot.status == JobStatus::Queued {
            self.spawn_worker(id.clone());
            Ok(snapshot)
        } else {
            Err(JobError::NotRetryable(id.clone()))
        }
    }

    pub fn snapshot(&self, id: &JobId) -> Option<JobSnapshot> {
        self.inner.jobs.lock().ok()?.get(id).cloned()
    }

    pub fn snapshots(&self) -> Vec<JobSnapshot> {
        self.inner
            .jobs
            .lock()
            .map(|jobs| jobs.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn drain_events(&self) -> Vec<JobSnapshot> {
        self.inner
            .events
            .lock()
            .map(|mut events| std::mem::take(&mut *events))
            .unwrap_or_default()
    }

    fn insert(&self, snapshot: JobSnapshot) -> Result<(), JobError> {
        self.inner
            .jobs
            .lock()
            .map_err(|_| JobError::Store("job lock poisoned".into()))?
            .insert(snapshot.id.clone(), snapshot.clone());
        self.persist()?;
        self.emit(snapshot);
        Ok(())
    }

    fn update<F>(&self, id: &JobId, update: F) -> Result<JobSnapshot, JobError>
    where
        F: FnOnce(&mut JobSnapshot),
    {
        let snapshot = {
            let mut jobs = self
                .inner
                .jobs
                .lock()
                .map_err(|_| JobError::Store("job lock poisoned".into()))?;
            let snapshot = jobs
                .get_mut(id)
                .ok_or_else(|| JobError::NotFound(id.clone()))?;
            update(snapshot);
            snapshot.clone()
        };
        self.persist()?;
        self.emit(snapshot.clone());
        Ok(snapshot)
    }

    fn persist(&self) -> Result<(), JobError> {
        self.inner
            .store
            .save(&self.snapshots())
            .map_err(JobError::Store)
    }

    fn emit(&self, snapshot: JobSnapshot) {
        if let Ok(mut events) = self.inner.events.lock() {
            events.push(snapshot);
        }
    }

    fn spawn_worker(&self, id: JobId) {
        let system = self.clone();
        thread::spawn(move || {
            let Some(initial) = system.snapshot(&id) else {
                return;
            };
            if initial.status != JobStatus::Queued {
                return;
            }
            let _ = system.update(&id, |snapshot| {
                if snapshot.status == JobStatus::Queued {
                    snapshot.status = JobStatus::Running;
                    snapshot.message = Some("Running".into());
                }
            });
            let steps = 20u8;
            let delay = Duration::from_millis((initial.duration_ms / steps as u64).max(20));
            for step in 1..=steps {
                thread::sleep(delay);
                let Some(current) = system.snapshot(&id) else {
                    return;
                };
                if current.status != JobStatus::Running {
                    return;
                }
                if initial.will_fail && step >= 14 {
                    let _ = system.update(&id, |snapshot| {
                        if snapshot.status == JobStatus::Running {
                            snapshot.progress = step.saturating_mul(5).min(100);
                            snapshot.status = JobStatus::Failed;
                            snapshot.message = Some("Operation failed (simulated)".into());
                            snapshot.retryable = true;
                        }
                    });
                    return;
                }
                if step == steps {
                    let _ = system.update(&id, |snapshot| {
                        if snapshot.status == JobStatus::Running {
                            snapshot.progress = 100;
                            snapshot.status = JobStatus::Completed;
                            snapshot.message = Some("Completed".into());
                            snapshot.retryable = false;
                        }
                    });
                } else {
                    let _ = system.update(&id, |snapshot| {
                        if snapshot.status == JobStatus::Running {
                            snapshot.progress = step.saturating_mul(5).min(99);
                            snapshot.message = Some(format!("Progress {}%", snapshot.progress));
                        }
                    });
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_report_progress_and_complete() {
        let jobs = JobSystem::default();
        let created = jobs
            .submit(JobSpec {
                kind: "test".into(),
                duration_ms: 200,
                will_fail: false,
            })
            .expect("submit");
        for _ in 0..20 {
            if jobs
                .snapshot(&created.id)
                .is_some_and(|job| job.status == JobStatus::Completed)
            {
                return;
            }
            thread::sleep(Duration::from_millis(30));
        }
        panic!("job did not complete: {:?}", jobs.snapshot(&created.id));
    }

    #[test]
    fn jobs_can_cancel_and_retry() {
        let jobs = JobSystem::default();
        let created = jobs
            .submit(JobSpec {
                kind: "cancel".into(),
                duration_ms: 1_000,
                will_fail: false,
            })
            .expect("submit");
        let cancelled = jobs.cancel(&created.id).expect("cancel");
        assert_eq!(cancelled.status, JobStatus::Cancelled);
        let retried = jobs.retry(&created.id).expect("retry");
        assert_eq!(retried.status, JobStatus::Queued);
    }

    #[test]
    fn failed_jobs_are_retryable() {
        let jobs = JobSystem::default();
        let created = jobs
            .submit(JobSpec {
                kind: "fail".into(),
                duration_ms: 200,
                will_fail: true,
            })
            .expect("submit");
        for _ in 0..30 {
            if let Some(failed) = jobs.snapshot(&created.id) {
                if failed.status == JobStatus::Failed {
                    assert!(failed.retryable);
                    return;
                }
            }
            thread::sleep(Duration::from_millis(30));
        }
        panic!("job did not fail: {:?}", jobs.snapshot(&created.id));
    }

    #[test]
    fn queued_and_running_jobs_are_marked_interrupted_on_restart() {
        let store = Arc::new(InMemoryJobStateStore::default());
        store
            .save(&[JobSnapshot {
                id: JobId("job-000007".into()),
                kind: "restart".into(),
                status: JobStatus::Running,
                progress: 35,
                message: Some("Running".into()),
                attempt: 1,
                retryable: true,
                duration_ms: 30_000,
                will_fail: false,
            }])
            .expect("fixture should persist");
        let restarted = JobSystem::with_store(store);
        let recovered = restarted
            .snapshot(&JobId("job-000007".into()))
            .expect("recovered job");
        assert_eq!(recovered.status, JobStatus::Interrupted);
        assert!(recovered.retryable);
        assert!(recovered
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("restart"));
    }
}
