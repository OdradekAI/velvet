use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use velvet_persistence::{InMemoryRecoveryJournalStore, RecoveryJournalStore};
use velvet_workbench_protocol::{
    CrashCaptureEvidence, PlatformPreflightEvidence, RecoveryAction, RecoveryCheckpoint,
    RecoveryDiagnosticBundle, RecoveryFailure, RecoveryFailureClass, RecoveryJournalState,
    RecoveryLaunchReceipt, RecoveryMode, RecoveryProjection, RecoveryTarget, StartupPhase,
    CRASH_CAPTURE_SCHEMA_VERSION, RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
};

const SAFE_MODE_FAILURE_THRESHOLD: u32 = 2;
const MAX_PLATFORM_PREFLIGHT_EVIDENCE: usize = 8;
const MAX_CRASH_CAPTURE_EVIDENCE: usize = 32;
const MAX_CRASH_EVIDENCE_TEXT: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupDeadlinePolicy {
    pub preflight_ms: u64,
    pub recovering_state_ms: u64,
    pub core_ready_ms: u64,
    pub workbench_ready_ms: u64,
    pub optional_activation_ms: u64,
}

impl Default for StartupDeadlinePolicy {
    fn default() -> Self {
        Self {
            preflight_ms: 10_000,
            recovering_state_ms: 30_000,
            core_ready_ms: 10_000,
            workbench_ready_ms: 10_000,
            optional_activation_ms: 30_000,
        }
    }
}

impl StartupDeadlinePolicy {
    fn deadline_at_ms(self, phase: &StartupPhase, started_at_ms: u64) -> Option<u64> {
        let timeout_ms = match phase {
            StartupPhase::Preflight => self.preflight_ms,
            StartupPhase::RecoveringState => self.recovering_state_ms,
            StartupPhase::CoreReady => self.core_ready_ms,
            StartupPhase::WorkbenchReady => self.workbench_ready_ms,
            StartupPhase::OptionalActivation => self.optional_activation_ms,
            StartupPhase::Ready => return None,
        };
        Some(started_at_ms.saturating_add(timeout_ms))
    }
}

pub trait RecoveryClock: Send + Sync {
    fn now_ms(&self) -> u64;
}

#[derive(Default)]
pub struct SystemRecoveryClock;

impl RecoveryClock for SystemRecoveryClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryError {
    Store(String),
    InvalidState(String),
}

/// Platform adapter seam for crash evidence. The adapter only reads bounded,
/// already-local evidence; it never decides Safe Mode or owns domain state.
pub trait CrashCaptureAdapter: Send + Sync {
    fn consume(&self) -> Result<Vec<CrashCaptureEvidence>, RecoveryError>;
}

struct RecoveryInner {
    state: RecoveryJournalState,
    journal_degraded: bool,
}

/// Application-owned lifecycle policy and evidence boundary.
///
/// The supervisor deliberately does not spawn a helper process and does not
/// own domain payloads. It only records launch evidence, chooses Safe Mode and
/// exposes user recovery actions. The journal adapter may be file-backed in
/// the native app or in-memory in tests/embedders.
pub struct RecoverySupervisor {
    journal: Arc<dyn RecoveryJournalStore>,
    clock: Arc<dyn RecoveryClock>,
    policy: StartupDeadlinePolicy,
    inner: Mutex<RecoveryInner>,
}

impl Default for RecoverySupervisor {
    fn default() -> Self {
        Self::new(
            Arc::new(InMemoryRecoveryJournalStore::default()),
            Arc::new(SystemRecoveryClock),
        )
    }
}

impl RecoverySupervisor {
    pub fn new(journal: Arc<dyn RecoveryJournalStore>, clock: Arc<dyn RecoveryClock>) -> Self {
        Self::with_policy(journal, clock, StartupDeadlinePolicy::default())
    }

    pub fn with_policy(
        journal: Arc<dyn RecoveryJournalStore>,
        clock: Arc<dyn RecoveryClock>,
        policy: StartupDeadlinePolicy,
    ) -> Self {
        let (state, journal_degraded) = match journal.load() {
            Ok(state)
                if state.schema_version == velvet_workbench_protocol::RECOVERY_SCHEMA_VERSION =>
            {
                (state, false)
            }
            Err(_) => (
                unavailable_state("Lifecycle journal was unavailable; using an in-memory receipt"),
                true,
            ),
            Ok(_) => (
                unavailable_state(
                    "Lifecycle journal schema is unsupported; preserving the original bytes",
                ),
                true,
            ),
        };

        Self {
            journal,
            clock,
            policy,
            inner: Mutex::new(RecoveryInner {
                state,
                journal_degraded,
            }),
        }
    }

    pub fn begin_launch(
        &self,
        app_version: impl Into<String>,
    ) -> Result<RecoveryProjection, RecoveryError> {
        let policy = self.policy;
        self.update_without_deadline(|inner, now| {
            if inner.state.active_launch.is_some() {
                inner.state.consecutive_failures =
                    inner.state.consecutive_failures.saturating_add(1);
                inner.state.last_failure = Some(RecoveryFailure {
                    class: RecoveryFailureClass::AbnormalExit,
                    phase: StartupPhase::Preflight,
                    message: "Previous launch ended before a completion checkpoint".into(),
                    at_ms: now,
                    target: None,
                });
            }

            let attempt_id = inner.state.next_attempt_id.max(1);
            inner.state.next_attempt_id = attempt_id.saturating_add(1);
            inner.state.active_launch = Some(RecoveryLaunchReceipt {
                attempt_id,
                app_version: app_version.into(),
                phase: StartupPhase::Preflight,
                started_at_ms: now,
                instance_id: Some(format!("pid-{}-{}", std::process::id(), now)),
                process_id: Some(std::process::id()),
                phase_started_at_ms: Some(now),
                deadline_at_ms: policy.deadline_at_ms(&StartupPhase::Preflight, now),
            });
            inner.state.shutdown_intent_at_ms = None;
            inner.state.shutdown_complete_at_ms = None;
        })
    }

    pub fn advance_to(&self, phase: StartupPhase) -> Result<RecoveryProjection, RecoveryError> {
        let policy = self.policy;
        self.update_with_deadline(|inner, now| {
            let safe_mode = inner.state.safe_mode_forced
                || inner.state.consecutive_failures >= SAFE_MODE_FAILURE_THRESHOLD;
            let Some(launch) = inner.state.active_launch.as_mut() else {
                return;
            };
            launch.phase = phase.clone();
            launch.phase_started_at_ms = Some(now);
            launch.deadline_at_ms = policy.deadline_at_ms(&phase, now);
            if !safe_mode && (phase == StartupPhase::WorkbenchReady || phase == StartupPhase::Ready)
            {
                inner.state.last_good = Some(RecoveryCheckpoint {
                    attempt_id: launch.attempt_id,
                    phase,
                    at_ms: now,
                });
                inner.state.consecutive_failures = 0;
            }
        })
    }

    pub fn mark_ready(&self) -> Result<RecoveryProjection, RecoveryError> {
        self.update_with_deadline(|inner, now| {
            let Some(launch) = inner.state.active_launch.as_mut() else {
                return;
            };
            launch.phase = StartupPhase::Ready;
            launch.phase_started_at_ms = Some(now);
            launch.deadline_at_ms = None;
            inner.state.last_good = Some(RecoveryCheckpoint {
                attempt_id: launch.attempt_id,
                phase: StartupPhase::Ready,
                at_ms: now,
            });
            inner.state.consecutive_failures = 0;
            inner.state.active_launch = None;
            inner.state.shutdown_intent_at_ms = None;
            inner.state.shutdown_complete_at_ms = None;
        })
    }

    pub fn record_failure(
        &self,
        class: RecoveryFailureClass,
        message: impl Into<String>,
    ) -> Result<RecoveryProjection, RecoveryError> {
        self.record_failure_with_target(class, message, None)
    }

    pub fn record_failure_for_owner(
        &self,
        class: RecoveryFailureClass,
        message: impl Into<String>,
        owner_id: impl Into<String>,
        scope: impl Into<String>,
        generation: u64,
    ) -> Result<RecoveryProjection, RecoveryError> {
        self.record_failure_with_target(
            class,
            message,
            Some(RecoveryTarget {
                owner_id: owner_id.into(),
                scope: scope.into(),
                generation,
            }),
        )
    }

    fn record_failure_with_target(
        &self,
        class: RecoveryFailureClass,
        message: impl Into<String>,
        target: Option<RecoveryTarget>,
    ) -> Result<RecoveryProjection, RecoveryError> {
        self.update_without_deadline(|inner, now| {
            let phase = inner
                .state
                .active_launch
                .as_ref()
                .map(|launch| launch.phase.clone())
                .unwrap_or(StartupPhase::Preflight);
            if matches!(
                class,
                RecoveryFailureClass::AbnormalExit
                    | RecoveryFailureClass::StartupTimeout
                    | RecoveryFailureClass::CoreMigration
                    | RecoveryFailureClass::StateCorrupt
                    | RecoveryFailureClass::PlatformInitialization
            ) && phase != StartupPhase::WorkbenchReady
                && phase != StartupPhase::Ready
            {
                inner.state.consecutive_failures =
                    inner.state.consecutive_failures.saturating_add(1);
            }
            if class == RecoveryFailureClass::CoreMigration {
                inner.state.safe_mode_forced = true;
            }
            inner.state.last_failure = Some(RecoveryFailure {
                class,
                phase,
                message: message.into(),
                at_ms: now,
                target,
            });
            inner.state.active_launch = None;
        })
    }

    pub fn record_platform_preflight(
        &self,
        evidence: Vec<PlatformPreflightEvidence>,
    ) -> Result<RecoveryProjection, RecoveryError> {
        self.update_without_deadline(|inner, _| {
            inner.state.platform_preflight = evidence
                .into_iter()
                .take(MAX_PLATFORM_PREFLIGHT_EVIDENCE)
                .map(normalize_platform_preflight)
                .collect();
        })
    }

    pub fn record_crash_evidence(
        &self,
        evidence: CrashCaptureEvidence,
    ) -> Result<RecoveryProjection, RecoveryError> {
        self.update_without_deadline(|inner, _| {
            if evidence.schema_version != CRASH_CAPTURE_SCHEMA_VERSION {
                return;
            }
            inner
                .state
                .crash_evidence
                .push(normalize_crash_evidence(evidence));
            if inner.state.crash_evidence.len() > MAX_CRASH_CAPTURE_EVIDENCE {
                let remove = inner.state.crash_evidence.len() - MAX_CRASH_CAPTURE_EVIDENCE;
                inner.state.crash_evidence.drain(0..remove);
            }
        })
    }

    pub fn launch_identity(&self) -> Option<(String, u64)> {
        self.inner.lock().ok().and_then(|inner| {
            inner.state.active_launch.as_ref().and_then(|launch| {
                launch
                    .instance_id
                    .clone()
                    .map(|instance_id| (instance_id, launch.attempt_id))
            })
        })
    }

    pub fn observe_deadline(&self) -> Result<RecoveryProjection, RecoveryError> {
        self.update_with_deadline(|_, _| {})
    }

    pub fn diagnostic_bundle(&self) -> RecoveryDiagnosticBundle {
        let mut projection = self.projection();
        if let Some(failure) = projection.last_failure.as_mut() {
            failure.message = failure.message.chars().take(512).collect();
        }
        RecoveryDiagnosticBundle {
            schema_version: RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
            exported_at_ms: self.clock.now_ms(),
            projection,
            plugin_diagnostics: Vec::new(),
            diagnostics_degraded: false,
            crash_evidence: self
                .inner
                .lock()
                .map(|inner| inner.state.crash_evidence.clone())
                .unwrap_or_default(),
        }
    }

    pub fn record_shutdown_intent(&self) -> Result<RecoveryProjection, RecoveryError> {
        self.update_without_deadline(|inner, now| {
            inner.state.shutdown_intent_at_ms = Some(now);
        })
    }

    pub fn record_shutdown_complete(&self) -> Result<RecoveryProjection, RecoveryError> {
        self.update_without_deadline(|inner, now| {
            inner.state.shutdown_complete_at_ms = Some(now);
            inner.state.active_launch = None;
        })
    }

    pub fn projection(&self) -> RecoveryProjection {
        self.inner
            .lock()
            .map(|inner| projection(&inner))
            .unwrap_or_else(|_| RecoveryProjection {
                schema_version: velvet_workbench_protocol::RECOVERY_SCHEMA_VERSION,
                mode: RecoveryMode::Safe,
                phase: StartupPhase::Preflight,
                attempt_id: 0,
                consecutive_failures: SAFE_MODE_FAILURE_THRESHOLD,
                last_failure: Some(RecoveryFailure {
                    class: RecoveryFailureClass::JournalUnavailable,
                    phase: StartupPhase::Preflight,
                    message: "Lifecycle state lock was unavailable".into(),
                    at_ms: 0,
                    target: None,
                }),
                last_good: None,
                actions: vec![RecoveryAction::Retry, RecoveryAction::ExportDiagnostics],
                platform_preflight: Vec::new(),
            })
    }

    pub fn is_safe_mode(&self) -> bool {
        matches!(self.projection().mode, RecoveryMode::Safe)
    }

    fn update_without_deadline<F>(&self, update: F) -> Result<RecoveryProjection, RecoveryError>
    where
        F: FnOnce(&mut RecoveryInner, u64),
    {
        self.update_with_deadline_inner(false, update)
    }

    fn update_with_deadline<F>(&self, update: F) -> Result<RecoveryProjection, RecoveryError>
    where
        F: FnOnce(&mut RecoveryInner, u64),
    {
        self.update_with_deadline_inner(true, update)
    }

    fn update_with_deadline_inner<F>(
        &self,
        enforce_deadline: bool,
        update: F,
    ) -> Result<RecoveryProjection, RecoveryError>
    where
        F: FnOnce(&mut RecoveryInner, u64),
    {
        let now = self.clock.now_ms();
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| RecoveryError::InvalidState("recovery state lock poisoned".into()))?;
        if enforce_deadline {
            if let Some(phase) = inner
                .state
                .active_launch
                .as_ref()
                .filter(|launch| launch.deadline_at_ms.is_some_and(|deadline| now > deadline))
                .map(|launch| launch.phase.clone())
            {
                record_startup_timeout(&mut inner.state, phase, now);
            } else {
                update(&mut inner, now);
            }
        } else {
            update(&mut inner, now);
        }
        if inner.journal_degraded {
            return Ok(projection(&inner));
        }
        if let Err(error) = self.journal.save(&inner.state) {
            inner.journal_degraded = true;
            inner.state.safe_mode_forced = true;
            inner.state.last_failure = Some(RecoveryFailure {
                class: RecoveryFailureClass::JournalUnavailable,
                phase: inner
                    .state
                    .active_launch
                    .as_ref()
                    .map(|launch| launch.phase.clone())
                    .unwrap_or(StartupPhase::Preflight),
                message: format!("Lifecycle journal could not be written: {error:?}"),
                at_ms: now,
                target: None,
            });
            return Err(RecoveryError::Store(format!("{error:?}")));
        }
        Ok(projection(&inner))
    }
}

fn unavailable_state(message: impl Into<String>) -> RecoveryJournalState {
    RecoveryJournalState {
        safe_mode_forced: true,
        last_failure: Some(RecoveryFailure {
            class: RecoveryFailureClass::JournalUnavailable,
            phase: StartupPhase::Preflight,
            message: message.into(),
            at_ms: 0,
            target: None,
        }),
        ..RecoveryJournalState::default()
    }
}

fn record_startup_timeout(state: &mut RecoveryJournalState, phase: StartupPhase, at_ms: u64) {
    let core_phase = !matches!(
        phase,
        StartupPhase::WorkbenchReady | StartupPhase::Ready | StartupPhase::OptionalActivation
    );
    if core_phase {
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        state.safe_mode_forced = true;
    }
    state.last_failure = Some(RecoveryFailure {
        class: RecoveryFailureClass::StartupTimeout,
        phase,
        message: "Startup phase exceeded its monotonic deadline".into(),
        at_ms,
        target: None,
    });
    state.active_launch = None;
}

fn projection(inner: &RecoveryInner) -> RecoveryProjection {
    let state = &inner.state;
    let (phase, attempt_id) = state
        .active_launch
        .as_ref()
        .map(|launch| (launch.phase.clone(), launch.attempt_id))
        .or_else(|| {
            state
                .last_good
                .as_ref()
                .map(|checkpoint| (checkpoint.phase.clone(), checkpoint.attempt_id))
        })
        .unwrap_or((StartupPhase::Preflight, 0));
    let safe = state.safe_mode_forced || state.consecutive_failures >= SAFE_MODE_FAILURE_THRESHOLD;
    let mode = if safe {
        RecoveryMode::Safe
    } else if inner.journal_degraded || state.last_failure.is_some() {
        if state.active_launch.is_some() {
            RecoveryMode::Starting
        } else {
            RecoveryMode::Degraded
        }
    } else if state.active_launch.is_some() {
        RecoveryMode::Starting
    } else {
        RecoveryMode::Ready
    };

    let mut actions = vec![RecoveryAction::Retry, RecoveryAction::ExportDiagnostics];
    if safe {
        actions.extend([
            RecoveryAction::DisableRecentComponent,
            RecoveryAction::RestoreSnapshot,
            RecoveryAction::ResetOptionalState,
        ]);
    }
    if state
        .last_failure
        .as_ref()
        .and_then(|failure| failure.target.as_ref())
        .is_some()
    {
        actions.push(RecoveryAction::QuarantineState);
    }
    RecoveryProjection {
        schema_version: state.schema_version,
        mode,
        phase,
        attempt_id,
        consecutive_failures: state.consecutive_failures,
        last_failure: state.last_failure.clone(),
        last_good: state.last_good.clone(),
        actions,
        platform_preflight: state.platform_preflight.clone(),
    }
}

fn normalize_platform_preflight(
    mut evidence: PlatformPreflightEvidence,
) -> PlatformPreflightEvidence {
    evidence.detail = bounded_text(&evidence.detail, MAX_CRASH_EVIDENCE_TEXT);
    evidence
}

fn normalize_crash_evidence(mut evidence: CrashCaptureEvidence) -> CrashCaptureEvidence {
    evidence.correlation_id = bounded_text(&evidence.correlation_id, 128);
    evidence.component = bounded_text(&evidence.component, 128);
    evidence.code = bounded_text(&evidence.code, 128);
    evidence.message = bounded_text(&evidence.message, MAX_CRASH_EVIDENCE_TEXT);
    evidence.runtime_nonce = evidence
        .runtime_nonce
        .map(|value| bounded_text(&value, 128));
    evidence.log_tail = evidence
        .log_tail
        .map(|value| bounded_text(&value, MAX_CRASH_EVIDENCE_TEXT));
    evidence.artifact = evidence.artifact.map(|mut artifact| {
        artifact.artifact_id = bounded_text(&artifact.artifact_id, 128);
        artifact.format = bounded_text(&artifact.format, 32);
        artifact.sha256 = bounded_text(&artifact.sha256, 128);
        artifact
    });
    evidence
}

fn bounded_text(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestClock(AtomicU64);

    impl RecoveryClock for TestClock {
        fn now_ms(&self) -> u64 {
            self.0.fetch_add(1, Ordering::Relaxed)
        }
    }

    #[test]
    fn two_unfinished_launches_enter_safe_mode_without_resetting_state() {
        let journal = Arc::new(InMemoryRecoveryJournalStore::default());
        let clock = Arc::new(TestClock(AtomicU64::new(1)));
        let first = RecoverySupervisor::new(journal.clone(), clock.clone());
        first
            .begin_launch("test")
            .expect("first launch should start");
        drop(first);

        let second = RecoverySupervisor::new(journal.clone(), clock.clone());
        let projection = second
            .begin_launch("test")
            .expect("second launch should start");
        assert_eq!(projection.mode, RecoveryMode::Starting);
        drop(second);

        let third = RecoverySupervisor::new(journal, clock);
        let projection = third
            .begin_launch("test")
            .expect("third launch should start");
        assert_eq!(projection.mode, RecoveryMode::Safe);
        assert_eq!(projection.consecutive_failures, 2);
        assert!(projection
            .actions
            .contains(&RecoveryAction::RestoreSnapshot));
    }

    #[test]
    fn workbench_checkpoint_clears_pre_ready_failure_count() {
        let journal = Arc::new(InMemoryRecoveryJournalStore::default());
        let clock = Arc::new(TestClock(AtomicU64::new(1)));
        let supervisor = RecoverySupervisor::new(journal, clock);
        supervisor
            .begin_launch("test")
            .expect("launch should start");
        supervisor
            .record_failure(RecoveryFailureClass::StartupTimeout, "timeout")
            .expect("failure should be recorded");
        supervisor.begin_launch("test").expect("retry should start");
        let projection = supervisor
            .advance_to(StartupPhase::WorkbenchReady)
            .expect("workbench checkpoint should persist");
        assert_eq!(projection.consecutive_failures, 0);
        assert_eq!(
            projection.last_good.as_ref().map(|value| &value.phase),
            Some(&StartupPhase::WorkbenchReady)
        );
    }

    #[test]
    fn completed_shutdown_closes_the_launch_receipt_for_the_next_start() {
        let journal = Arc::new(InMemoryRecoveryJournalStore::default());
        let clock = Arc::new(TestClock(AtomicU64::new(1)));
        let first = RecoverySupervisor::new(journal.clone(), clock.clone());
        first.begin_launch("test").expect("launch should start");
        first
            .record_shutdown_intent()
            .expect("shutdown intent should persist");
        let projection = first
            .record_shutdown_complete()
            .expect("shutdown completion should persist");
        assert_eq!(projection.mode, RecoveryMode::Ready);

        let second = RecoverySupervisor::new(journal, clock);
        let projection = second
            .begin_launch("test")
            .expect("next launch should start");
        assert_eq!(projection.consecutive_failures, 0);
        assert_eq!(projection.mode, RecoveryMode::Starting);
    }

    #[test]
    fn safe_mode_remains_active_until_an_explicit_recovery_action() {
        let journal = Arc::new(InMemoryRecoveryJournalStore::default());
        let clock = Arc::new(TestClock(AtomicU64::new(1)));
        let first = RecoverySupervisor::new(journal.clone(), clock.clone());
        first.begin_launch("test").expect("launch should start");
        drop(first);
        let second = RecoverySupervisor::new(journal.clone(), clock.clone());
        second.begin_launch("test").expect("retry should start");
        drop(second);

        let third = RecoverySupervisor::new(journal, clock);
        let projection = third
            .begin_launch("test")
            .expect("safe launch should start");
        assert_eq!(projection.mode, RecoveryMode::Safe);
        let after_checkpoint = third
            .advance_to(StartupPhase::WorkbenchReady)
            .expect("safe mode should still expose a projection");
        assert_eq!(after_checkpoint.mode, RecoveryMode::Safe);
        assert_eq!(after_checkpoint.consecutive_failures, 2);
    }

    #[test]
    fn startup_deadlines_are_typed_and_optional_activation_timeout_stays_degraded() {
        let journal = Arc::new(InMemoryRecoveryJournalStore::default());
        let clock = Arc::new(TestClock(AtomicU64::new(1)));
        let policy = StartupDeadlinePolicy {
            preflight_ms: 100,
            recovering_state_ms: 100,
            core_ready_ms: 100,
            workbench_ready_ms: 100,
            optional_activation_ms: 0,
        };
        let supervisor = RecoverySupervisor::with_policy(journal, clock, policy);
        supervisor
            .begin_launch("test")
            .expect("launch should start");
        supervisor
            .advance_to(StartupPhase::CoreReady)
            .expect("core should become ready");
        supervisor
            .advance_to(StartupPhase::WorkbenchReady)
            .expect("workbench should become ready");
        supervisor
            .advance_to(StartupPhase::OptionalActivation)
            .expect("optional activation should begin");

        let projection = supervisor
            .mark_ready()
            .expect("deadline breach should remain diagnosable");
        assert_eq!(projection.mode, RecoveryMode::Degraded);
        assert_eq!(projection.consecutive_failures, 0);
        assert_eq!(
            projection
                .last_failure
                .as_ref()
                .map(|failure| failure.class.clone()),
            Some(RecoveryFailureClass::StartupTimeout)
        );
        assert_eq!(
            projection
                .last_failure
                .as_ref()
                .map(|failure| failure.phase.clone()),
            Some(StartupPhase::OptionalActivation)
        );
    }

    #[test]
    fn core_startup_deadline_breach_increments_safe_mode_failures() {
        let journal = Arc::new(InMemoryRecoveryJournalStore::default());
        let clock = Arc::new(TestClock(AtomicU64::new(1)));
        let policy = StartupDeadlinePolicy {
            preflight_ms: 0,
            ..StartupDeadlinePolicy::default()
        };
        let supervisor = RecoverySupervisor::with_policy(journal, clock, policy);
        supervisor
            .begin_launch("test")
            .expect("launch should start");
        let projection = supervisor
            .advance_to(StartupPhase::CoreReady)
            .expect("deadline breach should be recorded");
        assert_eq!(projection.mode, RecoveryMode::Safe);
        assert_eq!(projection.consecutive_failures, 1);
        assert_eq!(
            projection.last_failure.map(|failure| failure.class),
            Some(RecoveryFailureClass::StartupTimeout)
        );
    }

    #[test]
    fn platform_initialization_failures_enter_safe_mode_after_two_attempts() {
        let journal = Arc::new(InMemoryRecoveryJournalStore::default());
        let clock = Arc::new(TestClock(AtomicU64::new(1)));
        let supervisor = RecoverySupervisor::new(journal, clock);
        supervisor
            .begin_launch("test")
            .expect("first launch should start");
        supervisor
            .record_failure(
                RecoveryFailureClass::PlatformInitialization,
                "Tauri builder failed",
            )
            .expect("the first platform failure should persist");
        supervisor.begin_launch("test").expect("retry should start");
        let projection = supervisor
            .record_failure(
                RecoveryFailureClass::PlatformInitialization,
                "WebView initialization failed",
            )
            .expect("the second platform failure should persist");
        assert_eq!(projection.mode, RecoveryMode::Safe);
        assert_eq!(projection.consecutive_failures, 2);
        assert_eq!(
            projection.last_failure.map(|failure| failure.class),
            Some(RecoveryFailureClass::PlatformInitialization)
        );
    }

    #[test]
    fn platform_and_crash_evidence_are_bounded_and_exported_with_correlation() {
        let journal = Arc::new(InMemoryRecoveryJournalStore::default());
        let clock = Arc::new(TestClock(AtomicU64::new(1)));
        let supervisor = RecoverySupervisor::new(journal, clock);
        supervisor
            .begin_launch("test")
            .expect("launch should start");
        let projection = supervisor
            .record_platform_preflight(vec![PlatformPreflightEvidence {
                capability: velvet_workbench_protocol::PlatformCapability::Webview,
                status: velvet_workbench_protocol::PlatformCapabilityStatus::Unavailable,
                detail: "x".repeat(1_000),
                checked_at_ms: 2,
            }])
            .expect("platform evidence should persist");
        assert_eq!(projection.platform_preflight[0].detail.chars().count(), 512);
        supervisor
            .record_crash_evidence(CrashCaptureEvidence {
                schema_version: CRASH_CAPTURE_SCHEMA_VERSION,
                correlation_id: "instance-1".into(),
                process_id: 42,
                parent_process_id: None,
                attempt_id: 1,
                component: "core".into(),
                kind: velvet_workbench_protocol::CrashCaptureKind::ProcessExit,
                code: "signal".into(),
                message: "forced termination".into(),
                runtime_nonce: None,
                log_tail: Some("tail".into()),
                artifact: None,
                occurred_at_ms: 3,
            })
            .expect("crash evidence should persist");
        let bundle = supervisor.diagnostic_bundle();
        assert_eq!(bundle.crash_evidence.len(), 1);
        assert_eq!(bundle.crash_evidence[0].correlation_id, "instance-1");
    }
}
