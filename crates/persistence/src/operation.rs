use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use velvet_workbench_protocol::{OperationPlan, OperationReceipt, OPERATION_SCHEMA_VERSION};

use crate::durability::{write_atomic, DurabilityMode};
use crate::PersistenceError;

pub const MAX_OPERATION_RECEIPTS: usize = 256;
pub const MAX_OPERATION_RECEIPT_ITEMS: usize = 512;
pub const MAX_OPERATION_RECEIPT_STORE_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationReceiptStoreState {
    schema_version: u16,
    receipts: Vec<OperationReceipt>,
}

/// Persistence adapter for the host-owned operation journal.
///
/// The adapter only retains bounded, schema-compatible receipts. It does not
/// execute operations, infer idempotency, or decide whether an undo token is
/// valid; those semantics remain with `ApplicationCore` and the provider
/// owner.
pub trait OperationReceiptStore: Send + Sync {
    fn load(&self) -> Result<Vec<OperationReceipt>, PersistenceError>;
    fn save(&self, receipts: &[OperationReceipt]) -> Result<(), PersistenceError>;
}

#[derive(Default)]
pub struct InMemoryOperationReceiptStore {
    receipts: Mutex<Vec<OperationReceipt>>,
}

impl OperationReceiptStore for InMemoryOperationReceiptStore {
    fn load(&self) -> Result<Vec<OperationReceipt>, PersistenceError> {
        self.receipts
            .lock()
            .map_err(|_| PersistenceError::Unavailable)
            .and_then(|receipts| bounded_receipts(&receipts))
    }

    fn save(&self, receipts: &[OperationReceipt]) -> Result<(), PersistenceError> {
        let receipts = bounded_receipts(receipts)?;
        self.receipts
            .lock()
            .map(|mut current| *current = receipts)
            .map_err(|_| PersistenceError::Unavailable)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileOperationReceiptStore {
    path: PathBuf,
    durability: DurabilityMode,
}

impl JsonFileOperationReceiptStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_durability(path, DurabilityMode::AtomicRenameOnly)
    }

    pub fn with_durability(path: impl Into<PathBuf>, durability: DurabilityMode) -> Self {
        Self {
            path: path.into(),
            durability,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl OperationReceiptStore for JsonFileOperationReceiptStore {
    fn load(&self) -> Result<Vec<OperationReceipt>, PersistenceError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(PersistenceError::Unavailable),
        };
        if metadata.len() > MAX_OPERATION_RECEIPT_STORE_BYTES {
            return Err(PersistenceError::CorruptState);
        }
        let bytes = fs::read(&self.path).map_err(|_| PersistenceError::Unavailable)?;
        let state = serde_json::from_slice::<OperationReceiptStoreState>(&bytes)
            .map_err(|_| PersistenceError::CorruptState)?;
        if state.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(PersistenceError::CorruptState);
        }
        bounded_receipts(&state.receipts)
    }

    fn save(&self, receipts: &[OperationReceipt]) -> Result<(), PersistenceError> {
        let receipts = bounded_receipts(receipts)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| PersistenceError::Unavailable)?;
        }
        let state = OperationReceiptStoreState {
            schema_version: OPERATION_SCHEMA_VERSION,
            receipts,
        };
        let bytes =
            serde_json::to_vec_pretty(&state).map_err(|_| PersistenceError::CorruptState)?;
        if bytes.len() as u64 > MAX_OPERATION_RECEIPT_STORE_BYTES {
            return Err(PersistenceError::CorruptState);
        }
        write_atomic(&self.path, &bytes, self.durability).map_err(|_| PersistenceError::Unavailable)
    }
}

fn bounded_receipts(
    receipts: &[OperationReceipt],
) -> Result<Vec<OperationReceipt>, PersistenceError> {
    if receipts.len() > MAX_OPERATION_RECEIPTS {
        return Err(PersistenceError::CorruptState);
    }
    let mut operation_ids = std::collections::BTreeSet::new();
    for receipt in receipts {
        if receipt.schema_version != OPERATION_SCHEMA_VERSION
            || receipt.plan_id.is_empty()
            || receipt.operation_id.is_empty()
            || receipt.plan_id.len() > OperationPlan::MAX_ID_BYTES
            || receipt.operation_id.len() > OperationPlan::MAX_ID_BYTES
            || receipt.items.len() > MAX_OPERATION_RECEIPT_ITEMS
            || !operation_ids.insert(receipt.operation_id.clone())
        {
            return Err(PersistenceError::CorruptState);
        }
        if receipt.items.iter().any(|item| {
            item.target_id.is_empty()
                || item.target_id.len() > OperationPlan::MAX_ID_BYTES
                || item.target_id.contains('\0')
                || item
                    .message
                    .as_ref()
                    .is_some_and(|message| message.len() > 4096)
        }) {
            return Err(PersistenceError::CorruptState);
        }
    }
    Ok(receipts.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use velvet_workbench_protocol::OperationReceiptStatus;

    fn temporary_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("velvet-{label}-{nonce}.json"))
    }

    fn receipt(operation_id: &str, status: OperationReceiptStatus) -> OperationReceipt {
        OperationReceipt {
            schema_version: OPERATION_SCHEMA_VERSION,
            plan_id: format!("plan-{operation_id}"),
            operation_id: operation_id.into(),
            status,
            reconciled: false,
            recorded_at_ms: 42,
            items: Vec::new(),
            undo_token: None,
        }
    }

    #[test]
    fn in_memory_store_bounds_schema_and_duplicate_receipts() {
        let store = InMemoryOperationReceiptStore::default();
        let receipts = vec![receipt("op-1", OperationReceiptStatus::Unknown)];
        store.save(&receipts).expect("receipt should save");
        assert_eq!(store.load().expect("receipt should load"), receipts);
        assert_eq!(
            store.save(&[
                receipt("op-1", OperationReceiptStatus::Started),
                receipt("op-1", OperationReceiptStatus::Committed,)
            ]),
            Err(PersistenceError::CorruptState)
        );
    }

    #[test]
    fn json_store_round_trips_unknown_receipts_and_rejects_invalid_state() {
        let path = temporary_path("operation-receipts");
        let store = JsonFileOperationReceiptStore::new(&path);
        let receipts = vec![receipt("op-1", OperationReceiptStatus::Unknown)];
        store.save(&receipts).expect("receipt should be written");
        assert_eq!(store.load().expect("receipt should load"), receipts);

        fs::write(
            &path,
            br#"{"schemaVersion":1,"receipts":[{"schemaVersion":99,"planId":"plan-1","operationId":"op-1","status":"unknown","reconciled":false,"recordedAtMs":42,"items":[],"undoToken":null}]}"#,
        )
        .expect("invalid receipt state should be writable");
        assert_eq!(store.load(), Err(PersistenceError::CorruptState));
        let _ = fs::remove_file(path);
    }
}
