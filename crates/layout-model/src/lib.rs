use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkbenchWorkspaceId {
    AssetManager,
    FeedWorkbench,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkbenchZone {
    Navigation,
    Contextual,
    Operations,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkbenchPart {
    TitleBar,
    ActivityBar,
    PrimarySidebar,
    WorkspaceArea,
    ContextSidebar,
    OperationsPanel,
    StatusBar,
    OverlayLayer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartState {
    pub visible: bool,
    pub size: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewState {
    pub visible: bool,
    pub order: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutState {
    pub schema_version: u32,
    pub active_workspace: WorkbenchWorkspaceId,
    pub parts: BTreeMap<WorkbenchPart, PartState>,
    pub views: BTreeMap<String, ViewState>,
}

impl Default for LayoutState {
    fn default() -> Self {
        let mut parts = BTreeMap::new();
        for part in [
            WorkbenchPart::TitleBar,
            WorkbenchPart::ActivityBar,
            WorkbenchPart::PrimarySidebar,
            WorkbenchPart::WorkspaceArea,
            WorkbenchPart::ContextSidebar,
            WorkbenchPart::OperationsPanel,
            WorkbenchPart::StatusBar,
            WorkbenchPart::OverlayLayer,
        ] {
            parts.insert(
                part,
                PartState {
                    visible: true,
                    size: 0,
                },
            );
        }

        Self {
            schema_version: 1,
            active_workspace: WorkbenchWorkspaceId::AssetManager,
            parts,
            views: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutCommand {
    ActivateWorkspace(WorkbenchWorkspaceId),
    SetPartVisibility { part: WorkbenchPart, visible: bool },
    ResizePart { part: WorkbenchPart, size: u32 },
    SetViewVisibility { view_id: String, visible: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutError {
    UnknownView(String),
}

pub trait LayoutService {
    fn snapshot(&self) -> LayoutState;
    fn apply(&mut self, command: LayoutCommand) -> Result<LayoutState, LayoutError>;
    fn migrate(&self, state: LayoutState) -> Result<LayoutState, LayoutError>;
}

#[derive(Clone, Debug)]
pub struct InMemoryLayoutService {
    state: LayoutState,
}

impl Default for InMemoryLayoutService {
    fn default() -> Self {
        Self {
            state: LayoutState::default(),
        }
    }
}

impl InMemoryLayoutService {
    pub fn from_state(state: LayoutState) -> Self {
        Self { state }
    }
}

impl LayoutService for InMemoryLayoutService {
    fn snapshot(&self) -> LayoutState {
        self.state.clone()
    }

    fn apply(&mut self, command: LayoutCommand) -> Result<LayoutState, LayoutError> {
        match command {
            LayoutCommand::ActivateWorkspace(workspace) => {
                self.state.active_workspace = workspace;
            }
            LayoutCommand::SetPartVisibility { part, visible } => {
                if let Some(state) = self.state.parts.get_mut(&part) {
                    state.visible = visible;
                }
            }
            LayoutCommand::ResizePart { part, size } => {
                if let Some(state) = self.state.parts.get_mut(&part) {
                    state.size = size;
                }
            }
            LayoutCommand::SetViewVisibility { view_id, visible } => {
                let state = self
                    .state
                    .views
                    .get_mut(&view_id)
                    .ok_or_else(|| LayoutError::UnknownView(view_id.clone()))?;
                state.visible = visible;
            }
        }

        Ok(self.snapshot())
    }

    fn migrate(&self, mut state: LayoutState) -> Result<LayoutState, LayoutError> {
        state.schema_version = 1;
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_starts_in_asset_manager() {
        let service = InMemoryLayoutService::default();
        assert_eq!(
            service.snapshot().active_workspace,
            WorkbenchWorkspaceId::AssetManager
        );
    }

    #[test]
    fn layout_commands_update_the_single_state_owner() {
        let mut service = InMemoryLayoutService::default();
        service
            .apply(LayoutCommand::ActivateWorkspace(
                WorkbenchWorkspaceId::FeedWorkbench,
            ))
            .expect("workspace activation should succeed");

        assert_eq!(
            service.snapshot().active_workspace,
            WorkbenchWorkspaceId::FeedWorkbench
        );
    }
}
