use std::collections::BTreeMap;
use std::fs;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

mod crash_capture;
mod instance;
#[allow(dead_code)]
mod capability_policy;
#[allow(dead_code)]
mod launcher_failure;
mod native_recovery_window;
mod platform_probe;
mod release_policy;
mod update_coordinator;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, ShortcutState};
use velvet_application_core::{ApplicationCore, ApplicationRecoveryStores, CrashCaptureAdapter};
use velvet_application_core::RELEASE_HEALTH_CHECKPOINT;
use velvet_job_system::{
    JobBudgetRequest, JobExecutionEnvironment, JobExecutionPolicy, JobId, JobProvenance,
    JobResult, JobSnapshot, JobStatus,
};
use velvet_layout_model::{
    LayoutCommand, LayoutState, ViewContainerState, ViewState, WorkbenchAppId, WorkbenchPart,
};
use velvet_plugin_api::{
    ActivationEvent, AppTarget, ContributionDescriptor, ExtensionId, ExtensionInstallRecoveryAction,
    ExtensionKind, ExtensionManifest, ExtensionSupport, LocalizationManifest, RuntimeKind,
    PreviewActionContext, RuntimeLaunchSpec, VersionRange, WorkbenchAppDescriptor,
    ZonePermissionSet,
    validate_reserved_metadata,
};
use velvet_plugin_host::JsonFileRuntimeLeaseStore;
use velvet_persistence::{
    DurabilityMode, JsonFileAssetSourceLocationStore, JsonFileDiagnosticEventStore,
    DurabilityReportStore, JsonFileDurabilityReportStore, JsonFileRecoveryJournalStore,
    JsonFileImportQuarantineStore, JsonFileOperationReceiptStore, JsonFileRecoverySetStore,
    JsonFileStateSnapshotStore,
    LegacyJobMigrationParticipant,
    LegacyLayoutMigrationParticipant, LegacyProfileMigrationParticipant,
    JsonFileReleasePackageStore, JsonFileReleaseUpdateStore, OwnerMigrationParticipant,
    SnapshotJobStateStore,
    SnapshotLayoutStateStore, SnapshotProfileStateStore, migrate_legacy_owner_snapshots,
};
use velvet_preview_core::{PreviewIntent, PreviewRenderer, PreviewSurface, ResolvedPreview};
use velvet_tauri_adapter::{
    platform_signals::PlatformSignalController, CoreEventSink, TauriTransport,
};
use velvet_workbench_protocol::{
    ActivationRequest, ActivationTarget, ActivationTargetKind, Command, CoreError, CoreTransport,
    CrashCaptureEvidence, CrashCaptureKind, Event,
    PlatformCapabilityStatus,
    PlatformPreflightEvidence, PluginDisableReason, RecoveryFailureClass, Request,
    ProtocolHandshakeRequest, ProtocolHandshakeResponse, RequestId, Response, ResponseEnvelope,
    StartupPhase,
    PROTOCOL_VERSION,
};
use update_coordinator::{NativeUpdateStatus, UpdateCoordinator, UpdateCoordinatorError};

use crash_capture::{
    arm_process_receipt, complete_process_receipt, update_process_receipt,
    FileCrashCaptureAdapter,
};
use launcher_failure::consume_launcher_failure;
use native_recovery_window::{
    consume_native_recovery_intent, launch_native_recovery_window, NativeRecoveryAction,
    NativeRecoveryPage, NativeRecoveryPageKind, NativeRecoveryRequest, NATIVE_RECOVERY_SCHEMA_VERSION,
};
use instance::{normalize_activation_request, AcquireOutcome, ProfileInstanceCoordinator};
use platform_probe::{HostPlatformPreflightProbe, PlatformPreflightProbe};

const CORE_EVENT_NAME: &str = "velvet_core_event";
const KEYBINDING_EVENT_NAME: &str = "velvet_keybinding";
const ACTIVATION_EVENT_NAME: &str = "velvet_activation_request";
const MAX_PENDING_ACTIVATIONS: usize = 32;
const TAURI_STARTUP_FAILURE_SCHEMA_VERSION: u16 = 1;
const TAURI_STARTUP_FAILURE_FILE: &str = "tauri-startup-failure.json";
const MAX_TAURI_STARTUP_FAILURE_BYTES: usize = 8 * 1024;

pub struct AppState {
    pub transport: Mutex<TauriTransport>,
    pub update_coordinator: UpdateCoordinator,
    pub platform_signals: PlatformSignalController,
    pub instance: Option<Mutex<ProfileInstanceCoordinator>>,
    pub pending_activation: Arc<Mutex<Vec<ActivationRequest>>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct NativeRecoveryState {
    locale: String,
    safe_mode: bool,
    core_recovery_target: Option<(String, String, u64)>,
    orphan_count: usize,
    corrupt_extension_states: Vec<(String, u64)>,
    quarantined_runtime_leases: Vec<(String, String)>,
    platform_preflight: Vec<PlatformPreflightEvidence>,
    crash_evidence_count: usize,
}

impl NativeRecoveryState {
    fn requires_surface(&self) -> bool {
        self.safe_mode
            || self.core_recovery_target.is_some()
            || self.orphan_count > 0
            || !self.corrupt_extension_states.is_empty()
            || !self.quarantined_runtime_leases.is_empty()
            || self
                .platform_preflight
                .iter()
                .any(|evidence| {
                    evidence.capability == velvet_workbench_protocol::PlatformCapability::Webview
                        && evidence.status == PlatformCapabilityStatus::Unavailable
                })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeHostLocale {
    English,
    SimplifiedChinese,
}

impl NativeHostLocale {
    fn from_profile_locale(locale: &str) -> Self {
        let normalized = locale.replace('_', "-").to_ascii_lowercase();
        if normalized == "zh"
            || normalized.starts_with("zh-")
                && (normalized.contains("hans")
                    || normalized.contains("cn")
                    || normalized.contains("sg"))
        {
            Self::SimplifiedChinese
        } else {
            Self::English
        }
    }

    fn recovery_title(self) -> &'static str {
        match self {
            Self::English => "Velvet recovery",
            Self::SimplifiedChinese => "Velvet 恢复",
        }
    }

    fn extension_repair_title(self) -> &'static str {
        match self {
            Self::English => "Extension storage repair",
            Self::SimplifiedChinese => "扩展存储修复",
        }
    }

    fn safe_mode_message(self) -> &'static str {
        match self {
            Self::English => "Velvet started in Safe Mode; core data was left unchanged.",
            Self::SimplifiedChinese => "Velvet 已进入安全模式；核心数据未被修改。",
        }
    }

    fn orphan_message(self, count: usize) -> String {
        match self {
            Self::English => format!(
                "{count} orphaned plugin runtime lease(s) were processed; uncertain identities were not killed."
            ),
            Self::SimplifiedChinese => format!(
                "已处理 {count} 个孤立插件运行时租约；身份不确定的进程未被终止。"
            ),
        }
    }

    fn corrupt_message(self, count: usize) -> String {
        match self {
            Self::English => format!(
                "{count} extension storage generation(s) are corrupt or from an unknown schema."
            ),
            Self::SimplifiedChinese => format!(
                "有 {count} 个扩展存储代际损坏或使用未知 schema。"
            ),
        }
    }

    fn core_corrupt_message(self, owner: &str, scope: &str, generation: u64) -> String {
        match self {
            Self::English => format!(
                "Core state {owner}:{scope} generation {generation} is corrupt and can be quarantined."
            ),
            Self::SimplifiedChinese => format!(
                "核心状态 {owner}:{scope} 第 {generation} 代已损坏，可以隔离。"
            ),
        }
    }

    fn quarantine_core_button(self) -> &'static str {
        match self {
            Self::English => "Quarantine damaged state",
            Self::SimplifiedChinese => "隔离损坏状态",
        }
    }

    fn quarantine_core_success(self) -> &'static str {
        match self {
            Self::English => "The damaged generation was quarantined and the previous validated state is selected. Restart Velvet to apply it.",
            Self::SimplifiedChinese => "损坏的数据代际已隔离，并已选择上一个经过验证的状态。请重启 Velvet 以应用。",
        }
    }

    fn quarantined_message(self, count: usize) -> String {
        match self {
            Self::English => format!(
                "{count} quarantined runtime lease record(s) remain for review. Closing a record never terminates a process."
            ),
            Self::SimplifiedChinese => format!(
                "仍有 {count} 个已隔离的运行时租约记录等待复核。关闭记录不会终止进程。"
            ),
        }
    }

    fn platform_message(self, evidence: &[PlatformPreflightEvidence]) -> String {
        let unavailable = evidence
            .iter()
            .filter(|item| item.status == PlatformCapabilityStatus::Unavailable)
            .map(|item| format!("{:?}", item.capability))
            .collect::<Vec<_>>()
            .join(", ");
        match self {
            Self::English => format!(
                "Platform preflight reported an unavailable capability: {unavailable}. The standalone recovery process can retry without loading the WebView."
            ),
            Self::SimplifiedChinese => format!(
                "平台启动前探测报告能力不可用：{unavailable}。独立恢复进程可以在不加载 WebView 的情况下重试。"
            ),
        }
    }

    fn crash_evidence_message(self, count: usize) -> String {
        match self {
            Self::English => format!("{count} bounded crash receipt(s) were correlated to prior launch or runtime identities."),
            Self::SimplifiedChinese => format!("已将 {count} 条有界崩溃 receipt 关联到之前的启动或运行时身份。"),
        }
    }

    fn restore_button(self) -> &'static str {
        match self {
            Self::English => "Restore validated state",
            Self::SimplifiedChinese => "恢复已验证状态",
        }
    }

    fn continue_safe_mode_button(self) -> &'static str {
        match self {
            Self::English => "Continue Safe Mode",
            Self::SimplifiedChinese => "继续安全模式",
        }
    }

    fn quarantine_button(self) -> &'static str {
        match self {
            Self::English => "Quarantine damaged state",
            Self::SimplifiedChinese => "隔离损坏状态",
        }
    }

    fn continue_button(self) -> &'static str {
        match self {
            Self::English => "Continue",
            Self::SimplifiedChinese => "继续",
        }
    }

    fn close_lease_button(self) -> &'static str {
        match self {
            Self::English => "Forget lease records",
            Self::SimplifiedChinese => "删除租约记录",
        }
    }

    fn repair_success(self) -> &'static str {
        match self {
            Self::English => "Damaged extension state was quarantined. Restart or explicitly re-enable the extension after inspection.",
            Self::SimplifiedChinese => "损坏的扩展状态已隔离。检查后请重启，或显式重新启用扩展。",
        }
    }

    fn restore_staged(self) -> &'static str {
        match self {
            Self::English => "The last validated recovery set is selected. Restart Velvet to apply it.",
            Self::SimplifiedChinese => "已选择上一个经过验证的恢复集。请重启 Velvet 以应用它。",
        }
    }

    fn restore_error(self, error: &str) -> String {
        match self {
            Self::English => format!("Velvet could not restore the validated state: {error}"),
            Self::SimplifiedChinese => format!("Velvet 无法恢复已验证状态：{error}"),
        }
    }

    fn close_success(self) -> &'static str {
        match self {
            Self::English => "Quarantined lease records were forgotten. No process was terminated.",
            Self::SimplifiedChinese => "已删除隔离的租约记录。没有终止任何进程。",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TauriStartupFailureRecord {
    schema_version: u16,
    phase: String,
    message: String,
    occurred_at_ms: u64,
    #[serde(default)]
    platform_preflight: Vec<PlatformPreflightEvidence>,
}

fn preflight_app_data_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("VELVET_APP_DATA_DIR") {
        return Some(PathBuf::from(path));
    }
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("com.velvet.desktop"));
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join("Library/Application Support/com.velvet.desktop"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|path| path.join(".local/share"))
            })
            .map(|path| path.join("com.velvet.desktop"))
    }
}

fn activation_request_from_args() -> ActivationRequest {
    let targets = std::env::args()
        .skip(1)
        .filter_map(|argument| {
            if argument.is_empty() || argument.starts_with('-') {
                return None;
            }
            if argument.starts_with("velvet://") || argument.starts_with("file://") {
                return Some(ActivationTarget {
                    kind: ActivationTargetKind::DeepLink,
                    value: argument,
                });
            }
            Some(ActivationTarget {
                kind: ActivationTargetKind::FilePath,
                value: argument,
            })
        })
        .take(32)
        .collect();
    let request = ActivationRequest {
        schema_version: velvet_workbench_protocol::ACTIVATION_SCHEMA_VERSION,
        request_id: format!("{}-{}", std::process::id(), now_ms()),
        targets,
    };
    match normalize_activation_request(request) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("Velvet ignored an invalid activation target: {error}");
            ActivationRequest {
                schema_version: velvet_workbench_protocol::ACTIVATION_SCHEMA_VERSION,
                request_id: format!("{}-{}", std::process::id(), now_ms()),
                targets: Vec::new(),
            }
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn bounded_startup_failure_message(error: impl std::fmt::Display) -> String {
    let mut message = error.to_string().replace(['\r', '\n'], " ");
    if message.len() > MAX_TAURI_STARTUP_FAILURE_BYTES {
        message = message
            .chars()
            .take(MAX_TAURI_STARTUP_FAILURE_BYTES)
            .collect();
    }
    message
}

fn record_tauri_startup_failure(phase: &str, error: impl std::fmt::Display) {
    record_tauri_startup_failure_with_evidence(phase, error, Vec::new());
}

fn record_tauri_startup_failure_with_evidence(
    phase: &str,
    error: impl std::fmt::Display,
    platform_preflight: Vec<PlatformPreflightEvidence>,
) {
    let message = bounded_startup_failure_message(error);
    eprintln!("Velvet Tauri initialization failed during {phase}: {message}");
    let Some(root) = preflight_app_data_dir() else {
        return;
    };
    if fs::create_dir_all(&root).is_err() {
        return;
    }
    let record = TauriStartupFailureRecord {
        schema_version: TAURI_STARTUP_FAILURE_SCHEMA_VERSION,
        phase: phase.into(),
        message,
        occurred_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default(),
        platform_preflight,
    };
    let Ok(bytes) = serde_json::to_vec_pretty(&record) else {
        return;
    };
    let path = root.join(TAURI_STARTUP_FAILURE_FILE);
    let temporary = root.join(format!("{TAURI_STARTUP_FAILURE_FILE}.tmp"));
    if fs::write(&temporary, bytes).is_ok() && fs::rename(&temporary, &path).is_err() {
        let _ = fs::remove_file(&path);
        let _ = fs::rename(temporary, path);
    }
}

fn consume_tauri_startup_failure(root: &Path) -> Option<TauriStartupFailureRecord> {
    let path = root.join(TAURI_STARTUP_FAILURE_FILE);
    let metadata = fs::metadata(&path).ok()?;
    if metadata.len() > MAX_TAURI_STARTUP_FAILURE_BYTES as u64 {
        return None;
    }
    let bytes = fs::read(&path).ok()?;
    let record = serde_json::from_slice::<TauriStartupFailureRecord>(&bytes).ok()?;
    if record.schema_version != TAURI_STARTUP_FAILURE_SCHEMA_VERSION {
        return None;
    }
    let _ = fs::remove_file(path);
    Some(record)
}

static NATIVE_RECOVERY_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
struct TauriEventSink<R: Runtime> {
    app: AppHandle<R>,
    sequence: Arc<AtomicU64>,
}

impl<R: Runtime> CoreEventSink for TauriEventSink<R> {
    fn emit(&self, event: Event) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let _ = self
            .app
            .emit(CORE_EVENT_NAME, WireEventEnvelope::with_sequence(event, sequence));
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub expected_revision: Option<u64>,
    pub actual_revision: Option<u64>,
}

impl From<CoreError> for WireError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::ProtocolVersionMismatch { expected, received } => Self {
                code: "protocol-version-mismatch".into(),
                message: format!("Expected protocol {expected}, received {received}"),
                retryable: false,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::ProtocolMajorMismatch { expected, received } => Self {
                code: "protocol-major-mismatch".into(),
                message: format!("Expected protocol major {expected}, received {received}"),
                retryable: false,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::SessionExpired { session_id } => Self {
                code: "session-expired".into(),
                message: format!("Transport session {session_id} is no longer valid"),
                retryable: false,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::RequestIdReuse { request_id } => Self {
                code: "request-id-reuse".into(),
                message: format!("Request id {request_id} was already used for another command"),
                retryable: false,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::StaleRevision { expected, actual } => Self {
                code: "stale-revision".into(),
                message: format!("Expected revision {expected}, received {actual}"),
                retryable: true,
                expected_revision: Some(expected),
                actual_revision: Some(actual),
            },
            CoreError::Profile(message) => Self {
                code: "profile-error".into(),
                message,
                retryable: true,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::Recovery(message) => Self {
                code: "recovery-error".into(),
                message,
                retryable: true,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::Release(message) => Self {
                code: "release-error".into(),
                message,
                retryable: true,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::Operation(message) => Self {
                code: "operation-error".into(),
                message,
                retryable: false,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::PluginResourceNotFound {
                extension_id,
                locale,
            } => Self {
                code: "plugin-resource-not-found".into(),
                message: format!("Localization resource {locale} is not available for {extension_id}"),
                retryable: false,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::PluginResource {
                extension_id,
                locale,
                message,
            } => Self {
                code: "plugin-resource-error".into(),
                message: format!("Localization resource {locale} for {extension_id} failed: {message}"),
                retryable: false,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::AssetSource {
                asset_id,
                message,
                retryable,
            } => Self {
                code: "asset-source-error".into(),
                message: format!("Preview source for asset {asset_id} failed: {message}"),
                retryable,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::AssetSourceRoot { message, retryable } => Self {
                code: "asset-source-root-error".into(),
                message,
                retryable,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::JobEnvironment { schema_version } => Self {
                code: "job-environment-schema-unsupported".into(),
                message: format!(
                    "Job execution environment schema {schema_version} is unsupported"
                ),
                retryable: false,
                expected_revision: None,
                actual_revision: None,
            },
            CoreError::PluginRuntimeFailed {
                extension_id,
                message,
            } => Self {
                code: "plugin-runtime-failed".into(),
                message: format!("Plugin {extension_id} runtime failed: {message}"),
                retryable: true,
                expected_revision: None,
                actual_revision: None,
            },
            other => Self {
                code: "core-error".into(),
                message: format!("{other:?}"),
                retryable: false,
                expected_revision: None,
                actual_revision: None,
            },
        }
    }
}

impl From<UpdateCoordinatorError> for WireError {
    fn from(error: UpdateCoordinatorError) -> Self {
        let (code, retryable) = match &error {
            UpdateCoordinatorError::Disabled(_) => ("updater-disabled", false),
            UpdateCoordinatorError::Updater(_) => ("updater-request-failed", true),
            UpdateCoordinatorError::Manifest(_) => ("updater-manifest-invalid", false),
            UpdateCoordinatorError::Core(_) => ("updater-core-rejected", false),
            UpdateCoordinatorError::Stale(_) => ("updater-stale", true),
            UpdateCoordinatorError::Busy => ("updater-busy", true),
            UpdateCoordinatorError::Install(_) => ("updater-install-failed", true),
        };
        Self {
            code: code.into(),
            message: error.to_string(),
            retryable,
            expected_revision: None,
            actual_revision: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireRequestEnvelope {
    pub protocol_version: u16,
    pub request_id: u64,
    #[serde(default)]
    pub session_id: Option<String>,
    pub request: WireRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WirePluginManifest {
    manifest_schema: u16,
    id: String,
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
    storage: Option<velvet_plugin_api::ExtensionStorageDeclaration>,
    workbench_app: Option<WorkbenchAppDescriptor>,
    contributions: Vec<ContributionDescriptor>,
    permission_sets: Vec<ZonePermissionSet>,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}

impl TryFrom<WirePluginManifest> for ExtensionManifest {
    type Error = String;

    fn try_from(value: WirePluginManifest) -> Result<Self, Self::Error> {
        validate_reserved_metadata(&value.extensions)?;
        Ok(Self {
            manifest_schema: value.manifest_schema,
            id: ExtensionId(value.id),
            version: value.version,
            host_api: value.host_api,
            bridge_api: value.bridge_api,
            runtime_protocol: value.runtime_protocol,
            runtime: value.runtime,
            extension_kind: value.extension_kind,
            launch: value.launch,
            activation_events: value.activation_events,
            extension_dependencies: value.extension_dependencies,
            target_app: value.target_app,
            support: value.support,
            required_capabilities: value.required_capabilities,
            optional_capabilities: value.optional_capabilities,
            localization: value.localization,
            storage: value.storage,
            workbench_app: value.workbench_app,
            contributions: value.contributions,
            permission_sets: value.permission_sets,
            extensions: value.extensions,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WireRequest {
    QueryLayout {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryProfile {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryRecovery {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryReleaseStatus {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    ReadPluginLocalization {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
        locale: String,
    },
    QueryExtensions {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryPluginInstallRecoveries {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryPluginInstallRecoveryLog {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryJobs {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QuerySnapshot {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryRequestOutcome {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "requestId")]
        request_id: u64,
    },
    QueryPluginProjection {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    QueryPluginStorageRecovery {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    QueryPluginRuntimeRecovery {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryImportQuarantine {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryOperationReceipts {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryApprovedSourceRoots {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    RegisterPlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        manifest: Box<WirePluginManifest>,
    },
    ActivatePlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    DisablePlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
        reason: String,
    },
    CheckPluginHealth {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    RecoverPlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    RestorePluginState {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    QuarantinePluginState {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
        generation: u64,
    },
    CloseQuarantinedRuntimeLease {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
        #[serde(rename = "runtimeNonce")]
        runtime_nonce: String,
    },
    DiscardImportQuarantine {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "bundleId")]
        bundle_id: String,
        #[serde(rename = "planId")]
        plan_id: String,
        #[serde(rename = "sectionId")]
        section_id: String,
    },
    CommitAssetDelete {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        plan: velvet_workbench_protocol::OperationPlan,
    },
    InstallPlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "sourceDir")]
        source_dir: String,
    },
    InstallPluginArchive {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "sourceArchive")]
        source_archive: String,
    },
    UninstallPlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    RegisterPreviewAsset {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "assetId")]
        asset_id: String,
        #[serde(rename = "sourcePath")]
        source_path: String,
        #[serde(rename = "contentKind")]
        content_kind: String,
        #[serde(rename = "approvedRoot", default)]
        approved_root: Option<String>,
    },
    AddApprovedSourceRoot {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        root: String,
    },
    RemoveApprovedSourceRoot {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        root: String,
    },
    RemovePreviewAsset {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "assetId")]
        asset_id: String,
    },
    InspectPlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "sourceDir")]
        source_dir: String,
    },
    InspectPluginArchive {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "sourceArchive")]
        source_archive: String,
    },
    CommitPluginInstall {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    RollbackPluginInstall {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    ResolvePluginInstallRecovery {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
        action: ExtensionInstallRecoveryAction,
    },
    ClearPluginInstallRecoveryLog {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    ExportPluginInstallRecoveryLog {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        destination: String,
    },
    ExportRecoveryDiagnostics {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        destination: String,
    },
    RestoreRecoverySet {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QuarantineRecoveryFailure {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    InvokePluginCommand {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
        #[serde(rename = "commandId")]
        command_id: String,
        args: serde_json::Value,
        context: serde_json::Value,
    },
    InvokePreviewAction {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
        #[serde(rename = "commandId")]
        command_id: String,
        args: serde_json::Value,
        context: PreviewActionContext,
    },
    ReleasePreviewSession {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    CreateJob {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "jobKind")]
        job_kind: String,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
        #[serde(rename = "willFail")]
        will_fail: bool,
        #[serde(default)]
        input: Option<serde_json::Value>,
        #[serde(default)]
        policy: Option<JobExecutionPolicy>,
        #[serde(default)]
        budget: Option<JobBudgetRequest>,
    },
    CancelJob {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "jobId")]
        job_id: String,
    },
    RetryJob {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "jobId")]
        job_id: String,
    },
    SetJobExecutionEnvironment {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        environment: JobExecutionEnvironment,
    },
    SetLocale {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        locale: String,
        #[serde(rename = "expectedRevision")]
        expected_revision: Option<u64>,
    },
    StageRelease {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "transactionId")]
        transaction_id: String,
        manifest: velvet_workbench_protocol::ReleaseManifestDto,
        #[serde(rename = "stagedPackageId")]
        staged_package_id: String,
        #[serde(rename = "packageGeneration")]
        package_generation: String,
    },
    CommitReleaseHealth {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "transactionId")]
        transaction_id: String,
        checkpoint: String,
    },
    RollbackRelease {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "transactionId")]
        transaction_id: String,
        reason: String,
    },
    LayoutCommand {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        command: WireLayoutCommand,
        #[serde(rename = "expectedRevision")]
        expected_revision: Option<u64>,
    },
    ResolvePreview {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "assetId")]
        asset_id: String,
        #[serde(rename = "contentKind")]
        content_kind: String,
        intent: String,
        surface: String,
    },
    ResolvePreviewAsset {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "assetId")]
        asset_id: String,
        intent: String,
        surface: String,
    },
}

impl WireRequest {
    fn into_core(self, outer_protocol_version: u16) -> Result<Request, WireError> {
        match self {
            Self::QueryLayout { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::Layout))
            }
            Self::QueryProfile { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::Profile))
            }
            Self::QueryRecovery { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::Recovery))
            }
            Self::QueryReleaseStatus { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::ReleaseStatus))
            }
            Self::ReadPluginLocalization {
                protocol_version,
                extension_id,
                locale,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::PluginLocalization {
                    extension_id: ExtensionId(extension_id),
                    locale,
                }))
            }
            Self::QueryExtensions { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::Extensions))
            }
            Self::QueryPluginInstallRecoveries { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::PluginInstallRecoveries,
                ))
            }
            Self::QueryPluginInstallRecoveryLog { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::PluginInstallRecoveryLog,
                ))
            }
            Self::QueryJobs { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::Jobs))
            }
            Self::QuerySnapshot { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::Snapshot))
            }
            Self::QueryRequestOutcome {
                protocol_version,
                request_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::RequestOutcome {
                    request_id: RequestId(request_id),
                }))
            }
            Self::QueryPluginProjection {
                protocol_version,
                extension_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::PluginProjection {
                        extension_id: ExtensionId(extension_id),
                    },
                ))
            }
            Self::QueryPluginStorageRecovery {
                protocol_version,
                extension_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::PluginStorageRecovery {
                        extension_id: ExtensionId(extension_id),
                    },
                ))
            }
            Self::QueryPluginRuntimeRecovery { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::PluginRuntimeRecovery,
                ))
            }
            Self::QueryImportQuarantine { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::ImportQuarantine,
                ))
            }
            Self::QueryOperationReceipts { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::OperationReceipts,
                ))
            }
            Self::QueryApprovedSourceRoots { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::ApprovedSourceRoots,
                ))
            }
            Self::RegisterPlugin {
                protocol_version,
                manifest,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                let manifest: ExtensionManifest = (*manifest)
                    .try_into()
                    .map_err(|message: String| WireError {
                        code: "invalid-plugin-manifest".into(),
                        message,
                        retryable: false,
                        expected_revision: None,
                        actual_revision: None,
                    })?;
                Ok(Request::Command(Command::RegisterPlugin {
                    manifest: Box::new(manifest.into()),
                }))
            }
            Self::ActivatePlugin {
                protocol_version,
                extension_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::ActivatePlugin {
                    extension_id: ExtensionId(extension_id),
                }))
            }
            Self::DisablePlugin {
                protocol_version,
                extension_id,
                reason,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::DisablePlugin {
                    extension_id: ExtensionId(extension_id),
                    reason: parse_disable_reason(&reason)?,
                }))
            }
            Self::CheckPluginHealth {
                protocol_version,
                extension_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::CheckPluginHealth {
                    extension_id: ExtensionId(extension_id),
                }))
            }
            Self::RecoverPlugin {
                protocol_version,
                extension_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::RecoverPlugin {
                    extension_id: ExtensionId(extension_id),
                }))
            }
            Self::RestorePluginState {
                protocol_version,
                extension_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::RestorePluginState {
                    extension_id: ExtensionId(extension_id),
                }))
            }
            Self::QuarantinePluginState {
                protocol_version,
                extension_id,
                generation,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::QuarantinePluginState {
                    extension_id: ExtensionId(extension_id),
                    generation,
                }))
            }
            Self::CloseQuarantinedRuntimeLease {
                protocol_version,
                extension_id,
                runtime_nonce,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::CloseQuarantinedRuntimeLease {
                    extension_id: ExtensionId(extension_id),
                    runtime_nonce,
                }))
            }
            Self::DiscardImportQuarantine {
                protocol_version,
                bundle_id,
                plan_id,
                section_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::DiscardImportQuarantine {
                    bundle_id,
                    plan_id,
                    section_id,
                }))
            }
            Self::CommitAssetDelete {
                protocol_version,
                plan,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::CommitAssetDelete { plan }))
            }
            Self::InstallPlugin {
                protocol_version,
                source_dir,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::InstallPlugin { source_dir }))
            }
            Self::InstallPluginArchive {
                protocol_version,
                source_archive,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::InstallPluginArchive {
                    source_archive,
                }))
            }
            Self::UninstallPlugin {
                protocol_version,
                extension_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::UninstallPlugin {
                    extension_id: ExtensionId(extension_id),
                }))
            }
            Self::RegisterPreviewAsset {
                protocol_version,
                asset_id,
                source_path,
                content_kind,
                approved_root,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::RegisterPreviewAsset {
                    asset_id: velvet_asset_core::AssetId(asset_id),
                    source_path,
                    content_kind,
                    approved_root,
                }))
            }
            Self::AddApprovedSourceRoot {
                protocol_version,
                root,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::AddApprovedSourceRoot { root }))
            }
            Self::RemoveApprovedSourceRoot {
                protocol_version,
                root,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::RemoveApprovedSourceRoot { root }))
            }
            Self::RemovePreviewAsset {
                protocol_version,
                asset_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::RemovePreviewAsset {
                    asset_id: velvet_asset_core::AssetId(asset_id),
                }))
            }
            Self::InspectPlugin {
                protocol_version,
                source_dir,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::InspectPlugin { source_dir }))
            }
            Self::InspectPluginArchive {
                protocol_version,
                source_archive,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::InspectPluginArchive {
                    source_archive,
                }))
            }
            Self::CommitPluginInstall {
                protocol_version,
                extension_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::CommitPluginInstall {
                    extension_id: ExtensionId(extension_id),
                }))
            }
            Self::RollbackPluginInstall {
                protocol_version,
                extension_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::RollbackPluginInstall {
                    extension_id: ExtensionId(extension_id),
                }))
            }
            Self::ResolvePluginInstallRecovery {
                protocol_version,
                extension_id,
                action,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::ResolvePluginInstallRecovery {
                    extension_id: ExtensionId(extension_id),
                    action,
                }))
            }
            Self::ClearPluginInstallRecoveryLog { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::ClearPluginInstallRecoveryLog))
            }
            Self::ExportPluginInstallRecoveryLog {
                protocol_version,
                destination,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::ExportPluginInstallRecoveryLog { destination }))
            }
            Self::ExportRecoveryDiagnostics {
                protocol_version,
                destination,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::ExportRecoveryDiagnostics { destination }))
            }
            Self::RestoreRecoverySet { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::RestoreRecoverySet))
            }
            Self::QuarantineRecoveryFailure { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::QuarantineRecoveryFailure))
            }
            Self::InvokePluginCommand {
                protocol_version,
                extension_id,
                command_id,
                args,
                context,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::InvokePluginCommand {
                    extension_id: ExtensionId(extension_id),
                    command_id,
                    args,
                    context,
                }))
            }
            Self::InvokePreviewAction {
                protocol_version,
                extension_id,
                command_id,
                args,
                context,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::InvokePreviewAction {
                    extension_id: ExtensionId(extension_id),
                    command_id,
                    args,
                    context,
                }))
            }
            Self::ReleasePreviewSession {
                protocol_version,
                session_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::ReleasePreviewSession { session_id }))
            }
            Self::CreateJob {
                protocol_version,
                job_kind,
                duration_ms,
                will_fail,
                input,
                policy,
                budget,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::CreateJob {
                    kind: job_kind,
                    duration_ms,
                    will_fail,
                    input: input.unwrap_or(serde_json::Value::Null),
                    policy: policy.unwrap_or_default(),
                    budget: budget.unwrap_or_default(),
                }))
            }
            Self::CancelJob {
                protocol_version,
                job_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::CancelJob {
                    job_id: JobId(job_id),
                }))
            }
            Self::RetryJob {
                protocol_version,
                job_id,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::RetryJob {
                    job_id: JobId(job_id),
                }))
            }
            Self::SetJobExecutionEnvironment {
                protocol_version,
                environment,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::SetJobExecutionEnvironment { environment }))
            }
            Self::SetLocale {
                protocol_version,
                locale,
                expected_revision,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::SetLocale {
                    locale,
                    expected_revision,
                }))
            }
            Self::StageRelease {
                protocol_version,
                transaction_id,
                manifest,
                staged_package_id,
                package_generation,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::StageRelease {
                    transaction_id,
                    manifest: Box::new(manifest),
                    staged_package_id,
                    package_generation,
                }))
            }
            Self::CommitReleaseHealth {
                protocol_version,
                transaction_id,
                checkpoint,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::CommitReleaseHealth {
                    transaction_id,
                    checkpoint,
                }))
            }
            Self::RollbackRelease {
                protocol_version,
                transaction_id,
                reason,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::RollbackRelease {
                    transaction_id,
                    reason,
                }))
            }
            Self::LayoutCommand {
                protocol_version,
                command,
                expected_revision,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::LayoutAt {
                    expected_revision,
                    command: command.into_core()?,
                }))
            }
            Self::ResolvePreview {
                protocol_version,
                asset_id,
                content_kind,
                intent,
                surface,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::ResolvePreview {
                        asset_id: velvet_asset_core::AssetId(asset_id),
                        content_kind,
                        intent: parse_preview_intent(&intent)?,
                        surface: parse_preview_surface(&surface)?,
                    },
                ))
            }
            Self::ResolvePreviewAsset {
                protocol_version,
                asset_id,
                intent,
                surface,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(
                    velvet_workbench_protocol::Query::ResolvePreviewAsset {
                        asset_id: velvet_asset_core::AssetId(asset_id),
                        intent: parse_preview_intent(&intent)?,
                        surface: parse_preview_surface(&surface)?,
                    },
                ))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "id")]
pub enum WireLayoutCommand {
    #[serde(rename = "workbench.activate-app")]
    ActivateApp {
        #[serde(rename = "appId")]
        app_id: String,
    },
    #[serde(rename = "workbench.set-part-visibility")]
    SetPartVisibility { part: String, visible: bool },
    #[serde(rename = "workbench.toggle-part")]
    TogglePart { part: String },
    #[serde(rename = "workbench.resize-part")]
    ResizePart { part: String, size: u32 },
    #[serde(rename = "workbench.collapse-part")]
    CollapsePart { part: String, collapsed: bool },
    #[serde(rename = "workbench.upsert-view-container")]
    UpsertViewContainer {
        #[serde(rename = "containerId")]
        container_id: String,
        visible: bool,
        order: u32,
    },
    #[serde(rename = "workbench.upsert-view")]
    UpsertView {
        #[serde(rename = "viewId")]
        view_id: String,
        #[serde(rename = "containerId")]
        container_id: String,
        visible: bool,
        order: u32,
    },
    #[serde(rename = "workbench.set-view-container-visibility")]
    SetViewContainerVisibility {
        #[serde(rename = "containerId")]
        container_id: String,
        visible: bool,
    },
    #[serde(rename = "workbench.move-view")]
    MoveView {
        #[serde(rename = "viewId")]
        view_id: String,
        #[serde(rename = "targetContainerId")]
        target_container_id: String,
        anchor: Option<WireOrderAnchor>,
    },
    #[serde(rename = "workbench.reorder-view-container")]
    ReorderViewContainer {
        #[serde(rename = "containerId")]
        container_id: String,
        anchor: Option<WireOrderAnchor>,
    },
    #[serde(rename = "workbench.remove-view-container")]
    RemoveViewContainer {
        #[serde(rename = "containerId")]
        container_id: String,
    },
    #[serde(rename = "workbench.set-view-visibility")]
    SetViewVisibility {
        #[serde(rename = "viewId")]
        view_id: String,
        visible: bool,
    },
    #[serde(rename = "workbench.move-view-before")]
    MoveViewBefore {
        #[serde(rename = "viewId")]
        view_id: String,
        #[serde(rename = "beforeViewId")]
        before_view_id: String,
    },
    #[serde(rename = "workbench.reset-layout")]
    ResetLayout,
    #[serde(rename = "workbench.reset-view-layout")]
    ResetViews,
}

#[derive(Debug, Deserialize)]
pub struct WireOrderAnchor {
    pub position: String,
    pub id: String,
}

impl WireLayoutCommand {
    fn into_core(self) -> Result<LayoutCommand, WireError> {
        match self {
            Self::ActivateApp { app_id } => Ok(LayoutCommand::ActivateApp(parse_app(&app_id)?)),
            Self::SetPartVisibility { part, visible } => Ok(LayoutCommand::SetPartVisibility {
                part: parse_part(&part)?,
                visible,
            }),
            Self::TogglePart { part } => Ok(LayoutCommand::TogglePartVisibility {
                part: parse_part(&part)?,
            }),
            Self::ResizePart { part, size } => Ok(LayoutCommand::ResizePart {
                part: parse_part(&part)?,
                size,
            }),
            Self::CollapsePart { part, collapsed } => Ok(LayoutCommand::SetPartCollapsed {
                part: parse_part(&part)?,
                collapsed,
            }),
            Self::UpsertViewContainer {
                container_id,
                visible,
                order,
            } => Ok(LayoutCommand::UpsertViewContainer {
                container_id,
                visible,
                order,
            }),
            Self::UpsertView {
                view_id,
                container_id,
                visible,
                order,
            } => Ok(LayoutCommand::UpsertView {
                view_id,
                container_id,
                visible,
                order,
            }),
            Self::SetViewContainerVisibility {
                container_id,
                visible,
            } => Ok(LayoutCommand::SetViewContainerVisibility {
                container_id,
                visible,
            }),
            Self::MoveView {
                view_id,
                target_container_id,
                anchor,
            } => {
                let (before_view_id, after_view_id) = view_anchor(anchor)?;
                Ok(LayoutCommand::MoveView {
                    view_id,
                    target_container_id,
                    before_view_id,
                    after_view_id,
                })
            }
            Self::ReorderViewContainer {
                container_id,
                anchor,
            } => {
                let (before_container_id, after_container_id) = view_anchor(anchor)?;
                Ok(LayoutCommand::ReorderViewContainer {
                    container_id,
                    before_container_id,
                    after_container_id,
                })
            }
            Self::RemoveViewContainer { container_id } => {
                Ok(LayoutCommand::RemoveViewContainer { container_id })
            }
            Self::SetViewVisibility { view_id, visible } => {
                Ok(LayoutCommand::SetViewVisibility { view_id, visible })
            }
            Self::MoveViewBefore {
                view_id,
                before_view_id,
            } => Ok(LayoutCommand::ReorderView {
                view_id,
                before_view_id: Some(before_view_id),
                after_view_id: None,
            }),
            Self::ResetLayout => Ok(LayoutCommand::ResetLayout),
            Self::ResetViews => Ok(LayoutCommand::ResetViews),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireResponseEnvelope {
    protocol_version: u16,
    request_id: u64,
    response: WireResponse,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum WireResponse {
    Layout {
        state: WireLayoutState,
    },
    Profile {
        state: velvet_workbench_protocol::ProfileState,
    },
    Recovery {
        projection: velvet_workbench_protocol::RecoveryProjection,
    },
    ReleaseStatus {
        status: velvet_workbench_protocol::ReleaseStatusDto,
    },
    PluginResource {
        resource: velvet_workbench_protocol::PluginResourceDto,
    },
    Extensions {
        extensions: Vec<WireExtensionSummary>,
    },
    Extension {
        extension: WireExtensionSummary,
    },
    PluginInstallPreview {
        preview: velvet_workbench_protocol::PluginInstallPreviewDto,
    },
    PluginInstallRecoveries {
        recoveries: Vec<velvet_plugin_api::ExtensionInstallRecovery>,
    },
    PluginInstallRecoveryLog {
        log: Vec<velvet_plugin_api::ExtensionInstallRecoveryLogEntry>,
    },
    PluginProjection {
        projection: velvet_workbench_protocol::PluginProjectionDto,
    },
    PluginStorageRecovery {
        recovery: velvet_workbench_protocol::PluginStorageRecoveryDto,
    },
    PluginRuntimeRecovery {
        recoveries: Vec<velvet_workbench_protocol::PluginRuntimeRecoveryDto>,
    },
    ImportQuarantine {
        entries: Vec<velvet_workbench_protocol::ImportQuarantineSummary>,
    },
    OperationReceipt {
        receipt: velvet_workbench_protocol::OperationReceipt,
    },
    OperationReceipts {
        receipts: Vec<velvet_workbench_protocol::OperationReceipt>,
    },
    Job {
        job: WireJobSnapshot,
    },
    Jobs {
        jobs: Vec<WireJobSnapshot>,
    },
    Snapshot {
        snapshot: WireAuthoritativeSnapshot,
    },
    RequestOutcome {
        outcome: WireRequestOutcome,
    },
    Preview {
        preview: WireResolvedPreview,
    },
    ApprovedSourceRoots {
        roots: Vec<String>,
    },
    Accepted,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireRequestOutcome {
    request_id: u64,
    status: &'static str,
}

impl WireResponseEnvelope {
    fn from_core(response: velvet_workbench_protocol::ResponseEnvelope) -> Self {
        let protocol_version = response.protocol_version;
        let request_id = response.request_id.0;
        let wire_response = match response.response {
            Response::Layout(state) => WireResponse::Layout {
                state: WireLayoutState::from(state),
            },
            Response::Profile(state) => WireResponse::Profile { state },
            Response::Recovery(projection) => WireResponse::Recovery { projection },
            Response::ReleaseStatus(status) => WireResponse::ReleaseStatus { status },
            Response::PluginResource(resource) => WireResponse::PluginResource { resource },
            Response::Extensions(extensions) => WireResponse::Extensions {
                extensions: extensions
                    .into_iter()
                    .map(WireExtensionSummary::from)
                    .collect(),
            },
            Response::Extension(extension) => WireResponse::Extension {
                extension: WireExtensionSummary::from(extension),
            },
            Response::PluginInstallPreview(preview) => WireResponse::PluginInstallPreview { preview },
            Response::PluginInstallRecoveries(recoveries) => {
                WireResponse::PluginInstallRecoveries { recoveries }
            }
            Response::PluginInstallRecoveryLog(log) => WireResponse::PluginInstallRecoveryLog { log },
            Response::PluginProjection(projection) => WireResponse::PluginProjection { projection },
            Response::PluginStorageRecovery(recovery) => {
                WireResponse::PluginStorageRecovery { recovery }
            }
            Response::PluginRuntimeRecovery(recoveries) => {
                WireResponse::PluginRuntimeRecovery { recoveries }
            }
            Response::ImportQuarantine(entries) => WireResponse::ImportQuarantine { entries },
            Response::OperationReceipt(receipt) => WireResponse::OperationReceipt { receipt },
            Response::OperationReceipts(receipts) => WireResponse::OperationReceipts { receipts },
            Response::Job(job) => WireResponse::Job {
                job: WireJobSnapshot::from(job),
            },
            Response::Jobs(jobs) => WireResponse::Jobs {
                jobs: jobs.into_iter().map(WireJobSnapshot::from).collect(),
            },
            Response::Snapshot(snapshot) => WireResponse::Snapshot {
                snapshot: WireAuthoritativeSnapshot::from(snapshot),
            },
            Response::RequestOutcome(outcome) => WireResponse::RequestOutcome {
                outcome: WireRequestOutcome {
                    request_id: outcome.request_id.0,
                    status: match outcome.status {
                        velvet_workbench_protocol::RequestOutcomeStatus::Unknown => "unknown",
                        velvet_workbench_protocol::RequestOutcomeStatus::Completed => "completed",
                    },
                },
            },
            Response::Preview(preview) => WireResponse::Preview {
                preview: WireResolvedPreview::from(preview),
            },
            Response::ApprovedSourceRoots(roots) => WireResponse::ApprovedSourceRoots { roots },
            Response::Accepted => WireResponse::Accepted,
        };

        Self {
            protocol_version,
            request_id,
            response: wire_response,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireLayoutState {
    schema_version: u32,
    revision: u64,
    active_app: String,
    parts: BTreeMap<String, WirePartState>,
    containers: BTreeMap<String, WireViewContainerState>,
    views: BTreeMap<String, WireViewState>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WirePartState {
    visible: bool,
    size: u32,
    collapsed: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireViewContainerState {
    visible: bool,
    order: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireViewState {
    visible: bool,
    order: u32,
    container_id: Option<String>,
}

impl From<LayoutState> for WireLayoutState {
    fn from(state: LayoutState) -> Self {
        Self {
            schema_version: state.schema_version,
            revision: state.revision,
            active_app: app_name(&state.active_app),
            parts: state
                .parts
                .into_iter()
                .map(|(part, value)| (part_name(&part), WirePartState::from(value)))
                .collect(),
            containers: state
                .containers
                .into_iter()
                .map(|(id, value)| (id, WireViewContainerState::from(value)))
                .collect(),
            views: state
                .views
                .into_iter()
                .map(|(id, value)| (id, WireViewState::from(value)))
                .collect(),
        }
    }
}

impl From<velvet_layout_model::PartState> for WirePartState {
    fn from(value: velvet_layout_model::PartState) -> Self {
        Self {
            visible: value.visible,
            size: value.size,
            collapsed: value.collapsed,
        }
    }
}

impl From<ViewContainerState> for WireViewContainerState {
    fn from(value: ViewContainerState) -> Self {
        Self {
            visible: value.visible,
            order: value.order,
        }
    }
}

impl From<ViewState> for WireViewState {
    fn from(value: ViewState) -> Self {
        Self {
            visible: value.visible,
            order: value.order,
            container_id: value.container_id,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireExtensionSummary {
    id: String,
    version: String,
    active: bool,
    state: String,
}

impl From<velvet_plugin_api::ExtensionSummary> for WireExtensionSummary {
    fn from(value: velvet_plugin_api::ExtensionSummary) -> Self {
        Self {
            id: value.id.0,
            version: value.version,
            active: value.active,
            state: extension_state_name(&value.state).into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireResolvedPreview {
    candidate: WirePreviewCandidate,
    session: WirePreviewSession,
    diagnostic: Option<String>,
    augmentations: Vec<WireResolvedPreviewAugmentation>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireResolvedPreviewAugmentation {
    augmentation_id: String,
    slot: String,
    session: Option<WirePreviewSession>,
    diagnostic: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WirePreviewCandidate {
    provider_id: String,
    content_kinds: Vec<String>,
    intents: Vec<String>,
    surfaces: Vec<String>,
    priority: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WirePreviewSession {
    provider_id: String,
    context: WirePreviewContext,
    session_id: String,
    renderer: WirePreviewRenderer,
    commands: Vec<String>,
    slots: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum WirePreviewRenderer {
    Generic,
    Text {
        text: String,
        truncated: bool,
    },
    Image {
        #[serde(rename = "mimeType")]
        mime_type: String,
        bytes: Vec<u8>,
        truncated: bool,
    },
    UiBridge {
        entrypoint: String,
        #[serde(rename = "bridgeVersion")]
        bridge_version: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WirePreviewContext {
    asset_id: String,
    content_kind: String,
    intent: String,
    surface: String,
}

impl From<ResolvedPreview> for WireResolvedPreview {
    fn from(value: ResolvedPreview) -> Self {
        Self {
            candidate: WirePreviewCandidate {
                provider_id: value.candidate.provider_id,
                content_kinds: value
                    .candidate
                    .content_kinds
                    .into_iter()
                    .map(|kind| kind.0)
                    .collect(),
                intents: value
                    .candidate
                    .intents
                    .into_iter()
                    .map(|intent| format!("{intent:?}").to_lowercase())
                    .collect(),
                surfaces: value
                    .candidate
                    .surfaces
                    .into_iter()
                    .map(|surface| format!("{surface:?}").to_lowercase())
                    .collect(),
                priority: value.candidate.priority,
            },
            session: value.session.into(),
            diagnostic: value.diagnostic,
            augmentations: value
                .augmentations
                .into_iter()
                .map(|augmentation| WireResolvedPreviewAugmentation {
                    augmentation_id: augmentation.augmentation_id,
                    slot: augmentation.slot,
                    session: augmentation.session.map(Into::into),
                    diagnostic: augmentation.diagnostic,
                })
                .collect(),
        }
    }
}

impl From<velvet_preview_core::PreviewSession> for WirePreviewSession {
    fn from(value: velvet_preview_core::PreviewSession) -> Self {
        Self {
            provider_id: value.provider_id,
            context: WirePreviewContext {
                asset_id: value.context.asset_id.0,
                content_kind: value.context.content_kind.0,
                intent: format!("{:?}", value.context.intent).to_lowercase(),
                surface: format!("{:?}", value.context.surface).to_lowercase(),
            },
            session_id: value.descriptor.session_id,
            renderer: value.descriptor.renderer.into(),
            commands: value.descriptor.commands,
            slots: value.descriptor.slots,
        }
    }
}

impl From<PreviewRenderer> for WirePreviewRenderer {
    fn from(value: PreviewRenderer) -> Self {
        match value {
            PreviewRenderer::Generic => WirePreviewRenderer::Generic,
            PreviewRenderer::Text { text, truncated } => {
                WirePreviewRenderer::Text { text, truncated }
            }
            PreviewRenderer::Image {
                mime_type,
                bytes,
                truncated,
            } => WirePreviewRenderer::Image {
                mime_type,
                bytes,
                truncated,
            },
            PreviewRenderer::UiBridge {
                entrypoint,
                bridge_version,
            } => WirePreviewRenderer::UiBridge {
                entrypoint,
                bridge_version,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireEventEnvelope {
    protocol_version: u16,
    sequence: u64,
    event: WireEvent,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum WireEvent {
    LayoutChanged {
        revision: u64,
        state: WireLayoutState,
    },
    ProfileChanged {
        revision: u64,
        state: velvet_workbench_protocol::ProfileState,
    },
    RecoveryChanged {
        projection: velvet_workbench_protocol::RecoveryProjection,
    },
    PluginDisabled {
        extension_id: String,
        reason: String,
    },
    PluginDiagnostic {
        diagnostic: velvet_workbench_protocol::PluginDiagnosticEvent,
    },
    JobChanged {
        snapshot: Box<WireJobSnapshot>,
    },
}

impl From<Event> for WireEventEnvelope {
    fn from(event: Event) -> Self {
        Self::with_sequence(event, 1)
    }
}

impl WireEventEnvelope {
    fn with_sequence(event: Event, sequence: u64) -> Self {
        let event = match event {
            Event::LayoutChanged { revision, state } => WireEvent::LayoutChanged {
                revision,
                state: WireLayoutState::from(state),
            },
            Event::ProfileChanged { revision, state } => WireEvent::ProfileChanged {
                revision,
                state,
            },
            Event::RecoveryChanged { projection } => WireEvent::RecoveryChanged { projection },
            Event::PluginDisabled {
                extension_id,
                reason,
            } => WireEvent::PluginDisabled {
                extension_id,
                reason,
            },
            Event::PluginDiagnostic { diagnostic } => WireEvent::PluginDiagnostic { diagnostic },
            Event::JobChanged { snapshot } => WireEvent::JobChanged {
                snapshot: Box::new(WireJobSnapshot::from(*snapshot)),
            },
        };
        Self {
            protocol_version: PROTOCOL_VERSION,
            sequence,
            event,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireJobSnapshot {
    id: String,
    kind: String,
    status: String,
    progress: u8,
    message: Option<String>,
    attempt: u32,
    retryable: bool,
    duration_ms: u64,
    will_fail: bool,
    policy: JobExecutionPolicy,
    budget: velvet_job_system::JobBudgetRequest,
    checkpoint: Option<velvet_job_system::JobCheckpoint>,
    input: serde_json::Value,
    provenance: WireJobProvenance,
    result: Option<WireJobResult>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireAuthoritativeSnapshot {
    schema_version: u16,
    layout: WireLayoutState,
    profile: velvet_workbench_protocol::ProfileState,
    recovery: velvet_workbench_protocol::RecoveryProjection,
    jobs: Vec<WireJobSnapshot>,
    extensions: Vec<WireExtensionSummary>,
}

impl From<velvet_workbench_protocol::AuthoritativeSnapshot> for WireAuthoritativeSnapshot {
    fn from(value: velvet_workbench_protocol::AuthoritativeSnapshot) -> Self {
        Self {
            schema_version: value.schema_version,
            layout: WireLayoutState::from(value.layout),
            profile: value.profile,
            recovery: value.recovery,
            jobs: value.jobs.into_iter().map(WireJobSnapshot::from).collect(),
            extensions: value
                .extensions
                .into_iter()
                .map(WireExtensionSummary::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireJobProvenance {
    owner_id: String,
    asset_id: Option<String>,
    content_kind: Option<String>,
    input_fingerprint: Option<String>,
    provider_id: Option<String>,
    provider_version: Option<String>,
    settings_fingerprint: Option<String>,
    schema_version: u16,
}

impl From<JobProvenance> for WireJobProvenance {
    fn from(value: JobProvenance) -> Self {
        Self {
            owner_id: value.owner_id,
            asset_id: value.asset_id,
            content_kind: value.content_kind,
            input_fingerprint: value.input_fingerprint,
            provider_id: value.provider_id,
            provider_version: value.provider_version,
            settings_fingerprint: value.settings_fingerprint,
            schema_version: value.schema_version,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WireJobResult {
    schema_version: u16,
    content_type: String,
    payload: serde_json::Value,
}

impl From<JobResult> for WireJobResult {
    fn from(value: JobResult) -> Self {
        Self {
            schema_version: value.schema_version,
            content_type: value.content_type,
            payload: value.payload,
        }
    }
}

impl From<JobSnapshot> for WireJobSnapshot {
    fn from(value: JobSnapshot) -> Self {
        Self {
            id: value.id.0,
            kind: value.kind,
            status: match value.status {
                JobStatus::Queued => "queued",
                JobStatus::Running => "running",
                JobStatus::Retrying => "retrying",
                JobStatus::Cancelling => "cancelling",
                JobStatus::Paused => "paused",
                JobStatus::Interrupted => "interrupted",
                JobStatus::Cancelled => "cancelled",
                JobStatus::Failed => "failed",
                JobStatus::Completed => "completed",
            }
            .into(),
            progress: value.progress,
            message: value.message,
            attempt: value.attempt,
            retryable: value.retryable,
            duration_ms: value.duration_ms,
            will_fail: value.will_fail,
            policy: value.policy,
            budget: value.budget,
            checkpoint: value.checkpoint,
            input: value.input,
            provenance: value.provenance.into(),
            result: value.result.map(Into::into),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeShortcutEvent {
    keybinding: String,
    state: &'static str,
}

#[tauri::command]
fn velvet_core_request(
    state: State<'_, AppState>,
    envelope: WireRequestEnvelope,
) -> Result<WireResponseEnvelope, WireError> {
    if envelope.protocol_version != PROTOCOL_VERSION {
        return Err(WireError::from(CoreError::ProtocolVersionMismatch {
            expected: PROTOCOL_VERSION,
            received: envelope.protocol_version,
        }));
    }
    let request = envelope.request.into_core(envelope.protocol_version)?;
    let transport = state
        .transport
        .lock()
        .map_err(|_| WireError {
            code: "core-lock-poisoned".into(),
            message: "The application core lock is poisoned.".into(),
            retryable: false,
            expected_revision: None,
            actual_revision: None,
        })?;
    transport
        .validate_session(envelope.session_id.as_deref())
        .map_err(WireError::from)?;
    let session_id = envelope.session_id.as_deref().ok_or_else(|| WireError {
        code: "session-required".into(),
        message: "A negotiated transport session is required for Tauri requests.".into(),
        retryable: false,
        expected_revision: None,
        actual_revision: None,
    })?;
    let response = transport
        .dispatch_session(session_id, RequestId(envelope.request_id), request)
        .map_err(WireError::from)?;

    Ok(WireResponseEnvelope::from_core(ResponseEnvelope {
        protocol_version: envelope.protocol_version,
        request_id: RequestId(envelope.request_id),
        response,
    }))
}

#[tauri::command]
fn velvet_core_handshake(
    state: State<'_, AppState>,
    request: ProtocolHandshakeRequest,
) -> Result<ProtocolHandshakeResponse, WireError> {
    state
        .transport
        .lock()
        .map_err(|_| WireError {
            code: "core-lock-poisoned".into(),
            message: "The application core lock is poisoned.".into(),
            retryable: false,
            expected_revision: None,
            actual_revision: None,
        })?
        .negotiate_handshake(request)
        .map_err(WireError::from)
}

#[tauri::command]
fn velvet_activation_pending(
    state: State<'_, AppState>,
) -> Result<Vec<ActivationRequest>, WireError> {
    let mut pending = state.pending_activation.lock().map_err(|_| WireError {
        code: "activation-queue-poisoned".into(),
        message: "The native activation queue is unavailable.".into(),
        retryable: true,
        expected_revision: None,
        actual_revision: None,
    })?;
    Ok(std::mem::take(&mut *pending))
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeUpdateInstallRequest {
    expected_version: String,
}

fn clone_transport(state: &State<'_, AppState>) -> Result<TauriTransport, WireError> {
    state
        .transport
        .lock()
        .map(|transport| transport.clone())
        .map_err(|_| WireError {
            code: "core-lock-poisoned".into(),
            message: "The application core lock is poisoned.".into(),
            retryable: false,
            expected_revision: None,
            actual_revision: None,
        })
}

#[tauri::command]
async fn velvet_update_check(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<NativeUpdateStatus, WireError> {
    let transport = clone_transport(&state)?;
    state
        .update_coordinator
        .check(&app, &transport)
        .await
        .map_err(WireError::from)
}

#[tauri::command]
async fn velvet_update_install(
    app: AppHandle,
    state: State<'_, AppState>,
    request: NativeUpdateInstallRequest,
) -> Result<NativeUpdateStatus, WireError> {
    let transport = clone_transport(&state)?;
    state
        .update_coordinator
        .install(&app, &transport, &request.expected_version)
        .await
        .map_err(WireError::from)
}

fn enqueue_activation(queue: &Mutex<Vec<ActivationRequest>>, request: ActivationRequest) {
    if let Ok(mut pending) = queue.lock() {
        pending.push(request);
        while pending.len() > MAX_PENDING_ACTIVATIONS {
            pending.remove(0);
        }
    }
}

pub fn run() {
    let app_data_root = preflight_app_data_dir();
    let initial_activation = activation_request_from_args();
    let instance = match app_data_root.as_ref() {
        Some(root) => match ProfileInstanceCoordinator::acquire(root, &initial_activation) {
            Ok(AcquireOutcome::Primary(instance)) => Some(instance),
            Ok(AcquireOutcome::Forwarded) => return,
            Ok(AcquireOutcome::Locked(record)) => {
                eprintln!(
                    "Velvet profile is already owned by pid {} (instance {})",
                    record.pid, record.instance_nonce
                );
                return;
            }
            Err(error) => {
                eprintln!("Velvet could not acquire the Profile lock: {error}");
                return;
            }
        },
        None => None,
    };
    if let Some(root) = app_data_root.as_deref() {
        let _ = arm_process_receipt(root);
    }
    let preflight = HostPlatformPreflightProbe.probe();
    if preflight.webview_unavailable() {
        let message = preflight
            .evidence
            .iter()
            .map(|evidence| format!("{:?}: {}", evidence.capability, evidence.detail))
            .collect::<Vec<_>>()
            .join("\n");
        record_tauri_startup_failure_with_evidence(
            "platform-preflight-webview",
            message.clone(),
            preflight.evidence.clone(),
        );
        if let Some(root) = app_data_root.as_deref() {
            let request = NativeRecoveryRequest {
                schema_version: NATIVE_RECOVERY_SCHEMA_VERSION,
                title: "Velvet recovery".into(),
                message: "Velvet could not verify a usable WebView before starting. Retry after repairing the platform, restore the last validated state, or continue in Safe Mode.".into(),
                actions: vec![
                    NativeRecoveryAction::Retry,
                    NativeRecoveryAction::RestoreRecoverySet,
                    NativeRecoveryAction::ContinueSafeMode,
                ],
                pages: vec![
                    NativeRecoveryPage {
                        kind: NativeRecoveryPageKind::Overview,
                        title: "Velvet recovery".into(),
                        message: "Velvet could not start its WebView host. This repair flow runs without Tauri or a WebView.".into(),
                    },
                    NativeRecoveryPage {
                        kind: NativeRecoveryPageKind::Evidence,
                        title: "Platform evidence".into(),
                        message: message.clone(),
                    },
                    NativeRecoveryPage {
                        kind: NativeRecoveryPageKind::Actions,
                        title: "Choose a recovery action".into(),
                        message: "Continue to the action dialog to retry, restore the last validated state, or continue in Safe Mode.".into(),
                    },
                ],
            };
            if let Err(error) = launch_native_recovery_window(root, request) {
                eprintln!("Velvet could not launch its standalone recovery window: {error}");
            }
            let _ = complete_process_receipt(root);
        }
        return;
    }

    let shortcuts = match ShortcutBuilder::new()
        .with_shortcuts(["CommandOrControl+B", "CommandOrControl+J"])
    {
        Ok(builder) => builder
            .with_handler(|app, shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    let _ = app.emit(
                        KEYBINDING_EVENT_NAME,
                        NativeShortcutEvent {
                            keybinding: shortcut.into_string(),
                            state: "pressed",
                        },
                    );
                }
            })
            .build(),
        Err(error) => {
            record_tauri_startup_failure("shortcut-configuration", error);
            std::process::exit(1);
        }
    };

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(shortcuts)
        .setup(move |app| {
            let mut instance = instance;
            let pending_activation = Arc::new(Mutex::new(
                if initial_activation.targets.is_empty() {
                    Vec::new()
                } else {
                    vec![initial_activation.clone()]
                },
            ));
            if let Some(coordinator) = instance.as_mut() {
                let activation_app = app.handle().clone();
                let activation_queue = pending_activation.clone();
                coordinator.start_listener(move |request| {
                    enqueue_activation(&activation_queue, request.clone());
                    if let Err(error) = activation_app.emit(ACTIVATION_EVENT_NAME, request) {
                        eprintln!("Velvet could not forward an activation request: {error}");
                    }
                })?;
            }
            let sink = TauriEventSink {
                app: app.handle().clone(),
                sequence: Arc::new(AtomicU64::new(1)),
            };
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| {
                    std::io::Error::other(format!(
                        "Velvet app data directory should be available: {error}"
                    ))
                })?;
            let layout_path = app_data_dir.join("workbench-layout.json");
            let jobs_path = app_data_dir.join("jobs.json");
            let profile_path = app_data_dir.join("profile.json");
            let asset_sources_path = app_data_dir.join("asset-sources.json");
            let recovery_path = app_data_dir.join("recovery-journal.json");
            let operation_receipts_path = app_data_dir.join("operation-receipts.json");
            let import_quarantine_path = app_data_dir.join("import-quarantine.json");
            let diagnostic_events_path = app_data_dir.join("diagnostic-events.json");
            let durability_report_path = app_data_dir.join("durability-report.json");
            let snapshot_root = app_data_dir.join("snapshots");
            let recovery_set_root = app_data_dir.join("recovery-sets");
            let runtime_leases_path = app_data_dir.join("runtime-leases.json");
            let release_transaction_path = app_data_dir.join("release-transaction.json");
            let release_package_root = app_data_dir.join("release-packages");
            let crash_receipts_path = app_data_dir.join(crash_capture::CRASH_RECEIPTS_FILE);
            let plugin_directory = app_data_dir.join("plugins");
            let durability = if cfg!(unix) {
                DurabilityMode::RequireFsync
            } else {
                DurabilityMode::AtomicRenameOnly
            };
            let snapshot_store = Arc::new(JsonFileStateSnapshotStore::with_durability(
                snapshot_root,
                durability,
            ));
            let recovery_set_store = Arc::new(JsonFileRecoverySetStore::with_durability(
                recovery_set_root,
                durability,
            ));
            let legacy_participants: Vec<Box<dyn OwnerMigrationParticipant>> = vec![
                Box::new(LegacyLayoutMigrationParticipant::new(layout_path.clone())),
                Box::new(LegacyProfileMigrationParticipant::new(profile_path.clone())),
                Box::new(LegacyJobMigrationParticipant::new(jobs_path.clone())),
            ];
            let migration_participants: Vec<&dyn OwnerMigrationParticipant> = legacy_participants
                .iter()
                .map(|participant| participant.as_ref() as &dyn OwnerMigrationParticipant)
                .collect();
            let migration_result = migrate_legacy_owner_snapshots(
                snapshot_store.as_ref(),
                recovery_set_store.as_ref(),
                &migration_participants,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or_default(),
            );
            let durability_report = velvet_persistence::DurabilityReport::for_mode(durability);
            let durability_report_store =
                JsonFileDurabilityReportStore::new(durability_report_path);
            let durability_report_error = durability_report_store
                .save(&durability_report)
                .err()
                .map(|error| error.to_string());
            let mut core = ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store_and_recovery_stores(
                Arc::new(SnapshotLayoutStateStore::new(
                    snapshot_store.clone(),
                    Some(recovery_set_store.clone()),
                )),
                &plugin_directory,
                Arc::new(SnapshotJobStateStore::new(
                    snapshot_store.clone(),
                    Some(recovery_set_store.clone()),
                )),
                Arc::new(SnapshotProfileStateStore::new(
                    snapshot_store.clone(),
                    Some(recovery_set_store.clone()),
                )),
                Arc::new(JsonFileAssetSourceLocationStore::new(asset_sources_path)),
                ApplicationRecoveryStores::new(
                    Arc::new(JsonFileRecoveryJournalStore::new(recovery_path)),
                    Some(snapshot_store),
                    Some(recovery_set_store),
                )
                .with_diagnostics(Arc::new(JsonFileDiagnosticEventStore::with_durability(
                    diagnostic_events_path,
                    durability,
                )))
                .with_operation_receipts(Arc::new(
                    JsonFileOperationReceiptStore::with_durability(
                        operation_receipts_path,
                        durability,
                    ),
                ))
                .with_import_quarantine(Arc::new(
                    JsonFileImportQuarantineStore::with_durability(
                        import_quarantine_path,
                        durability,
                    ),
                ))
                .with_runtime_leases(Arc::new(JsonFileRuntimeLeaseStore::with_durability(
                    runtime_leases_path,
                    durability,
                )))
                .with_release_updates(
                    Arc::new(JsonFileReleaseUpdateStore::with_durability(
                        release_transaction_path,
                        durability,
                    )),
                    release_policy::preview_release_trust_policy(),
                )
                .with_release_packages(Arc::new(JsonFileReleasePackageStore::with_durability(
                    release_package_root,
                    durability,
                )))
                .with_durability_report(protocol_durability_report(&durability_report)),
            );
            if let Err(error) = migration_result {
                core.record_recovery_failure(
                    RecoveryFailureClass::CoreMigration,
                    format!("legacy owner migration failed: {error:?}"),
                );
            }
            if let Some(error) = durability_report_error {
                eprintln!("Velvet could not persist its durability report: {error}");
            }
            if let Some((instance_id, attempt_id)) = core.recovery_launch_identity() {
                core.configure_runtime_correlation(
                    instance_id.clone(),
                    attempt_id,
                    Some(crash_receipts_path),
                );
                let _ = update_process_receipt(
                    &app_data_dir,
                    &instance_id,
                    attempt_id,
                    StartupPhase::RecoveringState,
                );
            }
            core.record_platform_preflight(HostPlatformPreflightProbe.probe().evidence);
            let crash_capture = FileCrashCaptureAdapter::new(&app_data_dir);
            if let Ok(evidence) = crash_capture.consume() {
                for evidence in evidence {
                    core.record_crash_evidence(evidence);
                }
            }
            let mut startup_failure_reason = None;
            if let Some(failure) = consume_tauri_startup_failure(&app_data_dir) {
                startup_failure_reason = Some(format!(
                    "Tauri initialization failed during {}: {}",
                    failure.phase, failure.message
                ));
                core.record_platform_preflight(failure.platform_preflight);
                core.record_recovery_failure(
                    RecoveryFailureClass::PlatformInitialization,
                    startup_failure_reason
                        .as_deref()
                        .unwrap_or("Tauri initialization failed"),
                );
            }
            if let Some(failure) = consume_launcher_failure(&app_data_dir) {
                let reason = format!(
                    "launcher reported a pre-ready application failure during {}: {}",
                    failure.phase, failure.message
                );
                startup_failure_reason = Some(match startup_failure_reason {
                    Some(previous) => format!("{previous}; {reason}"),
                    None => reason,
                });
                core.record_crash_evidence(CrashCaptureEvidence {
                    schema_version: velvet_workbench_protocol::CRASH_CAPTURE_SCHEMA_VERSION,
                    correlation_id: "launcher".into(),
                    process_id: std::process::id(),
                    parent_process_id: None,
                    attempt_id: 0,
                    component: "launcher".into(),
                    kind: CrashCaptureKind::ProcessExit,
                    code: format!("launcher-{}", failure.phase),
                    message: failure.message,
                    runtime_nonce: None,
                    log_tail: failure.stderr_tail,
                    artifact: None,
                    occurred_at_ms: failure.occurred_at_ms,
                });
                core.record_recovery_failure(
                    RecoveryFailureClass::PlatformInitialization,
                    "launcher reported a pre-ready application failure",
                );
            }
            if let Some(reason) = startup_failure_reason {
                match core.rollback_pending_release_after_startup_failure(reason.clone()) {
                    Ok(true) => core.record_recovery_failure(
                        RecoveryFailureClass::ReleaseRollback,
                        format!(
                            "the staged release was rolled back after startup failure: {reason}"
                        ),
                    ),
                    Ok(false) => {}
                    Err(error) => core.record_recovery_failure(
                        RecoveryFailureClass::ReleaseRollback,
                        format!("automatic release rollback failed after startup failure: {error:?}"),
                    ),
                }
            }
            if let Some(intent) = consume_native_recovery_intent(&app_data_dir) {
                if matches!(intent.action, NativeRecoveryAction::RestoreRecoverySet) {
                    if let Err(error) = core.restore_recovery_set() {
                        eprintln!("Velvet could not apply the standalone recovery intent: {error:?}");
                    }
                }
            }
            core.mark_workbench_ready();
            let orphan_count = match core.reconcile_plugin_runtime_orphans() {
                Ok(reports) => {
                    if !reports.is_empty() {
                        eprintln!(
                            "Velvet reconciled {} persisted plugin runtime lease(s)",
                            reports.len()
                        );
                    }
                    // Only bounded counts cross into the native recovery surface.
                    reports.len()
                }
                Err(error) => {
                    eprintln!("Velvet could not reconcile plugin runtime leases: {error:?}");
                    0
                }
            };
            if core.in_safe_mode() {
                eprintln!("Velvet entered Safe Mode; optional plugins remain inactive");
                if let Err(error) = core.load_plugins_from_directory(&plugin_directory) {
                    eprintln!("Velvet could not inspect plugin manifests for Safe Mode repair: {error:?}");
                }
            } else if !core.begin_optional_activation() {
                eprintln!("Velvet could not enter the optional activation phase");
            } else {
                match core.load_plugins_from_directory(&plugin_directory) {
                    Ok(report) if !report.failures.is_empty() => {
                        eprintln!(
                            "Velvet skipped {} invalid plugin manifest(s)",
                            report.failures.len()
                        );
                    }
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!("Velvet could not scan plugin manifests: {error:?}");
                    }
                }
                if let Err(error) = core.cleanup_plugin_install_recovery_orphans() {
                    eprintln!(
                        "Velvet could not clean orphaned plugin recovery directories: {error:?}"
                    );
                }
                for error in core.activate_registered_plugins() {
                    eprintln!("Velvet could not start a discovered plugin: {error:?}");
                }
            }
            let native_recovery = NativeRecoveryState {
                locale: core.profile_state().locale,
                safe_mode: core.in_safe_mode(),
                core_recovery_target: core
                    .recovery_projection()
                    .last_failure
                    .and_then(|failure| failure.target)
                    .map(|target| (target.owner_id, target.scope, target.generation)),
                orphan_count,
                corrupt_extension_states: collect_corrupt_extension_states(&core),
                quarantined_runtime_leases: collect_quarantined_runtime_leases(&core),
                platform_preflight: core.recovery_projection().platform_preflight.clone(),
                crash_evidence_count: core.diagnostic_bundle().crash_evidence.len(),
            };
            if !core.in_safe_mode() {
                if let Ok(status) = core.release_status() {
                    if let Some(transaction_id) = status
                        .pending
                        .as_ref()
                        .map(|transaction| transaction.transaction_id.clone())
                    {
                        if let Err(error) = core.commit_release_health(
                            &transaction_id,
                            RELEASE_HEALTH_CHECKPOINT,
                        ) {
                            core.record_recovery_failure(
                                RecoveryFailureClass::ReleaseRollback,
                                format!(
                                    "the staged release could not pass the workbench-ready health barrier: {error:?}"
                                ),
                            );
                        }
                    }
                }
            }
            core.mark_ready();
            let transport = TauriTransport::with_event_sink(core, sink);
            let platform_signals = PlatformSignalController::start(transport.clone());
            app.manage(AppState {
                transport: Mutex::new(transport),
                update_coordinator: UpdateCoordinator::new(
                    env!("CARGO_PKG_VERSION"),
                    velvet_workbench_protocol::ReleaseChannel::Preview,
                ),
                platform_signals,
                instance: instance.map(Mutex::new),
                pending_activation: pending_activation.clone(),
            });
            if !initial_activation.targets.is_empty() {
                app.emit(ACTIVATION_EVENT_NAME, initial_activation.clone())?;
            }
            show_native_recovery_surface(app.handle(), native_recovery);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            velvet_core_handshake,
            velvet_core_request,
            velvet_activation_pending,
            velvet_update_check,
            velvet_update_install
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(app) => {
            let run_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                app.run(|app, event| match event {
                    tauri::RunEvent::ExitRequested { .. } => {
                        if let Some(state) = app.try_state::<AppState>() {
                            if let Ok(transport) = state.transport.lock() {
                                if let Err(error) = transport.record_shutdown_intent() {
                                    eprintln!("Velvet could not persist shutdown intent: {error:?}");
                                }
                            }
                        }
                    }
                    tauri::RunEvent::Exit => {
                        if let Some(state) = app.try_state::<AppState>() {
                            if let Ok(transport) = state.transport.lock() {
                                if let Err(error) = transport.record_shutdown_complete() {
                                    eprintln!("Velvet could not persist shutdown completion: {error:?}");
                                }
                            }
                        }
                        if let Some(root) = preflight_app_data_dir() {
                            let _ = complete_process_receipt(&root);
                        }
                    }
                    tauri::RunEvent::Resumed => {
                        if let Some(state) = app.try_state::<AppState>() {
                            state.platform_signals.on_resumed();
                        }
                    }
                    _ => {}
                });
            }));
            if let Err(payload) = run_result {
                record_tauri_startup_failure("tauri-run", panic_payload_message(payload));
                std::process::exit(1);
            }
        }
        Err(error) => {
            record_tauri_startup_failure("tauri-build", error);
            std::process::exit(1);
        }
    }
}

/// Runs the standalone platform-native repair process. It intentionally does
/// not construct a Tauri application or a WebView.
pub fn run_native_recovery_window(root: PathBuf) {
    if let Err(error) = native_recovery_window::run_native_recovery_window(&root) {
        eprintln!("Velvet standalone recovery window failed: {error}");
        let _ = native_recovery_window::consume_native_recovery_intent(&root);
    }
}

fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "Tauri run panicked without a string payload".into()
    }
}

fn protocol_durability_report(
    report: &velvet_persistence::DurabilityReport,
) -> velvet_workbench_protocol::DurabilityReport {
    velvet_workbench_protocol::DurabilityReport {
        schema_version: report.schema_version,
        mode: match report.mode {
            DurabilityMode::AtomicRenameOnly => {
                velvet_workbench_protocol::DurabilityMode::AtomicRenameOnly
            }
            DurabilityMode::RequireFsync => {
                velvet_workbench_protocol::DurabilityMode::RequireFsync
            }
        },
        atomic_replace: report.atomic_replace,
        file_sync: report.file_sync,
        directory_sync: report.directory_sync,
        crash_durable: report.crash_durable,
        detail: report.detail.clone(),
    }
}

fn collect_corrupt_extension_states(core: &ApplicationCore) -> Vec<(String, u64)> {
    core.discover_plugins()
        .into_iter()
        .filter_map(|summary| {
            let recovery = core
                .query_plugin_storage_recovery(&summary.id)
                .ok()?;
            if recovery.status == velvet_workbench_protocol::PluginStorageRecoveryStatus::Corrupt {
                recovery
                    .quarantine_generation
                    .map(|generation| (summary.id.0, generation))
            } else {
                None
            }
        })
        .collect()
}

fn collect_quarantined_runtime_leases(core: &ApplicationCore) -> Vec<(String, String)> {
    core.query_plugin_runtime_recovery()
        .unwrap_or_default()
        .into_iter()
        .map(|recovery| (recovery.extension_id.0, recovery.runtime_nonce))
        .collect()
}

fn dispatch_native_recovery_command<R: Runtime>(
    app: &AppHandle<R>,
    command: Command,
) -> Result<(), String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "native recovery state is unavailable".to_owned())?;
    let transport = state
        .transport
        .lock()
        .map_err(|_| "native recovery core lock is poisoned".to_owned())?;
    transport
        .dispatch(
            RequestId(NATIVE_RECOVERY_REQUEST_ID.fetch_add(1, Ordering::Relaxed)),
            Request::Command(command),
        )
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

/// Shows a minimal host-native recovery action surface. It only uses the
/// platform dialog plugin and typed Core commands, so it remains available
/// when the normal WebView/renderer cannot mount.
fn show_native_recovery_surface<R: Runtime>(
    app: &AppHandle<R>,
    recovery: NativeRecoveryState,
) {
    if !recovery.requires_surface() {
        return;
    }
    let locale = NativeHostLocale::from_profile_locale(&recovery.locale);
    let mut details = Vec::new();
    if recovery.safe_mode {
        details.push(locale.safe_mode_message().to_owned());
    }
    if recovery.orphan_count > 0 {
        details.push(locale.orphan_message(recovery.orphan_count));
    }
    if !recovery.corrupt_extension_states.is_empty() {
        details.push(locale.corrupt_message(recovery.corrupt_extension_states.len()));
    }
    if !recovery.quarantined_runtime_leases.is_empty() {
        details.push(locale.quarantined_message(recovery.quarantined_runtime_leases.len()));
    }
    if recovery
        .platform_preflight
        .iter()
        .any(|evidence| {
            evidence.capability == velvet_workbench_protocol::PlatformCapability::Webview
                && evidence.status == PlatformCapabilityStatus::Unavailable
        })
    {
        details.push(locale.platform_message(&recovery.platform_preflight));
    }
    if recovery.crash_evidence_count > 0 {
        details.push(locale.crash_evidence_message(recovery.crash_evidence_count));
    }
    let message = details.join("\n\n");

    if recovery.safe_mode {
        let handle = app.clone();
        app.dialog()
            .message(message)
            .title(locale.recovery_title())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                locale.restore_button().into(),
                locale.continue_safe_mode_button().into(),
            ))
            .show(move |restore| {
                if !restore {
                    return;
                }
                match dispatch_native_recovery_command(&handle, Command::RestoreRecoverySet) {
                    Ok(()) => handle
                        .dialog()
                        .message(locale.restore_staged())
                        .title(locale.recovery_title())
                        .buttons(MessageDialogButtons::Ok)
                        .show(|_| {}),
                    Err(error) => handle
                        .dialog()
                        .message(locale.restore_error(&error))
                        .title(locale.recovery_title())
                        .kind(MessageDialogKind::Error)
                        .buttons(MessageDialogButtons::Ok)
                        .show(|_| {}),
                }
            });
        return;
    }

    if let Some((owner_id, scope, generation)) = recovery.core_recovery_target {
        let handle = app.clone();
        details.push(locale.core_corrupt_message(&owner_id, &scope, generation));
        let message = details.join("\n\n");
        app.dialog()
            .message(message)
            .title(locale.recovery_title())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                locale.quarantine_core_button().into(),
                locale.continue_button().into(),
            ))
            .show(move |quarantine| {
                if !quarantine {
                    return;
                }
                match dispatch_native_recovery_command(
                    &handle,
                    Command::QuarantineRecoveryFailure,
                ) {
                    Ok(()) => handle
                        .dialog()
                        .message(locale.quarantine_core_success())
                        .title(locale.recovery_title())
                        .buttons(MessageDialogButtons::Ok)
                        .show(|_| {}),
                    Err(error) => handle
                        .dialog()
                        .message(locale.restore_error(&error))
                        .title(locale.recovery_title())
                        .kind(MessageDialogKind::Error)
                        .buttons(MessageDialogButtons::Ok)
                        .show(|_| {}),
                }
            });
        return;
    }

    if !recovery.corrupt_extension_states.is_empty() {
        let handle = app.clone();
        let targets = recovery.corrupt_extension_states;
        app.dialog()
            .message(message)
            .title(locale.extension_repair_title())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                locale.quarantine_button().into(),
                locale.continue_button().into(),
            ))
            .show(move |quarantine| {
                if !quarantine {
                    return;
                }
                let mut failures = Vec::new();
                for (extension_id, generation) in targets {
                    if let Err(error) = dispatch_native_recovery_command(
                        &handle,
                        Command::QuarantinePluginState {
                            extension_id: ExtensionId(extension_id.clone()),
                            generation,
                        },
                    ) {
                        failures.push(format!("{extension_id}: {error}"));
                    }
                }
                let result = if failures.is_empty() {
                    locale.repair_success().to_owned()
                } else {
                    format!("Some extension state could not be quarantined:\n{}", failures.join("\n"))
                };
                handle
                    .dialog()
                    .message(result)
                    .title(locale.extension_repair_title())
                    .buttons(MessageDialogButtons::Ok)
                    .show(|_| {});
            });
        return;
    }

    if !recovery.quarantined_runtime_leases.is_empty() {
        let handle = app.clone();
        let targets = recovery.quarantined_runtime_leases;
        app.dialog()
            .message(message)
            .title(locale.recovery_title())
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                locale.close_lease_button().into(),
                locale.continue_button().into(),
            ))
            .show(move |close_records| {
                if !close_records {
                    return;
                }
                let mut failures = Vec::new();
                for (extension_id, runtime_nonce) in targets {
                    if let Err(error) = dispatch_native_recovery_command(
                        &handle,
                        Command::CloseQuarantinedRuntimeLease {
                            extension_id: ExtensionId(extension_id.clone()),
                            runtime_nonce,
                        },
                    ) {
                        failures.push(format!("{extension_id}: {error}"));
                    }
                }
                let result = if failures.is_empty() {
                    locale.close_success().to_owned()
                } else {
                    format!("Some lease records could not be closed:\n{}", failures.join("\n"))
                };
                handle
                    .dialog()
                    .message(result)
                    .title(locale.recovery_title())
                    .buttons(MessageDialogButtons::Ok)
                    .show(|_| {});
            });
        return;
    }

    app.dialog()
        .message(message)
        .title(locale.recovery_title())
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::Ok)
        .show(|_| {});
}

fn validate_inner_protocol(outer: u16, inner: u16) -> Result<(), WireError> {
    if outer != inner {
        return Err(WireError {
            code: "protocol-version-mismatch".into(),
            message: format!("Envelope protocol {outer} does not match request protocol {inner}"),
            retryable: false,
            expected_revision: None,
            actual_revision: None,
        });
    }
    Ok(())
}

fn parse_disable_reason(value: &str) -> Result<PluginDisableReason, WireError> {
    match value {
        "user" => Ok(PluginDisableReason::User),
        "crash" => Ok(PluginDisableReason::Crash),
        "incompatible" => Ok(PluginDisableReason::Incompatible),
        _ => Err(invalid_wire_value("plugin disable reason", value)),
    }
}

fn parse_app(value: &str) -> Result<WorkbenchAppId, WireError> {
    if value.is_empty()
        || !value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric() || (matches!(character, '.' | '_' | '-') && index > 0)
        })
    {
        return Err(invalid_wire_value("app id", value));
    }
    Ok(WorkbenchAppId::from_id(value))
}

fn parse_part(value: &str) -> Result<WorkbenchPart, WireError> {
    let part = match value {
        "title-bar" => WorkbenchPart::TitleBar,
        "activity-bar" => WorkbenchPart::ActivityBar,
        "primary-sidebar" => WorkbenchPart::PrimarySidebar,
        "workspace-area" => WorkbenchPart::WorkspaceArea,
        "auxiliary-sidebar" => WorkbenchPart::AuxiliarySidebar,
        "operations-panel" => WorkbenchPart::OperationsPanel,
        "status-bar" => WorkbenchPart::StatusBar,
        "overlay-layer" => WorkbenchPart::OverlayLayer,
        _ => return Err(invalid_wire_value("part", value)),
    };
    Ok(part)
}

fn view_anchor(
    anchor: Option<WireOrderAnchor>,
) -> Result<(Option<String>, Option<String>), WireError> {
    match anchor {
        None => Ok((None, None)),
        Some(anchor) if anchor.position == "before" => Ok((Some(anchor.id), None)),
        Some(anchor) if anchor.position == "after" => Ok((None, Some(anchor.id))),
        Some(anchor) => Err(invalid_wire_value("order anchor", &anchor.position)),
    }
}

fn parse_preview_intent(value: &str) -> Result<PreviewIntent, WireError> {
    match value {
        "quick" => Ok(PreviewIntent::Quick),
        "inspect" => Ok(PreviewIntent::Inspect),
        "analyze" => Ok(PreviewIntent::Analyze),
        _ => Err(invalid_wire_value("preview intent", value)),
    }
}

fn parse_preview_surface(value: &str) -> Result<PreviewSurface, WireError> {
    match value {
        "workspace" => Ok(PreviewSurface::Workspace),
        "thumbnail" => Ok(PreviewSurface::Thumbnail),
        "inspector" => Ok(PreviewSurface::Inspector),
        _ => Err(invalid_wire_value("preview surface", value)),
    }
}

fn invalid_wire_value(kind: &str, value: &str) -> WireError {
    WireError {
        code: "invalid-request".into(),
        message: format!("Unknown {kind}: {value}"),
        retryable: false,
        expected_revision: None,
        actual_revision: None,
    }
}

fn app_name(app: &WorkbenchAppId) -> String {
    app.as_str().into()
}

fn part_name(part: &WorkbenchPart) -> String {
    match part {
        WorkbenchPart::TitleBar => "title-bar",
        WorkbenchPart::ActivityBar => "activity-bar",
        WorkbenchPart::PrimarySidebar => "primary-sidebar",
        WorkbenchPart::WorkspaceArea => "workspace-area",
        WorkbenchPart::AuxiliarySidebar => "auxiliary-sidebar",
        WorkbenchPart::OperationsPanel => "operations-panel",
        WorkbenchPart::StatusBar => "status-bar",
        WorkbenchPart::OverlayLayer => "overlay-layer",
    }
    .into()
}

fn extension_state_name(state: &velvet_plugin_api::ExtensionState) -> &'static str {
    match state {
        velvet_plugin_api::ExtensionState::Registered => "registered",
        velvet_plugin_api::ExtensionState::Starting => "starting",
        velvet_plugin_api::ExtensionState::Active => "active",
        velvet_plugin_api::ExtensionState::Disabled => "disabled",
        velvet_plugin_api::ExtensionState::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_transport_envelope_preserves_optional_session_and_handshake_shape() {
        let envelope: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 7,
            "sessionId": "core-test-1",
            "request": { "kind": "query-profile", "protocolVersion": 1 }
        }))
        .expect("session-aware request should deserialize");
        assert_eq!(envelope.session_id.as_deref(), Some("core-test-1"));
        assert!(matches!(envelope.request, WireRequest::QueryProfile { .. }));

        let handshake = ProtocolHandshakeRequest {
            client_id: "desktop-test".into(),
            protocol_major: 1,
            protocol_minor: 0,
            features: vec!["session-v1".into()],
            schema_versions: BTreeMap::from([("profile".into(), 1)]),
        };
        let json = serde_json::to_value(handshake).expect("handshake should use wire names");
        assert_eq!(json["protocolMajor"], 1);
        assert_eq!(json["schemaVersions"]["profile"], 1);
    }

    #[test]
    fn wire_snapshot_and_request_outcome_keep_resync_contract_typed() {
        let snapshot = ApplicationCore::default()
            .dispatch(Request::Query(velvet_workbench_protocol::Query::Snapshot))
            .expect("snapshot query should succeed");
        let response = WireResponseEnvelope::from_core(ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: RequestId(51),
            response: snapshot,
        });
        let value = serde_json::to_value(response).expect("snapshot should serialize");
        assert_eq!(value["response"]["kind"], "snapshot");
        assert_eq!(value["response"]["snapshot"]["schemaVersion"], 1);

        let outcome: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 52,
            "sessionId": "core-test-1",
            "request": {
                "kind": "query-request-outcome",
                "protocolVersion": 1,
                "requestId": 44
            }
        }))
        .expect("outcome query should deserialize");
        assert!(matches!(
            outcome.request.into_core(1).expect("outcome query should map"),
            Request::Query(velvet_workbench_protocol::Query::RequestOutcome {
                request_id: RequestId(44)
            })
        ));
    }

    #[test]
    fn wire_release_transaction_commands_and_status_keep_typed_boundaries() {
        let stage: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 61,
            "request": {
                "kind": "stage-release",
                "protocolVersion": 1,
                "transactionId": "txn-1",
                "stagedPackageId": "stage-1",
                "packageGeneration": "pkg-1",
                "manifest": {
                    "schemaVersion": 1,
                    "channel": "preview",
                    "version": "0.0.5",
                    "target": "test-target",
                    "artifactSha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "updaterSchema": 1,
                    "minDataSchema": 1,
                    "maxDataSchema": 1,
                    "minClientVersion": "0.0.1",
                    "releaseSequence": 1,
                    "issuedAtMs": 100,
                    "expiresAtMs": 1000,
                    "signingKeyId": "root-1",
                    "signature": "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
                }
            }
        }))
        .expect("stage request should deserialize");
        assert!(matches!(
            stage.request.into_core(1).expect("stage request should map"),
            Request::Command(Command::StageRelease { .. })
        ));

        let response = WireResponseEnvelope::from_core(ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: RequestId(62),
            response: Response::ReleaseStatus(velvet_workbench_protocol::ReleaseStatusDto {
                schema_version: 1,
                committed_sequence: 0,
                committed_package_generation: "bootstrap".into(),
                pending: None,
                last_transaction: None,
            }),
        });
        let value = serde_json::to_value(response).expect("release status should serialize");
        assert_eq!(value["response"]["kind"], "release-status");
        assert_eq!(value["response"]["status"]["committedPackageGeneration"], "bootstrap");
    }

    #[test]
    fn native_recovery_state_requires_a_surface_only_for_repairable_host_failures() {
        assert!(!NativeRecoveryState::default().requires_surface());
        assert!(NativeRecoveryState {
            locale: "en".into(),
            safe_mode: true,
            ..NativeRecoveryState::default()
        }
        .requires_surface());
        assert!(NativeRecoveryState {
            locale: "en".into(),
            core_recovery_target: Some(("layout".into(), "workspace".into(), 2)),
            ..NativeRecoveryState::default()
        }
        .requires_surface());
        assert!(NativeRecoveryState {
            locale: "en".into(),
            corrupt_extension_states: vec![("sample.reader".into(), 7)],
            ..NativeRecoveryState::default()
        }
        .requires_surface());
        assert!(NativeRecoveryState {
            locale: "en".into(),
            quarantined_runtime_leases: vec![("sample.reader".into(), "nonce".into())],
            ..NativeRecoveryState::default()
        }
        .requires_surface());
        assert!(NativeRecoveryState {
            locale: "en".into(),
            platform_preflight: vec![PlatformPreflightEvidence {
                capability: velvet_workbench_protocol::PlatformCapability::Webview,
                status: PlatformCapabilityStatus::Unavailable,
                detail: "missing".into(),
                checked_at_ms: 1,
            }],
            ..NativeRecoveryState::default()
        }
        .requires_surface());
    }

    #[test]
    fn durability_report_mapping_preserves_protocol_fields() {
        let report = velvet_persistence::DurabilityReport::for_mode(DurabilityMode::AtomicRenameOnly);
        let wire = protocol_durability_report(&report);
        assert_eq!(wire.schema_version, report.schema_version);
        assert_eq!(wire.atomic_replace, report.atomic_replace);
        assert_eq!(wire.file_sync, report.file_sync);
        assert_eq!(wire.directory_sync, report.directory_sync);
        assert_eq!(wire.crash_durable, report.crash_durable);
        assert_eq!(wire.detail, report.detail);
    }

    #[test]
    fn native_recovery_copy_follows_profile_locale_and_falls_back_to_english() {
        assert_eq!(
            NativeHostLocale::from_profile_locale("zh-Hans"),
            NativeHostLocale::SimplifiedChinese
        );
        assert_eq!(
            NativeHostLocale::from_profile_locale("zh-CN"),
            NativeHostLocale::SimplifiedChinese
        );
        assert_eq!(
            NativeHostLocale::from_profile_locale("fr-FR"),
            NativeHostLocale::English
        );
        assert_eq!(NativeHostLocale::SimplifiedChinese.continue_button(), "继续");
        assert_eq!(NativeHostLocale::English.continue_button(), "Continue");
    }

    #[test]
    fn tauri_startup_failure_marker_is_bounded_and_consumed() {
        let root = std::env::temp_dir().join(format!(
            "velvet-tauri-startup-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        std::env::set_var("VELVET_APP_DATA_DIR", &root);
        record_tauri_startup_failure_with_evidence(
            "tauri-build",
            "a\nplatform failure",
            vec![PlatformPreflightEvidence {
                capability: velvet_workbench_protocol::PlatformCapability::Webview,
                status: PlatformCapabilityStatus::Unavailable,
                detail: "missing WebKit".into(),
                checked_at_ms: 1,
            }],
        );
        let record = consume_tauri_startup_failure(&root).expect("the marker should be readable");
        assert_eq!(record.phase, "tauri-build");
        assert_eq!(record.message, "a platform failure");
        assert_eq!(record.platform_preflight.len(), 1);
        assert!(consume_tauri_startup_failure(&root).is_none());
        std::env::remove_var("VELVET_APP_DATA_DIR");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wire_runtime_recovery_query_and_close_command_use_stable_names() {
        let query: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 41,
                "request": {
                    "kind": "query-plugin-runtime-recovery",
                    "protocolVersion": 1
                }
            }"#,
        )
        .expect("runtime recovery query should deserialize");
        assert!(matches!(
            query.request.into_core(1).expect("query should map"),
            Request::Query(velvet_workbench_protocol::Query::PluginRuntimeRecovery)
        ));

        let close: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 42,
                "request": {
                    "kind": "close-quarantined-runtime-lease",
                    "protocolVersion": 1,
                    "extensionId": "sample.reader",
                    "runtimeNonce": "nonce-7"
                }
            }"#,
        )
        .expect("runtime lease close should deserialize");
        assert!(matches!(
            close.request.into_core(1).expect("command should map"),
            Request::Command(Command::CloseQuarantinedRuntimeLease {
                extension_id,
                runtime_nonce
            }) if extension_id.0 == "sample.reader" && runtime_nonce == "nonce-7"
        ));

        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(43),
                response: Response::PluginRuntimeRecovery(vec![
                    velvet_workbench_protocol::PluginRuntimeRecoveryDto {
                        extension_id: ExtensionId("sample.reader".into()),
                        runtime_nonce: "nonce-7".into(),
                        state: velvet_workbench_protocol::PluginRuntimeRecoveryState::Quarantined,
                        started_at_ms: 17,
                    },
                ]),
            },
        );
        let value = serde_json::to_value(response).expect("runtime recovery should serialize");
        assert_eq!(value["response"]["kind"], "plugin-runtime-recovery");
        assert_eq!(value["response"]["recoveries"][0]["runtimeNonce"], "nonce-7");
        assert_eq!(value["response"]["recoveries"][0]["state"], "quarantined");

        let import_query: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 44,
                "request": {
                    "kind": "query-import-quarantine",
                    "protocolVersion": 1
                }
            }"#,
        )
        .expect("import quarantine query should deserialize");
        assert!(matches!(
            import_query.request.into_core(1).expect("query should map"),
            Request::Query(velvet_workbench_protocol::Query::ImportQuarantine)
        ));

        let discard: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 45,
                "request": {
                    "kind": "discard-import-quarantine",
                    "protocolVersion": 1,
                    "bundleId": "bundle-1",
                    "planId": "plan-1",
                    "sectionId": "extension-sample"
                }
            }"#,
        )
        .expect("import quarantine discard should deserialize");
        assert!(matches!(
            discard.request.into_core(1).expect("command should map"),
            Request::Command(Command::DiscardImportQuarantine {
                bundle_id,
                plan_id,
                section_id
            }) if bundle_id == "bundle-1" && plan_id == "plan-1" && section_id == "extension-sample"
        ));

        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(46),
                response: Response::ImportQuarantine(vec![
                    velvet_workbench_protocol::ImportQuarantineSummary {
                        schema_version: 1,
                        bundle_id: "bundle-1".into(),
                        plan_id: "plan-1".into(),
                        section_id: "extension-sample".into(),
                        owner_id: "sample.reader".into(),
                        section_schema_version: 2,
                        quarantined_at_ms: 42,
                        reason: "disabled".into(),
                        payload_bytes: 3,
                    },
                ]),
            },
        );
        let value = serde_json::to_value(response).expect("import quarantine should serialize");
        assert_eq!(value["response"]["kind"], "import-quarantine");
        assert_eq!(value["response"]["entries"][0]["payloadBytes"], 3);
        assert!(value["response"]["entries"][0].get("payload").is_none());

        let operation_query: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 47,
                "request": {
                    "kind": "query-operation-receipts",
                    "protocolVersion": 1
                }
            }"#,
        )
        .expect("operation receipt query should deserialize");
        assert!(matches!(
            operation_query.request.into_core(1).expect("query should map"),
            Request::Query(velvet_workbench_protocol::Query::OperationReceipts)
        ));

        let operation_plan = velvet_workbench_protocol::OperationPlan {
            schema_version: 1,
            plan_id: "plan-delete".into(),
            operation_id: "operation-delete".into(),
            owner_id: "asset".into(),
            kind: velvet_workbench_protocol::OperationKind::Delete,
            targets: vec![velvet_workbench_protocol::OperationTarget {
                target_id: "asset-1".into(),
                expected_revision: None,
                expected_fingerprint: Some("sha256:asset-1".into()),
            }],
            created_at_ms: 1,
            expires_at_ms: 100,
            retry_policy: velvet_workbench_protocol::OperationRetryPolicy::Never,
            undo: velvet_workbench_protocol::OperationUndoCapability::None,
        };
        let commit: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 48,
            "request": {
                "kind": "commit-asset-delete",
                "protocolVersion": 1,
                "plan": operation_plan,
            }
        }))
        .expect("asset delete command should deserialize");
        assert!(matches!(
            commit.request.into_core(1).expect("command should map"),
            Request::Command(Command::CommitAssetDelete { plan })
                if plan.operation_id == "operation-delete"
        ));

        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(49),
                response: Response::OperationReceipt(
                    velvet_workbench_protocol::OperationReceipt {
                        schema_version: 1,
                        plan_id: "plan-delete".into(),
                        operation_id: "operation-delete".into(),
                        status: velvet_workbench_protocol::OperationReceiptStatus::Committed,
                        reconciled: true,
                        recorded_at_ms: 42,
                        items: vec![velvet_workbench_protocol::OperationItemReceipt {
                            target_id: "asset-1".into(),
                            status: velvet_workbench_protocol::OperationReceiptStatus::Committed,
                            message: None,
                        }],
                        undo_token: None,
                    },
                ),
            },
        );
        let value = serde_json::to_value(response).expect("operation receipt should serialize");
        assert_eq!(value["response"]["kind"], "operation-receipt");
        assert_eq!(value["response"]["receipt"]["operationId"], "operation-delete");
    }

    #[test]
    fn wire_plugin_manifest_preserves_extension_storage_declaration() {
        let manifest = ExtensionManifest::try_from(WirePluginManifest {
            manifest_schema: 1,
            id: "sample.storage".into(),
            version: "1.0.0".into(),
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
            storage: Some(velvet_plugin_api::ExtensionStorageDeclaration {
                schema_version: 2,
                quota_bytes: 8192,
            }),
            workbench_app: None,
            contributions: Vec::new(),
            permission_sets: Vec::new(),
            extensions: BTreeMap::new(),
        })
        .expect("the wire manifest should map to the domain manifest");

        assert_eq!(
            manifest.storage,
            Some(velvet_plugin_api::ExtensionStorageDeclaration {
                schema_version: 2,
                quota_bytes: 8192,
            })
        );
    }

    #[test]
    fn wire_plugin_diagnostic_event_preserves_typed_failure_evidence() {
        let value = serde_json::to_value(WireEventEnvelope::from(Event::PluginDiagnostic {
            diagnostic: velvet_workbench_protocol::PluginDiagnosticEvent {
                schema_version: velvet_workbench_protocol::PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
                sequence: 7,
                extension_id: "sample.reader".into(),
                kind: velvet_workbench_protocol::PluginDiagnosticKind::Job,
                code: "job-failed".into(),
                message: "runtime exited".into(),
                job_id: Some("job-000001".into()),
                attempt: Some(2),
                retryable: true,
                occurred_at_ms: 42,
            },
        }))
        .expect("plugin diagnostic event should serialize");

        assert_eq!(value["event"]["kind"], "plugin-diagnostic");
        assert_eq!(value["sequence"], 1);
        assert_eq!(value["event"]["diagnostic"]["schemaVersion"], 1);
        assert_eq!(value["event"]["diagnostic"]["jobId"], "job-000001");
    }

    #[test]
    fn wire_requests_accept_camel_case_protocol_fields() {
        let envelope: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 7,
                "request": {
                    "kind": "layout-command",
                    "protocolVersion": 1,
                    "command": {
                        "id": "workbench.toggle-part",
                        "part": "primary-sidebar"
                    },
                    "expectedRevision": 0
                }
            }"#,
        )
        .expect("the TypeScript envelope should deserialize");

        assert_eq!(envelope.protocol_version, 1);
        assert_eq!(envelope.request_id, 7);
        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the wire command should map"),
            Request::Command(Command::LayoutAt {
                expected_revision: Some(0),
                command: LayoutCommand::TogglePartVisibility {
                    part: WorkbenchPart::PrimarySidebar
                }
            })
        ));
    }

    #[test]
    fn wire_extension_query_and_runtime_state_use_stable_names() {
        let envelope: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 12,
                "request": {
                    "kind": "query-extensions",
                    "protocolVersion": 1
                }
            }"#,
        )
        .expect("the extension query should deserialize");

        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the extension query should map"),
            Request::Query(velvet_workbench_protocol::Query::Extensions)
        ));

        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(12),
                response: Response::Extensions(vec![velvet_plugin_api::ExtensionSummary {
                    id: velvet_plugin_api::ExtensionId("sample.reader".into()),
                    version: "1.0.0".into(),
                    active: false,
                    state: velvet_plugin_api::ExtensionState::Failed,
                }]),
            },
        );
        let value = serde_json::to_value(response).expect("extension state should serialize");
        assert_eq!(value["response"]["extensions"][0]["state"], "failed");

        let recovery_response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(25),
                response: Response::PluginInstallRecoveries(vec![
                    velvet_plugin_api::ExtensionInstallRecovery {
                        extension_id: velvet_plugin_api::ExtensionId("sample.reader".into()),
                        kind: velvet_plugin_api::ExtensionInstallRecoveryKind::Rollback,
                        operation: velvet_plugin_api::ExtensionInstallOperation::Upgrade,
                        candidate_version: Some("1.1.0".into()),
                        current: None,
                        rollback_version: Some("1.0.0".into()),
                        can_resume: false,
                        can_commit: true,
                        can_rollback: true,
                        can_discard: true,
                        issues: Vec::new(),
                    },
                ]),
            },
        );
        let recovery_value =
            serde_json::to_value(recovery_response).expect("plugin recovery should serialize");
        assert_eq!(
            recovery_value["response"]["recoveries"][0]["kind"],
            "rollback"
        );

        let log_response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(26),
                response: Response::PluginInstallRecoveryLog(vec![
                    velvet_plugin_api::ExtensionInstallRecoveryLogEntry {
                        extension_id: velvet_plugin_api::ExtensionId("sample.reader".into()),
                        action: velvet_plugin_api::ExtensionInstallRecoveryAction::Rollback,
                        status: velvet_plugin_api::ExtensionInstallRecoveryLogStatus::Failed,
                        attempt: 2,
                        message: "temporary blocker".into(),
                        removed_paths: Vec::new(),
                        occurred_at_ms: 1,
                    },
                ]),
            },
        );
        let log_value = serde_json::to_value(log_response)
            .expect("plugin recovery log should serialize");
        assert_eq!(log_value["response"]["log"][0]["status"], "failed");
    }

    #[test]
    fn wire_plugin_storage_recovery_response_preserves_metadata_only_projection() {
        let value = serde_json::to_value(WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(33),
                response: Response::PluginStorageRecovery(
                    velvet_workbench_protocol::PluginStorageRecoveryDto {
                        extension_id: velvet_plugin_api::ExtensionId("velvet.example.echo".into()),
                        status: velvet_workbench_protocol::PluginStorageRecoveryStatus::Available,
                        current_generation: Some(7),
                        previous_generation: Some(6),
                        schema_version: Some(2),
                        previous_schema_version: Some(2),
                        quarantine_generation: Some(7),
                        reason: None,
                    },
                ),
            },
        ))
        .expect("plugin storage recovery should serialize");

        assert_eq!(value["response"]["kind"], "plugin-storage-recovery");
        assert_eq!(
            value["response"]["recovery"]["extensionId"],
            "velvet.example.echo"
        );
        assert_eq!(value["response"]["recovery"]["currentGeneration"], 7);
        assert!(value["response"]["recovery"].get("payload").is_none());
    }

    #[test]
    fn wire_recovery_query_and_projection_use_stable_names() {
        let envelope: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 29,
                "request": {
                    "kind": "query-recovery",
                    "protocolVersion": 1
                }
            }"#,
        )
        .expect("the recovery query should deserialize");
        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the recovery query should map"),
            Request::Query(velvet_workbench_protocol::Query::Recovery)
        ));

        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(29),
                response: Response::Recovery(velvet_workbench_protocol::RecoveryProjection {
                    schema_version: 1,
                    mode: velvet_workbench_protocol::RecoveryMode::Safe,
                    phase: velvet_workbench_protocol::StartupPhase::Preflight,
                    attempt_id: 2,
                    consecutive_failures: 2,
                    last_failure: None,
                    last_good: None,
                    actions: vec![velvet_workbench_protocol::RecoveryAction::Retry],
                    platform_preflight: Vec::new(),
                    durability_report: None,
                }),
            },
        );
        let value = serde_json::to_value(response).expect("recovery projection should serialize");
        assert_eq!(value["response"]["kind"], "recovery");
        assert_eq!(value["response"]["projection"]["mode"], "safe");
        assert_eq!(value["response"]["projection"]["actions"][0], "retry");
    }

    #[test]
    fn wire_recovery_projection_preserves_release_rollback_failure_class() {
        let response = WireResponseEnvelope::from_core(ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: RequestId(43),
            response: Response::Recovery(velvet_workbench_protocol::RecoveryProjection {
                schema_version: velvet_workbench_protocol::RECOVERY_SCHEMA_VERSION,
                mode: velvet_workbench_protocol::RecoveryMode::Safe,
                phase: velvet_workbench_protocol::StartupPhase::Preflight,
                attempt_id: 1,
                consecutive_failures: 1,
                last_failure: Some(velvet_workbench_protocol::RecoveryFailure {
                    class: RecoveryFailureClass::ReleaseRollback,
                    phase: velvet_workbench_protocol::StartupPhase::Preflight,
                    message: "staged release rolled back".into(),
                    at_ms: 2,
                    target: None,
                }),
                last_good: None,
                actions: vec![velvet_workbench_protocol::RecoveryAction::Retry],
                platform_preflight: Vec::new(),
                durability_report: None,
            }),
        });
        let value = serde_json::to_value(response).expect("recovery response should serialize");
        assert_eq!(
            value["response"]["projection"]["lastFailure"]["class"],
            "release-rollback"
        );
    }

    #[test]
    fn wire_recovery_diagnostics_export_maps_to_a_typed_command() {
        let envelope: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 30,
            "request": {
                "kind": "export-recovery-diagnostics",
                "protocolVersion": 1,
                "destination": "/tmp/velvet-recovery.json"
            }
        }))
        .expect("the recovery diagnostics request should deserialize");
        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the recovery diagnostics request should map"),
            Request::Command(Command::ExportRecoveryDiagnostics { destination })
                if destination == "/tmp/velvet-recovery.json"
        ));
    }

    #[test]
    fn wire_recovery_restore_and_quarantine_map_to_typed_commands() {
        for (kind, expected) in [
            (
                "restore-recovery-set",
                Command::RestoreRecoverySet,
            ),
            (
                "quarantine-recovery-failure",
                Command::QuarantineRecoveryFailure,
            ),
        ] {
            let envelope: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
                "protocolVersion": 1,
                "requestId": 31,
                "request": {
                    "kind": kind,
                    "protocolVersion": 1
                }
            }))
            .expect("the recovery action request should deserialize");
            assert_eq!(
                envelope
                    .request
                    .into_core(1)
                    .expect("the recovery action request should map"),
                Request::Command(expected)
            );
        }
    }

    #[test]
    fn wire_plugin_localization_request_and_resource_use_stable_names() {
        let envelope: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 31,
            "request": {
                "kind": "read-plugin-localization",
                "protocolVersion": 1,
                "extensionId": "sample.localized",
                "locale": "zh-Hans"
            }
        }))
        .expect("the plugin resource request should deserialize");
        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the plugin resource request should map"),
            Request::Query(velvet_workbench_protocol::Query::PluginLocalization {
                extension_id,
                locale
            }) if extension_id.0 == "sample.localized" && locale == "zh-Hans"
        ));

        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(31),
                response: Response::PluginResource(velvet_workbench_protocol::PluginResourceDto {
                    schema_version: velvet_workbench_protocol::PLUGIN_RESOURCE_SCHEMA_VERSION,
                    extension_id: velvet_plugin_api::ExtensionId("sample.localized".into()),
                    locale: "zh-Hans".into(),
                    content_type: "application/json".into(),
                    bytes: "{\"ready\":\"就绪\"}".as_bytes().to_vec(),
                }),
            },
        );
        let value =
            serde_json::to_value(response).expect("the resource response should serialize");
        assert_eq!(value["response"]["resource"]["schemaVersion"], 1);
        assert_eq!(
            value["response"]["resource"]["extensionId"],
            "sample.localized"
        );
        assert_eq!(value["response"]["resource"]["locale"], "zh-Hans");
        assert!(value["response"]["resource"]["bytes"].is_array());
    }

    #[test]
    fn wire_plugin_lifecycle_requests_map_to_versioned_core_commands() {
        let register: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 13,
                "request": {
                    "kind": "register-plugin",
                    "protocolVersion": 1,
                    "manifest": {
                        "manifestSchema": 1,
                        "id": "sample.reader",
                        "version": "1.0.0",
                        "hostApi": "^1",
                        "bridgeApi": "^1",
                        "runtimeProtocol": "^1",
                        "runtime": "built-in",
                        "extensionKind": ["capability"],
                        "launch": null,
                        "activationEvents": [],
                        "extensionDependencies": [],
                        "targetApp": null,
                        "support": {
                            "trustedWorkspace": true,
                            "virtualWorkspace": false,
                            "profiles": true
                        },
                        "requiredCapabilities": [],
                        "optionalCapabilities": [],
                        "localization": null,
                        "workbenchApp": null,
                        "contributions": [],
                        "permissionSets": []
                    }
                }
            }"#,
        )
        .expect("the plugin registration request should deserialize");

        assert!(matches!(
            register
                .request
                .into_core(1)
                .expect("the plugin registration request should map"),
            Request::Command(Command::RegisterPlugin { manifest })
                if manifest.id.0 == "sample.reader"
        ));

        let disable: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 14,
                "request": {
                    "kind": "disable-plugin",
                    "protocolVersion": 1,
                    "extensionId": "sample.reader",
                    "reason": "user"
                }
            }"#,
        )
        .expect("the plugin disable request should deserialize");

        assert!(matches!(
            disable
                .request
                .into_core(1)
                .expect("the plugin disable request should map"),
            Request::Command(Command::DisablePlugin {
                extension_id,
                reason: PluginDisableReason::User,
            }) if extension_id.0 == "sample.reader"
        ));

        let install: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 18,
                "request": {
                    "kind": "install-plugin",
                    "protocolVersion": 1,
                    "sourceDir": "/tmp/velvet-example-plugin"
                }
            }"#,
        )
        .expect("the plugin install request should deserialize");
        assert!(matches!(
            install
                .request
                .into_core(1)
                .expect("the plugin install request should map"),
            Request::Command(Command::InstallPlugin { source_dir })
                if source_dir == "/tmp/velvet-example-plugin"
        ));

        let inspect: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 20,
                "request": {
                    "kind": "inspect-plugin",
                    "protocolVersion": 1,
                    "sourceDir": "/tmp/velvet-example-plugin"
                }
            }"#,
        )
        .expect("the plugin inspect request should deserialize");
        assert!(matches!(
            inspect
                .request
                .into_core(1)
                .expect("the plugin inspect request should map"),
            Request::Command(Command::InspectPlugin { source_dir })
                if source_dir == "/tmp/velvet-example-plugin"
        ));

        let recoveries: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 23,
                "request": {
                    "kind": "query-plugin-install-recoveries",
                    "protocolVersion": 1
                }
            }"#,
        )
        .expect("the plugin recovery query should deserialize");
        assert!(matches!(
            recoveries
                .request
                .into_core(1)
                .expect("the plugin recovery query should map"),
            Request::Query(velvet_workbench_protocol::Query::PluginInstallRecoveries)
        ));

        let recovery_log: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 27,
                "request": {
                    "kind": "query-plugin-install-recovery-log",
                    "protocolVersion": 1
                }
            }"#,
        )
        .expect("the plugin recovery log query should deserialize");
        assert!(matches!(
            recovery_log
                .request
                .into_core(1)
                .expect("the plugin recovery log query should map"),
            Request::Query(velvet_workbench_protocol::Query::PluginInstallRecoveryLog)
        ));

        let storage_recovery: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 30,
            "request": {
                "kind": "query-plugin-storage-recovery",
                "protocolVersion": 1,
                "extensionId": "velvet.example.echo"
            }
        }))
        .expect("the plugin storage recovery query should deserialize");
        assert!(matches!(
            storage_recovery
                .request
                .into_core(1)
                .expect("the plugin storage recovery query should map"),
            Request::Query(velvet_workbench_protocol::Query::PluginStorageRecovery {
                extension_id
            }) if extension_id.0 == "velvet.example.echo"
        ));

        for (kind, request_id, expected) in [
            ("restore-plugin-state", 31, "restore"),
            ("quarantine-plugin-state", 32, "quarantine"),
        ] {
            let request: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
                "protocolVersion": 1,
                "requestId": request_id,
                "request": {
                    "kind": kind,
                    "protocolVersion": 1,
                    "extensionId": "velvet.example.echo",
                    "generation": 4
                }
            }))
            .expect("the plugin storage action should deserialize");
            let mapped = request
                .request
                .into_core(1)
                .expect("the plugin storage action should map");
            match (expected, mapped) {
                (
                    "restore",
                    Request::Command(Command::RestorePluginState { extension_id }),
                ) if extension_id.0 == "velvet.example.echo" => {}
                (
                    "quarantine",
                    Request::Command(Command::QuarantinePluginState {
                        extension_id,
                        generation,
                    }),
                ) if extension_id.0 == "velvet.example.echo" && generation == 4 => {}
                _ => panic!("the plugin storage action mapped to an unexpected command"),
            }
        }

        let clear_recovery_log: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 28,
            "request": {
                "kind": "clear-plugin-install-recovery-log",
                "protocolVersion": 1
            }
        }))
        .expect("the recovery log clear request should deserialize");
        assert!(matches!(
            clear_recovery_log
                .request
                .into_core(1)
                .expect("the recovery log clear request should map"),
            Request::Command(Command::ClearPluginInstallRecoveryLog)
        ));

        let export_recovery_log: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 29,
            "request": {
                "kind": "export-plugin-install-recovery-log",
                "protocolVersion": 1,
                "destination": "/tmp/velvet-recovery-log.json"
            }
        }))
        .expect("the recovery log export request should deserialize");
        assert!(matches!(
            export_recovery_log
                .request
                .into_core(1)
                .expect("the recovery log export request should map"),
            Request::Command(Command::ExportPluginInstallRecoveryLog { destination })
                if destination == "/tmp/velvet-recovery-log.json"
        ));

        for (kind, request_id, expected) in [
            ("commit-plugin-install", 21, "commit"),
            ("rollback-plugin-install", 22, "rollback"),
        ] {
            let request: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
                "protocolVersion": 1,
                "requestId": request_id,
                "request": {
                    "kind": kind,
                    "protocolVersion": 1,
                    "extensionId": "velvet.example.echo"
                }
            }))
            .expect("the plugin transaction request should deserialize");
            let mapped = request
                .request
                .into_core(1)
                .expect("the plugin transaction request should map");
            match (expected, mapped) {
                (
                    "commit",
                    Request::Command(Command::CommitPluginInstall { extension_id }),
                )
                | (
                    "rollback",
                    Request::Command(Command::RollbackPluginInstall { extension_id }),
                ) if extension_id.0 == "velvet.example.echo" => {}
                _ => panic!("the plugin transaction request mapped to an unexpected command"),
            }
        }

        let resolve: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 24,
                "request": {
                    "kind": "resolve-plugin-install-recovery",
                    "protocolVersion": 1,
                    "extensionId": "velvet.example.echo",
                    "action": "rollback"
                }
            }"#,
        )
        .expect("the plugin recovery resolution should deserialize");
        assert!(matches!(
            resolve
                .request
                .into_core(1)
                .expect("the plugin recovery resolution should map"),
            Request::Command(Command::ResolvePluginInstallRecovery {
                extension_id,
                action: velvet_plugin_api::ExtensionInstallRecoveryAction::Rollback,
            }) if extension_id.0 == "velvet.example.echo"
        ));

        let uninstall: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 19,
                "request": {
                    "kind": "uninstall-plugin",
                    "protocolVersion": 1,
                    "extensionId": "velvet.example.echo"
                }
            }"#,
        )
        .expect("the plugin uninstall request should deserialize");
        assert!(matches!(
            uninstall
                .request
                .into_core(1)
                .expect("the plugin uninstall request should map"),
            Request::Command(Command::UninstallPlugin { extension_id })
                if extension_id.0 == "velvet.example.echo"
        ));
    }

    #[test]
    fn wire_archive_install_requests_preserve_source_paths() {
        let inspect: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 41,
            "request": {
                "kind": "inspect-plugin-archive",
                "protocolVersion": 1,
                "sourceArchive": "/tmp/sample.velvet-plugin.zip"
            }
        }))
        .expect("archive inspect should deserialize");
        assert!(matches!(
            inspect
                .request
                .into_core(1)
                .expect("archive inspect should map"),
            Request::Command(Command::InspectPluginArchive { source_archive })
                if source_archive == "/tmp/sample.velvet-plugin.zip"
        ));

        let install: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 42,
            "request": {
                "kind": "install-plugin-archive",
                "protocolVersion": 1,
                "sourceArchive": "/tmp/sample.velvet-plugin.zip"
            }
        }))
        .expect("archive install should deserialize");
        assert!(matches!(
            install
                .request
                .into_core(1)
                .expect("archive install should map"),
            Request::Command(Command::InstallPluginArchive { source_archive })
                if source_archive == "/tmp/sample.velvet-plugin.zip"
        ));
    }

    #[test]
    fn wire_preview_asset_location_requests_keep_source_paths_at_the_core_boundary() {
        let register: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 43,
            "request": {
                "kind": "register-preview-asset",
                "protocolVersion": 1,
                "assetId": "asset-1",
                "sourcePath": "/tmp/asset-1.txt",
                "contentKind": "text/plain",
                "approvedRoot": "/tmp"
            }
        }))
        .expect("asset registration should deserialize");
        assert!(matches!(
            register
                .request
                .into_core(1)
                .expect("asset registration should map"),
            Request::Command(Command::RegisterPreviewAsset {
                asset_id,
                source_path,
                content_kind,
                approved_root,
            }) if asset_id.0 == "asset-1"
                && source_path == "/tmp/asset-1.txt"
                && content_kind == "text/plain"
                && approved_root.as_deref() == Some("/tmp")
        ));

        let legacy_register: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 46,
            "request": {
                "kind": "register-preview-asset",
                "protocolVersion": 1,
                "assetId": "asset-legacy",
                "sourcePath": "/tmp/asset-legacy.txt",
                "contentKind": "text/plain"
            }
        }))
        .expect("legacy asset registration should deserialize");
        assert!(matches!(
            legacy_register
                .request
                .into_core(1)
                .expect("legacy asset registration should map"),
            Request::Command(Command::RegisterPreviewAsset { approved_root, .. })
                if approved_root.is_none()
        ));

        let resolve: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 44,
            "request": {
                "kind": "resolve-preview-asset",
                "protocolVersion": 1,
                "assetId": "asset-1",
                "intent": "quick",
                "surface": "workspace"
            }
        }))
        .expect("asset preview query should deserialize");
        assert!(matches!(
            resolve
                .request
                .into_core(1)
                .expect("asset preview query should map"),
            Request::Query(velvet_workbench_protocol::Query::ResolvePreviewAsset {
                asset_id,
                intent: PreviewIntent::Quick,
                surface: PreviewSurface::Workspace,
            }) if asset_id.0 == "asset-1"
        ));

        let remove: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 45,
            "request": {
                "kind": "remove-preview-asset",
                "protocolVersion": 1,
                "assetId": "asset-1"
            }
        }))
        .expect("asset removal should deserialize");
        assert!(matches!(
            remove
                .request
                .into_core(1)
                .expect("asset removal should map"),
            Request::Command(Command::RemovePreviewAsset { asset_id })
                if asset_id.0 == "asset-1"
        ));

        let query_roots: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 47,
            "request": {
                "kind": "query-approved-source-roots",
                "protocolVersion": 1
            }
        }))
        .expect("approved root query should deserialize");
        assert!(matches!(
            query_roots
                .request
                .into_core(1)
                .expect("approved root query should map"),
            Request::Query(velvet_workbench_protocol::Query::ApprovedSourceRoots)
        ));

        let add_root: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 48,
            "request": {
                "kind": "add-approved-source-root",
                "protocolVersion": 1,
                "root": "/tmp/assets"
            }
        }))
        .expect("approved root add should deserialize");
        assert!(matches!(
            add_root
                .request
                .into_core(1)
                .expect("approved root add should map"),
            Request::Command(Command::AddApprovedSourceRoot { root }) if root == "/tmp/assets"
        ));

        let remove_root: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 49,
            "request": {
                "kind": "remove-approved-source-root",
                "protocolVersion": 1,
                "root": "/tmp/assets"
            }
        }))
        .expect("approved root removal should deserialize");
        assert!(matches!(
            remove_root
                .request
                .into_core(1)
                .expect("approved root removal should map"),
            Request::Command(Command::RemoveApprovedSourceRoot { root }) if root == "/tmp/assets"
        ));

        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: 1,
                request_id: velvet_workbench_protocol::RequestId(50),
                response: Response::ApprovedSourceRoots(vec!["/tmp/assets".into()]),
            },
        );
        assert_eq!(
            serde_json::to_value(response).expect("approved root response should serialize"),
            serde_json::json!({
                "protocolVersion": 1,
                "requestId": 50,
                "response": {
                    "kind": "approved-source-roots",
                    "roots": ["/tmp/assets"]
                }
            })
        );
    }

    #[test]
    fn wire_plugin_command_invocation_preserves_payload_and_context() {
        let envelope: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 17,
                "request": {
                    "kind": "invoke-plugin-command",
                    "protocolVersion": 1,
                    "extensionId": "sample.reader",
                    "commandId": "sample.reader.open",
                    "args": {"source": "context-menu"},
                    "context": {"activeAppId": "asset-manager", "keys": {"kind": "asset"}}
                }
            }"#,
        )
        .expect("the plugin command invocation should deserialize");

        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the plugin command invocation should map"),
            Request::Command(Command::InvokePluginCommand {
                extension_id,
                command_id,
                args,
                context,
            }) if extension_id.0 == "sample.reader"
                && command_id == "sample.reader.open"
                && args["source"] == "context-menu"
                && context["activeAppId"] == "asset-manager"
        ));
    }

    #[test]
    fn wire_preview_action_invocation_preserves_typed_preview_context() {
        let envelope: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 18,
            "request": {
                "kind": "invoke-preview-action",
                "protocolVersion": 1,
                "extensionId": "sample.reader",
                "commandId": "sample.reader.analyze",
                "args": {"scope": "metadata"},
                "context": {
                    "sessionId": "sample.reader:asset-1",
                    "providerId": "sample.reader.preview",
                    "assetId": "asset-1",
                    "contentKind": "image",
                    "intent": "analyze",
                    "surface": "workspace"
                }
            }
        }))
        .expect("preview action request should deserialize");

        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("preview action request should map"),
            Request::Command(Command::InvokePreviewAction {
                extension_id,
                command_id,
                args,
                context,
            }) if extension_id.0 == "sample.reader"
                && command_id == "sample.reader.analyze"
                && args["scope"] == "metadata"
                && context.session_id == "sample.reader:asset-1"
                && context.intent == velvet_plugin_api::PreviewIntent::Analyze
                && context.surface == velvet_plugin_api::PreviewSurface::Workspace
        ));
    }

    #[test]
    fn wire_preview_session_release_preserves_session_identifier() {
        let envelope: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 20,
            "request": {
                "kind": "release-preview-session",
                "protocolVersion": 1,
                "sessionId": "sample.reader:asset-1"
            }
        }))
        .expect("preview session release should deserialize");

        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("preview session release should map"),
            Request::Command(Command::ReleasePreviewSession { session_id })
                if session_id == "sample.reader:asset-1"
        ));
    }

    #[test]
    fn wire_runtime_failure_has_a_retryable_plugin_error_code() {
        let error = WireError::from(CoreError::PluginRuntimeFailed {
            extension_id: "velvet.example.echo".into(),
            message: "sidecar exited".into(),
        });

        assert_eq!(error.code, "plugin-runtime-failed");
        assert!(error.retryable);
        assert!(error.message.contains("velvet.example.echo"));
    }

    #[test]
    fn wire_missing_plugin_resource_is_non_retryable_and_path_free() {
        let error = WireError::from(CoreError::PluginResourceNotFound {
            extension_id: "sample.localized".into(),
            locale: "fr".into(),
        });

        assert_eq!(error.code, "plugin-resource-not-found");
        assert!(!error.retryable);
        assert!(error.message.contains("sample.localized"));
        assert!(!error.message.contains("/"));
    }

    #[test]
    fn wire_asset_source_errors_are_typed_and_path_free() {
        let error = WireError::from(CoreError::AssetSource {
            asset_id: "asset-1".into(),
            message: "preview source was not found".into(),
            retryable: false,
        });

        assert_eq!(error.code, "asset-source-error");
        assert!(!error.retryable);
        assert!(error.message.contains("asset-1"));
        assert!(!error.message.contains("/tmp"));
    }

    #[test]
    fn wire_asset_source_root_errors_are_typed_and_path_free() {
        let error = WireError::from(CoreError::AssetSourceRoot {
            message: "approved source root access was denied".into(),
            retryable: false,
        });

        assert_eq!(error.code, "asset-source-root-error");
        assert!(!error.retryable);
        assert!(!error.message.contains("/tmp"));
    }

    #[test]
    fn wire_preview_renderer_preserves_builtin_content_projection() {
        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(18),
                response: Response::Preview(ResolvedPreview {
                    candidate: velvet_preview_core::PreviewCandidate {
                        provider_id: "builtin.text".into(),
                        content_kinds: vec![velvet_asset_core::ContentKindId("text/plain".into())],
                        intents: vec![PreviewIntent::Quick],
                        surfaces: vec![PreviewSurface::Workspace],
                        priority: 100,
                    },
                    session: velvet_preview_core::PreviewSession {
                        provider_id: "builtin.text".into(),
                        context: velvet_preview_core::PreviewContext {
                            asset_id: velvet_asset_core::AssetId("asset-1".into()),
                            content_kind: velvet_asset_core::ContentKindId("text/plain".into()),
                            intent: PreviewIntent::Quick,
                            surface: PreviewSurface::Workspace,
                        },
                        descriptor: velvet_preview_core::PreviewSessionDescriptor {
                            session_id: "builtin.text:asset-1".into(),
                            renderer: PreviewRenderer::Text {
                                text: "hello".into(),
                                truncated: true,
                            },
                            commands: Vec::new(),
                            slots: Vec::new(),
                        },
                    },
                    diagnostic: Some("source read failed".into()),
                    augmentations: vec![velvet_preview_core::ResolvedPreviewAugmentation {
                        augmentation_id: "example.metadata".into(),
                        slot: "metadata".into(),
                        session: None,
                        diagnostic: Some("augmentation unavailable".into()),
                    }],
                }),
            },
        );
        let value = serde_json::to_value(response).expect("preview response should serialize");
        assert_eq!(value["response"]["preview"]["session"]["renderer"]["kind"], "text");
        assert_eq!(
            value["response"]["preview"]["session"]["renderer"]["text"],
            "hello"
        );
        assert_eq!(
            value["response"]["preview"]["session"]["renderer"]["truncated"],
            true
        );
        assert_eq!(
            value["response"]["preview"]["diagnostic"],
            "source read failed"
        );
        assert_eq!(
            value["response"]["preview"]["augmentations"][0]["augmentationId"],
            "example.metadata"
        );
        assert_eq!(
            value["response"]["preview"]["augmentations"][0]["diagnostic"],
            "augmentation unavailable"
        );

        let image = serde_json::to_value(WirePreviewRenderer::Image {
            mime_type: "image/png".into(),
            bytes: vec![1, 2, 3],
            truncated: false,
        })
        .expect("image renderer should serialize");
        assert_eq!(image["mimeType"], "image/png");

        let bridge = serde_json::to_value(WirePreviewRenderer::UiBridge {
            entrypoint: "tauri-webview://preview".into(),
            bridge_version: "1".into(),
        })
        .expect("UI Bridge renderer should serialize");
        assert_eq!(bridge["bridgeVersion"], "1");
    }

    #[test]
    fn wire_plugin_projection_query_and_response_preserve_ui_metadata() {
        let envelope: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 16,
                "request": {
                    "kind": "query-plugin-projection",
                    "protocolVersion": 1,
                    "extensionId": "sample.reader"
                }
            }"#,
        )
        .expect("the plugin projection query should deserialize");

        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the plugin projection query should map"),
            Request::Query(velvet_workbench_protocol::Query::PluginProjection {
                extension_id
            }) if extension_id.0 == "sample.reader"
        ));

        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(16),
                response: Response::PluginProjection(velvet_workbench_protocol::PluginProjectionDto {
                    summary: velvet_plugin_api::ExtensionSummary {
                        id: velvet_plugin_api::ExtensionId("sample.reader".into()),
                        version: "1.0.0".into(),
                        active: true,
                        state: velvet_plugin_api::ExtensionState::Active,
                    },
                    manifest: velvet_workbench_protocol::PluginManifestDto::from(
                        velvet_plugin_api::ExtensionManifest {
                            id: velvet_plugin_api::ExtensionId("sample.reader".into()),
                            version: "1.0.0".into(),
                            target_app: None,
                            contributions: vec![velvet_plugin_api::ContributionDescriptor {
                                id: "sample.reader.view".into(),
                                kind: velvet_plugin_api::ContributionKind::View,
                                scope: velvet_plugin_api::ContributionScope::Workbench,
                                namespace: Some("workbench".into()),
                                container: Some(velvet_plugin_api::HostContainer::PrimarySidebar),
                                zone: Some(velvet_layout_model::WorkbenchZone::Navigation),
                                title: Some("Reader".into()),
                                title_key: None,
                                icon: Some("book".into()),
                                order: Some(1),
                                default_visible: Some(true),
                                view_container_id: None,
                                activity_bar: None,
                                label: None,
                                label_key: None,
                                command_id: None,
                                group: None,
                                when: None,
                                anchor: None,
                                ui: Some(velvet_plugin_api::UiSurfaceDescriptor {
                                    kind: velvet_plugin_api::UiSurfaceKind::View,
                                    entrypoint: "wasm://sample/reader".into(),
                                    bridge_version: "1".into(),
                                }),
                                preview: None,
                            }],
                            ..velvet_plugin_api::ExtensionManifest::default()
                        },
                    ),
                }),
            },
        );
        let value = serde_json::to_value(response).expect("plugin projection should serialize");

        assert_eq!(value["response"]["kind"], "plugin-projection");
        assert_eq!(
            value["response"]["projection"]["manifest"]["contributions"][0]["ui"]["entrypoint"],
            "wasm://sample/reader"
        );
        assert_eq!(
            value["response"]["projection"]["manifest"]["contributions"][0]["namespace"],
            "workbench"
        );
    }

    #[test]
    fn wire_plugin_lifecycle_response_uses_extension_summary_shape() {
        let response = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(15),
                response: Response::Extension(velvet_plugin_api::ExtensionSummary {
                    id: velvet_plugin_api::ExtensionId("sample.reader".into()),
                    version: "1.0.0".into(),
                    active: true,
                    state: velvet_plugin_api::ExtensionState::Active,
                }),
            },
        );
        let value = serde_json::to_value(response).expect("extension response should serialize");

        assert_eq!(value["response"]["kind"], "extension");
        assert_eq!(value["response"]["extension"]["id"], "sample.reader");
        assert_eq!(value["response"]["extension"]["state"], "active");
    }

    #[test]
    fn wire_view_registration_maps_to_layout_command() {
        let envelope: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 11,
                "request": {
                    "kind": "layout-command",
                    "protocolVersion": 1,
                    "command": {
                        "id": "workbench.upsert-view",
                        "viewId": "sample.extension.asset-list",
                        "containerId": "sample.extension.library",
                        "visible": true,
                        "order": 2
                    }
                }
            }"#,
        )
        .expect("the view registration request should deserialize");

        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the view registration command should map"),
            Request::Command(Command::LayoutAt {
                command: LayoutCommand::UpsertView {
                    view_id,
                    container_id,
                    visible: true,
                    order: 2,
                },
                ..
            }) if view_id == "sample.extension.asset-list"
                && container_id == "sample.extension.library"
        ));
    }

    #[test]
    fn wire_layout_response_uses_stable_part_names() {
        let response =
            WireResponseEnvelope::from_core(velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(9),
                response: Response::Layout(LayoutState::default()),
            });
        let value = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(value["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(value["requestId"], 9);
        assert_eq!(
            value["response"]["state"]["parts"]["primary-sidebar"]["visible"],
            true
        );
    }

    #[test]
    fn wire_activate_app_command_uses_the_native_app_identifier() {
        let envelope: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 8,
                "request": {
                    "kind": "layout-command",
                    "protocolVersion": 1,
                    "command": {
                        "id": "workbench.activate-app",
                        "appId": "feed-workbench"
                    }
                }
            }"#,
        )
        .expect("the native app activation request should deserialize");

        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the native app command should map"),
            Request::Command(Command::LayoutAt {
                command: LayoutCommand::ActivateApp(WorkbenchAppId::FeedWorkbench),
                ..
            })
        ));
    }

    #[test]
    fn wire_plugin_app_identifier_is_preserved_for_native_activation() {
        let envelope: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 10,
                "request": {
                    "kind": "layout-command",
                    "protocolVersion": 1,
                    "command": {
                        "id": "workbench.activate-app",
                        "appId": "sample.extension.research"
                    }
                }
            }"#,
        )
        .expect("the plugin app activation request should deserialize");

        assert!(matches!(
            envelope
                .request
                .into_core(1)
                .expect("the plugin app command should map"),
            Request::Command(Command::LayoutAt {
                command: LayoutCommand::ActivateApp(WorkbenchAppId::Plugin(id)),
                ..
            }) if id == "sample.extension.research"
        ));
    }

    #[test]
    fn wire_job_requests_and_responses_preserve_lifecycle_fields() {
        let envelope: WireRequestEnvelope = serde_json::from_str(
            r#"{
                "protocolVersion": 1,
                "requestId": 11,
                "request": {
                    "kind": "create-job",
                    "protocolVersion": 1,
                    "jobKind": "sample",
                    "durationMs": 2400,
                    "willFail": true,
                    "input": {"assetId": "asset-1"},
                    "budget": {
                        "memoryBytes": 4096,
                        "diskBytes": 8192,
                        "childProcesses": 1,
                        "concurrency": 1,
                        "cpuMillisPerMinute": 1000,
                        "networkBytesPerMinute": 2048
                    }
                }
            }"#,
        )
        .expect("job request should deserialize");
        assert!(matches!(
            envelope.request.into_core(1).expect("job request should map"),
            Request::Command(Command::CreateJob { kind, duration_ms, will_fail, input, policy, budget })
                if kind == "sample"
                    && duration_ms == 2400
                    && will_fail
                    && input["assetId"] == "asset-1"
                    && policy == JobExecutionPolicy::default()
                    && budget.memory_bytes == 4096
                    && budget.network_bytes_per_minute == 2048
        ));

        let cancel: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 13,
            "request": {
                "kind": "cancel-job",
                "protocolVersion": 1,
                "jobId": "job-000001"
            }
        }))
        .expect("cancel request should deserialize");
        assert!(matches!(
            cancel.request.into_core(1).expect("cancel request should map"),
            Request::Command(Command::CancelJob { job_id }) if job_id.0 == "job-000001"
        ));

        let retry: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 14,
            "request": {
                "kind": "retry-job",
                "protocolVersion": 1,
                "jobId": "job-000001"
            }
        }))
        .expect("retry request should deserialize");
        assert!(matches!(
            retry.request.into_core(1).expect("retry request should map"),
            Request::Command(Command::RetryJob { job_id }) if job_id.0 == "job-000001"
        ));

        let environment: WireRequestEnvelope = serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "requestId": 16,
            "request": {
                "kind": "set-job-execution-environment",
                "protocolVersion": 1,
                "environment": { "suspended": true, "network": "metered" }
            }
        }))
        .expect("environment request should deserialize");
        assert!(matches!(
            environment.request.into_core(1).expect("environment request should map"),
            Request::Command(Command::SetJobExecutionEnvironment { environment })
                if environment.suspended
                    && environment.network == velvet_job_system::JobNetworkState::Metered
        ));

        let response = WireResponseEnvelope::from_core(velvet_workbench_protocol::ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: RequestId(12),
            response: Response::Job(JobSnapshot {
                id: JobId("job-000001".into()),
                kind: "sample".into(),
                status: JobStatus::Failed,
                progress: 70,
                message: Some("failed".into()),
                attempt: 2,
                retryable: true,
                duration_ms: 2400,
                will_fail: true,
                policy: JobExecutionPolicy {
                    deadline_ms: Some(5_000),
                    ..JobExecutionPolicy::default()
                },
                budget: velvet_job_system::JobBudgetRequest {
                    memory_bytes: 4_096,
                    ..velvet_job_system::JobBudgetRequest::default()
                },
                checkpoint: Some(velvet_job_system::JobCheckpoint {
                    schema_version: velvet_job_system::JOB_CHECKPOINT_SCHEMA_VERSION,
                    payload: serde_json::json!({ "cursor": 3 }),
                }),
                input: serde_json::Value::Null,
                provenance: velvet_job_system::JobProvenance::for_owner("test"),
                result: None,
            }),
        });
        let value = serde_json::to_value(response).expect("job response should serialize");
        assert_eq!(value["response"]["kind"], "job");
        assert_eq!(value["response"]["job"]["status"], "failed");
        assert_eq!(value["response"]["job"]["attempt"], 2);
        assert_eq!(value["response"]["job"]["policy"]["deadlineMs"], 5_000);
        assert_eq!(value["response"]["job"]["budget"]["memoryBytes"], 4_096);
        assert_eq!(value["response"]["job"]["checkpoint"]["payload"]["cursor"], 3);
        assert_eq!(value["response"]["job"]["provenance"]["ownerId"], "test");
        assert!(value["response"]["job"]["result"].is_null());

        let completed = WireResponseEnvelope::from_core(
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(15),
                response: Response::Job(JobSnapshot {
                    id: JobId("job-000002".into()),
                    kind: "metadata".into(),
                    status: JobStatus::Completed,
                    progress: 100,
                    message: Some("Completed".into()),
                    attempt: 1,
                    retryable: false,
                    duration_ms: 200,
                    will_fail: false,
                    policy: velvet_job_system::JobExecutionPolicy::default(),
                    budget: velvet_job_system::JobBudgetRequest::default(),
                    checkpoint: None,
                    input: serde_json::Value::Null,
                    provenance: velvet_job_system::JobProvenance {
                        owner_id: "sample.extension".into(),
                        asset_id: Some("asset-1".into()),
                        content_kind: Some("image".into()),
                        input_fingerprint: Some("sha256:input".into()),
                        provider_id: Some("sample.preview".into()),
                        provider_version: Some("1.2.0".into()),
                        settings_fingerprint: Some("settings:v1".into()),
                        schema_version: 1,
                    },
                    result: Some(JobResult {
                        schema_version: 1,
                        content_type: "application/json".into(),
                        payload: serde_json::json!({ "assetId": "asset-1" }),
                    }),
                }),
            },
        );
        let completed = serde_json::to_value(completed).expect("completed job should serialize");
        assert_eq!(completed["response"]["job"]["provenance"]["providerId"], "sample.preview");
        assert_eq!(completed["response"]["job"]["result"]["contentType"], "application/json");
        assert_eq!(completed["response"]["job"]["result"]["payload"]["assetId"], "asset-1");
    }
}
