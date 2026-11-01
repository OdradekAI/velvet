//! Host-owned durable state for extensions.
//!
//! An extension gets a capability handle, never a filesystem path. Each
//! extension owns an independent snapshot namespace, and a failed export or
//! migration cannot publish a core `RecoverySet` pointer or mutate another
//! extension's state.

use std::collections::BTreeMap;
use std::sync::Arc;

use crc32fast::hash;
use serde::{Deserialize, Serialize};
use velvet_persistence::{SnapshotError, StateSnapshot, StateSnapshotMetadata, StateSnapshotStore};
use velvet_plugin_api::ExtensionId;

pub const EXTENSION_STORAGE_SCOPE: &str = "storage";
pub const MAX_EXTENSION_STORAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_EXTENSION_STORAGE_KEY_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtensionStorageError {
    Invalid(String),
    Unavailable(String),
    Corrupt(String),
    QuotaExceeded { limit: usize, actual: usize },
    Conflict { expected: u64, actual: u64 },
    SchemaMismatch { expected: u16, actual: u16 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExtensionStoragePayload {
    schema_version: u16,
    entries: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStorageView {
    pub extension_id: ExtensionId,
    pub schema_version: u16,
    pub generation: u64,
    pub entries: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStorageExport {
    pub extension_id: ExtensionId,
    pub schema_version: u16,
    pub source_generation: u64,
    pub entries: BTreeMap<String, Vec<u8>>,
}

pub trait BackupParticipant: Send + Sync {
    fn export(
        &self,
        source: &ExtensionStorageView,
    ) -> Result<BTreeMap<String, Vec<u8>>, ExtensionStorageError>;
}

pub trait MigrationParticipant: Send + Sync {
    fn migrate(
        &self,
        source: &ExtensionStorageView,
        destination: &mut ExtensionStorageTransaction,
    ) -> Result<(), ExtensionStorageError>;
}

#[derive(Clone)]
pub struct ExtensionStorage {
    extension_id: ExtensionId,
    schema_version: u16,
    quota_bytes: usize,
    snapshots: Arc<dyn StateSnapshotStore>,
}

impl ExtensionStorage {
    pub fn new(
        extension_id: ExtensionId,
        schema_version: u16,
        quota_bytes: u64,
        snapshots: Arc<dyn StateSnapshotStore>,
    ) -> Result<Self, ExtensionStorageError> {
        if schema_version == 0 {
            return Err(ExtensionStorageError::Invalid(
                "storage schemaVersion must be non-zero".into(),
            ));
        }
        let quota_bytes = usize::try_from(quota_bytes).map_err(|_| {
            ExtensionStorageError::Invalid("storage quotaBytes does not fit this platform".into())
        })?;
        if quota_bytes == 0 || quota_bytes > MAX_EXTENSION_STORAGE_BYTES {
            return Err(ExtensionStorageError::Invalid(format!(
                "storage quotaBytes must be between 1 and {MAX_EXTENSION_STORAGE_BYTES}"
            )));
        }
        if extension_id.0.is_empty()
            || extension_id.0.contains('/')
            || extension_id.0.contains('\\')
            || extension_id.0.contains("..")
        {
            return Err(ExtensionStorageError::Invalid(
                "extension id cannot be used as a storage namespace".into(),
            ));
        }
        Ok(Self {
            extension_id,
            schema_version,
            quota_bytes,
            snapshots,
        })
    }

    pub fn extension_id(&self) -> &ExtensionId {
        &self.extension_id
    }

    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn quota_bytes(&self) -> usize {
        self.quota_bytes
    }

    pub fn load(&self) -> Result<ExtensionStorageView, ExtensionStorageError> {
        self.load_snapshot(
            self.snapshots
                .load_latest(&self.owner_id(), EXTENSION_STORAGE_SCOPE)?,
        )
    }

    pub fn previous(&self) -> Result<Option<ExtensionStorageView>, ExtensionStorageError> {
        self.snapshots
            .load_previous(&self.owner_id(), EXTENSION_STORAGE_SCOPE)?
            .map(|snapshot| self.decode_snapshot(snapshot))
            .transpose()
    }

    /// Copies the previous committed generation into a new current generation.
    /// This leaves the old generation intact, so a failed restore cannot erase
    /// the only known-good extension state.
    pub fn restore_previous(&self) -> Result<ExtensionStorageView, ExtensionStorageError> {
        let Some(previous) = self.previous()? else {
            return Err(ExtensionStorageError::Unavailable(
                "no previous extension storage generation exists".into(),
            ));
        };
        let mut transaction = self.begin_transaction()?;
        transaction.entries = previous.entries;
        transaction.commit()
    }

    pub fn begin_transaction(&self) -> Result<ExtensionStorageTransaction, ExtensionStorageError> {
        let current = self.load()?;
        Ok(ExtensionStorageTransaction {
            storage: self.clone(),
            base_generation: current.generation,
            entries: current.entries,
            committed: false,
        })
    }

    pub fn export_with(
        &self,
        participant: &dyn BackupParticipant,
    ) -> Result<ExtensionStorageExport, ExtensionStorageError> {
        let source = self.load()?;
        let entries = participant.export(&source)?;
        self.export_entries(&source, entries)
    }

    /// Validates an externally produced backup result without publishing it.
    /// The source generation is retained so a caller can display or persist a
    /// portable export without confusing it with a committed extension state.
    pub fn export_entries(
        &self,
        source: &ExtensionStorageView,
        entries: BTreeMap<String, Vec<u8>>,
    ) -> Result<ExtensionStorageExport, ExtensionStorageError> {
        if source.extension_id != self.extension_id {
            return Err(ExtensionStorageError::Corrupt(
                "extension storage export has the wrong owner".into(),
            ));
        }
        if source.schema_version != self.schema_version {
            return Err(ExtensionStorageError::SchemaMismatch {
                expected: self.schema_version,
                actual: source.schema_version,
            });
        }
        self.validate_entries(&entries)?;
        Ok(ExtensionStorageExport {
            extension_id: self.extension_id.clone(),
            schema_version: self.schema_version,
            source_generation: source.generation,
            entries,
        })
    }

    pub fn migrate_with(
        &self,
        participant: &dyn MigrationParticipant,
    ) -> Result<ExtensionStorageView, ExtensionStorageError> {
        let source = self.load_any_schema()?;
        let mut destination = ExtensionStorageTransaction {
            storage: self.clone(),
            base_generation: source.generation,
            entries: source.entries.clone(),
            committed: false,
        };
        participant.migrate(&source, &mut destination)?;
        destination.commit()
    }

    /// Loads the latest generation without requiring it to already use the
    /// destination schema. This is the source view passed to a migration
    /// participant; callers must not publish it directly.
    pub fn load_for_migration(&self) -> Result<ExtensionStorageView, ExtensionStorageError> {
        self.load_any_schema()
    }

    /// Commits a host-validated migration result against the exact source
    /// generation that was presented to the participant. A concurrent writer
    /// therefore produces a conflict instead of losing either update.
    pub fn commit_migrated_entries(
        &self,
        source: &ExtensionStorageView,
        entries: BTreeMap<String, Vec<u8>>,
    ) -> Result<ExtensionStorageView, ExtensionStorageError> {
        if source.extension_id != self.extension_id {
            return Err(ExtensionStorageError::Corrupt(
                "extension storage migration has the wrong owner".into(),
            ));
        }
        self.validate_entries(&entries)?;
        let destination = ExtensionStorageTransaction {
            storage: self.clone(),
            base_generation: source.generation,
            entries,
            committed: false,
        };
        destination.commit()
    }

    fn owner_id(&self) -> String {
        format!("extension.{}", self.extension_id.0)
    }

    fn load_snapshot(
        &self,
        snapshot: Option<StateSnapshot>,
    ) -> Result<ExtensionStorageView, ExtensionStorageError> {
        match snapshot {
            Some(snapshot) => self.decode_snapshot(snapshot),
            None => Ok(ExtensionStorageView {
                extension_id: self.extension_id.clone(),
                schema_version: self.schema_version,
                generation: 0,
                entries: BTreeMap::new(),
            }),
        }
    }

    fn load_any_schema(&self) -> Result<ExtensionStorageView, ExtensionStorageError> {
        self.snapshots
            .load_latest(&self.owner_id(), EXTENSION_STORAGE_SCOPE)?
            .map(|snapshot| self.decode_snapshot_without_schema_check(snapshot))
            .transpose()
            .map(|view| {
                view.unwrap_or_else(|| ExtensionStorageView {
                    extension_id: self.extension_id.clone(),
                    schema_version: self.schema_version,
                    generation: 0,
                    entries: BTreeMap::new(),
                })
            })
    }

    fn decode_snapshot(
        &self,
        snapshot: StateSnapshot,
    ) -> Result<ExtensionStorageView, ExtensionStorageError> {
        let view = self.decode_snapshot_without_schema_check(snapshot)?;
        if view.schema_version != self.schema_version {
            return Err(ExtensionStorageError::SchemaMismatch {
                expected: self.schema_version,
                actual: view.schema_version,
            });
        }
        Ok(view)
    }

    fn decode_snapshot_without_schema_check(
        &self,
        snapshot: StateSnapshot,
    ) -> Result<ExtensionStorageView, ExtensionStorageError> {
        if snapshot.metadata.owner_id != self.owner_id()
            || snapshot.metadata.scope != EXTENSION_STORAGE_SCOPE
        {
            return Err(ExtensionStorageError::Corrupt(
                "extension storage snapshot has the wrong owner or scope".into(),
            ));
        }
        let payload: ExtensionStoragePayload =
            serde_json::from_slice(&snapshot.payload).map_err(|error| {
                ExtensionStorageError::Corrupt(format!("storage payload is invalid: {error}"))
            })?;
        if payload.schema_version != snapshot.metadata.schema_version {
            return Err(ExtensionStorageError::Corrupt(
                "storage payload schema does not match snapshot metadata".into(),
            ));
        }
        self.validate_entries(&payload.entries)?;
        Ok(ExtensionStorageView {
            extension_id: self.extension_id.clone(),
            schema_version: payload.schema_version,
            generation: snapshot.metadata.generation,
            entries: payload.entries,
        })
    }

    fn validate_entries(
        &self,
        entries: &BTreeMap<String, Vec<u8>>,
    ) -> Result<(), ExtensionStorageError> {
        for key in entries.keys() {
            if key.is_empty()
                || key.len() > MAX_EXTENSION_STORAGE_KEY_BYTES
                || key.chars().any(char::is_control)
            {
                return Err(ExtensionStorageError::Invalid(format!(
                    "storage key is empty, too long, or contains a control character: {key:?}"
                )));
            }
        }
        let bytes = serde_json::to_vec(&ExtensionStoragePayload {
            schema_version: self.schema_version,
            entries: entries.clone(),
        })
        .map_err(|error| {
            ExtensionStorageError::Invalid(format!("storage could not be encoded: {error}"))
        })?;
        if bytes.len() > self.quota_bytes {
            return Err(ExtensionStorageError::QuotaExceeded {
                limit: self.quota_bytes,
                actual: bytes.len(),
            });
        }
        Ok(())
    }
}

pub struct ExtensionStorageTransaction {
    storage: ExtensionStorage,
    base_generation: u64,
    entries: BTreeMap<String, Vec<u8>>,
    committed: bool,
}

impl ExtensionStorageTransaction {
    pub fn base_generation(&self) -> u64 {
        self.base_generation
    }

    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.entries.get(key).map(Vec::as_slice)
    }

    pub fn put(
        &mut self,
        key: impl Into<String>,
        value: Vec<u8>,
    ) -> Result<(), ExtensionStorageError> {
        let key = key.into();
        let previous = self.entries.insert(key.clone(), value);
        if let Err(error) = self.storage.validate_entries(&self.entries) {
            if let Some(previous) = previous {
                self.entries.insert(key, previous);
            } else {
                self.entries.remove(&key);
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    pub fn replace_entries(
        &mut self,
        entries: BTreeMap<String, Vec<u8>>,
    ) -> Result<(), ExtensionStorageError> {
        self.storage.validate_entries(&entries)?;
        self.entries = entries;
        Ok(())
    }

    pub fn commit(mut self) -> Result<ExtensionStorageView, ExtensionStorageError> {
        if self.committed {
            return Err(ExtensionStorageError::Invalid(
                "extension storage transaction was already committed".into(),
            ));
        }
        let latest = self
            .storage
            .snapshots
            .load_latest(&self.storage.owner_id(), EXTENSION_STORAGE_SCOPE)?
            .map(|snapshot| snapshot.metadata.generation)
            .unwrap_or(0);
        if latest != self.base_generation {
            return Err(ExtensionStorageError::Conflict {
                expected: self.base_generation,
                actual: latest,
            });
        }
        self.storage.validate_entries(&self.entries)?;
        let generation = self.base_generation.saturating_add(1).max(1);
        let payload = serde_json::to_vec(&ExtensionStoragePayload {
            schema_version: self.storage.schema_version,
            entries: self.entries.clone(),
        })
        .map_err(|error| {
            ExtensionStorageError::Invalid(format!("storage could not be encoded: {error}"))
        })?;
        let snapshot = StateSnapshot {
            metadata: StateSnapshotMetadata {
                owner_id: self.storage.owner_id(),
                scope: EXTENSION_STORAGE_SCOPE.into(),
                schema_version: self.storage.schema_version,
                generation,
                created_at_ms: 0,
                checksum: hash(&payload),
                committed: false,
            },
            payload,
        };
        self.storage.snapshots.stage(&snapshot)?;
        self.storage.snapshots.commit(
            &snapshot.metadata.owner_id,
            &snapshot.metadata.scope,
            generation,
        )?;
        self.committed = true;
        Ok(ExtensionStorageView {
            extension_id: self.storage.extension_id.clone(),
            schema_version: self.storage.schema_version,
            generation,
            entries: self.entries,
        })
    }
}

impl From<SnapshotError> for ExtensionStorageError {
    fn from(error: SnapshotError) -> Self {
        match error {
            SnapshotError::Invalid(message) => Self::Invalid(message),
            SnapshotError::Unavailable(message) => Self::Unavailable(message),
            SnapshotError::Corrupt(message) => Self::Corrupt(message),
            SnapshotError::NotFound => {
                Self::Unavailable("extension storage snapshot not found".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_persistence::InMemoryStateSnapshotStore;

    fn storage(id: &str, schema: u16, quota: u64) -> ExtensionStorage {
        ExtensionStorage::new(
            ExtensionId(id.into()),
            schema,
            quota,
            Arc::new(InMemoryStateSnapshotStore::default()),
        )
        .unwrap()
    }

    struct CopyBackup;
    impl BackupParticipant for CopyBackup {
        fn export(
            &self,
            source: &ExtensionStorageView,
        ) -> Result<BTreeMap<String, Vec<u8>>, ExtensionStorageError> {
            Ok(source.entries.clone())
        }
    }

    struct FailingMigration;
    impl MigrationParticipant for FailingMigration {
        fn migrate(
            &self,
            _source: &ExtensionStorageView,
            _destination: &mut ExtensionStorageTransaction,
        ) -> Result<(), ExtensionStorageError> {
            Err(ExtensionStorageError::Unavailable(
                "injected migration failure".into(),
            ))
        }
    }

    struct RenameMigration;
    impl MigrationParticipant for RenameMigration {
        fn migrate(
            &self,
            source: &ExtensionStorageView,
            destination: &mut ExtensionStorageTransaction,
        ) -> Result<(), ExtensionStorageError> {
            let value = source
                .entries
                .get("answer")
                .cloned()
                .ok_or_else(|| ExtensionStorageError::Invalid("answer is missing".into()))?;
            destination.put("answer.v2", value)
        }
    }

    #[test]
    fn storage_is_namespaced_and_transactional() {
        let first = storage("sample.one", 1, 1024);
        let second = storage_with_same_store(&first, "sample.two");
        let mut transaction = first.begin_transaction().unwrap();
        transaction.put("answer", b"42".to_vec()).unwrap();
        transaction.commit().unwrap();
        assert_eq!(first.load().unwrap().entries["answer"], b"42");
        assert!(second.load().unwrap().entries.is_empty());
    }

    #[test]
    fn concurrent_transactions_conflict_without_lost_update() {
        let store = Arc::new(InMemoryStateSnapshotStore::default());
        let first =
            ExtensionStorage::new(ExtensionId("sample.one".into()), 1, 1024, store).unwrap();
        let mut a = first.begin_transaction().unwrap();
        let mut b = first.begin_transaction().unwrap();
        a.put("a", vec![1]).unwrap();
        a.commit().unwrap();
        b.put("b", vec![2]).unwrap();
        assert!(matches!(
            b.commit(),
            Err(ExtensionStorageError::Conflict { .. })
        ));
        assert!(first.load().unwrap().entries.contains_key("a"));
        assert!(!first.load().unwrap().entries.contains_key("b"));
    }

    #[test]
    fn participant_failure_does_not_publish_migration() {
        let store = Arc::new(InMemoryStateSnapshotStore::default());
        let first = ExtensionStorage::new(ExtensionId("sample.one".into()), 1, 1024, store.clone())
            .unwrap();
        let mut tx = first.begin_transaction().unwrap();
        tx.put("answer", b"42".to_vec()).unwrap();
        let prior = tx.commit().unwrap();
        let migrated =
            ExtensionStorage::new(ExtensionId("sample.one".into()), 2, 1024, store).unwrap();
        assert!(migrated.migrate_with(&FailingMigration).is_err());
        let current = first.load().unwrap();
        assert_eq!(current.generation, prior.generation);
        assert_eq!(current.entries, prior.entries);
    }

    #[test]
    fn migration_commits_a_new_schema_generation_only_after_participant_success() {
        let store = Arc::new(InMemoryStateSnapshotStore::default());
        let first = ExtensionStorage::new(ExtensionId("sample.one".into()), 1, 1024, store.clone())
            .unwrap();
        let mut tx = first.begin_transaction().unwrap();
        tx.put("answer", b"42".to_vec()).unwrap();
        tx.commit().unwrap();

        let migrated =
            ExtensionStorage::new(ExtensionId("sample.one".into()), 2, 1024, store).unwrap();
        let result = migrated.migrate_with(&RenameMigration).unwrap();
        assert_eq!(result.generation, 2);
        assert_eq!(result.schema_version, 2);
        assert_eq!(result.entries["answer.v2"], b"42");
    }

    #[test]
    fn backup_participant_is_bounded_by_host_quota() {
        let store = storage("sample.one", 1, 128);
        let export = store.export_with(&CopyBackup).unwrap();
        assert_eq!(export.source_generation, 0);
        assert!(export.entries.is_empty());
        let mut tx = store.begin_transaction().unwrap();
        tx.put("stable", vec![1]).unwrap();
        assert!(tx.put("stable", vec![2; 256]).is_err());
        assert_eq!(tx.get("stable"), Some(&[1][..]));
        assert!(matches!(
            tx.put("bad\nkey", vec![1]),
            Err(ExtensionStorageError::Invalid(_))
        ));
        tx.commit().unwrap();
        let export = store.export_with(&CopyBackup).unwrap();
        assert_eq!(export.entries["stable"], vec![1]);
    }

    fn storage_with_same_store(source: &ExtensionStorage, id: &str) -> ExtensionStorage {
        ExtensionStorage::new(
            ExtensionId(id.into()),
            source.schema_version,
            source.quota_bytes as u64,
            source.snapshots.clone(),
        )
        .unwrap()
    }
}
