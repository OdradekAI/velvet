use std::sync::{Arc, Mutex};

use velvet_application_core::{ApplicationCore, InProcessCoreTransport};
use velvet_workbench_protocol::{
    Command, CoreError, CoreTransport, Event, Request, RequestId, Response,
};

pub trait CoreEventSink: Send + Sync {
    fn emit(&self, event: Event);
}

#[derive(Clone, Default)]
pub struct InMemoryEventSink {
    events: Arc<Mutex<Vec<Event>>>,
}

impl InMemoryEventSink {
    pub fn events(&self) -> Vec<Event> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }
}

impl CoreEventSink for InMemoryEventSink {
    fn emit(&self, event: Event) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

/// Adapter seam for Tauri commands and events.
///
/// The actual `tauri::State` and command macros belong in the desktop app.
/// Keeping this adapter free of Tauri types lets protocol tests run without a webview.
#[derive(Clone)]
pub struct TauriTransport {
    inner: InProcessCoreTransport,
    event_sink: Arc<dyn CoreEventSink>,
}

impl TauriTransport {
    pub fn new(core: ApplicationCore) -> Self {
        Self::with_event_sink(core, InMemoryEventSink::default())
    }

    pub fn with_event_sink<S>(core: ApplicationCore, event_sink: S) -> Self
    where
        S: CoreEventSink + 'static,
    {
        Self {
            inner: InProcessCoreTransport::new(core),
            event_sink: Arc::new(event_sink),
        }
    }
}

impl CoreTransport for TauriTransport {
    fn dispatch(&self, request_id: RequestId, request: Request) -> Result<Response, CoreError> {
        let emits_layout_event = matches!(
            &request,
            Request::Command(Command::Layout(_)) | Request::Command(Command::LayoutAt { .. })
        );
        let disabled_extension = match &request {
            Request::Command(Command::DisablePlugin {
                extension_id,
                reason,
            }) => Some((extension_id.0.clone(), disable_reason_name(reason))),
            _ => None,
        };
        let response = self.inner.dispatch(request_id, request)?;

        if emits_layout_event {
            if let Response::Layout(state) = &response {
                self.event_sink.emit(Event::LayoutChanged {
                    revision: state.revision,
                    state: state.clone(),
                });
            }
        }

        if let (Some((extension_id, reason)), Response::Extension(summary)) =
            (disabled_extension, &response)
        {
            if !summary.active {
                self.event_sink.emit(Event::PluginDisabled {
                    extension_id,
                    reason,
                });
            }
        }

        Ok(response)
    }
}

fn disable_reason_name(reason: &velvet_workbench_protocol::PluginDisableReason) -> String {
    match reason {
        velvet_workbench_protocol::PluginDisableReason::User => "user".into(),
        velvet_workbench_protocol::PluginDisableReason::Crash => "crash".into(),
        velvet_workbench_protocol::PluginDisableReason::Incompatible => "incompatible".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_layout_model::{LayoutCommand, WorkbenchPart};
    use velvet_plugin_api::{ExtensionId, ExtensionManifest, RuntimeKind, VersionRange};
    use velvet_workbench_protocol::PluginDisableReason;

    #[test]
    fn successful_layout_commands_emit_authoritative_events() {
        let sink = InMemoryEventSink::default();
        let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
        let response = transport
            .dispatch(
                RequestId(1),
                Request::Command(Command::LayoutAt {
                    expected_revision: Some(0),
                    command: LayoutCommand::SetPartVisibility {
                        part: WorkbenchPart::PrimarySidebar,
                        visible: false,
                    },
                }),
            )
            .expect("layout command should succeed");

        assert!(matches!(response, Response::Layout(state) if state.revision == 1));
        assert!(matches!(
            sink.events().as_slice(),
            [Event::LayoutChanged { revision: 1, .. }]
        ));
    }

    #[test]
    fn rejected_layout_commands_do_not_emit_events() {
        let sink = InMemoryEventSink::default();
        let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
        let error = transport
            .dispatch(
                RequestId(1),
                Request::Command(Command::LayoutAt {
                    expected_revision: Some(9),
                    command: LayoutCommand::ResetLayout,
                }),
            )
            .expect_err("stale layout command should fail");

        assert!(matches!(error, CoreError::StaleRevision { .. }));
        assert!(sink.events().is_empty());
    }

    #[test]
    fn disabling_a_plugin_emits_a_lifecycle_event() {
        let sink = InMemoryEventSink::default();
        let mut core = ApplicationCore::default();
        let manifest = ExtensionManifest {
            id: ExtensionId("sample.reader".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::BuiltIn,
            launch: None,
            target_app: None,
            contributions: vec![],
            permission_sets: vec![],
        };
        let id = manifest.id.clone();
        core.register_plugin(manifest)
            .expect("plugin should register");
        core.activate_plugin(&id).expect("plugin should activate");
        let transport = TauriTransport::with_event_sink(core, sink.clone());

        transport
            .dispatch(
                RequestId(2),
                Request::Command(Command::DisablePlugin {
                    extension_id: id,
                    reason: PluginDisableReason::User,
                }),
            )
            .expect("plugin disable should succeed");

        assert!(matches!(
            sink.events().as_slice(),
            [Event::PluginDisabled { extension_id, reason }] if extension_id == "sample.reader"
                && reason == "user"
        ));
    }
}
