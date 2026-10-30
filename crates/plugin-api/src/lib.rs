use std::collections::BTreeMap;

use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use velvet_layout_model::WorkbenchZone;

pub const MANIFEST_SCHEMA_VERSION: u16 = 1;

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
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
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
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub slots: Vec<PreviewAugmentationSlot>,
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
    pub anchor: Option<ContextAnchor>,
    #[serde(default)]
    pub ui: Option<UiSurfaceDescriptor>,
    #[serde(default)]
    pub preview: Option<PreviewContributionDescriptor>,
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
    pub workbench_app: Option<WorkbenchAppDescriptor>,
    pub contributions: Vec<ContributionDescriptor>,
    pub permission_sets: Vec<ZonePermissionSet>,
    /// Reserved package metadata. Only keys prefixed with `x-` are accepted.
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
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
}
