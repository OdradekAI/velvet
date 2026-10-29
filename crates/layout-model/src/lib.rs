use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchAppId {
    AssetManager,
    FeedWorkbench,
}

/// Compatibility alias while older protocol clients still use workspace terminology.
pub type WorkbenchWorkspaceId = WorkbenchAppId;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum WorkbenchZone {
    Navigation,
    Contextual,
    Operations,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchPart {
    TitleBar,
    ActivityBar,
    PrimarySidebar,
    WorkspaceArea,
    AuxiliarySidebar,
    OperationsPanel,
    StatusBar,
    OverlayLayer,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartState {
    pub visible: bool,
    pub size: u32,
    pub collapsed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewState {
    pub visible: bool,
    pub order: u32,
    pub container_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewContainerState {
    pub visible: bool,
    pub order: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutState {
    pub schema_version: u32,
    pub revision: u64,
    pub active_app: WorkbenchAppId,
    pub parts: BTreeMap<WorkbenchPart, PartState>,
    pub containers: BTreeMap<String, ViewContainerState>,
    pub views: BTreeMap<String, ViewState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PartConstraints {
    pub hideable: bool,
    pub collapsible: bool,
    pub resizable: bool,
    pub min_size: u32,
    pub max_size: u32,
    pub default_size: u32,
}

pub fn part_constraints(part: &WorkbenchPart) -> PartConstraints {
    match part {
        WorkbenchPart::TitleBar => PartConstraints {
            hideable: true,
            collapsible: false,
            resizable: false,
            min_size: 36,
            max_size: 36,
            default_size: 36,
        },
        WorkbenchPart::ActivityBar => PartConstraints {
            hideable: true,
            collapsible: false,
            resizable: false,
            min_size: 48,
            max_size: 48,
            default_size: 48,
        },
        WorkbenchPart::PrimarySidebar => PartConstraints {
            hideable: true,
            collapsible: true,
            resizable: true,
            min_size: 180,
            max_size: 520,
            default_size: 260,
        },
        WorkbenchPart::WorkspaceArea => PartConstraints {
            hideable: false,
            collapsible: false,
            resizable: false,
            min_size: 0,
            max_size: u32::MAX,
            default_size: 0,
        },
        WorkbenchPart::AuxiliarySidebar => PartConstraints {
            hideable: true,
            collapsible: true,
            resizable: true,
            min_size: 220,
            max_size: 520,
            default_size: 300,
        },
        WorkbenchPart::OperationsPanel => PartConstraints {
            hideable: true,
            collapsible: true,
            resizable: true,
            min_size: 120,
            max_size: 520,
            default_size: 180,
        },
        WorkbenchPart::StatusBar => PartConstraints {
            hideable: true,
            collapsible: false,
            resizable: false,
            min_size: 24,
            max_size: 24,
            default_size: 24,
        },
        WorkbenchPart::OverlayLayer => PartConstraints {
            hideable: false,
            collapsible: false,
            resizable: false,
            min_size: 0,
            max_size: 0,
            default_size: 0,
        },
    }
}

impl Default for LayoutState {
    fn default() -> Self {
        let mut parts = BTreeMap::new();
        for part in [
            WorkbenchPart::TitleBar,
            WorkbenchPart::ActivityBar,
            WorkbenchPart::PrimarySidebar,
            WorkbenchPart::WorkspaceArea,
            WorkbenchPart::AuxiliarySidebar,
            WorkbenchPart::OperationsPanel,
            WorkbenchPart::StatusBar,
            WorkbenchPart::OverlayLayer,
        ] {
            let default_size = part_constraints(&part).default_size;
            parts.insert(
                part,
                PartState {
                    visible: true,
                    size: default_size,
                    collapsed: false,
                },
            );
        }

        Self {
            schema_version: 1,
            revision: 0,
            active_app: WorkbenchAppId::AssetManager,
            parts,
            containers: BTreeMap::new(),
            views: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutCommand {
    ActivateApp(WorkbenchAppId),
    #[allow(deprecated)]
    ActivateWorkspace(WorkbenchWorkspaceId),
    SetPartVisibility {
        part: WorkbenchPart,
        visible: bool,
    },
    TogglePartVisibility {
        part: WorkbenchPart,
    },
    ResizePart {
        part: WorkbenchPart,
        size: u32,
    },
    SetPartCollapsed {
        part: WorkbenchPart,
        collapsed: bool,
    },
    UpsertViewContainer {
        container_id: String,
        visible: bool,
        order: u32,
    },
    SetViewContainerVisibility {
        container_id: String,
        visible: bool,
    },
    ReorderViewContainer {
        container_id: String,
        before_container_id: Option<String>,
        after_container_id: Option<String>,
    },
    RemoveViewContainer {
        container_id: String,
    },
    SetViewVisibility {
        view_id: String,
        visible: bool,
    },
    ReorderView {
        view_id: String,
        before_view_id: Option<String>,
        after_view_id: Option<String>,
    },
    MoveView {
        view_id: String,
        target_container_id: String,
        before_view_id: Option<String>,
        after_view_id: Option<String>,
    },
    RemoveView {
        view_id: String,
    },
    ResetLayout,
    ResetViews,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutError {
    UnknownPart(WorkbenchPart),
    UnknownView(String),
    UnknownContainer(String),
    UnknownAnchor(String),
    InvalidOrderAnchor,
    PartCannotHide(WorkbenchPart),
    PartCannotCollapse(WorkbenchPart),
    PartCannotResize(WorkbenchPart),
    InvalidPartSize {
        part: WorkbenchPart,
        size: u32,
        min: u32,
        max: u32,
    },
}

pub trait LayoutService {
    fn snapshot(&self) -> LayoutState;
    fn apply(&mut self, command: LayoutCommand) -> Result<LayoutState, LayoutError>;
    fn migrate(&self, state: LayoutState) -> Result<LayoutState, LayoutError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryLayoutService {
    state: LayoutState,
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
            LayoutCommand::ActivateApp(app) | LayoutCommand::ActivateWorkspace(app) => {
                self.state.active_app = app;
            }
            LayoutCommand::SetPartVisibility { part, visible } => {
                let constraints = part_constraints(&part);
                if !visible && !constraints.hideable {
                    return Err(LayoutError::PartCannotHide(part));
                }
                self.state
                    .parts
                    .get_mut(&part)
                    .ok_or_else(|| LayoutError::UnknownPart(part.clone()))?
                    .visible = visible;
            }
            LayoutCommand::TogglePartVisibility { part } => {
                let constraints = part_constraints(&part);
                let state = self
                    .state
                    .parts
                    .get_mut(&part)
                    .ok_or_else(|| LayoutError::UnknownPart(part.clone()))?;
                if state.visible && !constraints.hideable {
                    return Err(LayoutError::PartCannotHide(part));
                }
                state.visible = !state.visible;
            }
            LayoutCommand::ResizePart { part, size } => {
                let constraints = part_constraints(&part);
                if !constraints.resizable {
                    return Err(LayoutError::PartCannotResize(part));
                }
                if size < constraints.min_size || size > constraints.max_size {
                    return Err(LayoutError::InvalidPartSize {
                        part,
                        size,
                        min: constraints.min_size,
                        max: constraints.max_size,
                    });
                }
                self.state
                    .parts
                    .get_mut(&part)
                    .ok_or_else(|| LayoutError::UnknownPart(part.clone()))?
                    .size = size;
            }
            LayoutCommand::SetPartCollapsed { part, collapsed } => {
                let constraints = part_constraints(&part);
                if collapsed && !constraints.collapsible {
                    return Err(LayoutError::PartCannotCollapse(part));
                }
                self.state
                    .parts
                    .get_mut(&part)
                    .ok_or_else(|| LayoutError::UnknownPart(part.clone()))?
                    .collapsed = collapsed;
            }
            LayoutCommand::UpsertViewContainer {
                container_id,
                visible,
                order,
            } => {
                self.state
                    .containers
                    .insert(container_id, ViewContainerState { visible, order });
            }
            LayoutCommand::SetViewContainerVisibility {
                container_id,
                visible,
            } => {
                self.state
                    .containers
                    .get_mut(&container_id)
                    .ok_or_else(|| LayoutError::UnknownContainer(container_id.clone()))?
                    .visible = visible;
            }
            LayoutCommand::ReorderViewContainer {
                container_id,
                before_container_id,
                after_container_id,
            } => self.reorder_container(
                &container_id,
                before_container_id.as_deref(),
                after_container_id.as_deref(),
            )?,
            LayoutCommand::RemoveViewContainer { container_id } => {
                self.state
                    .containers
                    .remove(&container_id)
                    .ok_or_else(|| LayoutError::UnknownContainer(container_id.clone()))?;
                for view in self.state.views.values_mut() {
                    if view.container_id.as_deref() == Some(container_id.as_str()) {
                        view.container_id = None;
                    }
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
            LayoutCommand::ReorderView {
                view_id,
                before_view_id,
                after_view_id,
            } => self.reorder_view(
                &view_id,
                before_view_id.as_deref(),
                after_view_id.as_deref(),
            )?,
            LayoutCommand::MoveView {
                view_id,
                target_container_id,
                before_view_id,
                after_view_id,
            } => {
                if !self.state.containers.contains_key(&target_container_id) {
                    return Err(LayoutError::UnknownContainer(target_container_id));
                }
                let view = self
                    .state
                    .views
                    .get_mut(&view_id)
                    .ok_or_else(|| LayoutError::UnknownView(view_id.clone()))?;
                view.container_id = Some(target_container_id);
                self.reorder_view(
                    &view_id,
                    before_view_id.as_deref(),
                    after_view_id.as_deref(),
                )?;
            }
            LayoutCommand::RemoveView { view_id } => {
                self.state.views.remove(&view_id);
            }
            LayoutCommand::ResetLayout => {
                let revision = self.state.revision;
                self.state = LayoutState::default();
                self.state.revision = revision;
            }
            LayoutCommand::ResetViews => {
                self.state.views.clear();
            }
        }

        self.state.revision = self.state.revision.saturating_add(1);

        Ok(self.snapshot())
    }

    fn migrate(&self, mut state: LayoutState) -> Result<LayoutState, LayoutError> {
        state.schema_version = 1;
        Ok(state)
    }
}

impl InMemoryLayoutService {
    fn reorder_container(
        &mut self,
        container_id: &str,
        before_container_id: Option<&str>,
        after_container_id: Option<&str>,
    ) -> Result<(), LayoutError> {
        if before_container_id.is_some() && after_container_id.is_some() {
            return Err(LayoutError::InvalidOrderAnchor);
        }

        if !self.state.containers.contains_key(container_id) {
            return Err(LayoutError::UnknownContainer(container_id.into()));
        }

        let mut ids: Vec<String> = self.state.containers.keys().cloned().collect();
        ids.sort_by_key(|id| {
            self.state
                .containers
                .get(id)
                .map(|state| state.order)
                .unwrap_or(0)
        });
        ids.retain(|id| id != container_id);

        let insertion_index = if let Some(anchor) = before_container_id {
            ids.iter()
                .position(|id| id == anchor)
                .ok_or_else(|| LayoutError::UnknownAnchor(anchor.into()))?
        } else if let Some(anchor) = after_container_id {
            ids.iter()
                .position(|id| id == anchor)
                .map(|index| index + 1)
                .ok_or_else(|| LayoutError::UnknownAnchor(anchor.into()))?
        } else {
            ids.len()
        };

        ids.insert(insertion_index, container_id.into());
        for (order, id) in ids.into_iter().enumerate() {
            if let Some(state) = self.state.containers.get_mut(&id) {
                state.order = order as u32;
            }
        }

        Ok(())
    }

    fn reorder_view(
        &mut self,
        view_id: &str,
        before_view_id: Option<&str>,
        after_view_id: Option<&str>,
    ) -> Result<(), LayoutError> {
        if before_view_id.is_some() && after_view_id.is_some() {
            return Err(LayoutError::InvalidOrderAnchor);
        }

        if !self.state.views.contains_key(view_id) {
            return Err(LayoutError::UnknownView(view_id.into()));
        }

        let container_id = self
            .state
            .views
            .get(view_id)
            .and_then(|view| view.container_id.clone());

        let mut ids: Vec<String> = self
            .state
            .views
            .iter()
            .filter(|(_, view)| view.container_id == container_id)
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort_by_key(|id| {
            self.state
                .views
                .get(id)
                .map(|state| state.order)
                .unwrap_or(0)
        });
        ids.retain(|id| id != view_id);

        let insertion_index = if let Some(anchor) = before_view_id {
            ids.iter()
                .position(|id| id == anchor)
                .ok_or_else(|| LayoutError::UnknownAnchor(anchor.into()))?
        } else if let Some(anchor) = after_view_id {
            ids.iter()
                .position(|id| id == anchor)
                .map(|index| index + 1)
                .ok_or_else(|| LayoutError::UnknownAnchor(anchor.into()))?
        } else {
            ids.len()
        };

        ids.insert(insertion_index, view_id.into());
        for (order, id) in ids.into_iter().enumerate() {
            if let Some(state) = self.state.views.get_mut(&id) {
                state.order = order as u32;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_starts_in_asset_manager() {
        let service = InMemoryLayoutService::default();
        assert_eq!(service.snapshot().active_app, WorkbenchAppId::AssetManager);
    }

    #[test]
    fn layout_commands_update_the_single_state_owner() {
        let mut service = InMemoryLayoutService::default();
        service
            .apply(LayoutCommand::ActivateApp(WorkbenchAppId::FeedWorkbench))
            .expect("workspace activation should succeed");

        assert_eq!(service.snapshot().active_app, WorkbenchAppId::FeedWorkbench);
    }

    #[test]
    fn toggle_part_visibility_respects_hideability_constraints() {
        let mut service = InMemoryLayoutService::default();
        service
            .apply(LayoutCommand::TogglePartVisibility {
                part: WorkbenchPart::PrimarySidebar,
            })
            .expect("hideable parts should toggle");
        assert!(
            !service
                .snapshot()
                .parts
                .get(&WorkbenchPart::PrimarySidebar)
                .expect("primary sidebar should exist")
                .visible
        );

        let error = service
            .apply(LayoutCommand::TogglePartVisibility {
                part: WorkbenchPart::WorkspaceArea,
            })
            .expect_err("the required workspace area cannot be hidden");
        assert_eq!(
            error,
            LayoutError::PartCannotHide(WorkbenchPart::WorkspaceArea)
        );
    }

    #[test]
    fn layout_revision_increments_and_reset_preserves_monotonicity() {
        let mut service = InMemoryLayoutService::default();
        service
            .apply(LayoutCommand::SetPartCollapsed {
                part: WorkbenchPart::PrimarySidebar,
                collapsed: true,
            })
            .unwrap();
        let revision = service.snapshot().revision;
        service.apply(LayoutCommand::ResetLayout).unwrap();

        assert!(service.snapshot().revision > revision);
        assert!(
            !service
                .snapshot()
                .parts
                .get(&WorkbenchPart::PrimarySidebar)
                .unwrap()
                .collapsed
        );
    }

    #[test]
    fn view_order_uses_before_anchor_and_removal_is_recoverable() {
        let mut state = LayoutState::default();
        state.views.insert(
            "a".into(),
            ViewState {
                visible: true,
                order: 0,
                container_id: Some("library".into()),
            },
        );
        state.views.insert(
            "b".into(),
            ViewState {
                visible: true,
                order: 1,
                container_id: Some("library".into()),
            },
        );
        let mut service = InMemoryLayoutService::from_state(state);
        service
            .apply(LayoutCommand::ReorderView {
                view_id: "b".into(),
                before_view_id: Some("a".into()),
                after_view_id: None,
            })
            .unwrap();
        assert_eq!(service.snapshot().views.get("b").unwrap().order, 0);

        service
            .apply(LayoutCommand::RemoveView {
                view_id: "b".into(),
            })
            .unwrap();
        assert!(!service.snapshot().views.contains_key("b"));
    }

    #[test]
    fn view_container_order_and_view_moves_stay_in_host_state() {
        let mut service = InMemoryLayoutService::default();
        service
            .apply(LayoutCommand::UpsertViewContainer {
                container_id: "library".into(),
                visible: true,
                order: 0,
            })
            .unwrap();
        service
            .apply(LayoutCommand::UpsertViewContainer {
                container_id: "inspector".into(),
                visible: true,
                order: 1,
            })
            .unwrap();
        service
            .apply(LayoutCommand::ReorderViewContainer {
                container_id: "inspector".into(),
                before_container_id: Some("library".into()),
                after_container_id: None,
            })
            .unwrap();

        let mut state = service.snapshot();
        state.views.insert(
            "asset-details".into(),
            ViewState {
                visible: true,
                order: 0,
                container_id: Some("library".into()),
            },
        );
        let mut service = InMemoryLayoutService::from_state(state);
        service
            .apply(LayoutCommand::MoveView {
                view_id: "asset-details".into(),
                target_container_id: "inspector".into(),
                before_view_id: None,
                after_view_id: None,
            })
            .unwrap();

        assert_eq!(
            service
                .snapshot()
                .containers
                .get("inspector")
                .unwrap()
                .order,
            0
        );
        assert_eq!(
            service
                .snapshot()
                .views
                .get("asset-details")
                .unwrap()
                .container_id
                .as_deref(),
            Some("inspector")
        );

        service
            .apply(LayoutCommand::RemoveViewContainer {
                container_id: "inspector".into(),
            })
            .unwrap();
        assert_eq!(
            service
                .snapshot()
                .views
                .get("asset-details")
                .unwrap()
                .container_id,
            None
        );
    }

    #[test]
    fn view_container_visibility_changes_preserve_user_order() {
        let mut service = InMemoryLayoutService::default();
        service
            .apply(LayoutCommand::UpsertViewContainer {
                container_id: "plugin.library".into(),
                visible: true,
                order: 3,
            })
            .unwrap();
        service
            .apply(LayoutCommand::SetViewContainerVisibility {
                container_id: "plugin.library".into(),
                visible: false,
            })
            .unwrap();

        let container = service
            .snapshot()
            .containers
            .get("plugin.library")
            .unwrap();
        assert!(!container.visible);
        assert_eq!(container.order, 3);

        service
            .apply(LayoutCommand::SetViewContainerVisibility {
                container_id: "plugin.library".into(),
                visible: true,
            })
            .unwrap();
        let container = service
            .snapshot()
            .containers
            .get("plugin.library")
            .unwrap();
        assert!(container.visible);
        assert_eq!(container.order, 3);
    }

    #[test]
    fn fixed_parts_reject_unsupported_geometry_commands() {
        let mut service = InMemoryLayoutService::default();

        assert_eq!(
            service.apply(LayoutCommand::SetPartVisibility {
                part: WorkbenchPart::WorkspaceArea,
                visible: false,
            }),
            Err(LayoutError::PartCannotHide(WorkbenchPart::WorkspaceArea))
        );
        assert_eq!(
            service.apply(LayoutCommand::SetPartCollapsed {
                part: WorkbenchPart::ActivityBar,
                collapsed: true,
            }),
            Err(LayoutError::PartCannotCollapse(WorkbenchPart::ActivityBar))
        );
        assert_eq!(
            service.apply(LayoutCommand::ResizePart {
                part: WorkbenchPart::StatusBar,
                size: 30,
            }),
            Err(LayoutError::PartCannotResize(WorkbenchPart::StatusBar))
        );
    }

    #[test]
    fn resizable_parts_enforce_vscode_like_bounds() {
        let mut service = InMemoryLayoutService::default();

        service
            .apply(LayoutCommand::ResizePart {
                part: WorkbenchPart::PrimarySidebar,
                size: 420,
            })
            .unwrap();
        assert_eq!(
            service
                .snapshot()
                .parts
                .get(&WorkbenchPart::PrimarySidebar)
                .unwrap()
                .size,
            420
        );
        assert_eq!(
            service.apply(LayoutCommand::ResizePart {
                part: WorkbenchPart::PrimarySidebar,
                size: 100,
            }),
            Err(LayoutError::InvalidPartSize {
                part: WorkbenchPart::PrimarySidebar,
                size: 100,
                min: 180,
                max: 520,
            })
        );
    }
}
