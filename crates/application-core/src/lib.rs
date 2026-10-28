use std::sync::{Arc, Mutex};

use velvet_asset_core::ContentKindId;
use velvet_layout_model::{InMemoryLayoutService, LayoutService};
use velvet_persistence::{InMemoryLayoutStateStore, LayoutStateStore};
use velvet_plugin_host::{InMemoryPluginHost, PluginHost};
use velvet_preview_core::{GenericFallbackProvider, PreviewContext, PreviewResolver};
use velvet_workbench_protocol::{
    Command, CoreError, CoreTransport, Query, Request, RequestId, Response,
};

pub struct ApplicationCore {
    layout: InMemoryLayoutService,
    plugins: InMemoryPluginHost,
    previews: PreviewResolver,
    layout_store: Arc<dyn LayoutStateStore>,
}

impl Default for ApplicationCore {
    fn default() -> Self {
        let layout_store = Arc::new(InMemoryLayoutStateStore::default());
        let mut previews = PreviewResolver::default();
        previews.register(Arc::new(GenericFallbackProvider));

        Self {
            layout: InMemoryLayoutService::default(),
            plugins: InMemoryPluginHost::default(),
            previews,
            layout_store,
        }
    }
}

impl ApplicationCore {
    pub fn with_store(layout_store: Arc<dyn LayoutStateStore>) -> Self {
        let mut core = Self {
            layout: InMemoryLayoutService::default(),
            plugins: InMemoryPluginHost::default(),
            previews: PreviewResolver::default(),
            layout_store,
        };

        if let Ok(Some(state)) = core.layout_store.load() {
            core.layout = InMemoryLayoutService::from_state(state);
        }

        core.previews.register(Arc::new(GenericFallbackProvider));
        core
    }

    pub fn dispatch(&mut self, request: Request) -> Result<Response, CoreError> {
        match request {
            Request::Command(Command::Layout(command)) => {
                let state = self
                    .layout
                    .apply(command)
                    .map_err(|error| CoreError::Layout(format!("{error:?}")))?;
                self.layout_store
                    .save(&state)
                    .map_err(|error| CoreError::Layout(format!("{error:?}")))?;
                Ok(Response::Layout(state))
            }
            Request::Query(Query::Layout) => Ok(Response::Layout(self.layout.snapshot())),
            Request::Query(Query::Extensions) => Ok(Response::Extensions(self.plugins.discover())),
            Request::Query(Query::ResolvePreview {
                asset_id,
                content_kind,
                intent,
                surface,
            }) => self
                .previews
                .resolve(velvet_preview_core::PreviewRequest {
                    context: PreviewContext {
                        asset_id,
                        content_kind: ContentKindId(content_kind),
                        intent,
                        surface,
                    },
                })
                .map(Response::Preview)
                .map_err(|error| CoreError::InvalidCommand(format!("{error:?}"))),
        }
    }
}

#[derive(Clone)]
pub struct InProcessCoreTransport {
    core: Arc<Mutex<ApplicationCore>>,
}

impl InProcessCoreTransport {
    pub fn new(core: ApplicationCore) -> Self {
        Self {
            core: Arc::new(Mutex::new(core)),
        }
    }
}

impl CoreTransport for InProcessCoreTransport {
    fn dispatch(&self, _request_id: RequestId, request: Request) -> Result<Response, CoreError> {
        self.core
            .lock()
            .map_err(|_| CoreError::InvalidCommand("core lock poisoned".into()))?
            .dispatch(request)
    }
}
