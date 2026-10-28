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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContributionKind {
    View,
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
    pub zone: Option<WorkbenchZone>,
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
    pub contributions: Vec<ContributionDescriptor>,
    pub permission_sets: Vec<ZonePermissionSet>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionSummary {
    pub id: ExtensionId,
    pub version: String,
    pub active: bool,
}
