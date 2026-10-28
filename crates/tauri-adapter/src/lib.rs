use velvet_application_core::{ApplicationCore, InProcessCoreTransport};
use velvet_workbench_protocol::{CoreError, CoreTransport, Request, RequestId, Response};

/// Adapter seam for Tauri commands and events.
///
/// The actual `tauri::State` and command macros belong in the desktop app.
/// Keeping this adapter free of Tauri types lets protocol tests run without a webview.
#[derive(Clone)]
pub struct TauriTransport {
    inner: InProcessCoreTransport,
}

impl TauriTransport {
    pub fn new(core: ApplicationCore) -> Self {
        Self {
            inner: InProcessCoreTransport::new(core),
        }
    }
}

impl CoreTransport for TauriTransport {
    fn dispatch(&self, request_id: RequestId, request: Request) -> Result<Response, CoreError> {
        self.inner.dispatch(request_id, request)
    }
}
