use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use velvet_asset_core::AssetId;
use velvet_job_system::{JobId, JobSnapshot};
use velvet_layout_model::{LayoutCommand, LayoutState};
use velvet_plugin_api::{
    AppTarget, ContributionDescriptor, ExtensionId, ExtensionInstallPreview,
    ExtensionInstallRecovery, ExtensionInstallRecoveryAction, ExtensionInstallRecoveryLogEntry,
    ExtensionKind, ExtensionManifest, ExtensionSummary, ExtensionSupport, LocalizationManifest,
    PreviewActionContext, RuntimeKind, RuntimeLaunchSpec, VersionRange, WorkbenchAppDescriptor,
    ZonePermissionSet,
};
use velvet_preview_core::{PreviewIntent, PreviewSurface, ResolvedPreview};

pub const PROTOCOL_VERSION: u16 = 1;
pub const PROFILE_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileState {
    pub schema_version: u16,
    pub revision: u64,
    pub locale: String,
}

impl Default for ProfileState {
    fn default() -> Self {
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            revision: 0,
            locale: "en".into(),
        }
    }
}

/// Transport-owned mirror of the Rust plugin-api manifest.
///
/// `velvet-plugin-api` owns manifest semantics and validation. This DTO keeps
/// the protocol boundary explicit so a future wire evolution does not silently
/// become a domain-model change.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifestDto {
    pub manifest_schema: u16,
    pub id: ExtensionId,
    pub version: String,
    pub host_api: VersionRange,
    pub bridge_api: VersionRange,
    pub runtime_protocol: VersionRange,
    pub runtime: RuntimeKind,
    pub extension_kind: Vec<ExtensionKind>,
    pub launch: Option<RuntimeLaunchSpec>,
    pub activation_events: Vec<velvet_plugin_api::ActivationEvent>,
    pub extension_dependencies: Vec<ExtensionId>,
    pub target_app: Option<AppTarget>,
    pub support: ExtensionSupport,
    pub required_capabilities: Vec<String>,
    pub optional_capabilities: Vec<String>,
    pub localization: Option<LocalizationManifest>,
    pub workbench_app: Option<WorkbenchAppDescriptor>,
    pub contributions: Vec<ContributionDescriptor>,
    pub permission_sets: Vec<ZonePermissionSet>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

impl From<ExtensionManifest> for PluginManifestDto {
    fn from(manifest: ExtensionManifest) -> Self {
        Self {
            manifest_schema: manifest.manifest_schema,
            id: manifest.id,
            version: manifest.version,
            host_api: manifest.host_api,
            bridge_api: manifest.bridge_api,
            runtime_protocol: manifest.runtime_protocol,
            runtime: manifest.runtime,
            extension_kind: manifest.extension_kind,
            launch: manifest.launch,
            activation_events: manifest.activation_events,
            extension_dependencies: manifest.extension_dependencies,
            target_app: manifest.target_app,
            support: manifest.support,
            required_capabilities: manifest.required_capabilities,
            optional_capabilities: manifest.optional_capabilities,
            localization: manifest.localization,
            workbench_app: manifest.workbench_app,
            contributions: manifest.contributions,
            permission_sets: manifest.permission_sets,
            extensions: manifest.extensions,
        }
    }
}

impl From<PluginManifestDto> for ExtensionManifest {
    fn from(manifest: PluginManifestDto) -> Self {
        Self {
            manifest_schema: manifest.manifest_schema,
            id: manifest.id,
            version: manifest.version,
            host_api: manifest.host_api,
            bridge_api: manifest.bridge_api,
            runtime_protocol: manifest.runtime_protocol,
            runtime: manifest.runtime,
            extension_kind: manifest.extension_kind,
            launch: manifest.launch,
            activation_events: manifest.activation_events,
            extension_dependencies: manifest.extension_dependencies,
            target_app: manifest.target_app,
            support: manifest.support,
            required_capabilities: manifest.required_capabilities,
            optional_capabilities: manifest.optional_capabilities,
            localization: manifest.localization,
            workbench_app: manifest.workbench_app,
            contributions: manifest.contributions,
            permission_sets: manifest.permission_sets,
            extensions: manifest.extensions,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginProjectionDto {
    pub summary: ExtensionSummary,
    pub manifest: PluginManifestDto,
}

impl From<velvet_plugin_api::ExtensionProjection> for PluginProjectionDto {
    fn from(projection: velvet_plugin_api::ExtensionProjection) -> Self {
        Self {
            summary: projection.summary,
            manifest: projection.manifest.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginInstallPreviewDto {
    pub manifest: PluginManifestDto,
    pub existing: Option<ExtensionSummary>,
    pub operation: velvet_plugin_api::ExtensionInstallOperation,
    pub compatible: bool,
    pub can_install: bool,
    pub issues: Vec<String>,
}

impl From<ExtensionInstallPreview> for PluginInstallPreviewDto {
    fn from(preview: ExtensionInstallPreview) -> Self {
        Self {
            manifest: preview.manifest.into(),
            existing: preview.existing,
            operation: preview.operation,
            compatible: preview.compatible,
            can_install: preview.can_install,
            issues: preview.issues,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Layout(LayoutCommand),
    LayoutAt {
        expected_revision: Option<u64>,
        command: LayoutCommand,
    },
    RegisterPlugin {
        manifest: Box<PluginManifestDto>,
    },
    ActivatePlugin {
        extension_id: ExtensionId,
    },
    DisablePlugin {
        extension_id: ExtensionId,
        reason: PluginDisableReason,
    },
    CheckPluginHealth {
        extension_id: ExtensionId,
    },
    RecoverPlugin {
        extension_id: ExtensionId,
    },
    InstallPlugin {
        source_dir: String,
    },
    InspectPlugin {
        source_dir: String,
    },
    CommitPluginInstall {
        extension_id: ExtensionId,
    },
    RollbackPluginInstall {
        extension_id: ExtensionId,
    },
    ResolvePluginInstallRecovery {
        extension_id: ExtensionId,
        action: ExtensionInstallRecoveryAction,
    },
    ClearPluginInstallRecoveryLog,
    ExportPluginInstallRecoveryLog {
        destination: String,
    },
    UninstallPlugin {
        extension_id: ExtensionId,
    },
    InvokePluginCommand {
        extension_id: ExtensionId,
        command_id: String,
        args: Value,
        context: Value,
    },
    InvokePreviewAction {
        extension_id: ExtensionId,
        command_id: String,
        args: Value,
        context: PreviewActionContext,
    },
    ReleasePreviewSession {
        session_id: String,
    },
    CreateJob {
        kind: String,
        duration_ms: u64,
        will_fail: bool,
        input: Value,
    },
    CancelJob {
        job_id: JobId,
    },
    RetryJob {
        job_id: JobId,
    },
    SetLocale {
        locale: String,
        expected_revision: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginDisableReason {
    User,
    Crash,
    Incompatible,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Query {
    Layout,
    Profile,
    Extensions,
    PluginProjection {
        extension_id: ExtensionId,
    },
    ResolvePreview {
        asset_id: AssetId,
        content_kind: String,
        intent: PreviewIntent,
        surface: PreviewSurface,
    },
    PluginInstallRecoveries,
    PluginInstallRecoveryLog,
    Jobs,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    Command(Command),
    Query(Query),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response {
    Layout(LayoutState),
    Profile(ProfileState),
    Extensions(Vec<ExtensionSummary>),
    Extension(ExtensionSummary),
    PluginInstallPreview(PluginInstallPreviewDto),
    PluginInstallRecoveries(Vec<ExtensionInstallRecovery>),
    PluginInstallRecoveryLog(Vec<ExtensionInstallRecoveryLogEntry>),
    PluginProjection(PluginProjectionDto),
    Job(JobSnapshot),
    Jobs(Vec<JobSnapshot>),
    Preview(ResolvedPreview),
    Accepted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    LayoutChanged {
        revision: u64,
        state: LayoutState,
    },
    ProfileChanged {
        revision: u64,
        state: ProfileState,
    },
    PluginDisabled {
        extension_id: String,
        reason: String,
    },
    JobChanged {
        snapshot: Box<JobSnapshot>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    ProtocolVersionMismatch {
        expected: u16,
        received: u16,
    },
    InvalidCommand(String),
    Layout(String),
    Plugin(String),
    PluginRuntimeFailed {
        extension_id: String,
        message: String,
    },
    Job(String),
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    Profile(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestEnvelope {
    pub protocol_version: u16,
    pub request_id: RequestId,
    pub request: Request,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseEnvelope {
    pub protocol_version: u16,
    pub request_id: RequestId,
    pub response: Response,
}

pub trait CoreTransport: Send + Sync {
    fn dispatch(&self, request_id: RequestId, request: Request) -> Result<Response, CoreError>;

    fn dispatch_envelope(&self, envelope: RequestEnvelope) -> Result<ResponseEnvelope, CoreError> {
        if envelope.protocol_version != PROTOCOL_VERSION {
            return Err(CoreError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                received: envelope.protocol_version,
            });
        }

        let request_id = envelope.request_id.clone();
        let response = self.dispatch(envelope.request_id, envelope.request)?;
        Ok(ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            response,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_manifest_dto_round_trips_without_becoming_the_domain_owner() {
        let manifest = ExtensionManifest {
            id: ExtensionId("sample.reader".into()),
            extension_kind: vec![ExtensionKind::Ui, ExtensionKind::Capability],
            activation_events: vec![velvet_plugin_api::ActivationEvent::WorkbenchReady],
            ..ExtensionManifest::default()
        };

        let dto = PluginManifestDto::from(manifest.clone());
        let json = serde_json::to_value(&dto).expect("protocol DTO should serialize");
        assert_eq!(json["manifestSchema"], 1);
        assert_eq!(json["bridgeApi"], "1");
        assert_eq!(json["activationEvents"][0]["kind"], "onWorkbenchReady");

        let decoded: PluginManifestDto =
            serde_json::from_value(json).expect("protocol DTO should deserialize");
        assert_eq!(ExtensionManifest::from(decoded), manifest);
    }

    #[test]
    fn profile_state_round_trips_with_canonical_wire_names() {
        let state = ProfileState {
            schema_version: PROFILE_SCHEMA_VERSION,
            revision: 4,
            locale: "zh-Hans".into(),
        };

        let json = serde_json::to_value(&state).expect("profile state should serialize");
        assert_eq!(json["schemaVersion"], PROFILE_SCHEMA_VERSION);
        assert_eq!(json["revision"], 4);
        assert_eq!(json["locale"], "zh-Hans");
        assert!(json.get("schema_version").is_none());

        let decoded: ProfileState =
            serde_json::from_value(json).expect("profile state should deserialize");
        assert_eq!(decoded, state);
        assert!(serde_json::from_value::<ProfileState>(serde_json::json!({
            "schemaVersion": PROFILE_SCHEMA_VERSION,
            "revision": 4,
            "locale": "zh-Hans",
            "unexpected": true
        }))
        .is_err());
    }
}
