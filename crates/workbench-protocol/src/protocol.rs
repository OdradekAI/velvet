//! Protocol DTOs, negotiation, envelopes and CoreTransport implementation.
//!
//! The crate facade re-exports these contract types. This module owns wire
//! semantics only; it does not own persistence, plugin runtime or UI state.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use velvet_asset_core::AssetId;
use velvet_job_system::{
    JobBudgetRequest, JobExecutionEnvironment, JobExecutionPolicy, JobId, JobSnapshot,
    JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION,
};
use velvet_layout_model::{LayoutCommand, LayoutState, WorkbenchZone};
use velvet_plugin_api::{
    AppTarget, Capability, ContributionDescriptor, ContributionKind, ContributionScope,
    ExtensionId, ExtensionInstallPreview, ExtensionInstallRecovery, ExtensionInstallRecoveryAction,
    ExtensionInstallRecoveryLogEntry, ExtensionKind, ExtensionManifest,
    ExtensionStorageDeclaration, ExtensionSummary, ExtensionSupport, KeybindingPlatform,
    LocalizationManifest, MenuLocation, PreviewActionContext, RuntimeKind, RuntimeLaunchSpec,
    VersionRange, WorkbenchAppDescriptor, ZonePermissionSet,
};
use velvet_preview_core::{PreviewIntent, PreviewSurface, ResolvedPreview};

pub const PROTOCOL_VERSION: u16 = 1;
/// Major/minor transport version used during connection negotiation.
///
/// `PROTOCOL_VERSION` remains the legacy per-request discriminator while the
/// handshake gives the desktop and core an explicit compatibility window.
pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 0;
pub const TRANSPORT_FEATURE_SESSION_V1: &str = "session-v1";
pub const TRANSPORT_FEATURE_SCHEMA_NEGOTIATION_V1: &str = "schema-negotiation-v1";
pub const PROFILE_SCHEMA_VERSION: u16 = 1;
pub const PLUGIN_RESOURCE_SCHEMA_VERSION: u16 = 1;
pub const RECOVERY_SCHEMA_VERSION: u16 = 1;
pub const RECOVERY_DIAGNOSTIC_SCHEMA_VERSION: u16 = 1;
pub const PLUGIN_DIAGNOSTIC_SCHEMA_VERSION: u16 = 1;
pub const PLUGIN_LOG_SCHEMA_VERSION: u16 = 1;
pub const PLUGIN_NOTIFICATION_SCHEMA_VERSION: u16 = 1;
pub const PLUGIN_REGISTRY_SCHEMA_VERSION: u16 = 1;
pub const PLUGIN_CAPABILITY_SCHEMA_VERSION: u16 = 1;
pub const PLUGIN_COMMAND_SURFACE_SCHEMA_VERSION: u16 = 1;
pub const CRASH_CAPTURE_SCHEMA_VERSION: u16 = 1;
pub const CORE_SNAPSHOT_SCHEMA_VERSION: u16 = 1;
pub const ACTIVATION_SCHEMA_VERSION: u16 = 1;
pub const RELEASE_MANIFEST_SCHEMA_VERSION: u16 = 1;
pub const RELEASE_TRANSACTION_SCHEMA_VERSION: u16 = 1;
pub const OPERATION_SCHEMA_VERSION: u16 = 1;
pub const PORTABLE_BUNDLE_SCHEMA_VERSION: u16 = 1;
pub const IMPORT_PLAN_SCHEMA_VERSION: u16 = 1;
pub const IMPORT_RECEIPT_SCHEMA_VERSION: u16 = 1;
pub const ASSET_BUNDLE_SCHEMA_VERSION: u16 = 1;
pub const IMPORT_QUARANTINE_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationErrorCode {
    InvalidPlan,
    StaleTarget,
    AlreadyRecorded,
    ReconciliationRequired,
    ProviderFailed,
    ProviderUnknown,
    Persistence,
}

impl OperationErrorCode {
    pub const fn as_wire_code(self) -> &'static str {
        match self {
            Self::InvalidPlan => "operation-invalid-plan",
            Self::StaleTarget => "operation-stale-target",
            Self::AlreadyRecorded => "operation-already-recorded",
            Self::ReconciliationRequired => "operation-reconciliation-required",
            Self::ProviderFailed => "operation-provider-failed",
            Self::ProviderUnknown => "operation-provider-unknown",
            Self::Persistence => "operation-persistence",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationError {
    pub code: OperationErrorCode,
    pub message: String,
    pub operation_id: Option<String>,
    pub retryable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtocolHandshakeRequest {
    pub client_id: String,
    pub protocol_major: u16,
    pub protocol_minor: u16,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub schema_versions: BTreeMap<String, u16>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtocolHandshakeResponse {
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub selected_features: Vec<String>,
    pub schema_versions: BTreeMap<String, u16>,
    pub session_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationRequest {
    pub schema_version: u16,
    pub request_id: String,
    pub targets: Vec<ActivationTarget>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationTarget {
    pub kind: ActivationTargetKind,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActivationTargetKind {
    FilePath,
    DeepLink,
}

impl ActivationRequest {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_version != ACTIVATION_SCHEMA_VERSION {
            return Err(CoreError::InvalidCommand(format!(
                "unsupported activation schema {}",
                self.schema_version
            )));
        }
        if self.request_id.is_empty() || self.request_id.len() > 128 {
            return Err(CoreError::InvalidCommand(
                "activation request id must contain 1..=128 bytes".into(),
            ));
        }
        if self.targets.len() > 32 {
            return Err(CoreError::InvalidCommand(
                "activation request contains too many targets".into(),
            ));
        }
        for target in &self.targets {
            if target.value.is_empty() || target.value.len() > 4096 || target.value.contains('\0') {
                return Err(CoreError::InvalidCommand(
                    "activation target is empty, oversized or contains NUL".into(),
                ));
            }
            if matches!(target.kind, ActivationTargetKind::DeepLink)
                && !target.value.starts_with("velvet://")
                && !target.value.starts_with("file://")
            {
                return Err(CoreError::InvalidCommand(
                    "activation deep link scheme is not allowed".into(),
                ));
            }
        }
        Ok(())
    }
}

pub fn supported_transport_features() -> Vec<String> {
    vec![
        TRANSPORT_FEATURE_SESSION_V1.into(),
        TRANSPORT_FEATURE_SCHEMA_NEGOTIATION_V1.into(),
    ]
}

pub fn supported_schema_versions() -> BTreeMap<String, u16> {
    BTreeMap::from([
        ("plugin-protocol".into(), PLUGIN_PROTOCOL_SCHEMA_VERSION),
        ("profile".into(), PROFILE_SCHEMA_VERSION),
        ("recovery".into(), RECOVERY_SCHEMA_VERSION),
        (
            "recovery-diagnostic".into(),
            RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
        ),
        ("plugin-resource".into(), PLUGIN_RESOURCE_SCHEMA_VERSION),
        ("plugin-diagnostic".into(), PLUGIN_DIAGNOSTIC_SCHEMA_VERSION),
        ("plugin-log".into(), PLUGIN_LOG_SCHEMA_VERSION),
        (
            "plugin-notification".into(),
            PLUGIN_NOTIFICATION_SCHEMA_VERSION,
        ),
        ("plugin-registry".into(), PLUGIN_REGISTRY_SCHEMA_VERSION),
        ("plugin-capability".into(), PLUGIN_CAPABILITY_SCHEMA_VERSION),
        (
            "plugin-command-surface".into(),
            PLUGIN_COMMAND_SURFACE_SCHEMA_VERSION,
        ),
        ("crash-capture".into(), CRASH_CAPTURE_SCHEMA_VERSION),
        ("release-manifest".into(), RELEASE_MANIFEST_SCHEMA_VERSION),
        (
            "release-transaction".into(),
            RELEASE_TRANSACTION_SCHEMA_VERSION,
        ),
        (
            "job-execution-environment".into(),
            JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION,
        ),
        ("operation".into(), OPERATION_SCHEMA_VERSION),
        ("portable-bundle".into(), PORTABLE_BUNDLE_SCHEMA_VERSION),
        ("asset-bundle".into(), ASSET_BUNDLE_SCHEMA_VERSION),
        ("import-plan".into(), IMPORT_PLAN_SCHEMA_VERSION),
        ("import-receipt".into(), IMPORT_RECEIPT_SCHEMA_VERSION),
        ("import-quarantine".into(), IMPORT_QUARANTINE_SCHEMA_VERSION),
    ])
}

pub fn negotiate_protocol(
    request: ProtocolHandshakeRequest,
    session_id: String,
) -> Result<ProtocolHandshakeResponse, CoreError> {
    if request.protocol_major != PROTOCOL_MAJOR {
        return Err(CoreError::ProtocolMajorMismatch {
            expected: PROTOCOL_MAJOR,
            received: request.protocol_major,
        });
    }
    if request.client_id.is_empty() {
        return Err(CoreError::InvalidCommand(
            "client id must not be empty".into(),
        ));
    }
    if session_id.is_empty() {
        return Err(CoreError::InvalidCommand(
            "session id must not be empty".into(),
        ));
    }

    let supported_features = supported_transport_features();
    let selected_features = request
        .features
        .into_iter()
        .filter(|feature| supported_features.iter().any(|item| item == feature))
        .collect();
    let server_schemas = supported_schema_versions();
    let schema_versions = request
        .schema_versions
        .into_iter()
        .filter_map(|(name, client_version)| {
            server_schemas
                .get(&name)
                .map(|server_version| (name, client_version.min(*server_version)))
        })
        .collect();

    Ok(ProtocolHandshakeResponse {
        protocol_major: PROTOCOL_MAJOR,
        protocol_minor: negotiate_minor(request.protocol_minor, PROTOCOL_MINOR),
        selected_features,
        schema_versions,
        session_id,
    })
}

fn negotiate_minor(client: u16, server: u16) -> u16 {
    if client < server {
        client
    } else {
        server
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileState {
    pub schema_version: u16,
    pub revision: u64,
    pub locale: String,
    /// Explicit Profile authorization for unsigned local extension packages.
    /// `None` preserves legacy profiles and lets the composition policy decide
    /// the default; it is never treated as a production trust grant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_developer_mode: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthoritativeSnapshot {
    pub schema_version: u16,
    pub layout: LayoutState,
    pub profile: ProfileState,
    pub recovery: RecoveryProjection,
    pub jobs: Vec<JobSnapshot>,
    pub extensions: Vec<ExtensionSummary>,
}

impl Default for ProfileState {
    fn default() -> Self {
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            revision: 0,
            locale: "en".into(),
            plugin_developer_mode: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginResourceDto {
    pub schema_version: u16,
    pub extension_id: ExtensionId,
    pub locale: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
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
    pub storage: Option<ExtensionStorageDeclaration>,
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
            storage: manifest.storage,
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
            storage: manifest.storage,
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

/// Host-owned contribution registrations projected to a desktop client.
///
/// The descriptor remains declarative. No runtime process, framework object or
/// registry implementation crosses the transport seam.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginContributionRegistrationDto {
    pub extension_id: ExtensionId,
    pub contribution_id: String,
    pub namespace: String,
    pub kind: ContributionKind,
    pub scope: ContributionScope,
    pub zone: Option<WorkbenchZone>,
    pub schema_version: u16,
    pub descriptor: ContributionDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginRegistrySnapshotDto {
    pub schema_version: u16,
    pub generation: u64,
    pub contributions: Vec<PluginContributionRegistrationDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginCapabilityDenyReason {
    NotRequested,
    HostPolicy,
    UserApprovalRequired,
    Revoked,
    Inactive,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCapabilityDenialDto {
    pub capability: Capability,
    pub reason: PluginCapabilityDenyReason,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCapabilityGrantDto {
    pub schema_version: u16,
    pub extension_id: ExtensionId,
    pub generation: u64,
    pub session_id: Option<String>,
    pub active: bool,
    pub requested: Vec<Capability>,
    pub granted: Vec<Capability>,
    pub denied: Vec<PluginCapabilityDenialDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginCapabilityAuditAction {
    Granted,
    Revoked,
    Denied,
    Cleared,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCapabilityAuditDto {
    pub schema_version: u16,
    pub sequence: u64,
    pub extension_id: ExtensionId,
    pub capability: Capability,
    pub action: PluginCapabilityAuditAction,
    pub reason: Option<PluginCapabilityDenyReason>,
    pub generation: u64,
    pub occurred_at_ms: u64,
}

impl Default for PluginCapabilityGrantDto {
    fn default() -> Self {
        Self {
            schema_version: PLUGIN_CAPABILITY_SCHEMA_VERSION,
            extension_id: ExtensionId(String::new()),
            generation: 0,
            session_id: None,
            active: false,
            requested: Vec::new(),
            granted: Vec::new(),
            denied: Vec::new(),
        }
    }
}

impl Default for PluginRegistrySnapshotDto {
    fn default() -> Self {
        Self {
            schema_version: PLUGIN_REGISTRY_SCHEMA_VERSION,
            generation: 0,
            contributions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCommandProjectionDto {
    pub extension_id: ExtensionId,
    pub id: String,
    pub namespace: String,
    pub scope: ContributionScope,
    pub title: Option<String>,
    pub title_key: Option<String>,
    pub when: Option<String>,
    pub enablement: Option<String>,
    pub order: Option<u32>,
    pub required_capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginMenuProjectionDto {
    pub extension_id: ExtensionId,
    pub id: String,
    pub namespace: String,
    pub scope: ContributionScope,
    pub location: MenuLocation,
    pub command_id: String,
    pub label: Option<String>,
    pub label_key: Option<String>,
    pub group: Option<String>,
    pub order: Option<u32>,
    pub when: Option<String>,
    pub anchor: Option<velvet_plugin_api::ContextAnchor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginKeybindingProjectionDto {
    pub extension_id: ExtensionId,
    pub id: String,
    pub namespace: String,
    pub scope: ContributionScope,
    pub command_id: String,
    pub keybinding: String,
    pub platform: KeybindingPlatform,
    pub when: Option<String>,
    pub order: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCommandSurfaceSnapshotDto {
    pub schema_version: u16,
    pub generation: u64,
    pub context_revision: u64,
    pub commands: Vec<PluginCommandProjectionDto>,
    pub menus: Vec<PluginMenuProjectionDto>,
    pub keybindings: Vec<PluginKeybindingProjectionDto>,
}

impl Default for PluginCommandSurfaceSnapshotDto {
    fn default() -> Self {
        Self {
            schema_version: PLUGIN_COMMAND_SURFACE_SCHEMA_VERSION,
            generation: 0,
            context_revision: 0,
            commands: Vec::new(),
            menus: Vec::new(),
            keybindings: Vec::new(),
        }
    }
}

/// Versioned plugin-specific messages carried by the CoreTransport seam.
///
/// The envelope is intentionally narrower than the general CoreTransport
/// request set: plugin code can invoke a declared command or query its own
/// host projection, but it cannot address ApplicationCore internals directly.
pub const PLUGIN_PROTOCOL_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginRequestEnvelope {
    pub protocol_version: u16,
    pub session_id: String,
    /// Identity asserted by the host runtime adapter. Validation requires the
    /// message target to match this id before it reaches ApplicationCore.
    pub caller_extension_id: ExtensionId,
    pub request_id: RequestId,
    pub message: PluginRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PluginRequest {
    InvokeCommand {
        extension_id: ExtensionId,
        command_id: String,
        args: Value,
        context: Value,
    },
    QueryProjection {
        extension_id: ExtensionId,
    },
    QueryCapabilities {
        extension_id: ExtensionId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginResponseEnvelope {
    pub protocol_version: u16,
    pub session_id: String,
    pub request_id: RequestId,
    pub response: PluginResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PluginResponse {
    Accepted,
    Projection {
        projection: Box<PluginProjectionDto>,
    },
    Capabilities {
        capabilities: Box<PluginCapabilityGrantDto>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginEventEnvelope {
    pub protocol_version: u16,
    pub session_id: String,
    pub sequence: u64,
    pub event: PluginEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PluginEvent {
    Disabled {
        extension_id: ExtensionId,
        reason: String,
    },
    Diagnostic {
        diagnostic: PluginDiagnosticEvent,
    },
}

impl PluginRequestEnvelope {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.protocol_version != PLUGIN_PROTOCOL_SCHEMA_VERSION {
            return Err(CoreError::ProtocolVersionMismatch {
                expected: PLUGIN_PROTOCOL_SCHEMA_VERSION,
                received: self.protocol_version,
            });
        }
        if self.session_id.trim().is_empty() {
            return Err(CoreError::InvalidCommand(
                "plugin transport session id must not be empty".into(),
            ));
        }
        if self.request_id.0 == 0 {
            return Err(CoreError::InvalidCommand(
                "plugin transport request id must be non-zero".into(),
            ));
        }
        let target_extension_id = match &self.message {
            PluginRequest::InvokeCommand { extension_id, .. }
            | PluginRequest::QueryProjection { extension_id }
            | PluginRequest::QueryCapabilities { extension_id } => extension_id,
        };
        if target_extension_id != &self.caller_extension_id {
            return Err(CoreError::InvalidCommand(
                "plugin caller cannot address another extension namespace".into(),
            ));
        }
        Ok(())
    }

    pub fn validate_session(&self, expected_session_id: &str) -> Result<(), CoreError> {
        self.validate()?;
        if self.session_id != expected_session_id {
            return Err(CoreError::SessionExpired {
                session_id: self.session_id.clone(),
            });
        }
        Ok(())
    }

    /// Returns the replay policy inherited by the general transport cache.
    /// Invocation is deliberately non-idempotent; projection reads are safe to
    /// repeat after a reconnect.
    pub const fn idempotency_scope(&self) -> Option<&'static str> {
        match &self.message {
            PluginRequest::InvokeCommand { .. } => None,
            PluginRequest::QueryProjection { .. } => Some("plugin-projection"),
            PluginRequest::QueryCapabilities { .. } => Some("plugin-capabilities"),
        }
    }

    pub fn into_request(self) -> Request {
        match self.message {
            PluginRequest::InvokeCommand {
                extension_id,
                command_id,
                args,
                context,
            } => Request::Command(Command::InvokePluginCommand {
                extension_id,
                command_id,
                args,
                context,
                context_revision: 0,
                deadline_ms: None,
                idempotency_key: None,
            }),
            PluginRequest::QueryProjection { extension_id } => {
                Request::Query(Query::PluginProjection { extension_id })
            }
            PluginRequest::QueryCapabilities { extension_id } => {
                Request::Query(Query::PluginCapabilities { extension_id })
            }
        }
    }

    pub fn into_request_envelope(self) -> RequestEnvelope {
        let protocol_version = self.protocol_version;
        let request_id = self.request_id.clone();
        let request = self.into_request();
        RequestEnvelope {
            protocol_version,
            request_id,
            request,
        }
    }
}

impl PluginResponseEnvelope {
    pub fn from_core(session_id: String, response: ResponseEnvelope) -> Result<Self, CoreError> {
        let plugin_response = match response.response {
            Response::Accepted => PluginResponse::Accepted,
            Response::PluginProjection(projection) => PluginResponse::Projection {
                projection: Box::new(projection),
            },
            Response::PluginCapabilities(capabilities) => PluginResponse::Capabilities {
                capabilities: Box::new(capabilities),
            },
            _ => {
                return Err(CoreError::InvalidCommand(
                    "core response is not a plugin response".into(),
                ))
            }
        };
        Ok(Self {
            protocol_version: PLUGIN_PROTOCOL_SCHEMA_VERSION,
            session_id,
            request_id: response.request_id,
            response: plugin_response,
        })
    }
}

impl PluginEventEnvelope {
    pub fn from_core(session_id: String, sequence: u64, event: Event) -> Result<Self, CoreError> {
        let plugin_event = match event {
            Event::PluginDisabled {
                extension_id,
                reason,
            } => PluginEvent::Disabled {
                extension_id: ExtensionId(extension_id),
                reason,
            },
            Event::PluginDiagnostic { diagnostic } => PluginEvent::Diagnostic { diagnostic },
            _ => {
                return Err(CoreError::InvalidCommand(
                    "core event is not a plugin event".into(),
                ))
            }
        };
        Ok(Self {
            protocol_version: PLUGIN_PROTOCOL_SCHEMA_VERSION,
            session_id,
            sequence,
            event: plugin_event,
        })
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginStorageRecoveryStatus {
    Available,
    Empty,
    Corrupt,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginStorageRecoveryDto {
    pub extension_id: ExtensionId,
    pub status: PluginStorageRecoveryStatus,
    pub current_generation: Option<u64>,
    pub previous_generation: Option<u64>,
    pub schema_version: Option<u16>,
    pub previous_schema_version: Option<u16>,
    pub quarantine_generation: Option<u64>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginRuntimeRecoveryState {
    Quarantined,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginRuntimeRecoveryDto {
    pub extension_id: ExtensionId,
    pub runtime_nonce: String,
    pub state: PluginRuntimeRecoveryState,
    pub started_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartupPhase {
    Preflight,
    RecoveringState,
    CoreReady,
    WorkbenchReady,
    OptionalActivation,
    Ready,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryMode {
    Starting,
    Safe,
    Degraded,
    Ready,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryFailureClass {
    AbnormalExit,
    StartupTimeout,
    CoreMigration,
    StateCorrupt,
    JournalUnavailable,
    OptionalComponent,
    PlatformInitialization,
    ReleaseRollback,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryAction {
    Retry,
    DisableRecentComponent,
    RestoreSnapshot,
    QuarantineState,
    ResetOptionalState,
    ExportDiagnostics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryTarget {
    pub owner_id: String,
    pub scope: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryFailure {
    pub class: RecoveryFailureClass,
    pub phase: StartupPhase,
    pub message: String,
    pub at_ms: u64,
    #[serde(default)]
    pub target: Option<RecoveryTarget>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryCheckpoint {
    pub attempt_id: u64,
    pub phase: StartupPhase,
    pub at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DurabilityMode {
    AtomicRenameOnly,
    RequireFsync,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DurabilityReport {
    pub schema_version: u16,
    pub mode: DurabilityMode,
    pub atomic_replace: bool,
    pub file_sync: bool,
    pub directory_sync: bool,
    pub crash_durable: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryProjection {
    pub schema_version: u16,
    pub mode: RecoveryMode,
    pub phase: StartupPhase,
    pub attempt_id: u64,
    pub consecutive_failures: u32,
    pub last_failure: Option<RecoveryFailure>,
    pub last_good: Option<RecoveryCheckpoint>,
    pub actions: Vec<RecoveryAction>,
    #[serde(default)]
    pub platform_preflight: Vec<PlatformPreflightEvidence>,
    #[serde(default)]
    pub durability_report: Option<DurabilityReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<velvet_job_system::StorageAvailability>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryDiagnosticBundle {
    pub schema_version: u16,
    pub exported_at_ms: u64,
    pub projection: RecoveryProjection,
    #[serde(default)]
    pub plugin_diagnostics: Vec<PluginDiagnosticEvent>,
    #[serde(default)]
    pub diagnostics_degraded: bool,
    #[serde(default)]
    pub crash_evidence: Vec<CrashCaptureEvidence>,
}

/// Combined lifecycle and extension diagnostic export. The legacy name is
/// retained in the wire contract while the bundle now carries bounded plugin
/// and Job evidence as well as the recovery projection.
pub type DiagnosticBundle = RecoveryDiagnosticBundle;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlatformCapability {
    Webview,
    Gpu,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlatformCapabilityStatus {
    Available,
    Degraded,
    Unavailable,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlatformPreflightEvidence {
    pub capability: PlatformCapability,
    pub status: PlatformCapabilityStatus,
    pub detail: String,
    pub checked_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrashCaptureKind {
    ProcessExit,
    Panic,
    Signal,
    Exception,
    Oom,
    Runtime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrashCaptureEvidence {
    pub schema_version: u16,
    pub correlation_id: String,
    pub process_id: u32,
    #[serde(default)]
    pub parent_process_id: Option<u32>,
    pub attempt_id: u64,
    pub component: String,
    pub kind: CrashCaptureKind,
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub runtime_nonce: Option<String>,
    #[serde(default)]
    pub log_tail: Option<String>,
    #[serde(default)]
    pub artifact: Option<CrashArtifactAttachment>,
    pub occurred_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrashArtifactAttachment {
    pub artifact_id: String,
    pub format: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub collected_at_ms: u64,
    #[serde(default)]
    pub metadata: Option<CrashArtifactMetadata>,
}

/// Bounded, path-free fields parsed from a platform crash artifact header.
/// The raw dump/report never crosses the transport boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrashArtifactMetadata {
    #[serde(default)]
    pub process_name: Option<String>,
    #[serde(default)]
    pub process_id: Option<u32>,
    #[serde(default)]
    pub exception_type: Option<String>,
    #[serde(default)]
    pub signal: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginDiagnosticKind {
    Activation,
    Runtime,
    Invocation,
    Job,
    Storage,
    Migration,
    Shutdown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginDiagnosticEvent {
    pub schema_version: u16,
    pub sequence: u64,
    pub extension_id: String,
    pub kind: PluginDiagnosticKind,
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub attempt: Option<u32>,
    pub retryable: bool,
    pub occurred_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginNotificationSeverity {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginLogRecordDto {
    pub schema_version: u16,
    pub sequence: u64,
    pub extension_id: String,
    pub level: PluginLogLevel,
    pub category: String,
    pub message: String,
    pub session_id: Option<String>,
    pub job_id: Option<String>,
    pub operation_id: Option<String>,
    pub retryable: bool,
    pub occurred_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginNotificationRecordDto {
    pub schema_version: u16,
    pub sequence: u64,
    pub extension_id: String,
    pub severity: PluginNotificationSeverity,
    pub title: String,
    pub message: String,
    pub session_id: Option<String>,
    pub job_id: Option<String>,
    pub operation_id: Option<String>,
    pub retryable: bool,
    pub occurred_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryLaunchReceipt {
    pub attempt_id: u64,
    pub app_version: String,
    pub phase: StartupPhase,
    pub started_at_ms: u64,
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub process_id: Option<u32>,
    #[serde(default)]
    pub phase_started_at_ms: Option<u64>,
    #[serde(default)]
    pub deadline_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryJournalState {
    pub schema_version: u16,
    pub next_attempt_id: u64,
    pub consecutive_failures: u32,
    pub safe_mode_forced: bool,
    pub active_launch: Option<RecoveryLaunchReceipt>,
    pub last_failure: Option<RecoveryFailure>,
    pub last_good: Option<RecoveryCheckpoint>,
    pub shutdown_intent_at_ms: Option<u64>,
    pub shutdown_complete_at_ms: Option<u64>,
    #[serde(default)]
    pub platform_preflight: Vec<PlatformPreflightEvidence>,
    #[serde(default)]
    pub crash_evidence: Vec<CrashCaptureEvidence>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseChannel {
    Stable,
    Preview,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleaseManifestDto {
    pub schema_version: u16,
    pub channel: ReleaseChannel,
    pub version: String,
    pub target: String,
    pub artifact_sha256: String,
    pub updater_schema: u16,
    pub min_data_schema: u16,
    pub max_data_schema: u16,
    pub min_client_version: String,
    pub release_sequence: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub signing_key_id: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseTransactionState {
    Staged,
    Committed,
    RolledBack,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleaseTransactionDto {
    pub schema_version: u16,
    pub transaction_id: String,
    pub manifest: ReleaseManifestDto,
    pub staged_package_id: String,
    pub package_generation: String,
    pub base_data_generation: Option<u64>,
    pub committed_data_generation: Option<u64>,
    pub state: ReleaseTransactionState,
    pub health_checkpoint: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleaseStatusDto {
    pub schema_version: u16,
    pub committed_sequence: u64,
    pub committed_package_generation: String,
    pub pending: Option<ReleaseTransactionDto>,
    pub last_transaction: Option<ReleaseTransactionDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationKind {
    Delete,
    Overwrite,
    Move,
    BatchJob,
    Remote,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationRetryPolicy {
    ProviderDeclaredIdempotent,
    ReconcileBeforeRetry,
    Never,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationUndoCapability {
    None,
    ProviderToken,
    BoundedBackup,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationTarget {
    pub target_id: String,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub expected_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationPlan {
    pub schema_version: u16,
    pub plan_id: String,
    pub operation_id: String,
    pub owner_id: String,
    pub kind: OperationKind,
    pub targets: Vec<OperationTarget>,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub retry_policy: OperationRetryPolicy,
    pub undo: OperationUndoCapability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationPlanError {
    UnsupportedSchema { schema_version: u16 },
    InvalidField(&'static str),
    EmptyTargets,
    TooManyTargets,
    DuplicateTarget(String),
    MissingTargetEvidence(String),
    Expired,
    StaleTarget(String),
}

impl OperationPlan {
    pub const MAX_TARGETS: usize = 512;
    pub const MAX_ID_BYTES: usize = 128;
    pub const MAX_FINGERPRINT_BYTES: usize = 256;

    pub fn validate(&self, now_ms: u64) -> Result<(), OperationPlanError> {
        self.validate_internal(now_ms, true)
    }

    /// Validates the immutable plan envelope before the provider has observed
    /// current target evidence. The host must replace missing evidence with a
    /// provider-owned observation before allowing commit.
    pub fn validate_for_preview(&self, now_ms: u64) -> Result<(), OperationPlanError> {
        self.validate_internal(now_ms, false)
    }

    fn validate_internal(
        &self,
        now_ms: u64,
        require_target_evidence: bool,
    ) -> Result<(), OperationPlanError> {
        if self.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(OperationPlanError::UnsupportedSchema {
                schema_version: self.schema_version,
            });
        }
        for (name, value) in [
            ("planId", self.plan_id.as_str()),
            ("operationId", self.operation_id.as_str()),
            ("ownerId", self.owner_id.as_str()),
        ] {
            if value.is_empty() || value.len() > Self::MAX_ID_BYTES || value.contains('\0') {
                return Err(OperationPlanError::InvalidField(name));
            }
        }
        if self.targets.is_empty() {
            return Err(OperationPlanError::EmptyTargets);
        }
        if self.targets.len() > Self::MAX_TARGETS {
            return Err(OperationPlanError::TooManyTargets);
        }
        if self.expires_at_ms <= self.created_at_ms || now_ms >= self.expires_at_ms {
            return Err(OperationPlanError::Expired);
        }
        let mut seen = std::collections::BTreeSet::new();
        for target in &self.targets {
            if target.target_id.is_empty()
                || target.target_id.len() > Self::MAX_ID_BYTES
                || target.target_id.contains('\0')
            {
                return Err(OperationPlanError::InvalidField("targetId"));
            }
            if !seen.insert(target.target_id.clone()) {
                return Err(OperationPlanError::DuplicateTarget(
                    target.target_id.clone(),
                ));
            }
            if require_target_evidence
                && target.expected_revision.is_none()
                && target.expected_fingerprint.is_none()
            {
                return Err(OperationPlanError::MissingTargetEvidence(
                    target.target_id.clone(),
                ));
            }
            if target
                .expected_fingerprint
                .as_ref()
                .is_some_and(|fingerprint| {
                    fingerprint.is_empty()
                        || fingerprint.len() > Self::MAX_FINGERPRINT_BYTES
                        || fingerprint.contains('\0')
                })
            {
                return Err(OperationPlanError::InvalidField("expectedFingerprint"));
            }
        }
        Ok(())
    }

    pub fn revalidate(
        &self,
        now_ms: u64,
        observed_targets: &[OperationTarget],
    ) -> Result<(), OperationPlanError> {
        self.validate(now_ms)?;
        if observed_targets.len() != self.targets.len() {
            return Err(OperationPlanError::StaleTarget("target-set".into()));
        }
        let observed = observed_targets
            .iter()
            .map(|target| (target.target_id.as_str(), target))
            .collect::<std::collections::BTreeMap<_, _>>();
        for expected in &self.targets {
            let Some(actual) = observed.get(expected.target_id.as_str()) else {
                return Err(OperationPlanError::StaleTarget(expected.target_id.clone()));
            };
            if actual.expected_revision != expected.expected_revision
                || actual.expected_fingerprint != expected.expected_fingerprint
            {
                return Err(OperationPlanError::StaleTarget(expected.target_id.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationReceiptStatus {
    Started,
    Committed,
    Partial,
    RolledBack,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationItemReceipt {
    pub target_id: String,
    pub status: OperationReceiptStatus,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationReceipt {
    pub schema_version: u16,
    pub plan_id: String,
    pub operation_id: String,
    pub status: OperationReceiptStatus,
    pub reconciled: bool,
    pub recorded_at_ms: u64,
    pub items: Vec<OperationItemReceipt>,
    #[serde(default)]
    pub undo_token: Option<String>,
}

impl OperationReceipt {
    pub fn retry_requires_reconciliation(&self) -> bool {
        matches!(
            self.status,
            OperationReceiptStatus::Started | OperationReceiptStatus::Unknown
        ) && !self.reconciled
    }
}

pub const PORTABLE_BUNDLE_MAX_SECTIONS: usize = 64;
pub const PORTABLE_BUNDLE_MAX_SECTION_BYTES: usize = 2 * 1024 * 1024;
pub const PORTABLE_BUNDLE_MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
pub const IMPORT_PLAN_MAX_ENTRIES: usize = PORTABLE_BUNDLE_MAX_SECTIONS;
pub const ASSET_BUNDLE_MAX_RECORDS: usize = 4096;
pub const ASSET_BUNDLE_MAX_FIELD_BYTES: usize = 256;

/// Host-local source availability. An imported `Available` value is only a
/// statement about the exporting machine; the asset participant normalizes it
/// before it becomes local state because a portable bundle never carries a
/// source path, root or volume identity.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAvailability {
    Available,
    Degraded,
    PermissionDenied,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableAssetRecord {
    pub asset_id: String,
    pub content_kind: String,
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub source_availability: SourceAvailability,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableAssetMetadata {
    pub schema_version: u16,
    pub assets: Vec<PortableAssetRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortableAssetMetadataError {
    UnsupportedSchema { schema_version: u16 },
    TooManyRecords,
    DuplicateAsset(String),
    InvalidField(&'static str),
}

impl PortableAssetMetadata {
    pub fn validate(&self) -> Result<(), PortableAssetMetadataError> {
        if self.schema_version != ASSET_BUNDLE_SCHEMA_VERSION {
            return Err(PortableAssetMetadataError::UnsupportedSchema {
                schema_version: self.schema_version,
            });
        }
        if self.assets.len() > ASSET_BUNDLE_MAX_RECORDS {
            return Err(PortableAssetMetadataError::TooManyRecords);
        }
        let mut asset_ids = std::collections::BTreeSet::new();
        for asset in &self.assets {
            validate_asset_bundle_field(&asset.asset_id, "assetId")?;
            validate_asset_bundle_field(&asset.content_kind, "contentKind")?;
            if !asset_ids.insert(asset.asset_id.clone()) {
                return Err(PortableAssetMetadataError::DuplicateAsset(
                    asset.asset_id.clone(),
                ));
            }
            if let Some(fingerprint) = asset.fingerprint.as_deref() {
                validate_asset_bundle_field(fingerprint, "fingerprint")?;
            }
        }
        Ok(())
    }
}

fn validate_asset_bundle_field(
    value: &str,
    field: &'static str,
) -> Result<(), PortableAssetMetadataError> {
    if value.is_empty() || value.len() > ASSET_BUNDLE_MAX_FIELD_BYTES || value.contains('\0') {
        return Err(PortableAssetMetadataError::InvalidField(field));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PortableBundleScope {
    Profile,
    Workspace,
    Asset,
    Extension,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableBundleSection {
    pub section_id: String,
    pub scope: PortableBundleScope,
    pub owner_id: String,
    pub schema_version: u16,
    pub required: bool,
    #[serde(default)]
    pub extension_id: Option<String>,
    pub sha256: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub created_at_ms: u64,
    pub source_build: String,
    pub sections: Vec<PortableBundleSection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortableBundleError {
    UnsupportedSchema { schema_version: u16 },
    InvalidField(&'static str),
    EmptySections,
    TooManySections,
    DuplicateSection(String),
    OversizedSection(String),
    OversizedBundle,
    InvalidExtensionSection(String),
    InvalidDigest(String),
}

impl PortableBundle {
    pub const MAX_ID_BYTES: usize = 128;
    pub const MAX_BUILD_BYTES: usize = 128;

    pub fn validate(&self) -> Result<(), PortableBundleError> {
        if self.schema_version != PORTABLE_BUNDLE_SCHEMA_VERSION {
            return Err(PortableBundleError::UnsupportedSchema {
                schema_version: self.schema_version,
            });
        }
        validate_bundle_identifier(&self.bundle_id, "bundleId")?;
        if self.source_build.is_empty()
            || self.source_build.len() > Self::MAX_BUILD_BYTES
            || self.source_build.contains('\0')
            || self.source_build.contains('/')
            || self.source_build.contains('\\')
        {
            return Err(PortableBundleError::InvalidField("sourceBuild"));
        }
        if self.sections.is_empty() {
            return Err(PortableBundleError::EmptySections);
        }
        if self.sections.len() > PORTABLE_BUNDLE_MAX_SECTIONS {
            return Err(PortableBundleError::TooManySections);
        }
        let mut section_ids = std::collections::BTreeSet::new();
        let mut total_bytes = 0u64;
        for section in &self.sections {
            validate_bundle_identifier(&section.section_id, "sectionId")?;
            validate_bundle_identifier(&section.owner_id, "ownerId")?;
            if !section_ids.insert(section.section_id.clone()) {
                return Err(PortableBundleError::DuplicateSection(
                    section.section_id.clone(),
                ));
            }
            if section.payload.len() > PORTABLE_BUNDLE_MAX_SECTION_BYTES {
                return Err(PortableBundleError::OversizedSection(
                    section.section_id.clone(),
                ));
            }
            total_bytes = total_bytes.saturating_add(section.payload.len() as u64);
            if total_bytes > PORTABLE_BUNDLE_MAX_TOTAL_BYTES {
                return Err(PortableBundleError::OversizedBundle);
            }
            if section.sha256.len() != 64
                || !section.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(PortableBundleError::InvalidDigest(
                    section.section_id.clone(),
                ));
            }
            match section.scope {
                PortableBundleScope::Extension
                    if section.extension_id.is_none()
                        || section.required
                        || section.owner_id
                            != section.extension_id.as_deref().unwrap_or_default() =>
                {
                    return Err(PortableBundleError::InvalidExtensionSection(
                        section.section_id.clone(),
                    ));
                }
                PortableBundleScope::Extension => {}
                _ if section.extension_id.is_some() => {
                    return Err(PortableBundleError::InvalidExtensionSection(
                        section.section_id.clone(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn total_payload_bytes(&self) -> u64 {
        self.sections
            .iter()
            .map(|section| section.payload.len() as u64)
            .sum()
    }
}

fn validate_bundle_identifier(value: &str, field: &'static str) -> Result<(), PortableBundleError> {
    if value.is_empty()
        || value.len() > PortableBundle::MAX_ID_BYTES
        || value.contains('\0')
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(PortableBundleError::InvalidField(field));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportMigration {
    pub section_id: String,
    pub from_schema_version: u16,
    pub to_schema_version: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportConflict {
    pub section_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportPlan {
    pub schema_version: u16,
    pub plan_id: String,
    pub bundle_id: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub selected_section_ids: Vec<String>,
    pub migrations: Vec<ImportMigration>,
    pub conflicts: Vec<ImportConflict>,
    pub unavailable_sources: Vec<String>,
    pub skipped_extension_sections: Vec<String>,
    pub estimated_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportPlanError {
    UnsupportedSchema { schema_version: u16 },
    InvalidField(&'static str),
    EmptySelection,
    TooManyEntries,
    DuplicateSection(String),
    Expired,
}

impl ImportPlan {
    pub fn validate(&self, now_ms: u64) -> Result<(), ImportPlanError> {
        if self.schema_version != IMPORT_PLAN_SCHEMA_VERSION {
            return Err(ImportPlanError::UnsupportedSchema {
                schema_version: self.schema_version,
            });
        }
        for (field, value) in [
            ("planId", self.plan_id.as_str()),
            ("bundleId", self.bundle_id.as_str()),
        ] {
            if value.is_empty()
                || value.len() > PortableBundle::MAX_ID_BYTES
                || value.contains('\0')
            {
                return Err(ImportPlanError::InvalidField(field));
            }
        }
        if self.selected_section_ids.is_empty() {
            return Err(ImportPlanError::EmptySelection);
        }
        if self.selected_section_ids.len() > IMPORT_PLAN_MAX_ENTRIES
            || self.migrations.len() > IMPORT_PLAN_MAX_ENTRIES
            || self.conflicts.len() > IMPORT_PLAN_MAX_ENTRIES
            || self.unavailable_sources.len() > IMPORT_PLAN_MAX_ENTRIES
            || self.skipped_extension_sections.len() > IMPORT_PLAN_MAX_ENTRIES
        {
            return Err(ImportPlanError::TooManyEntries);
        }
        if self.expires_at_ms <= self.created_at_ms || now_ms >= self.expires_at_ms {
            return Err(ImportPlanError::Expired);
        }
        let mut section_ids = std::collections::BTreeSet::new();
        for section_id in &self.selected_section_ids {
            validate_plan_identifier(section_id, "selectedSectionId")?;
            if !section_ids.insert(section_id.clone()) {
                return Err(ImportPlanError::DuplicateSection(section_id.clone()));
            }
        }
        for migration in &self.migrations {
            validate_plan_identifier(&migration.section_id, "migrationSectionId")?;
        }
        for conflict in &self.conflicts {
            validate_plan_identifier(&conflict.section_id, "conflictSectionId")?;
            if conflict.reason.is_empty()
                || conflict.reason.len() > 4096
                || conflict.reason.contains('\0')
            {
                return Err(ImportPlanError::InvalidField("conflictReason"));
            }
        }
        for source in &self.unavailable_sources {
            validate_plan_identifier(source, "unavailableSource")?;
        }
        for section_id in &self.skipped_extension_sections {
            validate_plan_identifier(section_id, "skippedExtensionSection")?;
        }
        Ok(())
    }
}

fn validate_plan_identifier(value: &str, field: &'static str) -> Result<(), ImportPlanError> {
    if value.is_empty()
        || value.len() > PortableBundle::MAX_ID_BYTES
        || value.contains('\0')
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(ImportPlanError::InvalidField(field));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportReceiptStatus {
    Committed,
    Partial,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportOwnerStatus {
    Committed,
    Skipped,
    Quarantined,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportOwnerReceipt {
    pub owner_id: String,
    pub scope: String,
    pub status: ImportOwnerStatus,
    #[serde(default)]
    pub generation: Option<u64>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportReceipt {
    pub schema_version: u16,
    pub plan_id: String,
    pub bundle_id: String,
    pub status: ImportReceiptStatus,
    pub recorded_at_ms: u64,
    #[serde(default)]
    pub recovery_set_generation: Option<u64>,
    pub owners: Vec<ImportOwnerReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportReceiptError {
    UnsupportedSchema { schema_version: u16 },
    InvalidField(&'static str),
    TooManyOwners,
    DuplicateOwner(String),
}

pub const IMPORT_QUARANTINE_MAX_REASON_BYTES: usize = 4096;

/// An inert extension section retained after an import could not safely
/// activate or commit it. The payload is never executable and is only exposed
/// through an explicit host recovery path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportQuarantineEntry {
    pub schema_version: u16,
    pub bundle_id: String,
    pub plan_id: String,
    pub section_id: String,
    pub owner_id: String,
    pub section_schema_version: u16,
    pub quarantined_at_ms: u64,
    pub reason: String,
    pub payload: Vec<u8>,
}

/// Host-facing metadata for a retained extension section. The raw payload is
/// intentionally omitted from transport projections and remains native-only.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportQuarantineSummary {
    pub schema_version: u16,
    pub bundle_id: String,
    pub plan_id: String,
    pub section_id: String,
    pub owner_id: String,
    pub section_schema_version: u16,
    pub quarantined_at_ms: u64,
    pub reason: String,
    pub payload_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportQuarantineError {
    UnsupportedSchema { schema_version: u16 },
    InvalidField(&'static str),
    OversizedPayload,
}

impl ImportQuarantineEntry {
    pub fn validate(&self) -> Result<(), ImportQuarantineError> {
        if self.schema_version != IMPORT_QUARANTINE_SCHEMA_VERSION {
            return Err(ImportQuarantineError::UnsupportedSchema {
                schema_version: self.schema_version,
            });
        }
        for (field, value) in [
            ("bundleId", self.bundle_id.as_str()),
            ("planId", self.plan_id.as_str()),
            ("sectionId", self.section_id.as_str()),
            ("ownerId", self.owner_id.as_str()),
        ] {
            if value.is_empty()
                || value.len() > PortableBundle::MAX_ID_BYTES
                || value.contains('\0')
                || value.contains('/')
                || value.contains('\\')
            {
                return Err(ImportQuarantineError::InvalidField(field));
            }
        }
        if self.reason.is_empty()
            || self.reason.len() > IMPORT_QUARANTINE_MAX_REASON_BYTES
            || self.reason.contains('\0')
        {
            return Err(ImportQuarantineError::InvalidField("reason"));
        }
        if self.payload.len() > PORTABLE_BUNDLE_MAX_SECTION_BYTES {
            return Err(ImportQuarantineError::OversizedPayload);
        }
        Ok(())
    }

    pub fn summary(&self) -> ImportQuarantineSummary {
        ImportQuarantineSummary {
            schema_version: self.schema_version,
            bundle_id: self.bundle_id.clone(),
            plan_id: self.plan_id.clone(),
            section_id: self.section_id.clone(),
            owner_id: self.owner_id.clone(),
            section_schema_version: self.section_schema_version,
            quarantined_at_ms: self.quarantined_at_ms,
            reason: self.reason.clone(),
            payload_bytes: self.payload.len() as u64,
        }
    }
}

impl ImportReceipt {
    pub fn validate(&self) -> Result<(), ImportReceiptError> {
        if self.schema_version != IMPORT_RECEIPT_SCHEMA_VERSION {
            return Err(ImportReceiptError::UnsupportedSchema {
                schema_version: self.schema_version,
            });
        }
        for (field, value) in [
            ("planId", self.plan_id.as_str()),
            ("bundleId", self.bundle_id.as_str()),
        ] {
            if value.is_empty()
                || value.len() > PortableBundle::MAX_ID_BYTES
                || value.contains('\0')
                || value.contains('/')
                || value.contains('\\')
            {
                return Err(ImportReceiptError::InvalidField(field));
            }
        }
        if self.owners.len() > IMPORT_PLAN_MAX_ENTRIES {
            return Err(ImportReceiptError::TooManyOwners);
        }
        let mut owners = std::collections::BTreeSet::new();
        for owner in &self.owners {
            if owner.owner_id.is_empty()
                || owner.owner_id.len() > PortableBundle::MAX_ID_BYTES
                || owner.owner_id.contains('\0')
                || owner.owner_id.contains('/')
                || owner.owner_id.contains('\\')
                || owner.scope.is_empty()
                || owner.scope.len() > PortableBundle::MAX_ID_BYTES
                || owner.scope.contains('\0')
                || owner.scope.contains('/')
                || owner.scope.contains('\\')
            {
                return Err(ImportReceiptError::InvalidField("owner"));
            }
            let key = format!("{}:{}", owner.owner_id, owner.scope);
            if !owners.insert(key.clone()) {
                return Err(ImportReceiptError::DuplicateOwner(key));
            }
            if owner.generation == Some(0) {
                return Err(ImportReceiptError::InvalidField("generation"));
            }
            if owner
                .message
                .as_ref()
                .is_some_and(|message| message.is_empty() || message.len() > 4096)
            {
                return Err(ImportReceiptError::InvalidField("message"));
            }
        }
        Ok(())
    }
}

impl Default for RecoveryJournalState {
    fn default() -> Self {
        Self {
            schema_version: RECOVERY_SCHEMA_VERSION,
            next_attempt_id: 1,
            consecutive_failures: 0,
            safe_mode_forced: false,
            active_launch: None,
            last_failure: None,
            last_good: None,
            shutdown_intent_at_ms: None,
            shutdown_complete_at_ms: None,
            platform_preflight: Vec::new(),
            crash_evidence: Vec::new(),
        }
    }
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
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
    GrantPluginCapabilities {
        extension_id: ExtensionId,
        capabilities: Vec<Capability>,
        expected_generation: Option<u64>,
    },
    RevokePluginCapabilities {
        extension_id: ExtensionId,
        capabilities: Vec<Capability>,
        expected_generation: Option<u64>,
    },
    InstallPlugin {
        source_dir: String,
    },
    InstallPluginArchive {
        source_archive: String,
    },
    InspectPlugin {
        source_dir: String,
    },
    InspectPluginArchive {
        source_archive: String,
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
    ExportRecoveryDiagnostics {
        destination: String,
    },
    RestoreRecoverySet,
    QuarantineRecoveryFailure,
    RestorePluginState {
        extension_id: ExtensionId,
    },
    QuarantinePluginState {
        extension_id: ExtensionId,
        generation: u64,
    },
    CloseQuarantinedRuntimeLease {
        extension_id: ExtensionId,
        runtime_nonce: String,
    },
    DiscardImportQuarantine {
        bundle_id: String,
        plan_id: String,
        section_id: String,
    },
    CommitAssetDelete {
        plan: OperationPlan,
    },
    PreviewAssetDelete {
        plan: OperationPlan,
    },
    UninstallPlugin {
        extension_id: ExtensionId,
    },
    RegisterPreviewAsset {
        asset_id: AssetId,
        source_path: String,
        content_kind: String,
        approved_root: Option<String>,
    },
    AddApprovedSourceRoot {
        root: String,
    },
    RemoveApprovedSourceRoot {
        root: String,
    },
    RemovePreviewAsset {
        asset_id: AssetId,
    },
    InvokePluginCommand {
        extension_id: ExtensionId,
        command_id: String,
        args: Value,
        context: Value,
        context_revision: u64,
        deadline_ms: Option<u64>,
        idempotency_key: Option<String>,
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
        policy: JobExecutionPolicy,
        budget: JobBudgetRequest,
    },
    CancelJob {
        job_id: JobId,
    },
    RetryJob {
        job_id: JobId,
    },
    SetJobExecutionEnvironment {
        environment: JobExecutionEnvironment,
    },
    SetLocale {
        locale: String,
        expected_revision: Option<u64>,
    },
    SetPluginDeveloperMode {
        enabled: bool,
        expected_revision: Option<u64>,
    },
    StageRelease {
        transaction_id: String,
        manifest: Box<ReleaseManifestDto>,
        staged_package_id: String,
        package_generation: String,
    },
    CommitReleaseHealth {
        transaction_id: String,
        checkpoint: String,
    },
    RollbackRelease {
        transaction_id: String,
        reason: String,
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
    Recovery,
    PluginLocalization {
        extension_id: ExtensionId,
        locale: String,
    },
    Extensions,
    PluginProjection {
        extension_id: ExtensionId,
    },
    PluginRegistry {
        extension_id: Option<ExtensionId>,
    },
    PluginCapabilities {
        extension_id: ExtensionId,
    },
    PluginCapabilityAudit,
    PluginLogs {
        extension_id: Option<ExtensionId>,
    },
    PluginNotifications {
        extension_id: Option<ExtensionId>,
    },
    PluginCommandSurface {
        extension_id: Option<ExtensionId>,
    },
    ResolvePreview {
        asset_id: AssetId,
        content_kind: String,
        intent: PreviewIntent,
        surface: PreviewSurface,
    },
    ResolvePreviewAsset {
        asset_id: AssetId,
        intent: PreviewIntent,
        surface: PreviewSurface,
    },
    ApprovedSourceRoots,
    PluginInstallRecoveries,
    PluginInstallRecoveryLog,
    PluginStorageRecovery {
        extension_id: ExtensionId,
    },
    PluginRuntimeRecovery,
    ImportQuarantine,
    OperationReceipts,
    Jobs,
    Snapshot,
    RequestOutcome {
        request_id: RequestId,
    },
    /// Adapter-owned outcome lookup for string idempotency keys. Core-only
    /// transports may return `Unknown` until they provide the same cache.
    PluginCommandOutcome {
        extension_id: ExtensionId,
        command_id: String,
        idempotency_key: String,
    },
    ReleaseStatus,
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
    Recovery(RecoveryProjection),
    PluginResource(PluginResourceDto),
    Extensions(Vec<ExtensionSummary>),
    Extension(ExtensionSummary),
    PluginInstallPreview(PluginInstallPreviewDto),
    PluginInstallRecoveries(Vec<ExtensionInstallRecovery>),
    PluginInstallRecoveryLog(Vec<ExtensionInstallRecoveryLogEntry>),
    PluginStorageRecovery(PluginStorageRecoveryDto),
    PluginRuntimeRecovery(Vec<PluginRuntimeRecoveryDto>),
    ImportQuarantine(Vec<ImportQuarantineSummary>),
    OperationReceipt(OperationReceipt),
    OperationReceipts(Vec<OperationReceipt>),
    OperationPlan(OperationPlan),
    PluginProjection(PluginProjectionDto),
    PluginRegistry(PluginRegistrySnapshotDto),
    PluginCapabilities(PluginCapabilityGrantDto),
    PluginCapabilityAudit(Vec<PluginCapabilityAuditDto>),
    PluginLogs(Vec<PluginLogRecordDto>),
    PluginNotifications(Vec<PluginNotificationRecordDto>),
    PluginCommandSurface(PluginCommandSurfaceSnapshotDto),
    Job(JobSnapshot),
    Jobs(Vec<JobSnapshot>),
    Preview(ResolvedPreview),
    ApprovedSourceRoots(Vec<String>),
    Snapshot(AuthoritativeSnapshot),
    RequestOutcome(RequestOutcome),
    ReleaseStatus(ReleaseStatusDto),
    Accepted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestOutcomeStatus {
    Unknown,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestOutcome {
    pub request_id: RequestId,
    pub status: RequestOutcomeStatus,
}

/// Commands whose successful result may be safely returned again when the
/// same request id is received within one transport session.
pub fn is_idempotent_request(request: &Request) -> bool {
    request_idempotency_scope(request).is_some()
}

pub fn request_idempotency_scope(request: &Request) -> Option<&'static str> {
    match request {
        Request::Command(Command::LayoutAt { .. }) => Some("layout-at"),
        Request::Command(Command::CancelJob { .. }) => Some("cancel-job"),
        Request::Command(Command::CloseQuarantinedRuntimeLease { .. }) => {
            Some("close-quarantined-runtime-lease")
        }
        Request::Command(Command::DiscardImportQuarantine { .. }) => {
            Some("discard-import-quarantine")
        }
        Request::Command(Command::SetLocale {
            expected_revision: Some(_),
            ..
        }) => Some("set-locale-at-revision"),
        Request::Command(Command::SetPluginDeveloperMode {
            expected_revision: Some(_),
            ..
        }) => Some("set-plugin-developer-mode-at-revision"),
        Request::Command(Command::CommitReleaseHealth { .. }) => Some("commit-release-health"),
        Request::Command(Command::InvokePluginCommand {
            idempotency_key: Some(_),
            ..
        }) => Some("plugin-command"),
        Request::Command(Command::RollbackRelease { .. }) => Some("rollback-release"),
        Request::Command(_) | Request::Query(_) => None,
    }
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
    RecoveryChanged {
        projection: RecoveryProjection,
    },
    PluginDisabled {
        extension_id: String,
        reason: String,
    },
    PluginDiagnostic {
        diagnostic: PluginDiagnosticEvent,
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
    ProtocolMajorMismatch {
        expected: u16,
        received: u16,
    },
    SessionExpired {
        session_id: String,
    },
    RequestIdReuse {
        request_id: u64,
    },
    InvalidCommand(String),
    Layout(String),
    Plugin(String),
    PluginCommand {
        extension_id: String,
        command_id: String,
        code: String,
        retryable: bool,
    },
    PluginCapabilityDenied {
        extension_id: String,
        capability: Capability,
        reason: PluginCapabilityDenyReason,
        retryable: bool,
    },
    PluginCapabilityGenerationConflict {
        extension_id: String,
        expected: u64,
        actual: u64,
    },
    PluginRuntimeFailed {
        extension_id: String,
        message: String,
    },
    Job(String),
    JobEnvironment {
        schema_version: u16,
    },
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    Profile(String),
    Recovery(String),
    Release(String),
    Storage {
        availability: velvet_job_system::StorageAvailability,
    },
    Operation(OperationError),
    PluginResourceNotFound {
        extension_id: String,
        locale: String,
    },
    PluginResource {
        extension_id: String,
        locale: String,
        message: String,
    },
    AssetSource {
        asset_id: String,
        message: String,
        retryable: bool,
    },
    AssetSourceRoot {
        message: String,
        retryable: bool,
    },
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

    /// Dispatches the narrow plugin contract without exposing the general
    /// ApplicationCore request set to a plugin caller.
    fn dispatch_plugin(
        &self,
        envelope: PluginRequestEnvelope,
    ) -> Result<PluginResponseEnvelope, CoreError> {
        envelope.validate()?;
        let session_id = envelope.session_id.clone();
        let response = self.dispatch_envelope(envelope.into_request_envelope())?;
        PluginResponseEnvelope::from_core(session_id, response)
    }

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
#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
