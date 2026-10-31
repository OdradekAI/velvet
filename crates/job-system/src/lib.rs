use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

pub const JOB_SCHEMA_VERSION: u16 = 1;
pub const MAX_JOB_INPUT_BYTES: usize = 256 * 1024;
pub const MAX_JOB_RESULT_BYTES: usize = 1024 * 1024;

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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JobProvenance {
    pub owner_id: String,
    #[serde(default)]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub content_kind: Option<String>,
    #[serde(default)]
    pub input_fingerprint: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub provider_version: Option<String>,
    #[serde(default)]
    pub settings_fingerprint: Option<String>,
    pub schema_version: u16,
}

impl JobProvenance {
    pub fn for_owner(owner_id: impl Into<String>) -> Self {
        Self {
            owner_id: owner_id.into(),
            schema_version: JOB_SCHEMA_VERSION,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobResult {
    pub schema_version: u16,
    pub content_type: String,
    pub payload: Value,
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
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub provenance: JobProvenance,
    #[serde(default)]
    pub result: Option<JobResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobSpec {
    pub kind: String,
    pub duration_ms: u64,
    pub will_fail: bool,
    pub input: Value,
    pub provenance: JobProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobExecutionResult {
    pub content_type: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobExecutionError {
    Cancelled,
    Failed(String),
}

pub trait JobRunner: Send + Sync {
    fn run(&self, context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError>;
}

pub struct JobExecutionContext {
    pub id: JobId,
    pub kind: String,
    pub attempt: u32,
    pub duration_ms: u64,
    pub will_fail: bool,
    pub input: Value,
    pub provenance: JobProvenance,
    reporter: JobProgressReporter,
    cancellation: JobCancellation,
}

impl JobExecutionContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub fn report_progress(&self, progress: u8, message: impl Into<String>) -> bool {
        self.reporter.report(progress, message)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobError {
    Store(String),
    NotFound(JobId),
    NotRetryable(JobId),
    RunnerAlreadyRegistered(String),
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
    submission: Mutex<()>,
    store: Arc<dyn JobStateStore>,
    events: Mutex<Vec<JobSnapshot>>,
    runners: RwLock<BTreeMap<String, RegisteredRunner>>,
    fallback_runner: Arc<dyn JobRunner>,
}

struct RegisteredRunner {
    owner_id: String,
    runner: Arc<dyn JobRunner>,
}

/// Rust-owned asynchronous work coordinator. Runners execute behind a typed
/// context while this type remains the only owner of lifecycle, results and
/// persistence state.
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
            snapshot.provenance = normalize_provenance(snapshot.provenance);
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
                submission: Mutex::new(()),
                store,
                events: Mutex::new(Vec::new()),
                runners: RwLock::new(BTreeMap::new()),
                fallback_runner: Arc::new(DeterministicJobRunner),
            }),
        };
        let _ = system.persist();
        system
    }

    pub fn submit(&self, spec: JobSpec) -> Result<JobSnapshot, JobError> {
        let _submission = self
            .inner
            .submission
            .lock()
            .map_err(|_| JobError::Store("job submission lock poisoned".into()))?;
        self.submit_locked(spec)
    }

    /// Reuses a completed result only when the producer supplied an input
    /// fingerprint and every provenance field matches exactly. A missing
    /// fingerprint is deliberately a cache miss so callers cannot present a
    /// result for an unknown input as current.
    pub fn submit_or_reuse(&self, spec: JobSpec) -> Result<JobSnapshot, JobError> {
        let _submission = self
            .inner
            .submission
            .lock()
            .map_err(|_| JobError::Store("job submission lock poisoned".into()))?;
        let mut spec = spec;
        spec.provenance = normalize_provenance(spec.provenance);
        if let Some(snapshot) = self.find_reusable_result(&spec.kind, &spec.provenance) {
            return Ok(snapshot);
        }
        self.submit_locked(spec)
    }

    pub fn find_reusable_result(
        &self,
        kind: &str,
        provenance: &JobProvenance,
    ) -> Option<JobSnapshot> {
        provenance.input_fingerprint.as_ref()?;
        let provenance = normalize_provenance(provenance.clone());
        self.inner
            .jobs
            .lock()
            .ok()?
            .values()
            .filter(|snapshot| {
                snapshot.kind == kind
                    && snapshot.status == JobStatus::Completed
                    && snapshot
                        .result
                        .as_ref()
                        .is_some_and(|result| result.schema_version == JOB_SCHEMA_VERSION)
                    && snapshot.provenance == provenance
            })
            .max_by(|left, right| (left.attempt, &left.id).cmp(&(right.attempt, &right.id)))
            .cloned()
    }

    fn submit_locked(&self, spec: JobSpec) -> Result<JobSnapshot, JobError> {
        validate_json_size("Job input", &spec.input, MAX_JOB_INPUT_BYTES)?;
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
            input: spec.input,
            provenance: normalize_provenance(spec.provenance),
            result: None,
        };
        self.insert(snapshot.clone())?;
        self.spawn_worker(id);
        Ok(snapshot)
    }

    pub fn register_runner(
        &self,
        owner_id: impl Into<String>,
        kind: impl Into<String>,
        runner: Arc<dyn JobRunner>,
    ) -> Result<(), JobError> {
        let owner_id = owner_id.into();
        let kind = kind.into();
        if owner_id.trim().is_empty() || kind.trim().is_empty() {
            return Err(JobError::RunnerAlreadyRegistered(
                "job runner owner and kind cannot be empty".into(),
            ));
        }
        let mut runners = self
            .inner
            .runners
            .write()
            .map_err(|_| JobError::Store("job runner registry lock poisoned".into()))?;
        if runners.contains_key(&kind) {
            return Err(JobError::RunnerAlreadyRegistered(kind));
        }
        runners.insert(kind, RegisteredRunner { owner_id, runner });
        Ok(())
    }

    pub fn unregister_runners(&self, owner_id: &str) -> Result<usize, JobError> {
        let mut runners = self
            .inner
            .runners
            .write()
            .map_err(|_| JobError::Store("job runner registry lock poisoned".into()))?;
        let before = runners.len();
        runners.retain(|_, runner| runner.owner_id != owner_id);
        Ok(before.saturating_sub(runners.len()))
    }

    pub fn cancel_owned_jobs(&self, owner_id: &str) -> Result<usize, JobError> {
        let cancelled = {
            let mut jobs = self
                .inner
                .jobs
                .lock()
                .map_err(|_| JobError::Store("job lock poisoned".into()))?;
            let mut cancelled = Vec::new();
            for snapshot in jobs.values_mut() {
                if snapshot.provenance.owner_id != owner_id
                    || !matches!(snapshot.status, JobStatus::Queued | JobStatus::Running)
                {
                    continue;
                }
                snapshot.status = JobStatus::Cancelled;
                snapshot.message = Some("Cancelled because the owner was disabled".into());
                snapshot.retryable = true;
                cancelled.push(snapshot.clone());
            }
            cancelled
        };
        if cancelled.is_empty() {
            return Ok(0);
        }
        self.persist()?;
        for snapshot in &cancelled {
            self.emit(snapshot.clone());
        }
        Ok(cancelled.len())
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
            snapshot.result = None;
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
            let Ok(Some(initial)) = system.start_attempt(&id) else {
                return;
            };
            let attempt = initial.attempt;
            let Some(runner) = system.runner_for(&initial) else {
                let _ = system.fail_attempt(
                    &id,
                    attempt,
                    format!(
                        "Job runner unavailable for owner {}",
                        initial.provenance.owner_id
                    ),
                );
                return;
            };
            let context = JobExecutionContext {
                id: id.clone(),
                kind: initial.kind.clone(),
                attempt,
                duration_ms: initial.duration_ms,
                will_fail: initial.will_fail,
                input: initial.input.clone(),
                provenance: initial.provenance.clone(),
                reporter: JobProgressReporter {
                    system: system.clone(),
                    id: id.clone(),
                    attempt,
                },
                cancellation: JobCancellation {
                    system: system.clone(),
                    id: id.clone(),
                    attempt,
                },
            };
            let outcome = catch_unwind(AssertUnwindSafe(|| runner.run(context)));
            match outcome {
                Ok(Ok(result)) => {
                    let _ = system.complete_attempt(&id, attempt, result);
                }
                Ok(Err(JobExecutionError::Cancelled)) => {
                    let _ = system.cancel_attempt(&id, attempt);
                }
                Ok(Err(JobExecutionError::Failed(message))) => {
                    let _ = system.fail_attempt(&id, attempt, message);
                }
                Err(_) => {
                    let _ = system.fail_attempt(&id, attempt, "Job runner panicked".into());
                }
            }
        });
    }

    fn runner_for(&self, snapshot: &JobSnapshot) -> Option<Arc<dyn JobRunner>> {
        if let Some(runner) = self.inner.runners.read().ok().and_then(|runners| {
            runners.get(&snapshot.kind).and_then(|entry| {
                (entry.owner_id == snapshot.provenance.owner_id).then(|| entry.runner.clone())
            })
        }) {
            return Some(runner);
        }

        (snapshot.provenance.owner_id == "core").then(|| self.inner.fallback_runner.clone())
    }

    fn start_attempt(&self, id: &JobId) -> Result<Option<JobSnapshot>, JobError> {
        let snapshot = {
            let mut jobs = self
                .inner
                .jobs
                .lock()
                .map_err(|_| JobError::Store("job lock poisoned".into()))?;
            let snapshot = jobs
                .get_mut(id)
                .ok_or_else(|| JobError::NotFound(id.clone()))?;
            if snapshot.status != JobStatus::Queued {
                return Ok(None);
            }
            snapshot.status = JobStatus::Running;
            snapshot.message = Some("Running".into());
            snapshot.clone()
        };
        self.persist()?;
        self.emit(snapshot.clone());
        Ok(Some(snapshot))
    }

    fn update_attempt<F>(
        &self,
        id: &JobId,
        attempt: u32,
        update: F,
    ) -> Result<Option<JobSnapshot>, JobError>
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
            if snapshot.attempt != attempt || snapshot.status != JobStatus::Running {
                return Ok(None);
            }
            update(snapshot);
            snapshot.clone()
        };
        self.persist()?;
        self.emit(snapshot.clone());
        Ok(Some(snapshot))
    }

    fn complete_attempt(
        &self,
        id: &JobId,
        attempt: u32,
        result: JobExecutionResult,
    ) -> Result<Option<JobSnapshot>, JobError> {
        if let Err(error) = validate_json_size("Job result", &result.payload, MAX_JOB_RESULT_BYTES)
        {
            let message = match error {
                JobError::Store(message) => message,
                other => format!("{other:?}"),
            };
            let _ = self.fail_attempt(id, attempt, message);
            return Ok(None);
        }
        self.update_attempt(id, attempt, |snapshot| {
            snapshot.progress = 100;
            snapshot.status = JobStatus::Completed;
            snapshot.message = Some("Completed".into());
            snapshot.retryable = false;
            snapshot.result = Some(JobResult {
                schema_version: JOB_SCHEMA_VERSION,
                content_type: result.content_type,
                payload: result.payload,
            });
        })
    }

    fn fail_attempt(
        &self,
        id: &JobId,
        attempt: u32,
        message: String,
    ) -> Result<Option<JobSnapshot>, JobError> {
        self.update_attempt(id, attempt, |snapshot| {
            snapshot.status = JobStatus::Failed;
            snapshot.message = Some(message);
            snapshot.retryable = true;
        })
    }

    fn cancel_attempt(&self, id: &JobId, attempt: u32) -> Result<Option<JobSnapshot>, JobError> {
        self.update_attempt(id, attempt, |snapshot| {
            snapshot.status = JobStatus::Cancelled;
            snapshot.message = Some("Cancelled by runner".into());
            snapshot.retryable = true;
        })
    }
}

#[derive(Clone)]
struct JobProgressReporter {
    system: JobSystem,
    id: JobId,
    attempt: u32,
}

impl JobProgressReporter {
    fn report(&self, progress: u8, message: impl Into<String>) -> bool {
        self.system
            .update_attempt(&self.id, self.attempt, |snapshot| {
                snapshot.progress = progress.min(100);
                snapshot.message = Some(message.into());
            })
            .ok()
            .flatten()
            .is_some()
    }
}

#[derive(Clone)]
struct JobCancellation {
    system: JobSystem,
    id: JobId,
    attempt: u32,
}

impl JobCancellation {
    fn is_cancelled(&self) -> bool {
        self.system.snapshot(&self.id).is_none_or(|snapshot| {
            snapshot.attempt != self.attempt
                || !matches!(snapshot.status, JobStatus::Queued | JobStatus::Running)
        })
    }
}

#[derive(Default)]
pub struct DeterministicJobRunner;

impl JobRunner for DeterministicJobRunner {
    fn run(&self, context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
        let steps = 20u8;
        let delay = Duration::from_millis((context.duration_ms / steps as u64).max(20));
        for step in 1..=steps {
            thread::sleep(delay);
            if context.is_cancelled() {
                return Err(JobExecutionError::Cancelled);
            }
            if context.will_fail && step >= 14 {
                return Err(JobExecutionError::Failed(
                    "Operation failed (simulated)".into(),
                ));
            }
            if step == steps {
                context.report_progress(100, "Completed");
            } else {
                let progress = step.saturating_mul(5).min(99);
                context.report_progress(progress, format!("Progress {progress}%"));
            }
        }
        Ok(JobExecutionResult {
            content_type: "application/json".into(),
            payload: json!({
                "jobKind": context.kind,
                "attempt": context.attempt,
                "status": "completed",
            }),
        })
    }
}

fn normalize_provenance(mut provenance: JobProvenance) -> JobProvenance {
    if provenance.owner_id.trim().is_empty() {
        provenance.owner_id = "core".into();
    }
    if provenance.schema_version == 0 {
        provenance.schema_version = JOB_SCHEMA_VERSION;
    }
    provenance
}

fn validate_json_size(field: &str, value: &Value, max_bytes: usize) -> Result<(), JobError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| JobError::Store(format!("{field} could not be encoded: {error}")))?;
    if bytes.len() > max_bytes {
        return Err(JobError::Store(format!(
            "{field} exceeds the {} byte limit ({} bytes)",
            max_bytes,
            bytes.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct ResultRunner;

    impl JobRunner for ResultRunner {
        fn run(
            &self,
            context: JobExecutionContext,
        ) -> Result<JobExecutionResult, JobExecutionError> {
            assert!(!context.is_cancelled());
            assert!(context.report_progress(55, "halfway"));
            Ok(JobExecutionResult {
                content_type: "application/vnd.velvet.metadata+json".into(),
                payload: json!({
                    "assetId": context.provenance.asset_id,
                    "provider": context.provenance.provider_id,
                    "attempt": context.attempt,
                }),
            })
        }
    }

    struct PanicRunner;

    impl JobRunner for PanicRunner {
        fn run(
            &self,
            _context: JobExecutionContext,
        ) -> Result<JobExecutionResult, JobExecutionError> {
            panic!("runner fixture panic")
        }
    }

    struct AttemptRunner;

    impl JobRunner for AttemptRunner {
        fn run(
            &self,
            context: JobExecutionContext,
        ) -> Result<JobExecutionResult, JobExecutionError> {
            if context.attempt == 1 {
                thread::sleep(Duration::from_millis(150));
            }
            Ok(JobExecutionResult {
                content_type: "application/json".into(),
                payload: json!({ "attempt": context.attempt }),
            })
        }
    }

    #[test]
    fn jobs_report_progress_and_complete() {
        let jobs = JobSystem::default();
        let created = jobs
            .submit(JobSpec {
                kind: "test".into(),
                duration_ms: 200,
                will_fail: false,
                input: serde_json::Value::Null,
                provenance: JobProvenance::for_owner("core"),
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
                input: serde_json::Value::Null,
                provenance: JobProvenance::for_owner("core"),
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
                input: serde_json::Value::Null,
                provenance: JobProvenance::for_owner("core"),
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
                input: serde_json::Value::Null,
                provenance: JobProvenance::for_owner("test"),
                result: None,
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

    #[test]
    fn legacy_snapshots_receive_default_provenance_on_load() {
        let legacy = serde_json::json!({
            "id": "job-000009",
            "kind": "legacy",
            "status": "Completed",
            "progress": 100,
            "message": "Completed",
            "attempt": 1,
            "retryable": false,
            "duration_ms": 200,
            "will_fail": false
        });
        let store = Arc::new(InMemoryJobStateStore::default());
        store
            .save(&[serde_json::from_value(legacy).expect("legacy snapshot should decode")])
            .expect("legacy snapshot should persist");
        let jobs = JobSystem::with_store(store);
        let restored = jobs
            .snapshot(&JobId("job-000009".into()))
            .expect("legacy job should load");
        assert_eq!(restored.provenance.owner_id, "core");
        assert_eq!(restored.provenance.schema_version, JOB_SCHEMA_VERSION);
    }

    #[test]
    fn registered_runner_persists_result_and_provenance() {
        let store = Arc::new(InMemoryJobStateStore::default());
        let jobs = JobSystem::with_store(store.clone());
        jobs.register_runner("sample.extension", "metadata", Arc::new(ResultRunner))
            .expect("runner should register");
        let created = jobs
            .submit(JobSpec {
                kind: "metadata".into(),
                duration_ms: 200,
                will_fail: false,
                input: serde_json::Value::Null,
                provenance: JobProvenance {
                    owner_id: "sample.extension".into(),
                    asset_id: Some("asset-1".into()),
                    content_kind: Some("image".into()),
                    input_fingerprint: Some("sha256:input".into()),
                    provider_id: Some("sample.preview".into()),
                    provider_version: Some("1.2.0".into()),
                    settings_fingerprint: Some("settings:v1".into()),
                    schema_version: JOB_SCHEMA_VERSION,
                },
            })
            .expect("job should submit");

        for _ in 0..20 {
            if let Some(snapshot) = jobs.snapshot(&created.id) {
                if snapshot.status == JobStatus::Completed {
                    assert_eq!(snapshot.provenance.owner_id, "sample.extension");
                    assert_eq!(
                        snapshot.provenance.input_fingerprint.as_deref(),
                        Some("sha256:input")
                    );
                    assert_eq!(
                        snapshot
                            .result
                            .as_ref()
                            .map(|result| result.content_type.as_str()),
                        Some("application/vnd.velvet.metadata+json")
                    );
                    assert_eq!(
                        snapshot.result.as_ref().unwrap().payload["assetId"],
                        "asset-1"
                    );

                    let restarted = JobSystem::with_store(store);
                    let restored = restarted
                        .snapshot(&created.id)
                        .expect("completed job should survive restart");
                    assert_eq!(restored, snapshot);
                    return;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("job did not complete: {:?}", jobs.snapshot(&created.id));
    }

    #[test]
    fn completed_results_are_reused_only_for_matching_input_provenance() {
        let jobs = JobSystem::default();
        jobs.register_runner("sample.extension", "cacheable", Arc::new(ResultRunner))
            .expect("runner should register");
        let provenance = JobProvenance {
            owner_id: "sample.extension".into(),
            input_fingerprint: Some("sha256:input-1".into()),
            provider_id: Some("sample.provider".into()),
            provider_version: Some("1.0.0".into()),
            schema_version: JOB_SCHEMA_VERSION,
            ..JobProvenance::default()
        };
        let created = jobs
            .submit_or_reuse(JobSpec {
                kind: "cacheable".into(),
                duration_ms: 200,
                will_fail: false,
                input: serde_json::Value::Null,
                provenance: provenance.clone(),
            })
            .expect("job should submit");

        for _ in 0..20 {
            if jobs
                .snapshot(&created.id)
                .is_some_and(|snapshot| snapshot.status == JobStatus::Completed)
            {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        let reused = jobs
            .submit_or_reuse(JobSpec {
                kind: "cacheable".into(),
                duration_ms: 10_000,
                will_fail: true,
                input: serde_json::Value::Null,
                provenance,
            })
            .expect("matching result should be reused");
        assert_eq!(reused.id, created.id);
        assert_eq!(reused.status, JobStatus::Completed);

        let changed_input = jobs
            .submit_or_reuse(JobSpec {
                kind: "cacheable".into(),
                duration_ms: 200,
                will_fail: false,
                input: serde_json::Value::Null,
                provenance: JobProvenance {
                    input_fingerprint: Some("sha256:input-2".into()),
                    ..reused.provenance
                },
            })
            .expect("changed input should create a new job");
        assert_ne!(changed_input.id, created.id);
    }

    #[test]
    fn missing_input_fingerprint_is_always_a_cache_miss() {
        let jobs = JobSystem::default();
        jobs.register_runner("sample.extension", "uncacheable", Arc::new(ResultRunner))
            .expect("runner should register");
        let spec = || JobSpec {
            kind: "uncacheable".into(),
            duration_ms: 200,
            will_fail: false,
            input: serde_json::Value::Null,
            provenance: JobProvenance::for_owner("sample.extension"),
        };
        let first = jobs
            .submit_or_reuse(spec())
            .expect("first job should submit");
        for _ in 0..20 {
            if jobs
                .snapshot(&first.id)
                .is_some_and(|snapshot| snapshot.status == JobStatus::Completed)
            {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let second = jobs
            .submit_or_reuse(spec())
            .expect("missing fingerprint should submit a new job");
        assert_ne!(second.id, first.id);
    }

    #[test]
    fn oversized_job_input_is_rejected_before_persistence() {
        let jobs = JobSystem::default();
        let error = jobs
            .submit(JobSpec {
                kind: "oversized".into(),
                duration_ms: 200,
                will_fail: false,
                input: Value::String("x".repeat(MAX_JOB_INPUT_BYTES + 1)),
                provenance: JobProvenance::for_owner("test"),
            })
            .expect_err("oversized input should be rejected");
        assert!(matches!(error, JobError::Store(message) if message.contains("Job input")));
        assert!(jobs.snapshots().is_empty());
    }

    #[test]
    fn panicking_runner_isolated_as_retryable_failure() {
        let jobs = JobSystem::default();
        jobs.register_runner("sample.extension", "panic", Arc::new(PanicRunner))
            .expect("runner should register");
        let created = jobs
            .submit(JobSpec {
                kind: "panic".into(),
                duration_ms: 200,
                will_fail: false,
                input: serde_json::Value::Null,
                provenance: JobProvenance::for_owner("sample.extension"),
            })
            .expect("job should submit");

        for _ in 0..20 {
            if let Some(snapshot) = jobs.snapshot(&created.id) {
                if snapshot.status == JobStatus::Failed {
                    assert!(snapshot.retryable);
                    assert_eq!(snapshot.message.as_deref(), Some("Job runner panicked"));
                    return;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "panicking job did not fail: {:?}",
            jobs.snapshot(&created.id)
        );
    }

    #[test]
    fn retry_attempt_cannot_be_overwritten_by_an_older_runner() {
        let jobs = JobSystem::default();
        jobs.register_runner("sample.extension", "attempt", Arc::new(AttemptRunner))
            .expect("runner should register");
        let created = jobs
            .submit(JobSpec {
                kind: "attempt".into(),
                duration_ms: 200,
                will_fail: false,
                input: serde_json::Value::Null,
                provenance: JobProvenance::for_owner("sample.extension"),
            })
            .expect("job should submit");
        thread::sleep(Duration::from_millis(20));
        jobs.cancel(&created.id).expect("job should cancel");
        jobs.retry(&created.id).expect("job should retry");

        for _ in 0..30 {
            if let Some(snapshot) = jobs.snapshot(&created.id) {
                if snapshot.status == JobStatus::Completed {
                    assert_eq!(snapshot.attempt, 2);
                    assert_eq!(snapshot.result.unwrap().payload["attempt"], 2);
                    return;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "retried job did not complete: {:?}",
            jobs.snapshot(&created.id)
        );
    }

    #[test]
    fn runner_registrations_can_be_removed_by_owner() {
        let jobs = JobSystem::default();
        jobs.register_runner("extension.a", "metadata", Arc::new(ResultRunner))
            .expect("runner should register");
        assert_eq!(
            jobs.unregister_runners("extension.b")
                .expect("owner lookup should succeed"),
            0
        );
        assert_eq!(
            jobs.unregister_runners("extension.a")
                .expect("owner cleanup should succeed"),
            1
        );
    }

    #[test]
    fn disabling_an_owner_cancels_its_queued_or_running_jobs() {
        let jobs = JobSystem::default();
        jobs.register_runner("extension.a", "owned", Arc::new(AttemptRunner))
            .expect("runner should register");
        let created = jobs
            .submit(JobSpec {
                kind: "owned".into(),
                duration_ms: 200,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("extension.a"),
            })
            .expect("owned job should submit");
        let cancelled = jobs
            .cancel_owned_jobs("extension.a")
            .expect("owner cancellation should succeed");
        assert_eq!(cancelled, 1);
        assert_eq!(
            jobs.snapshot(&created.id).map(|snapshot| snapshot.status),
            Some(JobStatus::Cancelled)
        );
    }

    #[test]
    fn retry_after_owner_cleanup_reports_missing_runner_instead_of_using_fallback() {
        let jobs = JobSystem::default();
        jobs.register_runner("extension.a", "owned", Arc::new(AttemptRunner))
            .expect("runner should register");
        let created = jobs
            .submit(JobSpec {
                kind: "owned".into(),
                duration_ms: 200,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("extension.a"),
            })
            .expect("owned job should submit");

        jobs.cancel_owned_jobs("extension.a")
            .expect("owner cancellation should succeed");
        jobs.unregister_runners("extension.a")
            .expect("runner cleanup should succeed");
        jobs.retry(&created.id).expect("cancelled job should retry");

        for _ in 0..30 {
            if let Some(snapshot) = jobs.snapshot(&created.id) {
                if snapshot.status == JobStatus::Failed {
                    assert!(snapshot.retryable);
                    assert_eq!(snapshot.result, None);
                    assert!(snapshot
                        .message
                        .as_deref()
                        .is_some_and(|message| message.contains("runner unavailable")));
                    return;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "owner retry did not report its missing runner: {:?}",
            jobs.snapshot(&created.id)
        );
    }
}
