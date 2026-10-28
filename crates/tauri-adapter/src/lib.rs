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
        let response = self.inner.dispatch(request_id, request)?;

        if emits_layout_event {
            if let Response::Layout(state) = &response {
                self.event_sink.emit(Event::LayoutChanged {
                    revision: state.revision,
                    state: state.clone(),
                });
            }
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_layout_model::{LayoutCommand, WorkbenchPart};

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
}
