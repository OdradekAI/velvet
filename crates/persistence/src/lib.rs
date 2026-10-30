use std::fs;
use std::path::{Path, PathBuf};

use velvet_job_system::{JobSnapshot, JobStateStore};
use velvet_layout_model::LayoutState;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistenceError {
    Unavailable,
    CorruptState,
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
    use std::time::{SystemTime, UNIX_EPOCH};
    use velvet_layout_model::WorkbenchPart;

    fn temporary_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("velvet-{name}-{}-{nonce}.json", std::process::id()))
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
}
