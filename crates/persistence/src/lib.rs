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
