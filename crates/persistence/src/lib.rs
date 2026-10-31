use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use velvet_job_system::{JobSnapshot, JobStateStore};
use velvet_layout_model::LayoutState;
use velvet_workbench_protocol::{ProfileState, RecoveryJournalState, RECOVERY_SCHEMA_VERSION};

mod durability;
mod owner_snapshots;
mod recovery_set;
mod snapshot;

pub use durability::DurabilityMode;

pub use owner_snapshots::{
    SnapshotJobStateStore, SnapshotLayoutStateStore, SnapshotProfileStateStore, JOB_SNAPSHOT_OWNER,
    JOB_SNAPSHOT_SCOPE, LAYOUT_SNAPSHOT_OWNER, LAYOUT_SNAPSHOT_SCOPE, PROFILE_SNAPSHOT_OWNER,
    PROFILE_SNAPSHOT_SCOPE,
};
pub use recovery_set::{
    commit_current_recovery_set, commit_recovery_set_after_owner_change, InMemoryRecoverySetStore,
    JsonFileRecoverySetStore, RecoverySet, RecoverySetError, RecoverySetMember, RecoverySetStore,
    CORE_RECOVERY_SET_ID, RECOVERY_SET_FORMAT_VERSION,
};
pub use snapshot::{
    InMemoryStateSnapshotStore, JsonFileStateSnapshotStore, SnapshotError, StateSnapshot,
    StateSnapshotMetadata, StateSnapshotStore, MAX_STATE_SNAPSHOT_BYTES,
    STATE_SNAPSHOT_FORMAT_VERSION,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistenceError {
    Unavailable,
    CorruptState,
}

/// Storage adapter for the lifecycle journal. The application core owns the
/// recovery state machine; this trait only persists its versioned bytes.
pub trait RecoveryJournalStore: Send + Sync {
    fn load(&self) -> Result<RecoveryJournalState, PersistenceError>;
    fn save(&self, state: &RecoveryJournalState) -> Result<(), PersistenceError>;
}

#[derive(Default)]
pub struct InMemoryRecoveryJournalStore {
    state: std::sync::Mutex<RecoveryJournalState>,
}

impl RecoveryJournalStore for InMemoryRecoveryJournalStore {
    fn load(&self) -> Result<RecoveryJournalState, PersistenceError> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| PersistenceError::Unavailable)
    }

    fn save(&self, state: &RecoveryJournalState) -> Result<(), PersistenceError> {
        self.state
            .lock()
            .map(|mut current| *current = state.clone())
            .map_err(|_| PersistenceError::Unavailable)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileRecoveryJournalStore {
    path: PathBuf,
}

impl JsonFileRecoveryJournalStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl RecoveryJournalStore for JsonFileRecoveryJournalStore {
    fn load(&self) -> Result<RecoveryJournalState, PersistenceError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RecoveryJournalState::default())
            }
            Err(_) => return Err(PersistenceError::Unavailable),
        };
        let state = serde_json::from_slice::<RecoveryJournalState>(&bytes)
            .map_err(|_| PersistenceError::CorruptState)?;
        if state.schema_version != RECOVERY_SCHEMA_VERSION {
            return Err(PersistenceError::CorruptState);
        }
        Ok(state)
    }

    fn save(&self, state: &RecoveryJournalState) -> Result<(), PersistenceError> {
        if state.schema_version != RECOVERY_SCHEMA_VERSION {
            return Err(PersistenceError::CorruptState);
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| PersistenceError::Unavailable)?;
        }
        let bytes = serde_json::to_vec_pretty(state).map_err(|_| PersistenceError::CorruptState)?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, bytes).map_err(|_| PersistenceError::Unavailable)?;
        fs::rename(temporary, &self.path).map_err(|_| PersistenceError::Unavailable)
    }
}

pub const ASSET_SOURCE_LOCATION_SCHEMA_VERSION: u16 = 1;

/// Storage-facing representation of an application-owned Asset source.
///
/// The persistence adapter stores the path because the application core owns
/// the source-location policy. It never projects this record to the renderer
/// or an extension runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetSourceLocationRecord {
    pub asset_id: String,
    pub source_path: String,
    pub content_kind: String,
    pub fingerprint: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetSourceLocationState {
    schema_version: u16,
    locations: Vec<AssetSourceLocationRecord>,
}

pub trait AssetSourceLocationStore: Send + Sync {
    fn load(&self) -> Result<Vec<AssetSourceLocationRecord>, PersistenceError>;
    fn save(&self, locations: &[AssetSourceLocationRecord]) -> Result<(), PersistenceError>;
}

#[derive(Default)]
pub struct InMemoryAssetSourceLocationStore {
    locations: std::sync::Mutex<Vec<AssetSourceLocationRecord>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileAssetSourceLocationStore {
    path: PathBuf,
}

impl JsonFileAssetSourceLocationStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl AssetSourceLocationStore for InMemoryAssetSourceLocationStore {
    fn load(&self) -> Result<Vec<AssetSourceLocationRecord>, PersistenceError> {
        self.locations
            .lock()
            .map(|locations| locations.clone())
            .map_err(|_| PersistenceError::Unavailable)
    }

    fn save(&self, locations: &[AssetSourceLocationRecord]) -> Result<(), PersistenceError> {
        self.locations
            .lock()
            .map(|mut current| *current = locations.to_vec())
            .map_err(|_| PersistenceError::Unavailable)
    }
}

impl AssetSourceLocationStore for JsonFileAssetSourceLocationStore {
    fn load(&self) -> Result<Vec<AssetSourceLocationRecord>, PersistenceError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(PersistenceError::Unavailable),
        };

        let state = serde_json::from_slice::<AssetSourceLocationState>(&bytes)
            .map_err(|_| PersistenceError::CorruptState)?;
        if state.schema_version != ASSET_SOURCE_LOCATION_SCHEMA_VERSION {
            return Err(PersistenceError::CorruptState);
        }
        Ok(state.locations)
    }

    fn save(&self, locations: &[AssetSourceLocationRecord]) -> Result<(), PersistenceError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| PersistenceError::Unavailable)?;
        }

        let state = AssetSourceLocationState {
            schema_version: ASSET_SOURCE_LOCATION_SCHEMA_VERSION,
            locations: locations.to_vec(),
        };
        let bytes =
            serde_json::to_vec_pretty(&state).map_err(|_| PersistenceError::CorruptState)?;
        let temporary_path = self.path.with_extension("json.tmp");
        fs::write(&temporary_path, bytes).map_err(|_| PersistenceError::Unavailable)?;
        fs::rename(temporary_path, &self.path).map_err(|_| PersistenceError::Unavailable)
    }
}

pub trait LayoutStateStore: Send + Sync {
    fn load(&self) -> Result<Option<LayoutState>, PersistenceError>;
    fn save(&self, state: &LayoutState) -> Result<(), PersistenceError>;
}

#[derive(Default)]
pub struct InMemoryLayoutStateStore {
    state: std::sync::Mutex<Option<LayoutState>>,
}

/// JSON-backed layout storage for the native desktop application.
///
/// The path is supplied by the platform composition root so this crate does
/// not know how an operating system chooses an application data directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileLayoutStateStore {
    path: PathBuf,
}

impl JsonFileLayoutStateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl LayoutStateStore for JsonFileLayoutStateStore {
    fn load(&self) -> Result<Option<LayoutState>, PersistenceError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(PersistenceError::Unavailable),
        };

        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| PersistenceError::CorruptState)
    }

    fn save(&self, state: &LayoutState) -> Result<(), PersistenceError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| PersistenceError::Unavailable)?;
        }

        let bytes = serde_json::to_vec_pretty(state).map_err(|_| PersistenceError::CorruptState)?;
        let temporary_path = self.path.with_extension("json.tmp");
        fs::write(&temporary_path, bytes).map_err(|_| PersistenceError::Unavailable)?;
        fs::rename(temporary_path, &self.path).map_err(|_| PersistenceError::Unavailable)
    }
}

impl LayoutStateStore for InMemoryLayoutStateStore {
    fn load(&self) -> Result<Option<LayoutState>, PersistenceError> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| PersistenceError::Unavailable)
    }

    fn save(&self, state: &LayoutState) -> Result<(), PersistenceError> {
        self.state
            .lock()
            .map(|mut current| *current = Some(state.clone()))
            .map_err(|_| PersistenceError::Unavailable)
    }
}

pub trait ProfileStateStore: Send + Sync {
    fn load(&self) -> Result<Option<ProfileState>, PersistenceError>;
    fn save(&self, state: &ProfileState) -> Result<(), PersistenceError>;
}

#[derive(Default)]
pub struct InMemoryProfileStateStore {
    state: std::sync::Mutex<Option<ProfileState>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileProfileStateStore {
    path: PathBuf,
}

impl JsonFileProfileStateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl ProfileStateStore for InMemoryProfileStateStore {
    fn load(&self) -> Result<Option<ProfileState>, PersistenceError> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| PersistenceError::Unavailable)
    }

    fn save(&self, state: &ProfileState) -> Result<(), PersistenceError> {
        self.state
            .lock()
            .map(|mut current| *current = Some(state.clone()))
            .map_err(|_| PersistenceError::Unavailable)
    }
}

impl ProfileStateStore for JsonFileProfileStateStore {
    fn load(&self) -> Result<Option<ProfileState>, PersistenceError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(PersistenceError::Unavailable),
        };

        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| PersistenceError::CorruptState)
    }

    fn save(&self, state: &ProfileState) -> Result<(), PersistenceError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| PersistenceError::Unavailable)?;
        }

        let bytes = serde_json::to_vec_pretty(state).map_err(|_| PersistenceError::CorruptState)?;
        let temporary_path = self.path.with_extension("json.tmp");
        fs::write(&temporary_path, bytes).map_err(|_| PersistenceError::Unavailable)?;
        fs::rename(temporary_path, &self.path).map_err(|_| PersistenceError::Unavailable)
    }
}

/// JSON-backed JobSystem storage. JobSystem remains the state owner; this
/// adapter only supplies atomic file reads/writes for the native composition.
pub struct JsonFileJobStateStore {
    path: PathBuf,
    write_lock: std::sync::Mutex<()>,
}

impl JsonFileJobStateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write_lock: std::sync::Mutex::new(()),
        }
    }
}

impl JobStateStore for JsonFileJobStateStore {
    fn load(&self) -> Result<Vec<JobSnapshot>, String> {
        match fs::read_to_string(&self.path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(|error| error.to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.to_string()),
        }
    }

    fn save(&self, jobs: &[JobSnapshot]) -> Result<(), String> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| "job store lock poisoned".to_string())?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let contents = serde_json::to_string_pretty(jobs).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, contents).map_err(|error| error.to_string())?;
        fs::rename(temporary, &self.path).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use velvet_layout_model::WorkbenchPart;
    use velvet_workbench_protocol::{RecoveryLaunchReceipt, StartupPhase};

    static TEMPORARY_PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let sequence = TEMPORARY_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "velvet-{name}-{}-{nonce}-{sequence}.json",
            std::process::id()
        ))
    }

    #[test]
    fn file_store_round_trips_layout_state() {
        let path = temporary_path("layout-round-trip");
        let store = JsonFileLayoutStateStore::new(&path);
        let mut state = LayoutState::default();
        state
            .parts
            .get_mut(&WorkbenchPart::PrimarySidebar)
            .expect("primary sidebar should exist")
            .visible = false;

        store.save(&state).expect("layout should be written");
        let restored = store
            .load()
            .expect("layout should be readable")
            .expect("written layout should exist");

        assert_eq!(restored, state);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_file_is_an_empty_store() {
        let path = temporary_path("layout-missing");
        let store = JsonFileLayoutStateStore::new(path);

        assert_eq!(store.load().expect("missing files are valid"), None);
    }

    #[test]
    fn file_store_round_trips_profile_locale() {
        let path = temporary_path("profile-round-trip");
        let store = JsonFileProfileStateStore::new(&path);
        let state = ProfileState {
            schema_version: velvet_workbench_protocol::PROFILE_SCHEMA_VERSION,
            revision: 3,
            locale: "zh-Hans".into(),
        };

        store.save(&state).expect("profile should be written");
        assert_eq!(
            store
                .load()
                .expect("profile should be readable")
                .expect("written profile should exist"),
            state
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_store_round_trips_asset_source_locations() {
        let path = temporary_path("asset-source-round-trip");
        let store = JsonFileAssetSourceLocationStore::new(&path);
        let locations = vec![AssetSourceLocationRecord {
            asset_id: "asset-1".into(),
            source_path: "/tmp/asset-1.txt".into(),
            content_kind: "text/plain".into(),
            fingerprint: Some("sha256:asset-1".into()),
        }];

        store
            .save(&locations)
            .expect("asset source locations should be written");
        assert_eq!(
            store
                .load()
                .expect("asset source locations should be readable"),
            locations
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn asset_source_store_rejects_unknown_schema() {
        let path = temporary_path("asset-source-schema");
        fs::write(&path, br#"{"schemaVersion":99,"locations":[]}"#)
            .expect("the invalid asset source state should be writable");
        let store = JsonFileAssetSourceLocationStore::new(&path);

        assert_eq!(store.load(), Err(PersistenceError::CorruptState));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_store_round_trips_job_snapshots() {
        use velvet_job_system::{JobId, JobStatus};

        let path = temporary_path("jobs-round-trip");
        let store = JsonFileJobStateStore::new(&path);
        let jobs = vec![JobSnapshot {
            id: JobId("job-000001".into()),
            kind: "sample".into(),
            status: JobStatus::Interrupted,
            progress: 25,
            message: Some("restart".into()),
            attempt: 2,
            retryable: true,
            duration_ms: 1_000,
            will_fail: false,
            input: serde_json::Value::Null,
            provenance: velvet_job_system::JobProvenance::for_owner("test"),
            result: Some(velvet_job_system::JobResult {
                schema_version: 1,
                content_type: "application/json".into(),
                payload: serde_json::json!({ "ok": true }),
            }),
        }];

        store.save(&jobs).expect("jobs should be written");
        assert_eq!(store.load().expect("jobs should be readable"), jobs);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn recovery_journal_round_trips_and_rejects_unknown_schema() {
        let path = temporary_path("recovery-journal-round-trip");
        let store = JsonFileRecoveryJournalStore::new(&path);
        let state = RecoveryJournalState {
            next_attempt_id: 4,
            consecutive_failures: 1,
            active_launch: Some(RecoveryLaunchReceipt {
                attempt_id: 3,
                app_version: "0.0.4".into(),
                phase: StartupPhase::OptionalActivation,
                started_at_ms: 10,
                phase_started_at_ms: Some(20),
                deadline_at_ms: Some(30),
            }),
            ..RecoveryJournalState::default()
        };
        store
            .save(&state)
            .expect("recovery journal should be written");
        assert_eq!(
            store.load().expect("recovery journal should be readable"),
            state
        );

        fs::write(
            &path,
            br#"{"schemaVersion":1,"nextAttemptId":2,"consecutiveFailures":0,"safeModeForced":false,"activeLaunch":{"attemptId":1,"appVersion":"0.0.3","phase":"preflight","startedAtMs":1},"lastFailure":null,"lastGood":null,"shutdownIntentAtMs":null,"shutdownCompleteAtMs":null}"#,
        )
        .expect("legacy recovery journal should be writable");
        let legacy = store
            .load()
            .expect("legacy recovery journal should remain readable");
        assert_eq!(
            legacy
                .active_launch
                .and_then(|launch| launch.deadline_at_ms),
            None
        );

        fs::write(&path, br#"{"schemaVersion":99,"nextAttemptId":1,"consecutiveFailures":0,"safeModeForced":false,"activeLaunch":null,"lastFailure":null,"lastGood":null,"shutdownIntentAtMs":null,"shutdownCompleteAtMs":null}"#)
            .expect("invalid recovery journal should be writable");
        assert_eq!(store.load(), Err(PersistenceError::CorruptState));
        let _ = fs::remove_file(path);
    }
}
