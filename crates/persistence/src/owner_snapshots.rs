use std::sync::{Arc, Mutex};

use crc32fast::hash;
use velvet_job_system::{JobSnapshot, JobStateStore, JOB_SCHEMA_VERSION};
use velvet_layout_model::LayoutState;
use velvet_workbench_protocol::{ProfileState, PROFILE_SCHEMA_VERSION};

use crate::{
    commit_recovery_set_after_owner_change, LayoutStateStore, PersistenceError, ProfileStateStore,
    RecoverySetError, RecoverySetStore, StateSnapshot, StateSnapshotMetadata, StateSnapshotStore,
    CORE_RECOVERY_SET_ID,
};

pub const LAYOUT_SNAPSHOT_OWNER: &str = "layout";
pub const LAYOUT_SNAPSHOT_SCOPE: &str = "workspace";
pub const PROFILE_SNAPSHOT_OWNER: &str = "profile";
pub const PROFILE_SNAPSHOT_SCOPE: &str = "user";
pub const JOB_SNAPSHOT_OWNER: &str = "job";
pub const JOB_SNAPSHOT_SCOPE: &str = "runtime";

pub struct SnapshotLayoutStateStore {
    snapshots: Arc<dyn StateSnapshotStore>,
    recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    legacy: Arc<dyn LayoutStateStore>,
}

impl SnapshotLayoutStateStore {
    pub fn new(
        snapshots: Arc<dyn StateSnapshotStore>,
        recovery_sets: Option<Arc<dyn RecoverySetStore>>,
        legacy: Arc<dyn LayoutStateStore>,
    ) -> Self {
        Self {
            snapshots,
            recovery_sets,
            legacy,
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
            None => self.legacy.load(),
        }
    }

    fn save(&self, state: &LayoutState) -> Result<(), PersistenceError> {
        let payload = serde_json::to_vec(state).map_err(|_| PersistenceError::CorruptState)?;
        let schema_version =
            u16::try_from(state.schema_version).map_err(|_| PersistenceError::CorruptState)?;
        let snapshot = make_snapshot(
            LAYOUT_SNAPSHOT_OWNER,
            LAYOUT_SNAPSHOT_SCOPE,
            schema_version,
            state.revision.max(1),
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
    legacy: Arc<dyn ProfileStateStore>,
}

impl SnapshotProfileStateStore {
    pub fn new(
        snapshots: Arc<dyn StateSnapshotStore>,
        recovery_sets: Option<Arc<dyn RecoverySetStore>>,
        legacy: Arc<dyn ProfileStateStore>,
    ) -> Self {
        Self {
            snapshots,
            recovery_sets,
            legacy,
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
            None => self.legacy.load(),
        }
    }

    fn save(&self, state: &ProfileState) -> Result<(), PersistenceError> {
        if state.schema_version != PROFILE_SCHEMA_VERSION {
            return Err(PersistenceError::CorruptState);
        }
        let payload = serde_json::to_vec(state).map_err(|_| PersistenceError::CorruptState)?;
        let snapshot = make_snapshot(
            PROFILE_SNAPSHOT_OWNER,
            PROFILE_SNAPSHOT_SCOPE,
            PROFILE_SCHEMA_VERSION,
            state.revision.max(1),
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
    legacy: Arc<dyn JobStateStore>,
    next_generation: Mutex<u64>,
}

impl SnapshotJobStateStore {
    pub fn new(
        snapshots: Arc<dyn StateSnapshotStore>,
        recovery_sets: Option<Arc<dyn RecoverySetStore>>,
        legacy: Arc<dyn JobStateStore>,
    ) -> Self {
        let next_generation = snapshots
            .load_latest(JOB_SNAPSHOT_OWNER, JOB_SNAPSHOT_SCOPE)
            .ok()
            .flatten()
            .map(|snapshot| snapshot.metadata.generation.saturating_add(1))
            .unwrap_or(1);
        Self {
            snapshots,
            recovery_sets,
            legacy,
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
            None => self.legacy.load(),
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
                .map_err(snapshot_error);
            return match snapshot? {
                Some(snapshot) => Ok(Some(snapshot)),
                None => Err(PersistenceError::CorruptState),
            };
        }
        // Until the first complete RecoverySet exists, staged or individually
        // committed owner generations are not authoritative. Keep the legacy
        // owner state visible so bootstrap cannot expose a mixed set.
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
        crate::SnapshotError::Invalid(_) | crate::SnapshotError::Corrupt(_) => {
            PersistenceError::CorruptState
        }
        crate::SnapshotError::Unavailable(_) | crate::SnapshotError::NotFound => {
            PersistenceError::Unavailable
        }
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
        InMemoryLayoutStateStore, InMemoryProfileStateStore, InMemoryRecoverySetStore,
        InMemoryStateSnapshotStore, RecoverySet, RecoverySetMember,
    };
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use velvet_job_system::InMemoryJobStateStore;
    use velvet_layout_model::{LayoutState, WorkbenchAppId};
    use velvet_workbench_protocol::ProfileState;

    #[test]
    fn layout_profile_and_job_stores_publish_one_recovery_set_after_each_owner_exists() {
        let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
        let recovery_sets = Arc::new(InMemoryRecoverySetStore::default());
        let layout = SnapshotLayoutStateStore::new(
            snapshots.clone(),
            Some(recovery_sets.clone()),
            Arc::new(InMemoryLayoutStateStore::default()),
        );
        let profile = SnapshotProfileStateStore::new(
            snapshots.clone(),
            Some(recovery_sets.clone()),
            Arc::new(InMemoryProfileStateStore::default()),
        );
        let jobs = SnapshotJobStateStore::new(
            snapshots,
            Some(recovery_sets.clone()),
            Arc::new(InMemoryJobStateStore::default()),
        );

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
        let layout = SnapshotLayoutStateStore::new(
            snapshots.clone(),
            Some(recovery_sets.clone()),
            Arc::new(InMemoryLayoutStateStore::default()),
        );
        let profile = SnapshotProfileStateStore::new(
            snapshots.clone(),
            Some(recovery_sets.clone()),
            Arc::new(InMemoryProfileStateStore::default()),
        );
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
        let legacy = Arc::new(InMemoryLayoutStateStore::default());
        let legacy_state = LayoutState {
            active_app: WorkbenchAppId::FeedWorkbench,
            ..LayoutState::default()
        };
        legacy
            .save(&legacy_state)
            .expect("legacy state should save");
        let layout = SnapshotLayoutStateStore::new(snapshots, Some(recovery_sets), legacy);

        let snapshot_state = LayoutState {
            active_app: WorkbenchAppId::AssetManager,
            ..LayoutState::default()
        };
        layout
            .save(&snapshot_state)
            .expect("the owner snapshot should stage");

        assert_eq!(
            layout
                .load()
                .expect("legacy state should remain visible")
                .expect("legacy state should exist")
                .active_app,
            WorkbenchAppId::FeedWorkbench
        );
    }
}
