use std::sync::{Arc, Mutex};

use velvet_asset_core::ContentKindId;
use velvet_layout_model::{InMemoryLayoutService, LayoutService};
use velvet_persistence::{InMemoryLayoutStateStore, LayoutStateStore};
use velvet_plugin_api::{ExtensionId, ExtensionManifest, ExtensionState};
use velvet_plugin_host::{
    DisableReason, InMemoryPluginHost, PluginError, PluginHost, PluginRuntime,
};
use velvet_preview_core::{GenericFallbackProvider, PreviewContext, PreviewResolver};
use velvet_workbench_protocol::{
    Command, CoreError, CoreTransport, Query, Request, RequestEnvelope, RequestId, Response,
    ResponseEnvelope, PROTOCOL_VERSION,
};

pub struct ApplicationCore {
    layout: InMemoryLayoutService,
    plugins: InMemoryPluginHost,
    previews: PreviewResolver,
    layout_store: Arc<dyn LayoutStateStore>,
}

impl Default for ApplicationCore {
    fn default() -> Self {
        Self::with_store(Arc::new(InMemoryLayoutStateStore::default()))
    }
}

impl ApplicationCore {
    pub fn with_store(layout_store: Arc<dyn LayoutStateStore>) -> Self {
        Self::with_store_and_runtime(
            layout_store,
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
        )
    }

    pub fn with_plugin_runtime(
        layout_store: Arc<dyn LayoutStateStore>,
        runtime: Box<dyn PluginRuntime>,
    ) -> Self {
        Self::with_store_and_runtime(layout_store, runtime)
    }

    fn with_store_and_runtime(
        layout_store: Arc<dyn LayoutStateStore>,
        runtime: Box<dyn PluginRuntime>,
    ) -> Self {
        let mut layout = InMemoryLayoutService::default();
        if let Ok(Some(state)) = layout_store.load() {
            if let Ok(migrated) = layout.migrate(state) {
                layout = InMemoryLayoutService::from_state(migrated);
            }
        }

        let mut core = Self {
            layout,
            plugins: InMemoryPluginHost::with_runtime(runtime),
            previews: PreviewResolver::default(),
            layout_store,
        };

        core.previews.register(Arc::new(GenericFallbackProvider));
        core
    }

    pub fn register_plugin(&mut self, manifest: ExtensionManifest) -> Result<(), PluginError> {
        self.plugins.register(manifest)
    }

    pub fn activate_plugin(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        self.plugins.activate(id)
    }

    pub fn disable_plugin(
        &mut self,
        id: &ExtensionId,
        reason: DisableReason,
    ) -> Result<(), PluginError> {
        self.plugins.disable(id, reason)
    }

    pub fn check_plugin_health(&mut self, id: &ExtensionId) -> Result<ExtensionState, PluginError> {
        self.plugins.check_health(id)
    }

    pub fn recover_plugin(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        self.plugins.recover(id)
    }

    pub fn dispatch(&mut self, request: Request) -> Result<Response, CoreError> {
        match request {
            Request::Command(Command::Layout(command)) => self.apply_layout(None, command),
            Request::Command(Command::LayoutAt {
                expected_revision,
                command,
            }) => self.apply_layout(expected_revision, command),
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

    pub fn dispatch_envelope(
        &mut self,
        envelope: RequestEnvelope,
    ) -> Result<ResponseEnvelope, CoreError> {
        if envelope.protocol_version != PROTOCOL_VERSION {
            return Err(CoreError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                received: envelope.protocol_version,
            });
        }

        let request_id = envelope.request_id.clone();
        let response = self.dispatch(envelope.request)?;
        Ok(ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            response,
        })
    }

    fn apply_layout(
        &mut self,
        expected_revision: Option<u64>,
        command: velvet_layout_model::LayoutCommand,
    ) -> Result<Response, CoreError> {
        let actual_revision = self.layout.snapshot().revision;
        if let Some(expected) = expected_revision {
            if expected != actual_revision {
                return Err(CoreError::StaleRevision {
                    expected,
                    actual: actual_revision,
                });
            }
        }

        let mut next_layout = self.layout.clone();
        let state = next_layout
            .apply(command)
            .map_err(|error| CoreError::Layout(format!("{error:?}")))?;
        self.layout_store
            .save(&state)
            .map_err(|error| CoreError::Layout(format!("{error:?}")))?;
        self.layout = next_layout;
        Ok(Response::Layout(state))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use velvet_layout_model::{LayoutCommand, WorkbenchPart};
    use velvet_persistence::JsonFileLayoutStateStore;
    use velvet_plugin_api::{ExtensionId, ExtensionManifest, RuntimeKind, VersionRange};

    static NEXT_TEST_FILE: AtomicU64 = AtomicU64::new(0);

    fn temporary_layout_path() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let sequence = NEXT_TEST_FILE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "velvet-application-core-restart-{}-{nonce}-{sequence}.json",
            std::process::id(),
        ))
    }

    #[test]
    fn layout_queries_return_the_canonical_revisioned_snapshot() {
        let mut core = ApplicationCore::default();
        let response = core
            .dispatch_envelope(RequestEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(1),
                request: Request::Query(Query::Layout),
            })
            .expect("layout query should succeed");

        assert_eq!(response.protocol_version, PROTOCOL_VERSION);
        assert_eq!(response.request_id, RequestId(1));
        match response.response {
            Response::Layout(state) => {
                assert_eq!(state.revision, 0);
                assert_eq!(
                    state.active_app,
                    velvet_layout_model::WorkbenchAppId::AssetManager
                );
            }
            other => panic!("expected layout response, got {other:?}"),
        }
    }

    #[test]
    fn layout_commands_require_the_current_revision() {
        let mut core = ApplicationCore::default();
        let command = Request::Command(Command::LayoutAt {
            expected_revision: Some(0),
            command: LayoutCommand::SetPartVisibility {
                part: WorkbenchPart::PrimarySidebar,
                visible: false,
            },
        });

        let first = core
            .dispatch(command.clone())
            .expect("first command should succeed");
        match first {
            Response::Layout(state) => assert_eq!(state.revision, 1),
            other => panic!("expected layout response, got {other:?}"),
        }

        let stale = core
            .dispatch(command)
            .expect_err("stale command should be rejected");
        assert_eq!(
            stale,
            CoreError::StaleRevision {
                expected: 0,
                actual: 1,
            }
        );
    }

    #[test]
    fn envelopes_reject_unknown_protocol_versions_before_dispatch() {
        let mut core = ApplicationCore::default();
        let error = core
            .dispatch_envelope(RequestEnvelope {
                protocol_version: PROTOCOL_VERSION + 1,
                request_id: RequestId(2),
                request: Request::Query(Query::Layout),
            })
            .expect_err("unknown protocol versions should be rejected");

        assert_eq!(
            error,
            CoreError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                received: PROTOCOL_VERSION + 1,
            }
        );
    }

    #[test]
    fn layout_state_is_restored_when_application_core_restarts() {
        let path = temporary_layout_path();
        let first_store = Arc::new(JsonFileLayoutStateStore::new(&path));
        let mut first_core = ApplicationCore::with_store(first_store);

        first_core
            .dispatch(Request::Command(Command::Layout(
                LayoutCommand::SetPartVisibility {
                    part: WorkbenchPart::PrimarySidebar,
                    visible: false,
                },
            )))
            .expect("the first core should persist the layout change");
        drop(first_core);

        let second_store = Arc::new(JsonFileLayoutStateStore::new(&path));
        let mut second_core = ApplicationCore::with_store(second_store);
        let response = second_core
            .dispatch(Request::Query(Query::Layout))
            .expect("the restarted core should load the persisted layout");

        match response {
            Response::Layout(state) => {
                assert_eq!(state.revision, 1);
                assert!(
                    !state
                        .parts
                        .get(&WorkbenchPart::PrimarySidebar)
                        .expect("primary sidebar should exist")
                        .visible
                );
            }
            other => panic!("expected a layout response, got {other:?}"),
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn native_app_switch_is_restored_after_restart() {
        let path = temporary_layout_path();
        let first_store = Arc::new(JsonFileLayoutStateStore::new(&path));
        let mut first_core = ApplicationCore::with_store(first_store);

        first_core
            .dispatch(Request::Command(Command::Layout(
                LayoutCommand::ActivateApp(velvet_layout_model::WorkbenchAppId::FeedWorkbench),
            )))
            .expect("the native app switch should persist");
        drop(first_core);

        let second_store = Arc::new(JsonFileLayoutStateStore::new(&path));
        let mut second_core = ApplicationCore::with_store(second_store);
        let response = second_core
            .dispatch(Request::Query(Query::Layout))
            .expect("the restarted core should load the active app");

        match response {
            Response::Layout(state) => assert_eq!(
                state.active_app,
                velvet_layout_model::WorkbenchAppId::FeedWorkbench
            ),
            other => panic!("expected a layout response, got {other:?}"),
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn plugin_view_container_visibility_and_order_are_restored_after_restart() {
        let path = temporary_layout_path();
        let first_store = Arc::new(JsonFileLayoutStateStore::new(&path));
        let mut first_core = ApplicationCore::with_store(first_store);

        first_core
            .dispatch(Request::Command(Command::Layout(
                LayoutCommand::UpsertViewContainer {
                    container_id: "plugin.library".into(),
                    visible: false,
                    order: 4,
                },
            )))
            .expect("the plugin container should be persisted");
        drop(first_core);

        let second_store = Arc::new(JsonFileLayoutStateStore::new(&path));
        let mut second_core = ApplicationCore::with_store(second_store);
        let response = second_core
            .dispatch(Request::Query(Query::Layout))
            .expect("the restarted core should restore plugin layout state");

        match response {
            Response::Layout(state) => {
                let container = state
                    .containers
                    .get("plugin.library")
                    .expect("plugin container should survive restart");
                assert!(!container.visible);
                assert_eq!(container.order, 4);
            }
            other => panic!("expected a layout response, got {other:?}"),
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn application_core_owns_plugin_runtime_lifecycle() {
        let mut core = ApplicationCore::default();
        let manifest = ExtensionManifest {
            id: ExtensionId("builtin.reader".into()),
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
            .expect("the core should register a plugin manifest");
        core.activate_plugin(&id)
            .expect("the core should start the plugin runtime");
        let response = core
            .dispatch(Request::Query(Query::Extensions))
            .expect("the core should expose plugin state");
        match response {
            Response::Extensions(extensions) => {
                assert_eq!(
                    extensions[0].state,
                    velvet_plugin_api::ExtensionState::Active
                );
                assert!(extensions[0].active);
            }
            other => panic!("expected extension response, got {other:?}"),
        }

        core.disable_plugin(&id, DisableReason::User)
            .expect("the core should stop the plugin runtime");
        let response = core
            .dispatch(Request::Query(Query::Extensions))
            .expect("the core should expose disabled plugin state");
        match response {
            Response::Extensions(extensions) => {
                assert_eq!(
                    extensions[0].state,
                    velvet_plugin_api::ExtensionState::Disabled
                );
                assert!(!extensions[0].active);
            }
            other => panic!("expected extension response, got {other:?}"),
        }
    }
}
