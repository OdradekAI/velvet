use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct ResultRunner;

impl JobRunner for ResultRunner {
    fn run(&self, context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
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
    fn run(&self, _context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
        panic!("runner fixture panic")
    }
}

struct AttemptRunner;

impl JobRunner for AttemptRunner {
    fn run(&self, context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
        if context.attempt == 1 {
            thread::sleep(Duration::from_millis(150));
        }
        Ok(JobExecutionResult {
            content_type: "application/json".into(),
            payload: json!({ "attempt": context.attempt }),
        })
    }
}

struct HoldingRunner;

impl JobRunner for HoldingRunner {
    fn run(&self, context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(20));
            if context.is_cancelled() {
                return Err(JobExecutionError::Cancelled);
            }
        }
        Ok(JobExecutionResult {
            content_type: "application/json".into(),
            payload: Value::Null,
        })
    }
}

struct TransientThenSuccessRunner {
    calls: Arc<AtomicUsize>,
}

impl JobRunner for TransientThenSuccessRunner {
    fn run(&self, context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            return Err(JobExecutionError::Transient(
                "temporary network failure".into(),
            ));
        }
        Ok(JobExecutionResult {
            content_type: "application/json".into(),
            payload: json!({ "attempt": context.attempt }),
        })
    }
}

struct DeadlineRunner;

impl JobRunner for DeadlineRunner {
    fn run(&self, context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
        while !context.is_deadline_exceeded() {
            thread::sleep(Duration::from_millis(5));
        }
        Err(JobExecutionError::TimedOut)
    }
}

struct IgnoringDeadlineRunner;

impl JobRunner for IgnoringDeadlineRunner {
    fn run(&self, _context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
        thread::sleep(Duration::from_millis(150));
        Ok(JobExecutionResult {
            content_type: "application/json".into(),
            payload: Value::Null,
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
fn retry_policy_bounds_malformed_large_values_without_panicking() {
    let bounded = JobRetryPolicy {
        max_attempts: u32::MAX,
        initial_backoff_ms: u64::MAX,
        max_backoff_ms: 1,
    }
    .bounded();
    assert_eq!(bounded.max_attempts, 8);
    assert_eq!(bounded.initial_backoff_ms, 60_000);
    assert_eq!(bounded.max_backoff_ms, 60_000);
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
    assert!(matches!(
        cancelled.status,
        JobStatus::Cancelled | JobStatus::Cancelling
    ));
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
fn transient_failures_use_bounded_exponential_retry_and_complete() {
    let jobs = JobSystem::default();
    let calls = Arc::new(AtomicUsize::new(0));
    jobs.register_runner(
        "sample.extension",
        "transient",
        Arc::new(TransientThenSuccessRunner {
            calls: calls.clone(),
        }),
    )
    .expect("runner should register");
    let created = jobs
        .submit_with_policy(
            JobSpec {
                kind: "transient".into(),
                duration_ms: 200,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("sample.extension"),
            },
            JobExecutionPolicy {
                retry: JobRetryPolicy {
                    max_attempts: 2,
                    initial_backoff_ms: 20,
                    max_backoff_ms: 20,
                },
                ..JobExecutionPolicy::default()
            },
        )
        .expect("job should submit");
    for _ in 0..40 {
        if jobs
            .snapshot(&created.id)
            .is_some_and(|snapshot| snapshot.status == JobStatus::Completed)
        {
            let snapshot = jobs.snapshot(&created.id).expect("completed snapshot");
            assert_eq!(snapshot.attempt, 2);
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "transient job did not complete: {:?}",
        jobs.snapshot(&created.id)
    );
}

#[test]
fn deadline_is_observed_and_transient_timeout_is_limited_by_policy() {
    let jobs = JobSystem::default();
    jobs.register_runner("core", "deadline", Arc::new(DeadlineRunner))
        .expect("runner should register");
    let created = jobs
        .submit_with_policy(
            JobSpec {
                kind: "deadline".into(),
                duration_ms: 200,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("core"),
            },
            JobExecutionPolicy {
                deadline_ms: Some(20),
                cancellation_grace_ms: 5,
                retry: JobRetryPolicy {
                    max_attempts: 1,
                    initial_backoff_ms: 1,
                    max_backoff_ms: 1,
                },
                ..JobExecutionPolicy::default()
            },
        )
        .expect("job should submit");
    for _ in 0..30 {
        if jobs
            .snapshot(&created.id)
            .is_some_and(|snapshot| snapshot.status == JobStatus::Failed)
        {
            let snapshot = jobs.snapshot(&created.id).expect("failed snapshot");
            assert!(snapshot.message.unwrap_or_default().contains("deadline"));
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "deadline job did not fail: {:?}",
        jobs.snapshot(&created.id)
    );
}

#[test]
fn unresponsive_runner_is_fenced_after_deadline_grace() {
    let jobs = JobSystem::default();
    jobs.register_runner(
        "core",
        "ignoring-deadline",
        Arc::new(IgnoringDeadlineRunner),
    )
    .expect("runner should register");
    let created = jobs
        .submit_with_policy(
            JobSpec {
                kind: "ignoring-deadline".into(),
                duration_ms: 200,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("core"),
            },
            JobExecutionPolicy {
                deadline_ms: Some(20),
                cancellation_grace_ms: 5,
                retry: JobRetryPolicy {
                    max_attempts: 1,
                    initial_backoff_ms: 1,
                    max_backoff_ms: 1,
                },
                ..JobExecutionPolicy::default()
            },
        )
        .expect("job should submit");
    for _ in 0..20 {
        if jobs
            .snapshot(&created.id)
            .is_some_and(|snapshot| snapshot.status == JobStatus::Failed)
        {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "unresponsive job was not fenced: {:?}",
        jobs.snapshot(&created.id)
    );
}

#[test]
fn checkpoint_and_environment_pause_resume_are_durable() {
    let store = Arc::new(InMemoryJobStateStore::default());
    let jobs = JobSystem::with_store(store.clone());
    let created = jobs
        .submit_with_policy(
            JobSpec {
                kind: "networked".into(),
                duration_ms: 1_000,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("core"),
            },
            JobExecutionPolicy {
                network: JobNetworkPolicy::Unmetered,
                ..JobExecutionPolicy::default()
            },
        )
        .expect("job should submit");
    jobs.set_execution_environment(JobExecutionEnvironment {
        suspended: false,
        network: JobNetworkState::Offline,
        ..JobExecutionEnvironment::default()
    })
    .expect("offline environment should apply");
    let paused = jobs.snapshot(&created.id).expect("paused job");
    assert_eq!(paused.status, JobStatus::Paused);
    jobs.save_checkpoint(&created.id, paused.attempt, json!({ "step": 3 }))
        .expect("checkpoint should persist");
    let restored = JobSystem::with_store(store.clone())
        .snapshot(&created.id)
        .expect("checkpoint should survive restart");
    assert_eq!(restored.checkpoint.unwrap().payload["step"], 3);

    let invalid_store = Arc::new(InMemoryJobStateStore::default());
    invalid_store
        .save(&[JobSnapshot {
            id: JobId("job-000099".into()),
            kind: "invalid-checkpoint".into(),
            status: JobStatus::Paused,
            progress: 10,
            message: None,
            attempt: 1,
            retryable: true,
            duration_ms: 200,
            will_fail: false,
            policy: JobExecutionPolicy::default(),
            budget: JobBudgetRequest::default(),
            checkpoint: Some(JobCheckpoint {
                schema_version: JOB_CHECKPOINT_SCHEMA_VERSION + 1,
                payload: Value::Null,
            }),
            input: Value::Null,
            provenance: JobProvenance::for_owner("core"),
            result: None,
        }])
        .expect("invalid checkpoint fixture should persist");
    let invalid = JobSystem::with_store(invalid_store)
        .snapshot(&JobId("job-000099".into()))
        .expect("invalid checkpoint job");
    assert!(invalid.checkpoint.is_none());
    assert!(invalid
        .message
        .unwrap_or_default()
        .contains("Checkpoint discarded"));

    let unknown_jobs = JobSystem::default();
    unknown_jobs
        .set_execution_environment(JobExecutionEnvironment {
            suspended: false,
            network: JobNetworkState::Unknown,
            ..JobExecutionEnvironment::default()
        })
        .expect("unknown network environment should apply");
    let unknown = unknown_jobs
        .submit_with_policy(
            JobSpec {
                kind: "unknown-network".into(),
                duration_ms: 1_000,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("core"),
            },
            JobExecutionPolicy {
                network: JobNetworkPolicy::Unmetered,
                ..JobExecutionPolicy::default()
            },
        )
        .expect("unknown-network Job should submit");
    assert_eq!(unknown.status, JobStatus::Paused);
    assert!(unknown
        .message
        .as_deref()
        .is_some_and(|message| message.contains("unknown")));

    jobs.set_execution_environment(JobExecutionEnvironment::default())
        .expect("online environment should resume");
    for _ in 0..60 {
        if jobs
            .snapshot(&created.id)
            .is_some_and(|snapshot| snapshot.status == JobStatus::Completed)
        {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "resumed job did not complete: {:?}",
        jobs.snapshot(&created.id)
    );
}

#[test]
fn background_jobs_pause_for_storage_or_power_pressure() {
    let jobs = JobSystem::default();
    jobs.set_execution_environment(JobExecutionEnvironment {
        storage: StorageAvailability::ReadOnly,
        ..JobExecutionEnvironment::default()
    })
    .expect("read-only storage should be accepted");
    let storage_blocked = jobs
        .submit(JobSpec {
            kind: "storage-dependent".into(),
            duration_ms: 200,
            will_fail: false,
            input: Value::Null,
            provenance: JobProvenance::for_owner("core"),
        })
        .expect("background Job should be admitted as paused");
    assert_eq!(storage_blocked.status, JobStatus::Paused);
    assert_eq!(
        storage_blocked.message.as_deref(),
        Some("Paused while Profile storage is read-only")
    );

    jobs.set_execution_environment(JobExecutionEnvironment {
        power: PowerPressure::Critical,
        ..JobExecutionEnvironment::default()
    })
    .expect("critical power pressure should be accepted");
    let power_blocked = jobs
        .submit(JobSpec {
            kind: "power-sensitive".into(),
            duration_ms: 200,
            will_fail: false,
            input: Value::Null,
            provenance: JobProvenance::for_owner("core"),
        })
        .expect("background Job should be admitted as paused");
    assert_eq!(power_blocked.status, JobStatus::Paused);
    assert_eq!(
        power_blocked.message.as_deref(),
        Some("Paused under critical power pressure")
    );

    let foreground = jobs
        .submit_with_policy(
            JobSpec {
                kind: "foreground-recovery".into(),
                duration_ms: 200,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("core"),
            },
            JobExecutionPolicy {
                background: false,
                ..JobExecutionPolicy::default()
            },
        )
        .expect("foreground recovery Job should not be blocked by background pressure");
    assert_ne!(foreground.status, JobStatus::Paused);

    let invalid = jobs.set_execution_environment(JobExecutionEnvironment {
        schema_version: JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION + 1,
        ..JobExecutionEnvironment::default()
    });
    assert!(matches!(
        invalid,
        Err(JobError::InvalidEnvironment { schema_version })
            if schema_version == JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION + 1
    ));
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
            policy: JobExecutionPolicy::default(),
            budget: JobBudgetRequest::default(),
            checkpoint: None,
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
fn provider_version_and_settings_changes_invalidate_result_reuse() {
    let jobs = JobSystem::default();
    jobs.register_runner("sample.extension", "derived", Arc::new(ResultRunner))
        .expect("runner should register");
    let provenance = JobProvenance {
        owner_id: "sample.extension".into(),
        input_fingerprint: Some("sha256:asset-1".into()),
        provider_id: Some("sample.preview".into()),
        provider_version: Some("1.0.0".into()),
        settings_fingerprint: Some("settings:v1".into()),
        schema_version: JOB_SCHEMA_VERSION,
        ..JobProvenance::default()
    };
    let created = jobs
        .submit_or_reuse(JobSpec {
            kind: "derived".into(),
            duration_ms: 200,
            will_fail: false,
            input: Value::Null,
            provenance: provenance.clone(),
        })
        .expect("derived job should submit");

    for _ in 0..30 {
        if jobs
            .snapshot(&created.id)
            .is_some_and(|snapshot| snapshot.status == JobStatus::Completed)
        {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        jobs.snapshot(&created.id).map(|snapshot| snapshot.status),
        Some(JobStatus::Completed)
    );

    let provider_changed = jobs
        .submit_or_reuse(JobSpec {
            kind: "derived".into(),
            duration_ms: 200,
            will_fail: false,
            input: Value::Null,
            provenance: JobProvenance {
                provider_id: Some("sample.preview.decoder".into()),
                ..provenance.clone()
            },
        })
        .expect("provider change should create a new job");
    assert_ne!(provider_changed.id, created.id);

    let version_changed = jobs
        .submit_or_reuse(JobSpec {
            kind: "derived".into(),
            duration_ms: 200,
            will_fail: false,
            input: Value::Null,
            provenance: JobProvenance {
                provider_version: Some("1.1.0".into()),
                ..provenance.clone()
            },
        })
        .expect("provider version change should create a new job");
    assert_ne!(version_changed.id, created.id);

    let settings_changed = jobs
        .submit_or_reuse(JobSpec {
            kind: "derived".into(),
            duration_ms: 200,
            will_fail: false,
            input: Value::Null,
            provenance: JobProvenance {
                settings_fingerprint: Some("settings:v2".into()),
                ..provenance
            },
        })
        .expect("settings change should create a new job");
    assert_ne!(settings_changed.id, created.id);
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

#[test]
fn hard_concurrency_budget_rejects_admission_and_releases_on_cancel() {
    let mut policy = JobBudgetPolicy::unlimited();
    policy.device.concurrency = 1;
    policy.default_owner.concurrency = 1;
    policy.per_job.concurrency = 1;
    let jobs =
        JobSystem::with_store_and_budget_policy(Arc::new(InMemoryJobStateStore::default()), policy);
    let request = JobBudgetRequest {
        concurrency: 1,
        ..JobBudgetRequest::default()
    };
    let first = jobs
        .submit_with_budget(
            JobSpec {
                kind: "budgeted".into(),
                duration_ms: 200,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("core"),
            },
            request.clone(),
        )
        .expect("the first job should reserve the device budget");
    let second = jobs.submit_with_budget(
        JobSpec {
            kind: "budgeted".into(),
            duration_ms: 200,
            will_fail: false,
            input: Value::Null,
            provenance: JobProvenance::for_owner("core"),
        },
        request.clone(),
    );
    assert!(matches!(
        second,
        Err(JobError::BudgetExceeded {
            resource: BudgetResource::Concurrency,
            ..
        })
    ));
    assert_eq!(jobs.budget_usage().concurrency, 1);
    jobs.cancel(&first.id)
        .expect("the reserved job should cancel");
    let retried = jobs
        .retry(&first.id)
        .expect("retry should reuse the persisted resource claim");
    assert_eq!(retried.budget, request);
    jobs.cancel(&first.id)
        .expect("the retried job should cancel");
    assert_eq!(jobs.budget_usage().concurrency, 0);
}

#[test]
fn owner_budget_isolated_from_another_owner_and_soft_pressure_does_not_reject() {
    let mut policy = JobBudgetPolicy::unlimited();
    policy.device.concurrency = 2;
    policy.default_owner.concurrency = 1;
    policy.per_job.concurrency = 1;
    policy.device.cpu_millis_per_minute = 100;
    policy.default_owner.cpu_millis_per_minute = 100;
    policy.device.network_bytes_per_minute = 100;
    policy.default_owner.network_bytes_per_minute = 100;
    let jobs =
        JobSystem::with_store_and_budget_policy(Arc::new(InMemoryJobStateStore::default()), policy);
    jobs.register_runner("owner.a", "budgeted.owner.a", Arc::new(HoldingRunner))
        .expect("owner A runner should register");
    jobs.register_runner("owner.b", "budgeted.owner.b", Arc::new(HoldingRunner))
        .expect("owner B runner should register");
    let request = JobBudgetRequest {
        concurrency: 1,
        cpu_millis_per_minute: 101,
        network_bytes_per_minute: 101,
        ..JobBudgetRequest::default()
    };
    let make_spec = |owner_id: &str| JobSpec {
        kind: format!("budgeted.{owner_id}"),
        duration_ms: 200,
        will_fail: false,
        input: Value::Null,
        provenance: JobProvenance::for_owner(owner_id),
    };
    let first = jobs
        .submit_with_budget(make_spec("owner.a"), request.clone())
        .expect("the first owner job should be admitted");
    assert!(jobs.budget_pressure("owner.a").delay_ms > 0);
    assert!(matches!(
        jobs.submit_with_budget(make_spec("owner.a"), request.clone()),
        Err(JobError::BudgetExceeded {
            resource: BudgetResource::Concurrency,
            ..
        })
    ));
    let second_owner = jobs
        .submit_with_budget(make_spec("owner.b"), request)
        .expect("a separate owner should have an independent budget");
    assert_eq!(jobs.budget_usage().concurrency, 2);
    jobs.cancel(&first.id).expect("the first job should cancel");
    jobs.cancel(&second_owner.id)
        .expect("the second owner job should cancel");
    assert_eq!(jobs.budget_usage().concurrency, 0);
}

#[test]
fn retrying_a_paused_job_does_not_duplicate_its_reservation() {
    let mut policy = JobBudgetPolicy::unlimited();
    policy.device.concurrency = 1;
    policy.default_owner.concurrency = 1;
    policy.per_job.concurrency = 1;
    let jobs =
        JobSystem::with_store_and_budget_policy(Arc::new(InMemoryJobStateStore::default()), policy);
    jobs.set_execution_environment(JobExecutionEnvironment {
        suspended: false,
        network: JobNetworkState::Offline,
        ..JobExecutionEnvironment::default()
    })
    .expect("offline environment should apply");
    let job = jobs
        .submit_with_policy_and_budget(
            JobSpec {
                kind: "paused-budget".into(),
                duration_ms: 200,
                will_fail: false,
                input: Value::Null,
                provenance: JobProvenance::for_owner("core"),
            },
            JobExecutionPolicy {
                network: JobNetworkPolicy::Unmetered,
                ..JobExecutionPolicy::default()
            },
            JobBudgetRequest {
                concurrency: 1,
                ..JobBudgetRequest::default()
            },
        )
        .expect("paused job should reserve its claim");
    assert_eq!(job.status, JobStatus::Paused);
    assert_eq!(jobs.budget_usage().concurrency, 1);
    jobs.retry(&job.id).expect("paused job should be retryable");
    assert_eq!(jobs.budget_usage().concurrency, 1);
    jobs.cancel(&job.id).expect("retried job should cancel");
    assert_eq!(jobs.budget_usage().concurrency, 0);
}
