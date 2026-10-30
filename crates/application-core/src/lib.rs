use std::sync::{Arc, Mutex};

use velvet_asset_core::ContentKindId;
use velvet_job_system::{JobError, JobId, JobSpec, JobStateStore, JobSystem};
use velvet_layout_model::{InMemoryLayoutService, LayoutService};
use velvet_persistence::{InMemoryLayoutStateStore, LayoutStateStore};
use velvet_plugin_api::{
    ExtensionId, ExtensionInstallPreview, ExtensionInstallRecovery, ExtensionInstallRecoveryAction,
    ExtensionInstallRecoveryLogEntry, ExtensionManifest, ExtensionState,
};
use velvet_plugin_host::{
    DisableReason, InMemoryPluginHost, JsonManifestDiscovery, ManifestDiscoveryError,
    ManifestLoadReport, PluginError, PluginHost, PluginRuntime,
};
use velvet_preview_core::{GenericFallbackProvider, PreviewContext, PreviewResolver};
use velvet_workbench_protocol::{
    Command, CoreError, CoreTransport, PluginDisableReason, PluginInstallPreviewDto,
    PluginProjectionDto, Query, Request, RequestEnvelope, RequestId, Response, ResponseEnvelope,
    PROTOCOL_VERSION,
};

pub struct ApplicationCore {
    layout: InMemoryLayoutService,
    plugins: InMemoryPluginHost,
    previews: PreviewResolver,
    jobs: JobSystem,
    layout_store: Arc<dyn LayoutStateStore>,
    plugin_directory: Option<std::path::PathBuf>,
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

    pub fn with_store_and_job_store(
        layout_store: Arc<dyn LayoutStateStore>,
        job_store: Arc<dyn JobStateStore>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs(
            layout_store,
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            None,
            JobSystem::with_store(job_store),
        )
    }

    pub fn with_plugin_runtime(
        layout_store: Arc<dyn LayoutStateStore>,
        runtime: Box<dyn PluginRuntime>,
    ) -> Self {
        Self::with_store_and_runtime(layout_store, runtime)
    }

    pub fn with_store_and_plugin_directory(
        layout_store: Arc<dyn LayoutStateStore>,
        plugin_directory: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory(
            layout_store,
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            Some(plugin_directory.into()),
        )
    }

    pub fn with_store_and_plugin_directory_and_job_store(
        layout_store: Arc<dyn LayoutStateStore>,
        plugin_directory: impl Into<std::path::PathBuf>,
        job_store: Arc<dyn JobStateStore>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs(
            layout_store,
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            Some(plugin_directory.into()),
            JobSystem::with_store(job_store),
        )
    }

    fn with_store_and_runtime(
        layout_store: Arc<dyn LayoutStateStore>,
        runtime: Box<dyn PluginRuntime>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory(layout_store, runtime, None)
    }

    fn with_store_and_runtime_and_plugin_directory(
        layout_store: Arc<dyn LayoutStateStore>,
        runtime: Box<dyn PluginRuntime>,
        plugin_directory: Option<std::path::PathBuf>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs(
            layout_store,
            runtime,
            plugin_directory,
            JobSystem::default(),
        )
    }

    fn with_store_and_runtime_and_plugin_directory_and_jobs(
        layout_store: Arc<dyn LayoutStateStore>,
        runtime: Box<dyn PluginRuntime>,
        plugin_directory: Option<std::path::PathBuf>,
        jobs: JobSystem,
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
            jobs,
            layout_store,
            plugin_directory,
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

    pub fn discover_plugins(&self) -> Vec<velvet_plugin_api::ExtensionSummary> {
        self.plugins.discover()
    }

    /// Starts manifests discovered during native application startup. Other
    /// embedders may keep the host lazy and call `activate_plugin` explicitly.
    pub fn activate_registered_plugins(&mut self) -> Vec<PluginError> {
        let recovery_ids = self
            .inspect_plugin_install_recoveries()
            .unwrap_or_default()
            .into_iter()
            .map(|recovery| recovery.extension_id)
            .collect::<Vec<_>>();
        self.plugins
            .discover()
            .into_iter()
            .filter(|summary| summary.state == ExtensionState::Registered)
            .filter(|summary| !recovery_ids.contains(&summary.id))
            .filter_map(|summary| self.plugins.activate(&summary.id).err())
            .collect()
    }

    pub fn load_plugins_from_directory(
        &mut self,
        root: impl AsRef<std::path::Path>,
    ) -> Result<ManifestLoadReport, ManifestDiscoveryError> {
        JsonManifestDiscovery::new(root.as_ref()).load_into(&mut self.plugins)
    }

    pub fn install_plugin_from_directory(
        &mut self,
        source_dir: impl AsRef<std::path::Path>,
    ) -> Result<velvet_plugin_api::ExtensionSummary, PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Err(PluginError::InstallFailed(
                "plugin installation is unavailable without an application plugin directory".into(),
            ));
        };
        self.plugins
            .install_from_directory(source_dir.as_ref(), plugin_directory)
    }

    pub fn inspect_plugin_from_directory(
        &self,
        source_dir: impl AsRef<std::path::Path>,
    ) -> Result<ExtensionInstallPreview, PluginError> {
        self.plugins.inspect_from_directory(source_dir.as_ref())
    }

    pub fn commit_plugin_install(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Err(PluginError::InstallFailed(
                "plugin installation is unavailable without an application plugin directory".into(),
            ));
        };
        self.plugins.commit_install(id, plugin_directory)
    }

    pub fn rollback_plugin_install(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Err(PluginError::InstallFailed(
                "plugin installation is unavailable without an application plugin directory".into(),
            ));
        };
        self.plugins.rollback_install(id, plugin_directory)
    }

    pub fn inspect_plugin_install_recoveries(
        &self,
    ) -> Result<Vec<ExtensionInstallRecovery>, PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Ok(Vec::new());
        };
        self.plugins.inspect_install_recoveries(plugin_directory)
    }

    pub fn resolve_plugin_install_recovery(
        &mut self,
        id: &ExtensionId,
        action: ExtensionInstallRecoveryAction,
    ) -> Result<(), PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Err(PluginError::InstallFailed(
                "plugin installation recovery is unavailable without an application plugin directory".into(),
            ));
        };
        self.plugins
            .resolve_install_recovery(id, action, plugin_directory)
    }

    pub fn inspect_plugin_install_recovery_log(
        &self,
    ) -> Result<Vec<ExtensionInstallRecoveryLogEntry>, PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Ok(Vec::new());
        };
        self.plugins.inspect_install_recovery_log(plugin_directory)
    }

    pub fn cleanup_plugin_install_recovery_orphans(&mut self) -> Result<(), PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Ok(());
        };
        self.plugins
            .cleanup_install_recovery_orphans(plugin_directory)
    }

    pub fn clear_plugin_install_recovery_log(&mut self) -> Result<(), PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Ok(());
        };
        self.plugins.clear_install_recovery_log(plugin_directory)
    }

    pub fn export_plugin_install_recovery_log(
        &self,
        destination: impl AsRef<std::path::Path>,
    ) -> Result<(), PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Err(PluginError::InstallFailed(
                "plugin recovery log export is unavailable without an application plugin directory"
                    .into(),
            ));
        };
        self.plugins
            .export_install_recovery_log(plugin_directory, destination.as_ref())
    }

    pub fn create_job(
        &self,
        kind: String,
        duration_ms: u64,
        will_fail: bool,
    ) -> Result<velvet_job_system::JobSnapshot, JobError> {
        self.jobs.submit(JobSpec {
            kind,
            duration_ms,
            will_fail,
        })
    }

    pub fn cancel_job(&self, id: &JobId) -> Result<velvet_job_system::JobSnapshot, JobError> {
        self.jobs.cancel(id)
    }

    pub fn retry_job(&self, id: &JobId) -> Result<velvet_job_system::JobSnapshot, JobError> {
        self.jobs.retry(id)
    }

    pub fn job_snapshots(&self) -> Vec<velvet_job_system::JobSnapshot> {
        self.jobs.snapshots()
    }

    pub fn drain_job_events(&self) -> Vec<velvet_job_system::JobSnapshot> {
        self.jobs.drain_events()
    }

    pub fn uninstall_plugin(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Err(PluginError::UninstallFailed(
                "plugin uninstallation is unavailable without an application plugin directory"
                    .into(),
            ));
        };
        self.plugins.uninstall_from_directory(id, plugin_directory)
    }

    pub fn dispatch(&mut self, request: Request) -> Result<Response, CoreError> {
        match request {
            Request::Command(Command::Layout(command)) => self.apply_layout(None, command),
            Request::Command(Command::LayoutAt {
                expected_revision,
                command,
            }) => self.apply_layout(expected_revision, command),
            Request::Command(Command::RegisterPlugin { manifest }) => {
                let manifest: ExtensionManifest = (*manifest).into();
                let id = manifest.id.clone();
                self.register_plugin(manifest).map_err(plugin_error)?;
                self.extension_response(&id)
            }
            Request::Command(Command::ActivatePlugin { extension_id }) => {
                self.activate_plugin(&extension_id).map_err(plugin_error)?;
                self.extension_response(&extension_id)
            }
            Request::Command(Command::DisablePlugin {
                extension_id,
                reason,
            }) => {
                self.disable_plugin(&extension_id, disable_reason(reason))
                    .map_err(plugin_error)?;
                self.extension_response(&extension_id)
            }
            Request::Command(Command::CheckPluginHealth { extension_id }) => {
                self.check_plugin_health(&extension_id)
                    .map_err(plugin_error)?;
                self.extension_response(&extension_id)
            }
            Request::Command(Command::RecoverPlugin { extension_id }) => {
                self.recover_plugin(&extension_id).map_err(plugin_error)?;
                self.extension_response(&extension_id)
            }
            Request::Command(Command::InstallPlugin { source_dir }) => self
                .install_plugin_from_directory(source_dir)
                .map(Response::Extension)
                .map_err(plugin_error),
            Request::Command(Command::InspectPlugin { source_dir }) => self
                .inspect_plugin_from_directory(source_dir)
                .map(PluginInstallPreviewDto::from)
                .map(Response::PluginInstallPreview)
                .map_err(plugin_error),
            Request::Command(Command::CommitPluginInstall { extension_id }) => {
                self.commit_plugin_install(&extension_id)
                    .map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::RollbackPluginInstall { extension_id }) => {
                self.rollback_plugin_install(&extension_id)
                    .map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::ResolvePluginInstallRecovery {
                extension_id,
                action,
            }) => {
                self.resolve_plugin_install_recovery(&extension_id, action)
                    .map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::ClearPluginInstallRecoveryLog) => {
                self.clear_plugin_install_recovery_log()
                    .map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::ExportPluginInstallRecoveryLog { destination }) => {
                self.export_plugin_install_recovery_log(destination)
                    .map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::UninstallPlugin { extension_id }) => {
                self.uninstall_plugin(&extension_id).map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::InvokePluginCommand {
                extension_id,
                command_id,
                args,
                context,
            }) => {
                self.plugins
                    .invoke_command(&extension_id, command_id, args, context)
                    .map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::CreateJob {
                kind,
                duration_ms,
                will_fail,
            }) => self
                .create_job(kind, duration_ms, will_fail)
                .map(Response::Job)
                .map_err(job_error),
            Request::Command(Command::CancelJob { job_id }) => self
                .cancel_job(&job_id)
                .map(Response::Job)
                .map_err(job_error),
            Request::Command(Command::RetryJob { job_id }) => self
                .retry_job(&job_id)
                .map(Response::Job)
                .map_err(job_error),
            Request::Query(Query::Layout) => Ok(Response::Layout(self.layout.snapshot())),
            Request::Query(Query::Extensions) => Ok(Response::Extensions(self.plugins.discover())),
            Request::Query(Query::PluginInstallRecoveries) => self
                .inspect_plugin_install_recoveries()
                .map(Response::PluginInstallRecoveries)
                .map_err(plugin_error),
            Request::Query(Query::PluginInstallRecoveryLog) => self
                .inspect_plugin_install_recovery_log()
                .map(Response::PluginInstallRecoveryLog)
                .map_err(plugin_error),
            Request::Query(Query::Jobs) => Ok(Response::Jobs(self.job_snapshots())),
            Request::Query(Query::PluginProjection { extension_id }) => self
                .plugins
                .projection(&extension_id)
                .map(PluginProjectionDto::from)
                .map(Response::PluginProjection)
                .map_err(plugin_error),
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

    fn extension_response(&self, id: &ExtensionId) -> Result<Response, CoreError> {
        self.plugins
            .discover()
            .into_iter()
            .find(|summary| &summary.id == id)
            .map(Response::Extension)
            .ok_or_else(|| CoreError::Plugin(format!("extension disappeared: {}", id.0)))
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

fn disable_reason(reason: PluginDisableReason) -> DisableReason {
    match reason {
        PluginDisableReason::User => DisableReason::User,
        PluginDisableReason::Crash => DisableReason::Crash,
        PluginDisableReason::Incompatible => DisableReason::Incompatible,
    }
}

fn plugin_error(error: PluginError) -> CoreError {
    match error {
        PluginError::RuntimeInvocation {
            extension_id,
            message,
        } => CoreError::PluginRuntimeFailed {
            extension_id: extension_id.0,
            message,
        },
        other => CoreError::Plugin(format!("{other:?}")),
    }
}

fn job_error(error: JobError) -> CoreError {
    CoreError::Job(format!("{error:?}"))
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

    pub fn drain_job_events(&self) -> Vec<velvet_job_system::JobSnapshot> {
        self.core
            .lock()
            .map(|core| core.drain_job_events())
            .unwrap_or_default()
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
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use velvet_layout_model::{LayoutCommand, WorkbenchPart, WorkbenchZone};
    use velvet_persistence::JsonFileLayoutStateStore;
    use velvet_plugin_api::{
        ContributionDescriptor, ContributionKind, ContributionScope, ExtensionId,
        ExtensionInstallRecoveryLogStatus, ExtensionManifest, HostContainer, RuntimeKind,
        UiSurfaceDescriptor, UiSurfaceKind, VersionRange,
    };

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
            ..ExtensionManifest::default()
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

    #[test]
    fn plugin_lifecycle_commands_return_authoritative_extension_state() {
        let mut core = ApplicationCore::default();
        let manifest = ExtensionManifest {
            id: ExtensionId("command.reader".into()),
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

        let registered = core
            .dispatch(Request::Command(Command::RegisterPlugin {
                manifest: Box::new(manifest.into()),
            }))
            .expect("register command should succeed");
        assert!(matches!(
            registered,
            Response::Extension(summary)
                if summary.id == id && summary.state == velvet_plugin_api::ExtensionState::Registered
        ));

        let activated = core
            .dispatch(Request::Command(Command::ActivatePlugin {
                extension_id: id.clone(),
            }))
            .expect("activate command should succeed");
        assert!(matches!(
            activated,
            Response::Extension(summary)
                if summary.id == id && summary.state == velvet_plugin_api::ExtensionState::Active
        ));

        let disabled = core
            .dispatch(Request::Command(Command::DisablePlugin {
                extension_id: id.clone(),
                reason: PluginDisableReason::User,
            }))
            .expect("disable command should succeed");
        assert!(matches!(
            disabled,
            Response::Extension(summary)
                if summary.id == id && summary.state == velvet_plugin_api::ExtensionState::Disabled
        ));
    }

    #[test]
    fn plugin_command_dispatch_reaches_the_active_runtime() {
        let mut core = ApplicationCore::default();
        let manifest = ExtensionManifest {
            id: ExtensionId("command.runtime".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::BuiltIn,
            launch: None,
            target_app: None,
            contributions: vec![ContributionDescriptor {
                id: "command.runtime.open".into(),
                kind: ContributionKind::Command,
                scope: ContributionScope::Workbench,
                namespace: Some("workbench".into()),
                container: None,
                zone: None,
                title: Some("Open runtime command".into()),
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
            }],
            permission_sets: vec![],
            ..ExtensionManifest::default()
        };
        let id = manifest.id.clone();

        core.dispatch(Request::Command(Command::RegisterPlugin {
            manifest: Box::new(manifest.into()),
        }))
        .expect("command plugin should register");
        core.dispatch(Request::Command(Command::ActivatePlugin {
            extension_id: id.clone(),
        }))
        .expect("command plugin should activate");

        let response = core
            .dispatch(Request::Command(Command::InvokePluginCommand {
                extension_id: id,
                command_id: "command.runtime.open".into(),
                args: serde_json::json!({ "source": "test" }),
                context: serde_json::json!({
                    "activeAppId": "asset-manager",
                    "keys": { "resourceKind": "asset" }
                }),
            }))
            .expect("the active command should be accepted by the runtime");
        assert_eq!(response, Response::Accepted);
    }

    #[test]
    fn plugin_projection_query_returns_active_view_and_ui_bridge_metadata() {
        let mut core = ApplicationCore::default();
        let manifest = ExtensionManifest {
            id: ExtensionId("projection.reader".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::BuiltIn,
            launch: None,
            target_app: None,
            contributions: vec![ContributionDescriptor {
                id: "reader.view".into(),
                kind: ContributionKind::View,
                scope: ContributionScope::Workbench,
                namespace: Some("workbench".into()),
                container: Some(HostContainer::PrimarySidebar),
                zone: Some(WorkbenchZone::Navigation),
                title: Some("Reader".into()),
                title_key: None,
                icon: Some("book".into()),
                order: Some(2),
                default_visible: Some(true),
                view_container_id: None,
                activity_bar: None,
                label: None,
                label_key: None,
                command_id: None,
                group: None,
                when: None,
                anchor: None,
                ui: Some(UiSurfaceDescriptor {
                    kind: UiSurfaceKind::View,
                    entrypoint: "wasm://projection/reader".into(),
                    bridge_version: "1".into(),
                }),
            }],
            permission_sets: vec![],
            ..ExtensionManifest::default()
        };
        let id = manifest.id.clone();

        core.dispatch(Request::Command(Command::RegisterPlugin {
            manifest: Box::new(manifest.into()),
        }))
        .expect("projection plugin should register");
        core.dispatch(Request::Command(Command::ActivatePlugin {
            extension_id: id.clone(),
        }))
        .expect("projection plugin should activate");

        let response = core
            .dispatch(Request::Query(Query::PluginProjection { extension_id: id }))
            .expect("plugin projection query should succeed");
        assert!(matches!(
            response,
            Response::PluginProjection(projection)
                if projection.summary.state == velvet_plugin_api::ExtensionState::Active
                    && projection.manifest.contributions[0].namespace.as_deref() == Some("workbench")
                    && projection.manifest.contributions[0]
                        .ui
                        .as_ref()
                        .expect("projection should include UI metadata")
                        .entrypoint == "wasm://projection/reader"
        ));
    }

    #[test]
    fn application_core_discovers_local_plugin_manifests_without_activating_them() {
        let root = temporary_layout_path().with_extension("plugins");
        fs::create_dir_all(&root).expect("the plugin directory should be writable");
        let manifest = ExtensionManifest {
            id: ExtensionId("local.reader".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::BuiltIn,
            launch: None,
            target_app: None,
            contributions: vec![],
            permission_sets: vec![],
            ..ExtensionManifest::default()
        };
        fs::write(
            root.join("local-reader.json"),
            serde_json::to_vec(&manifest).expect("the manifest should serialize"),
        )
        .expect("the manifest should be written");

        let mut core = ApplicationCore::default();
        let report = core
            .load_plugins_from_directory(&root)
            .expect("the plugin directory should be scanned");
        assert_eq!(report.loaded.len(), 1);
        assert!(report.failures.is_empty());
        match core
            .dispatch(Request::Query(Query::Extensions))
            .expect("the core should expose discovered plugins")
        {
            Response::Extensions(extensions) => {
                assert_eq!(extensions[0].id, ExtensionId("local.reader".into()));
                assert_eq!(
                    extensions[0].state,
                    velvet_plugin_api::ExtensionState::Registered
                );
            }
            other => panic!("expected extension response, got {other:?}"),
        }

        fs::remove_dir_all(root).expect("the plugin directory should be removed");
    }

    #[test]
    fn application_core_holds_recovered_plugins_until_the_user_resolves_them() {
        let root = temporary_layout_path().with_extension("recovery-plugins");
        let id = ExtensionId("recovery.core".into());
        let current = ExtensionManifest {
            id: id.clone(),
            version: "0.2.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::BuiltIn,
            launch: None,
            target_app: None,
            contributions: vec![],
            permission_sets: vec![],
            ..ExtensionManifest::default()
        };
        let mut previous = current.clone();
        previous.version = "0.1.0".into();
        fs::create_dir_all(root.join(&id.0)).expect("the current package should be writable");
        fs::write(
            root.join(&id.0).join("manifest.json"),
            serde_json::to_vec(&current).expect("the current manifest should serialize"),
        )
        .expect("the current manifest should be written");
        fs::create_dir_all(root.join(format!(".{}.rollback-crash", id.0)))
            .expect("the rollback package should be writable");
        fs::write(
            root.join(format!(".{}.rollback-crash", id.0))
                .join("manifest.json"),
            serde_json::to_vec(&previous).expect("the previous manifest should serialize"),
        )
        .expect("the previous manifest should be written");

        let mut core = ApplicationCore::with_store_and_plugin_directory(
            Arc::new(InMemoryLayoutStateStore::default()),
            &root,
        );
        core.load_plugins_from_directory(&root)
            .expect("the current package should be discovered");
        assert!(core.activate_registered_plugins().is_empty());
        assert_eq!(core.discover_plugins()[0].state, ExtensionState::Registered);
        match core
            .dispatch(Request::Query(Query::PluginInstallRecoveries))
            .expect("recovery query should succeed")
        {
            Response::PluginInstallRecoveries(recoveries) => {
                assert_eq!(recoveries.len(), 1);
                assert!(recoveries[0].can_rollback);
            }
            other => panic!("expected plugin recovery response, got {other:?}"),
        }

        core.dispatch(Request::Command(Command::ResolvePluginInstallRecovery {
            extension_id: id.clone(),
            action: ExtensionInstallRecoveryAction::Rollback,
        }))
        .expect("the previous package should be restored");
        assert!(core.activate_registered_plugins().is_empty());
        assert_eq!(core.discover_plugins()[0].state, ExtensionState::Active);
        assert!(matches!(
            core.dispatch(Request::Query(Query::PluginInstallRecoveries))
                .expect("recovery query should be empty"),
            Response::PluginInstallRecoveries(recoveries) if recoveries.is_empty()
        ));
        match core
            .dispatch(Request::Query(Query::PluginInstallRecoveryLog))
            .expect("recovery log query should succeed")
        {
            Response::PluginInstallRecoveryLog(log) => {
                assert_eq!(log.len(), 1);
                assert_eq!(log[0].status, ExtensionInstallRecoveryLogStatus::Succeeded);
            }
            other => panic!("expected plugin recovery log response, got {other:?}"),
        }

        fs::remove_dir_all(root).expect("the recovery package should be removed");
    }

    #[test]
    fn application_core_installs_and_starts_a_plugin_after_restart() {
        let layout_path = temporary_layout_path();
        let plugin_root = layout_path.with_extension("plugins");
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/plugins/echo-sidecar");
        let store = Arc::new(JsonFileLayoutStateStore::new(&layout_path));
        let mut first_core =
            ApplicationCore::with_store_and_plugin_directory(store.clone(), &plugin_root);
        let installed = first_core
            .install_plugin_from_directory(&source)
            .expect("the application core should install the sample plugin");
        assert_eq!(
            installed.state,
            velvet_plugin_api::ExtensionState::Registered
        );
        drop(first_core);

        let mut restarted_core = ApplicationCore::with_store_and_plugin_directory(
            Arc::new(JsonFileLayoutStateStore::new(&layout_path)),
            &plugin_root,
        );
        let report = restarted_core
            .load_plugins_from_directory(&plugin_root)
            .expect("the restarted core should discover the installed package");
        assert_eq!(report.failures, Vec::new());
        assert_eq!(
            restarted_core.discover_plugins()[0].state,
            velvet_plugin_api::ExtensionState::Registered
        );
        assert!(restarted_core.activate_registered_plugins().is_empty());
        assert_eq!(
            restarted_core.discover_plugins()[0].state,
            velvet_plugin_api::ExtensionState::Active
        );

        restarted_core
            .uninstall_plugin(&ExtensionId("velvet.example.echo".into()))
            .expect("the application core should uninstall the package");
        assert!(restarted_core.discover_plugins().is_empty());
        assert!(!plugin_root.join("velvet.example.echo").exists());

        let _ = fs::remove_dir_all(plugin_root);
        let _ = fs::remove_file(layout_path);
    }

    #[test]
    fn application_core_exposes_interrupted_jobs_after_restart() {
        use velvet_job_system::{JobId, JobSnapshot, JobStatus};

        let layout_path = temporary_layout_path();
        let jobs_path = layout_path.with_extension("jobs.json");
        let job_store = Arc::new(velvet_persistence::JsonFileJobStateStore::new(&jobs_path));
        job_store
            .save(&[JobSnapshot {
                id: JobId("job-000041".into()),
                kind: "restartable".into(),
                status: JobStatus::Running,
                progress: 35,
                message: Some("Running".into()),
                attempt: 1,
                retryable: true,
                duration_ms: 30_000,
                will_fail: false,
            }])
            .expect("the fixture job should persist");

        let mut core = ApplicationCore::with_store_and_job_store(
            Arc::new(JsonFileLayoutStateStore::new(&layout_path)),
            job_store,
        );
        match core
            .dispatch(Request::Query(Query::Jobs))
            .expect("jobs should be queryable after restart")
        {
            Response::Jobs(jobs) => {
                assert_eq!(jobs.len(), 1);
                assert_eq!(jobs[0].status, JobStatus::Interrupted);
                assert!(jobs[0].retryable);
            }
            other => panic!("expected jobs response, got {other:?}"),
        }

        let _ = fs::remove_file(layout_path);
        let _ = fs::remove_file(jobs_path);
    }
}
