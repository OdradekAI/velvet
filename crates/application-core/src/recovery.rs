use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use velvet_persistence::{InMemoryRecoveryJournalStore, RecoveryJournalStore};
use velvet_workbench_protocol::{
    RecoveryAction, RecoveryCheckpoint, RecoveryFailure, RecoveryFailureClass,
    RecoveryJournalState, RecoveryLaunchReceipt, RecoveryMode, RecoveryProjection, StartupPhase,
};

const SAFE_MODE_FAILURE_THRESHOLD: u32 = 2;

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
        self.update(|inner, now| {
            if inner.state.active_launch.is_some() {
                inner.state.consecutive_failures =
                    inner.state.consecutive_failures.saturating_add(1);
                inner.state.last_failure = Some(RecoveryFailure {
                    class: RecoveryFailureClass::AbnormalExit,
                    phase: StartupPhase::Preflight,
                    message: "Previous launch ended before a completion checkpoint".into(),
                    at_ms: now,
                });
            }

            let attempt_id = inner.state.next_attempt_id.max(1);
            inner.state.next_attempt_id = attempt_id.saturating_add(1);
            inner.state.active_launch = Some(RecoveryLaunchReceipt {
                attempt_id,
                app_version: app_version.into(),
                phase: StartupPhase::Preflight,
                started_at_ms: now,
            });
            inner.state.shutdown_intent_at_ms = None;
            inner.state.shutdown_complete_at_ms = None;
        })
    }

    pub fn advance_to(&self, phase: StartupPhase) -> Result<RecoveryProjection, RecoveryError> {
        self.update(|inner, now| {
            let safe_mode = inner.state.safe_mode_forced
                || inner.state.consecutive_failures >= SAFE_MODE_FAILURE_THRESHOLD;
            let Some(launch) = inner.state.active_launch.as_mut() else {
                return;
            };
            launch.phase = phase.clone();
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
        self.update(|inner, now| {
            let Some(launch) = inner.state.active_launch.as_mut() else {
                return;
            };
            launch.phase = StartupPhase::Ready;
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
        self.update(|inner, now| {
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
            });
            inner.state.active_launch = None;
        })
    }

    pub fn record_shutdown_intent(&self) -> Result<RecoveryProjection, RecoveryError> {
        self.update(|inner, now| {
            inner.state.shutdown_intent_at_ms = Some(now);
        })
    }

    pub fn record_shutdown_complete(&self) -> Result<RecoveryProjection, RecoveryError> {
        self.update(|inner, now| {
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
                }),
                last_good: None,
                actions: vec![RecoveryAction::Retry, RecoveryAction::ExportDiagnostics],
            })
    }

    pub fn is_safe_mode(&self) -> bool {
        matches!(self.projection().mode, RecoveryMode::Safe)
    }

    fn update<F>(&self, update: F) -> Result<RecoveryProjection, RecoveryError>
    where
        F: FnOnce(&mut RecoveryInner, u64),
    {
        let now = self.clock.now_ms();
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| RecoveryError::InvalidState("recovery state lock poisoned".into()))?;
        update(&mut inner, now);
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
        }),
        ..RecoveryJournalState::default()
    }
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
    RecoveryProjection {
        schema_version: state.schema_version,
        mode,
        phase,
        attempt_id,
        consecutive_failures: state.consecutive_failures,
        last_failure: state.last_failure.clone(),
        last_good: state.last_good.clone(),
        actions,
    }
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
}
