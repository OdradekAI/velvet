use std::fs;
use std::path::{Path, PathBuf};

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
}
