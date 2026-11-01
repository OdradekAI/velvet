use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use velvet_asset_core::AssetId;
use velvet_job_system::{
    JobBudgetRequest, JobExecutionEnvironment, JobExecutionPolicy, JobId, JobSnapshot,
};
use velvet_layout_model::{LayoutCommand, LayoutState};
use velvet_plugin_api::{
    AppTarget, ContributionDescriptor, ExtensionId, ExtensionInstallPreview,
    ExtensionInstallRecovery, ExtensionInstallRecoveryAction, ExtensionInstallRecoveryLogEntry,
    ExtensionKind, ExtensionManifest, ExtensionStorageDeclaration, ExtensionSummary,
    ExtensionSupport, LocalizationManifest, PreviewActionContext, RuntimeKind, RuntimeLaunchSpec,
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
pub const CRASH_CAPTURE_SCHEMA_VERSION: u16 = 1;
pub const CORE_SNAPSHOT_SCHEMA_VERSION: u16 = 1;
pub const ACTIVATION_SCHEMA_VERSION: u16 = 1;

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
        ("profile".into(), PROFILE_SCHEMA_VERSION),
        ("recovery".into(), RECOVERY_SCHEMA_VERSION),
        (
            "recovery-diagnostic".into(),
            RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
        ),
        ("plugin-resource".into(), PLUGIN_RESOURCE_SCHEMA_VERSION),
        ("plugin-diagnostic".into(), PLUGIN_DIAGNOSTIC_SCHEMA_VERSION),
        ("crash-capture".into(), CRASH_CAPTURE_SCHEMA_VERSION),
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
    Jobs,
    Snapshot,
    RequestOutcome {
        request_id: RequestId,
    },
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
    PluginProjection(PluginProjectionDto),
    Job(JobSnapshot),
    Jobs(Vec<JobSnapshot>),
    Preview(ResolvedPreview),
    ApprovedSourceRoots(Vec<String>),
    Snapshot(AuthoritativeSnapshot),
    RequestOutcome(RequestOutcome),
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
        Request::Command(Command::SetLocale {
            expected_revision: Some(_),
            ..
        }) => Some("set-locale-at-revision"),
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
    Recovery(String),
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
            storage: Some(ExtensionStorageDeclaration {
                schema_version: 1,
                quota_bytes: 4096,
            }),
            ..ExtensionManifest::default()
        };

        let dto = PluginManifestDto::from(manifest.clone());
        let json = serde_json::to_value(&dto).expect("protocol DTO should serialize");
        assert_eq!(json["manifestSchema"], 1);
        assert_eq!(json["bridgeApi"], "1");
        assert_eq!(json["activationEvents"][0]["kind"], "onWorkbenchReady");
        assert_eq!(json["storage"]["schemaVersion"], 1);
        assert_eq!(json["storage"]["quotaBytes"], 4096);

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

    #[test]
    fn plugin_resource_dto_round_trips_with_bounded_payload_shape() {
        let resource = PluginResourceDto {
            schema_version: PLUGIN_RESOURCE_SCHEMA_VERSION,
            extension_id: ExtensionId("sample.localized".into()),
            locale: "zh-Hans".into(),
            content_type: "application/json".into(),
            bytes: "{\"ready\":\"就绪\"}".as_bytes().to_vec(),
        };

        let json = serde_json::to_value(&resource).expect("resource should serialize");
        assert_eq!(json["schemaVersion"], PLUGIN_RESOURCE_SCHEMA_VERSION);
        assert_eq!(json["extensionId"], "sample.localized");
        assert_eq!(json["locale"], "zh-Hans");
        assert_eq!(json["contentType"], "application/json");
        assert!(json["bytes"].is_array());
        let decoded: PluginResourceDto =
            serde_json::from_value(json).expect("resource should deserialize");
        assert_eq!(decoded, resource);
    }

    #[test]
    fn recovery_projection_uses_stable_wire_names() {
        let projection = RecoveryProjection {
            schema_version: RECOVERY_SCHEMA_VERSION,
            mode: RecoveryMode::Safe,
            phase: StartupPhase::Preflight,
            attempt_id: 2,
            consecutive_failures: 2,
            last_failure: Some(RecoveryFailure {
                class: RecoveryFailureClass::AbnormalExit,
                phase: StartupPhase::Preflight,
                message: "previous launch did not complete".into(),
                at_ms: 42,
                target: None,
            }),
            last_good: None,
            actions: vec![RecoveryAction::Retry, RecoveryAction::RestoreSnapshot],
            platform_preflight: Vec::new(),
            durability_report: Some(DurabilityReport {
                schema_version: 1,
                mode: DurabilityMode::RequireFsync,
                atomic_replace: true,
                file_sync: true,
                directory_sync: true,
                crash_durable: true,
                detail: "fsync".into(),
            }),
        };
        let json = serde_json::to_value(&projection).expect("recovery should serialize");
        assert_eq!(json["schemaVersion"], RECOVERY_SCHEMA_VERSION);
        assert_eq!(json["mode"], "safe");
        assert_eq!(json["phase"], "preflight");
        assert_eq!(json["lastFailure"]["class"], "abnormal-exit");
        assert_eq!(json["actions"][1], "restore-snapshot");
        assert_eq!(json["durabilityReport"]["mode"], "require-fsync");
        let decoded: RecoveryProjection =
            serde_json::from_value(json).expect("recovery should deserialize");
        assert_eq!(decoded, projection);
    }

    #[test]
    fn recovery_diagnostic_bundle_uses_stable_wire_names() {
        let bundle = RecoveryDiagnosticBundle {
            schema_version: RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
            exported_at_ms: 42,
            projection: RecoveryProjection {
                schema_version: RECOVERY_SCHEMA_VERSION,
                mode: RecoveryMode::Degraded,
                phase: StartupPhase::OptionalActivation,
                attempt_id: 2,
                consecutive_failures: 0,
                last_failure: None,
                last_good: None,
                actions: vec![RecoveryAction::Retry, RecoveryAction::ExportDiagnostics],
                platform_preflight: Vec::new(),
                durability_report: None,
            },
            plugin_diagnostics: Vec::new(),
            diagnostics_degraded: false,
            crash_evidence: Vec::new(),
        };
        let json = serde_json::to_value(&bundle).expect("diagnostic bundle should serialize");
        assert_eq!(json["schemaVersion"], RECOVERY_DIAGNOSTIC_SCHEMA_VERSION);
        assert_eq!(json["exportedAtMs"], 42);
        assert_eq!(json["projection"]["mode"], "degraded");
        assert_eq!(json["projection"]["actions"][1], "export-diagnostics");
        assert_eq!(json["pluginDiagnostics"], serde_json::json!([]));
        assert_eq!(json["diagnosticsDegraded"], false);
        assert_eq!(json["crashEvidence"], serde_json::json!([]));

        let legacy = serde_json::json!({
            "schemaVersion": RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
            "exportedAtMs": 7,
            "projection": bundle.projection.clone(),
        });
        let decoded: RecoveryDiagnosticBundle =
            serde_json::from_value(legacy).expect("legacy diagnostic bundle should decode");
        assert!(decoded.plugin_diagnostics.is_empty());
        assert!(!decoded.diagnostics_degraded);
    }

    #[test]
    fn crash_artifact_attachment_round_trips_without_a_path() {
        let evidence = CrashCaptureEvidence {
            schema_version: CRASH_CAPTURE_SCHEMA_VERSION,
            correlation_id: "launch-1".into(),
            process_id: 7,
            parent_process_id: None,
            attempt_id: 2,
            component: "core".into(),
            kind: CrashCaptureKind::Exception,
            code: "system-crash-artifact".into(),
            message: "artifact attached".into(),
            runtime_nonce: None,
            log_tail: None,
            artifact: Some(CrashArtifactAttachment {
                artifact_id: "0123456789abcdef01234567".into(),
                format: "dmp".into(),
                size_bytes: 42,
                sha256: "a".repeat(64),
                collected_at_ms: 9,
                metadata: Some(CrashArtifactMetadata {
                    process_name: Some("Velvet".into()),
                    process_id: Some(7),
                    exception_type: Some("EXC_CRASH".into()),
                    signal: Some("SIGABRT".into()),
                    reason: Some("abort".into()),
                }),
            }),
            occurred_at_ms: 10,
        };
        let json = serde_json::to_value(&evidence).expect("crash evidence should serialize");
        assert_eq!(json["artifact"]["format"], "dmp");
        assert_eq!(json["artifact"]["metadata"]["signal"], "SIGABRT");
        assert!(json.get("path").is_none());
        let decoded: CrashCaptureEvidence =
            serde_json::from_value(json).expect("crash evidence should deserialize");
        assert_eq!(decoded, evidence);
    }

    #[test]
    fn plugin_diagnostic_event_uses_a_bounded_typed_wire_shape() {
        let event = PluginDiagnosticEvent {
            schema_version: PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
            sequence: 4,
            extension_id: "sample.reader".into(),
            kind: PluginDiagnosticKind::Job,
            code: "job-failed".into(),
            message: "runtime exited".into(),
            job_id: Some("job-000001".into()),
            attempt: Some(2),
            retryable: true,
            occurred_at_ms: 12,
        };
        let json = serde_json::to_value(&event).expect("plugin diagnostic should serialize");
        assert_eq!(json["schemaVersion"], PLUGIN_DIAGNOSTIC_SCHEMA_VERSION);
        assert_eq!(json["kind"], "job");
        assert_eq!(json["jobId"], "job-000001");
        let decoded: PluginDiagnosticEvent =
            serde_json::from_value(json).expect("plugin diagnostic should deserialize");
        assert_eq!(decoded, event);
    }

    #[test]
    fn recovery_quarantine_target_uses_stable_wire_names() {
        let failure = RecoveryFailure {
            class: RecoveryFailureClass::CoreMigration,
            phase: StartupPhase::RecoveringState,
            message: "migration failed".into(),
            at_ms: 9,
            target: Some(RecoveryTarget {
                owner_id: "layout".into(),
                scope: "workspace".into(),
                generation: 3,
            }),
        };
        let json = serde_json::to_value(&failure).expect("recovery failure should serialize");
        assert_eq!(json["target"]["ownerId"], "layout");
        assert_eq!(json["target"]["generation"], 3);
        assert_eq!(
            serde_json::to_value(RecoveryAction::QuarantineState)
                .expect("quarantine action should serialize"),
            "quarantine-state"
        );
    }

    #[test]
    fn job_governance_contract_round_trips_policy_checkpoint_and_environment() {
        let policy = JobExecutionPolicy {
            deadline_ms: Some(5_000),
            cancellation_grace_ms: 250,
            retry: velvet_job_system::JobRetryPolicy {
                max_attempts: 4,
                initial_backoff_ms: 50,
                max_backoff_ms: 500,
            },
            network: velvet_job_system::JobNetworkPolicy::Unmetered,
            background: true,
        };
        let request = Request::Command(Command::CreateJob {
            kind: "index".into(),
            duration_ms: 1_000,
            will_fail: false,
            input: Value::Null,
            policy: policy.clone(),
            budget: JobBudgetRequest::default(),
        });
        assert_eq!(
            match request {
                Request::Command(Command::CreateJob { policy: actual, .. }) => actual,
                _ => unreachable!(),
            },
            policy
        );
        let environment = JobExecutionEnvironment {
            suspended: true,
            network: velvet_job_system::JobNetworkState::Unknown,
        };
        let command = Command::SetJobExecutionEnvironment { environment };
        assert_eq!(command, Command::SetJobExecutionEnvironment { environment });
        let checkpoint = velvet_job_system::JobCheckpoint {
            schema_version: velvet_job_system::JOB_CHECKPOINT_SCHEMA_VERSION,
            payload: serde_json::json!({ "cursor": 4 }),
        };
        let json = serde_json::to_value(&checkpoint).expect("checkpoint should serialize");
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["payload"]["cursor"], 4);
    }

    #[test]
    fn protocol_handshake_negotiates_minor_features_and_schema_versions() {
        let response = negotiate_protocol(
            ProtocolHandshakeRequest {
                client_id: "desktop-test".into(),
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR + 1,
                features: vec![TRANSPORT_FEATURE_SESSION_V1.into(), "future-feature".into()],
                schema_versions: BTreeMap::from([
                    ("profile".into(), PROFILE_SCHEMA_VERSION + 1),
                    ("future".into(), 1),
                ]),
            },
            "session-test".into(),
        )
        .expect("compatible protocol should negotiate");

        assert_eq!(response.protocol_major, PROTOCOL_MAJOR);
        assert_eq!(response.protocol_minor, PROTOCOL_MINOR);
        assert_eq!(
            response.selected_features,
            vec![TRANSPORT_FEATURE_SESSION_V1]
        );
        assert_eq!(response.schema_versions["profile"], PROFILE_SCHEMA_VERSION);
        assert!(!response.schema_versions.contains_key("future"));
        assert_eq!(response.session_id, "session-test");
    }

    #[test]
    fn protocol_handshake_rejects_a_major_mismatch_before_any_command() {
        assert_eq!(
            negotiate_protocol(
                ProtocolHandshakeRequest {
                    client_id: "desktop-test".into(),
                    protocol_major: PROTOCOL_MAJOR + 1,
                    protocol_minor: 0,
                    features: Vec::new(),
                    schema_versions: BTreeMap::new(),
                },
                "session-test".into(),
            ),
            Err(CoreError::ProtocolMajorMismatch {
                expected: PROTOCOL_MAJOR,
                received: PROTOCOL_MAJOR + 1,
            })
        );
    }

    #[test]
    fn request_outcome_contract_marks_only_declared_commands_idempotent() {
        assert!(is_idempotent_request(&Request::Command(
            Command::CancelJob {
                job_id: JobId("job-1".into()),
            }
        )));
        assert!(!is_idempotent_request(&Request::Command(
            Command::RetryJob {
                job_id: JobId("job-1".into()),
            }
        )));
        assert!(!is_idempotent_request(&Request::Command(Command::Layout(
            LayoutCommand::ResetLayout,
        ))));
    }
}
