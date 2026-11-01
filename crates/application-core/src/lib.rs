use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

mod preview_source;
mod recovery;

pub use recovery::{
    RecoveryClock, RecoveryError, RecoverySupervisor, StartupDeadlinePolicy, SystemRecoveryClock,
};

pub use preview_source::{
    AssetSourceLocationResolver, FilePreviewAssetSource, InMemoryPreviewAssetSource,
    PersistentAssetSourceLocationResolver, PreviewAssetRegistration,
};

/// Persistence seams used by the application lifecycle and recovery path.
///
/// The composition root owns the concrete stores. `ApplicationCore` only
/// consumes these protocol-shaped adapters and never reaches into their file
/// layout or transaction details. Diagnostic history is optional so embedders
/// can run without durable diagnostics while the native composition enables it.
pub struct ApplicationRecoveryStores {
    pub journal: Arc<dyn RecoveryJournalStore>,
    pub snapshots: Option<Arc<dyn StateSnapshotStore>>,
    pub recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    pub diagnostics: Option<Arc<dyn DiagnosticEventStore>>,
    pub runtime_leases: Option<Arc<dyn velvet_plugin_host::RuntimeLeaseStore>>,
}

impl ApplicationRecoveryStores {
    pub fn new(
        journal: Arc<dyn RecoveryJournalStore>,
        snapshots: Option<Arc<dyn StateSnapshotStore>>,
        recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    ) -> Self {
        Self {
            journal,
            snapshots,
            recovery_sets,
            diagnostics: None,
            runtime_leases: None,
        }
    }

    pub fn with_diagnostics(mut self, diagnostics: Arc<dyn DiagnosticEventStore>) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    pub fn with_runtime_leases(
        mut self,
        runtime_leases: Arc<dyn velvet_plugin_host::RuntimeLeaseStore>,
    ) -> Self {
        self.runtime_leases = Some(runtime_leases);
        self
    }

    pub fn journal_only(journal: Arc<dyn RecoveryJournalStore>) -> Self {
        Self::new(journal, None, None)
    }
}

use velvet_asset_core::{AssetId, ContentFingerprint, ContentKindId};
use velvet_job_system::{
    JobError, JobExecutionContext, JobExecutionError, JobExecutionResult, JobId, JobProvenance,
    JobRunner, JobSpec, JobStateStore, JobStatus, JobSystem, JOB_SCHEMA_VERSION,
};
use velvet_layout_model::{InMemoryLayoutService, LayoutService};
use velvet_persistence::{
    AssetSourceLocationStore, DiagnosticEventStore, InMemoryAssetSourceLocationStore,
    InMemoryLayoutStateStore, InMemoryProfileStateStore, InMemoryRecoveryJournalStore,
    LayoutStateStore, ProfileStateStore, RecoveryJournalStore, RecoverySetStore,
    StateSnapshotStore, CORE_RECOVERY_SET_ID, MAX_DIAGNOSTIC_EVENTS,
};
use velvet_plugin_api::{
    ContributionKind, ExtensionId, ExtensionInstallPreview, ExtensionInstallRecovery,
    ExtensionInstallRecoveryAction, ExtensionInstallRecoveryLogEntry, ExtensionManifest,
    ExtensionState, Permission, PreviewActionContext, PreviewActionResult, PreviewIntent,
    PreviewRendererDescriptor, PreviewSurface,
};
use velvet_plugin_host::{
    sanitize_plugin_diagnostic_message, DisableReason, ExtensionStorageRecovery,
    ExtensionStorageRecoveryStatus, InMemoryPluginHost, JsonManifestDiscovery,
    ManifestDiscoveryError, ManifestLoadReport, PluginDiagnosticKind, PluginDiagnosticRecord,
    PluginError, PluginHost, PluginJobHandle, PluginJobInvocation, PluginResourceLoader,
    PluginRuntime,
};
use velvet_preview_core::{
    BuiltinImagePreviewProvider, BuiltinTextPreviewProvider, GenericFallbackProvider,
    PreviewAssetSource, PreviewCandidate, PreviewContext, PreviewProvider, PreviewRenderer,
    PreviewRequest, PreviewResolver, PreviewSession, PreviewSessionDescriptor, PreviewSourceError,
    ResolvedPreviewAugmentation,
};
use velvet_workbench_protocol::{
    Command, CoreError, CoreTransport, PluginDiagnosticEvent,
    PluginDiagnosticKind as WirePluginDiagnosticKind, PluginDisableReason, PluginInstallPreviewDto,
    PluginProjectionDto, PluginResourceDto, PluginRuntimeRecoveryDto, PluginRuntimeRecoveryState,
    PluginStorageRecoveryDto, PluginStorageRecoveryStatus, ProfileState, Query, Request,
    RequestEnvelope, RequestId, Response, ResponseEnvelope, PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
    PLUGIN_RESOURCE_SCHEMA_VERSION, PROFILE_SCHEMA_VERSION, PROTOCOL_VERSION,
};

pub struct ApplicationCore {
    layout: InMemoryLayoutService,
    plugins: InMemoryPluginHost,
    previews: PreviewResolver,
    jobs: JobSystem,
    profile: ProfileState,
    preview_actions: BTreeMap<String, PreviewActionAuthorization>,
    preview_locations: Arc<PersistentAssetSourceLocationResolver>,
    layout_store: Arc<dyn LayoutStateStore>,
    profile_store: Arc<dyn ProfileStateStore>,
    plugin_directory: Option<std::path::PathBuf>,
    plugin_resources: Option<PluginResourceLoader>,
    recovery: RecoverySupervisor,
    recovery_snapshots: Option<Arc<dyn StateSnapshotStore>>,
    recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    diagnostic_store: Option<Arc<dyn DiagnosticEventStore>>,
    plugin_diagnostics: VecDeque<PluginDiagnosticEvent>,
    diagnostic_history: VecDeque<PluginDiagnosticEvent>,
    diagnostics_degraded: bool,
    next_plugin_diagnostic_sequence: u64,
}

const MAX_PREVIEW_ACTION_SESSIONS: usize = 128;

struct PreviewActionAuthorization {
    extension_id: ExtensionId,
    provider_id: String,
    asset_id: String,
    content_kind: String,
    intent: PreviewIntent,
    surface: PreviewSurface,
    commands: Vec<String>,
}

struct PluginJobRunner {
    handle: PluginJobHandle,
}

impl JobRunner for PluginJobRunner {
    fn run(&self, context: JobExecutionContext) -> Result<JobExecutionResult, JobExecutionError> {
        if context.is_cancelled() {
            return Err(JobExecutionError::Cancelled);
        }
        let invocation = PluginJobInvocation {
            job_id: context.id.0.clone(),
            job_kind: context.kind.clone(),
            attempt: context.attempt,
            input: context.input.clone(),
            provenance: serde_json::to_value(&context.provenance).map_err(|error| {
                JobExecutionError::Failed(format!("job provenance could not be encoded: {error}"))
            })?,
        };
        let execution = self
            .handle
            .invoke(invocation)
            .map_err(|error| JobExecutionError::Failed(format!("{error:?}")))?;
        for event in execution.events {
            if event.kind != "progress" {
                return Err(JobExecutionError::Failed(format!(
                    "unsupported plugin Job event kind {}",
                    event.kind
                )));
            }
            let progress = event.progress.ok_or_else(|| {
                JobExecutionError::Failed("plugin Job progress event is missing progress".into())
            })?;
            if !context.report_progress(progress, event.message.unwrap_or_default()) {
                return Err(JobExecutionError::Cancelled);
            }
        }
        if context.is_cancelled() {
            return Err(JobExecutionError::Cancelled);
        }
        Ok(JobExecutionResult {
            content_type: execution.result.content_type,
            payload: execution.result.payload,
        })
    }
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

    pub fn with_store_and_profile_store(
        layout_store: Arc<dyn LayoutStateStore>,
        profile_store: Arc<dyn ProfileStateStore>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile(
            layout_store,
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            None,
            JobSystem::default(),
            profile_store,
            Arc::new(InMemoryAssetSourceLocationStore::default()),
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
        Self::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile(
            layout_store,
            runtime,
            plugin_directory,
            jobs,
            Arc::new(InMemoryProfileStateStore::default()),
            Arc::new(InMemoryAssetSourceLocationStore::default()),
        )
    }

    pub fn with_store_and_plugin_directory_and_job_store_and_profile_store(
        layout_store: Arc<dyn LayoutStateStore>,
        plugin_directory: impl Into<std::path::PathBuf>,
        job_store: Arc<dyn JobStateStore>,
        profile_store: Arc<dyn ProfileStateStore>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile(
            layout_store,
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            Some(plugin_directory.into()),
            JobSystem::with_store(job_store),
            profile_store,
            Arc::new(InMemoryAssetSourceLocationStore::default()),
        )
    }

    pub fn with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store(
        layout_store: Arc<dyn LayoutStateStore>,
        plugin_directory: impl Into<std::path::PathBuf>,
        job_store: Arc<dyn JobStateStore>,
        profile_store: Arc<dyn ProfileStateStore>,
        asset_source_store: Arc<dyn AssetSourceLocationStore>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile(
            layout_store,
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            Some(plugin_directory.into()),
            JobSystem::with_store(job_store),
            profile_store,
            asset_source_store,
        )
    }

    pub fn with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store_and_recovery_journal(
        layout_store: Arc<dyn LayoutStateStore>,
        plugin_directory: impl Into<std::path::PathBuf>,
        job_store: Arc<dyn JobStateStore>,
        profile_store: Arc<dyn ProfileStateStore>,
        asset_source_store: Arc<dyn AssetSourceLocationStore>,
        recovery_journal: Arc<dyn RecoveryJournalStore>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile_and_recovery(
            layout_store,
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            Some(plugin_directory.into()),
            JobSystem::with_store(job_store),
            profile_store,
            asset_source_store,
            ApplicationRecoveryStores::journal_only(recovery_journal),
        )
    }

    pub fn with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store_and_recovery_stores(
        layout_store: Arc<dyn LayoutStateStore>,
        plugin_directory: impl Into<std::path::PathBuf>,
        job_store: Arc<dyn JobStateStore>,
        profile_store: Arc<dyn ProfileStateStore>,
        asset_source_store: Arc<dyn AssetSourceLocationStore>,
        recovery_stores: ApplicationRecoveryStores,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile_and_recovery_and_snapshot_stores(
            layout_store,
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            Some(plugin_directory.into()),
            JobSystem::with_store(job_store),
            profile_store,
            asset_source_store,
            recovery_stores,
        )
    }

    fn with_store_and_runtime_and_plugin_directory_and_jobs_and_profile(
        layout_store: Arc<dyn LayoutStateStore>,
        runtime: Box<dyn PluginRuntime>,
        plugin_directory: Option<std::path::PathBuf>,
        jobs: JobSystem,
        profile_store: Arc<dyn ProfileStateStore>,
        asset_source_store: Arc<dyn AssetSourceLocationStore>,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile_and_recovery(
            layout_store,
            runtime,
            plugin_directory,
            jobs,
            profile_store,
            asset_source_store,
            ApplicationRecoveryStores::journal_only(Arc::new(
                InMemoryRecoveryJournalStore::default(),
            )),
        )
    }

    fn with_store_and_runtime_and_plugin_directory_and_jobs_and_profile_and_recovery(
        layout_store: Arc<dyn LayoutStateStore>,
        runtime: Box<dyn PluginRuntime>,
        plugin_directory: Option<std::path::PathBuf>,
        jobs: JobSystem,
        profile_store: Arc<dyn ProfileStateStore>,
        asset_source_store: Arc<dyn AssetSourceLocationStore>,
        recovery_stores: ApplicationRecoveryStores,
    ) -> Self {
        Self::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile_and_recovery_and_snapshot_stores(
            layout_store,
            runtime,
            plugin_directory,
            jobs,
            profile_store,
            asset_source_store,
            recovery_stores,
        )
    }

    fn with_store_and_runtime_and_plugin_directory_and_jobs_and_profile_and_recovery_and_snapshot_stores(
        layout_store: Arc<dyn LayoutStateStore>,
        runtime: Box<dyn PluginRuntime>,
        plugin_directory: Option<std::path::PathBuf>,
        jobs: JobSystem,
        profile_store: Arc<dyn ProfileStateStore>,
        asset_source_store: Arc<dyn AssetSourceLocationStore>,
        recovery_stores: ApplicationRecoveryStores,
    ) -> Self {
        let ApplicationRecoveryStores {
            journal: recovery_journal,
            snapshots: recovery_snapshots,
            recovery_sets,
            diagnostics: diagnostic_store,
            runtime_leases,
        } = recovery_stores;
        let recovery = RecoverySupervisor::new(recovery_journal, Arc::new(SystemRecoveryClock));
        let launch_started = recovery.begin_launch(env!("CARGO_PKG_VERSION"));
        let mut layout = InMemoryLayoutService::default();
        match layout_store.load() {
            Ok(Some(state)) => {
                let generation = state.revision.max(1);
                match layout.migrate(state) {
                    Ok(migrated) => layout = InMemoryLayoutService::from_state(migrated),
                    Err(error) => {
                        let _ = recovery.record_failure_for_owner(
                            velvet_workbench_protocol::RecoveryFailureClass::CoreMigration,
                            format!("layout migration failed: {error:?}"),
                            "layout",
                            "workspace",
                            generation,
                        );
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                let _ = recovery.record_failure(
                    velvet_workbench_protocol::RecoveryFailureClass::StateCorrupt,
                    format!("layout state could not be loaded: {error:?}"),
                );
            }
        }

        let profile = match profile_store.load() {
            Ok(Some(state)) if valid_profile_state(&state) => state,
            Ok(Some(state)) => {
                let _ = recovery.record_failure_for_owner(
                    velvet_workbench_protocol::RecoveryFailureClass::StateCorrupt,
                    "profile state failed validation",
                    "profile",
                    "user",
                    state.revision.max(1),
                );
                ProfileState::default()
            }
            Ok(None) => ProfileState::default(),
            Err(error) => {
                let _ = recovery.record_failure(
                    velvet_workbench_protocol::RecoveryFailureClass::StateCorrupt,
                    format!("profile state could not be loaded: {error:?}"),
                );
                ProfileState::default()
            }
        };

        let plugin_resources = plugin_directory.as_deref().map(PluginResourceLoader::new);
        let preview_locations = Arc::new(PersistentAssetSourceLocationResolver::new(
            asset_source_store,
        ));
        let mut plugins = InMemoryPluginHost::with_runtime(runtime);
        if let Some(snapshots) = recovery_snapshots.clone() {
            plugins.set_extension_storage_store(snapshots);
        }
        if let Some(runtime_leases) = runtime_leases {
            plugins.set_runtime_lease_store(runtime_leases);
            plugins.set_runtime_orphan_reconciler(Arc::new(
                velvet_plugin_host::PlatformRuntimeOrphanReconciler,
            ));
        }
        let (diagnostic_history, diagnostics_degraded) =
            match diagnostic_store.as_ref().map(|store| store.load()) {
                None => (Vec::new(), false),
                Some(Ok(events)) => (
                    events
                        .into_iter()
                        .map(normalize_plugin_diagnostic_event)
                        .collect(),
                    false,
                ),
                Some(Err(_)) => (Vec::new(), true),
            };
        let next_plugin_diagnostic_sequence = diagnostic_history
            .last()
            .map(|event| event.sequence.saturating_add(1))
            .unwrap_or(1);
        let mut core = Self {
            layout,
            plugins,
            previews: PreviewResolver::default(),
            jobs,
            profile,
            preview_actions: BTreeMap::new(),
            preview_locations: preview_locations.clone(),
            layout_store,
            profile_store,
            plugin_directory,
            plugin_resources,
            recovery,
            recovery_snapshots,
            recovery_sets,
            diagnostic_store,
            plugin_diagnostics: VecDeque::new(),
            diagnostic_history: diagnostic_history.into_iter().collect(),
            diagnostics_degraded,
            next_plugin_diagnostic_sequence,
        };

        if launch_started.is_ok() && !core.recovery.is_safe_mode() {
            let _ = core
                .recovery
                .advance_to(velvet_workbench_protocol::StartupPhase::CoreReady);
        }

        core.previews.register(Arc::new(BuiltinTextPreviewProvider));
        core.previews
            .register(Arc::new(BuiltinImagePreviewProvider));
        core.previews.register(Arc::new(GenericFallbackProvider));
        core.previews
            .set_asset_source(Arc::new(FilePreviewAssetSource::new(preview_locations)));
        core
    }

    pub fn recovery_projection(&self) -> velvet_workbench_protocol::RecoveryProjection {
        self.recovery.projection()
    }

    pub fn restore_recovery_set(&self) -> Result<(), CoreError> {
        let Some(snapshot_store) = self.recovery_snapshots.as_deref() else {
            return Err(CoreError::Recovery(
                "recovery snapshots are unavailable in this composition".into(),
            ));
        };
        let Some(recovery_sets) = self.recovery_sets.as_deref() else {
            return Err(CoreError::Recovery(
                "recovery set index is unavailable in this composition".into(),
            ));
        };
        let Some(previous) = recovery_sets
            .load_previous(CORE_RECOVERY_SET_ID)
            .map_err(|error| CoreError::Recovery(format!("{error:?}")))?
        else {
            return Err(CoreError::Recovery(
                "no previous committed recovery set is available".into(),
            ));
        };
        for member in previous.members.values() {
            let snapshot = snapshot_store
                .load_generation(&member.owner_id, &member.scope, member.generation)
                .map_err(|error| CoreError::Recovery(format!("{error:?}")))?;
            if snapshot.is_none() {
                return Err(CoreError::Recovery(format!(
                    "recovery set member {}:{} is missing",
                    member.owner_id, member.scope
                )));
            }
        }
        recovery_sets
            .restore_previous(CORE_RECOVERY_SET_ID)
            .map_err(|error| CoreError::Recovery(format!("{error:?}")))?
            .ok_or_else(|| {
                CoreError::Recovery("no previous committed recovery set is available".into())
            })?;
        Ok(())
    }

    pub fn quarantine_recovery_failure(&self) -> Result<(), CoreError> {
        let Some(target) = self
            .recovery
            .projection()
            .last_failure
            .and_then(|failure| failure.target)
        else {
            return Err(CoreError::Recovery(
                "the latest recovery failure has no quarantinable snapshot".into(),
            ));
        };
        let Some(snapshot_store) = self.recovery_snapshots.as_deref() else {
            return Err(CoreError::Recovery(
                "recovery snapshots are unavailable in this composition".into(),
            ));
        };
        if let Some(recovery_sets) = self.recovery_sets.as_deref() {
            recovery_sets
                .restore_previous(CORE_RECOVERY_SET_ID)
                .map_err(|error| CoreError::Recovery(format!("{error:?}")))?;
        }
        snapshot_store
            .quarantine(&target.owner_id, &target.scope, target.generation)
            .map_err(|error| CoreError::Recovery(format!("{error:?}")))
    }

    pub fn in_safe_mode(&self) -> bool {
        self.recovery.is_safe_mode()
    }

    pub fn mark_workbench_ready(&self) {
        if self.recovery.is_safe_mode() {
            return;
        }
        let _ = self
            .recovery
            .advance_to(velvet_workbench_protocol::StartupPhase::WorkbenchReady);
    }

    pub fn begin_optional_activation(&self) -> bool {
        if self.recovery.is_safe_mode() {
            return false;
        }
        self.recovery
            .advance_to(velvet_workbench_protocol::StartupPhase::OptionalActivation)
            .map(|projection| projection.mode == velvet_workbench_protocol::RecoveryMode::Starting)
            .unwrap_or(false)
    }

    fn optional_activation_allows_next(&self) -> bool {
        let projection = self
            .recovery
            .observe_deadline()
            .unwrap_or_else(|_| self.recovery.projection());
        if projection.mode == velvet_workbench_protocol::RecoveryMode::Safe {
            return false;
        }
        if matches!(
            projection.last_failure.as_ref(),
            Some(failure)
                if failure.class
                    == velvet_workbench_protocol::RecoveryFailureClass::StartupTimeout
                    && failure.phase
                        == velvet_workbench_protocol::StartupPhase::OptionalActivation
        ) {
            return false;
        }
        true
    }

    pub fn mark_ready(&self) {
        if self.recovery.is_safe_mode() {
            return;
        }
        let _ = self.recovery.mark_ready();
    }

    pub fn record_shutdown_intent(
        &self,
    ) -> Result<velvet_workbench_protocol::RecoveryProjection, RecoveryError> {
        self.recovery.record_shutdown_intent()
    }

    pub fn record_shutdown_complete(
        &self,
    ) -> Result<velvet_workbench_protocol::RecoveryProjection, RecoveryError> {
        self.recovery.record_shutdown_complete()
    }

    pub fn record_recovery_failure(
        &self,
        class: velvet_workbench_protocol::RecoveryFailureClass,
        message: impl Into<String>,
    ) {
        let _ = self.recovery.record_failure(class, message);
    }

    pub fn record_recovery_failure_for_owner(
        &self,
        class: velvet_workbench_protocol::RecoveryFailureClass,
        message: impl Into<String>,
        owner_id: impl Into<String>,
        scope: impl Into<String>,
        generation: u64,
    ) {
        let _ = self
            .recovery
            .record_failure_for_owner(class, message, owner_id, scope, generation);
    }

    pub fn export_recovery_diagnostics(
        &self,
        destination: impl AsRef<std::path::Path>,
    ) -> Result<(), RecoveryError> {
        let bytes = serde_json::to_vec_pretty(&self.diagnostic_bundle()).map_err(|error| {
            RecoveryError::Store(format!("diagnostic serialization failed: {error}"))
        })?;
        fs::write(destination.as_ref(), bytes)
            .map_err(|error| RecoveryError::Store(format!("diagnostic export failed: {error}")))
    }

    pub fn register_plugin(&mut self, manifest: ExtensionManifest) -> Result<(), PluginError> {
        self.plugins.register(manifest)
    }

    /// Installs the host-owned, read-only source adapter used by built-in
    /// preview providers. External providers never receive this object; they
    /// use the bounded runtime invocation context instead.
    pub fn set_preview_asset_source(&mut self, source: Arc<dyn PreviewAssetSource>) {
        self.previews.set_asset_source(source);
    }

    pub fn register_preview_asset(
        &self,
        asset_id: AssetId,
        source_path: impl AsRef<std::path::Path>,
        content_kind: ContentKindId,
        fingerprint: Option<ContentFingerprint>,
    ) -> Result<(), velvet_preview_core::PreviewSourceError> {
        self.preview_locations
            .register(asset_id, source_path, content_kind, fingerprint)
    }

    pub fn remove_preview_asset(
        &self,
        asset_id: &AssetId,
    ) -> Result<bool, velvet_preview_core::PreviewSourceError> {
        self.preview_locations.remove(asset_id)
    }

    pub fn preview_asset_registration(
        &self,
        asset_id: &AssetId,
    ) -> Result<PreviewAssetRegistration, velvet_preview_core::PreviewSourceError> {
        self.preview_locations.registration(asset_id)
    }

    /// Registers a trusted built-in provider through the preview-core seam.
    /// Product composition owns which providers are available; the resolver
    /// still owns matching, priority and fallback semantics.
    pub fn register_preview_provider(&mut self, provider: Arc<dyn PreviewProvider>) {
        self.previews.register(provider);
    }

    pub fn activate_plugin(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        self.plugins.activate(id)?;
        if let Err(error) = self.register_plugin_job_runners(id) {
            self.clear_plugin_runtime_state(id);
            let _ = self.plugins.disable(id, DisableReason::Incompatible);
            return Err(error);
        }
        Ok(())
    }

    pub fn disable_plugin(
        &mut self,
        id: &ExtensionId,
        reason: DisableReason,
    ) -> Result<(), PluginError> {
        self.clear_plugin_runtime_state(id);
        self.plugins.disable(id, reason)
    }

    pub fn check_plugin_health(&mut self, id: &ExtensionId) -> Result<ExtensionState, PluginError> {
        let result = self.plugins.check_health(id);
        if result.is_err() {
            self.clear_plugin_runtime_state(id);
            self.record_recovery_failure(
                velvet_workbench_protocol::RecoveryFailureClass::OptionalComponent,
                format!("plugin {} failed its health check", id.0),
            );
        }
        result
    }

    pub fn recover_plugin(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        self.plugins.recover(id)?;
        if let Err(error) = self.register_plugin_job_runners(id) {
            self.clear_plugin_runtime_state(id);
            let _ = self.plugins.disable(id, DisableReason::Incompatible);
            return Err(error);
        }
        Ok(())
    }

    pub fn discover_plugins(&self) -> Vec<velvet_plugin_api::ExtensionSummary> {
        self.plugins.discover()
    }

    pub fn query_plugin_storage_recovery(
        &self,
        id: &ExtensionId,
    ) -> Result<PluginStorageRecoveryDto, PluginError> {
        self.plugins
            .extension_storage_recovery(id)
            .map(plugin_storage_recovery_dto)
    }

    pub fn restore_plugin_state(
        &mut self,
        id: &ExtensionId,
    ) -> Result<PluginStorageRecoveryDto, PluginError> {
        self.plugins.restore_extension_state(id)?;
        self.plugins
            .extension_storage_recovery(id)
            .map(plugin_storage_recovery_dto)
    }

    pub fn quarantine_plugin_state(
        &mut self,
        id: &ExtensionId,
        generation: u64,
    ) -> Result<PluginStorageRecoveryDto, PluginError> {
        self.plugins.quarantine_extension_state(id, generation)?;
        self.plugins
            .extension_storage_recovery(id)
            .map(plugin_storage_recovery_dto)
    }

    pub fn query_plugin_runtime_recovery(
        &self,
    ) -> Result<Vec<PluginRuntimeRecoveryDto>, PluginError> {
        self.plugins.quarantined_runtime_leases().map(|leases| {
            leases
                .into_iter()
                .map(|lease| PluginRuntimeRecoveryDto {
                    extension_id: lease.extension_id,
                    runtime_nonce: lease.runtime_nonce,
                    state: PluginRuntimeRecoveryState::Quarantined,
                    started_at_ms: lease.started_at_ms,
                })
                .collect()
        })
    }

    pub fn close_quarantined_plugin_runtime_lease(
        &mut self,
        extension_id: &ExtensionId,
        runtime_nonce: &str,
    ) -> Result<(), PluginError> {
        self.plugins
            .close_quarantined_runtime_lease(extension_id, runtime_nonce)
    }

    pub fn reconcile_plugin_runtime_orphans(
        &mut self,
    ) -> Result<Vec<velvet_plugin_host::RuntimeOrphanReport>, PluginError> {
        let reports = self.plugins.reconcile_runtime_orphans()?;
        for report in &reports {
            if report.lease_closed || report.quarantined {
                self.clear_plugin_runtime_state(&report.extension_id);
            }
        }
        Ok(reports)
    }

    /// Starts manifests discovered during native application startup. Other
    /// embedders may keep the host lazy and call `activate_plugin` explicitly.
    pub fn activate_registered_plugins(&mut self) -> Vec<PluginError> {
        if self.in_safe_mode() {
            return Vec::new();
        }
        let recovery_ids = self
            .inspect_plugin_install_recoveries()
            .unwrap_or_default()
            .into_iter()
            .map(|recovery| recovery.extension_id)
            .collect::<Vec<_>>();
        let mut failures = Vec::new();
        for summary in self.plugins.discover() {
            if !self.optional_activation_allows_next() {
                break;
            }
            if summary.state != ExtensionState::Registered || recovery_ids.contains(&summary.id) {
                continue;
            }
            if let Err(error) = self.activate_plugin(&summary.id) {
                failures.push(error);
            }
        }
        for failure in &failures {
            self.record_recovery_failure(
                velvet_workbench_protocol::RecoveryFailureClass::OptionalComponent,
                format!("optional plugin activation failed: {failure:?}"),
            );
        }
        failures
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

    pub fn install_plugin_from_archive(
        &mut self,
        source_archive: impl AsRef<std::path::Path>,
    ) -> Result<velvet_plugin_api::ExtensionSummary, PluginError> {
        let Some(plugin_directory) = self.plugin_directory.as_deref() else {
            return Err(PluginError::InstallFailed(
                "plugin installation is unavailable without an application plugin directory".into(),
            ));
        };
        self.plugins
            .install_from_archive(source_archive.as_ref(), plugin_directory)
    }

    pub fn inspect_plugin_from_directory(
        &self,
        source_dir: impl AsRef<std::path::Path>,
    ) -> Result<ExtensionInstallPreview, PluginError> {
        self.plugins.inspect_from_directory(source_dir.as_ref())
    }

    pub fn inspect_plugin_from_archive(
        &self,
        source_archive: impl AsRef<std::path::Path>,
    ) -> Result<ExtensionInstallPreview, PluginError> {
        self.plugins.inspect_from_archive(source_archive.as_ref())
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
        self.create_job_with_provenance(
            kind,
            duration_ms,
            will_fail,
            JobProvenance::for_owner("core"),
        )
    }

    pub fn create_job_with_provenance(
        &self,
        kind: String,
        duration_ms: u64,
        will_fail: bool,
        provenance: JobProvenance,
    ) -> Result<velvet_job_system::JobSnapshot, JobError> {
        self.create_job_with_provenance_and_input(
            kind,
            duration_ms,
            will_fail,
            provenance,
            serde_json::Value::Null,
        )
    }

    pub fn create_job_with_provenance_and_input(
        &self,
        kind: String,
        duration_ms: u64,
        will_fail: bool,
        provenance: JobProvenance,
        input: serde_json::Value,
    ) -> Result<velvet_job_system::JobSnapshot, JobError> {
        self.jobs.submit_or_reuse(JobSpec {
            kind,
            duration_ms,
            will_fail,
            input,
            provenance,
        })
    }

    pub fn register_job_runner(
        &self,
        kind: impl Into<String>,
        runner: Arc<dyn velvet_job_system::JobRunner>,
    ) -> Result<(), JobError> {
        self.jobs.register_runner("core", kind, runner)
    }

    pub fn register_owned_job_runner(
        &self,
        owner_id: impl Into<String>,
        kind: impl Into<String>,
        runner: Arc<dyn velvet_job_system::JobRunner>,
    ) -> Result<(), JobError> {
        self.jobs.register_runner(owner_id, kind, runner)
    }

    pub fn unregister_owned_job_runners(&self, owner_id: &str) -> Result<usize, JobError> {
        self.jobs.unregister_runners(owner_id)
    }

    pub fn invoke_preview_action(
        &mut self,
        extension_id: ExtensionId,
        command_id: String,
        args: serde_json::Value,
        context: PreviewActionContext,
    ) -> Result<Response, CoreError> {
        let Some(authorization) = self.preview_actions.get(&context.session_id) else {
            return Err(CoreError::InvalidCommand(
                "preview action session is no longer active".into(),
            ));
        };
        if authorization.extension_id != extension_id
            || authorization.provider_id != context.provider_id
            || authorization.asset_id != context.asset_id
            || authorization.content_kind != context.content_kind
            || authorization.intent != context.intent
            || authorization.surface != context.surface
            || !authorization.commands.iter().any(|id| id == &command_id)
        {
            return Err(CoreError::InvalidCommand(
                "preview action is not declared by the active preview session".into(),
            ));
        }
        let job_provenance = self.preview_job_provenance(&extension_id, &context);
        let job_input = args.clone();
        let result =
            match self
                .plugins
                .invoke_preview_action(&extension_id, command_id, args, context)
            {
                Ok(result) => result,
                Err(error) => {
                    let plugin_failed = self
                        .plugins
                        .discover()
                        .into_iter()
                        .find(|summary| summary.id == extension_id)
                        .is_some_and(|summary| summary.state == ExtensionState::Failed);
                    if plugin_failed {
                        self.clear_plugin_runtime_state(&extension_id);
                        return Err(plugin_error(error));
                    }
                    return Err(CoreError::Plugin(format!(
                        "preview action failed: {error:?}"
                    )));
                }
            };
        let Some(value) = result.result else {
            return Err(CoreError::InvalidCommand(
                "preview action returned no result".into(),
            ));
        };
        let action = serde_json::from_value::<PreviewActionResult>(value).map_err(|error| {
            CoreError::InvalidCommand(format!(
                "preview action returned an invalid result: {error}"
            ))
        })?;
        match action {
            PreviewActionResult::Accepted => Ok(Response::Accepted),
            PreviewActionResult::Job {
                job_kind,
                duration_ms,
            } => {
                if job_kind.trim().is_empty() {
                    return Err(CoreError::InvalidCommand(
                        "preview action job kind cannot be empty".into(),
                    ));
                }
                let job_declared = self
                    .plugins
                    .active_contributions(&extension_id)
                    .map_err(plugin_error)?
                    .into_iter()
                    .any(|contribution| {
                        contribution.kind == ContributionKind::Job && contribution.id == job_kind
                    });
                if !job_declared {
                    return Err(CoreError::Plugin(format!(
                        "extension {} did not declare preview job {}",
                        extension_id.0, job_kind
                    )));
                }
                if !self
                    .plugins
                    .has_permission(&extension_id, Permission::JobsCreate)
                    .map_err(plugin_error)?
                {
                    return Err(CoreError::Plugin(format!(
                        "extension {} lacks jobs-create permission for preview action",
                        extension_id.0
                    )));
                }
                self.create_job_with_provenance_and_input(
                    job_kind,
                    duration_ms,
                    false,
                    job_provenance,
                    job_input,
                )
                .map(Response::Job)
                .map_err(job_error)
            }
        }
    }

    fn preview_job_provenance(
        &self,
        extension_id: &ExtensionId,
        context: &PreviewActionContext,
    ) -> JobProvenance {
        let input_fingerprint = self
            .previews
            .asset_fingerprint(&velvet_asset_core::AssetId(context.asset_id.clone()))
            .ok()
            .flatten()
            .map(|fingerprint| fingerprint.0);
        JobProvenance {
            owner_id: extension_id.0.clone(),
            asset_id: Some(context.asset_id.clone()),
            content_kind: Some(context.content_kind.clone()),
            input_fingerprint,
            provider_id: Some(context.provider_id.clone()),
            provider_version: self
                .plugins
                .discover()
                .into_iter()
                .find(|summary| summary.id == *extension_id)
                .map(|summary| summary.version),
            settings_fingerprint: None,
            schema_version: JOB_SCHEMA_VERSION,
        }
    }

    fn remember_preview_actions(&mut self, extension_id: &ExtensionId, session: &PreviewSession) {
        if session.descriptor.commands.is_empty() {
            return;
        }
        if self.preview_actions.len() >= MAX_PREVIEW_ACTION_SESSIONS
            && !self
                .preview_actions
                .contains_key(&session.descriptor.session_id)
        {
            if let Some(oldest) = self.preview_actions.keys().next().cloned() {
                self.preview_actions.remove(&oldest);
            }
        }
        self.preview_actions.insert(
            session.descriptor.session_id.clone(),
            PreviewActionAuthorization {
                extension_id: extension_id.clone(),
                provider_id: session.provider_id.clone(),
                asset_id: session.context.asset_id.0.clone(),
                content_kind: session.context.content_kind.0.clone(),
                intent: plugin_preview_intent(&session.context.intent),
                surface: plugin_preview_surface(&session.context.surface),
                commands: session.descriptor.commands.clone(),
            },
        );
    }

    fn forget_preview_actions(&mut self, extension_id: &ExtensionId) {
        self.preview_actions
            .retain(|_, authorization| &authorization.extension_id != extension_id);
    }

    fn clear_plugin_runtime_state(&mut self, extension_id: &ExtensionId) {
        self.forget_preview_actions(extension_id);
        let _ = self.jobs.cancel_owned_jobs(&extension_id.0);
        let _ = self.jobs.unregister_runners(&extension_id.0);
    }

    fn register_plugin_job_runners(&self, extension_id: &ExtensionId) -> Result<(), PluginError> {
        self.jobs
            .unregister_runners(&extension_id.0)
            .map_err(|error| {
                PluginError::InvalidContribution(format!(
                    "could not clear Job runners for {}: {error:?}",
                    extension_id.0
                ))
            })?;
        for handle in self.plugins.job_handles(extension_id)? {
            let kind = handle.job_kind().to_owned();
            self.jobs
                .register_runner(
                    extension_id.0.clone(),
                    kind.clone(),
                    Arc::new(PluginJobRunner { handle }),
                )
                .map_err(|error| {
                    PluginError::InvalidContribution(format!(
                        "Job runner {kind} for {} could not be registered: {error:?}",
                        extension_id.0
                    ))
                })?;
        }
        Ok(())
    }

    fn release_preview_session(&mut self, session_id: &str) -> Response {
        self.preview_actions.remove(session_id);
        Response::Accepted
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

    pub fn profile_state(&self) -> ProfileState {
        self.profile.clone()
    }

    pub fn layout_snapshot(&self) -> velvet_layout_model::LayoutState {
        self.layout.snapshot()
    }

    pub fn drain_job_events(&mut self) -> Vec<velvet_job_system::JobSnapshot> {
        let snapshots = self.jobs.drain_events();
        for snapshot in &snapshots {
            if snapshot.provenance.owner_id != "core"
                && matches!(snapshot.status, JobStatus::Failed | JobStatus::Interrupted)
            {
                self.enqueue_plugin_diagnostic(PluginDiagnosticEvent {
                    schema_version: PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
                    sequence: 0,
                    extension_id: snapshot.provenance.owner_id.clone(),
                    kind: WirePluginDiagnosticKind::Job,
                    code: if snapshot.status == JobStatus::Interrupted {
                        "job-interrupted".into()
                    } else {
                        "job-failed".into()
                    },
                    message: sanitize_plugin_diagnostic_message(
                        snapshot
                            .message
                            .as_deref()
                            .unwrap_or("plugin Job failed without a message"),
                    ),
                    job_id: Some(snapshot.id.0.clone()),
                    attempt: Some(snapshot.attempt),
                    retryable: snapshot.retryable,
                    occurred_at_ms: diagnostic_timestamp_ms(),
                });
            }
        }
        snapshots
    }

    pub fn drain_plugin_diagnostics(&mut self) -> Vec<PluginDiagnosticEvent> {
        let host_records = self.plugins.drain_diagnostics();
        for record in host_records {
            self.enqueue_plugin_diagnostic(plugin_diagnostic_from_record(record));
        }
        self.plugin_diagnostics.drain(..).collect()
    }

    fn enqueue_plugin_diagnostic(&mut self, mut event: PluginDiagnosticEvent) {
        event = normalize_plugin_diagnostic_event(event);
        event.sequence = self.next_plugin_diagnostic_sequence();
        if self.plugin_diagnostics.len() == MAX_DIAGNOSTIC_EVENTS {
            self.plugin_diagnostics.pop_front();
        }
        if self.diagnostic_history.len() == MAX_DIAGNOSTIC_EVENTS {
            self.diagnostic_history.pop_front();
        }
        self.plugin_diagnostics.push_back(event.clone());
        self.diagnostic_history.push_back(event);
        self.persist_diagnostic_history();
    }

    fn persist_diagnostic_history(&mut self) {
        let Some(store) = self.diagnostic_store.as_ref() else {
            return;
        };
        let events = self.diagnostic_history.iter().cloned().collect::<Vec<_>>();
        self.diagnostics_degraded = store.save(&events).is_err();
    }

    pub fn diagnostic_bundle(&self) -> velvet_workbench_protocol::DiagnosticBundle {
        let mut bundle = self.recovery.diagnostic_bundle();
        bundle.plugin_diagnostics = self.diagnostic_history.iter().cloned().collect();
        bundle.diagnostics_degraded = self.diagnostics_degraded;
        bundle
    }

    fn next_plugin_diagnostic_sequence(&mut self) -> u64 {
        let next = self.next_plugin_diagnostic_sequence;
        self.next_plugin_diagnostic_sequence = next.saturating_add(1);
        next
    }

    pub fn uninstall_plugin(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        let Some(plugin_directory) = self.plugin_directory.clone() else {
            return Err(PluginError::UninstallFailed(
                "plugin uninstallation is unavailable without an application plugin directory"
                    .into(),
            ));
        };
        self.clear_plugin_runtime_state(id);
        self.plugins.uninstall_from_directory(id, &plugin_directory)
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
            Request::Command(Command::InstallPluginArchive { source_archive }) => self
                .install_plugin_from_archive(source_archive)
                .map(Response::Extension)
                .map_err(plugin_error),
            Request::Command(Command::InspectPlugin { source_dir }) => self
                .inspect_plugin_from_directory(source_dir)
                .map(PluginInstallPreviewDto::from)
                .map(Response::PluginInstallPreview)
                .map_err(plugin_error),
            Request::Command(Command::InspectPluginArchive { source_archive }) => self
                .inspect_plugin_from_archive(source_archive)
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
            Request::Command(Command::ExportRecoveryDiagnostics { destination }) => {
                self.export_recovery_diagnostics(destination)
                    .map_err(|error| CoreError::Recovery(format!("{error:?}")))?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::RestoreRecoverySet) => {
                self.restore_recovery_set()?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::QuarantineRecoveryFailure) => {
                self.quarantine_recovery_failure()?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::RestorePluginState { extension_id }) => self
                .restore_plugin_state(&extension_id)
                .map(Response::PluginStorageRecovery)
                .map_err(plugin_error),
            Request::Command(Command::QuarantinePluginState {
                extension_id,
                generation,
            }) => self
                .quarantine_plugin_state(&extension_id, generation)
                .map(Response::PluginStorageRecovery)
                .map_err(plugin_error),
            Request::Command(Command::CloseQuarantinedRuntimeLease {
                extension_id,
                runtime_nonce,
            }) => {
                self.close_quarantined_plugin_runtime_lease(&extension_id, &runtime_nonce)
                    .map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::UninstallPlugin { extension_id }) => {
                self.uninstall_plugin(&extension_id).map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::RegisterPreviewAsset {
                asset_id,
                source_path,
                content_kind,
            }) => self
                .register_preview_asset(
                    asset_id.clone(),
                    source_path,
                    ContentKindId(content_kind),
                    None,
                )
                .map(|_| Response::Accepted)
                .map_err(|error| preview_source_error(&asset_id, error)),
            Request::Command(Command::RemovePreviewAsset { asset_id }) => self
                .remove_preview_asset(&asset_id)
                .map(|_| Response::Accepted)
                .map_err(|error| preview_source_error(&asset_id, error)),
            Request::Command(Command::InvokePluginCommand {
                extension_id,
                command_id,
                args,
                context,
            }) => {
                match self
                    .plugins
                    .invoke_command(&extension_id, command_id, args, context)
                {
                    Ok(_) => Ok(Response::Accepted),
                    Err(error) => {
                        let plugin_failed = self
                            .plugins
                            .discover()
                            .into_iter()
                            .find(|summary| summary.id == extension_id)
                            .is_some_and(|summary| summary.state == ExtensionState::Failed);
                        if plugin_failed {
                            self.clear_plugin_runtime_state(&extension_id);
                            self.record_recovery_failure(
                                velvet_workbench_protocol::RecoveryFailureClass::OptionalComponent,
                                format!("plugin {} runtime invocation failed", extension_id.0),
                            );
                        }
                        Err(plugin_error(error))
                    }
                }
            }
            Request::Command(Command::InvokePreviewAction {
                extension_id,
                command_id,
                args,
                context,
            }) => self.invoke_preview_action(extension_id, command_id, args, context),
            Request::Command(Command::ReleasePreviewSession { session_id }) => {
                Ok(self.release_preview_session(&session_id))
            }
            Request::Command(Command::CreateJob {
                kind,
                duration_ms,
                will_fail,
                input,
            }) => self
                .create_job_with_provenance_and_input(
                    kind,
                    duration_ms,
                    will_fail,
                    JobProvenance::for_owner("core"),
                    input,
                )
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
            Request::Command(Command::SetLocale {
                locale,
                expected_revision,
            }) => self.set_locale(expected_revision, locale),
            Request::Query(Query::Layout) => Ok(Response::Layout(self.layout.snapshot())),
            Request::Query(Query::Profile) => Ok(Response::Profile(self.profile.clone())),
            Request::Query(Query::Recovery) => Ok(Response::Recovery(self.recovery_projection())),
            Request::Query(Query::PluginLocalization {
                extension_id,
                locale,
            }) => self.read_plugin_localization(&extension_id, &locale),
            Request::Query(Query::Extensions) => Ok(Response::Extensions(self.plugins.discover())),
            Request::Query(Query::PluginInstallRecoveries) => self
                .inspect_plugin_install_recoveries()
                .map(Response::PluginInstallRecoveries)
                .map_err(plugin_error),
            Request::Query(Query::PluginInstallRecoveryLog) => self
                .inspect_plugin_install_recovery_log()
                .map(Response::PluginInstallRecoveryLog)
                .map_err(plugin_error),
            Request::Query(Query::PluginStorageRecovery { extension_id }) => self
                .query_plugin_storage_recovery(&extension_id)
                .map(Response::PluginStorageRecovery)
                .map_err(plugin_error),
            Request::Query(Query::PluginRuntimeRecovery) => self
                .query_plugin_runtime_recovery()
                .map(Response::PluginRuntimeRecovery)
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
                .resolve_preview(PreviewRequest {
                    context: PreviewContext {
                        asset_id,
                        content_kind: ContentKindId(content_kind),
                        intent,
                        surface,
                    },
                })
                .map(Response::Preview)
                .map_err(|error| CoreError::InvalidCommand(format!("{error:?}"))),
            Request::Query(Query::ResolvePreviewAsset {
                asset_id,
                intent,
                surface,
            }) => {
                let registration = self
                    .preview_asset_registration(&asset_id)
                    .map_err(|error| preview_source_error(&asset_id, error))?;
                self.resolve_preview(PreviewRequest {
                    context: PreviewContext {
                        asset_id,
                        content_kind: registration.content_kind,
                        intent,
                        surface,
                    },
                })
                .map(Response::Preview)
                .map_err(|error| CoreError::InvalidCommand(format!("{error:?}")))
            }
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

    fn set_locale(
        &mut self,
        expected_revision: Option<u64>,
        locale: String,
    ) -> Result<Response, CoreError> {
        if !valid_locale(&locale) {
            return Err(CoreError::Profile(format!("invalid locale: {locale}")));
        }
        if let Some(expected) = expected_revision {
            if expected != self.profile.revision {
                return Err(CoreError::StaleRevision {
                    expected,
                    actual: self.profile.revision,
                });
            }
        }
        if locale == self.profile.locale {
            return Ok(Response::Profile(self.profile.clone()));
        }

        let next = ProfileState {
            schema_version: PROFILE_SCHEMA_VERSION,
            revision: self
                .profile
                .revision
                .checked_add(1)
                .ok_or_else(|| CoreError::Profile("profile revision exhausted".into()))?,
            locale,
        };
        self.profile_store
            .save(&next)
            .map_err(|error| CoreError::Profile(format!("{error:?}")))?;
        self.profile = next.clone();
        Ok(Response::Profile(next))
    }

    fn read_plugin_localization(
        &self,
        extension_id: &ExtensionId,
        locale: &str,
    ) -> Result<Response, CoreError> {
        let loader = self
            .plugin_resources
            .as_ref()
            .ok_or_else(|| CoreError::PluginResource {
                extension_id: extension_id.0.clone(),
                locale: locale.into(),
                message: "plugin resources are unavailable without an installed package root"
                    .into(),
            })?;
        let projection = self
            .plugins
            .projection(extension_id)
            .map_err(plugin_error)?;
        let resource = loader
            .read_localization_bundle(extension_id, &projection.manifest, locale)
            .map_err(|error| plugin_resource_error(extension_id, locale, error))?;
        Ok(Response::PluginResource(PluginResourceDto {
            schema_version: PLUGIN_RESOURCE_SCHEMA_VERSION,
            extension_id: resource.extension_id,
            locale: resource.locale,
            content_type: resource.content_type,
            bytes: resource.bytes,
        }))
    }

    fn resolve_preview(
        &mut self,
        request: PreviewRequest,
    ) -> Result<velvet_preview_core::ResolvedPreview, velvet_preview_core::PreviewError> {
        let mut candidates = self
            .plugins
            .discover()
            .into_iter()
            .filter(|summary| summary.active)
            .flat_map(|summary| {
                let extension_id = summary.id.clone();
                self.plugins
                    .active_contributions(&extension_id)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(move |contribution| {
                        if contribution.kind != ContributionKind::PreviewProvider {
                            return None;
                        }
                        let candidate = plugin_preview_candidate(&extension_id, &contribution)?;
                        Some((candidate, extension_id.clone(), contribution.id))
                    })
            })
            .filter(|(candidate, _, _)| candidate.matches(&request.context))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .0
                .priority
                .cmp(&left.0.priority)
                .then_with(|| left.0.provider_id.cmp(&right.0.provider_id))
        });

        let mut first_diagnostic = None;
        for (candidate, extension_id, contribution_id) in candidates {
            let context = preview_context_payload(&request.context);
            let result = match self
                .plugins
                .invoke_preview(&extension_id, contribution_id, context)
            {
                Ok(result) => result,
                Err(error) => {
                    if first_diagnostic.is_none() {
                        first_diagnostic = Some(format!("preview provider failed: {error:?}"));
                    }
                    continue;
                }
            };
            let Some(value) = result.result else {
                if first_diagnostic.is_none() {
                    first_diagnostic =
                        Some("preview provider returned no session descriptor".into());
                }
                continue;
            };
            let Ok(descriptor) =
                serde_json::from_value::<velvet_plugin_api::PreviewSessionDescriptor>(value)
            else {
                if first_diagnostic.is_none() {
                    first_diagnostic =
                        Some("preview provider returned a malformed session descriptor".into());
                }
                continue;
            };
            let provider_id = candidate.provider_id.clone();
            let session = match preview_session_from_descriptor(
                provider_id,
                request.context.clone(),
                descriptor,
                "preview provider",
            ) {
                Ok(session) => session,
                Err(diagnostic) => {
                    if first_diagnostic.is_none() {
                        first_diagnostic = Some(diagnostic);
                    }
                    continue;
                }
            };
            self.remember_preview_actions(&extension_id, &session);
            let mut resolved = velvet_preview_core::ResolvedPreview {
                candidate,
                session,
                diagnostic: first_diagnostic,
                augmentations: Vec::new(),
            };
            resolved.augmentations = self.resolve_preview_augmentations(&request.context);
            return Ok(resolved);
        }

        let fallback_context = request.context.clone();
        let mut fallback = self.previews.resolve(request)?;
        if first_diagnostic.is_some() {
            fallback.diagnostic = first_diagnostic;
        }
        fallback.augmentations = self.resolve_preview_augmentations(&fallback_context);
        Ok(fallback)
    }

    fn resolve_preview_augmentations(
        &mut self,
        context: &PreviewContext,
    ) -> Vec<ResolvedPreviewAugmentation> {
        let mut candidates = self
            .plugins
            .discover()
            .into_iter()
            .filter(|summary| summary.active)
            .flat_map(|summary| {
                let extension_id = summary.id.clone();
                self.plugins
                    .active_contributions(&extension_id)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(move |contribution| {
                        if !matches!(
                            contribution.kind,
                            ContributionKind::PreviewAugmentation | ContributionKind::Inspector
                        ) {
                            return None;
                        }
                        let slot = contribution.preview.as_ref()?.slot.clone()?;
                        let candidate = plugin_preview_candidate(&extension_id, &contribution)?;
                        candidate.matches(context).then_some((
                            candidate,
                            extension_id.clone(),
                            contribution.id,
                            slot,
                        ))
                    })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            preview_slot_name(left.3.clone())
                .cmp(&preview_slot_name(right.3.clone()))
                .then_with(|| right.0.priority.cmp(&left.0.priority))
                .then_with(|| left.0.provider_id.cmp(&right.0.provider_id))
        });

        candidates
            .into_iter()
            .map(|(candidate, extension_id, contribution_id, slot)| {
                let augmentation_id = candidate.provider_id.clone();
                let slot = preview_slot_name(slot);
                let context_payload = preview_context_payload(context);
                let result = self.plugins.invoke_preview_augmentation(
                    &extension_id,
                    contribution_id,
                    context_payload,
                );
                match result {
                    Err(error) => ResolvedPreviewAugmentation {
                        augmentation_id,
                        slot,
                        session: None,
                        diagnostic: Some(format!("preview augmentation failed: {error:?}")),
                    },
                    Ok(result) => {
                        let Some(value) = result.result else {
                            return ResolvedPreviewAugmentation {
                                augmentation_id,
                                slot,
                                session: None,
                                diagnostic: Some(
                                    "preview augmentation returned no session descriptor".into(),
                                ),
                            };
                        };
                        let Ok(descriptor) = serde_json::from_value::<
                            velvet_plugin_api::PreviewSessionDescriptor,
                        >(value) else {
                            return ResolvedPreviewAugmentation {
                                augmentation_id,
                                slot,
                                session: None,
                                diagnostic: Some(
                                    "preview augmentation returned a malformed session descriptor"
                                        .into(),
                                ),
                            };
                        };
                        match preview_session_from_descriptor(
                            augmentation_id.clone(),
                            context.clone(),
                            descriptor,
                            "preview augmentation",
                        ) {
                            Ok(session) => {
                                self.remember_preview_actions(&extension_id, &session);
                                ResolvedPreviewAugmentation {
                                    augmentation_id,
                                    slot,
                                    session: Some(session),
                                    diagnostic: None,
                                }
                            }
                            Err(diagnostic) => ResolvedPreviewAugmentation {
                                augmentation_id,
                                slot,
                                session: None,
                                diagnostic: Some(diagnostic),
                            },
                        }
                    }
                }
            })
            .collect()
    }
}

fn preview_context_payload(context: &PreviewContext) -> serde_json::Value {
    serde_json::json!({
        "assetId": context.asset_id.0,
        "contentKind": context.content_kind.0,
        "intent": format!("{:?}", context.intent).to_lowercase(),
        "surface": format!("{:?}", context.surface).to_lowercase(),
    })
}

fn plugin_preview_intent(
    intent: &velvet_preview_core::PreviewIntent,
) -> velvet_plugin_api::PreviewIntent {
    match intent {
        velvet_preview_core::PreviewIntent::Quick => velvet_plugin_api::PreviewIntent::Quick,
        velvet_preview_core::PreviewIntent::Inspect => velvet_plugin_api::PreviewIntent::Inspect,
        velvet_preview_core::PreviewIntent::Analyze => velvet_plugin_api::PreviewIntent::Analyze,
    }
}

fn plugin_preview_surface(
    surface: &velvet_preview_core::PreviewSurface,
) -> velvet_plugin_api::PreviewSurface {
    match surface {
        velvet_preview_core::PreviewSurface::Workspace => {
            velvet_plugin_api::PreviewSurface::Workspace
        }
        velvet_preview_core::PreviewSurface::Thumbnail => {
            velvet_plugin_api::PreviewSurface::Thumbnail
        }
        velvet_preview_core::PreviewSurface::Inspector => {
            velvet_plugin_api::PreviewSurface::Inspector
        }
    }
}

fn plugin_preview_candidate(
    extension_id: &ExtensionId,
    contribution: &velvet_plugin_api::ContributionDescriptor,
) -> Option<PreviewCandidate> {
    let preview = contribution.preview.as_ref()?;
    Some(PreviewCandidate {
        provider_id: format!("{}.{}", extension_id.0, contribution.id),
        content_kinds: preview
            .content_kinds
            .iter()
            .cloned()
            .map(ContentKindId)
            .collect(),
        intents: preview
            .intents
            .iter()
            .map(|intent| match intent {
                velvet_plugin_api::PreviewIntent::Quick => {
                    velvet_preview_core::PreviewIntent::Quick
                }
                velvet_plugin_api::PreviewIntent::Inspect => {
                    velvet_preview_core::PreviewIntent::Inspect
                }
                velvet_plugin_api::PreviewIntent::Analyze => {
                    velvet_preview_core::PreviewIntent::Analyze
                }
            })
            .collect(),
        surfaces: preview
            .surfaces
            .iter()
            .map(|surface| match surface {
                velvet_plugin_api::PreviewSurface::Workspace => {
                    velvet_preview_core::PreviewSurface::Workspace
                }
                velvet_plugin_api::PreviewSurface::Thumbnail => {
                    velvet_preview_core::PreviewSurface::Thumbnail
                }
                velvet_plugin_api::PreviewSurface::Inspector => {
                    velvet_preview_core::PreviewSurface::Inspector
                }
            })
            .collect(),
        priority: preview.priority,
    })
}

fn preview_session_from_descriptor(
    provider_id: String,
    context: PreviewContext,
    descriptor: velvet_plugin_api::PreviewSessionDescriptor,
    label: &str,
) -> Result<PreviewSession, String> {
    if !valid_preview_descriptor(&descriptor) {
        return Err(format!("{label} returned an invalid session descriptor"));
    }
    let renderer = match descriptor.renderer {
        PreviewRendererDescriptor::Generic => PreviewRenderer::Generic,
        PreviewRendererDescriptor::UiBridge {
            entrypoint,
            bridge_version,
        } => PreviewRenderer::UiBridge {
            entrypoint,
            bridge_version,
        },
    };
    Ok(PreviewSession {
        provider_id,
        context,
        descriptor: PreviewSessionDescriptor {
            session_id: descriptor.session_id,
            renderer,
            commands: descriptor.commands,
            slots: descriptor
                .slots
                .into_iter()
                .map(preview_slot_name)
                .collect(),
        },
    })
}

fn preview_slot_name(slot: velvet_plugin_api::PreviewAugmentationSlot) -> String {
    match slot {
        velvet_plugin_api::PreviewAugmentationSlot::PreviewOverlay => "preview-overlay",
        velvet_plugin_api::PreviewAugmentationSlot::Inspector => "inspector",
        velvet_plugin_api::PreviewAugmentationSlot::Metadata => "metadata",
        velvet_plugin_api::PreviewAugmentationSlot::Histogram => "histogram",
        velvet_plugin_api::PreviewAugmentationSlot::ToolbarAction => "toolbar-action",
        velvet_plugin_api::PreviewAugmentationSlot::ContextMenu => "context-menu",
        velvet_plugin_api::PreviewAugmentationSlot::AnalysisAction => "analysis-action",
    }
    .into()
}

fn valid_preview_descriptor(descriptor: &velvet_plugin_api::PreviewSessionDescriptor) -> bool {
    if descriptor.session_id.trim().is_empty() {
        return false;
    }
    match &descriptor.renderer {
        PreviewRendererDescriptor::Generic => true,
        PreviewRendererDescriptor::UiBridge {
            entrypoint,
            bridge_version,
        } => !entrypoint.trim().is_empty() && !bridge_version.trim().is_empty(),
    }
}

fn disable_reason(reason: PluginDisableReason) -> DisableReason {
    match reason {
        PluginDisableReason::User => DisableReason::User,
        PluginDisableReason::Crash => DisableReason::Crash,
        PluginDisableReason::Incompatible => DisableReason::Incompatible,
    }
}

fn plugin_storage_recovery_dto(recovery: ExtensionStorageRecovery) -> PluginStorageRecoveryDto {
    PluginStorageRecoveryDto {
        extension_id: recovery.extension_id,
        status: match recovery.status {
            ExtensionStorageRecoveryStatus::Available => PluginStorageRecoveryStatus::Available,
            ExtensionStorageRecoveryStatus::Empty => PluginStorageRecoveryStatus::Empty,
            ExtensionStorageRecoveryStatus::Corrupt => PluginStorageRecoveryStatus::Corrupt,
            ExtensionStorageRecoveryStatus::Unavailable => PluginStorageRecoveryStatus::Unavailable,
        },
        current_generation: recovery.current_generation,
        previous_generation: recovery.previous_generation,
        schema_version: recovery.schema_version,
        previous_schema_version: recovery.previous_schema_version,
        quarantine_generation: recovery.quarantine_generation,
        reason: recovery.reason,
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

fn plugin_diagnostic_from_record(record: PluginDiagnosticRecord) -> PluginDiagnosticEvent {
    PluginDiagnosticEvent {
        schema_version: PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
        sequence: 0,
        extension_id: record.extension_id.0,
        kind: match record.kind {
            PluginDiagnosticKind::Activation => WirePluginDiagnosticKind::Activation,
            PluginDiagnosticKind::Runtime => WirePluginDiagnosticKind::Runtime,
            PluginDiagnosticKind::Invocation => WirePluginDiagnosticKind::Invocation,
            PluginDiagnosticKind::Job => WirePluginDiagnosticKind::Job,
            PluginDiagnosticKind::Storage => WirePluginDiagnosticKind::Storage,
            PluginDiagnosticKind::Migration => WirePluginDiagnosticKind::Migration,
            PluginDiagnosticKind::Shutdown => WirePluginDiagnosticKind::Shutdown,
        },
        code: record.code,
        message: record.message,
        job_id: record.job_id,
        attempt: record.attempt,
        retryable: record.retryable,
        occurred_at_ms: record.occurred_at_ms,
    }
}

fn normalize_plugin_diagnostic_event(mut event: PluginDiagnosticEvent) -> PluginDiagnosticEvent {
    event.message = sanitize_plugin_diagnostic_message(&event.message);
    event
}

fn diagnostic_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn preview_source_error(asset_id: &AssetId, error: PreviewSourceError) -> CoreError {
    let retryable = matches!(&error, PreviewSourceError::Unavailable);
    let message = match error {
        PreviewSourceError::Unavailable => "preview source is unavailable".into(),
        PreviewSourceError::NotFound => "preview source was not found".into(),
        PreviewSourceError::PermissionDenied => "preview source access was denied".into(),
        PreviewSourceError::InvalidRequest(message) | PreviewSourceError::Failed(message) => {
            message
        }
    };
    CoreError::AssetSource {
        asset_id: asset_id.0.clone(),
        message,
        retryable,
    }
}

fn plugin_resource_error(
    extension_id: &ExtensionId,
    locale: &str,
    error: PluginError,
) -> CoreError {
    match error {
        PluginError::ResourceNotFound { .. } => CoreError::PluginResourceNotFound {
            extension_id: extension_id.0.clone(),
            locale: locale.into(),
        },
        PluginError::ResourceDenied { message, .. } => CoreError::PluginResource {
            extension_id: extension_id.0.clone(),
            locale: locale.into(),
            message,
        },
        PluginError::ResourceTooLarge { limit, .. } => CoreError::PluginResource {
            extension_id: extension_id.0.clone(),
            locale: locale.into(),
            message: format!("localization resource exceeds the {limit} byte limit"),
        },
        other => CoreError::PluginResource {
            extension_id: extension_id.0.clone(),
            locale: locale.into(),
            message: format!("{other:?}"),
        },
    }
}

fn job_error(error: JobError) -> CoreError {
    CoreError::Job(format!("{error:?}"))
}

fn valid_profile_state(state: &ProfileState) -> bool {
    state.schema_version == PROFILE_SCHEMA_VERSION && valid_locale(&state.locale)
}

fn valid_locale(locale: &str) -> bool {
    let mut segments = locale.split('-');
    let Some(language) = segments.next() else {
        return false;
    };
    if !(2..=8).contains(&language.len())
        || !language
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return false;
    }
    segments.all(|segment| {
        (1..=8).contains(&segment.len())
            && segment
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
    })
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
            .map(|mut core| core.drain_job_events())
            .unwrap_or_default()
    }

    pub fn drain_plugin_diagnostics(&self) -> Vec<PluginDiagnosticEvent> {
        self.core
            .lock()
            .map(|mut core| core.drain_plugin_diagnostics())
            .unwrap_or_default()
    }

    pub fn recovery_projection(&self) -> velvet_workbench_protocol::RecoveryProjection {
        self.core
            .lock()
            .map(|core| core.recovery_projection())
            .unwrap_or_else(|_| RecoverySupervisor::default().projection())
    }

    pub fn record_shutdown_intent(&self) -> Result<(), RecoveryError> {
        self.core
            .lock()
            .map_err(|_| RecoveryError::InvalidState("core lock poisoned".into()))?
            .record_shutdown_intent()
            .map(|_| ())
    }

    pub fn record_shutdown_complete(&self) -> Result<(), RecoveryError> {
        self.core
            .lock()
            .map_err(|_| RecoveryError::InvalidState("core lock poisoned".into()))?
            .record_shutdown_complete()
            .map(|_| ())
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
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use velvet_layout_model::{
        LayoutCommand, LayoutState, WorkbenchAppId, WorkbenchPart, WorkbenchZone,
    };
    use velvet_persistence::{
        InMemoryAssetSourceLocationStore, InMemoryDiagnosticEventStore, InMemoryLayoutStateStore,
        InMemoryProfileStateStore, InMemoryRecoveryJournalStore, InMemoryRecoverySetStore,
        InMemoryStateSnapshotStore, JsonFileAssetSourceLocationStore, JsonFileDiagnosticEventStore,
        JsonFileJobStateStore, JsonFileLayoutStateStore, LayoutStateStore, SnapshotJobStateStore,
        SnapshotLayoutStateStore, SnapshotProfileStateStore,
    };
    use velvet_plugin_api::{
        ContributionDescriptor, ContributionKind, ContributionScope, ExtensionId,
        ExtensionInstallRecoveryLogStatus, ExtensionManifest, HostContainer, LocalizationManifest,
        Permission, PreviewContributionDescriptor, PreviewIntent as ManifestPreviewIntent,
        PreviewSurface as ManifestPreviewSurface, RuntimeKind, UiSurfaceDescriptor, UiSurfaceKind,
        VersionRange, ZonePermissionSet,
    };
    use velvet_plugin_host::{
        PluginInvocation, PluginInvocationResult, PluginRuntime, RuntimeError, RuntimeHealth,
        RuntimeProcess,
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

    struct PreviewRuntime {
        fail_augmentation: bool,
        reject_action: bool,
        job_delay_ms: u64,
    }
    struct PreviewProcess {
        fail_augmentation: bool,
        reject_action: bool,
        job_delay_ms: u64,
    }

    struct FailingStartRuntime;

    impl PluginRuntime for FailingStartRuntime {
        fn start(
            &mut self,
            _manifest: &ExtensionManifest,
            _permissions: &velvet_plugin_host::PermissionGrant,
        ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
            Err(RuntimeError::StartFailed("runner unavailable".into()))
        }
    }

    impl RuntimeProcess for PreviewProcess {
        fn poll(&mut self) -> RuntimeHealth {
            RuntimeHealth::Running
        }

        fn shutdown(&mut self) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn invoke(
            &mut self,
            invocation: &PluginInvocation,
        ) -> Result<PluginInvocationResult, RuntimeError> {
            if self.fail_augmentation && invocation.method == "preview.augment" {
                return Err(RuntimeError::InvocationFailed(
                    "metadata renderer unavailable".into(),
                ));
            }
            if self.reject_action && invocation.method == "preview.action" {
                return Err(RuntimeError::InvocationRejected(
                    "preview action is unavailable".into(),
                ));
            }
            if invocation.method == "job.run" {
                if self.job_delay_ms > 0 {
                    std::thread::sleep(Duration::from_millis(self.job_delay_ms));
                }
                return Ok(PluginInvocationResult {
                    result: Some(serde_json::json!({
                        "contentType": "application/json",
                        "payload": {
                            "jobKind": invocation.command_id.clone(),
                            "accepted": true,
                        }
                    })),
                    error: None,
                    events: vec![velvet_plugin_host::PluginInvocationEvent {
                        kind: "progress".into(),
                        progress: Some(100),
                        message: Some("Completed".into()),
                    }],
                });
            }
            if invocation.method == "preview.action" {
                let result = if invocation.command_id == "preview.analyze" {
                    serde_json::json!({
                        "kind": "job",
                        "jobKind": "preview.metadata-analysis",
                        "durationMs": 900,
                    })
                } else {
                    serde_json::json!({ "kind": "accepted" })
                };
                return Ok(PluginInvocationResult {
                    result: Some(result),
                    error: None,
                    events: Vec::new(),
                });
            }
            let (session_id, entrypoint) = match invocation.method.as_str() {
                "preview.open" => ("preview.example:asset-1", "wasm://preview/image"),
                "preview.augment" => (
                    "preview.example:metadata:asset-1",
                    "wasm://preview/metadata",
                ),
                other => panic!("unexpected preview invocation method: {other}"),
            };
            Ok(PluginInvocationResult {
                result: Some(serde_json::json!({
                    "sessionId": session_id,
                    "renderer": {
                        "kind": "ui-bridge",
                        "entrypoint": entrypoint,
                        "bridgeVersion": "1"
                    },
                    "commands": if invocation.method == "preview.open" {
                        serde_json::json!(["preview.zoom"])
                    } else {
                        serde_json::json!(["preview.inspect-metadata", "preview.analyze"])
                    },
                    "slots": []
                })),
                error: None,
                events: Vec::new(),
            })
        }
    }

    impl PluginRuntime for PreviewRuntime {
        fn start(
            &mut self,
            _manifest: &ExtensionManifest,
            _permissions: &velvet_plugin_host::PermissionGrant,
        ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
            Ok(Box::new(PreviewProcess {
                fail_augmentation: self.fail_augmentation,
                reject_action: self.reject_action,
                job_delay_ms: self.job_delay_ms,
            }))
        }
    }

    fn preview_command(id: &str, title: &str) -> ContributionDescriptor {
        ContributionDescriptor {
            id: id.into(),
            kind: ContributionKind::Command,
            scope: ContributionScope::Workbench,
            namespace: Some("workbench".into()),
            container: None,
            zone: None,
            title: Some(title.into()),
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
        }
    }

    fn preview_job(id: &str) -> ContributionDescriptor {
        let mut job = preview_command(id, id);
        job.kind = ContributionKind::Job;
        job.scope = ContributionScope::Capability;
        job.namespace = Some("capability.preview.example".into());
        job.title = None;
        job
    }

    fn preview_manifest() -> ExtensionManifest {
        ExtensionManifest {
            id: ExtensionId("preview.example".into()),
            version: "0.1.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::BuiltIn,
            launch: None,
            target_app: None,
            contributions: vec![
                ContributionDescriptor {
                    id: "preview.image".into(),
                    kind: ContributionKind::PreviewProvider,
                    scope: ContributionScope::Capability,
                    namespace: Some("capability.preview.example".into()),
                    container: None,
                    zone: None,
                    title: None,
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
                    preview: Some(PreviewContributionDescriptor {
                        content_kinds: vec!["image".into()],
                        intents: vec![ManifestPreviewIntent::Quick],
                        surfaces: vec![ManifestPreviewSurface::Workspace],
                        priority: 20,
                        slot: None,
                    }),
                },
                ContributionDescriptor {
                    id: "preview.metadata".into(),
                    kind: ContributionKind::PreviewAugmentation,
                    scope: ContributionScope::Capability,
                    namespace: Some("capability.preview.example".into()),
                    container: None,
                    zone: None,
                    title: None,
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
                    preview: Some(PreviewContributionDescriptor {
                        content_kinds: vec!["image".into()],
                        intents: vec![ManifestPreviewIntent::Quick],
                        surfaces: vec![ManifestPreviewSurface::Workspace],
                        priority: 5,
                        slot: Some(velvet_plugin_api::PreviewAugmentationSlot::Metadata),
                    }),
                },
                preview_command("preview.zoom", "Zoom preview"),
                preview_command("preview.inspect-metadata", "Inspect preview metadata"),
                preview_command("preview.analyze", "Analyze preview metadata"),
                preview_job("preview.metadata-analysis"),
            ],
            permission_sets: vec![
                ZonePermissionSet {
                    zone: WorkbenchZone::Contextual,
                    permissions: vec![Permission::PreviewRender],
                },
                ZonePermissionSet {
                    zone: WorkbenchZone::Operations,
                    permissions: vec![Permission::JobsCreate],
                },
            ],
            ..ExtensionManifest::default()
        }
    }

    #[test]
    fn preview_queries_execute_active_provider_and_project_renderer_descriptor() {
        let mut core = ApplicationCore::with_plugin_runtime(
            Arc::new(InMemoryLayoutStateStore::default()),
            Box::new(PreviewRuntime {
                fail_augmentation: false,
                reject_action: false,
                job_delay_ms: 0,
            }),
        );
        let manifest = preview_manifest();
        let id = manifest.id.clone();
        core.register_plugin(manifest)
            .expect("preview provider should register");
        core.activate_plugin(&id)
            .expect("preview provider should activate");

        let response = core
            .dispatch(Request::Query(Query::ResolvePreview {
                asset_id: velvet_asset_core::AssetId("asset-1".into()),
                content_kind: "image".into(),
                intent: velvet_preview_core::PreviewIntent::Quick,
                surface: velvet_preview_core::PreviewSurface::Workspace,
            }))
            .expect("preview query should resolve");
        match response {
            Response::Preview(preview) => {
                assert_eq!(
                    preview.candidate.provider_id,
                    "preview.example.preview.image"
                );
                assert_eq!(
                    preview.session.descriptor.session_id,
                    "preview.example:asset-1"
                );
                assert!(matches!(
                    preview.session.descriptor.renderer,
                    velvet_preview_core::PreviewRenderer::UiBridge { .. }
                ));
                assert_eq!(preview.session.descriptor.commands, vec!["preview.zoom"]);
                assert!(preview.session.descriptor.slots.is_empty());
                assert_eq!(preview.augmentations.len(), 1);
                assert_eq!(
                    preview.augmentations[0].augmentation_id,
                    "preview.example.preview.metadata"
                );
                assert_eq!(preview.augmentations[0].slot, "metadata");
                assert!(preview.augmentations[0].diagnostic.is_none());
                assert!(matches!(
                    preview.augmentations[0]
                        .session
                        .as_ref()
                        .map(|session| &session.descriptor.renderer),
                    Some(velvet_preview_core::PreviewRenderer::UiBridge { .. })
                ));
            }
            other => panic!("expected preview response, got {other:?}"),
        }
    }

    #[test]
    fn preview_augmentation_failure_keeps_primary_preview_usable() {
        let mut core = ApplicationCore::with_plugin_runtime(
            Arc::new(InMemoryLayoutStateStore::default()),
            Box::new(PreviewRuntime {
                fail_augmentation: true,
                reject_action: false,
                job_delay_ms: 0,
            }),
        );
        let manifest = preview_manifest();
        let id = manifest.id.clone();
        core.register_plugin(manifest)
            .expect("preview provider should register");
        core.activate_plugin(&id)
            .expect("preview provider should activate");

        let response = core
            .dispatch(Request::Query(Query::ResolvePreview {
                asset_id: velvet_asset_core::AssetId("asset-1".into()),
                content_kind: "image".into(),
                intent: velvet_preview_core::PreviewIntent::Quick,
                surface: velvet_preview_core::PreviewSurface::Workspace,
            }))
            .expect("preview query should resolve");
        match response {
            Response::Preview(preview) => {
                assert_eq!(
                    preview.candidate.provider_id,
                    "preview.example.preview.image"
                );
                assert_eq!(preview.augmentations.len(), 1);
                assert!(preview.augmentations[0].session.is_none());
                assert_eq!(
                    preview.augmentations[0].diagnostic.as_deref(),
                    Some(
                        "preview augmentation failed: RuntimeInvocation { extension_id: ExtensionId(\"preview.example\"), message: \"metadata renderer unavailable\" }",
                    )
                );
            }
            other => panic!("expected preview response, got {other:?}"),
        }
    }

    #[test]
    fn preview_action_hands_job_creation_to_the_rust_job_system() {
        let mut core = ApplicationCore::with_plugin_runtime(
            Arc::new(InMemoryLayoutStateStore::default()),
            Box::new(PreviewRuntime {
                fail_augmentation: false,
                reject_action: false,
                job_delay_ms: 0,
            }),
        );
        let source = Arc::new(InMemoryPreviewAssetSource::default());
        source.insert_with_fingerprint(
            velvet_asset_core::AssetId("asset-1".into()),
            velvet_preview_core::PreviewAssetContent::Text {
                text: "source-v1".into(),
                truncated: false,
            },
            velvet_asset_core::ContentFingerprint("sha256:source-v1".into()),
        );
        core.set_preview_asset_source(source);
        let manifest = preview_manifest();
        let id = manifest.id.clone();
        core.register_plugin(manifest)
            .expect("preview action provider should register");
        core.activate_plugin(&id)
            .expect("preview action provider should activate");

        core.dispatch(Request::Query(Query::ResolvePreview {
            asset_id: velvet_asset_core::AssetId("asset-1".into()),
            content_kind: "image".into(),
            intent: velvet_preview_core::PreviewIntent::Quick,
            surface: velvet_preview_core::PreviewSurface::Workspace,
        }))
        .expect("preview session should authorize its actions");

        let invalid_action = core.dispatch(Request::Command(Command::InvokePreviewAction {
            extension_id: id.clone(),
            command_id: "preview.not-declared".into(),
            args: serde_json::Value::Null,
            context: velvet_plugin_api::PreviewActionContext {
                session_id: "preview.example:metadata:asset-1".into(),
                provider_id: "preview.example.preview.metadata".into(),
                asset_id: "asset-1".into(),
                content_kind: "image".into(),
                intent: velvet_plugin_api::PreviewIntent::Quick,
                surface: velvet_plugin_api::PreviewSurface::Workspace,
            },
        }));
        assert!(matches!(
            invalid_action,
            Err(CoreError::InvalidCommand(message))
                if message.contains("not declared by the active preview session")
        ));

        let mismatched_context = core.dispatch(Request::Command(Command::InvokePreviewAction {
            extension_id: id.clone(),
            command_id: "preview.zoom".into(),
            args: serde_json::Value::Null,
            context: velvet_plugin_api::PreviewActionContext {
                session_id: "preview.example:asset-1".into(),
                provider_id: "preview.example.preview.image".into(),
                asset_id: "asset-1".into(),
                content_kind: "image".into(),
                intent: velvet_plugin_api::PreviewIntent::Quick,
                surface: velvet_plugin_api::PreviewSurface::Inspector,
            },
        }));
        assert!(matches!(
            mismatched_context,
            Err(CoreError::InvalidCommand(message))
                if message.contains("not declared by the active preview session")
        ));

        let response = core
            .dispatch(Request::Command(Command::InvokePreviewAction {
                extension_id: id,
                command_id: "preview.analyze".into(),
                args: serde_json::json!({ "scope": "metadata" }),
                context: velvet_plugin_api::PreviewActionContext {
                    session_id: "preview.example:metadata:asset-1".into(),
                    provider_id: "preview.example.preview.metadata".into(),
                    asset_id: "asset-1".into(),
                    content_kind: "image".into(),
                    intent: velvet_plugin_api::PreviewIntent::Quick,
                    surface: velvet_plugin_api::PreviewSurface::Workspace,
                },
            }))
            .expect("preview action should create a job");
        let job_id = match response {
            Response::Job(job) => {
                assert_eq!(job.kind, "preview.metadata-analysis");
                assert_eq!(job.status, velvet_job_system::JobStatus::Queued);
                assert_eq!(job.provenance.owner_id, "preview.example");
                assert_eq!(job.provenance.asset_id.as_deref(), Some("asset-1"));
                assert_eq!(
                    job.provenance.provider_id.as_deref(),
                    Some("preview.example.preview.metadata")
                );
                assert_eq!(job.provenance.provider_version.as_deref(), Some("0.1.0"));
                assert_eq!(
                    job.provenance.input_fingerprint.as_deref(),
                    Some("sha256:source-v1")
                );
                job.id
            }
            other => panic!("expected job response, got {other:?}"),
        };
        for _ in 0..20 {
            if let Some(job) = core
                .job_snapshots()
                .into_iter()
                .find(|job| job.id == job_id)
            {
                if job.status == velvet_job_system::JobStatus::Completed {
                    assert_eq!(
                        job.result
                            .as_ref()
                            .map(|result| result.content_type.as_str()),
                        Some("application/json")
                    );
                    assert_eq!(
                        job.result
                            .as_ref()
                            .map(|result| result.payload["accepted"].as_bool()),
                        Some(Some(true))
                    );
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!(
            "plugin Job runner did not complete: {:?}",
            core.job_snapshots()
        );
    }

    #[test]
    fn disabling_plugin_cancels_in_flight_job_and_retries_without_fallback() {
        let mut core = ApplicationCore::with_plugin_runtime(
            Arc::new(InMemoryLayoutStateStore::default()),
            Box::new(PreviewRuntime {
                fail_augmentation: false,
                reject_action: false,
                job_delay_ms: 250,
            }),
        );
        let manifest = preview_manifest();
        let id = manifest.id.clone();
        core.register_plugin(manifest)
            .expect("preview action provider should register");
        core.activate_plugin(&id)
            .expect("preview action provider should activate");
        core.dispatch(Request::Query(Query::ResolvePreview {
            asset_id: velvet_asset_core::AssetId("asset-1".into()),
            content_kind: "image".into(),
            intent: velvet_preview_core::PreviewIntent::Quick,
            surface: velvet_preview_core::PreviewSurface::Workspace,
        }))
        .expect("preview session should authorize its actions");

        let response = core
            .dispatch(Request::Command(Command::InvokePreviewAction {
                extension_id: id.clone(),
                command_id: "preview.analyze".into(),
                args: serde_json::json!({ "scope": "metadata" }),
                context: velvet_plugin_api::PreviewActionContext {
                    session_id: "preview.example:metadata:asset-1".into(),
                    provider_id: "preview.example.preview.metadata".into(),
                    asset_id: "asset-1".into(),
                    content_kind: "image".into(),
                    intent: velvet_plugin_api::PreviewIntent::Quick,
                    surface: velvet_plugin_api::PreviewSurface::Workspace,
                },
            }))
            .expect("preview action should create a job");
        let job_id = match response {
            Response::Job(job) => job.id,
            other => panic!("expected job response, got {other:?}"),
        };

        for _ in 0..50 {
            if core
                .job_snapshots()
                .into_iter()
                .any(|job| job.id == job_id && job.status == velvet_job_system::JobStatus::Running)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(core.job_snapshots().into_iter().any(|job| {
            job.id == job_id && job.status == velvet_job_system::JobStatus::Running
        }));

        core.disable_plugin(&id, DisableReason::User)
            .expect("plugin disable should succeed");
        let cancelled = core
            .job_snapshots()
            .into_iter()
            .find(|job| job.id == job_id)
            .expect("cancelled job should remain persisted");
        assert_eq!(cancelled.status, velvet_job_system::JobStatus::Cancelled);
        assert!(cancelled.retryable);
        assert_eq!(cancelled.result, None);
        assert_eq!(
            cancelled.message.as_deref(),
            Some("Cancelled because the owner was disabled")
        );

        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            core.job_snapshots()
                .into_iter()
                .find(|job| job.id == job_id)
                .map(|job| job.status),
            Some(velvet_job_system::JobStatus::Cancelled)
        );

        core.retry_job(&job_id)
            .expect("cancelled plugin job should be retryable");
        for _ in 0..50 {
            if core
                .job_snapshots()
                .into_iter()
                .find(|job| job.id == job_id)
                .is_some_and(|job| job.status == velvet_job_system::JobStatus::Failed)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let failed_retry = core
            .job_snapshots()
            .into_iter()
            .find(|job| job.id == job_id)
            .expect("retried job should remain persisted");
        assert_eq!(failed_retry.status, velvet_job_system::JobStatus::Failed);
        assert!(failed_retry.retryable);
        assert!(failed_retry
            .message
            .as_deref()
            .is_some_and(|message| message.contains("runner unavailable")));
        assert_eq!(failed_retry.result, None);
    }

    #[test]
    fn rejected_preview_action_keeps_the_plugin_and_session_available() {
        let mut core = ApplicationCore::with_plugin_runtime(
            Arc::new(InMemoryLayoutStateStore::default()),
            Box::new(PreviewRuntime {
                fail_augmentation: false,
                reject_action: true,
                job_delay_ms: 0,
            }),
        );
        let manifest = preview_manifest();
        let id = manifest.id.clone();
        core.register_plugin(manifest)
            .expect("preview action provider should register");
        core.activate_plugin(&id)
            .expect("preview action provider should activate");
        core.dispatch(Request::Query(Query::ResolvePreview {
            asset_id: velvet_asset_core::AssetId("asset-1".into()),
            content_kind: "image".into(),
            intent: velvet_preview_core::PreviewIntent::Quick,
            surface: velvet_preview_core::PreviewSurface::Workspace,
        }))
        .expect("preview session should authorize its actions");

        let error = core
            .dispatch(Request::Command(Command::InvokePreviewAction {
                extension_id: id.clone(),
                command_id: "preview.zoom".into(),
                args: serde_json::Value::Null,
                context: velvet_plugin_api::PreviewActionContext {
                    session_id: "preview.example:asset-1".into(),
                    provider_id: "preview.example.preview.image".into(),
                    asset_id: "asset-1".into(),
                    content_kind: "image".into(),
                    intent: velvet_plugin_api::PreviewIntent::Quick,
                    surface: velvet_plugin_api::PreviewSurface::Workspace,
                },
            }))
            .expect_err("runtime rejection should stay local to the action");
        assert!(
            matches!(error, CoreError::Plugin(message) if message.contains("preview action failed"))
        );

        let summary = core
            .dispatch(Request::Query(Query::Extensions))
            .expect("plugin inventory should remain available");
        assert!(matches!(
            summary,
            Response::Extensions(extensions)
                if extensions.iter().any(|extension| extension.id == id && extension.active)
        ));
    }

    #[test]
    fn built_in_preview_providers_use_the_injected_source_and_keep_fallback_available() {
        let source = Arc::new(InMemoryPreviewAssetSource::default());
        source.insert_text(
            velvet_asset_core::AssetId("asset-text".into()),
            "built-in text preview",
        );
        let mut core = ApplicationCore::default();
        core.set_preview_asset_source(source);

        let response = core
            .dispatch(Request::Query(Query::ResolvePreview {
                asset_id: velvet_asset_core::AssetId("asset-text".into()),
                content_kind: "text/plain".into(),
                intent: velvet_preview_core::PreviewIntent::Quick,
                surface: velvet_preview_core::PreviewSurface::Workspace,
            }))
            .expect("the built-in preview should resolve");
        match response {
            Response::Preview(preview) => {
                assert_eq!(preview.candidate.provider_id, "builtin.text");
                assert!(matches!(
                    preview.session.descriptor.renderer,
                    velvet_preview_core::PreviewRenderer::Text { ref text, truncated: false }
                        if text == "built-in text preview"
                ));
            }
            other => panic!("expected preview response, got {other:?}"),
        }

        let fallback = core
            .dispatch(Request::Query(Query::ResolvePreview {
                asset_id: velvet_asset_core::AssetId("missing".into()),
                content_kind: "text/plain".into(),
                intent: velvet_preview_core::PreviewIntent::Quick,
                surface: velvet_preview_core::PreviewSurface::Workspace,
            }))
            .expect("provider failure should preserve generic fallback");
        match fallback {
            Response::Preview(preview) => {
                assert_eq!(preview.candidate.provider_id, "builtin.generic-fallback");
                assert!(preview.diagnostic.is_some());
            }
            other => panic!("expected fallback preview response, got {other:?}"),
        }
    }

    #[test]
    fn persistent_asset_locations_survive_restart_and_stay_path_free_at_the_protocol() {
        let source_path = temporary_layout_path().with_extension("preview.txt");
        let plugin_root = temporary_layout_path().with_extension("plugins");
        let jobs_path = temporary_layout_path().with_extension("jobs.json");
        let asset_sources_path = temporary_layout_path().with_extension("asset-sources.json");
        fs::write(&source_path, "restartable preview")
            .expect("the preview source should be writable");

        let build_core = || {
            ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store(
                Arc::new(InMemoryLayoutStateStore::default()),
                &plugin_root,
                Arc::new(JsonFileJobStateStore::new(&jobs_path)),
                Arc::new(InMemoryProfileStateStore::default()),
                Arc::new(JsonFileAssetSourceLocationStore::new(&asset_sources_path)),
            )
        };

        let mut first = build_core();
        assert_eq!(
            first
                .dispatch(Request::Command(Command::RegisterPreviewAsset {
                    asset_id: AssetId("asset-restart".into()),
                    source_path: source_path.to_string_lossy().into_owned(),
                    content_kind: "text/plain".into(),
                }))
                .expect("the source location should register"),
            Response::Accepted
        );
        assert!(matches!(
            first
                .dispatch(Request::Query(Query::ResolvePreviewAsset {
                    asset_id: AssetId("asset-restart".into()),
                    intent: velvet_preview_core::PreviewIntent::Quick,
                    surface: velvet_preview_core::PreviewSurface::Workspace,
                }))
                .expect("the registered asset should resolve"),
            Response::Preview(preview)
                if matches!(
                    preview.session.descriptor.renderer,
                    velvet_preview_core::PreviewRenderer::Text { ref text, truncated: false }
                        if text == "restartable preview"
                )
        ));
        drop(first);

        let mut restarted = build_core();
        assert!(matches!(
            restarted
                .dispatch(Request::Query(Query::ResolvePreviewAsset {
                    asset_id: AssetId("asset-restart".into()),
                    intent: velvet_preview_core::PreviewIntent::Quick,
                    surface: velvet_preview_core::PreviewSurface::Workspace,
                }))
                .expect("the persisted asset should resolve after restart"),
            Response::Preview(_)
        ));
        restarted
            .dispatch(Request::Command(Command::RemovePreviewAsset {
                asset_id: AssetId("asset-restart".into()),
            }))
            .expect("the source location should be removed");
        assert!(matches!(
            restarted.dispatch(Request::Query(Query::ResolvePreviewAsset {
                asset_id: AssetId("asset-restart".into()),
                intent: velvet_preview_core::PreviewIntent::Quick,
                surface: velvet_preview_core::PreviewSurface::Workspace,
            })),
            Err(CoreError::AssetSource { asset_id, .. }) if asset_id == "asset-restart"
        ));

        let _ = fs::remove_file(source_path);
        let _ = fs::remove_file(jobs_path);
        let _ = fs::remove_file(asset_sources_path);
        let _ = fs::remove_dir_all(plugin_root);
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
    fn profile_locale_is_rust_owned_revisioned_and_persisted() {
        let profile_store = Arc::new(InMemoryProfileStateStore::default());
        let mut core = ApplicationCore::with_store_and_profile_store(
            Arc::new(InMemoryLayoutStateStore::default()),
            profile_store.clone(),
        );

        assert_eq!(
            core.dispatch(Request::Query(Query::Profile))
                .expect("profile query should succeed"),
            Response::Profile(ProfileState::default())
        );
        let changed = core
            .dispatch(Request::Command(Command::SetLocale {
                locale: "zh-Hans".into(),
                expected_revision: Some(0),
            }))
            .expect("locale change should succeed");
        assert_eq!(
            changed,
            Response::Profile(ProfileState {
                schema_version: PROFILE_SCHEMA_VERSION,
                revision: 1,
                locale: "zh-Hans".into(),
            })
        );
        assert_eq!(
            profile_store
                .load()
                .expect("profile store should load")
                .expect("changed profile should be persisted")
                .locale,
            "zh-Hans"
        );
        assert!(matches!(
            core.dispatch(Request::Command(Command::SetLocale {
                locale: "fr".into(),
                expected_revision: Some(0),
            })),
            Err(CoreError::StaleRevision {
                expected: 0,
                actual: 1,
            })
        ));
        assert!(matches!(
            core.dispatch(Request::Command(Command::SetLocale {
                locale: "not a locale".into(),
                expected_revision: Some(1),
            })),
            Err(CoreError::Profile(_))
        ));
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
    fn recovery_query_returns_the_authoritative_startup_projection() {
        let mut core = ApplicationCore::default();
        let response = core
            .dispatch(Request::Query(Query::Recovery))
            .expect("recovery query should succeed");

        match response {
            Response::Recovery(projection) => {
                assert_eq!(projection.schema_version, 1);
                assert_eq!(projection.attempt_id, 1);
                assert!(matches!(
                    projection.phase,
                    velvet_workbench_protocol::StartupPhase::CoreReady
                ));
            }
            other => panic!("expected a recovery projection, got {other:?}"),
        }
    }

    #[test]
    fn recovery_diagnostics_export_is_bounded_and_uses_the_authoritative_projection() {
        let destination = temporary_layout_path().with_extension("recovery.json");
        let core = ApplicationCore::default();
        core.record_recovery_failure(
            velvet_workbench_protocol::RecoveryFailureClass::OptionalComponent,
            "x".repeat(2_000),
        );
        core.export_recovery_diagnostics(&destination)
            .expect("recovery diagnostics should export");

        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&destination).expect("export should be readable"))
                .expect("export should be valid JSON");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["projection"]["schemaVersion"], 1);
        assert_eq!(value["pluginDiagnostics"], serde_json::json!([]));
        assert_eq!(value["diagnosticsDegraded"], false);
        assert_eq!(
            value["projection"]["lastFailure"]["message"]
                .as_str()
                .expect("failure message should be text")
                .chars()
                .count(),
            512
        );
        let _ = fs::remove_file(destination);
    }

    #[test]
    fn diagnostic_history_survives_core_restart_without_blocking_core_startup() {
        let diagnostic_store = Arc::new(InMemoryDiagnosticEventStore::default());
        let build_core = || {
            ApplicationCore::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile_and_recovery(
                Arc::new(InMemoryLayoutStateStore::default()),
                Box::new(velvet_plugin_host::ProcessPluginRuntime),
                None,
                JobSystem::default(),
                Arc::new(InMemoryProfileStateStore::default()),
                Arc::new(InMemoryAssetSourceLocationStore::default()),
                ApplicationRecoveryStores::new(
                    Arc::new(InMemoryRecoveryJournalStore::default()),
                    None,
                    None,
                )
                .with_diagnostics(diagnostic_store.clone()),
            )
        };

        let mut first = build_core();
        first.enqueue_plugin_diagnostic(PluginDiagnosticEvent {
            schema_version: PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
            sequence: 0,
            extension_id: "sample.reader".into(),
            kind: WirePluginDiagnosticKind::Runtime,
            code: "runtime-crashed".into(),
            message: "failed at /Users/example/private/file.txt".into(),
            job_id: None,
            attempt: None,
            retryable: true,
            occurred_at_ms: 42,
        });
        assert_eq!(first.diagnostic_bundle().plugin_diagnostics.len(), 1);
        drop(first);

        let restarted = build_core();
        let bundle = restarted.diagnostic_bundle();
        assert_eq!(bundle.plugin_diagnostics.len(), 1);
        assert_eq!(bundle.plugin_diagnostics[0].sequence, 1);
        assert_eq!(
            bundle.plugin_diagnostics[0].message,
            "failed at <redacted-path>"
        );
        assert!(!bundle.diagnostics_degraded);
        assert!(!restarted.in_safe_mode());
    }

    #[test]
    fn corrupt_diagnostic_history_degrades_observability_without_blocking_startup() {
        let diagnostic_path = temporary_layout_path().with_extension("diagnostics.json");
        fs::write(&diagnostic_path, br#"not-json"#)
            .expect("the corrupt diagnostics fixture should be writable");
        let core = ApplicationCore::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile_and_recovery(
            Arc::new(InMemoryLayoutStateStore::default()),
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            None,
            JobSystem::default(),
            Arc::new(InMemoryProfileStateStore::default()),
            Arc::new(InMemoryAssetSourceLocationStore::default()),
            ApplicationRecoveryStores::new(
                Arc::new(InMemoryRecoveryJournalStore::default()),
                None,
                None,
            )
            .with_diagnostics(Arc::new(JsonFileDiagnosticEventStore::new(&diagnostic_path))),
        );

        assert!(!core.in_safe_mode());
        assert!(core.diagnostic_bundle().diagnostics_degraded);
        let _ = fs::remove_file(diagnostic_path);
    }

    #[test]
    fn core_migration_failure_enters_safe_mode_without_overwriting_the_failed_state() {
        let store = Arc::new(InMemoryLayoutStateStore::default());
        let invalid = LayoutState {
            schema_version: 99,
            ..LayoutState::default()
        };
        store
            .save(&invalid)
            .expect("the invalid layout fixture should be writable");

        let mut core = ApplicationCore::with_store(store.clone());
        let projection = core.recovery_projection();
        assert_eq!(
            projection.mode,
            velvet_workbench_protocol::RecoveryMode::Safe
        );
        assert!(matches!(
            projection.last_failure.map(|failure| failure.class),
            Some(velvet_workbench_protocol::RecoveryFailureClass::CoreMigration)
        ));
        core.mark_workbench_ready();
        core.mark_ready();
        assert!(core.in_safe_mode());
        assert_eq!(
            store
                .load()
                .expect("the failed layout should remain readable")
                .expect("the failed layout should remain present")
                .schema_version,
            99
        );

        core.register_plugin(ExtensionManifest {
            id: ExtensionId("safe-mode.plugin".into()),
            runtime: RuntimeKind::BuiltIn,
            launch: None,
            ..ExtensionManifest::default()
        })
        .expect("the safe-mode plugin fixture should register");
        assert!(core.activate_registered_plugins().is_empty());
        assert!(!core.discover_plugins()[0].active);
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
    fn optional_plugin_start_failure_degrades_without_entering_core_safe_mode() {
        let mut core = ApplicationCore::with_plugin_runtime(
            Arc::new(InMemoryLayoutStateStore::default()),
            Box::new(FailingStartRuntime),
        );
        core.register_plugin(ExtensionManifest {
            id: ExtensionId("optional.broken".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Sidecar,
            launch: Some(velvet_plugin_api::RuntimeLaunchSpec {
                program: "/velvet/test/does-not-exist".into(),
                args: Vec::new(),
            }),
            target_app: None,
            contributions: Vec::new(),
            permission_sets: Vec::new(),
            ..ExtensionManifest::default()
        })
        .expect("optional plugin should register");

        let failures = core.activate_registered_plugins();
        assert_eq!(failures.len(), 1);
        assert!(matches!(
            core.recovery_projection().mode,
            velvet_workbench_protocol::RecoveryMode::Degraded
        ));
        assert_eq!(core.recovery_projection().consecutive_failures, 0);
    }

    #[test]
    fn native_registration_rejects_unscoped_external_runtime() {
        let mut core = ApplicationCore::default();
        let manifest = ExtensionManifest {
            id: ExtensionId("unscoped.external".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Sidecar,
            launch: Some(velvet_plugin_api::RuntimeLaunchSpec {
                program: "/tmp/untrusted-sidecar".into(),
                args: Vec::new(),
            }),
            target_app: None,
            contributions: Vec::new(),
            permission_sets: Vec::new(),
            ..ExtensionManifest::default()
        };

        let error = core
            .dispatch(Request::Command(Command::RegisterPlugin {
                manifest: Box::new(manifest.into()),
            }))
            .expect_err("native registration must require an installed package root");
        assert!(matches!(
            error,
            CoreError::Plugin(message) if message.contains("committed package")
        ));
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
                preview: None,
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
                preview: None,
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
    fn application_core_reads_declared_plugin_localization_without_exposing_paths() {
        let root = temporary_layout_path().with_extension("plugins");
        let package = root.join("local.reader");
        fs::create_dir_all(package.join("l10n")).expect("the plugin package should be writable");
        let manifest = ExtensionManifest {
            id: ExtensionId("local.reader".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::BuiltIn,
            launch: None,
            target_app: None,
            contributions: vec![],
            permission_sets: vec![],
            localization: Some(LocalizationManifest {
                default_locale: "en".into(),
                supported_locales: vec!["en".into(), "zh-Hans".into()],
                resource_root: "l10n".into(),
                bundle_schema: 1,
            }),
            ..ExtensionManifest::default()
        };
        fs::write(
            package.join("manifest.json"),
            serde_json::to_vec(&manifest).expect("the manifest should serialize"),
        )
        .expect("the manifest should be written");
        fs::write(package.join("l10n/en.json"), br#"{"ready":"Ready"}"#)
            .expect("the localization bundle should be written");

        let mut core = ApplicationCore::with_store_and_plugin_directory(
            Arc::new(InMemoryLayoutStateStore::default()),
            &root,
        );
        core.load_plugins_from_directory(&root)
            .expect("the plugin package should be discovered");
        let resource = core
            .dispatch(Request::Query(Query::PluginLocalization {
                extension_id: manifest.id.clone(),
                locale: "en".into(),
            }))
            .expect("the declared localization bundle should load");
        assert!(matches!(
            resource,
            Response::PluginResource(resource)
                if resource.extension_id == manifest.id
                    && resource.locale == "en"
                    && resource.bytes == br#"{"ready":"Ready"}"#
        ));
        assert!(matches!(
            core.dispatch(Request::Query(Query::PluginLocalization {
                extension_id: manifest.id,
                locale: "fr".into(),
            })),
            Err(CoreError::PluginResourceNotFound { .. })
        ));

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
                input: serde_json::Value::Null,
                provenance: velvet_job_system::JobProvenance::for_owner("test"),
                result: None,
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

    #[test]
    fn snapshot_backed_owners_restore_the_previous_recovery_set_and_quarantine_failed_state() {
        let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
        let recovery_sets = Arc::new(InMemoryRecoverySetStore::default());
        let plugin_root = temporary_layout_path().with_extension("plugins");
        let asset_sources = Arc::new(InMemoryAssetSourceLocationStore::default());

        let build_core = || {
            ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store_and_recovery_stores(
                Arc::new(SnapshotLayoutStateStore::new(
                    snapshots.clone(),
                    Some(recovery_sets.clone()),
                    Arc::new(InMemoryLayoutStateStore::default()),
                )),
                &plugin_root,
                Arc::new(SnapshotJobStateStore::new(
                    snapshots.clone(),
                    Some(recovery_sets.clone()),
                    Arc::new(velvet_job_system::InMemoryJobStateStore::default()),
                )),
                Arc::new(SnapshotProfileStateStore::new(
                    snapshots.clone(),
                    Some(recovery_sets.clone()),
                    Arc::new(InMemoryProfileStateStore::default()),
                )),
                asset_sources.clone(),
                ApplicationRecoveryStores::new(
                    Arc::new(InMemoryRecoveryJournalStore::default()),
                    Some(snapshots.clone()),
                    Some(recovery_sets.clone()),
                ),
            )
        };

        let mut first = build_core();
        first
            .dispatch(Request::Command(Command::Layout(
                LayoutCommand::SetPartVisibility {
                    part: WorkbenchPart::PrimarySidebar,
                    visible: false,
                },
            )))
            .expect("first layout generation should persist");
        first
            .dispatch(Request::Command(Command::SetLocale {
                locale: "zh-Hans".into(),
                expected_revision: Some(0),
            }))
            .expect("first profile generation should persist");
        let first_set = recovery_sets
            .load_latest(CORE_RECOVERY_SET_ID)
            .expect("first recovery set should load")
            .expect("first recovery set should be published");

        let mut second = build_core();
        second
            .dispatch(Request::Command(Command::Layout(
                LayoutCommand::ActivateApp(WorkbenchAppId::FeedWorkbench),
            )))
            .expect("second layout generation should persist");
        second
            .dispatch(Request::Command(Command::SetLocale {
                locale: "en-US".into(),
                expected_revision: Some(1),
            }))
            .expect("second profile generation should persist");
        assert!(
            recovery_sets
                .load_latest(CORE_RECOVERY_SET_ID)
                .expect("second recovery set should load")
                .expect("second recovery set should be published")
                .generation
                > first_set.generation
        );

        second
            .dispatch(Request::Command(Command::RestoreRecoverySet))
            .expect("restore command should publish the previous set");
        let restored = build_core();
        assert_eq!(restored.profile_state().locale, "zh-Hans");
        assert_eq!(
            restored.layout_snapshot().active_app,
            WorkbenchAppId::FeedWorkbench
        );
        assert!(
            !restored
                .layout_snapshot()
                .parts
                .get(&WorkbenchPart::PrimarySidebar)
                .expect("primary sidebar should exist")
                .visible
        );

        let mut third = build_core();
        third
            .dispatch(Request::Command(Command::Layout(
                LayoutCommand::ActivateApp(WorkbenchAppId::AssetManager),
            )))
            .expect("a new layout generation should persist");
        third.record_recovery_failure_for_owner(
            velvet_workbench_protocol::RecoveryFailureClass::CoreMigration,
            "fixture migration failure",
            "layout",
            "workspace",
            3,
        );
        third
            .dispatch(Request::Command(Command::QuarantineRecoveryFailure))
            .expect("quarantine command should isolate the failed generation");
        assert!(snapshots
            .load_generation("layout", "workspace", 3)
            .expect("quarantined generation lookup should work")
            .is_none());
        let after_quarantine = build_core();
        assert_eq!(
            after_quarantine.layout_snapshot().active_app,
            WorkbenchAppId::FeedWorkbench
        );
        let _ = fs::remove_dir_all(plugin_root);
    }
}
