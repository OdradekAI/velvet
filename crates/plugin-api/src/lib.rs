use serde::{Deserialize, Serialize};
use velvet_layout_model::WorkbenchZone;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ExtensionId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionRange(pub String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeKind {
    BuiltIn,
    Wasm,
    Sidecar,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ContributionScope {
    Workbench,
    WorkbenchApp {
        #[serde(rename = "appId")]
        app_id: String,
    },
    Capability,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostContainer {
    ActivityBar,
    TitleBar,
    WorkspaceArea,
    PrimarySidebar,
    AuxiliarySidebar,
    OperationsPanel,
    StatusBar,
    OverlayLayer,
    ContextMenu,
    CommandPalette,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    WorkspaceOpen,
    LibraryRead,
    SourceConnect,
    AssetRead,
    MetadataRead,
    MetadataWrite,
    PreviewRender,
    JobsCreate,
    JobsRead,
    LogsRead,
    ExternalProcess,
    Network,
}

/// Describes the executable boundary used to host an external extension.
///
/// A WASM extension points at its declared runner (for example a WASI host)
/// and passes the module path through `args`; a sidecar points directly at its
/// isolated process. The host never derives this command from a UI entrypoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLaunchSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContributionKind {
    ActivityBarEntry,
    CompanionView,
    OperationView,
    ViewContainer,
    View,
    Dialog,
    ContextMenu,
    PreviewProvider,
    PreviewAugmentation,
    Inspector,
    Command,
    Job,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiSurfaceKind {
    AppSurface,
    View,
    Dialog,
    Popover,
    Inspector,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSurfaceDescriptor {
    pub kind: UiSurfaceKind,
    pub entrypoint: String,
    pub bridge_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBarDescriptor {
    pub title: String,
    pub icon: String,
    pub order: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionDescriptor {
    pub id: String,
    pub kind: ContributionKind,
    pub scope: ContributionScope,
    pub namespace: String,
    #[serde(default)]
    pub container: Option<HostContainer>,
    #[serde(default)]
    pub zone: Option<WorkbenchZone>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub title_key: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub order: Option<u32>,
    #[serde(default)]
    pub default_visible: Option<bool>,
    #[serde(default)]
    pub view_container_id: Option<String>,
    #[serde(default)]
    pub activity_bar: Option<ActivityBarDescriptor>,
    #[serde(default)]
    pub ui: Option<UiSurfaceDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppTarget {
    pub app_id: String,
    pub api: VersionRange,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZonePermissionSet {
    pub zone: WorkbenchZone,
    pub permissions: Vec<Permission>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    pub id: ExtensionId,
    pub version: String,
    pub host_api: VersionRange,
    pub runtime: RuntimeKind,
    pub launch: Option<RuntimeLaunchSpec>,
    pub target_app: Option<AppTarget>,
    pub contributions: Vec<ContributionDescriptor>,
    pub permission_sets: Vec<ZonePermissionSet>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionState {
    Registered,
    Starting,
    Active,
    Disabled,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionSummary {
    pub id: ExtensionId,
    pub version: String,
    pub active: bool,
    pub state: ExtensionState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionProjection {
    pub summary: ExtensionSummary,
    pub target_app: Option<AppTarget>,
    pub contributions: Vec<ContributionDescriptor>,
}
