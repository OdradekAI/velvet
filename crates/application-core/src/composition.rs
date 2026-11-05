//! ApplicationCore composition, coordination and transport dispatch.
//!
//! This private module is the composition root for the domain owners. The
//! public crate facade re-exports its stable types while the owner-specific
//! helpers remain in the sibling modules.

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "bundle.rs"]
mod bundle;
#[path = "bundle_participants.rs"]
mod bundle_participants;
#[path = "operation.rs"]
mod operation;
#[path = "preview_source.rs"]
mod preview_source;
#[path = "recovery.rs"]
mod recovery;
#[path = "release.rs"]
mod release;

pub use operation::{
    FileAssetOperationProvider, OperationCoordinator, OperationCoordinatorError,
    OperationExecution, OperationPhase, OperationProvider, OperationProviderError,
    ASSET_OPERATION_OWNER,
};

pub use bundle::{
    BundleImportError, BundleImportParticipant, BundleImportParticipantError,
    BundleInspectionContext, BundleInspectionError, PortableBundleImporter,
    PortableBundleInspector,
};

pub use bundle_participants::{
    AssetBundleParticipant, LayoutBundleParticipant, ProfileBundleParticipant, ASSET_BUNDLE_OWNER,
    ASSET_BUNDLE_SCHEMA_VERSION, ASSET_BUNDLE_SCOPE, LAYOUT_BUNDLE_OWNER,
    LAYOUT_BUNDLE_SCHEMA_VERSION, LAYOUT_BUNDLE_SCOPE, PROFILE_BUNDLE_OWNER, PROFILE_BUNDLE_SCOPE,
};

pub use release::{
    canonical_unsigned_manifest, ReleaseTrustPolicy, ReleaseUpdateError, ReleaseUpdateManager,
    RELEASE_HEALTH_CHECKPOINT,
};

pub use recovery::{
    CrashCaptureAdapter, RecoveryClock, RecoveryError, RecoverySupervisor, StartupDeadlinePolicy,
    SystemRecoveryClock,
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
    pub plugin_trust: Option<Arc<dyn velvet_plugin_host::PluginTrustStore>>,
    pub durability_report: Option<velvet_workbench_protocol::DurabilityReport>,
    pub release_updates: Option<Arc<dyn velvet_persistence::ReleaseUpdateStore>>,
    pub release_packages: Option<Arc<dyn velvet_persistence::ReleasePackageStore>>,
    pub release_trust: Option<ReleaseTrustPolicy>,
    pub operation_receipts: Option<Arc<dyn velvet_persistence::OperationReceiptStore>>,
    pub import_quarantine: Option<Arc<dyn velvet_persistence::ImportQuarantineStore>>,
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
            plugin_trust: None,
            durability_report: None,
            release_updates: None,
            release_packages: None,
            release_trust: None,
            operation_receipts: None,
            import_quarantine: None,
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

    pub fn with_plugin_trust(
        mut self,
        plugin_trust: Arc<dyn velvet_plugin_host::PluginTrustStore>,
    ) -> Self {
        self.plugin_trust = Some(plugin_trust);
        self
    }

    pub fn with_durability_report(
        mut self,
        durability_report: velvet_workbench_protocol::DurabilityReport,
    ) -> Self {
        self.durability_report = Some(durability_report);
        self
    }

    pub fn with_release_updates(
        mut self,
        store: Arc<dyn velvet_persistence::ReleaseUpdateStore>,
        trust: ReleaseTrustPolicy,
    ) -> Self {
        self.release_updates = Some(store);
        self.release_trust = Some(trust);
        self
    }

    pub fn with_release_packages(
        mut self,
        store: Arc<dyn velvet_persistence::ReleasePackageStore>,
    ) -> Self {
        self.release_packages = Some(store);
        self
    }

    pub fn with_operation_receipts(
        mut self,
        store: Arc<dyn velvet_persistence::OperationReceiptStore>,
    ) -> Self {
        self.operation_receipts = Some(store);
        self
    }

    pub fn with_import_quarantine(
        mut self,
        store: Arc<dyn velvet_persistence::ImportQuarantineStore>,
    ) -> Self {
        self.import_quarantine = Some(store);
        self
    }

    pub fn journal_only(journal: Arc<dyn RecoveryJournalStore>) -> Self {
        Self::new(journal, None, None)
    }
}

use velvet_asset_core::{AssetId, ContentFingerprint, ContentKindId};
use velvet_job_system::{
    JobBudgetRequest, JobError, JobExecutionContext, JobExecutionEnvironment, JobExecutionError,
    JobExecutionPolicy, JobExecutionResult, JobId, JobProvenance, JobRunner, JobSpec,
    JobStateStore, JobStatus, JobSystem, StorageAvailability, JOB_SCHEMA_VERSION,
};
use velvet_layout_model::{InMemoryLayoutService, LayoutService};
use velvet_persistence::{
    AssetSourceLocationStore, DiagnosticEventStore, InMemoryAssetSourceLocationStore,
    InMemoryLayoutStateStore, InMemoryProfileStateStore, InMemoryRecoveryJournalStore,
    LayoutStateStore, ProfileStateStore, RecoveryJournalStore, RecoverySetStore,
    StateSnapshotStore, CORE_RECOVERY_SET_ID, MAX_DIAGNOSTIC_EVENTS,
};
use velvet_plugin_api::{
    Capability, ContributionKind, ExtensionId, ExtensionInstallPreview, ExtensionInstallRecovery,
    ExtensionInstallRecoveryAction, ExtensionInstallRecoveryLogEntry, ExtensionManifest,
    ExtensionState, Permission, PreviewActionContext, PreviewActionResult, PreviewIntent,
    PreviewRendererDescriptor, PreviewSurface,
};
use velvet_plugin_host::{
    sanitize_plugin_diagnostic_message, CapabilityDenyReason, CapabilityGrantSnapshot,
    DisableReason, ExtensionStorageRecovery, ExtensionStorageRecoveryStatus, InMemoryPluginHost,
    JsonManifestDiscovery, ManifestDiscoveryError, ManifestLoadReport, PluginDiagnosticKind,
    PluginDiagnosticRecord, PluginError, PluginHost, PluginJobHandle, PluginJobInvocation,
    PluginResourceLoader, PluginRuntime, RuntimeCrashEvidence,
};
use velvet_preview_core::{
    BuiltinImagePreviewProvider, BuiltinTextPreviewProvider, GenericFallbackProvider,
    PreviewAssetSource, PreviewCandidate, PreviewContext, PreviewProvider, PreviewRenderer,
    PreviewRequest, PreviewResolver, PreviewSession, PreviewSessionDescriptor, PreviewSourceError,
    ResolvedPreviewAugmentation,
};
use velvet_workbench_protocol::{
    Command, CoreError, CoreTransport, CrashCaptureEvidence, ImportPlan, ImportQuarantineEntry,
    ImportQuarantineSummary, ImportReceipt, OperationError, OperationErrorCode, OperationPlan,
    OperationReceipt, PlatformPreflightEvidence, PluginCapabilityAuditAction,
    PluginCapabilityAuditDto, PluginCapabilityDenyReason, PluginCapabilityGrantDto,
    PluginCommandProjectionDto, PluginCommandSurfaceSnapshotDto, PluginContributionRegistrationDto,
    PluginDiagnosticEvent, PluginDiagnosticKind as WirePluginDiagnosticKind, PluginDisableReason,
    PluginInstallPreviewDto, PluginKeybindingProjectionDto, PluginLogLevel, PluginLogRecordDto,
    PluginMenuProjectionDto, PluginNotificationRecordDto, PluginNotificationSeverity,
    PluginProjectionDto, PluginRegistrySnapshotDto, PluginResourceDto, PluginRuntimeRecoveryDto,
    PluginRuntimeRecoveryState, PluginStorageRecoveryDto, PluginStorageRecoveryStatus,
    PortableBundle, ProfileState, Query, Request, RequestEnvelope, RequestId, Response,
    ResponseEnvelope, PLUGIN_CAPABILITY_SCHEMA_VERSION, PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
    PLUGIN_REGISTRY_SCHEMA_VERSION, PLUGIN_RESOURCE_SCHEMA_VERSION, PROFILE_SCHEMA_VERSION,
    PROTOCOL_VERSION,
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
    durability_report: Option<velvet_workbench_protocol::DurabilityReport>,
    release_updates: Option<ReleaseUpdateManager>,
    diagnostic_store: Option<Arc<dyn DiagnosticEventStore>>,
    operation_receipts: Arc<dyn velvet_persistence::OperationReceiptStore>,
    import_quarantine: Option<Arc<dyn velvet_persistence::ImportQuarantineStore>>,
    storage_availability: Arc<Mutex<StorageAvailability>>,
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
            provenance: context.provenance.clone(),
            deadline_ms: context.deadline_ms(),
            cancellation_grace_ms: context.cancellation_grace_ms(),
            checkpoint: context.checkpoint().cloned(),
            budget: context.budget().clone(),
        };
        let execution = self
            .handle
            .invoke(invocation)
            .map_err(|error| JobExecutionError::Failed(format!("{error:?}")))?;
        for event in execution.events {
            if event.kind == "checkpoint" {
                let payload = event.checkpoint.ok_or_else(|| {
                    JobExecutionError::Failed(
                        "plugin Job checkpoint event is missing payload".into(),
                    )
                })?;
                context.save_checkpoint(payload).map_err(|error| {
                    JobExecutionError::Failed(format!("plugin Job checkpoint failed: {error:?}"))
                })?;
                continue;
            }
            if event.kind != "progress" {
                return Err(JobExecutionError::Failed(format!(
                    "unsupported plugin Job event kind {}",
                    event.kind
                )));
            }
            if context.is_paused() {
                return Err(JobExecutionError::Paused);
            }
            let progress = event.progress.ok_or_else(|| {
                JobExecutionError::Failed("plugin Job progress event is missing progress".into())
            })?;
            if !context.report_progress(progress, event.message.unwrap_or_default()) {
                return Err(JobExecutionError::Cancelled);
            }
        }
        if context.is_paused() {
            return Err(JobExecutionError::Paused);
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
            durability_report,
            diagnostics: diagnostic_store,
            runtime_leases,
            plugin_trust,
            release_updates: release_updates_store,
            release_packages,
            release_trust,
            operation_receipts,
            import_quarantine,
        } = recovery_stores;
        let mut storage_availability = StorageAvailability::Writable;
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
            Err(velvet_persistence::PersistenceError::CorruptOwner {
                owner_id,
                scope,
                generation,
            }) => {
                let _ = recovery.record_failure_for_owner(
                    velvet_workbench_protocol::RecoveryFailureClass::StateCorrupt,
                    "layout snapshot generation is corrupt",
                    owner_id,
                    scope,
                    generation,
                );
            }
            Err(error) => {
                if matches!(error, velvet_persistence::PersistenceError::Unavailable) {
                    storage_availability = StorageAvailability::Unavailable;
                }
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
            Err(velvet_persistence::PersistenceError::CorruptOwner {
                owner_id,
                scope,
                generation,
            }) => {
                let _ = recovery.record_failure_for_owner(
                    velvet_workbench_protocol::RecoveryFailureClass::StateCorrupt,
                    "profile snapshot generation is corrupt",
                    owner_id,
                    scope,
                    generation,
                );
                ProfileState::default()
            }
            Err(error) => {
                if matches!(error, velvet_persistence::PersistenceError::Unavailable) {
                    storage_availability = StorageAvailability::Unavailable;
                }
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
        if let Some(enabled) = profile.plugin_developer_mode {
            if let Err(error) = plugins.set_plugin_developer_mode(enabled) {
                let _ = recovery.record_failure(
                    velvet_workbench_protocol::RecoveryFailureClass::OptionalComponent,
                    format!("Profile plugin trust mode could not be applied: {error:?}"),
                );
            }
        }
        if let Some(snapshots) = recovery_snapshots.clone() {
            plugins.set_extension_storage_store(snapshots);
        }
        if let Some(runtime_leases) = runtime_leases {
            plugins.set_runtime_lease_store(runtime_leases);
            plugins.set_runtime_orphan_reconciler(Arc::new(
                velvet_plugin_host::PlatformRuntimeOrphanReconciler,
            ));
        }
        if let Some(plugin_trust) = plugin_trust {
            if let Err(error) = plugins.set_plugin_trust_store(plugin_trust) {
                let _ = recovery.record_failure(
                    velvet_workbench_protocol::RecoveryFailureClass::OptionalComponent,
                    format!("plugin trust records could not be loaded: {error:?}"),
                );
            }
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
        let release_updates = release_updates_store
            .zip(release_trust)
            .map(|(store, trust)| {
                ReleaseUpdateManager::new(store, release_packages, recovery_sets.clone(), trust)
            });
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
            durability_report,
            release_updates,
            diagnostic_store,
            operation_receipts: operation_receipts.unwrap_or_else(|| {
                Arc::new(velvet_persistence::InMemoryOperationReceiptStore::default())
            }),
            import_quarantine,
            storage_availability: Arc::new(Mutex::new(storage_availability)),
            plugin_diagnostics: VecDeque::new(),
            diagnostic_history: diagnostic_history.into_iter().collect(),
            diagnostics_degraded,
            next_plugin_diagnostic_sequence,
        };

        if storage_availability != StorageAvailability::Writable {
            let mut environment = core.jobs.execution_environment();
            environment.storage = storage_availability;
            let _ = core.jobs.set_execution_environment(environment);
        }

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

    /// Returns the host-owned operation policy seam. Providers remain owned by
    /// their domain and are supplied for each commit; the coordinator only
    /// controls validation, receipt transitions and replay blocking.
    pub fn operation_coordinator(&self) -> OperationCoordinator {
        OperationCoordinator::new(self.operation_receipts.clone())
    }

    pub fn operation_receipts(&self) -> Result<Vec<OperationReceipt>, CoreError> {
        self.operation_coordinator()
            .load_receipts()
            .map_err(operation_error)
    }

    /// Builds the concrete file provider against the same source owner used by
    /// Preview. The provider borrows the resolver so every operation repeats
    /// canonical path, approved-root and volume validation at commit time.
    pub fn file_asset_operation_provider(&self) -> FileAssetOperationProvider<'_> {
        FileAssetOperationProvider::new(self.preview_locations.as_ref())
    }

    /// Commits the first concrete operation slice through the host-owned
    /// coordinator. The provider accepts only an Asset permanent-delete plan;
    /// no path, destination or shell command crosses this method.
    pub fn commit_asset_delete(
        &self,
        plan: &OperationPlan,
        now_ms: u64,
    ) -> Result<OperationReceipt, CoreError> {
        let mut provider = self.file_asset_operation_provider();
        self.operation_coordinator()
            .commit(plan, now_ms, &mut provider)
            .map_err(operation_error)
    }

    /// Produces a fresh Asset delete plan from source-owner observations. The
    /// caller may provide only opaque AssetIds; trusted fingerprints are
    /// filled by the provider and must be sent back unchanged for commit.
    pub fn preview_asset_delete(
        &self,
        plan: &OperationPlan,
        now_ms: u64,
    ) -> Result<OperationPlan, CoreError> {
        plan.validate_for_preview(now_ms)
            .map_err(|error| operation_error(OperationCoordinatorError::Plan(error)))?;
        let mut provider = self.file_asset_operation_provider();
        let observed = provider.observe(plan).map_err(|error| {
            operation_error(OperationCoordinatorError::ProviderFailed {
                phase: OperationPhase::Observe,
                message: format!("{error:?}"),
            })
        })?;
        let mut refreshed = plan.clone();
        refreshed.targets = observed;
        refreshed
            .validate(now_ms)
            .map_err(|error| operation_error(OperationCoordinatorError::Plan(error)))?;
        Ok(refreshed)
    }

    /// Imports a portable bundle through the application-owned snapshot and
    /// recovery-set stores. The Tauri layer can expose this seam as a typed
    /// command without gaining a second persistence owner or direct access to
    /// extension bytes.
    pub fn import_portable_bundle(
        &self,
        bundle: &PortableBundle,
        plan: &ImportPlan,
        participants: &[&dyn BundleImportParticipant],
        now_ms: u64,
    ) -> Result<ImportReceipt, BundleImportError> {
        let Some(snapshots) = self.recovery_snapshots.as_deref() else {
            return Err(BundleImportError::Snapshot(
                velvet_persistence::SnapshotError::Unavailable(
                    "recovery snapshots are unavailable in this composition".into(),
                ),
            ));
        };
        let Some(recovery_sets) = self.recovery_sets.as_deref() else {
            return Err(BundleImportError::RecoverySet(
                velvet_persistence::RecoverySetError::Unavailable(
                    "recovery set index is unavailable in this composition".into(),
                ),
            ));
        };
        let importer = PortableBundleImporter::new(snapshots, recovery_sets);
        let importer = if let Some(quarantine) = self.import_quarantine.as_deref() {
            importer.with_quarantine_store(quarantine)
        } else {
            importer
        };
        importer.import(bundle, plan, participants, now_ms)
    }

    /// Lists retained extension sections for a host recovery surface. The
    /// caller remains inside the host boundary; this method does not decode or
    /// project the inert payload to the renderer.
    pub fn import_quarantine_entries(&self) -> Result<Vec<ImportQuarantineEntry>, CoreError> {
        let Some(store) = self.import_quarantine.as_deref() else {
            return Ok(Vec::new());
        };
        store.load().map_err(|error| {
            CoreError::Recovery(format!("import quarantine unavailable: {error:?}"))
        })
    }

    pub fn import_quarantine_summaries(&self) -> Result<Vec<ImportQuarantineSummary>, CoreError> {
        self.import_quarantine_entries()
            .map(|entries| entries.iter().map(ImportQuarantineEntry::summary).collect())
    }

    /// Discards one retained extension section by its stable provenance key.
    /// A missing key is an idempotent no-op so a repeated recovery action does
    /// not turn a successful deletion into a transport error.
    pub fn discard_import_quarantine(
        &self,
        bundle_id: &str,
        plan_id: &str,
        section_id: &str,
    ) -> Result<bool, CoreError> {
        for (field, value) in [
            ("bundleId", bundle_id),
            ("planId", plan_id),
            ("sectionId", section_id),
        ] {
            if value.is_empty()
                || value.len() > PortableBundle::MAX_ID_BYTES
                || value.contains('\0')
                || value.contains('/')
                || value.contains('\\')
            {
                return Err(CoreError::InvalidCommand(format!(
                    "invalid import quarantine {field}"
                )));
            }
        }
        let Some(store) = self.import_quarantine.as_deref() else {
            return Ok(false);
        };
        store
            .remove(bundle_id, plan_id, section_id)
            .map_err(|error| {
                CoreError::Recovery(format!("import quarantine unavailable: {error:?}"))
            })
    }

    pub fn recovery_projection(&self) -> velvet_workbench_protocol::RecoveryProjection {
        let mut projection = self.recovery.projection();
        projection.durability_report = self.durability_report.clone();
        projection.storage = Some(self.storage_availability());
        projection
    }

    pub fn storage_availability(&self) -> StorageAvailability {
        self.storage_availability
            .lock()
            .map(|availability| *availability)
            .unwrap_or(StorageAvailability::Unknown)
    }

    fn set_storage_availability(&self, availability: StorageAvailability) {
        if let Ok(mut current) = self.storage_availability.lock() {
            *current = availability;
        }
    }

    fn ensure_storage_writable(&self) -> Result<(), CoreError> {
        let availability = self.storage_availability();
        if availability == StorageAvailability::Writable {
            Ok(())
        } else {
            Err(CoreError::Storage { availability })
        }
    }

    fn ensure_storage_writable_for_job(&self) -> Result<(), JobError> {
        match self.storage_availability() {
            StorageAvailability::Writable => Ok(()),
            availability => Err(JobError::Store(format!(
                "Profile storage is {availability:?}"
            ))),
        }
    }

    pub fn recovery_launch_identity(&self) -> Option<(String, u64)> {
        self.recovery.launch_identity()
    }

    pub fn configure_runtime_correlation(
        &mut self,
        instance_id: String,
        attempt_id: u64,
        crash_receipt_path: Option<std::path::PathBuf>,
    ) {
        self.plugins
            .set_runtime_context(instance_id, attempt_id, crash_receipt_path);
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
            let restored = recovery_sets
                .restore_previous(CORE_RECOVERY_SET_ID)
                .map_err(|error| CoreError::Recovery(format!("{error:?}")))?;
            if restored.is_none() {
                return Err(CoreError::Recovery(
                    "no previous committed recovery set is available".into(),
                ));
            }
        }
        snapshot_store
            .quarantine(&target.owner_id, &target.scope, target.generation)
            .map_err(|error| CoreError::Recovery(format!("{error:?}")))
    }

    pub fn release_status(&self) -> Result<velvet_workbench_protocol::ReleaseStatusDto, CoreError> {
        self.release_updates
            .as_ref()
            .ok_or_else(|| CoreError::Release("release update manager is unavailable".into()))?
            .status()
            .map_err(|error| CoreError::Release(error.to_string()))
    }

    pub fn stage_release_package(
        &self,
        transaction_id: &str,
        source: &std::path::Path,
        expected_sha256: &str,
    ) -> Result<String, CoreError> {
        self.release_updates
            .as_ref()
            .ok_or_else(|| CoreError::Release("release update manager is unavailable".into()))?
            .stage_package(transaction_id, source, expected_sha256)
            .map_err(|error| CoreError::Release(error.to_string()))
    }

    pub fn stage_release_package_bytes(
        &self,
        transaction_id: &str,
        bytes: &[u8],
        expected_sha256: &str,
    ) -> Result<String, CoreError> {
        self.release_updates
            .as_ref()
            .ok_or_else(|| CoreError::Release("release update manager is unavailable".into()))?
            .stage_package_bytes(transaction_id, bytes, expected_sha256)
            .map_err(|error| CoreError::Release(error.to_string()))
    }

    pub fn validate_release_manifest(
        &self,
        manifest: &velvet_workbench_protocol::ReleaseManifestDto,
        now_ms: u64,
        expected_target: &str,
    ) -> Result<(), CoreError> {
        self.release_updates
            .as_ref()
            .ok_or_else(|| CoreError::Release("release update manager is unavailable".into()))?
            .validate_manifest(manifest, now_ms, expected_target)
            .map_err(|error| CoreError::Release(error.to_string()))
    }

    pub fn discard_staged_release_package(&self, staged_package_id: &str) -> Result<(), CoreError> {
        self.release_updates
            .as_ref()
            .ok_or_else(|| CoreError::Release("release update manager is unavailable".into()))?
            .discard_package(staged_package_id)
            .map_err(|error| CoreError::Release(error.to_string()))
    }

    pub fn stage_release(
        &self,
        transaction_id: String,
        manifest: velvet_workbench_protocol::ReleaseManifestDto,
        staged_package_id: String,
        package_generation: String,
    ) -> Result<velvet_workbench_protocol::ReleaseStatusDto, CoreError> {
        self.release_updates
            .as_ref()
            .ok_or_else(|| CoreError::Release("release update manager is unavailable".into()))?
            .stage(
                transaction_id,
                manifest,
                staged_package_id,
                package_generation,
            )
            .map_err(|error| CoreError::Release(error.to_string()))
    }

    pub fn commit_release_health(
        &self,
        transaction_id: &str,
        checkpoint: &str,
    ) -> Result<velvet_workbench_protocol::ReleaseStatusDto, CoreError> {
        self.release_updates
            .as_ref()
            .ok_or_else(|| CoreError::Release("release update manager is unavailable".into()))?
            .commit_health(transaction_id, checkpoint)
            .map_err(|error| CoreError::Release(error.to_string()))
    }

    pub fn rollback_release(
        &self,
        transaction_id: &str,
        reason: String,
    ) -> Result<velvet_workbench_protocol::ReleaseStatusDto, CoreError> {
        self.release_updates
            .as_ref()
            .ok_or_else(|| CoreError::Release("release update manager is unavailable".into()))?
            .rollback(transaction_id, reason)
            .map_err(|error| CoreError::Release(error.to_string()))
    }

    /// Roll back the one staged release after a pre-ready launcher or Tauri
    /// initialization failure. The release journal is the authority for the
    /// decision; a committed release is never changed and repeated launches
    /// are therefore idempotent.
    pub fn rollback_pending_release_after_startup_failure(
        &self,
        reason: impl Into<String>,
    ) -> Result<bool, CoreError> {
        let status = self.release_status()?;
        let Some(transaction_id) = status
            .pending
            .as_ref()
            .map(|transaction| transaction.transaction_id.clone())
        else {
            return Ok(false);
        };
        self.rollback_release(&transaction_id, reason.into())?;
        Ok(true)
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

    pub fn record_platform_preflight(&self, evidence: Vec<PlatformPreflightEvidence>) {
        let _ = self.recovery.record_platform_preflight(evidence);
    }

    pub fn record_crash_evidence(&self, evidence: CrashCaptureEvidence) {
        let _ = self.recovery.record_crash_evidence(evidence);
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
        self.register_preview_asset_with_approved_root(
            asset_id,
            source_path,
            content_kind,
            fingerprint,
            None,
        )
    }

    pub fn register_preview_asset_with_approved_root(
        &self,
        asset_id: AssetId,
        source_path: impl AsRef<std::path::Path>,
        content_kind: ContentKindId,
        fingerprint: Option<ContentFingerprint>,
        approved_root: Option<&std::path::Path>,
    ) -> Result<(), velvet_preview_core::PreviewSourceError> {
        self.preview_locations.register_with_approved_root(
            asset_id,
            source_path,
            content_kind,
            fingerprint,
            approved_root,
        )
    }

    pub fn remove_preview_asset(
        &self,
        asset_id: &AssetId,
    ) -> Result<bool, velvet_preview_core::PreviewSourceError> {
        self.preview_locations.remove(asset_id)
    }

    pub fn approved_source_roots(
        &self,
    ) -> Result<Vec<std::path::PathBuf>, velvet_preview_core::PreviewSourceError> {
        self.preview_locations.approved_roots()
    }

    pub fn add_approved_source_root(
        &self,
        root: impl AsRef<std::path::Path>,
    ) -> Result<std::path::PathBuf, velvet_preview_core::PreviewSourceError> {
        self.preview_locations.add_approved_root(root)
    }

    pub fn remove_approved_source_root(
        &self,
        root: impl AsRef<std::path::Path>,
    ) -> Result<bool, velvet_preview_core::PreviewSourceError> {
        self.preview_locations.remove_approved_root(root)
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

    pub fn set_plugin_trust_policy(&mut self, policy: velvet_plugin_host::PluginTrustPolicy) {
        let mut policy = policy;
        if let Some(enabled) = self.profile.plugin_developer_mode {
            let _ = policy.set_developer_mode(enabled);
        }
        self.plugins.set_plugin_trust_policy(policy);
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

    /// Projects the authoritative PluginHost registry without exposing its
    /// internal map or runtime handles to the transport client.
    pub fn query_plugin_registry(
        &self,
        extension_id: Option<&ExtensionId>,
    ) -> PluginRegistrySnapshotDto {
        let snapshot = self.plugins.registry_snapshot();
        let contributions = snapshot
            .contributions
            .into_iter()
            .filter(|registration| extension_id.is_none_or(|id| &registration.extension_id == id))
            .map(|registration| PluginContributionRegistrationDto {
                extension_id: registration.extension_id,
                contribution_id: registration.contribution_id,
                namespace: registration.namespace,
                kind: registration.kind,
                scope: registration.scope,
                zone: registration.zone,
                schema_version: registration.schema_version,
                descriptor: registration.descriptor,
            })
            .collect();
        PluginRegistrySnapshotDto {
            schema_version: PLUGIN_REGISTRY_SCHEMA_VERSION,
            generation: snapshot.generation,
            contributions,
        }
    }

    pub fn query_plugin_command_surface(
        &self,
        extension_id: Option<&ExtensionId>,
    ) -> PluginCommandSurfaceSnapshotDto {
        let snapshot = self.plugins.registry_snapshot();
        let mut surface = PluginCommandSurfaceSnapshotDto {
            schema_version: velvet_workbench_protocol::PLUGIN_COMMAND_SURFACE_SCHEMA_VERSION,
            generation: snapshot.generation,
            context_revision: 0,
            ..PluginCommandSurfaceSnapshotDto::default()
        };
        for registration in snapshot
            .contributions
            .into_iter()
            .filter(|registration| extension_id.is_none_or(|id| id == &registration.extension_id))
        {
            let descriptor = registration.descriptor;
            match registration.kind {
                ContributionKind::Command => surface.commands.push(PluginCommandProjectionDto {
                    extension_id: registration.extension_id,
                    id: registration.contribution_id,
                    namespace: registration.namespace,
                    scope: registration.scope,
                    title: descriptor.title,
                    title_key: descriptor.title_key,
                    when: descriptor.when,
                    enablement: descriptor.enablement,
                    order: descriptor.order,
                    required_capabilities: descriptor.required_capabilities,
                }),
                ContributionKind::Menu | ContributionKind::ContextMenu => {
                    if let (Some(command_id), Some(location)) = (
                        descriptor.command_id,
                        descriptor.menu_location.or_else(|| {
                            (registration.kind == ContributionKind::ContextMenu)
                                .then_some(velvet_plugin_api::MenuLocation::ContextMenu)
                        }),
                    ) {
                        surface.menus.push(PluginMenuProjectionDto {
                            extension_id: registration.extension_id,
                            id: registration.contribution_id,
                            namespace: registration.namespace,
                            scope: registration.scope,
                            location,
                            command_id,
                            label: descriptor.label,
                            label_key: descriptor.label_key,
                            group: descriptor.group,
                            order: descriptor.order,
                            when: descriptor.when,
                            anchor: descriptor.anchor,
                        });
                    }
                }
                ContributionKind::Keybinding => {
                    if let (Some(keybinding), Some(command_id)) =
                        (descriptor.keybinding, descriptor.command_id)
                    {
                        surface.keybindings.push(PluginKeybindingProjectionDto {
                            extension_id: registration.extension_id,
                            id: registration.contribution_id,
                            namespace: registration.namespace,
                            scope: registration.scope,
                            command_id,
                            keybinding,
                            platform: descriptor
                                .keybinding_platform
                                .unwrap_or(velvet_plugin_api::KeybindingPlatform::Any),
                            when: descriptor.when,
                            order: descriptor.order,
                        });
                    }
                }
                _ => {}
            }
        }
        surface
            .commands
            .sort_by(|left, right| left.id.cmp(&right.id));
        surface.menus.sort_by(|left, right| left.id.cmp(&right.id));
        surface
            .keybindings
            .sort_by(|left, right| left.id.cmp(&right.id));
        surface
    }

    pub fn query_plugin_capabilities(
        &self,
        extension_id: &ExtensionId,
    ) -> Result<PluginCapabilityGrantDto, PluginError> {
        self.plugins
            .capability_snapshot(extension_id)
            .map(plugin_capability_dto)
    }

    pub fn grant_plugin_capabilities(
        &mut self,
        extension_id: &ExtensionId,
        capabilities: &[Capability],
        expected_generation: Option<u64>,
    ) -> Result<PluginCapabilityGrantDto, PluginError> {
        self.plugins
            .grant_capabilities(extension_id, capabilities, expected_generation)
            .map(plugin_capability_dto)
    }

    pub fn revoke_plugin_capabilities(
        &mut self,
        extension_id: &ExtensionId,
        capabilities: &[Capability],
        expected_generation: Option<u64>,
    ) -> Result<PluginCapabilityGrantDto, PluginError> {
        self.plugins
            .revoke_capabilities(extension_id, capabilities, expected_generation)
            .map(plugin_capability_dto)
    }

    pub fn query_plugin_capability_audit(&self) -> Vec<PluginCapabilityAuditDto> {
        self.plugins
            .capability_audit()
            .into_iter()
            .map(plugin_capability_audit_dto)
            .collect()
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
        self.ensure_storage_writable_for_job()?;
        self.jobs.submit_or_reuse(JobSpec {
            kind,
            duration_ms,
            will_fail,
            input,
            provenance,
        })
    }

    pub fn create_job_with_policy(
        &self,
        kind: String,
        duration_ms: u64,
        will_fail: bool,
        input: serde_json::Value,
        policy: JobExecutionPolicy,
    ) -> Result<velvet_job_system::JobSnapshot, JobError> {
        self.create_job_with_policy_and_budget(
            kind,
            duration_ms,
            will_fail,
            input,
            policy,
            JobBudgetRequest::default(),
        )
    }

    pub fn create_job_with_policy_and_budget(
        &self,
        kind: String,
        duration_ms: u64,
        will_fail: bool,
        input: serde_json::Value,
        policy: JobExecutionPolicy,
        budget: JobBudgetRequest,
    ) -> Result<velvet_job_system::JobSnapshot, JobError> {
        self.ensure_storage_writable_for_job()?;
        self.jobs.submit_or_reuse_with_policy_and_budget(
            JobSpec {
                kind,
                duration_ms,
                will_fail,
                input,
                provenance: JobProvenance::for_owner("core"),
            },
            policy,
            budget,
        )
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
                self.ensure_storage_writable()?;
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

    pub fn set_job_execution_environment(
        &self,
        environment: JobExecutionEnvironment,
    ) -> Result<Vec<velvet_job_system::JobSnapshot>, JobError> {
        let storage = environment.storage;
        let result = self.jobs.set_execution_environment(environment)?;
        self.set_storage_availability(storage);
        Ok(result)
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
        let runtime_crashes = self.plugins.drain_crash_evidence();
        for crash in runtime_crashes {
            self.record_runtime_crash_evidence(crash);
        }
        let host_records = self.plugins.drain_diagnostics();
        for record in host_records {
            self.enqueue_plugin_diagnostic(plugin_diagnostic_from_record(record));
        }
        self.plugin_diagnostics.drain(..).collect()
    }

    fn record_runtime_crash_evidence(&mut self, crash: RuntimeCrashEvidence) {
        let (fallback_correlation_id, fallback_attempt_id) = self
            .recovery
            .launch_identity()
            .unwrap_or_else(|| ("unknown-launch".into(), 0));
        let _ = self.recovery.record_crash_evidence(CrashCaptureEvidence {
            schema_version: velvet_workbench_protocol::CRASH_CAPTURE_SCHEMA_VERSION,
            correlation_id: crash.host_instance_id.unwrap_or(fallback_correlation_id),
            process_id: crash.process_id.unwrap_or_else(std::process::id),
            parent_process_id: Some(std::process::id()),
            attempt_id: crash.launch_attempt_id.unwrap_or(fallback_attempt_id),
            component: crash.extension_id.0,
            kind: velvet_workbench_protocol::CrashCaptureKind::Runtime,
            code: "runtime-crashed".into(),
            message: crash.message.clone(),
            runtime_nonce: crash.runtime_nonce,
            log_tail: Some(crash.message),
            artifact: None,
            occurred_at_ms: crash.occurred_at_ms,
        });
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
        bundle.projection.durability_report = self.durability_report.clone();
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
            Request::Command(Command::GrantPluginCapabilities {
                extension_id,
                capabilities,
                expected_generation,
            }) => self
                .grant_plugin_capabilities(&extension_id, &capabilities, expected_generation)
                .map(Response::PluginCapabilities)
                .map_err(plugin_error),
            Request::Command(Command::RevokePluginCapabilities {
                extension_id,
                capabilities,
                expected_generation,
            }) => self
                .revoke_plugin_capabilities(&extension_id, &capabilities, expected_generation)
                .map(Response::PluginCapabilities)
                .map_err(plugin_error),
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
            Request::Command(Command::DiscardImportQuarantine {
                bundle_id,
                plan_id,
                section_id,
            }) => self
                .discard_import_quarantine(&bundle_id, &plan_id, &section_id)
                .map(|_| Response::Accepted),
            Request::Command(Command::CommitAssetDelete { plan }) => self
                .commit_asset_delete(&plan, diagnostic_timestamp_ms())
                .map(Response::OperationReceipt),
            Request::Command(Command::PreviewAssetDelete { plan }) => self
                .preview_asset_delete(&plan, diagnostic_timestamp_ms())
                .map(Response::OperationPlan),
            Request::Command(Command::UninstallPlugin { extension_id }) => {
                self.uninstall_plugin(&extension_id).map_err(plugin_error)?;
                Ok(Response::Accepted)
            }
            Request::Command(Command::RegisterPreviewAsset {
                asset_id,
                source_path,
                content_kind,
                approved_root,
            }) => self
                .register_preview_asset_with_approved_root(
                    asset_id.clone(),
                    source_path,
                    ContentKindId(content_kind),
                    None,
                    approved_root.as_deref().map(std::path::Path::new),
                )
                .map(|_| Response::Accepted)
                .map_err(|error| preview_source_error(&asset_id, error)),
            Request::Command(Command::RemovePreviewAsset { asset_id }) => self
                .remove_preview_asset(&asset_id)
                .map(|_| Response::Accepted)
                .map_err(|error| preview_source_error(&asset_id, error)),
            Request::Command(Command::AddApprovedSourceRoot { root }) => self
                .add_approved_source_root(&root)
                .map(|_| Response::Accepted)
                .map_err(approved_source_root_error),
            Request::Command(Command::RemoveApprovedSourceRoot { root }) => self
                .remove_approved_source_root(&root)
                .map(|_| Response::Accepted)
                .map_err(approved_source_root_error),
            Request::Command(Command::InvokePluginCommand {
                extension_id,
                command_id,
                args,
                context,
                context_revision,
                deadline_ms,
                idempotency_key,
            }) => {
                let command_id_for_error = command_id.clone();
                let context =
                    plugin_command_context(context, context_revision, deadline_ms, idempotency_key)
                        .map_err(|error| match error {
                            CoreError::InvalidCommand(_) => CoreError::PluginCommand {
                                extension_id: extension_id.0.clone(),
                                command_id: command_id_for_error.clone(),
                                code: "command-metadata-invalid".into(),
                                retryable: false,
                            },
                            other => other,
                        })?;
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
                        Err(plugin_command_error(
                            &extension_id,
                            &command_id_for_error,
                            error,
                        ))
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
                policy,
                budget,
            }) => {
                self.ensure_storage_writable()?;
                self.create_job_with_policy_and_budget(
                    kind,
                    duration_ms,
                    will_fail,
                    input,
                    policy,
                    budget,
                )
                .map(Response::Job)
                .map_err(job_error)
            }
            Request::Command(Command::CancelJob { job_id }) => self
                .cancel_job(&job_id)
                .map(Response::Job)
                .map_err(job_error),
            Request::Command(Command::RetryJob { job_id }) => self
                .retry_job(&job_id)
                .map(Response::Job)
                .map_err(job_error),
            Request::Command(Command::SetJobExecutionEnvironment { environment }) => self
                .set_job_execution_environment(environment)
                .map(Response::Jobs)
                .map_err(job_error),
            Request::Command(Command::SetLocale {
                locale,
                expected_revision,
            }) => self.set_locale(expected_revision, locale),
            Request::Command(Command::SetPluginDeveloperMode {
                enabled,
                expected_revision,
            }) => self.set_plugin_developer_mode(expected_revision, enabled),
            Request::Command(Command::StageRelease {
                transaction_id,
                manifest,
                staged_package_id,
                package_generation,
            }) => self
                .stage_release(
                    transaction_id,
                    *manifest,
                    staged_package_id,
                    package_generation,
                )
                .map(Response::ReleaseStatus),
            Request::Command(Command::CommitReleaseHealth {
                transaction_id,
                checkpoint,
            }) => self
                .commit_release_health(&transaction_id, &checkpoint)
                .map(Response::ReleaseStatus),
            Request::Command(Command::RollbackRelease {
                transaction_id,
                reason,
            }) => self
                .rollback_release(&transaction_id, reason)
                .map(Response::ReleaseStatus),
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
            Request::Query(Query::ImportQuarantine) => self
                .import_quarantine_summaries()
                .map(Response::ImportQuarantine),
            Request::Query(Query::OperationReceipts) => {
                self.operation_receipts().map(Response::OperationReceipts)
            }
            Request::Query(Query::Jobs) => Ok(Response::Jobs(self.job_snapshots())),
            Request::Query(Query::Snapshot) => Ok(Response::Snapshot(
                velvet_workbench_protocol::AuthoritativeSnapshot {
                    schema_version: velvet_workbench_protocol::CORE_SNAPSHOT_SCHEMA_VERSION,
                    layout: self.layout.snapshot(),
                    profile: self.profile.clone(),
                    recovery: self.recovery_projection(),
                    jobs: self.job_snapshots(),
                    extensions: self.plugins.discover(),
                },
            )),
            Request::Query(Query::RequestOutcome { request_id }) => Ok(Response::RequestOutcome(
                velvet_workbench_protocol::RequestOutcome {
                    request_id,
                    status: velvet_workbench_protocol::RequestOutcomeStatus::Unknown,
                },
            )),
            Request::Query(Query::PluginCommandOutcome { .. }) => Ok(Response::RequestOutcome(
                velvet_workbench_protocol::RequestOutcome {
                    request_id: RequestId(0),
                    status: velvet_workbench_protocol::RequestOutcomeStatus::Unknown,
                },
            )),
            Request::Query(Query::ReleaseStatus) => {
                self.release_status().map(Response::ReleaseStatus)
            }
            Request::Query(Query::PluginProjection { extension_id }) => self
                .plugins
                .projection(&extension_id)
                .map(PluginProjectionDto::from)
                .map(Response::PluginProjection)
                .map_err(plugin_error),
            Request::Query(Query::PluginRegistry { extension_id }) => Ok(Response::PluginRegistry(
                self.query_plugin_registry(extension_id.as_ref()),
            )),
            Request::Query(Query::PluginCapabilities { extension_id }) => self
                .query_plugin_capabilities(&extension_id)
                .map(Response::PluginCapabilities)
                .map_err(plugin_error),
            Request::Query(Query::PluginCapabilityAudit) => Ok(Response::PluginCapabilityAudit(
                self.query_plugin_capability_audit(),
            )),
            Request::Query(Query::PluginLogs { extension_id }) => Ok(Response::PluginLogs(
                self.plugins
                    .logs_snapshot()
                    .into_iter()
                    .filter(|record| {
                        extension_id
                            .as_ref()
                            .is_none_or(|id| id.0 == record.extension_id.0)
                    })
                    .map(plugin_log_dto)
                    .collect(),
            )),
            Request::Query(Query::PluginNotifications { extension_id }) => {
                Ok(Response::PluginNotifications(
                    self.plugins
                        .notifications_snapshot()
                        .into_iter()
                        .filter(|record| {
                            extension_id
                                .as_ref()
                                .is_none_or(|id| id.0 == record.extension_id.0)
                        })
                        .map(plugin_notification_dto)
                        .collect(),
                ))
            }
            Request::Query(Query::PluginCommandSurface { extension_id }) => {
                Ok(Response::PluginCommandSurface(
                    self.query_plugin_command_surface(extension_id.as_ref()),
                ))
            }
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
            Request::Query(Query::ApprovedSourceRoots) => self
                .approved_source_roots()
                .map(|roots| {
                    Response::ApprovedSourceRoots(
                        roots
                            .into_iter()
                            .map(|root| root.to_string_lossy().into_owned())
                            .collect(),
                    )
                })
                .map_err(approved_source_root_error),
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
        self.ensure_storage_writable()?;
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
        self.layout_store.save(&state).map_err(|error| {
            if matches!(error, velvet_persistence::PersistenceError::Unavailable) {
                self.set_storage_availability(StorageAvailability::Unavailable);
                return CoreError::Storage {
                    availability: StorageAvailability::Unavailable,
                };
            }
            CoreError::Layout(format!("{error:?}"))
        })?;
        self.layout = next_layout;
        Ok(Response::Layout(state))
    }

    fn set_locale(
        &mut self,
        expected_revision: Option<u64>,
        locale: String,
    ) -> Result<Response, CoreError> {
        self.ensure_storage_writable()?;
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
            plugin_developer_mode: self.profile.plugin_developer_mode,
        };
        self.profile_store.save(&next).map_err(|error| {
            if matches!(error, velvet_persistence::PersistenceError::Unavailable) {
                self.set_storage_availability(StorageAvailability::Unavailable);
                return CoreError::Storage {
                    availability: StorageAvailability::Unavailable,
                };
            }
            CoreError::Profile(format!("{error:?}"))
        })?;
        self.profile = next.clone();
        Ok(Response::Profile(next))
    }

    fn set_plugin_developer_mode(
        &mut self,
        expected_revision: Option<u64>,
        enabled: bool,
    ) -> Result<Response, CoreError> {
        self.ensure_storage_writable()?;
        if enabled && !self.plugins.developer_mode_allowed() {
            return Err(CoreError::Profile(
                "developer mode is disabled by the active release policy".into(),
            ));
        }
        if let Some(expected) = expected_revision {
            if expected != self.profile.revision {
                return Err(CoreError::StaleRevision {
                    expected,
                    actual: self.profile.revision,
                });
            }
        }
        if self.profile.plugin_developer_mode == Some(enabled) {
            return Ok(Response::Profile(self.profile.clone()));
        }

        let next = ProfileState {
            schema_version: PROFILE_SCHEMA_VERSION,
            revision: self
                .profile
                .revision
                .checked_add(1)
                .ok_or_else(|| CoreError::Profile("profile revision exhausted".into()))?,
            locale: self.profile.locale.clone(),
            plugin_developer_mode: Some(enabled),
        };
        self.profile_store.save(&next).map_err(|error| {
            if matches!(error, velvet_persistence::PersistenceError::Unavailable) {
                self.set_storage_availability(StorageAvailability::Unavailable);
                return CoreError::Storage {
                    availability: StorageAvailability::Unavailable,
                };
            }
            CoreError::Profile(format!("{error:?}"))
        })?;
        self.plugins
            .set_plugin_developer_mode(enabled)
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
        PluginError::CapabilityDenied {
            extension_id,
            capability,
            reason,
            retryable,
        } => CoreError::PluginCapabilityDenied {
            extension_id: extension_id.0,
            capability,
            reason: plugin_capability_deny_reason(reason),
            retryable,
        },
        PluginError::CapabilityGenerationConflict {
            extension_id,
            expected,
            actual,
        } => CoreError::PluginCapabilityGenerationConflict {
            extension_id: extension_id.0,
            expected,
            actual,
        },
        other => CoreError::Plugin(format!("{other:?}")),
    }
}

fn plugin_command_error(
    extension_id: &ExtensionId,
    command_id: &str,
    error: PluginError,
) -> CoreError {
    match &error {
        PluginError::InvalidContribution(message) => CoreError::PluginCommand {
            extension_id: extension_id.0.clone(),
            command_id: command_id.into(),
            code: if message.contains("not declared") {
                "unknown-command".into()
            } else {
                "command-invalid".into()
            },
            retryable: false,
        },
        PluginError::PermissionDenied { .. } | PluginError::CapabilityDenied { .. } => {
            plugin_error(error)
        }
        PluginError::RuntimeInvocation { .. } | PluginError::RuntimeCrashed { .. } => {
            CoreError::PluginCommand {
                extension_id: extension_id.0.clone(),
                command_id: command_id.into(),
                code: "command-runtime-failed".into(),
                retryable: true,
            }
        }
        _ => plugin_error(error),
    }
}

fn plugin_command_context(
    context: serde_json::Value,
    context_revision: u64,
    deadline_ms: Option<u64>,
    idempotency_key: Option<String>,
) -> Result<serde_json::Value, CoreError> {
    if deadline_ms.is_some_and(|deadline| !(1..=120_000).contains(&deadline)) {
        return Err(CoreError::InvalidCommand(
            "plugin command deadline must be between 1 and 120000 ms".into(),
        ));
    }
    if idempotency_key
        .as_deref()
        .is_some_and(|key| key.is_empty() || key.len() > 128 || key.contains('\0'))
    {
        return Err(CoreError::InvalidCommand(
            "plugin command idempotency key must contain 1..=128 bytes".into(),
        ));
    }
    let mut metadata = serde_json::Map::new();
    metadata.insert("contextRevision".into(), context_revision.into());
    if let Some(deadline) = deadline_ms {
        metadata.insert("deadlineMs".into(), deadline.into());
    }
    if let Some(key) = idempotency_key {
        metadata.insert("idempotencyKey".into(), key.into());
    }
    match context {
        serde_json::Value::Object(mut object) => {
            object.insert("_velvet".into(), serde_json::Value::Object(metadata));
            Ok(serde_json::Value::Object(object))
        }
        payload => Ok(serde_json::json!({
            "payload": payload,
            "_velvet": metadata,
        })),
    }
}

fn plugin_capability_deny_reason(reason: CapabilityDenyReason) -> PluginCapabilityDenyReason {
    match reason {
        CapabilityDenyReason::NotRequested => PluginCapabilityDenyReason::NotRequested,
        CapabilityDenyReason::HostPolicy => PluginCapabilityDenyReason::HostPolicy,
        CapabilityDenyReason::UserApprovalRequired => {
            PluginCapabilityDenyReason::UserApprovalRequired
        }
        CapabilityDenyReason::Revoked => PluginCapabilityDenyReason::Revoked,
        CapabilityDenyReason::Inactive => PluginCapabilityDenyReason::Inactive,
        CapabilityDenyReason::Unsupported => PluginCapabilityDenyReason::Unsupported,
    }
}

fn plugin_capability_dto(snapshot: CapabilityGrantSnapshot) -> PluginCapabilityGrantDto {
    PluginCapabilityGrantDto {
        schema_version: PLUGIN_CAPABILITY_SCHEMA_VERSION,
        extension_id: snapshot.extension_id,
        generation: snapshot.generation,
        session_id: snapshot.session_id,
        active: snapshot.active,
        requested: snapshot.requested,
        granted: snapshot.granted,
        denied: snapshot
            .denied
            .into_iter()
            .map(
                |denial| velvet_workbench_protocol::PluginCapabilityDenialDto {
                    capability: denial.capability,
                    reason: plugin_capability_deny_reason(denial.reason),
                },
            )
            .collect(),
    }
}

fn plugin_capability_audit_dto(
    record: velvet_plugin_host::CapabilityAuditRecord,
) -> PluginCapabilityAuditDto {
    PluginCapabilityAuditDto {
        schema_version: velvet_plugin_host::CAPABILITY_AUDIT_SCHEMA_VERSION,
        sequence: record.sequence,
        extension_id: record.extension_id,
        capability: record.capability,
        action: match record.action {
            velvet_plugin_host::CapabilityAuditAction::Granted => {
                PluginCapabilityAuditAction::Granted
            }
            velvet_plugin_host::CapabilityAuditAction::Revoked => {
                PluginCapabilityAuditAction::Revoked
            }
            velvet_plugin_host::CapabilityAuditAction::Denied => {
                PluginCapabilityAuditAction::Denied
            }
            velvet_plugin_host::CapabilityAuditAction::Cleared => {
                PluginCapabilityAuditAction::Cleared
            }
        },
        reason: record.reason.map(plugin_capability_deny_reason),
        generation: record.generation,
        occurred_at_ms: record.occurred_at_ms,
    }
}

fn plugin_log_dto(record: velvet_plugin_host::PluginLogRecord) -> PluginLogRecordDto {
    PluginLogRecordDto {
        schema_version: velvet_workbench_protocol::PLUGIN_LOG_SCHEMA_VERSION,
        sequence: record.sequence,
        extension_id: record.extension_id.0,
        level: match record.level {
            velvet_plugin_host::PluginLogLevel::Trace => PluginLogLevel::Trace,
            velvet_plugin_host::PluginLogLevel::Debug => PluginLogLevel::Debug,
            velvet_plugin_host::PluginLogLevel::Info => PluginLogLevel::Info,
            velvet_plugin_host::PluginLogLevel::Warn => PluginLogLevel::Warn,
            velvet_plugin_host::PluginLogLevel::Error => PluginLogLevel::Error,
        },
        category: record.category,
        message: record.message,
        session_id: record.session_id,
        job_id: record.job_id,
        operation_id: record.operation_id,
        retryable: record.retryable,
        occurred_at_ms: record.occurred_at_ms,
    }
}

fn plugin_notification_dto(
    record: velvet_plugin_host::PluginNotificationRecord,
) -> PluginNotificationRecordDto {
    PluginNotificationRecordDto {
        schema_version: velvet_workbench_protocol::PLUGIN_NOTIFICATION_SCHEMA_VERSION,
        sequence: record.sequence,
        extension_id: record.extension_id.0,
        severity: match record.severity {
            velvet_plugin_host::PluginNotificationSeverity::Info => {
                PluginNotificationSeverity::Info
            }
            velvet_plugin_host::PluginNotificationSeverity::Success => {
                PluginNotificationSeverity::Success
            }
            velvet_plugin_host::PluginNotificationSeverity::Warning => {
                PluginNotificationSeverity::Warning
            }
            velvet_plugin_host::PluginNotificationSeverity::Error => {
                PluginNotificationSeverity::Error
            }
        },
        title: record.title,
        message: record.message,
        session_id: record.session_id,
        job_id: record.job_id,
        operation_id: record.operation_id,
        retryable: record.retryable,
        occurred_at_ms: record.occurred_at_ms,
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

fn operation_error(error: OperationCoordinatorError) -> CoreError {
    let operation = match error {
        OperationCoordinatorError::Plan(plan_error) => OperationError {
            code: match &plan_error {
                velvet_workbench_protocol::OperationPlanError::StaleTarget(_) => {
                    OperationErrorCode::StaleTarget
                }
                _ => OperationErrorCode::InvalidPlan,
            },
            message: format!("{plan_error:?}"),
            operation_id: None,
            retryable: matches!(
                &plan_error,
                velvet_workbench_protocol::OperationPlanError::StaleTarget(_)
                    | velvet_workbench_protocol::OperationPlanError::Expired
            ),
        },
        OperationCoordinatorError::Persistence(persistence_error) => OperationError {
            code: OperationErrorCode::Persistence,
            message: format!("{persistence_error:?}"),
            operation_id: None,
            retryable: true,
        },
        OperationCoordinatorError::AlreadyRecorded {
            operation_id,
            status,
        } => OperationError {
            code: OperationErrorCode::AlreadyRecorded,
            message: format!("operation {operation_id} is already recorded with status {status:?}"),
            operation_id: Some(operation_id),
            retryable: false,
        },
        OperationCoordinatorError::ReconciliationRequired { operation_id } => OperationError {
            code: OperationErrorCode::ReconciliationRequired,
            message: format!("operation {operation_id} requires reconciliation before retry"),
            operation_id: Some(operation_id),
            retryable: false,
        },
        OperationCoordinatorError::ProviderFailed { phase, message } => OperationError {
            code: OperationErrorCode::ProviderFailed,
            message: format!("{phase:?}: {message}"),
            operation_id: None,
            retryable: matches!(phase, OperationPhase::Observe),
        },
        OperationCoordinatorError::ProviderUnknown { phase, message } => OperationError {
            code: OperationErrorCode::ProviderUnknown,
            message: format!("{phase:?}: {message}"),
            operation_id: None,
            retryable: false,
        },
        OperationCoordinatorError::InvalidExecution(message) => OperationError {
            code: OperationErrorCode::ProviderFailed,
            message,
            operation_id: None,
            retryable: false,
        },
    };
    CoreError::Operation(operation)
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

fn approved_source_root_error(error: PreviewSourceError) -> CoreError {
    let retryable = matches!(error, PreviewSourceError::Unavailable);
    let message = match error {
        PreviewSourceError::Unavailable => "approved source root is unavailable".into(),
        PreviewSourceError::NotFound => "approved source root was not found".into(),
        PreviewSourceError::PermissionDenied => "approved source root access was denied".into(),
        PreviewSourceError::InvalidRequest(message) | PreviewSourceError::Failed(message) => {
            message
        }
    };
    CoreError::AssetSourceRoot { message, retryable }
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
    match error {
        JobError::InvalidEnvironment { schema_version } => {
            CoreError::JobEnvironment { schema_version }
        }
        other => CoreError::Job(format!("{other:?}")),
    }
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

    pub fn with_core<R>(
        &self,
        operation: impl FnOnce(&ApplicationCore) -> Result<R, CoreError>,
    ) -> Result<R, CoreError> {
        let core = self
            .core
            .lock()
            .map_err(|_| CoreError::InvalidCommand("core lock poisoned".into()))?;
        operation(&core)
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
#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;
