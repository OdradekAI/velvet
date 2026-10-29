use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, ShortcutState};
use velvet_application_core::ApplicationCore;
use velvet_layout_model::{
    LayoutCommand, LayoutState, ViewContainerState, ViewState, WorkbenchAppId, WorkbenchPart,
};
use velvet_plugin_api::{ExtensionId, ExtensionManifest};
use velvet_persistence::JsonFileLayoutStateStore;
use velvet_preview_core::{PreviewIntent, PreviewSurface, ResolvedPreview};
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
                message: format!("Expected layout revision {expected}, received {actual}"),
                retryable: true,
                expected_revision: Some(expected),
                actual_revision: Some(actual),
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
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WireRequest {
    QueryLayout {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    QueryExtensions {
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
        manifest: ExtensionManifest,
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
            Self::QueryExtensions { protocol_version } => {
                validate_inner_protocol(outer_protocol_version, protocol_version)?;
                Ok(Request::Query(velvet_workbench_protocol::Query::Extensions))
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
                Ok(Request::Command(Command::RegisterPlugin { manifest }))
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
    Extensions {
        extensions: Vec<WireExtensionSummary>,
    },
    Extension {
        extension: WireExtensionSummary,
    },
    PluginProjection {
        projection: velvet_plugin_api::ExtensionProjection,
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
            Response::Extensions(extensions) => WireResponse::Extensions {
                extensions: extensions
                    .into_iter()
                    .map(WireExtensionSummary::from)
                    .collect(),
            },
            Response::Extension(extension) => WireResponse::Extension {
                extension: WireExtensionSummary::from(extension),
            },
            Response::PluginProjection(projection) => WireResponse::PluginProjection { projection },
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WirePreviewCandidate {
    provider_id: String,
    content_kinds: Vec<String>,
    intents: Vec<String>,
    priority: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WirePreviewSession {
    provider_id: String,
    context: WirePreviewContext,
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
                priority: value.candidate.priority,
            },
            session: WirePreviewSession {
                provider_id: value.session.provider_id,
                context: WirePreviewContext {
                    asset_id: value.session.context.asset_id.0,
                    content_kind: value.session.context.content_kind.0,
                    intent: format!("{:?}", value.session.context.intent).to_lowercase(),
                    surface: format!("{:?}", value.session.context.surface).to_lowercase(),
                },
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
    PluginDisabled {
        extension_id: String,
        reason: String,
    },
}

impl From<Event> for WireEventEnvelope {
    fn from(event: Event) -> Self {
        let event = match event {
            Event::LayoutChanged { revision, state } => WireEvent::LayoutChanged {
                revision,
                state: WireLayoutState::from(state),
            },
            Event::PluginDisabled {
                extension_id,
                reason,
            } => WireEvent::PluginDisabled {
                extension_id,
                reason,
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
            let mut core = ApplicationCore::with_store(Arc::new(
                JsonFileLayoutStateStore::new(layout_path),
            ));
            match core.load_plugins_from_directory(app_data_dir.join("plugins")) {
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
                        "id": "sample.reader",
                        "version": "1.0.0",
                        "hostApi": "^1",
                        "runtime": "built-in",
                        "launch": null,
                        "targetApp": null,
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
                response: Response::PluginProjection(velvet_plugin_api::ExtensionProjection {
                    summary: velvet_plugin_api::ExtensionSummary {
                        id: velvet_plugin_api::ExtensionId("sample.reader".into()),
                        version: "1.0.0".into(),
                        active: true,
                        state: velvet_plugin_api::ExtensionState::Active,
                    },
                    target_app: None,
                    contributions: vec![velvet_plugin_api::ContributionDescriptor {
                        id: "sample.reader.view".into(),
                        kind: velvet_plugin_api::ContributionKind::View,
                        scope: velvet_plugin_api::ContributionScope::Workbench,
                        namespace: "workbench".into(),
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
                    }],
                }),
            },
        );
        let value = serde_json::to_value(response).expect("plugin projection should serialize");

        assert_eq!(value["response"]["kind"], "plugin-projection");
        assert_eq!(
            value["response"]["projection"]["contributions"][0]["ui"]["entrypoint"],
            "wasm://sample/reader"
        );
        assert_eq!(
            value["response"]["projection"]["contributions"][0]["namespace"],
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
}
