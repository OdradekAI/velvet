use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::durability::{write_atomic, DurabilityMode};
use crate::snapshot::{SnapshotError, StateSnapshot, StateSnapshotStore};

pub const RECOVERY_SET_FORMAT_VERSION: u16 = 1;
pub const CORE_RECOVERY_SET_ID: &str = "core";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoverySetMember {
    pub owner_id: String,
    pub scope: String,
    pub generation: u64,
    pub schema_version: u16,
    pub checksum: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoverySet {
    pub set_id: String,
    pub generation: u64,
    pub created_at_ms: u64,
    pub members: BTreeMap<String, RecoverySetMember>,
    pub committed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoverySetError {
    Invalid(String),
    Incomplete(String),
    Unavailable(String),
    Corrupt(String),
    NotFound,
}

pub trait RecoverySetStore: Send + Sync {
    fn stage(&self, recovery_set: &RecoverySet) -> Result<(), RecoverySetError>;
    fn commit(&self, set_id: &str, generation: u64) -> Result<(), RecoverySetError>;
    fn load_latest(&self, set_id: &str) -> Result<Option<RecoverySet>, RecoverySetError>;
    fn load_previous(&self, set_id: &str) -> Result<Option<RecoverySet>, RecoverySetError>;
    fn restore_previous(&self, set_id: &str) -> Result<Option<RecoverySet>, RecoverySetError>;
    fn quarantine(&self, set_id: &str, generation: u64) -> Result<(), RecoverySetError>;
}

/// Stages a related group of owner snapshots and publishes its RecoverySet
/// pointer only after every staged generation has committed. Readers that use
/// the pointer therefore keep seeing the previous set while the transaction
/// is incomplete.
pub struct RecoverySetTransaction<'a> {
    snapshot_store: &'a dyn StateSnapshotStore,
    set_store: &'a dyn RecoverySetStore,
    set_id: String,
    created_at_ms: u64,
    scopes: Vec<(String, String)>,
    changed: BTreeMap<String, (String, String, u64)>,
}

impl<'a> RecoverySetTransaction<'a> {
    pub fn new(
        snapshot_store: &'a dyn StateSnapshotStore,
        set_store: &'a dyn RecoverySetStore,
        set_id: impl Into<String>,
        created_at_ms: u64,
        scopes: &[(&str, &str)],
    ) -> Result<Self, RecoverySetError> {
        if scopes.is_empty() {
            return Err(RecoverySetError::Invalid(
                "a recovery set transaction must contain at least one owner".into(),
            ));
        }
        let mut normalized = Vec::with_capacity(scopes.len());
        let mut seen = BTreeMap::new();
        for (owner_id, scope) in scopes {
            validate_segment(owner_id, "owner id")?;
            validate_segment(scope, "scope")?;
            let key = format!("{owner_id}:{scope}");
            if seen.insert(key, ()).is_some() {
                return Err(RecoverySetError::Invalid(
                    "a recovery set transaction cannot repeat an owner scope".into(),
                ));
            }
            normalized.push(((*owner_id).to_owned(), (*scope).to_owned()));
        }
        let set_id = set_id.into();
        validate_segment(&set_id, "recovery set id")?;
        Ok(Self {
            snapshot_store,
            set_store,
            set_id,
            created_at_ms,
            scopes: normalized,
            changed: BTreeMap::new(),
        })
    }

    pub fn stage(&mut self, snapshot: &StateSnapshot) -> Result<(), RecoverySetError> {
        let owner_id = &snapshot.metadata.owner_id;
        let scope = &snapshot.metadata.scope;
        if !self.scopes.iter().any(|(expected_owner, expected_scope)| {
            expected_owner == owner_id && expected_scope == scope
        }) {
            return Err(RecoverySetError::Invalid(format!(
                "snapshot {owner_id}:{scope} is not part of the recovery set transaction"
            )));
        }
        self.snapshot_store
            .stage(snapshot)
            .map_err(map_snapshot_error)?;
        self.changed.insert(
            format!("{owner_id}:{scope}"),
            (
                owner_id.clone(),
                scope.clone(),
                snapshot.metadata.generation,
            ),
        );
        Ok(())
    }

    pub fn commit(self) -> Result<RecoverySet, RecoverySetError> {
        if self.changed.is_empty() {
            return Err(RecoverySetError::Invalid(
                "a recovery set transaction must stage at least one snapshot".into(),
            ));
        }
        for (owner_id, scope, generation) in self.changed.values() {
            self.snapshot_store
                .commit(owner_id, scope, *generation)
                .map_err(map_snapshot_error)?;
        }
        let changed = self
            .changed
            .values()
            .map(|(owner_id, scope, _)| (owner_id.as_str(), scope.as_str()))
            .collect::<Vec<_>>();
        commit_recovery_set_after_owner_changes(
            self.snapshot_store,
            self.set_store,
            &self.set_id,
            self.created_at_ms,
            &self
                .scopes
                .iter()
                .map(|(owner_id, scope)| (owner_id.as_str(), scope.as_str()))
                .collect::<Vec<_>>(),
            &changed,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileRecoverySetStore {
    root: PathBuf,
    durability: DurabilityMode,
}

impl JsonFileRecoverySetStore {
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

    fn set_dir(&self, set_id: &str) -> Result<PathBuf, RecoverySetError> {
        validate_segment(set_id, "recovery set id")?;
        Ok(self.root.join(set_id))
    }

    fn staged_path(&self, set_id: &str, generation: u64) -> Result<PathBuf, RecoverySetError> {
        Ok(self
            .set_dir(set_id)?
            .join(format!("{generation}.stage.json")))
    }

    fn committed_path(&self, set_id: &str, generation: u64) -> Result<PathBuf, RecoverySetError> {
        Ok(self.set_dir(set_id)?.join(format!("{generation}.set.json")))
    }

    fn quarantine_path(&self, set_id: &str, generation: u64) -> Result<PathBuf, RecoverySetError> {
        Ok(self
            .set_dir(set_id)?
            .join(format!("{generation}.quarantine.json")))
    }
}

impl RecoverySetStore for JsonFileRecoverySetStore {
    fn stage(&self, recovery_set: &RecoverySet) -> Result<(), RecoverySetError> {
        validate_set(recovery_set, false)?;
        let directory = self.set_dir(&recovery_set.set_id)?;
        fs::create_dir_all(&directory).map_err(|error| {
            RecoverySetError::Unavailable(format!(
                "recovery set directory could not be created: {error}"
            ))
        })?;
        write_json_atomic(
            &self.staged_path(&recovery_set.set_id, recovery_set.generation)?,
            recovery_set,
            self.durability,
        )
    }

    fn commit(&self, set_id: &str, generation: u64) -> Result<(), RecoverySetError> {
        let staged = self.staged_path(set_id, generation)?;
        let bytes = fs::read(&staged).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => RecoverySetError::NotFound,
            _ => RecoverySetError::Unavailable(format!(
                "staged recovery set could not be read: {error}"
            )),
        })?;
        let mut recovery_set = decode_set(&bytes)?;
        if recovery_set.set_id != set_id || recovery_set.generation != generation {
            return Err(RecoverySetError::Corrupt(
                "staged recovery set identity does not match its commit request".into(),
            ));
        }
        validate_set(&recovery_set, false)?;
        recovery_set.committed = true;
        write_json_atomic(
            &self.committed_path(set_id, generation)?,
            &recovery_set,
            self.durability,
        )?;
        fs::remove_file(staged).map_err(|error| {
            RecoverySetError::Unavailable(format!(
                "staged recovery set could not be retired: {error}"
            ))
        })
    }

    fn load_latest(&self, set_id: &str) -> Result<Option<RecoverySet>, RecoverySetError> {
        self.load_committed(set_id, None)
    }

    fn load_previous(&self, set_id: &str) -> Result<Option<RecoverySet>, RecoverySetError> {
        self.load_committed(set_id, Some(1))
    }

    fn restore_previous(&self, set_id: &str) -> Result<Option<RecoverySet>, RecoverySetError> {
        let Some(previous) = self.load_previous(set_id)? else {
            return Ok(None);
        };
        let generation = self
            .load_latest(set_id)?
            .map(|current| current.generation.saturating_add(1))
            .unwrap_or(1);
        let restored = RecoverySet {
            generation,
            committed: false,
            ..previous
        };
        self.stage(&restored)?;
        self.commit(set_id, generation)?;
        Ok(Some(RecoverySet {
            committed: true,
            ..restored
        }))
    }

    fn quarantine(&self, set_id: &str, generation: u64) -> Result<(), RecoverySetError> {
        let source = self.committed_path(set_id, generation)?;
        if !source.exists() {
            return Err(RecoverySetError::NotFound);
        }
        fs::rename(source, self.quarantine_path(set_id, generation)?).map_err(|error| {
            RecoverySetError::Unavailable(format!("recovery set could not be quarantined: {error}"))
        })
    }
}

impl JsonFileRecoverySetStore {
    fn load_committed(
        &self,
        set_id: &str,
        skip_latest: Option<usize>,
    ) -> Result<Option<RecoverySet>, RecoverySetError> {
        let directory = self.set_dir(set_id)?;
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(RecoverySetError::Unavailable(format!(
                    "recovery set directory could not be read: {error}"
                )))
            }
        };
        let mut sets = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                RecoverySetError::Unavailable(format!(
                    "recovery set entry could not be read: {error}"
                ))
            })?;
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(generation_text) = file_name.strip_suffix(".set.json") else {
                continue;
            };
            let generation = generation_text.parse::<u64>().map_err(|_| {
                RecoverySetError::Corrupt(format!(
                    "recovery set filename has an invalid generation: {file_name}"
                ))
            })?;
            let recovery_set = decode_set(&fs::read(&path).map_err(|error| {
                RecoverySetError::Unavailable(format!("recovery set could not be read: {error}"))
            })?)?;
            validate_set(&recovery_set, true)?;
            if recovery_set.set_id != set_id || recovery_set.generation != generation {
                return Err(RecoverySetError::Corrupt(
                    "recovery set identity does not match its path".into(),
                ));
            }
            sets.push(recovery_set);
        }
        sets.sort_by_key(|recovery_set| recovery_set.generation);
        let index = match skip_latest {
            None => sets.len().checked_sub(1),
            Some(skip) => sets.len().checked_sub(skip + 1),
        };
        Ok(index.and_then(|index| sets.into_iter().nth(index)))
    }
}

#[derive(Default)]
pub struct InMemoryRecoverySetStore {
    sets: Mutex<BTreeMap<(String, u64), RecoverySet>>,
}

impl RecoverySetStore for InMemoryRecoverySetStore {
    fn stage(&self, recovery_set: &RecoverySet) -> Result<(), RecoverySetError> {
        validate_set(recovery_set, false)?;
        self.sets
            .lock()
            .map_err(|_| RecoverySetError::Unavailable("recovery set lock poisoned".into()))?
            .insert(
                (recovery_set.set_id.clone(), recovery_set.generation),
                recovery_set.clone(),
            );
        Ok(())
    }

    fn commit(&self, set_id: &str, generation: u64) -> Result<(), RecoverySetError> {
        let mut sets = self
            .sets
            .lock()
            .map_err(|_| RecoverySetError::Unavailable("recovery set lock poisoned".into()))?;
        let Some(recovery_set) = sets.get_mut(&(set_id.into(), generation)) else {
            return Err(RecoverySetError::NotFound);
        };
        validate_set(recovery_set, false)?;
        recovery_set.committed = true;
        Ok(())
    }

    fn load_latest(&self, set_id: &str) -> Result<Option<RecoverySet>, RecoverySetError> {
        self.load_by_index(set_id, 0)
    }

    fn load_previous(&self, set_id: &str) -> Result<Option<RecoverySet>, RecoverySetError> {
        self.load_by_index(set_id, 1)
    }

    fn restore_previous(&self, set_id: &str) -> Result<Option<RecoverySet>, RecoverySetError> {
        let Some(previous) = self.load_previous(set_id)? else {
            return Ok(None);
        };
        let generation = self
            .load_latest(set_id)?
            .map(|current| current.generation.saturating_add(1))
            .unwrap_or(1);
        let restored = RecoverySet {
            generation,
            committed: false,
            ..previous
        };
        self.stage(&restored)?;
        self.commit(set_id, generation)?;
        Ok(Some(RecoverySet {
            committed: true,
            ..restored
        }))
    }

    fn quarantine(&self, set_id: &str, generation: u64) -> Result<(), RecoverySetError> {
        self.sets
            .lock()
            .map_err(|_| RecoverySetError::Unavailable("recovery set lock poisoned".into()))?
            .remove(&(set_id.into(), generation))
            .map(|_| ())
            .ok_or(RecoverySetError::NotFound)
    }
}

impl InMemoryRecoverySetStore {
    fn load_by_index(
        &self,
        set_id: &str,
        skip: usize,
    ) -> Result<Option<RecoverySet>, RecoverySetError> {
        let sets = self
            .sets
            .lock()
            .map_err(|_| RecoverySetError::Unavailable("recovery set lock poisoned".into()))?;
        let mut values = sets
            .values()
            .filter(|recovery_set| recovery_set.set_id == set_id && recovery_set.committed)
            .cloned()
            .collect::<Vec<_>>();
        values.sort_by_key(|recovery_set| recovery_set.generation);
        Ok(values.into_iter().rev().nth(skip))
    }
}

pub fn commit_current_recovery_set(
    snapshot_store: &dyn StateSnapshotStore,
    set_store: &dyn RecoverySetStore,
    set_id: &str,
    created_at_ms: u64,
    scopes: &[(&str, &str)],
) -> Result<RecoverySet, RecoverySetError> {
    commit_recovery_set_with_owners(
        snapshot_store,
        set_store,
        set_id,
        created_at_ms,
        scopes,
        &[],
    )
}

pub fn commit_recovery_set_after_owner_change(
    snapshot_store: &dyn StateSnapshotStore,
    set_store: &dyn RecoverySetStore,
    set_id: &str,
    created_at_ms: u64,
    scopes: &[(&str, &str)],
    changed_owner: &str,
    changed_scope: &str,
) -> Result<RecoverySet, RecoverySetError> {
    commit_recovery_set_after_owner_changes(
        snapshot_store,
        set_store,
        set_id,
        created_at_ms,
        scopes,
        &[(changed_owner, changed_scope)],
    )
}

pub fn commit_recovery_set_after_owner_changes(
    snapshot_store: &dyn StateSnapshotStore,
    set_store: &dyn RecoverySetStore,
    set_id: &str,
    created_at_ms: u64,
    scopes: &[(&str, &str)],
    changed_owners: &[(&str, &str)],
) -> Result<RecoverySet, RecoverySetError> {
    if changed_owners
        .iter()
        .any(|changed| !scopes.contains(changed))
    {
        return Err(RecoverySetError::Invalid(
            "a changed owner must be part of the recovery set scopes".into(),
        ));
    }
    commit_recovery_set_with_owners(
        snapshot_store,
        set_store,
        set_id,
        created_at_ms,
        scopes,
        changed_owners,
    )
}

fn commit_recovery_set_with_owners(
    snapshot_store: &dyn StateSnapshotStore,
    set_store: &dyn RecoverySetStore,
    set_id: &str,
    created_at_ms: u64,
    scopes: &[(&str, &str)],
    changed_owners: &[(&str, &str)],
) -> Result<RecoverySet, RecoverySetError> {
    if scopes.is_empty() {
        return Err(RecoverySetError::Invalid(
            "a recovery set must contain at least one owner".into(),
        ));
    }
    let current = set_store.load_latest(set_id)?;
    let mut members = BTreeMap::new();
    for (owner_id, scope) in scopes {
        let snapshot = if changed_owners.contains(&(*owner_id, *scope)) {
            snapshot_store.load_latest(owner_id, scope)
        } else if let Some(current) = current.as_ref() {
            let key = format!("{owner_id}:{scope}");
            if let Some(member) = current.members.get(&key) {
                snapshot_store.load_generation(owner_id, scope, member.generation)
            } else {
                snapshot_store.load_latest(owner_id, scope)
            }
        } else {
            snapshot_store.load_latest(owner_id, scope)
        }
        .map_err(map_snapshot_error)?;
        let snapshot = snapshot.ok_or_else(|| {
            RecoverySetError::Incomplete(format!(
                "owner {owner_id} has no committed snapshot for scope {scope}"
            ))
        })?;
        members.insert(
            format!("{owner_id}:{scope}"),
            RecoverySetMember {
                owner_id: snapshot.metadata.owner_id,
                scope: snapshot.metadata.scope,
                generation: snapshot.metadata.generation,
                schema_version: snapshot.metadata.schema_version,
                checksum: snapshot.metadata.checksum,
            },
        );
    }
    let previous_generation = current
        .map(|recovery_set| recovery_set.generation)
        .unwrap_or_default();
    let member_generation = members
        .values()
        .map(|member| member.generation)
        .max()
        .unwrap_or_default();
    let recovery_set = RecoverySet {
        set_id: set_id.into(),
        generation: previous_generation.saturating_add(1).max(member_generation),
        created_at_ms,
        members,
        committed: false,
    };
    set_store.stage(&recovery_set)?;
    set_store.commit(&recovery_set.set_id, recovery_set.generation)?;
    Ok(RecoverySet {
        committed: true,
        ..recovery_set
    })
}

fn validate_set(recovery_set: &RecoverySet, committed: bool) -> Result<(), RecoverySetError> {
    validate_segment(&recovery_set.set_id, "recovery set id")?;
    if recovery_set.generation == 0 || recovery_set.members.is_empty() {
        return Err(RecoverySetError::Invalid(
            "recovery set generation and members must be non-zero".into(),
        ));
    }
    if recovery_set.committed != committed {
        return Err(RecoverySetError::Invalid(
            "recovery set committed marker does not match the requested operation".into(),
        ));
    }
    for (key, member) in &recovery_set.members {
        validate_segment(&member.owner_id, "owner id")?;
        validate_segment(&member.scope, "scope")?;
        if member.generation == 0 || key != &format!("{}:{}", member.owner_id, member.scope) {
            return Err(RecoverySetError::Invalid(
                "recovery set member identity is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn validate_segment(value: &str, label: &str) -> Result<(), RecoverySetError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
        })
    {
        return Err(RecoverySetError::Invalid(format!(
            "{label} contains an unsafe path segment"
        )));
    }
    Ok(())
}

fn decode_set(bytes: &[u8]) -> Result<RecoverySet, RecoverySetError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Envelope {
        format_version: u16,
        recovery_set: RecoverySet,
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|error| {
        RecoverySetError::Corrupt(format!("recovery set JSON is invalid: {error}"))
    })?;
    if envelope.format_version != RECOVERY_SET_FORMAT_VERSION {
        return Err(RecoverySetError::Corrupt(
            "recovery set format version is unsupported".into(),
        ));
    }
    Ok(envelope.recovery_set)
}

fn write_json_atomic(
    path: &Path,
    recovery_set: &RecoverySet,
    durability: DurabilityMode,
) -> Result<(), RecoverySetError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Envelope<'a> {
        format_version: u16,
        recovery_set: &'a RecoverySet,
    }
    let bytes = serde_json::to_vec_pretty(&Envelope {
        format_version: RECOVERY_SET_FORMAT_VERSION,
        recovery_set,
    })
    .map_err(|error| {
        RecoverySetError::Corrupt(format!("recovery set JSON could not be encoded: {error}"))
    })?;
    write_atomic(path, &bytes, durability).map_err(|error| {
        RecoverySetError::Unavailable(format!("recovery set could not be published: {error}"))
    })
}

fn map_snapshot_error(error: SnapshotError) -> RecoverySetError {
    match error {
        SnapshotError::Invalid(message) => RecoverySetError::Invalid(message),
        SnapshotError::Unavailable(message) => RecoverySetError::Unavailable(message),
        SnapshotError::Corrupt(message) => RecoverySetError::Corrupt(message),
        SnapshotError::NotFound => RecoverySetError::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{
        InMemoryStateSnapshotStore, SnapshotError, StateSnapshot, StateSnapshotMetadata,
        StateSnapshotStore,
    };
    use crc32fast::hash;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMPORARY_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let sequence = TEMPORARY_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "velvet-recovery-set-{}-{nonce}-{sequence}",
            std::process::id(),
        ))
    }

    fn snapshot(owner_id: &str, scope: &str, generation: u64) -> StateSnapshot {
        let payload = format!("{owner_id}-{scope}-{generation}").into_bytes();
        StateSnapshot {
            metadata: StateSnapshotMetadata {
                owner_id: owner_id.into(),
                scope: scope.into(),
                schema_version: 1,
                generation,
                created_at_ms: generation,
                checksum: hash(&payload),
                committed: false,
            },
            payload,
        }
    }

    struct FailingCommitSnapshotStore {
        inner: InMemoryStateSnapshotStore,
        fail_profile_commit: AtomicBool,
    }

    impl FailingCommitSnapshotStore {
        fn new() -> Self {
            Self {
                inner: InMemoryStateSnapshotStore::default(),
                fail_profile_commit: AtomicBool::new(false),
            }
        }
    }

    impl StateSnapshotStore for FailingCommitSnapshotStore {
        fn stage(&self, snapshot: &StateSnapshot) -> Result<(), SnapshotError> {
            self.inner.stage(snapshot)
        }

        fn commit(
            &self,
            owner_id: &str,
            scope: &str,
            generation: u64,
        ) -> Result<(), SnapshotError> {
            if owner_id == "profile" && self.fail_profile_commit.load(Ordering::Relaxed) {
                return Err(SnapshotError::Unavailable(
                    "injected profile commit failure".into(),
                ));
            }
            self.inner.commit(owner_id, scope, generation)
        }

        fn load_latest(
            &self,
            owner_id: &str,
            scope: &str,
        ) -> Result<Option<StateSnapshot>, SnapshotError> {
            self.inner.load_latest(owner_id, scope)
        }

        fn load_previous(
            &self,
            owner_id: &str,
            scope: &str,
        ) -> Result<Option<StateSnapshot>, SnapshotError> {
            self.inner.load_previous(owner_id, scope)
        }

        fn load_generation(
            &self,
            owner_id: &str,
            scope: &str,
            generation: u64,
        ) -> Result<Option<StateSnapshot>, SnapshotError> {
            self.inner.load_generation(owner_id, scope, generation)
        }

        fn quarantine(
            &self,
            owner_id: &str,
            scope: &str,
            generation: u64,
        ) -> Result<(), SnapshotError> {
            self.inner.quarantine(owner_id, scope, generation)
        }
    }

    #[test]
    fn commit_barrier_does_not_publish_a_partial_set() {
        let root = temporary_root();
        let snapshots = InMemoryStateSnapshotStore::default();
        for (owner, scope) in [("layout", "workspace"), ("profile", "user")] {
            snapshots.stage(&snapshot(owner, scope, 1)).unwrap();
            snapshots.commit(owner, scope, 1).unwrap();
        }
        let sets = JsonFileRecoverySetStore::new(&root);
        let published = commit_current_recovery_set(
            &snapshots,
            &sets,
            CORE_RECOVERY_SET_ID,
            10,
            &[("layout", "workspace"), ("profile", "user")],
        )
        .unwrap();
        assert_eq!(published.members.len(), 2);
        assert_eq!(
            sets.load_latest(CORE_RECOVERY_SET_ID).unwrap(),
            Some(published)
        );
        assert_eq!(sets.load_previous(CORE_RECOVERY_SET_ID).unwrap(), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn previous_set_can_be_selected_after_a_new_set_is_published() {
        let root = temporary_root();
        let snapshots = InMemoryStateSnapshotStore::default();
        let sets = JsonFileRecoverySetStore::new(&root);
        for generation in [1, 2] {
            for (owner, scope) in [("layout", "workspace"), ("profile", "user")] {
                snapshots
                    .stage(&snapshot(owner, scope, generation))
                    .unwrap();
                snapshots.commit(owner, scope, generation).unwrap();
            }
            commit_current_recovery_set(
                &snapshots,
                &sets,
                CORE_RECOVERY_SET_ID,
                generation,
                &[("layout", "workspace"), ("profile", "user")],
            )
            .unwrap();
        }
        assert_eq!(
            sets.load_previous(CORE_RECOVERY_SET_ID)
                .unwrap()
                .unwrap()
                .generation,
            1
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn multi_owner_transaction_keeps_the_previous_set_until_all_members_commit() {
        let snapshots = InMemoryStateSnapshotStore::default();
        let sets = InMemoryRecoverySetStore::default();
        let scopes = [
            ("layout", "workspace"),
            ("profile", "user"),
            ("job", "runtime"),
        ];
        for (owner, scope) in scopes {
            snapshots.stage(&snapshot(owner, scope, 1)).unwrap();
            snapshots.commit(owner, scope, 1).unwrap();
        }
        let first =
            commit_current_recovery_set(&snapshots, &sets, CORE_RECOVERY_SET_ID, 1, &scopes)
                .unwrap();

        let mut transaction =
            RecoverySetTransaction::new(&snapshots, &sets, CORE_RECOVERY_SET_ID, 2, &scopes)
                .unwrap();
        transaction
            .stage(&snapshot("layout", "workspace", 2))
            .unwrap();
        transaction.stage(&snapshot("profile", "user", 2)).unwrap();
        assert_eq!(
            sets.load_latest(CORE_RECOVERY_SET_ID).unwrap(),
            Some(first.clone())
        );

        let committed = transaction.commit().unwrap();
        assert_ne!(committed.generation, first.generation);
        assert_eq!(
            sets.load_latest(CORE_RECOVERY_SET_ID)
                .unwrap()
                .unwrap()
                .members["layout:workspace"]
                .generation,
            2
        );
        assert_eq!(
            sets.load_latest(CORE_RECOVERY_SET_ID)
                .unwrap()
                .unwrap()
                .members["profile:user"]
                .generation,
            2
        );
        assert_eq!(
            sets.load_latest(CORE_RECOVERY_SET_ID)
                .unwrap()
                .unwrap()
                .members["job:runtime"]
                .generation,
            1
        );
    }

    #[test]
    fn failed_owner_commit_does_not_publish_a_mixed_recovery_set() {
        let snapshots = FailingCommitSnapshotStore::new();
        let sets = InMemoryRecoverySetStore::default();
        let scopes = [("layout", "workspace"), ("profile", "user")];
        for (owner, scope) in scopes {
            snapshots.stage(&snapshot(owner, scope, 1)).unwrap();
            snapshots.commit(owner, scope, 1).unwrap();
        }
        let first =
            commit_current_recovery_set(&snapshots, &sets, CORE_RECOVERY_SET_ID, 1, &scopes)
                .unwrap();

        let mut transaction =
            RecoverySetTransaction::new(&snapshots, &sets, CORE_RECOVERY_SET_ID, 2, &scopes)
                .unwrap();
        transaction
            .stage(&snapshot("layout", "workspace", 2))
            .unwrap();
        transaction.stage(&snapshot("profile", "user", 2)).unwrap();
        snapshots.fail_profile_commit.store(true, Ordering::Relaxed);

        assert!(matches!(
            transaction.commit(),
            Err(RecoverySetError::Unavailable(_))
        ));
        assert_eq!(sets.load_latest(CORE_RECOVERY_SET_ID).unwrap(), Some(first));
    }
}
