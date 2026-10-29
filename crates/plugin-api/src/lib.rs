use velvet_layout_model::WorkbenchZone;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExtensionId(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionRange(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeKind {
    BuiltIn,
    Wasm,
    Sidecar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContributionScope {
    Workbench,
    WorkbenchApp { app_id: String },
    Capability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeLaunchSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContributionDescriptor {
    pub id: String,
    pub kind: ContributionKind,
    pub scope: ContributionScope,
    pub namespace: String,
    pub container: Option<HostContainer>,
    pub zone: Option<WorkbenchZone>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppTarget {
    pub app_id: String,
    pub api: VersionRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZonePermissionSet {
    pub zone: WorkbenchZone,
    pub permissions: Vec<Permission>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExtensionState {
    Registered,
    Starting,
    Active,
    Disabled,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionSummary {
    pub id: ExtensionId,
    pub version: String,
    pub active: bool,
    pub state: ExtensionState,
}
