use std::collections::BTreeMap;

use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use velvet_layout_model::WorkbenchZone;

pub const MANIFEST_SCHEMA_VERSION: u16 = 1;
pub const PREVIEW_ACTION_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ExtensionId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionRange(pub String);

impl VersionRange {
    /// Alpha compatibility semantics shared by manifest validation and host
    /// negotiation: exact versions, `*`, or a caret major range.
    pub fn is_compatible_with(&self, provided: &Self) -> bool {
        if self.0 == "*" || self.0 == provided.0 {
            return true;
        }

        self.0
            .strip_prefix('^')
            .and_then(|range| range.split('.').next())
            == provided.0.split('.').next()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeKind {
    BuiltIn,
    Wasm,
    Sidecar,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionKind {
    Ui,
    Workspace,
    Capability,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionSupport {
    pub trusted_workspace: bool,
    pub virtual_workspace: bool,
    pub profiles: bool,
}

impl Default for ExtensionSupport {
    fn default() -> Self {
        Self {
            trusted_workspace: true,
            virtual_workspace: false,
            profiles: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ActivationEvent {
    #[serde(rename = "onWorkbenchReady")]
    WorkbenchReady,
    #[serde(rename = "onApp")]
    App { app_id: String },
    #[serde(rename = "onView")]
    View { contribution_id: String },
    #[serde(rename = "onCommand")]
    Command { contribution_id: String },
    #[serde(rename = "onPreview")]
    Preview { selector: String },
    #[serde(rename = "onJob")]
    Job { job_kind: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalizationManifest {
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub resource_root: String,
    pub bundle_schema: u16,
}

/// Declares the host-owned durable store available to an extension.
///
/// The extension receives a capability handle through `PluginHost`; it never
/// receives a filesystem path or a raw Tauri command. `quota_bytes` is a
/// bounded request, not an exemption from the host's global snapshot limit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionStorageDeclaration {
    pub schema_version: u16,
    pub quota_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchAppDescriptor {
    pub id: String,
    pub api: VersionRange,
    pub title: String,
    pub activity_bar_icon: String,
    pub activity_bar_order: Option<u32>,
    pub surface: UiSurfaceDescriptor,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MenuLocation {
    TitleBar,
    ContextMenu,
    CommandPalette,
    AppToolbar,
    StatusBar,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeybindingPlatform {
    Any,
    Macos,
    Windows,
    Linux,
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

impl Permission {
    /// Stable capability spelling used by manifests, runtime grants and
    /// diagnostics. The legacy zone permission remains a compatibility view
    /// over the broader capability contract.
    pub const fn capability_name(&self) -> &'static str {
        match self {
            Self::WorkspaceOpen => "workspace.open",
            Self::LibraryRead => "library.read",
            Self::SourceConnect => "source.connect",
            Self::AssetRead => "asset.read",
            Self::MetadataRead => "metadata.read",
            Self::MetadataWrite => "metadata.write",
            Self::PreviewRender => "preview.render",
            Self::JobsCreate => "jobs.create",
            Self::JobsRead => "jobs.read",
            Self::LogsRead => "logs.read",
            Self::ExternalProcess => "external-process",
            Self::Network => "network",
        }
    }

    pub fn from_capability_name(name: &str) -> Option<Self> {
        Some(match name {
            "workspace.open" => Self::WorkspaceOpen,
            "library.read" => Self::LibraryRead,
            "source.connect" => Self::SourceConnect,
            "asset.read" => Self::AssetRead,
            "metadata.read" => Self::MetadataRead,
            "metadata.write" => Self::MetadataWrite,
            "preview.render" => Self::PreviewRender,
            "jobs.create" => Self::JobsCreate,
            "jobs.read" => Self::JobsRead,
            "logs.read" => Self::LogsRead,
            "external-process" => Self::ExternalProcess,
            "network" => Self::Network,
            _ => return None,
        })
    }
}

/// Versioned host capability identifiers. `Permission` remains the manifest
/// zone compatibility contract; this type covers capabilities that do not
/// belong to a UI zone and must never be inferred from one.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Capability {
    #[serde(rename = "workspace.open")]
    WorkspaceOpen,
    #[serde(rename = "library.read")]
    LibraryRead,
    #[serde(rename = "source.connect")]
    SourceConnect,
    #[serde(rename = "asset.read")]
    AssetRead,
    #[serde(rename = "metadata.read")]
    MetadataRead,
    #[serde(rename = "metadata.write")]
    MetadataWrite,
    #[serde(rename = "preview.render")]
    PreviewRender,
    #[serde(rename = "jobs.create")]
    JobsCreate,
    #[serde(rename = "jobs.read")]
    JobsRead,
    #[serde(rename = "logs.read")]
    LogsRead,
    #[serde(rename = "external-process")]
    ExternalProcess,
    #[serde(rename = "network")]
    Network,
    #[serde(rename = "asset.facet.write")]
    AssetFacetWrite,
    #[serde(rename = "config.read")]
    ConfigRead,
    #[serde(rename = "config.write")]
    ConfigWrite,
    #[serde(rename = "storage.read")]
    StorageRead,
    #[serde(rename = "storage.write")]
    StorageWrite,
    #[serde(rename = "view.register")]
    ViewRegister,
    #[serde(rename = "menu.register")]
    MenuRegister,
    #[serde(rename = "command.register")]
    CommandRegister,
    #[serde(rename = "notifications.show")]
    NotificationsShow,
    #[serde(rename = "logs.write")]
    LogsWrite,
    #[serde(rename = "events.subscribe")]
    EventsSubscribe,
    #[serde(rename = "filesystem")]
    Filesystem,
    #[serde(rename = "process")]
    Process,
    #[serde(rename = "core-metadata-write")]
    CoreMetadataWrite,
    #[serde(rename = "secret")]
    Secret,
}

impl Capability {
    pub const fn name(&self) -> &'static str {
        match self {
            Self::WorkspaceOpen => "workspace.open",
            Self::LibraryRead => "library.read",
            Self::SourceConnect => "source.connect",
            Self::AssetRead => "asset.read",
            Self::MetadataRead => "metadata.read",
            Self::MetadataWrite => "metadata.write",
            Self::PreviewRender => "preview.render",
            Self::JobsCreate => "jobs.create",
            Self::JobsRead => "jobs.read",
            Self::LogsRead => "logs.read",
            Self::ExternalProcess => "external-process",
            Self::Network => "network",
            Self::AssetFacetWrite => "asset.facet.write",
            Self::ConfigRead => "config.read",
            Self::ConfigWrite => "config.write",
            Self::StorageRead => "storage.read",
            Self::StorageWrite => "storage.write",
            Self::ViewRegister => "view.register",
            Self::MenuRegister => "menu.register",
            Self::CommandRegister => "command.register",
            Self::NotificationsShow => "notifications.show",
            Self::LogsWrite => "logs.write",
            Self::EventsSubscribe => "events.subscribe",
            Self::Filesystem => "filesystem",
            Self::Process => "process",
            Self::CoreMetadataWrite => "core-metadata-write",
            Self::Secret => "secret",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "workspace.open" => Self::WorkspaceOpen,
            "library.read" => Self::LibraryRead,
            "source.connect" => Self::SourceConnect,
            "asset.read" => Self::AssetRead,
            "metadata.read" => Self::MetadataRead,
            "metadata.write" => Self::MetadataWrite,
            "preview.render" => Self::PreviewRender,
            "jobs.create" => Self::JobsCreate,
            "jobs.read" => Self::JobsRead,
            "logs.read" => Self::LogsRead,
            "external-process" => Self::ExternalProcess,
            "network" => Self::Network,
            "asset.facet.write" => Self::AssetFacetWrite,
            "config.read" => Self::ConfigRead,
            "config.write" => Self::ConfigWrite,
            "storage.read" => Self::StorageRead,
            "storage.write" => Self::StorageWrite,
            "view.register" => Self::ViewRegister,
            "menu.register" => Self::MenuRegister,
            "command.register" => Self::CommandRegister,
            "notifications.show" => Self::NotificationsShow,
            "logs.write" => Self::LogsWrite,
            "events.subscribe" => Self::EventsSubscribe,
            "filesystem" => Self::Filesystem,
            "process" => Self::Process,
            "core-metadata-write" => Self::CoreMetadataWrite,
            "secret" => Self::Secret,
            _ => return None,
        })
    }

    pub const fn is_high_risk(&self) -> bool {
        matches!(
            self,
            Self::MetadataWrite
                | Self::ExternalProcess
                | Self::Network
                | Self::Filesystem
                | Self::Process
                | Self::CoreMetadataWrite
                | Self::Secret
        )
    }

    pub fn legacy_permission(&self) -> Option<Permission> {
        Permission::from_capability_name(self.name())
    }
}

/// Describes the executable boundary used to host an external extension.
///
/// A WASM extension points at its declared runner (for example a WASI host)
/// and passes the module path through `args`; a sidecar points directly at its
/// isolated process. The host never derives this command from a UI entrypoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeLaunchSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContributionKind {
    AppSurface,
    ActivityBarEntry,
    CompanionView,
    OperationView,
    ViewContainer,
    View,
    Dialog,
    ContextMenu,
    Menu,
    Keybinding,
    Popover,
    Notification,
    PreviewProvider,
    PreviewAugmentation,
    Inspector,
    Command,
    Job,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextAnchor {
    Asset,
    FeedItem,
    Collection,
    Selection,
    View,
    Background,
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
#[serde(rename_all = "kebab-case")]
pub enum PreviewIntent {
    Quick,
    Inspect,
    Analyze,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewSurface {
    Workspace,
    Thumbnail,
    Inspector,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewAugmentationSlot {
    PreviewOverlay,
    Inspector,
    Metadata,
    Histogram,
    ToolbarAction,
    ContextMenu,
    AnalysisAction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewContributionDescriptor {
    pub content_kinds: Vec<String>,
    pub intents: Vec<PreviewIntent>,
    pub surfaces: Vec<PreviewSurface>,
    pub priority: i32,
    #[serde(default)]
    pub slot: Option<PreviewAugmentationSlot>,
}

/// Framework-neutral session returned by a preview provider runtime.
///
/// The host owns how this descriptor is projected into a desktop surface;
/// providers only select a renderer entrypoint and the augmentation slots they
/// expose. This keeps runtime responses independent from Vue, React or Tauri.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    deny_unknown_fields,
    rename_all_fields = "camelCase"
)]
pub enum PreviewRendererDescriptor {
    Generic,
    UiBridge {
        entrypoint: String,
        bridge_version: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewSessionDescriptor {
    pub session_id: String,
    pub renderer: PreviewRendererDescriptor,
    /// Host Command ids that the preview surface may render as actions.
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub slots: Vec<PreviewAugmentationSlot>,
}

/// Context passed when a host command is invoked from one preview session.
/// The host keeps this projection bounded to the active asset and preview
/// session; it never exposes storage handles or unrestricted source paths.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewActionContext {
    pub session_id: String,
    pub provider_id: String,
    pub asset_id: String,
    pub content_kind: String,
    pub intent: PreviewIntent,
    pub surface: PreviewSurface,
}

/// Typed result of a preview action. A plugin may accept the action directly
/// or ask the host to create a Rust-owned JobSystem job for deferred work.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PreviewActionResult {
    Accepted,
    Job {
        #[serde(rename = "jobKind")]
        job_kind: String,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSurfaceDescriptor {
    pub kind: UiSurfaceKind,
    pub entrypoint: String,
    pub bridge_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityBarDescriptor {
    pub title: String,
    pub icon: String,
    pub order: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContributionDescriptor {
    pub id: String,
    pub kind: ContributionKind,
    pub scope: ContributionScope,
    /// Optional manifest echo. The host derives the authoritative namespace
    /// from the extension id and discriminated scope before projection.
    #[serde(default)]
    pub namespace: Option<String>,
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
    pub label: Option<String>,
    #[serde(default)]
    pub label_key: Option<String>,
    #[serde(default)]
    pub command_id: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub enablement: Option<String>,
    #[serde(default)]
    pub keybinding: Option<String>,
    #[serde(default)]
    pub keybinding_platform: Option<KeybindingPlatform>,
    #[serde(default)]
    pub menu_location: Option<MenuLocation>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub anchor: Option<ContextAnchor>,
    #[serde(default)]
    pub ui: Option<UiSurfaceDescriptor>,
    #[serde(default)]
    pub preview: Option<PreviewContributionDescriptor>,
}

impl Default for ContributionDescriptor {
    fn default() -> Self {
        Self {
            id: "builtin.contribution".into(),
            kind: ContributionKind::Command,
            scope: ContributionScope::Workbench,
            namespace: None,
            container: None,
            zone: None,
            title: None,
            title_key: None,
            icon: None,
            order: None,
            default_visible: None,
            view_container_id: None,
            activity_bar: None,
            label: None,
            label_key: None,
            command_id: None,
            group: None,
            when: None,
            enablement: None,
            keybinding: None,
            keybinding_platform: None,
            menu_location: None,
            required_capabilities: Vec::new(),
            anchor: None,
            ui: None,
            preview: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppTarget {
    pub app_id: String,
    pub api: VersionRange,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZonePermissionSet {
    pub zone: WorkbenchZone,
    pub permissions: Vec<Permission>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    pub manifest_schema: u16,
    pub id: ExtensionId,
    pub version: String,
    pub host_api: VersionRange,
    pub bridge_api: VersionRange,
    pub runtime_protocol: VersionRange,
    pub runtime: RuntimeKind,
    pub extension_kind: Vec<ExtensionKind>,
    pub launch: Option<RuntimeLaunchSpec>,
    pub activation_events: Vec<ActivationEvent>,
    pub extension_dependencies: Vec<ExtensionId>,
    pub target_app: Option<AppTarget>,
    pub support: ExtensionSupport,
    pub required_capabilities: Vec<String>,
    pub optional_capabilities: Vec<String>,
    pub localization: Option<LocalizationManifest>,
    #[serde(default)]
    pub storage: Option<ExtensionStorageDeclaration>,
    pub workbench_app: Option<WorkbenchAppDescriptor>,
    pub contributions: Vec<ContributionDescriptor>,
    pub permission_sets: Vec<ZonePermissionSet>,
    /// Reserved package metadata. Only keys prefixed with `x-` are accepted.
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestValidationError {
    InvalidExtensionId(String),
    InvalidContributionId(String),
    DuplicateContributionId(String),
    InvalidDependencyId(String),
    DuplicateDependencyId(String),
    ActivationReferencesUnknownContribution(String),
}

impl std::fmt::Display for ManifestValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidExtensionId(id) => write!(formatter, "invalid extension id {id}"),
            Self::InvalidContributionId(id) => write!(formatter, "invalid contribution id {id}"),
            Self::DuplicateContributionId(id) => {
                write!(formatter, "duplicate contribution id {id}")
            }
            Self::InvalidDependencyId(id) => write!(formatter, "invalid dependency id {id}"),
            Self::DuplicateDependencyId(id) => write!(formatter, "duplicate dependency id {id}"),
            Self::ActivationReferencesUnknownContribution(id) => write!(
                formatter,
                "activation event references unknown contribution {id}"
            ),
        }
    }
}

impl std::error::Error for ManifestValidationError {}

impl ExtensionManifest {
    /// Validates identifiers and manifest-local references before host policy.
    ///
    /// This deliberately does not validate runtime availability, permissions,
    /// zones or target applications; those belong to `PluginHost` policy.
    pub fn validate_contract(&self) -> Result<(), ManifestValidationError> {
        if !is_valid_manifest_identifier(&self.id.0) {
            return Err(ManifestValidationError::InvalidExtensionId(
                self.id.0.clone(),
            ));
        }

        let mut contribution_ids = std::collections::BTreeSet::new();
        for contribution in &self.contributions {
            if !is_valid_manifest_identifier(&contribution.id) {
                return Err(ManifestValidationError::InvalidContributionId(
                    contribution.id.clone(),
                ));
            }
            if !contribution_ids.insert(contribution.id.clone()) {
                return Err(ManifestValidationError::DuplicateContributionId(
                    contribution.id.clone(),
                ));
            }
        }

        let mut dependency_ids = std::collections::BTreeSet::new();
        for dependency in &self.extension_dependencies {
            if !is_valid_manifest_identifier(&dependency.0) {
                return Err(ManifestValidationError::InvalidDependencyId(
                    dependency.0.clone(),
                ));
            }
            if !dependency_ids.insert(dependency.0.clone()) {
                return Err(ManifestValidationError::DuplicateDependencyId(
                    dependency.0.clone(),
                ));
            }
        }

        for activation in &self.activation_events {
            let contribution_id = match activation {
                ActivationEvent::View { contribution_id }
                | ActivationEvent::Command { contribution_id } => Some(contribution_id),
                ActivationEvent::WorkbenchReady
                | ActivationEvent::App { .. }
                | ActivationEvent::Preview { .. }
                | ActivationEvent::Job { .. } => None,
            };
            if let Some(contribution_id) = contribution_id {
                if !contribution_ids.contains(contribution_id) {
                    return Err(
                        ManifestValidationError::ActivationReferencesUnknownContribution(
                            contribution_id.clone(),
                        ),
                    );
                }
            }
        }

        Ok(())
    }
}

fn is_valid_manifest_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && b"._-".contains(&byte))
        })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawExtensionManifest {
    manifest_schema: u16,
    id: ExtensionId,
    version: String,
    host_api: VersionRange,
    bridge_api: VersionRange,
    runtime_protocol: VersionRange,
    runtime: RuntimeKind,
    extension_kind: Vec<ExtensionKind>,
    launch: Option<RuntimeLaunchSpec>,
    activation_events: Vec<ActivationEvent>,
    extension_dependencies: Vec<ExtensionId>,
    target_app: Option<AppTarget>,
    support: ExtensionSupport,
    required_capabilities: Vec<String>,
    optional_capabilities: Vec<String>,
    localization: Option<LocalizationManifest>,
    #[serde(default)]
    storage: Option<ExtensionStorageDeclaration>,
    workbench_app: Option<WorkbenchAppDescriptor>,
    contributions: Vec<ContributionDescriptor>,
    permission_sets: Vec<ZonePermissionSet>,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}

impl<'de> Deserialize<'de> for ExtensionManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawExtensionManifest::deserialize(deserializer)?;
        validate_reserved_metadata(&raw.extensions).map_err(de::Error::custom)?;
        Ok(Self {
            manifest_schema: raw.manifest_schema,
            id: raw.id,
            version: raw.version,
            host_api: raw.host_api,
            bridge_api: raw.bridge_api,
            runtime_protocol: raw.runtime_protocol,
            runtime: raw.runtime,
            extension_kind: raw.extension_kind,
            launch: raw.launch,
            activation_events: raw.activation_events,
            extension_dependencies: raw.extension_dependencies,
            target_app: raw.target_app,
            support: raw.support,
            required_capabilities: raw.required_capabilities,
            optional_capabilities: raw.optional_capabilities,
            localization: raw.localization,
            storage: raw.storage,
            workbench_app: raw.workbench_app,
            contributions: raw.contributions,
            permission_sets: raw.permission_sets,
            extensions: raw.extensions,
        })
    }
}

pub fn validate_reserved_metadata(metadata: &BTreeMap<String, Value>) -> Result<(), String> {
    if let Some(key) = metadata.keys().find(|key| !key.starts_with("x-")) {
        return Err(format!(
            "unknown manifest field {key}; only x-* fields are reserved"
        ));
    }
    Ok(())
}

impl Default for ExtensionManifest {
    fn default() -> Self {
        Self {
            manifest_schema: MANIFEST_SCHEMA_VERSION,
            id: ExtensionId("builtin.default".into()),
            version: "0.0.0".into(),
            host_api: VersionRange("1".into()),
            bridge_api: VersionRange("1".into()),
            runtime_protocol: VersionRange("1".into()),
            runtime: RuntimeKind::BuiltIn,
            extension_kind: vec![ExtensionKind::Capability],
            launch: None,
            activation_events: Vec::new(),
            extension_dependencies: Vec::new(),
            target_app: None,
            support: ExtensionSupport::default(),
            required_capabilities: Vec::new(),
            optional_capabilities: Vec::new(),
            localization: None,
            storage: None,
            workbench_app: None,
            contributions: Vec::new(),
            permission_sets: Vec::new(),
            extensions: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionInstallOperation {
    Install,
    Upgrade,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionInstallRecoveryKind {
    Staging,
    Rollback,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionInstallRecoveryAction {
    Resume,
    Commit,
    Rollback,
    Discard,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionInstallRecoveryLogStatus {
    Succeeded,
    Failed,
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
pub struct ExtensionInstallPreview {
    pub manifest: ExtensionManifest,
    pub existing: Option<ExtensionSummary>,
    pub operation: ExtensionInstallOperation,
    pub compatible: bool,
    pub can_install: bool,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInstallRecovery {
    pub extension_id: ExtensionId,
    pub kind: ExtensionInstallRecoveryKind,
    pub operation: ExtensionInstallOperation,
    pub candidate_version: Option<String>,
    pub current: Option<ExtensionSummary>,
    pub rollback_version: Option<String>,
    pub can_resume: bool,
    pub can_commit: bool,
    pub can_rollback: bool,
    pub can_discard: bool,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInstallRecoveryLogEntry {
    pub extension_id: ExtensionId,
    pub action: ExtensionInstallRecoveryAction,
    pub status: ExtensionInstallRecoveryLogStatus,
    pub attempt: u32,
    pub message: String,
    pub removed_paths: Vec<String>,
    pub occurred_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionProjection {
    pub summary: ExtensionSummary,
    pub manifest: ExtensionManifest,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_manifest_uses_the_versioned_camel_case_shape() {
        let manifest = ExtensionManifest {
            id: ExtensionId("sample.reader".into()),
            extension_kind: vec![ExtensionKind::Ui, ExtensionKind::Capability],
            activation_events: vec![ActivationEvent::WorkbenchReady],
            ..ExtensionManifest::default()
        };

        let value = serde_json::to_value(&manifest).expect("manifest should serialize");
        assert_eq!(value["manifestSchema"], MANIFEST_SCHEMA_VERSION);
        assert_eq!(value["bridgeApi"], "1");
        assert_eq!(value["runtimeProtocol"], "1");
        assert_eq!(value["extensionKind"][0], "ui");
        assert_eq!(value["activationEvents"][0]["kind"], "onWorkbenchReady");

        let decoded: ExtensionManifest =
            serde_json::from_value(value).expect("canonical manifest should round-trip");
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn canonical_manifest_rejects_unknown_fields() {
        let mut value =
            serde_json::to_value(ExtensionManifest::default()).expect("manifest should serialize");
        value["unexpected"] = serde_json::json!(true);

        let error = serde_json::from_value::<ExtensionManifest>(value)
            .expect_err("unknown manifest fields should be rejected");
        assert!(error.to_string().contains("unknown manifest field"));
    }

    #[test]
    fn canonical_manifest_accepts_only_reserved_x_fields() {
        let mut value =
            serde_json::to_value(ExtensionManifest::default()).expect("manifest should serialize");
        value["x-vendor"] = serde_json::json!({ "feature": true });

        let manifest: ExtensionManifest =
            serde_json::from_value(value).expect("reserved metadata should be accepted");
        assert_eq!(manifest.extensions["x-vendor"]["feature"], true);
    }

    #[test]
    fn manifest_contract_requires_stable_unique_identifiers() {
        let mut manifest = ExtensionManifest {
            id: ExtensionId("publisher.reader".into()),
            contributions: vec![ContributionDescriptor {
                id: "reader.command".into(),
                kind: ContributionKind::Command,
                scope: ContributionScope::Capability,
                namespace: None,
                container: None,
                zone: None,
                title: None,
                title_key: None,
                icon: None,
                order: None,
                default_visible: None,
                view_container_id: None,
                activity_bar: None,
                label: None,
                label_key: None,
                command_id: None,
                group: None,
                when: None,
                enablement: None,
                keybinding: None,
                keybinding_platform: None,
                menu_location: None,
                required_capabilities: Vec::new(),
                anchor: None,
                ui: None,
                preview: None,
            }],
            ..ExtensionManifest::default()
        };
        manifest
            .validate_contract()
            .expect("valid manifest identifiers should pass");

        manifest
            .contributions
            .push(manifest.contributions[0].clone());
        assert_eq!(
            manifest.validate_contract(),
            Err(ManifestValidationError::DuplicateContributionId(
                "reader.command".into()
            ))
        );
    }

    #[test]
    fn manifest_contract_rejects_unknown_activation_targets() {
        let manifest = ExtensionManifest {
            activation_events: vec![ActivationEvent::Command {
                contribution_id: "reader.missing".into(),
            }],
            ..ExtensionManifest::default()
        };

        assert_eq!(
            manifest.validate_contract(),
            Err(
                ManifestValidationError::ActivationReferencesUnknownContribution(
                    "reader.missing".into()
                )
            )
        );
    }

    #[test]
    fn manifest_contract_rejects_invalid_dependency_identifiers() {
        let manifest = ExtensionManifest {
            extension_dependencies: vec![ExtensionId("../unsafe".into())],
            ..ExtensionManifest::default()
        };

        assert_eq!(
            manifest.validate_contract(),
            Err(ManifestValidationError::InvalidDependencyId(
                "../unsafe".into()
            ))
        );
    }

    #[test]
    fn version_ranges_support_exact_wildcard_and_caret_major_matching() {
        let provided = VersionRange("1".into());
        assert!(VersionRange("1".into()).is_compatible_with(&provided));
        assert!(VersionRange("*".into()).is_compatible_with(&provided));
        assert!(VersionRange("^1".into()).is_compatible_with(&provided));
        assert!(!VersionRange("^2".into()).is_compatible_with(&provided));
    }

    #[test]
    fn preview_session_descriptor_uses_stable_wire_names() {
        let descriptor = PreviewSessionDescriptor {
            session_id: "sample.preview:asset-1".into(),
            renderer: PreviewRendererDescriptor::UiBridge {
                entrypoint: "wasm://sample/preview".into(),
                bridge_version: "1".into(),
            },
            commands: vec!["preview.zoom".into()],
            slots: vec![PreviewAugmentationSlot::PreviewOverlay],
        };
        let value = serde_json::to_value(&descriptor).expect("descriptor should serialize");
        assert_eq!(value["sessionId"], "sample.preview:asset-1");
        assert_eq!(value["renderer"]["kind"], "ui-bridge");
        assert_eq!(value["renderer"]["bridgeVersion"], "1");
        assert_eq!(value["slots"][0], "preview-overlay");
        assert_eq!(
            serde_json::from_value::<PreviewSessionDescriptor>(value)
                .expect("descriptor should round-trip"),
            descriptor
        );
    }

    #[test]
    fn preview_action_contract_round_trips_job_handoff() {
        let context = PreviewActionContext {
            session_id: "preview.example:asset-1".into(),
            provider_id: "preview.example.preview.image".into(),
            asset_id: "asset-1".into(),
            content_kind: "image".into(),
            intent: PreviewIntent::Analyze,
            surface: PreviewSurface::Workspace,
        };
        let value =
            serde_json::to_value(&context).expect("preview action context should serialize");
        assert_eq!(value["sessionId"], "preview.example:asset-1");
        assert_eq!(value["intent"], "analyze");

        let result = PreviewActionResult::Job {
            job_kind: "histogram".into(),
            duration_ms: 1200,
        };
        let value = serde_json::to_value(&result).expect("preview action result should serialize");
        assert_eq!(value["kind"], "job");
        assert_eq!(value["jobKind"], "histogram");
        assert_eq!(
            serde_json::from_value::<PreviewActionResult>(value)
                .expect("preview action result should round-trip"),
            result
        );
    }

    #[test]
    fn capability_names_are_stable_and_round_trip_on_the_wire() {
        for capability in [
            Capability::AssetRead,
            Capability::AssetFacetWrite,
            Capability::CoreMetadataWrite,
            Capability::Secret,
        ] {
            let name = capability.name();
            let value = serde_json::to_value(&capability).expect("capability should serialize");
            assert_eq!(value, name);
            assert_eq!(
                serde_json::from_value::<Capability>(value).expect("capability should decode"),
                capability
            );
            assert_eq!(Capability::from_name(name), Some(capability));
        }
    }

    #[test]
    fn high_risk_capabilities_are_explicit() {
        assert!(Capability::Network.is_high_risk());
        assert!(Capability::Filesystem.is_high_risk());
        assert!(Capability::Secret.is_high_risk());
        assert!(!Capability::AssetRead.is_high_risk());
        assert!(!Capability::EventsSubscribe.is_high_risk());
    }

    #[test]
    fn command_surface_fields_round_trip_as_stable_manifest_data() {
        let descriptor = ContributionDescriptor {
            id: "sample.inspect".into(),
            kind: ContributionKind::Command,
            scope: ContributionScope::Workbench,
            namespace: Some("workbench".into()),
            title: Some("Inspect".into()),
            enablement: Some("asset.selected".into()),
            keybinding: Some("ctrl+shift+i".into()),
            keybinding_platform: Some(KeybindingPlatform::Any),
            menu_location: Some(MenuLocation::CommandPalette),
            required_capabilities: vec!["command.register".into()],
            ..ContributionDescriptor::default()
        };
        let value = serde_json::to_value(&descriptor).expect("descriptor should serialize");
        assert_eq!(value["enablement"], "asset.selected");
        assert_eq!(value["keybindingPlatform"], "any");
        assert_eq!(value["menuLocation"], "command-palette");
        assert_eq!(
            serde_json::from_value::<ContributionDescriptor>(value)
                .expect("descriptor should decode"),
            descriptor
        );
    }

    #[test]
    fn asset_inspector_fixture_uses_the_public_manifest_contract() {
        let manifest: ExtensionManifest = serde_json::from_str(include_str!(
            "../../../examples/plugins/asset-inspector/manifest.json"
        ))
        .expect("Asset Inspector fixture manifest should decode");
        manifest
            .validate_contract()
            .expect("Asset Inspector fixture manifest should validate");
        assert_eq!(manifest.id.0, "com.acme.asset-inspector");
        assert!(manifest
            .required_capabilities
            .iter()
            .any(|capability| capability == "asset.facet.write"));
        assert!(manifest
            .activation_events
            .iter()
            .any(|event| matches!(event, ActivationEvent::Job { .. })));
    }
}
