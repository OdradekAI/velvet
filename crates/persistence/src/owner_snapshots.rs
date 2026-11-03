use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crc32fast::hash;
use velvet_job_system::{JobSnapshot, JobStateStore, JOB_SCHEMA_VERSION};
use velvet_layout_model::LayoutState;
use velvet_workbench_protocol::{ProfileState, PROFILE_SCHEMA_VERSION};

use crate::{
    commit_current_recovery_set, commit_recovery_set_after_owner_change, JsonFileJobStateStore,
    JsonFileLayoutStateStore, JsonFileProfileStateStore, LayoutStateStore, PersistenceError,
    ProfileStateStore, RecoverySetError, RecoverySetStore, StateSnapshot, StateSnapshotMetadata,
    StateSnapshotStore, CORE_RECOVERY_SET_ID,
};

pub const LAYOUT_SNAPSHOT_OWNER: &str = "layout";
pub const LAYOUT_SNAPSHOT_SCOPE: &str = "workspace";
pub const PROFILE_SNAPSHOT_OWNER: &str = "profile";
pub const PROFILE_SNAPSHOT_SCOPE: &str = "user";
pub const JOB_SNAPSHOT_OWNER: &str = "job";
pub const JOB_SNAPSHOT_SCOPE: &str = "runtime";

/// A one-time owner migration participant. It converts a legacy owner file
/// into a staged StateSnapshot without becoming a second live persistence
/// owner. The participant is only used before the canonical RecoverySet is
/// first published.
pub trait OwnerMigrationParticipant: Send + Sync {
    fn owner_id(&self) -> &'static str;
    fn scope(&self) -> &'static str;
    fn migrate(
        &self,
        snapshots: &dyn StateSnapshotStore,
    ) -> Result<Option<StateSnapshot>, PersistenceError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyLayoutMigrationParticipant {
    path: PathBuf,
}

impl LegacyLayoutMigrationParticipant {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl OwnerMigrationParticipant for LegacyLayoutMigrationParticipant {
    fn owner_id(&self) -> &'static str {
        LAYOUT_SNAPSHOT_OWNER
    }

    fn scope(&self) -> &'static str {
        LAYOUT_SNAPSHOT_SCOPE
    }

    fn migrate(
        &self,
        snapshots: &dyn StateSnapshotStore,
    ) -> Result<Option<StateSnapshot>, PersistenceError> {
        migrate_legacy_owner(snapshots, self.owner_id(), self.scope(), || {
            let state = JsonFileLayoutStateStore::new(&self.path)
                .load()?
                .unwrap_or_default();
            let schema_version =
                u16::try_from(state.schema_version).map_err(|_| PersistenceError::CorruptState)?;
            serde_json::to_vec(&state)
                .map_err(|_| PersistenceError::CorruptState)
                .map(|payload| (schema_version, payload))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyProfileMigrationParticipant {
    path: PathBuf,
}

impl LegacyProfileMigrationParticipant {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl OwnerMigrationParticipant for LegacyProfileMigrationParticipant {
    fn owner_id(&self) -> &'static str {
        PROFILE_SNAPSHOT_OWNER
    }

    fn scope(&self) -> &'static str {
        PROFILE_SNAPSHOT_SCOPE
    }

    fn migrate(
        &self,
        snapshots: &dyn StateSnapshotStore,
    ) -> Result<Option<StateSnapshot>, PersistenceError> {
        migrate_legacy_owner(snapshots, self.owner_id(), self.scope(), || {
            let state = JsonFileProfileStateStore::new(&self.path)
                .load()?
                .unwrap_or_default();
            if state.schema_version != PROFILE_SCHEMA_VERSION {
                return Err(PersistenceError::CorruptState);
            }
            serde_json::to_vec(&state)
                .map_err(|_| PersistenceError::CorruptState)
                .map(|payload| (state.schema_version, payload))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyJobMigrationParticipant {
    path: PathBuf,
}

impl LegacyJobMigrationParticipant {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl OwnerMigrationParticipant for LegacyJobMigrationParticipant {
    fn owner_id(&self) -> &'static str {
        JOB_SNAPSHOT_OWNER
    }

    fn scope(&self) -> &'static str {
        JOB_SNAPSHOT_SCOPE
    }

    fn migrate(
        &self,
        snapshots: &dyn StateSnapshotStore,
    ) -> Result<Option<StateSnapshot>, PersistenceError> {
        migrate_legacy_owner(snapshots, self.owner_id(), self.scope(), || {
            let jobs = JsonFileJobStateStore::new(&self.path)
                .load()
                .map_err(|_| PersistenceError::CorruptState)?;
            serde_json::to_vec(&jobs)
                .map_err(|_| PersistenceError::CorruptState)
                .map(|payload| (JOB_SCHEMA_VERSION, payload))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerMigrationReport {
    pub imported: Vec<String>,
    pub recovery_set_created: bool,
}

pub fn migrate_legacy_owner_snapshots(
    snapshots: &dyn StateSnapshotStore,
    recovery_sets: &dyn RecoverySetStore,
    participants: &[&dyn OwnerMigrationParticipant],
    created_at_ms: u64,
) -> Result<OwnerMigrationReport, PersistenceError> {
    if recovery_sets
        .load_latest(CORE_RECOVERY_SET_ID)
        .map_err(recovery_set_error)?
        .is_some()
    {
        return Ok(OwnerMigrationReport {
            imported: Vec::new(),
            recovery_set_created: false,
        });
    }

    let mut staged = Vec::new();
    for participant in participants {
        if let Some(mut snapshot) = participant.migrate(snapshots)? {
            snapshot.metadata.created_at_ms = created_at_ms;
            snapshots.stage(&snapshot).map_err(snapshot_error)?;
            staged.push(snapshot);
        }
    }
    for snapshot in &staged {
        snapshots
            .commit(
                &snapshot.metadata.owner_id,
                &snapshot.metadata.scope,
                snapshot.metadata.generation,
            )
            .map_err(snapshot_error)?;
    }

    commit_current_recovery_set(
        snapshots,
        recovery_sets,
        CORE_RECOVERY_SET_ID,
        created_at_ms,
        &[
            (LAYOUT_SNAPSHOT_OWNER, LAYOUT_SNAPSHOT_SCOPE),
            (PROFILE_SNAPSHOT_OWNER, PROFILE_SNAPSHOT_SCOPE),
            (JOB_SNAPSHOT_OWNER, JOB_SNAPSHOT_SCOPE),
        ],
    )
    .map_err(recovery_set_error)?;

    Ok(OwnerMigrationReport {
        imported: staged
            .iter()
            .map(|snapshot| format!("{}:{}", snapshot.metadata.owner_id, snapshot.metadata.scope))
            .collect(),
        recovery_set_created: true,
    })
}

fn migrate_legacy_owner<F>(
    snapshots: &dyn StateSnapshotStore,
    owner_id: &'static str,
    scope: &'static str,
    load: F,
) -> Result<Option<StateSnapshot>, PersistenceError>
where
    F: FnOnce() -> Result<(u16, Vec<u8>), PersistenceError>,
{
    if snapshots
        .load_latest(owner_id, scope)
        .map_err(snapshot_error)?
        .is_some()
    {
        return Ok(None);
    }
    let (schema_version, payload) = load()?;
    Ok(Some(StateSnapshot {
        metadata: StateSnapshotMetadata {
            owner_id: owner_id.into(),
            scope: scope.into(),
            schema_version,
            generation: 1,
            created_at_ms: 0,
            checksum: hash(&payload),
            committed: false,
        },
        payload,
    }))
}

pub struct SnapshotLayoutStateStore {
    snapshots: Arc<dyn StateSnapshotStore>,
    recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    next_generation: Mutex<u64>,
}

impl SnapshotLayoutStateStore {
    pub fn new(
        snapshots: Arc<dyn StateSnapshotStore>,
        recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    ) -> Self {
        let next_generation = next_snapshot_generation(
            snapshots.as_ref(),
            LAYOUT_SNAPSHOT_OWNER,
            LAYOUT_SNAPSHOT_SCOPE,
        );
        Self {
            snapshots,
            recovery_sets,
            next_generation: Mutex::new(next_generation),
        }
    }
}

impl LayoutStateStore for SnapshotLayoutStateStore {
    fn load(&self) -> Result<Option<LayoutState>, PersistenceError> {
        match load_scoped_snapshot(
            self.snapshots.as_ref(),
            self.recovery_sets.as_deref(),
            LAYOUT_SNAPSHOT_OWNER,
            LAYOUT_SNAPSHOT_SCOPE,
        )? {
            Some(snapshot) => serde_json::from_slice(&snapshot.payload)
                .map(Some)
                .map_err(|_| PersistenceError::CorruptState),
            None => Ok(None),
        }
    }

    fn save(&self, state: &LayoutState) -> Result<(), PersistenceError> {
        let payload = serde_json::to_vec(state).map_err(|_| PersistenceError::CorruptState)?;
        let schema_version =
            u16::try_from(state.schema_version).map_err(|_| PersistenceError::CorruptState)?;
        let mut next_generation = self
            .next_generation
            .lock()
            .map_err(|_| PersistenceError::Unavailable)?;
        let generation = (*next_generation).max(1);
        *next_generation = generation.saturating_add(1);
        let snapshot = make_snapshot(
            LAYOUT_SNAPSHOT_OWNER,
            LAYOUT_SNAPSHOT_SCOPE,
            schema_version,
            generation,
            payload,
        );
        self.snapshots
            .stage(&snapshot)
            .and_then(|_| {
                self.snapshots.commit(
                    LAYOUT_SNAPSHOT_OWNER,
                    LAYOUT_SNAPSHOT_SCOPE,
                    snapshot.metadata.generation,
                )
            })
            .map_err(snapshot_error)?;
        commit_recovery_set_if_ready(
            self.snapshots.as_ref(),
            self.recovery_sets.as_deref(),
            LAYOUT_SNAPSHOT_OWNER,
            LAYOUT_SNAPSHOT_SCOPE,
        )?;
        Ok(())
    }
}

pub struct SnapshotProfileStateStore {
    snapshots: Arc<dyn StateSnapshotStore>,
    recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    next_generation: Mutex<u64>,
}

impl SnapshotProfileStateStore {
    pub fn new(
        snapshots: Arc<dyn StateSnapshotStore>,
        recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    ) -> Self {
        let next_generation = next_snapshot_generation(
            snapshots.as_ref(),
            PROFILE_SNAPSHOT_OWNER,
            PROFILE_SNAPSHOT_SCOPE,
        );
        Self {
            snapshots,
            recovery_sets,
            next_generation: Mutex::new(next_generation),
        }
    }
}

impl ProfileStateStore for SnapshotProfileStateStore {
    fn load(&self) -> Result<Option<ProfileState>, PersistenceError> {
        match load_scoped_snapshot(
            self.snapshots.as_ref(),
            self.recovery_sets.as_deref(),
            PROFILE_SNAPSHOT_OWNER,
            PROFILE_SNAPSHOT_SCOPE,
        )? {
            Some(snapshot) => serde_json::from_slice(&snapshot.payload)
                .map(Some)
                .map_err(|_| PersistenceError::CorruptState),
            None => Ok(None),
        }
    }

    fn save(&self, state: &ProfileState) -> Result<(), PersistenceError> {
        if state.schema_version != PROFILE_SCHEMA_VERSION {
            return Err(PersistenceError::CorruptState);
        }
        let payload = serde_json::to_vec(state).map_err(|_| PersistenceError::CorruptState)?;
        let mut next_generation = self
            .next_generation
            .lock()
            .map_err(|_| PersistenceError::Unavailable)?;
        let generation = (*next_generation).max(1);
        *next_generation = generation.saturating_add(1);
        let snapshot = make_snapshot(
            PROFILE_SNAPSHOT_OWNER,
            PROFILE_SNAPSHOT_SCOPE,
            PROFILE_SCHEMA_VERSION,
            generation,
            payload,
        );
        self.snapshots
            .stage(&snapshot)
            .and_then(|_| {
                self.snapshots.commit(
                    PROFILE_SNAPSHOT_OWNER,
                    PROFILE_SNAPSHOT_SCOPE,
                    snapshot.metadata.generation,
                )
            })
            .map_err(snapshot_error)?;
        commit_recovery_set_if_ready(
            self.snapshots.as_ref(),
            self.recovery_sets.as_deref(),
            PROFILE_SNAPSHOT_OWNER,
            PROFILE_SNAPSHOT_SCOPE,
        )?;
        Ok(())
    }
}

pub struct SnapshotJobStateStore {
    snapshots: Arc<dyn StateSnapshotStore>,
    recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    next_generation: Mutex<u64>,
}

impl SnapshotJobStateStore {
    pub fn new(
        snapshots: Arc<dyn StateSnapshotStore>,
        recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    ) -> Self {
        let next_generation =
            next_snapshot_generation(snapshots.as_ref(), JOB_SNAPSHOT_OWNER, JOB_SNAPSHOT_SCOPE);
        Self {
            snapshots,
            recovery_sets,
            next_generation: Mutex::new(next_generation),
        }
    }
}

impl JobStateStore for SnapshotJobStateStore {
    fn load(&self) -> Result<Vec<JobSnapshot>, String> {
        match load_scoped_snapshot(
            self.snapshots.as_ref(),
            self.recovery_sets.as_deref(),
            JOB_SNAPSHOT_OWNER,
            JOB_SNAPSHOT_SCOPE,
        )
        .map_err(|error| format!("{error:?}"))?
        {
            Some(snapshot) => serde_json::from_slice(&snapshot.payload)
                .map_err(|error| format!("job snapshot payload is invalid: {error}")),
            None => Ok(Vec::new()),
        }
    }

    fn save(&self, jobs: &[JobSnapshot]) -> Result<(), String> {
        let payload = serde_json::to_vec(jobs).map_err(|error| error.to_string())?;
        let mut next_generation = self
            .next_generation
            .lock()
            .map_err(|_| "job snapshot generation lock poisoned".to_string())?;
        let generation = (*next_generation).max(1);
        *next_generation = generation.saturating_add(1);
        let snapshot = make_snapshot(
            JOB_SNAPSHOT_OWNER,
            JOB_SNAPSHOT_SCOPE,
            JOB_SCHEMA_VERSION,
            generation,
            payload,
        );
        self.snapshots
            .stage(&snapshot)
            .and_then(|_| {
                self.snapshots.commit(
                    JOB_SNAPSHOT_OWNER,
                    JOB_SNAPSHOT_SCOPE,
                    snapshot.metadata.generation,
                )
            })
            .map_err(|error| format!("{error:?}"))?;
        commit_recovery_set_if_ready(
            self.snapshots.as_ref(),
            self.recovery_sets.as_deref(),
            JOB_SNAPSHOT_OWNER,
            JOB_SNAPSHOT_SCOPE,
        )
        .map_err(|error| format!("{error:?}"))
    }
}

fn make_snapshot(
    owner_id: &str,
    scope: &str,
    schema_version: u16,
    generation: u64,
    payload: Vec<u8>,
) -> StateSnapshot {
    StateSnapshot {
        metadata: StateSnapshotMetadata {
            owner_id: owner_id.into(),
            scope: scope.into(),
            schema_version,
            generation,
            created_at_ms: 0,
            checksum: hash(&payload),
            committed: false,
        },
        payload,
    }
}

fn next_snapshot_generation(
    snapshots: &dyn StateSnapshotStore,
    owner_id: &str,
    scope: &str,
) -> u64 {
    snapshots
        .list_committed_generations(owner_id, scope)
        .ok()
        .and_then(|generations| generations.into_iter().max())
        .map(|generation| generation.saturating_add(1).max(1))
        .unwrap_or(1)
}

fn load_scoped_snapshot(
    snapshots: &dyn StateSnapshotStore,
    recovery_sets: Option<&dyn RecoverySetStore>,
    owner_id: &str,
    scope: &str,
) -> Result<Option<StateSnapshot>, PersistenceError> {
    if let Some(recovery_sets) = recovery_sets {
        if let Some(recovery_set) = recovery_sets
            .load_latest(CORE_RECOVERY_SET_ID)
            .map_err(recovery_set_error)?
        {
            let key = format!("{owner_id}:{scope}");
            let Some(member) = recovery_set.members.get(&key) else {
                return Err(PersistenceError::CorruptState);
            };
            let snapshot = snapshots
                .load_generation(owner_id, scope, member.generation)
                .map_err(|error| snapshot_error_for_owner(owner_id, scope, error));
            return match snapshot? {
                Some(snapshot) => Ok(Some(snapshot)),
                None => Err(PersistenceError::CorruptState),
            };
        }
        // Until the first complete RecoverySet exists, staged or individually
        // committed owner generations are not authoritative. Composition must
        // publish the bootstrap set before constructing the live owner stores.
        return Ok(None);
    }
    snapshots
        .load_latest(owner_id, scope)
        .map_err(snapshot_error)
}

fn commit_recovery_set_if_ready(
    snapshots: &dyn StateSnapshotStore,
    recovery_sets: Option<&dyn RecoverySetStore>,
    changed_owner: &str,
    changed_scope: &str,
) -> Result<(), PersistenceError> {
    let Some(recovery_sets) = recovery_sets else {
        return Ok(());
    };
    match commit_recovery_set_after_owner_change(
        snapshots,
        recovery_sets,
        CORE_RECOVERY_SET_ID,
        0,
        &[
            (LAYOUT_SNAPSHOT_OWNER, LAYOUT_SNAPSHOT_SCOPE),
            (PROFILE_SNAPSHOT_OWNER, PROFILE_SNAPSHOT_SCOPE),
            (JOB_SNAPSHOT_OWNER, JOB_SNAPSHOT_SCOPE),
        ],
        changed_owner,
        changed_scope,
    ) {
        Ok(_) | Err(RecoverySetError::Incomplete(_)) => Ok(()),
        Err(error) => Err(recovery_set_error(error)),
    }
}

fn snapshot_error(error: crate::SnapshotError) -> PersistenceError {
    match error {
        crate::SnapshotError::Invalid(_)
        | crate::SnapshotError::Corrupt(_)
        | crate::SnapshotError::CorruptGeneration { .. } => PersistenceError::CorruptState,
        crate::SnapshotError::Unavailable(_) | crate::SnapshotError::NotFound => {
            PersistenceError::Unavailable
        }
    }
}

fn snapshot_error_for_owner(
    owner_id: &str,
    scope: &str,
    error: crate::SnapshotError,
) -> PersistenceError {
    match error {
        crate::SnapshotError::CorruptGeneration { generation, .. } => {
            PersistenceError::CorruptOwner {
                owner_id: owner_id.into(),
                scope: scope.into(),
                generation,
            }
        }
        other => snapshot_error(other),
    }
}

fn recovery_set_error(error: RecoverySetError) -> PersistenceError {
    match error {
        RecoverySetError::Invalid(_)
        | RecoverySetError::Corrupt(_)
        | RecoverySetError::Incomplete(_) => PersistenceError::CorruptState,
        RecoverySetError::Unavailable(_) | RecoverySetError::NotFound => {
            PersistenceError::Unavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InMemoryRecoverySetStore, InMemoryStateSnapshotStore, RecoverySet, RecoverySetMember,
    };
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use velvet_layout_model::{LayoutState, WorkbenchAppId};
    use velvet_workbench_protocol::ProfileState;

    fn temporary_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "velvet-owner-migration-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn layout_profile_and_job_stores_publish_one_recovery_set_after_each_owner_exists() {
        let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
        let recovery_sets = Arc::new(InMemoryRecoverySetStore::default());
        let layout = SnapshotLayoutStateStore::new(snapshots.clone(), Some(recovery_sets.clone()));
        let profile =
            SnapshotProfileStateStore::new(snapshots.clone(), Some(recovery_sets.clone()));
        let jobs = SnapshotJobStateStore::new(snapshots, Some(recovery_sets.clone()));

        layout
            .save(&LayoutState::default())
            .expect("layout should publish a snapshot");
        profile
            .save(&ProfileState::default())
            .expect("profile should publish a snapshot");
        jobs.save(&[]).expect("jobs should publish a snapshot");

        let recovery_set = recovery_sets
            .load_latest(CORE_RECOVERY_SET_ID)
            .expect("recovery set should be readable")
            .expect("all owners should publish a recovery set");
        assert_eq!(recovery_set.members.len(), 3);
        assert_eq!(
            layout.load().expect("layout should load").unwrap(),
            LayoutState::default()
        );
        assert_eq!(
            profile.load().expect("profile should load").unwrap(),
            ProfileState::default()
        );
        assert!(jobs.load().expect("jobs should load").is_empty());
    }

    #[test]
    fn an_incomplete_committed_recovery_set_does_not_fall_back_to_legacy_state() {
        let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
        let recovery_sets = Arc::new(InMemoryRecoverySetStore::default());
        let layout = SnapshotLayoutStateStore::new(snapshots.clone(), Some(recovery_sets.clone()));
        let profile =
            SnapshotProfileStateStore::new(snapshots.clone(), Some(recovery_sets.clone()));
        let state = LayoutState::default();
        let payload = serde_json::to_vec(&state).expect("layout should encode");
        let snapshot = make_snapshot(
            LAYOUT_SNAPSHOT_OWNER,
            LAYOUT_SNAPSHOT_SCOPE,
            state.schema_version as u16,
            1,
            payload,
        );
        snapshots.stage(&snapshot).expect("snapshot should stage");
        snapshots
            .commit(LAYOUT_SNAPSHOT_OWNER, LAYOUT_SNAPSHOT_SCOPE, 1)
            .expect("snapshot should commit");
        let mut members = BTreeMap::new();
        members.insert(
            format!("{LAYOUT_SNAPSHOT_OWNER}:{LAYOUT_SNAPSHOT_SCOPE}"),
            RecoverySetMember {
                owner_id: LAYOUT_SNAPSHOT_OWNER.into(),
                scope: LAYOUT_SNAPSHOT_SCOPE.into(),
                generation: 1,
                schema_version: snapshot.metadata.schema_version,
                checksum: snapshot.metadata.checksum,
            },
        );
        let incomplete = RecoverySet {
            set_id: CORE_RECOVERY_SET_ID.into(),
            generation: 1,
            created_at_ms: 0,
            members,
            committed: false,
        };
        recovery_sets
            .stage(&incomplete)
            .expect("incomplete set should stage for the fixture");
        recovery_sets
            .commit(CORE_RECOVERY_SET_ID, 1)
            .expect("incomplete set should commit as a generic index");

        assert!(layout
            .load()
            .expect("the referenced layout should load")
            .is_some());
        assert_eq!(profile.load(), Err(PersistenceError::CorruptState));
    }

    #[test]
    fn bootstrap_does_not_expose_an_owner_snapshot_before_the_first_recovery_set() {
        let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
        let recovery_sets = Arc::new(InMemoryRecoverySetStore::default());
        let layout = SnapshotLayoutStateStore::new(snapshots, Some(recovery_sets));

        let snapshot_state = LayoutState {
            active_app: WorkbenchAppId::AssetManager,
            ..LayoutState::default()
        };
        layout
            .save(&snapshot_state)
            .expect("the owner snapshot should stage");

        assert_eq!(layout.load().expect("bootstrap should be empty"), None);
    }

    #[test]
    fn legacy_owner_migration_publishes_all_owners_once_and_is_idempotent() {
        let root = temporary_root();
        let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
        let recovery_sets = Arc::new(InMemoryRecoverySetStore::default());
        let layout_path = root.join("workbench-layout.json");
        let profile_path = root.join("profile.json");
        let jobs_path = root.join("jobs.json");
        let legacy_layout = JsonFileLayoutStateStore::new(&layout_path);
        let legacy_profile = JsonFileProfileStateStore::new(&profile_path);
        let legacy_jobs = JsonFileJobStateStore::new(&jobs_path);
        let layout_state = LayoutState {
            active_app: WorkbenchAppId::FeedWorkbench,
            ..LayoutState::default()
        };
        legacy_layout
            .save(&layout_state)
            .expect("legacy layout should save");
        legacy_profile
            .save(&ProfileState {
                locale: "zh-Hans".into(),
                plugin_developer_mode: None,
                ..ProfileState::default()
            })
            .expect("legacy profile should save");
        legacy_jobs.save(&[]).expect("legacy jobs should save");

        let participants: Vec<Box<dyn OwnerMigrationParticipant>> = vec![
            Box::new(LegacyLayoutMigrationParticipant::new(&layout_path)),
            Box::new(LegacyProfileMigrationParticipant::new(&profile_path)),
            Box::new(LegacyJobMigrationParticipant::new(&jobs_path)),
        ];
        let refs = participants
            .iter()
            .map(|participant| participant.as_ref() as &dyn OwnerMigrationParticipant)
            .collect::<Vec<_>>();
        let report =
            migrate_legacy_owner_snapshots(snapshots.as_ref(), recovery_sets.as_ref(), &refs, 42)
                .expect("migration should publish a complete recovery set");
        assert_eq!(report.imported.len(), 3);
        assert!(report.recovery_set_created);
        let set = recovery_sets
            .load_latest(CORE_RECOVERY_SET_ID)
            .expect("recovery set should load")
            .expect("migration should publish a recovery set");
        assert_eq!(set.members.len(), 3);

        let layout = SnapshotLayoutStateStore::new(snapshots.clone(), Some(recovery_sets.clone()));
        let profile =
            SnapshotProfileStateStore::new(snapshots.clone(), Some(recovery_sets.clone()));
        assert_eq!(
            layout.load().unwrap().unwrap().active_app,
            WorkbenchAppId::FeedWorkbench
        );
        assert_eq!(profile.load().unwrap().unwrap().locale, "zh-Hans");

        let second =
            migrate_legacy_owner_snapshots(snapshots.as_ref(), recovery_sets.as_ref(), &refs, 43)
                .expect("migration should be idempotent");
        assert!(second.imported.is_empty());
        assert!(!second.recovery_set_created);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_legacy_owner_migration_does_not_publish_a_recovery_set() {
        let root = temporary_root();
        std::fs::create_dir_all(&root).expect("migration fixture directory should exist");
        std::fs::write(root.join("workbench-layout.json"), b"not-json")
            .expect("corrupt legacy fixture should write");
        let snapshots = InMemoryStateSnapshotStore::default();
        let recovery_sets = InMemoryRecoverySetStore::default();
        let participant = LegacyLayoutMigrationParticipant::new(root.join("workbench-layout.json"));
        let refs: [&dyn OwnerMigrationParticipant; 1] = [&participant];
        assert!(migrate_legacy_owner_snapshots(&snapshots, &recovery_sets, &refs, 42).is_err());
        assert_eq!(
            recovery_sets.load_latest(CORE_RECOVERY_SET_ID).unwrap(),
            None
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
