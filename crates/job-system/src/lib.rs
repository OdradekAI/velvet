use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

mod budget;

use budget::{BudgetRejection, JobBudgetLedger};
pub use budget::{
    BudgetResource, JobBudgetLimits, JobBudgetPolicy, JobBudgetPressure, JobBudgetRequest,
    JobBudgetUsage, JOB_BUDGET_POLICY_SCHEMA_VERSION,
};

pub const JOB_SCHEMA_VERSION: u16 = 1;
pub const MAX_JOB_INPUT_BYTES: usize = 256 * 1024;
pub const MAX_JOB_RESULT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct JobId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Retrying,
    Cancelling,
    Paused,
    Interrupted,
    Cancelled,
    Failed,
    Completed,
}

pub const JOB_CHECKPOINT_SCHEMA_VERSION: u16 = 1;
pub const MAX_JOB_CHECKPOINT_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobRetryPolicy {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_initial_backoff_ms")]
    pub initial_backoff_ms: u64,
    #[serde(default = "default_max_backoff_ms")]
    pub max_backoff_ms: u64,
}

impl JobRetryPolicy {
    pub fn bounded(self) -> Self {
        let initial_backoff_ms = self.initial_backoff_ms.min(60_000);
        Self {
            max_attempts: self.max_attempts.clamp(1, 8),
            initial_backoff_ms,
            max_backoff_ms: self.max_backoff_ms.clamp(initial_backoff_ms, 300_000),
        }
    }
}

impl Default for JobRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            initial_backoff_ms: default_initial_backoff_ms(),
            max_backoff_ms: default_max_backoff_ms(),
        }
    }
}

fn default_max_attempts() -> u32 {
    3
}

fn default_initial_backoff_ms() -> u64 {
    100
}

fn default_max_backoff_ms() -> u64 {
    2_000
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobNetworkPolicy {
    #[default]
    LocalOnly,
    Unmetered,
    AnyNetwork,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobExecutionPolicy {
    #[serde(default)]
    pub deadline_ms: Option<u64>,
    #[serde(default = "default_cancel_grace_ms")]
    pub cancellation_grace_ms: u64,
    #[serde(default)]
    pub retry: JobRetryPolicy,
    #[serde(default)]
    pub network: JobNetworkPolicy,
    #[serde(default = "default_background")]
    pub background: bool,
}

impl Default for JobExecutionPolicy {
    fn default() -> Self {
        Self {
            deadline_ms: None,
            cancellation_grace_ms: default_cancel_grace_ms(),
            retry: JobRetryPolicy::default(),
            network: JobNetworkPolicy::default(),
            background: default_background(),
        }
    }
}

impl JobExecutionPolicy {
    fn bounded(self) -> Self {
        Self {
            deadline_ms: self.deadline_ms.map(|value| value.clamp(1, 86_400_000)),
            cancellation_grace_ms: self.cancellation_grace_ms.clamp(1, 60_000),
            retry: self.retry.bounded(),
            network: self.network,
            background: self.background,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobCheckpoint {
    pub schema_version: u16,
    pub payload: Value,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobNetworkState {
    #[default]
    Online,
    Metered,
    Offline,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobExecutionEnvironment {
    pub suspended: bool,
    pub network: JobNetworkState,
}

fn default_cancel_grace_ms() -> u64 {
    1_000
}

fn default_background() -> bool {
    true
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
    pub policy: JobExecutionPolicy,
    /// Host-owned resource claim captured with the Job so a restart can
    /// explain admission decisions and restore the same request on retry.
    #[serde(default)]
    pub budget: JobBudgetRequest,
    #[serde(default)]
    pub checkpoint: Option<JobCheckpoint>,
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
    Paused,
    TimedOut,
    Transient(String),
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
    budget: JobBudgetRequest,
    deadline_ms: Option<u64>,
    cancellation_grace_ms: u64,
    checkpoint: Option<JobCheckpoint>,
    deadline_at: Option<Instant>,
    reporter: JobProgressReporter,
    cancellation: JobCancellation,
}

impl JobExecutionContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub fn is_paused(&self) -> bool {
        self.cancellation.is_paused()
    }

    pub fn is_deadline_exceeded(&self) -> bool {
        self.deadline_at
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    pub fn deadline_ms(&self) -> Option<u64> {
        self.deadline_ms
    }

    pub fn cancellation_grace_ms(&self) -> u64 {
        self.cancellation_grace_ms
    }

    pub fn budget(&self) -> &JobBudgetRequest {
        &self.budget
    }

    pub fn checkpoint(&self) -> Option<&Value> {
        self.checkpoint
            .as_ref()
            .map(|checkpoint| &checkpoint.payload)
    }

    pub fn save_checkpoint(&self, payload: Value) -> Result<(), JobError> {
        self.reporter.checkpoint(payload)
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
    BudgetExceeded {
        owner_id: String,
        resource: BudgetResource,
        requested: u64,
        available: u64,
    },
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
    budget: Mutex<JobBudgetLedger>,
    environment: Mutex<JobExecutionEnvironment>,
    active_attempts: Mutex<BTreeMap<JobId, u32>>,
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
        Self::with_store_and_budget_policy(store, JobBudgetPolicy::default())
    }

    pub fn with_store_and_budget_policy(
        store: Arc<dyn JobStateStore>,
        policy: JobBudgetPolicy,
    ) -> Self {
        let loaded = store.load().unwrap_or_default();
        let mut jobs = BTreeMap::new();
        let mut next_id = 1;
        for mut snapshot in loaded {
            snapshot.provenance = normalize_provenance(snapshot.provenance);
            snapshot.policy = snapshot.policy.clone().bounded();
            if snapshot.checkpoint.as_ref().is_some_and(|checkpoint| {
                checkpoint.schema_version != JOB_CHECKPOINT_SCHEMA_VERSION
                    || serde_json::to_vec(&checkpoint.payload)
                        .map(|bytes| bytes.len() > MAX_JOB_CHECKPOINT_BYTES)
                        .unwrap_or(true)
            }) {
                snapshot.checkpoint = None;
                snapshot.message =
                    Some("Checkpoint discarded because it is unsupported or invalid".into());
            }
            if matches!(
                snapshot.status,
                JobStatus::Queued
                    | JobStatus::Running
                    | JobStatus::Retrying
                    | JobStatus::Cancelling
                    | JobStatus::Paused
            ) {
                snapshot.status = JobStatus::Interrupted;
                snapshot.message = Some(match snapshot.message.take() {
                    Some(message) if !message.is_empty() => {
                        format!("{message}; interrupted by application restart")
                    }
                    _ => "Interrupted by application restart".into(),
                });
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
                budget: Mutex::new(JobBudgetLedger::new(policy)),
                environment: Mutex::new(JobExecutionEnvironment::default()),
                active_attempts: Mutex::new(BTreeMap::new()),
            }),
        };
        let _ = system.persist();
        system
    }

    pub fn submit(&self, spec: JobSpec) -> Result<JobSnapshot, JobError> {
        self.submit_with_budget(spec, JobBudgetRequest::default())
    }

    pub fn submit_with_budget(
        &self,
        spec: JobSpec,
        budget: JobBudgetRequest,
    ) -> Result<JobSnapshot, JobError> {
        self.submit_with_policy_and_budget(spec, JobExecutionPolicy::default(), budget)
    }

    pub fn submit_with_policy(
        &self,
        spec: JobSpec,
        policy: JobExecutionPolicy,
    ) -> Result<JobSnapshot, JobError> {
        self.submit_with_policy_and_budget(spec, policy, JobBudgetRequest::default())
    }

    pub fn submit_with_policy_and_budget(
        &self,
        spec: JobSpec,
        policy: JobExecutionPolicy,
        budget: JobBudgetRequest,
    ) -> Result<JobSnapshot, JobError> {
        let _submission = self
            .inner
            .submission
            .lock()
            .map_err(|_| JobError::Store("job submission lock poisoned".into()))?;
        self.submit_locked(spec, policy, budget)
    }

    /// Reuses a completed result only when the producer supplied an input
    /// fingerprint and every provenance field matches exactly. A missing
    /// fingerprint is deliberately a cache miss so callers cannot present a
    /// result for an unknown input as current.
    pub fn submit_or_reuse(&self, spec: JobSpec) -> Result<JobSnapshot, JobError> {
        self.submit_or_reuse_with_budget(spec, JobBudgetRequest::default())
    }

    pub fn submit_or_reuse_with_budget(
        &self,
        spec: JobSpec,
        budget: JobBudgetRequest,
    ) -> Result<JobSnapshot, JobError> {
        self.submit_or_reuse_with_policy_and_budget(spec, JobExecutionPolicy::default(), budget)
    }

    pub fn submit_or_reuse_with_policy(
        &self,
        spec: JobSpec,
        policy: JobExecutionPolicy,
    ) -> Result<JobSnapshot, JobError> {
        self.submit_or_reuse_with_policy_and_budget(spec, policy, JobBudgetRequest::default())
    }

    pub fn submit_or_reuse_with_policy_and_budget(
        &self,
        spec: JobSpec,
        policy: JobExecutionPolicy,
        budget: JobBudgetRequest,
    ) -> Result<JobSnapshot, JobError> {
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
        self.submit_locked(spec, policy, budget)
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

    fn submit_locked(
        &self,
        spec: JobSpec,
        policy: JobExecutionPolicy,
        budget: JobBudgetRequest,
    ) -> Result<JobSnapshot, JobError> {
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
        let owner_id = normalize_provenance(spec.provenance.clone()).owner_id;
        self.reserve_budget(&id, &owner_id, &budget)?;
        let policy = policy.bounded();
        let initially_paused = self.should_pause(&policy);
        let snapshot = JobSnapshot {
            id: id.clone(),
            kind: spec.kind,
            status: if initially_paused {
                JobStatus::Paused
            } else {
                JobStatus::Queued
            },
            progress: 0,
            message: initially_paused.then(|| self.pause_message(&policy)),
            attempt: 1,
            retryable: true,
            duration_ms: spec.duration_ms.clamp(200, 30_000),
            will_fail: spec.will_fail,
            policy,
            budget: budget.clone(),
            checkpoint: None,
            input: spec.input,
            provenance: normalize_provenance(spec.provenance),
            result: None,
        };
        if let Err(error) = self.insert(snapshot.clone()) {
            self.release_budget(&id);
            return Err(error);
        }
        if snapshot.status == JobStatus::Queued {
            self.spawn_worker(id);
        }
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
                    || !matches!(
                        snapshot.status,
                        JobStatus::Queued
                            | JobStatus::Running
                            | JobStatus::Retrying
                            | JobStatus::Cancelling
                            | JobStatus::Paused
                    )
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
            self.release_budget(&snapshot.id);
        }
        Ok(cancelled.len())
    }

    pub fn cancel(&self, id: &JobId) -> Result<JobSnapshot, JobError> {
        let snapshot = self.update(id, |snapshot| match snapshot.status {
            JobStatus::Running => {
                snapshot.status = JobStatus::Cancelling;
                snapshot.message = Some("Cancellation requested".into());
                snapshot.retryable = true;
            }
            JobStatus::Queued
            | JobStatus::Retrying
            | JobStatus::Paused
            | JobStatus::Interrupted => {
                snapshot.status = JobStatus::Cancelled;
                snapshot.message = Some("Cancelled by user".into());
                snapshot.retryable = true;
            }
            JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Failed
            | JobStatus::Completed => {}
        })?;
        if snapshot.status == JobStatus::Cancelling {
            // A cancelling job no longer consumes an execution slot. The
            // watchdog only observes the attempt and will become a no-op if
            // the job is retried or otherwise leaves Cancelling first.
            self.release_budget(&snapshot.id);
            self.spawn_cancellation_watchdog(
                snapshot.id.clone(),
                snapshot.attempt,
                snapshot.policy.cancellation_grace_ms,
            );
        }
        Ok(snapshot)
    }

    pub fn retry(&self, id: &JobId) -> Result<JobSnapshot, JobError> {
        let budget = self
            .snapshot(id)
            .map(|snapshot| snapshot.budget)
            .unwrap_or_default();
        self.retry_with_budget(id, budget)
    }

    pub fn retry_with_budget(
        &self,
        id: &JobId,
        budget: JobBudgetRequest,
    ) -> Result<JobSnapshot, JobError> {
        self.retry_with_policy_and_budget(id, JobExecutionPolicy::default(), budget)
    }

    pub fn retry_with_policy(
        &self,
        id: &JobId,
        policy: JobExecutionPolicy,
    ) -> Result<JobSnapshot, JobError> {
        let budget = self
            .snapshot(id)
            .map(|snapshot| snapshot.budget)
            .unwrap_or_default();
        self.retry_with_policy_and_budget(id, policy, budget)
    }

    pub fn retry_with_policy_and_budget(
        &self,
        id: &JobId,
        policy: JobExecutionPolicy,
        budget: JobBudgetRequest,
    ) -> Result<JobSnapshot, JobError> {
        let _submission = self
            .inner
            .submission
            .lock()
            .map_err(|_| JobError::Store("job submission lock poisoned".into()))?;
        let current = self
            .snapshot(id)
            .ok_or_else(|| JobError::NotFound(id.clone()))?;
        if !matches!(
            current.status,
            JobStatus::Failed
                | JobStatus::Cancelled
                | JobStatus::Interrupted
                | JobStatus::Paused
                | JobStatus::Cancelling
        ) {
            return Err(JobError::NotRetryable(id.clone()));
        }
        let existing_budget = self
            .inner
            .budget
            .lock()
            .ok()
            .and_then(|ledger| ledger.reservation(&id.0));
        if existing_budget.as_ref() != Some(&budget) {
            if existing_budget.is_some() {
                self.release_budget(id);
            }
            if let Err(error) = self.reserve_budget(id, &current.provenance.owner_id, &budget) {
                if let Some(previous) = existing_budget.as_ref() {
                    let _ = self.reserve_budget(id, &current.provenance.owner_id, previous);
                }
                return Err(error);
            }
        }
        let snapshot = match self.update(id, |snapshot| {
            snapshot.attempt = snapshot.attempt.saturating_add(1);
            snapshot.status = JobStatus::Queued;
            snapshot.progress = 0;
            snapshot.message = None;
            snapshot.retryable = true;
            snapshot.result = None;
            snapshot.policy = policy.bounded();
            snapshot.budget = budget.clone();
        }) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                if existing_budget.as_ref() != Some(&budget) {
                    self.release_budget(id);
                    if let Some(previous) = existing_budget.as_ref() {
                        let _ = self.reserve_budget(id, &current.provenance.owner_id, previous);
                    }
                }
                return Err(error);
            }
        };
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

    pub fn budget_usage(&self) -> JobBudgetUsage {
        self.inner
            .budget
            .lock()
            .map(|budget| budget.usage())
            .unwrap_or_default()
    }

    pub fn budget_pressure(&self, owner_id: &str) -> JobBudgetPressure {
        self.inner
            .budget
            .lock()
            .map(|budget| budget.pressure(owner_id))
            .unwrap_or_default()
    }

    pub fn execution_environment(&self) -> JobExecutionEnvironment {
        self.inner
            .environment
            .lock()
            .map(|environment| *environment)
            .unwrap_or_default()
    }

    /// Applies suspend/offline policy to all eligible Jobs. Running runners
    /// observe `Paused` through their context, persist a checkpoint and return
    /// `JobExecutionError::Paused`; late results are rejected by attempt/state
    /// validation.
    pub fn set_execution_environment(
        &self,
        environment: JobExecutionEnvironment,
    ) -> Result<Vec<JobSnapshot>, JobError> {
        {
            let mut current = self
                .inner
                .environment
                .lock()
                .map_err(|_| JobError::Store("Job environment lock poisoned".into()))?;
            *current = environment;
        }
        let mut changed = Vec::new();
        let mut to_resume = Vec::new();
        {
            let mut jobs = self
                .inner
                .jobs
                .lock()
                .map_err(|_| JobError::Store("job lock poisoned".into()))?;
            for snapshot in jobs.values_mut() {
                if matches!(snapshot.status, JobStatus::Queued | JobStatus::Running)
                    && self.should_pause(&snapshot.policy)
                {
                    snapshot.status = JobStatus::Paused;
                    snapshot.message = Some(self.pause_message(&snapshot.policy));
                    changed.push(snapshot.clone());
                } else if snapshot.status == JobStatus::Paused
                    && !self.should_pause(&snapshot.policy)
                {
                    // A paused runner may still be unwinding. Fence its late
                    // progress/results before starting a replacement attempt;
                    // an initially blocked Job has no active runner and keeps
                    // its attempt number.
                    if self.is_active(&snapshot.id, snapshot.attempt) {
                        snapshot.attempt = snapshot.attempt.saturating_add(1);
                    }
                    snapshot.status = JobStatus::Queued;
                    snapshot.message = Some("Resuming after environment recovery".into());
                    to_resume.push(snapshot.id.clone());
                    changed.push(snapshot.clone());
                }
            }
        }
        if !changed.is_empty() {
            self.persist()?;
            for snapshot in &changed {
                self.emit(snapshot.clone());
            }
        }
        for id in to_resume {
            self.spawn_worker(id);
        }
        Ok(changed)
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

    fn reserve_budget(
        &self,
        id: &JobId,
        owner_id: &str,
        request: &JobBudgetRequest,
    ) -> Result<(), JobError> {
        self.inner
            .budget
            .lock()
            .map_err(|_| JobError::Store("Job budget lock poisoned".into()))?
            .reserve(&id.0, owner_id, request)
            .map_err(|error| budget_error(owner_id, error))
    }

    fn release_budget(&self, id: &JobId) {
        if let Ok(mut budget) = self.inner.budget.lock() {
            budget.release(&id.0);
        }
    }

    fn mark_active(&self, id: &JobId, attempt: u32) {
        if let Ok(mut active) = self.inner.active_attempts.lock() {
            active.insert(id.clone(), attempt);
        }
    }

    fn mark_inactive(&self, id: &JobId, attempt: u32) {
        if let Ok(mut active) = self.inner.active_attempts.lock() {
            if active.get(id).is_some_and(|current| *current == attempt) {
                active.remove(id);
            }
        }
    }

    fn is_active(&self, id: &JobId, attempt: u32) -> bool {
        self.inner
            .active_attempts
            .lock()
            .ok()
            .and_then(|active| active.get(id).copied())
            .is_some_and(|current| current == attempt)
    }

    fn should_pause(&self, policy: &JobExecutionPolicy) -> bool {
        let environment = self.execution_environment();
        if environment.suspended && policy.background {
            return true;
        }
        match policy.network {
            JobNetworkPolicy::LocalOnly => false,
            JobNetworkPolicy::Unmetered => environment.network != JobNetworkState::Online,
            JobNetworkPolicy::AnyNetwork => environment.network == JobNetworkState::Offline,
        }
    }

    fn pause_message(&self, policy: &JobExecutionPolicy) -> String {
        let environment = self.execution_environment();
        if environment.suspended && policy.background {
            return "Paused while the device is suspended".into();
        }
        match environment.network {
            JobNetworkState::Offline => "Paused while the network is offline".into(),
            JobNetworkState::Metered => "Paused on a metered network".into(),
            JobNetworkState::Online => "Paused by execution policy".into(),
            JobNetworkState::Unknown => "Paused while network state is unknown".into(),
        }
    }

    pub fn save_checkpoint(
        &self,
        id: &JobId,
        attempt: u32,
        payload: Value,
    ) -> Result<Option<JobSnapshot>, JobError> {
        validate_json_size("Job checkpoint", &payload, MAX_JOB_CHECKPOINT_BYTES)?;
        let checkpoint = JobCheckpoint {
            schema_version: JOB_CHECKPOINT_SCHEMA_VERSION,
            payload,
        };
        let snapshot = {
            let mut jobs = self
                .inner
                .jobs
                .lock()
                .map_err(|_| JobError::Store("job lock poisoned".into()))?;
            let snapshot = jobs
                .get_mut(id)
                .ok_or_else(|| JobError::NotFound(id.clone()))?;
            if snapshot.attempt != attempt
                || !matches!(snapshot.status, JobStatus::Running | JobStatus::Paused)
            {
                return Ok(None);
            }
            snapshot.checkpoint = Some(checkpoint);
            snapshot.clone()
        };
        self.persist()?;
        self.emit(snapshot.clone());
        Ok(Some(snapshot))
    }

    fn spawn_cancellation_watchdog(&self, id: JobId, attempt: u32, grace_ms: u64) {
        let system = self.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(grace_ms));
            let _ = system.update_attempt_if(&id, attempt, |snapshot| {
                if snapshot.status == JobStatus::Cancelling {
                    snapshot.status = JobStatus::Cancelled;
                    snapshot.message = Some("Cancelled after the grace period".into());
                }
            });
        });
    }

    fn spawn_deadline_watchdog(
        &self,
        id: JobId,
        attempt: u32,
        deadline_ms: Option<u64>,
        grace_ms: u64,
    ) {
        let Some(deadline_ms) = deadline_ms else {
            return;
        };
        let system = self.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(deadline_ms));
            thread::sleep(Duration::from_millis(grace_ms));
            if system.snapshot(&id).is_some_and(|snapshot| {
                snapshot.attempt == attempt && snapshot.status == JobStatus::Running
            }) {
                // Route an unresponsive runner through the same bounded retry
                // policy as a cooperative timeout. Retrying fences the old
                // attempt before a replacement runner can start.
                system.schedule_retry(&id, attempt, "Job deadline exceeded".into());
            }
        });
    }

    fn schedule_retry(&self, id: &JobId, attempt: u32, message: String) {
        let Some(snapshot) = self.snapshot(id) else {
            return;
        };
        if snapshot.attempt != attempt || snapshot.status != JobStatus::Running {
            return;
        }
        let policy = snapshot.policy.retry.bounded();
        if attempt >= policy.max_attempts {
            let _ = self.fail_attempt(id, attempt, message);
            return;
        }
        let exponent = attempt.saturating_sub(1).min(31);
        let delay_ms = policy
            .initial_backoff_ms
            .saturating_mul(1u64 << exponent)
            .min(policy.max_backoff_ms);
        let _ = self.update_attempt(id, attempt, |snapshot| {
            snapshot.status = JobStatus::Retrying;
            snapshot.message = Some(format!("Retrying in {delay_ms}ms: {message}"));
            snapshot.retryable = true;
        });
        let system = self.clone();
        let id = id.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(delay_ms));
            let should_start = system
                .update_attempt_if(&id, attempt, |snapshot| {
                    if snapshot.status == JobStatus::Retrying {
                        if system.should_pause(&snapshot.policy) {
                            snapshot.status = JobStatus::Paused;
                            snapshot.message = Some(system.pause_message(&snapshot.policy));
                        } else {
                            snapshot.attempt = snapshot.attempt.saturating_add(1);
                            snapshot.progress = 0;
                            snapshot.status = JobStatus::Queued;
                            snapshot.message = Some("Retrying".into());
                        }
                    }
                })
                .ok()
                .flatten()
                .is_some_and(|snapshot| snapshot.status == JobStatus::Queued);
            if should_start {
                system.spawn_worker(id);
            }
        });
    }

    fn update_attempt_if<F>(
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
            if snapshot.attempt != attempt {
                return Ok(None);
            }
            update(snapshot);
            snapshot.clone()
        };
        self.persist()?;
        self.emit(snapshot.clone());
        if matches!(
            snapshot.status,
            JobStatus::Cancelled | JobStatus::Failed | JobStatus::Completed
        ) {
            self.release_budget(id);
        }
        Ok(Some(snapshot))
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
        if matches!(
            snapshot.status,
            JobStatus::Cancelled | JobStatus::Failed | JobStatus::Completed
        ) {
            self.release_budget(id);
        }
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
            let delay_ms = system
                .snapshot(&id)
                .map(|snapshot| {
                    system
                        .budget_pressure(&snapshot.provenance.owner_id)
                        .delay_ms
                })
                .unwrap_or_default();
            if delay_ms > 0 {
                thread::sleep(Duration::from_millis(delay_ms));
            }
            let Ok(Some(initial)) = system.start_attempt(&id) else {
                return;
            };
            let attempt = initial.attempt;
            system.mark_active(&id, attempt);
            let attempt_started = Instant::now();
            system.spawn_deadline_watchdog(
                id.clone(),
                attempt,
                initial.policy.deadline_ms,
                initial.policy.cancellation_grace_ms,
            );
            let Some(runner) = system.runner_for(&initial) else {
                system.mark_inactive(&id, attempt);
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
                deadline_ms: initial.policy.deadline_ms,
                cancellation_grace_ms: initial.policy.cancellation_grace_ms,
                checkpoint: initial.checkpoint.clone(),
                budget: initial.budget.clone(),
                deadline_at: initial
                    .policy
                    .deadline_ms
                    .map(|deadline| attempt_started + Duration::from_millis(deadline)),
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
                Ok(Err(JobExecutionError::Paused)) => {
                    let _ = system.update_attempt_if(&id, attempt, |snapshot| {
                        snapshot.status = JobStatus::Paused;
                        snapshot.message = Some(system.pause_message(&snapshot.policy));
                    });
                }
                Ok(Err(JobExecutionError::TimedOut)) => {
                    system.schedule_retry(&id, attempt, "Job deadline exceeded".into());
                }
                Ok(Err(JobExecutionError::Transient(message))) => {
                    system.schedule_retry(&id, attempt, message);
                }
                Ok(Err(JobExecutionError::Failed(message))) => {
                    let _ = system.fail_attempt(&id, attempt, message);
                }
                Err(_) => {
                    let _ = system.fail_attempt(&id, attempt, "Job runner panicked".into());
                }
            }
            system.mark_inactive(&id, attempt);
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
        let (snapshot, paused) = {
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
            if self.should_pause(&snapshot.policy) {
                snapshot.status = JobStatus::Paused;
                snapshot.message = Some(self.pause_message(&snapshot.policy));
                (snapshot.clone(), true)
            } else {
                snapshot.status = JobStatus::Running;
                snapshot.message = Some("Running".into());
                (snapshot.clone(), false)
            }
        };
        if paused {
            self.persist()?;
            self.emit(snapshot);
            return Ok(None);
        }
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
        if matches!(
            snapshot.status,
            JobStatus::Cancelled | JobStatus::Failed | JobStatus::Completed
        ) {
            self.release_budget(id);
        }
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
        self.update_attempt_if(id, attempt, |snapshot| {
            if matches!(snapshot.status, JobStatus::Running | JobStatus::Cancelling) {
                snapshot.status = JobStatus::Cancelled;
                snapshot.message = Some("Cancelled by runner".into());
                snapshot.retryable = true;
            }
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

    fn checkpoint(&self, payload: Value) -> Result<(), JobError> {
        self.system
            .save_checkpoint(&self.id, self.attempt, payload)
            .map(|_| ())
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
                || matches!(
                    snapshot.status,
                    JobStatus::Cancelling
                        | JobStatus::Cancelled
                        | JobStatus::Retrying
                        | JobStatus::Paused
                        | JobStatus::Interrupted
                        | JobStatus::Failed
                        | JobStatus::Completed
                )
        })
    }

    fn is_paused(&self) -> bool {
        self.system.snapshot(&self.id).is_some_and(|snapshot| {
            snapshot.attempt == self.attempt && snapshot.status == JobStatus::Paused
        })
    }
}

#[derive(Default)]
pub struct DeterministicJobRunner;

impl JobRunner for DeterministicJobRunner {
    fn run(&self, context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
        let steps = 20u8;
        let delay = Duration::from_millis((context.duration_ms / steps as u64).max(20));
        let starting_step = context
            .checkpoint()
            .and_then(|checkpoint| checkpoint.get("step"))
            .and_then(Value::as_u64)
            .map(|step| step.min(steps as u64) as u8)
            .unwrap_or(0);
        for step in starting_step.saturating_add(1)..=steps {
            thread::sleep(delay);
            if context.is_paused() {
                context
                    .save_checkpoint(json!({
                        "step": step,
                        "progress": step.saturating_mul(5).min(99),
                    }))
                    .map_err(|error| {
                        JobExecutionError::Failed(format!("checkpoint failed: {error:?}"))
                    })?;
                return Err(JobExecutionError::Paused);
            }
            if context.is_deadline_exceeded() {
                return Err(JobExecutionError::TimedOut);
            }
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

fn budget_error(owner_id: &str, rejection: BudgetRejection) -> JobError {
    JobError::BudgetExceeded {
        owner_id: owner_id.to_owned(),
        resource: rejection.resource,
        requested: rejection.requested,
        available: rejection.available,
    }
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
    use std::sync::atomic::{AtomicUsize, Ordering};
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

    struct HoldingRunner;

    impl JobRunner for HoldingRunner {
        fn run(
            &self,
            context: JobExecutionContext,
        ) -> Result<JobExecutionResult, JobExecutionError> {
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
        fn run(
            &self,
            context: JobExecutionContext,
        ) -> Result<JobExecutionResult, JobExecutionError> {
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
        fn run(
            &self,
            context: JobExecutionContext,
        ) -> Result<JobExecutionResult, JobExecutionError> {
            while !context.is_deadline_exceeded() {
                thread::sleep(Duration::from_millis(5));
            }
            Err(JobExecutionError::TimedOut)
        }
    }

    struct IgnoringDeadlineRunner;

    impl JobRunner for IgnoringDeadlineRunner {
        fn run(
            &self,
            _context: JobExecutionContext,
        ) -> Result<JobExecutionResult, JobExecutionError> {
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
        let jobs = JobSystem::with_store_and_budget_policy(
            Arc::new(InMemoryJobStateStore::default()),
            policy,
        );
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
        let jobs = JobSystem::with_store_and_budget_policy(
            Arc::new(InMemoryJobStateStore::default()),
            policy,
        );
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
        let jobs = JobSystem::with_store_and_budget_policy(
            Arc::new(InMemoryJobStateStore::default()),
            policy,
        );
        jobs.set_execution_environment(JobExecutionEnvironment {
            suspended: false,
            network: JobNetworkState::Offline,
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
}
