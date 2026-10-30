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
        let response = match self.inner.dispatch(request_id, request) {
            Ok(response) => response,
            Err(error) => {
                if let CoreError::PluginRuntimeFailed { extension_id, .. } = &error {
                    self.event_sink.emit(Event::PluginDisabled {
                        extension_id: extension_id.clone(),
                        reason: "crash".into(),
                    });
                }
                return Err(error);
            }
        };

        for snapshot in self.inner.drain_job_events() {
            self.event_sink.emit(Event::JobChanged { snapshot });
        }

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
    use velvet_plugin_api::{
        ContributionDescriptor, ContributionKind, ContributionScope, ExtensionId,
        ExtensionManifest, RuntimeKind, RuntimeLaunchSpec, VersionRange,
    };
    use velvet_workbench_protocol::{PluginDisableReason, Query};

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
            ..ExtensionManifest::default()
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

    #[test]
    fn runtime_command_failure_emits_a_crash_event_for_frontend_cleanup() {
        let sink = InMemoryEventSink::default();
        let mut core = ApplicationCore::default();
        let manifest = ExtensionManifest {
            id: ExtensionId("sample.crashing-sidecar".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Sidecar,
            launch: Some(RuntimeLaunchSpec {
                program: "node".into(),
                args: vec![concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../examples/plugins/echo-sidecar/sidecar.mjs"
                )
                .into()],
            }),
            target_app: None,
            contributions: vec![ContributionDescriptor {
                id: "echo.crash".into(),
                kind: ContributionKind::Command,
                scope: ContributionScope::Workbench,
                namespace: Some("workbench".into()),
                container: None,
                zone: None,
                title: Some("Crash Echo Runtime".into()),
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
                anchor: None,
                ui: None,
                preview: None,
            }],
            permission_sets: vec![],
            ..ExtensionManifest::default()
        };
        let id = manifest.id.clone();
        core.register_plugin(manifest)
            .expect("the crashing sidecar should register");
        core.activate_plugin(&id)
            .expect("the crashing sidecar should start");
        let transport = TauriTransport::with_event_sink(core, sink.clone());

        let error = transport
            .dispatch(
                RequestId(3),
                Request::Command(Command::InvokePluginCommand {
                    extension_id: id.clone(),
                    command_id: "echo.crash".into(),
                    args: serde_json::Value::Null,
                    context: serde_json::Value::Null,
                }),
            )
            .expect_err("the crashing sidecar should fail its command");
        assert!(matches!(
            error,
            CoreError::PluginRuntimeFailed { extension_id, .. } if extension_id == id.0
        ));
        assert!(matches!(
            sink.events().as_slice(),
            [Event::PluginDisabled { extension_id, reason }] if extension_id == "sample.crashing-sidecar"
                && reason == "crash"
        ));
    }

    #[test]
    fn job_commands_emit_progress_and_terminal_events() {
        let sink = InMemoryEventSink::default();
        let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
        let response = transport
            .dispatch(
                RequestId(4),
                Request::Command(Command::CreateJob {
                    kind: "sample".into(),
                    duration_ms: 200,
                    will_fail: false,
                }),
            )
            .expect("job should be accepted");
        let job_id = match response {
            Response::Job(snapshot) => snapshot.id,
            other => panic!("expected job response, got {other:?}"),
        };
        std::thread::sleep(std::time::Duration::from_millis(80));
        let _ = transport.dispatch(RequestId(5), Request::Query(Query::Jobs));
        let events = sink.events();
        assert!(events.iter().any(|event| matches!(event, Event::JobChanged { snapshot } if snapshot.id == job_id && snapshot.progress > 0)));
        for request_id in 6..=20 {
            std::thread::sleep(std::time::Duration::from_millis(40));
            let _ = transport.dispatch(RequestId(request_id), Request::Query(Query::Jobs));
            if sink.events().iter().any(|event| matches!(event, Event::JobChanged { snapshot } if snapshot.id == job_id && snapshot.status == velvet_job_system::JobStatus::Completed)) {
                return;
            }
        }
        panic!("job never emitted a completed event: {:?}", sink.events());
    }
}
