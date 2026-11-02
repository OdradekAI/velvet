use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use velvet_persistence::{OperationReceiptStore, PersistenceError};
use velvet_workbench_protocol::{
    OperationItemReceipt, OperationPlan, OperationPlanError, OperationReceipt,
    OperationReceiptStatus, OperationTarget, OperationUndoCapability, OPERATION_SCHEMA_VERSION,
};

/// The point in the provider lifecycle where an operation failed or became
/// ambiguous. Keeping this explicit prevents an execution failure from being
/// mistaken for a validation failure after a receipt has been started.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationPhase {
    Observe,
    Execute,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationProviderError {
    Failed(String),
    Unknown(String),
}

/// A provider's final outcome. Providers return per-target receipts so the
/// host can preserve partial results without interpreting provider semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationExecution {
    pub status: OperationReceiptStatus,
    pub items: Vec<OperationItemReceipt>,
    pub undo_token: Option<String>,
}

impl OperationExecution {
    pub fn committed(items: Vec<OperationItemReceipt>, undo_token: Option<String>) -> Self {
        Self {
            status: OperationReceiptStatus::Committed,
            items,
            undo_token,
        }
    }

    pub fn partial(items: Vec<OperationItemReceipt>, undo_token: Option<String>) -> Self {
        Self {
            status: OperationReceiptStatus::Partial,
            items,
            undo_token,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationCoordinatorError {
    Plan(OperationPlanError),
    Persistence(PersistenceError),
    AlreadyRecorded {
        operation_id: String,
        status: OperationReceiptStatus,
    },
    ReconciliationRequired {
        operation_id: String,
    },
    ProviderFailed {
        phase: OperationPhase,
        message: String,
    },
    ProviderUnknown {
        phase: OperationPhase,
        message: String,
    },
    InvalidExecution(String),
}

/// Provider-owned observation and side-effect seam.
///
/// The provider supplies current target evidence and executes an already
/// validated plan. It never receives a persistence handle or an unrestricted
/// native command; the host controls the receipt transitions around it.
pub trait OperationProvider {
    fn observe(
        &mut self,
        plan: &OperationPlan,
    ) -> Result<Vec<OperationTarget>, OperationProviderError>;

    fn execute(
        &mut self,
        plan: &OperationPlan,
    ) -> Result<OperationExecution, OperationProviderError>;
}

#[derive(Clone)]
pub struct OperationCoordinator {
    receipts: Arc<dyn OperationReceiptStore>,
    gate: Arc<Mutex<()>>,
}

impl OperationCoordinator {
    pub fn new(receipts: Arc<dyn OperationReceiptStore>) -> Self {
        Self {
            receipts,
            gate: Arc::new(Mutex::new(())),
        }
    }

    pub fn load_receipts(&self) -> Result<Vec<OperationReceipt>, OperationCoordinatorError> {
        let _gate = self
            .gate
            .lock()
            .map_err(|_| OperationCoordinatorError::Persistence(PersistenceError::Unavailable))?;
        self.receipts
            .load()
            .map_err(OperationCoordinatorError::Persistence)
    }

    pub fn commit<P: OperationProvider>(
        &self,
        plan: &OperationPlan,
        now_ms: u64,
        provider: &mut P,
    ) -> Result<OperationReceipt, OperationCoordinatorError> {
        let _gate = self
            .gate
            .lock()
            .map_err(|_| OperationCoordinatorError::Persistence(PersistenceError::Unavailable))?;
        plan.validate(now_ms)
            .map_err(OperationCoordinatorError::Plan)?;
        self.reject_replay(&plan.operation_id)?;

        let observed = provider
            .observe(plan)
            .map_err(|error| provider_error(OperationPhase::Observe, error))?;
        plan.revalidate(now_ms, &observed)
            .map_err(OperationCoordinatorError::Plan)?;

        let started = started_receipt(plan, now_ms);
        self.append_receipt(started)?;

        match provider.execute(plan) {
            Ok(execution) => {
                validate_execution(plan, &execution)?;
                let receipt = OperationReceipt {
                    schema_version: OPERATION_SCHEMA_VERSION,
                    plan_id: plan.plan_id.clone(),
                    operation_id: plan.operation_id.clone(),
                    status: execution.status,
                    reconciled: true,
                    recorded_at_ms: now_ms,
                    items: execution.items,
                    undo_token: execution.undo_token,
                };
                self.append_receipt(receipt.clone())?;
                Ok(receipt)
            }
            Err(OperationProviderError::Unknown(message)) => {
                let receipt = unknown_receipt(plan, now_ms, message.clone());
                self.append_receipt(receipt)?;
                Err(OperationCoordinatorError::ProviderUnknown {
                    phase: OperationPhase::Execute,
                    message,
                })
            }
            Err(OperationProviderError::Failed(message)) => {
                // The started receipt remains authoritative until the owner
                // can query whether the external effect happened.
                Err(OperationCoordinatorError::ProviderFailed {
                    phase: OperationPhase::Execute,
                    message,
                })
            }
        }
    }

    fn reject_replay(&self, operation_id: &str) -> Result<(), OperationCoordinatorError> {
        let receipts = self
            .receipts
            .load()
            .map_err(OperationCoordinatorError::Persistence)?;
        if let Some(receipt) = receipts
            .into_iter()
            .find(|receipt| receipt.operation_id == operation_id)
        {
            if receipt.retry_requires_reconciliation() {
                return Err(OperationCoordinatorError::ReconciliationRequired {
                    operation_id: operation_id.into(),
                });
            }
            return Err(OperationCoordinatorError::AlreadyRecorded {
                operation_id: operation_id.into(),
                status: receipt.status,
            });
        }
        Ok(())
    }

    fn append_receipt(&self, receipt: OperationReceipt) -> Result<(), OperationCoordinatorError> {
        let mut receipts = self
            .receipts
            .load()
            .map_err(OperationCoordinatorError::Persistence)?;
        if let Some(existing) = receipts
            .iter_mut()
            .find(|existing| existing.operation_id == receipt.operation_id)
        {
            *existing = receipt;
        } else {
            receipts.push(receipt);
        }
        self.receipts
            .save(&receipts)
            .map_err(OperationCoordinatorError::Persistence)
    }
}

fn provider_error(
    phase: OperationPhase,
    error: OperationProviderError,
) -> OperationCoordinatorError {
    match error {
        OperationProviderError::Failed(message) => {
            OperationCoordinatorError::ProviderFailed { phase, message }
        }
        OperationProviderError::Unknown(message) => {
            OperationCoordinatorError::ProviderUnknown { phase, message }
        }
    }
}

fn started_receipt(plan: &OperationPlan, now_ms: u64) -> OperationReceipt {
    OperationReceipt {
        schema_version: OPERATION_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        operation_id: plan.operation_id.clone(),
        status: OperationReceiptStatus::Started,
        reconciled: false,
        recorded_at_ms: now_ms,
        items: plan
            .targets
            .iter()
            .map(|target| OperationItemReceipt {
                target_id: target.target_id.clone(),
                status: OperationReceiptStatus::Started,
                message: None,
            })
            .collect(),
        undo_token: None,
    }
}

fn unknown_receipt(plan: &OperationPlan, now_ms: u64, message: String) -> OperationReceipt {
    OperationReceipt {
        schema_version: OPERATION_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        operation_id: plan.operation_id.clone(),
        status: OperationReceiptStatus::Unknown,
        reconciled: false,
        recorded_at_ms: now_ms,
        items: plan
            .targets
            .iter()
            .map(|target| OperationItemReceipt {
                target_id: target.target_id.clone(),
                status: OperationReceiptStatus::Unknown,
                message: Some(message.clone()),
            })
            .collect(),
        undo_token: None,
    }
}

fn validate_execution(
    plan: &OperationPlan,
    execution: &OperationExecution,
) -> Result<(), OperationCoordinatorError> {
    if matches!(
        execution.status,
        OperationReceiptStatus::Started | OperationReceiptStatus::Unknown
    ) {
        return Err(OperationCoordinatorError::InvalidExecution(
            "provider must return a final operation status".into(),
        ));
    }
    if execution.items.iter().any(|item| {
        matches!(
            item.status,
            OperationReceiptStatus::Started | OperationReceiptStatus::Unknown
        )
    }) {
        return Err(OperationCoordinatorError::InvalidExecution(
            "final provider receipt cannot contain ambiguous item outcomes".into(),
        ));
    }
    if matches!(plan.undo, OperationUndoCapability::None) && execution.undo_token.is_some() {
        return Err(OperationCoordinatorError::InvalidExecution(
            "provider returned an undo token for a plan without undo capability".into(),
        ));
    }
    let expected = plan
        .targets
        .iter()
        .map(|target| target.target_id.as_str())
        .collect::<BTreeSet<_>>();
    let actual = execution
        .items
        .iter()
        .map(|item| item.target_id.as_str())
        .collect::<BTreeSet<_>>();
    if expected != actual || actual.len() != execution.items.len() {
        return Err(OperationCoordinatorError::InvalidExecution(
            "provider receipt must contain exactly one result per planned target".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_persistence::{InMemoryOperationReceiptStore, OperationReceiptStore};
    use velvet_workbench_protocol::{
        OperationKind, OperationRetryPolicy, OperationUndoCapability, OPERATION_SCHEMA_VERSION,
    };

    struct FakeProvider {
        observed: Vec<OperationTarget>,
        outcome: Result<OperationExecution, OperationProviderError>,
        execute_calls: usize,
    }

    impl OperationProvider for FakeProvider {
        fn observe(
            &mut self,
            _plan: &OperationPlan,
        ) -> Result<Vec<OperationTarget>, OperationProviderError> {
            Ok(self.observed.clone())
        }

        fn execute(
            &mut self,
            _plan: &OperationPlan,
        ) -> Result<OperationExecution, OperationProviderError> {
            self.execute_calls += 1;
            self.outcome.clone()
        }
    }

    fn plan() -> OperationPlan {
        OperationPlan {
            schema_version: OPERATION_SCHEMA_VERSION,
            plan_id: "plan-1".into(),
            operation_id: "operation-1".into(),
            owner_id: "asset".into(),
            kind: OperationKind::Move,
            targets: vec![OperationTarget {
                target_id: "asset-1".into(),
                expected_revision: Some(7),
                expected_fingerprint: None,
            }],
            created_at_ms: 1,
            expires_at_ms: 100,
            retry_policy: OperationRetryPolicy::ReconcileBeforeRetry,
            undo: OperationUndoCapability::ProviderToken,
        }
    }

    fn observed() -> Vec<OperationTarget> {
        vec![OperationTarget {
            target_id: "asset-1".into(),
            expected_revision: Some(7),
            expected_fingerprint: None,
        }]
    }

    fn committed_execution() -> OperationExecution {
        OperationExecution::committed(
            vec![OperationItemReceipt {
                target_id: "asset-1".into(),
                status: OperationReceiptStatus::Committed,
                message: None,
            }],
            Some("undo-1".into()),
        )
    }

    #[test]
    fn commit_writes_started_before_final_receipt() {
        let store = Arc::new(InMemoryOperationReceiptStore::default());
        let coordinator = OperationCoordinator::new(store.clone());
        let mut provider = FakeProvider {
            observed: observed(),
            outcome: Ok(committed_execution()),
            execute_calls: 0,
        };

        let receipt = coordinator
            .commit(&plan(), 10, &mut provider)
            .expect("operation should commit");
        assert_eq!(receipt.status, OperationReceiptStatus::Committed);
        assert_eq!(receipt.undo_token.as_deref(), Some("undo-1"));
        assert_eq!(provider.execute_calls, 1);
        assert_eq!(store.load().expect("receipts should load").len(), 1);
    }

    #[test]
    fn stale_observation_prevents_started_receipt_and_execution() {
        let store = Arc::new(InMemoryOperationReceiptStore::default());
        let coordinator = OperationCoordinator::new(store.clone());
        let mut provider = FakeProvider {
            observed: vec![OperationTarget {
                target_id: "asset-1".into(),
                expected_revision: Some(8),
                expected_fingerprint: None,
            }],
            outcome: Ok(committed_execution()),
            execute_calls: 0,
        };

        assert_eq!(
            coordinator.commit(&plan(), 10, &mut provider),
            Err(OperationCoordinatorError::Plan(
                OperationPlanError::StaleTarget("asset-1".into(),)
            ))
        );
        assert_eq!(provider.execute_calls, 0);
        assert!(store.load().expect("receipts should load").is_empty());
    }

    #[test]
    fn unknown_provider_outcome_is_durable_and_blocks_replay() {
        let store = Arc::new(InMemoryOperationReceiptStore::default());
        let coordinator = OperationCoordinator::new(store.clone());
        let mut provider = FakeProvider {
            observed: observed(),
            outcome: Err(OperationProviderError::Unknown("ipc disconnected".into())),
            execute_calls: 0,
        };

        assert_eq!(
            coordinator.commit(&plan(), 10, &mut provider),
            Err(OperationCoordinatorError::ProviderUnknown {
                phase: OperationPhase::Execute,
                message: "ipc disconnected".into(),
            })
        );
        let receipts = store.load().expect("receipts should load");
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].status, OperationReceiptStatus::Unknown);
        assert!(receipts[0].retry_requires_reconciliation());

        let mut retry_provider = FakeProvider {
            observed: observed(),
            outcome: Ok(committed_execution()),
            execute_calls: 0,
        };
        assert_eq!(
            coordinator.commit(&plan(), 11, &mut retry_provider),
            Err(OperationCoordinatorError::ReconciliationRequired {
                operation_id: "operation-1".into(),
            })
        );
        assert_eq!(retry_provider.execute_calls, 0);
    }

    #[test]
    fn failed_provider_keeps_started_receipt_for_reconciliation() {
        let store = Arc::new(InMemoryOperationReceiptStore::default());
        let coordinator = OperationCoordinator::new(store.clone());
        let mut provider = FakeProvider {
            observed: observed(),
            outcome: Err(OperationProviderError::Failed("permission denied".into())),
            execute_calls: 0,
        };

        assert_eq!(
            coordinator.commit(&plan(), 10, &mut provider),
            Err(OperationCoordinatorError::ProviderFailed {
                phase: OperationPhase::Execute,
                message: "permission denied".into(),
            })
        );
        let receipts = store.load().expect("receipts should load");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].retry_requires_reconciliation());
    }

    #[test]
    fn invalid_provider_receipt_does_not_publish_a_final_status() {
        let store = Arc::new(InMemoryOperationReceiptStore::default());
        let coordinator = OperationCoordinator::new(store.clone());
        let mut provider = FakeProvider {
            observed: observed(),
            outcome: Ok(OperationExecution::committed(Vec::new(), None)),
            execute_calls: 0,
        };

        assert!(matches!(
            coordinator.commit(&plan(), 10, &mut provider),
            Err(OperationCoordinatorError::InvalidExecution(_))
        ));
        let receipts = store.load().expect("receipts should load");
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].status, OperationReceiptStatus::Started);
    }

    #[test]
    fn final_receipt_cannot_hide_ambiguous_item_or_undo_capability() {
        let store = Arc::new(InMemoryOperationReceiptStore::default());
        let coordinator = OperationCoordinator::new(store.clone());
        let mut provider = FakeProvider {
            observed: observed(),
            outcome: Ok(OperationExecution::committed(
                vec![OperationItemReceipt {
                    target_id: "asset-1".into(),
                    status: OperationReceiptStatus::Unknown,
                    message: Some("provider lost contact".into()),
                }],
                Some("unexpected-undo".into()),
            )),
            execute_calls: 0,
        };

        assert!(matches!(
            coordinator.commit(&plan(), 10, &mut provider),
            Err(OperationCoordinatorError::InvalidExecution(_))
        ));
        assert_eq!(
            store.load().expect("receipts should load")[0].status,
            OperationReceiptStatus::Started
        );

        let mut no_undo_plan = plan();
        no_undo_plan.undo = OperationUndoCapability::None;
        let mut no_undo_provider = FakeProvider {
            observed: observed(),
            outcome: Ok(OperationExecution::committed(
                vec![OperationItemReceipt {
                    target_id: "asset-1".into(),
                    status: OperationReceiptStatus::Committed,
                    message: None,
                }],
                Some("unexpected-undo".into()),
            )),
            execute_calls: 0,
        };
        no_undo_plan.operation_id = "operation-2".into();
        assert!(matches!(
            coordinator.commit(&no_undo_plan, 10, &mut no_undo_provider),
            Err(OperationCoordinatorError::InvalidExecution(_))
        ));
    }
}
