use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, ShortcutState};
use velvet_application_core::ApplicationCore;
use velvet_job_system::{JobId, JobProvenance, JobResult, JobSnapshot, JobStatus};
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
use velvet_persistence::{
    JsonFileJobStateStore, JsonFileLayoutStateStore, JsonFileProfileStateStore,
};
use velvet_preview_core::{PreviewIntent, PreviewRenderer, PreviewSurface, ResolvedPreview};
use velvet_tauri_adapter::{CoreEventSink, TauriTransport};
use velvet_workbench_protocol::{
    Command, CoreError, CoreTransport, Event, PluginDisableReason, Request, RequestEnvelope,
    RequestId, Response, PROTOCOL_VERSION,
};

const CORE_EVENT_NAME: &str = "velvet_core_event";
const KEYBINDING_EVENT_NAME: &str = "velvet_keybinding";

pub struct AppState {
    pub transport: Mutex<TauriTransport>,
}

#[derive(Clone)]
struct TauriEventSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> CoreEventSink for TauriEventSink<R> {
    fn emit(&self, event: Event) {
        let _ = self
            .app
            .emit(CORE_EVENT_NAME, WireEventEnvelope::from(event));
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireRequestEnvelope {
    pub protocol_version: u16,
    pub request_id: u64,
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
    QueryPluginProjection {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    RegisterPlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        manifest: WirePluginManifest,
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
    InstallPlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "sourceDir")]
        source_dir: String,
    },
    UninstallPlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "extensionId")]
        extension_id: String,
    },
    InspectPlugin {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "sourceDir")]
        source_dir: String,
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
    SetLocale {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        locale: String,
        #[serde(rename = "expectedRevision")]
        expected_revision: Option<u64>,
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
            Self::RegisterPlugin {
                protocol_version,
                manifest,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                let manifest: ExtensionManifest = manifest
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
            Self::InstallPlugin {
                protocol_version,
                source_dir,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::InstallPlugin { source_dir }))
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
            Self::InspectPlugin {
                protocol_version,
                source_dir,
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::InspectPlugin { source_dir }))
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
            } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Command(Command::CreateJob {
                    kind: job_kind,
                    duration_ms,
                    will_fail,
                    input: input.unwrap_or(serde_json::Value::Null),
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
    Job {
        job: WireJobSnapshot,
    },
    Jobs {
        jobs: Vec<WireJobSnapshot>,
    },
    Preview {
        preview: WireResolvedPreview,
    },
    Accepted,
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
            Response::Job(job) => WireResponse::Job {
                job: WireJobSnapshot::from(job),
            },
            Response::Jobs(jobs) => WireResponse::Jobs {
                jobs: jobs.into_iter().map(WireJobSnapshot::from).collect(),
            },
            Response::Preview(preview) => WireResponse::Preview {
                preview: WireResolvedPreview::from(preview),
            },
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
    PluginDisabled {
        extension_id: String,
        reason: String,
    },
    JobChanged {
        snapshot: WireJobSnapshot,
    },
}

impl From<Event> for WireEventEnvelope {
    fn from(event: Event) -> Self {
        let event = match event {
            Event::LayoutChanged { revision, state } => WireEvent::LayoutChanged {
                revision,
                state: WireLayoutState::from(state),
            },
            Event::ProfileChanged { revision, state } => WireEvent::ProfileChanged {
                revision,
                state,
            },
            Event::PluginDisabled {
                extension_id,
                reason,
            } => WireEvent::PluginDisabled {
                extension_id,
                reason,
            },
            Event::JobChanged { snapshot } => WireEvent::JobChanged {
                snapshot: WireJobSnapshot::from(*snapshot),
            },
        };
        Self {
            protocol_version: PROTOCOL_VERSION,
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
    input: serde_json::Value,
    provenance: WireJobProvenance,
    result: Option<WireJobResult>,
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
    let request = envelope.request.into_core(envelope.protocol_version)?;
    let response = state
        .transport
        .lock()
        .map_err(|_| WireError {
            code: "core-lock-poisoned".into(),
            message: "The application core lock is poisoned.".into(),
            retryable: false,
            expected_revision: None,
            actual_revision: None,
        })?
        .dispatch_envelope(RequestEnvelope {
            protocol_version: envelope.protocol_version,
            request_id: RequestId(envelope.request_id),
            request,
        })
        .map_err(WireError::from)?;

    Ok(WireResponseEnvelope::from_core(response))
}

pub fn run() {
    let shortcuts = ShortcutBuilder::new()
        .with_shortcuts(["CommandOrControl+B", "CommandOrControl+J"])
        .expect("default Workbench shortcuts must be valid")
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
        .build();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(shortcuts)
        .setup(|app| {
            let sink = TauriEventSink {
                app: app.handle().clone(),
            };
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Velvet app data directory should be available");
            let layout_path = app_data_dir.join("workbench-layout.json");
            let jobs_path = app_data_dir.join("jobs.json");
            let profile_path = app_data_dir.join("profile.json");
            let plugin_directory = app_data_dir.join("plugins");
            let mut core = ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store(
                Arc::new(JsonFileLayoutStateStore::new(layout_path)),
                &plugin_directory,
                Arc::new(JsonFileJobStateStore::new(jobs_path)),
                Arc::new(JsonFileProfileStateStore::new(profile_path)),
            );
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
                eprintln!("Velvet could not clean orphaned plugin recovery directories: {error:?}");
            }
            for error in core.activate_registered_plugins() {
                eprintln!("Velvet could not start a discovered plugin: {error:?}");
            }
            app.manage(AppState {
                transport: Mutex::new(TauriTransport::with_event_sink(
                    core,
                    sink,
                )),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![velvet_core_request])
        .run(tauri::generate_context!())
        .expect("error while running the Velvet Tauri application");
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
                    "input": {"assetId": "asset-1"}
                }
            }"#,
        )
        .expect("job request should deserialize");
        assert!(matches!(
            envelope.request.into_core(1).expect("job request should map"),
            Request::Command(Command::CreateJob { kind, duration_ms, will_fail, input })
                if kind == "sample"
                    && duration_ms == 2400
                    && will_fail
                    && input["assetId"] == "asset-1"
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
                input: serde_json::Value::Null,
                provenance: velvet_job_system::JobProvenance::for_owner("test"),
                result: None,
            }),
        });
        let value = serde_json::to_value(response).expect("job response should serialize");
        assert_eq!(value["response"]["kind"], "job");
        assert_eq!(value["response"]["job"]["status"], "failed");
        assert_eq!(value["response"]["job"]["attempt"], 2);
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
