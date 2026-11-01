use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crc32fast::hash;
use serde::{Deserialize, Serialize};

use crate::durability::{write_atomic, DurabilityMode};

pub const STATE_SNAPSHOT_FORMAT_VERSION: u16 = 1;
pub const MAX_STATE_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateSnapshotMetadata {
    pub owner_id: String,
    pub scope: String,
    pub schema_version: u16,
    pub generation: u64,
    pub created_at_ms: u64,
    pub checksum: u32,
    pub committed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateSnapshot {
    pub metadata: StateSnapshotMetadata,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    Invalid(String),
    Unavailable(String),
    Corrupt(String),
    CorruptGeneration { generation: u64, message: String },
    NotFound,
}

pub trait StateSnapshotStore: Send + Sync {
    fn stage(&self, snapshot: &StateSnapshot) -> Result<(), SnapshotError>;
    fn commit(&self, owner_id: &str, scope: &str, generation: u64) -> Result<(), SnapshotError>;
    fn load_latest(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Option<StateSnapshot>, SnapshotError>;
    fn load_previous(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Option<StateSnapshot>, SnapshotError>;
    fn load_generation(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<Option<StateSnapshot>, SnapshotError>;
    /// Lists committed generation ids without decoding payloads. Recovery
    /// uses this to quarantine corrupt or future-schema bytes safely.
    fn list_committed_generations(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Vec<u64>, SnapshotError>;
    fn quarantine(&self, owner_id: &str, scope: &str, generation: u64)
        -> Result<(), SnapshotError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileStateSnapshotStore {
    root: PathBuf,
    durability: DurabilityMode,
}

impl JsonFileStateSnapshotStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_durability(root, DurabilityMode::AtomicRenameOnly)
    }

    pub fn with_durability(root: impl Into<PathBuf>, durability: DurabilityMode) -> Self {
        Self {
            root: root.into(),
            durability,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn durability_mode(&self) -> DurabilityMode {
        self.durability
    }

    fn scope_dir(&self, owner_id: &str, scope: &str) -> Result<PathBuf, SnapshotError> {
        validate_segment(owner_id, "owner id")?;
        validate_segment(scope, "scope")?;
        Ok(self.root.join(owner_id).join(scope))
    }

    fn staged_path(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<PathBuf, SnapshotError> {
        Ok(self
            .scope_dir(owner_id, scope)?
            .join(format!("{generation}.stage.json")))
    }

    fn committed_path(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<PathBuf, SnapshotError> {
        Ok(self
            .scope_dir(owner_id, scope)?
            .join(format!("{generation}.snapshot.json")))
    }

    fn quarantine_path(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<PathBuf, SnapshotError> {
        Ok(self
            .scope_dir(owner_id, scope)?
            .join(format!("{generation}.quarantine.json")))
    }

    fn load_committed(
        &self,
        owner_id: &str,
        scope: &str,
        skip_latest: usize,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        let directory = self.scope_dir(owner_id, scope)?;
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(SnapshotError::Unavailable(format!(
                    "snapshot directory could not be read: {error}"
                )))
            }
        };

        let mut snapshots = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                SnapshotError::Unavailable(format!("snapshot entry could not be read: {error}"))
            })?;
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(generation_text) = file_name.strip_suffix(".snapshot.json") else {
                continue;
            };
            let Ok(generation) = generation_text.parse::<u64>() else {
                // Files that cannot be addressed by a RecoverySet are retained
                // as evidence and ignored so one stray directory entry cannot
                // hide otherwise valid owner generations.
                continue;
            };
            if generation == 0 {
                continue;
            }
            let snapshot = decode_snapshot(&fs::read(&path).map_err(|error| {
                SnapshotError::Unavailable(format!("snapshot could not be read: {error}"))
            })?)
            .map_err(|error| with_generation(error, generation))?;
            validate_snapshot(&snapshot, true)
                .map_err(|error| with_generation(error, generation))?;
            if snapshot.metadata.owner_id != owner_id
                || snapshot.metadata.scope != scope
                || snapshot.metadata.generation != generation
            {
                return Err(SnapshotError::CorruptGeneration {
                    generation,
                    message: "committed snapshot identity does not match its path".into(),
                });
            }
            snapshots.push(snapshot);
        }
        snapshots.sort_by_key(|snapshot| snapshot.metadata.generation);
        Ok(snapshots.into_iter().rev().nth(skip_latest))
    }
}

impl StateSnapshotStore for JsonFileStateSnapshotStore {
    fn stage(&self, snapshot: &StateSnapshot) -> Result<(), SnapshotError> {
        validate_snapshot(snapshot, false)?;
        let directory = self.scope_dir(&snapshot.metadata.owner_id, &snapshot.metadata.scope)?;
        fs::create_dir_all(&directory).map_err(|error| {
            SnapshotError::Unavailable(format!("snapshot directory could not be created: {error}"))
        })?;
        let path = self.staged_path(
            &snapshot.metadata.owner_id,
            &snapshot.metadata.scope,
            snapshot.metadata.generation,
        )?;
        write_json_atomic(&path, snapshot, self.durability)
    }

    fn commit(&self, owner_id: &str, scope: &str, generation: u64) -> Result<(), SnapshotError> {
        let staged = self.staged_path(owner_id, scope, generation)?;
        let bytes = fs::read(&staged).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => SnapshotError::NotFound,
            _ => SnapshotError::Unavailable(format!("staged snapshot could not be read: {error}")),
        })?;
        let mut snapshot = decode_snapshot(&bytes)?;
        if snapshot.metadata.owner_id != owner_id
            || snapshot.metadata.scope != scope
            || snapshot.metadata.generation != generation
        {
            return Err(SnapshotError::Corrupt(
                "staged snapshot identity does not match its commit request".into(),
            ));
        }
        validate_snapshot(&snapshot, false)?;
        snapshot.metadata.committed = true;
        let committed = self.committed_path(owner_id, scope, generation)?;
        write_json_atomic(&committed, &snapshot, self.durability)?;
        fs::remove_file(staged).map_err(|error| {
            SnapshotError::Unavailable(format!("staged snapshot could not be retired: {error}"))
        })
    }

    fn load_latest(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        self.load_committed(owner_id, scope, 0)
    }

    fn load_previous(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        self.load_committed(owner_id, scope, 1)
    }

    fn load_generation(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        let path = self.committed_path(owner_id, scope, generation)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(SnapshotError::Unavailable(format!(
                    "snapshot could not be read: {error}"
                )))
            }
        };
        let snapshot =
            decode_snapshot(&bytes).map_err(|error| with_generation(error, generation))?;
        validate_snapshot(&snapshot, true).map_err(|error| with_generation(error, generation))?;
        if snapshot.metadata.owner_id != owner_id
            || snapshot.metadata.scope != scope
            || snapshot.metadata.generation != generation
        {
            return Err(SnapshotError::CorruptGeneration {
                generation,
                message: "committed snapshot identity does not match its path".into(),
            });
        }
        Ok(Some(snapshot))
    }

    fn list_committed_generations(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Vec<u64>, SnapshotError> {
        let directory = self.scope_dir(owner_id, scope)?;
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(SnapshotError::Unavailable(format!(
                    "snapshot directory could not be read: {error}"
                )))
            }
        };
        let mut generations = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                SnapshotError::Unavailable(format!("snapshot entry could not be read: {error}"))
            })?;
            let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(generation_text) = file_name.strip_suffix(".snapshot.json") else {
                continue;
            };
            let Ok(generation) = generation_text.parse::<u64>() else {
                continue;
            };
            if generation == 0 {
                continue;
            }
            generations.push(generation);
        }
        generations.sort_unstable();
        Ok(generations)
    }

    fn quarantine(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<(), SnapshotError> {
        let committed = self.committed_path(owner_id, scope, generation)?;
        let staged = self.staged_path(owner_id, scope, generation)?;
        let source = if committed.exists() {
            committed
        } else if staged.exists() {
            staged
        } else {
            return Err(SnapshotError::NotFound);
        };
        let target = self.quarantine_path(owner_id, scope, generation)?;
        fs::rename(source, target).map_err(|error| {
            SnapshotError::Unavailable(format!("snapshot could not be quarantined: {error}"))
        })
    }
}

#[derive(Default)]
pub struct InMemoryStateSnapshotStore {
    snapshots: Mutex<BTreeMap<(String, String, u64), StateSnapshot>>,
}

impl StateSnapshotStore for InMemoryStateSnapshotStore {
    fn stage(&self, snapshot: &StateSnapshot) -> Result<(), SnapshotError> {
        validate_snapshot(snapshot, false)?;
        self.snapshots
            .lock()
            .map_err(|_| SnapshotError::Unavailable("snapshot lock poisoned".into()))?
            .insert(
                (
                    snapshot.metadata.owner_id.clone(),
                    snapshot.metadata.scope.clone(),
                    snapshot.metadata.generation,
                ),
                snapshot.clone(),
            );
        Ok(())
    }

    fn commit(&self, owner_id: &str, scope: &str, generation: u64) -> Result<(), SnapshotError> {
        let mut snapshots = self
            .snapshots
            .lock()
            .map_err(|_| SnapshotError::Unavailable("snapshot lock poisoned".into()))?;
        let Some(snapshot) = snapshots.get_mut(&(owner_id.into(), scope.into(), generation)) else {
            return Err(SnapshotError::NotFound);
        };
        validate_snapshot(snapshot, false)?;
        snapshot.metadata.committed = true;
        Ok(())
    }

    fn load_latest(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        let snapshots = self
            .snapshots
            .lock()
            .map_err(|_| SnapshotError::Unavailable("snapshot lock poisoned".into()))?;
        snapshots
            .values()
            .filter(|snapshot| {
                snapshot.metadata.owner_id == owner_id
                    && snapshot.metadata.scope == scope
                    && snapshot.metadata.committed
            })
            .max_by_key(|snapshot| snapshot.metadata.generation)
            .cloned()
            .map_or(Ok(None), |snapshot| {
                let generation = snapshot.metadata.generation;
                validate_snapshot(&snapshot, true)
                    .map_err(|error| with_generation(error, generation))?;
                Ok(Some(snapshot))
            })
    }

    fn load_previous(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        let snapshots = self
            .snapshots
            .lock()
            .map_err(|_| SnapshotError::Unavailable("snapshot lock poisoned".into()))?;
        let mut matching = snapshots
            .values()
            .filter(|snapshot| {
                snapshot.metadata.owner_id == owner_id
                    && snapshot.metadata.scope == scope
                    && snapshot.metadata.committed
            })
            .collect::<Vec<_>>();
        matching.sort_by_key(|snapshot| snapshot.metadata.generation);
        matching
            .into_iter()
            .rev()
            .nth(1)
            .cloned()
            .map_or(Ok(None), |snapshot| {
                let generation = snapshot.metadata.generation;
                validate_snapshot(&snapshot, true)
                    .map_err(|error| with_generation(error, generation))?;
                Ok(Some(snapshot))
            })
    }

    fn load_generation(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        let snapshot = self
            .snapshots
            .lock()
            .map_err(|_| SnapshotError::Unavailable("snapshot lock poisoned".into()))?
            .get(&(owner_id.into(), scope.into(), generation))
            .filter(|snapshot| snapshot.metadata.committed)
            .cloned();
        snapshot.map_or(Ok(None), |snapshot| {
            validate_snapshot(&snapshot, true)
                .map_err(|error| with_generation(error, generation))?;
            Ok(Some(snapshot))
        })
    }

    fn list_committed_generations(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Vec<u64>, SnapshotError> {
        let snapshots = self
            .snapshots
            .lock()
            .map_err(|_| SnapshotError::Unavailable("snapshot lock poisoned".into()))?;
        let mut generations = snapshots
            .values()
            .filter(|snapshot| {
                snapshot.metadata.owner_id == owner_id
                    && snapshot.metadata.scope == scope
                    && snapshot.metadata.committed
            })
            .map(|snapshot| snapshot.metadata.generation)
            .collect::<Vec<_>>();
        generations.sort_unstable();
        Ok(generations)
    }

    fn quarantine(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<(), SnapshotError> {
        let removed = self
            .snapshots
            .lock()
            .map_err(|_| SnapshotError::Unavailable("snapshot lock poisoned".into()))?
            .remove(&(owner_id.into(), scope.into(), generation));
        removed.map(|_| ()).ok_or(SnapshotError::NotFound)
    }
}

fn validate_snapshot(snapshot: &StateSnapshot, committed: bool) -> Result<(), SnapshotError> {
    validate_segment(&snapshot.metadata.owner_id, "owner id")?;
    validate_segment(&snapshot.metadata.scope, "scope")?;
    if snapshot.metadata.schema_version == 0 || snapshot.metadata.generation == 0 {
        return Err(SnapshotError::Invalid(
            "snapshot schema and generation must be non-zero".into(),
        ));
    }
    if snapshot.payload.len() > MAX_STATE_SNAPSHOT_BYTES {
        return Err(SnapshotError::Invalid(format!(
            "snapshot payload exceeds {} bytes",
            MAX_STATE_SNAPSHOT_BYTES
        )));
    }
    if snapshot.metadata.committed != committed {
        return Err(SnapshotError::Invalid(
            "snapshot committed marker does not match the requested operation".into(),
        ));
    }
    if snapshot.metadata.checksum != hash(&snapshot.payload) {
        return Err(SnapshotError::Corrupt(
            "snapshot checksum does not match its payload".into(),
        ));
    }
    Ok(())
}

fn validate_segment(value: &str, label: &str) -> Result<(), SnapshotError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
        })
    {
        return Err(SnapshotError::Invalid(format!(
            "{label} contains an unsafe path segment"
        )));
    }
    Ok(())
}

fn with_generation(error: SnapshotError, generation: u64) -> SnapshotError {
    match error {
        SnapshotError::Invalid(message)
        | SnapshotError::Corrupt(message)
        | SnapshotError::CorruptGeneration { message, .. } => SnapshotError::CorruptGeneration {
            generation,
            message,
        },
        other => other,
    }
}

fn decode_snapshot(bytes: &[u8]) -> Result<StateSnapshot, SnapshotError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Envelope {
        format_version: u16,
        snapshot: StateSnapshot,
    }
    let envelope: Envelope = serde_json::from_slice(bytes)
        .map_err(|error| SnapshotError::Corrupt(format!("snapshot JSON is invalid: {error}")))?;
    if envelope.format_version != STATE_SNAPSHOT_FORMAT_VERSION {
        return Err(SnapshotError::Corrupt(
            "snapshot format version is unsupported".into(),
        ));
    }
    Ok(envelope.snapshot)
}

fn write_json_atomic(
    path: &Path,
    snapshot: &StateSnapshot,
    durability: DurabilityMode,
) -> Result<(), SnapshotError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Envelope<'a> {
        format_version: u16,
        snapshot: &'a StateSnapshot,
    }
    let bytes = serde_json::to_vec_pretty(&Envelope {
        format_version: STATE_SNAPSHOT_FORMAT_VERSION,
        snapshot,
    })
    .map_err(|error| {
        SnapshotError::Corrupt(format!("snapshot JSON could not be encoded: {error}"))
    })?;
    write_atomic(path, &bytes, durability).map_err(|error| {
        SnapshotError::Unavailable(format!("snapshot could not be published: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMPORARY_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let sequence = TEMPORARY_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "velvet-snapshots-{}-{nonce}-{sequence}",
            std::process::id()
        ))
    }

    fn snapshot(generation: u64, committed: bool) -> StateSnapshot {
        let payload = format!("layout-{generation}").into_bytes();
        StateSnapshot {
            metadata: StateSnapshotMetadata {
                owner_id: "layout".into(),
                scope: "workspace".into(),
                schema_version: 1,
                generation,
                created_at_ms: generation,
                checksum: hash(&payload),
                committed,
            },
            payload,
        }
    }

    #[test]
    fn staged_snapshot_is_invisible_until_commit_and_latest_generation_wins() {
        let root = temporary_root();
        let store = JsonFileStateSnapshotStore::new(&root);
        store.stage(&snapshot(1, false)).expect("stage should work");
        assert_eq!(store.load_latest("layout", "workspace").unwrap(), None);

        store
            .commit("layout", "workspace", 1)
            .expect("commit should work");
        store
            .stage(&snapshot(2, false))
            .expect("second stage should work");
        store
            .commit("layout", "workspace", 2)
            .expect("second commit should work");
        assert_eq!(
            store.load_latest("layout", "workspace").unwrap(),
            Some(snapshot(2, true))
        );
        assert_eq!(
            store.load_previous("layout", "workspace").unwrap(),
            Some(snapshot(1, true))
        );
        assert_eq!(
            store
                .list_committed_generations("layout", "workspace")
                .unwrap(),
            vec![1, 2]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checksum_corruption_is_reported_and_can_be_quarantined_without_losing_previous_generation() {
        let root = temporary_root();
        let store = JsonFileStateSnapshotStore::new(&root);
        store.stage(&snapshot(1, false)).expect("stage should work");
        store
            .commit("layout", "workspace", 1)
            .expect("commit should work");
        store
            .stage(&snapshot(2, false))
            .expect("second stage should work");
        store
            .commit("layout", "workspace", 2)
            .expect("second commit should work");

        let path = root.join("layout/workspace/2.snapshot.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["snapshot"]["payload"][0] = serde_json::Value::from(0);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        assert!(matches!(
            store.load_latest("layout", "workspace"),
            Err(SnapshotError::CorruptGeneration { generation: 2, .. })
        ));
        store
            .quarantine("layout", "workspace", 2)
            .expect("corrupt snapshot should quarantine");
        assert_eq!(
            store.load_latest("layout", "workspace").unwrap(),
            Some(snapshot(1, true))
        );
        assert!(root.join("layout/workspace/2.quarantine.json").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_directory_entries_do_not_hide_valid_generations() {
        let root = temporary_root();
        let store = JsonFileStateSnapshotStore::new(&root);
        store.stage(&snapshot(1, false)).expect("stage should work");
        store
            .commit("layout", "workspace", 1)
            .expect("commit should work");
        let directory = root.join("layout/workspace");
        fs::write(directory.join("damaged.snapshot.json"), b"not-json")
            .expect("unknown entry should write");
        assert_eq!(
            store.load_latest("layout", "workspace").unwrap(),
            Some(snapshot(1, true))
        );
        assert_eq!(
            store
                .list_committed_generations("layout", "workspace")
                .unwrap(),
            vec![1]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unsafe_owner_and_scope_segments_are_rejected() {
        let store = JsonFileStateSnapshotStore::new(temporary_root());
        let mut value = snapshot(1, false);
        value.metadata.owner_id = "../layout".into();
        assert!(matches!(
            store.stage(&value),
            Err(SnapshotError::Invalid(_))
        ));
    }

    #[test]
    fn in_memory_store_matches_staged_and_committed_visibility() {
        let store = InMemoryStateSnapshotStore::default();
        store.stage(&snapshot(1, false)).expect("stage should work");
        assert_eq!(store.load_latest("layout", "workspace").unwrap(), None);
        store
            .commit("layout", "workspace", 1)
            .expect("commit should work");
        assert_eq!(
            store.load_latest("layout", "workspace").unwrap(),
            Some(snapshot(1, true))
        );
        store
            .quarantine("layout", "workspace", 1)
            .expect("quarantine should work");
        assert_eq!(store.load_latest("layout", "workspace").unwrap(), None);
    }

    #[test]
    fn require_fsync_uses_the_durability_seam_before_advertising_a_generation() {
        let root = temporary_root();
        let store =
            JsonFileStateSnapshotStore::with_durability(&root, DurabilityMode::RequireFsync);
        let staged = store.stage(&snapshot(1, false));
        if cfg!(unix) {
            staged.expect("Unix directory fsync should be available for the fixture");
            store
                .commit("layout", "workspace", 1)
                .expect("the fsync-backed generation should commit");
            assert!(store
                .load_latest("layout", "workspace")
                .expect("the committed generation should load")
                .is_some());
        } else {
            assert!(staged.is_err());
            assert_eq!(store.load_latest("layout", "workspace").unwrap(), None);
        }
        let _ = fs::remove_dir_all(root);
    }
}
