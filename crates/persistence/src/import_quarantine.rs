use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use velvet_workbench_protocol::{
    ImportQuarantineEntry, IMPORT_QUARANTINE_SCHEMA_VERSION, PORTABLE_BUNDLE_MAX_SECTION_BYTES,
};

use crate::durability::{write_atomic, DurabilityMode};
use crate::PersistenceError;

pub const MAX_IMPORT_QUARANTINE_ENTRIES: usize = 128;
pub const MAX_IMPORT_QUARANTINE_STORE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportQuarantineStoreState {
    schema_version: u16,
    entries: Vec<ImportQuarantineEntry>,
}

/// Durable inert storage for extension bundle sections that could not be
/// activated or committed. It stores bytes and bounded provenance only; it
/// does not decode, execute or grant capabilities to the quarantined payload.
pub trait ImportQuarantineStore: Send + Sync {
    fn load(&self) -> Result<Vec<ImportQuarantineEntry>, PersistenceError>;
    fn save(&self, entries: &[ImportQuarantineEntry]) -> Result<(), PersistenceError>;
}

#[derive(Default)]
pub struct InMemoryImportQuarantineStore {
    entries: Mutex<Vec<ImportQuarantineEntry>>,
}

impl ImportQuarantineStore for InMemoryImportQuarantineStore {
    fn load(&self) -> Result<Vec<ImportQuarantineEntry>, PersistenceError> {
        self.entries
            .lock()
            .map_err(|_| PersistenceError::Unavailable)
            .and_then(|entries| bounded_entries(&entries))
    }

    fn save(&self, entries: &[ImportQuarantineEntry]) -> Result<(), PersistenceError> {
        let entries = bounded_entries(entries)?;
        self.entries
            .lock()
            .map(|mut current| *current = entries)
            .map_err(|_| PersistenceError::Unavailable)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileImportQuarantineStore {
    path: PathBuf,
    durability: DurabilityMode,
}

impl JsonFileImportQuarantineStore {
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

impl ImportQuarantineStore for JsonFileImportQuarantineStore {
    fn load(&self) -> Result<Vec<ImportQuarantineEntry>, PersistenceError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(PersistenceError::Unavailable),
        };
        if metadata.len() > MAX_IMPORT_QUARANTINE_STORE_BYTES {
            return Err(PersistenceError::CorruptState);
        }
        let bytes = fs::read(&self.path).map_err(|_| PersistenceError::Unavailable)?;
        let state = serde_json::from_slice::<ImportQuarantineStoreState>(&bytes)
            .map_err(|_| PersistenceError::CorruptState)?;
        if state.schema_version != IMPORT_QUARANTINE_SCHEMA_VERSION {
            return Err(PersistenceError::CorruptState);
        }
        bounded_entries(&state.entries)
    }

    fn save(&self, entries: &[ImportQuarantineEntry]) -> Result<(), PersistenceError> {
        let entries = bounded_entries(entries)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| PersistenceError::Unavailable)?;
        }
        let state = ImportQuarantineStoreState {
            schema_version: IMPORT_QUARANTINE_SCHEMA_VERSION,
            entries,
        };
        let bytes =
            serde_json::to_vec_pretty(&state).map_err(|_| PersistenceError::CorruptState)?;
        if bytes.len() as u64 > MAX_IMPORT_QUARANTINE_STORE_BYTES {
            return Err(PersistenceError::CorruptState);
        }
        write_atomic(&self.path, &bytes, self.durability).map_err(|_| PersistenceError::Unavailable)
    }
}

fn bounded_entries(
    entries: &[ImportQuarantineEntry],
) -> Result<Vec<ImportQuarantineEntry>, PersistenceError> {
    if entries.len() > MAX_IMPORT_QUARANTINE_ENTRIES {
        return Err(PersistenceError::CorruptState);
    }
    let mut keys = std::collections::BTreeSet::new();
    let mut total_bytes = 0u64;
    for entry in entries {
        entry
            .validate()
            .map_err(|_| PersistenceError::CorruptState)?;
        if entry.payload.len() > PORTABLE_BUNDLE_MAX_SECTION_BYTES {
            return Err(PersistenceError::CorruptState);
        }
        let key = format!("{}:{}:{}", entry.bundle_id, entry.plan_id, entry.section_id);
        if !keys.insert(key) {
            return Err(PersistenceError::CorruptState);
        }
        total_bytes = total_bytes.saturating_add(entry.payload.len() as u64);
        if total_bytes > MAX_IMPORT_QUARANTINE_STORE_BYTES {
            return Err(PersistenceError::CorruptState);
        }
    }
    Ok(entries.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("velvet-{label}-{nonce}.json"))
    }

    fn entry(section_id: &str) -> ImportQuarantineEntry {
        ImportQuarantineEntry {
            schema_version: IMPORT_QUARANTINE_SCHEMA_VERSION,
            bundle_id: "bundle-1".into(),
            plan_id: "plan-1".into(),
            section_id: section_id.into(),
            owner_id: "sample.extension".into(),
            section_schema_version: 1,
            quarantined_at_ms: 42,
            reason: "participant unavailable".into(),
            payload: vec![1, 2, 3],
        }
    }

    #[test]
    fn in_memory_store_bounds_and_deduplicates_quarantine_entries() {
        let store = InMemoryImportQuarantineStore::default();
        store
            .save(&[entry("section-1")])
            .expect("entry should save");
        assert_eq!(
            store.load().expect("entry should load"),
            vec![entry("section-1")]
        );
        assert_eq!(
            store.save(&[entry("section-1"), entry("section-1")]),
            Err(PersistenceError::CorruptState)
        );
    }

    #[test]
    fn json_store_round_trips_and_rejects_unknown_schema() {
        let path = temporary_path("import-quarantine");
        let store = JsonFileImportQuarantineStore::new(&path);
        store
            .save(&[entry("section-1")])
            .expect("entry should save");
        assert_eq!(
            store.load().expect("entry should load"),
            vec![entry("section-1")]
        );
        fs::write(&path, br#"{"schemaVersion":99,"entries":[]}"#)
            .expect("invalid state should be writable");
        assert_eq!(store.load(), Err(PersistenceError::CorruptState));
        let _ = fs::remove_file(path);
    }
}
