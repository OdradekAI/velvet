use super::*;
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use velvet_layout_model::{
    LayoutCommand, LayoutState, WorkbenchAppId, WorkbenchPart, WorkbenchZone,
};
use velvet_persistence::{
    ImportQuarantineStore, InMemoryAssetSourceLocationStore, InMemoryDiagnosticEventStore,
    InMemoryImportQuarantineStore, InMemoryLayoutStateStore, InMemoryProfileStateStore,
    InMemoryRecoveryJournalStore, InMemoryRecoverySetStore, InMemoryReleaseUpdateStore,
    InMemoryStateSnapshotStore, JsonFileAssetSourceLocationStore, JsonFileDiagnosticEventStore,
    JsonFileJobStateStore, JsonFileLayoutStateStore, LayoutStateStore, ProfileStateStore,
    ReleaseUpdateStore, SnapshotError, SnapshotJobStateStore, SnapshotLayoutStateStore,
    SnapshotProfileStateStore, StateSnapshot, StateSnapshotStore,
};
use velvet_plugin_api::{
    Capability, ContributionDescriptor, ContributionKind, ContributionScope, ExtensionId,
    ExtensionInstallRecoveryLogStatus, ExtensionManifest, HostContainer, LocalizationManifest,
    Permission, PreviewContributionDescriptor, PreviewIntent as ManifestPreviewIntent,
    PreviewSurface as ManifestPreviewSurface, RuntimeKind, UiSurfaceDescriptor, UiSurfaceKind,
    VersionRange, ZonePermissionSet,
};
use velvet_plugin_host::{
    JsonFilePluginTrustStore, PluginInvocation, PluginInvocationResult, PluginRuntime,
    RuntimeError, RuntimeHealth, RuntimeProcess,
};
use velvet_workbench_protocol::{
    OperationKind, OperationRetryPolicy, OperationTarget, OperationUndoCapability, PortableBundle,
    PortableBundleScope, PortableBundleSection, OPERATION_SCHEMA_VERSION,
    PORTABLE_BUNDLE_SCHEMA_VERSION,
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

struct UnavailableProfileStore;

impl ProfileStateStore for UnavailableProfileStore {
    fn load(&self) -> Result<Option<ProfileState>, velvet_persistence::PersistenceError> {
        Err(velvet_persistence::PersistenceError::Unavailable)
    }

    fn save(&self, _state: &ProfileState) -> Result<(), velvet_persistence::PersistenceError> {
        Err(velvet_persistence::PersistenceError::Unavailable)
    }
}

struct CorruptGenerationStore {
    inner: Arc<InMemoryStateSnapshotStore>,
    owner_id: &'static str,
    scope: &'static str,
    generation: u64,
}

impl StateSnapshotStore for CorruptGenerationStore {
    fn stage(&self, snapshot: &StateSnapshot) -> Result<(), SnapshotError> {
        self.inner.stage(snapshot)
    }

    fn commit(&self, owner_id: &str, scope: &str, generation: u64) -> Result<(), SnapshotError> {
        self.inner.commit(owner_id, scope, generation)
    }

    fn load_latest(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        self.inner.load_latest(owner_id, scope)
    }

    fn load_previous(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        self.inner.load_previous(owner_id, scope)
    }

    fn load_generation(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<Option<StateSnapshot>, SnapshotError> {
        if owner_id == self.owner_id && scope == self.scope && generation == self.generation {
            return Err(SnapshotError::CorruptGeneration {
                generation,
                message: "fixture corruption".into(),
            });
        }
        self.inner.load_generation(owner_id, scope, generation)
    }

    fn list_committed_generations(
        &self,
        owner_id: &str,
        scope: &str,
    ) -> Result<Vec<u64>, SnapshotError> {
        self.inner.list_committed_generations(owner_id, scope)
    }

    fn quarantine(
        &self,
        owner_id: &str,
        scope: &str,
        generation: u64,
    ) -> Result<(), SnapshotError> {
        self.inner.quarantine(owner_id, scope, generation)
    }
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
                    checkpoint: None,
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
    assert!(core
        .job_snapshots()
        .into_iter()
        .any(|job| { job.id == job_id && job.status == velvet_job_system::JobStatus::Running }));

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
fn application_core_commits_asset_delete_through_typed_command() {
    let source_path = temporary_layout_path().with_extension("delete.txt");
    let plugin_root = temporary_layout_path().with_extension("plugins");
    let jobs_path = temporary_layout_path().with_extension("jobs.json");
    let asset_sources_path = temporary_layout_path().with_extension("asset-sources.json");
    fs::write(&source_path, "delete me").expect("the delete source should be writable");
    let mut core = ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store(
            Arc::new(InMemoryLayoutStateStore::default()),
            &plugin_root,
            Arc::new(JsonFileJobStateStore::new(&jobs_path)),
            Arc::new(InMemoryProfileStateStore::default()),
            Arc::new(JsonFileAssetSourceLocationStore::new(&asset_sources_path)),
        );
    let root = source_path.parent().expect("source should have a parent");
    core.add_approved_source_root(root)
        .expect("the source root should be approved");
    core.register_preview_asset_with_approved_root(
        AssetId("asset-delete".into()),
        &source_path,
        ContentKindId("text/plain".into()),
        Some(ContentFingerprint("sha256:asset-delete".into())),
        Some(root),
    )
    .expect("the source location should register");
    let preview_plan = OperationPlan {
        schema_version: OPERATION_SCHEMA_VERSION,
        plan_id: "plan-delete".into(),
        operation_id: "operation-delete".into(),
        owner_id: ASSET_OPERATION_OWNER.into(),
        kind: OperationKind::Delete,
        targets: vec![OperationTarget {
            target_id: "asset-delete".into(),
            expected_revision: None,
            expected_fingerprint: None,
        }],
        created_at_ms: 0,
        expires_at_ms: u64::MAX,
        retry_policy: OperationRetryPolicy::Never,
        undo: OperationUndoCapability::None,
    };
    let plan = match core
        .dispatch(Request::Command(Command::PreviewAssetDelete {
            plan: preview_plan,
        }))
        .expect("asset delete preview should observe the source")
    {
        Response::OperationPlan(plan) => plan,
        other => panic!("expected refreshed operation plan, got {other:?}"),
    };
    assert_eq!(
        plan.targets[0].expected_fingerprint.as_deref(),
        Some("sha256:asset-delete")
    );
    let response = core
        .dispatch(Request::Command(Command::CommitAssetDelete {
            plan: plan.clone(),
        }))
        .expect("asset delete should commit");
    assert!(matches!(
        response,
        Response::OperationReceipt(receipt)
            if receipt.status == velvet_workbench_protocol::OperationReceiptStatus::Committed
    ));
    assert!(!source_path.exists());
    assert!(matches!(
        core.dispatch(Request::Query(Query::OperationReceipts))
            .expect("operation receipts should be queryable"),
        Response::OperationReceipts(receipts)
            if receipts.len() == 1
                && receipts[0].operation_id == "operation-delete"
    ));
    assert!(matches!(
        core.dispatch(Request::Command(Command::CommitAssetDelete { plan })),
        Err(CoreError::Operation(error))
            if error.code == OperationErrorCode::AlreadyRecorded
                && error.operation_id.as_deref() == Some("operation-delete")
    ));
    let _ = fs::remove_dir_all(plugin_root);
    let _ = fs::remove_file(jobs_path);
    let _ = fs::remove_file(asset_sources_path);
}

#[test]
fn persistent_asset_locations_survive_restart_and_stay_path_free_at_the_protocol() {
    let source_path = temporary_layout_path().with_extension("preview.txt");
    let plugin_root = temporary_layout_path().with_extension("plugins");
    let jobs_path = temporary_layout_path().with_extension("jobs.json");
    let asset_sources_path = temporary_layout_path().with_extension("asset-sources.json");
    fs::write(&source_path, "restartable preview").expect("the preview source should be writable");

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
    first
        .add_approved_source_root(source_path.parent().expect("source should have a parent"))
        .expect("the source root should be approved");
    assert_eq!(
        first
            .dispatch(Request::Command(Command::RegisterPreviewAsset {
                asset_id: AssetId("asset-restart".into()),
                source_path: source_path.to_string_lossy().into_owned(),
                content_kind: "text/plain".into(),
                approved_root: Some(
                    source_path
                        .parent()
                        .expect("source should have a parent")
                        .to_string_lossy()
                        .into_owned(),
                ),
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
    let expected_root =
        fs::canonicalize(source_path.parent().expect("source should have a parent"))
            .expect("source root should canonicalize")
            .to_string_lossy()
            .into_owned();
    assert!(matches!(
        restarted
            .dispatch(Request::Query(Query::ApprovedSourceRoots))
            .expect("the approved root query should succeed"),
        Response::ApprovedSourceRoots(roots)
            if roots.iter().any(|root| root == &expected_root)
    ));
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
        .remove_approved_source_root(source_path.parent().expect("source should have a parent"))
        .expect("the source root should be removable");
    assert!(matches!(
        restarted
            .dispatch(Request::Query(Query::ApprovedSourceRoots))
            .expect("the approved root query should succeed after revocation"),
        Response::ApprovedSourceRoots(roots) if roots.is_empty()
    ));
    restarted
        .add_approved_source_root(source_path.parent().expect("source should have a parent"))
        .expect("the source root should be restorable");
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
fn storage_degradation_is_projected_and_blocks_persistent_mutations() {
    let mut core = ApplicationCore::default();
    let environment = velvet_job_system::JobExecutionEnvironment {
        storage: StorageAvailability::ReadOnly,
        ..Default::default()
    };

    assert!(matches!(
        core.dispatch(Request::Command(Command::SetJobExecutionEnvironment {
            environment,
        })),
        Ok(Response::Jobs(_))
    ));
    assert_eq!(core.storage_availability(), StorageAvailability::ReadOnly);
    assert!(matches!(
        core.dispatch(Request::Query(Query::Recovery)),
        Ok(Response::Recovery(projection))
            if projection.storage == Some(StorageAvailability::ReadOnly)
    ));

    assert!(matches!(
        core.dispatch(Request::Command(Command::SetLocale {
            locale: "zh-CN".into(),
            expected_revision: Some(0),
        })),
        Err(CoreError::Storage {
            availability: StorageAvailability::ReadOnly
        })
    ));
    assert!(matches!(
        core.dispatch(Request::Command(Command::Layout(
            LayoutCommand::ActivateApp(WorkbenchAppId::FeedWorkbench),
        ))),
        Err(CoreError::Storage {
            availability: StorageAvailability::ReadOnly
        })
    ));
    assert!(matches!(
        core.dispatch(Request::Command(Command::CreateJob {
            kind: "storage-blocked".into(),
            duration_ms: 1,
            will_fail: false,
            input: serde_json::Value::Null,
            policy: velvet_job_system::JobExecutionPolicy::default(),
            budget: velvet_job_system::JobBudgetRequest::default(),
        })),
        Err(CoreError::Storage {
            availability: StorageAvailability::ReadOnly
        })
    ));

    let writable = velvet_job_system::JobExecutionEnvironment {
        storage: StorageAvailability::Writable,
        ..Default::default()
    };
    core.dispatch(Request::Command(Command::SetJobExecutionEnvironment {
        environment: writable,
    }))
    .expect("storage recovery should be accepted");
    assert_eq!(core.storage_availability(), StorageAvailability::Writable);
}

#[test]
fn unavailable_profile_store_degrades_without_writing_default_state() {
    let mut core = ApplicationCore::with_store_and_profile_store(
        Arc::new(InMemoryLayoutStateStore::default()),
        Arc::new(UnavailableProfileStore),
    );

    assert_eq!(
        core.storage_availability(),
        StorageAvailability::Unavailable
    );
    assert!(matches!(
        core.dispatch(Request::Query(Query::Recovery)),
        Ok(Response::Recovery(projection))
            if projection.storage == Some(StorageAvailability::Unavailable)
    ));
    assert!(matches!(
        core.dispatch(Request::Command(Command::SetLocale {
            locale: "zh-CN".into(),
            expected_revision: Some(0),
        })),
        Err(CoreError::Storage {
            availability: StorageAvailability::Unavailable
        })
    ));
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
            plugin_developer_mode: None,
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
fn profile_developer_mode_is_explicit_revisioned_and_restored() {
    let profile_store = Arc::new(InMemoryProfileStateStore::default());
    profile_store
        .save(&ProfileState {
            plugin_developer_mode: Some(false),
            ..ProfileState::default()
        })
        .expect("the profile trust setting should be writable");
    let plugin_root = temporary_layout_path().with_extension("plugins");
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/plugins/echo-sidecar");
    let mut core = ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store(
        Arc::new(InMemoryLayoutStateStore::default()),
        &plugin_root,
        Arc::new(velvet_job_system::InMemoryJobStateStore::default()),
        profile_store.clone(),
    );

    assert!(matches!(
        core.install_plugin_from_directory(&source),
        Err(PluginError::TrustDenied {
            state: velvet_plugin_host::PluginTrustState::Unknown,
            ..
        })
    ));
    let enabled = core
        .dispatch(Request::Command(Command::SetPluginDeveloperMode {
            enabled: true,
            expected_revision: Some(0),
        }))
        .expect("explicit Profile authorization should succeed");
    assert!(matches!(
        enabled,
        Response::Profile(ProfileState {
            revision: 1,
            plugin_developer_mode: Some(true),
            ..
        })
    ));
    assert!(matches!(
        core.dispatch(Request::Command(Command::SetPluginDeveloperMode {
            enabled: false,
            expected_revision: Some(0),
        })),
        Err(CoreError::StaleRevision {
            expected: 0,
            actual: 1,
        })
    ));
    assert!(matches!(
        core.dispatch(Request::Query(Query::Profile))
            .expect("the Profile should remain authoritative after a stale command"),
        Response::Profile(ProfileState {
            plugin_developer_mode: Some(true),
            revision: 1,
            ..
        })
    ));
    core.install_plugin_from_directory(&source)
        .expect("the explicitly authorized developer package should install");
    assert_eq!(
        profile_store
            .load()
            .expect("the profile should load")
            .expect("the profile should exist")
            .plugin_developer_mode,
        Some(true)
    );
    drop(core);
    let mut restarted =
        ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store(
            Arc::new(InMemoryLayoutStateStore::default()),
            &plugin_root,
            Arc::new(velvet_job_system::InMemoryJobStateStore::default()),
            profile_store,
        );
    assert!(matches!(
        restarted
            .dispatch(Request::Query(Query::Profile))
            .expect("the restarted profile should be readable"),
        Response::Profile(ProfileState {
            plugin_developer_mode: Some(true),
            ..
        })
    ));

    let _ = fs::remove_dir_all(plugin_root);
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
fn startup_failure_rolls_back_a_pending_release_once() {
    let release_store = Arc::new(InMemoryReleaseUpdateStore::new());
    release_store
        .save(&velvet_workbench_protocol::ReleaseStatusDto {
            schema_version: velvet_workbench_protocol::RELEASE_TRANSACTION_SCHEMA_VERSION,
            committed_sequence: 4,
            committed_package_generation: "pkg-4".into(),
            pending: Some(velvet_workbench_protocol::ReleaseTransactionDto {
                schema_version: velvet_workbench_protocol::RELEASE_TRANSACTION_SCHEMA_VERSION,
                transaction_id: "txn-5".into(),
                manifest: velvet_workbench_protocol::ReleaseManifestDto {
                    schema_version: velvet_workbench_protocol::RELEASE_MANIFEST_SCHEMA_VERSION,
                    channel: velvet_workbench_protocol::ReleaseChannel::Preview,
                    version: "0.0.5".into(),
                    target: "test-target".into(),
                    artifact_sha256: "0".repeat(64),
                    updater_schema: 1,
                    min_data_schema: 1,
                    max_data_schema: 1,
                    min_client_version: "0.0.1".into(),
                    release_sequence: 5,
                    issued_at_ms: 1,
                    expires_at_ms: 10,
                    signing_key_id: "root-1".into(),
                    signature: "00".into(),
                },
                staged_package_id: "stage-5".into(),
                package_generation: "pkg-5".into(),
                base_data_generation: None,
                committed_data_generation: None,
                state: velvet_workbench_protocol::ReleaseTransactionState::Staged,
                health_checkpoint: None,
                failure_reason: None,
                created_at_ms: 1,
            }),
            last_transaction: None,
        })
        .expect("pending release fixture should be valid");
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let recovery_stores = ApplicationRecoveryStores::new(
        Arc::new(InMemoryRecoveryJournalStore::default()),
        None,
        None,
    )
    .with_release_updates(
        release_store.clone(),
        ReleaseTrustPolicy::new(
            "root-1",
            signing_key.verifying_key(),
            velvet_workbench_protocol::ReleaseChannel::Preview,
        ),
    );
    let core = ApplicationCore::with_store_and_runtime_and_plugin_directory_and_jobs_and_profile_and_recovery(
            Arc::new(InMemoryLayoutStateStore::default()),
            Box::new(velvet_plugin_host::ProcessPluginRuntime),
            None,
            JobSystem::default(),
            Arc::new(InMemoryProfileStateStore::default()),
            Arc::new(InMemoryAssetSourceLocationStore::default()),
            recovery_stores,
        );

    assert!(core
        .rollback_pending_release_after_startup_failure("launcher failed")
        .expect("pending release should roll back"));
    let status = core.release_status().expect("release status should load");
    assert!(status.pending.is_none());
    assert_eq!(
        status
            .last_transaction
            .as_ref()
            .and_then(|transaction| transaction.failure_reason.as_deref()),
        Some("launcher failed")
    );
    assert!(!core
        .rollback_pending_release_after_startup_failure("repeated launcher failure")
        .expect("a completed rollback should be idempotent"));
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
fn corrupt_plugin_trust_history_degrades_optional_state_without_safe_mode() {
    let trust_path = temporary_layout_path().with_extension("plugin-trust.json");
    fs::write(&trust_path, br#"not-json"#)
        .expect("the corrupt plugin trust fixture should be writable");
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
            .with_plugin_trust(Arc::new(JsonFilePluginTrustStore::new(&trust_path))),
        );

    assert!(!core.in_safe_mode());
    assert!(matches!(
        core.recovery_projection()
            .last_failure
            .map(|failure| failure.class),
        Some(velvet_workbench_protocol::RecoveryFailureClass::OptionalComponent)
    ));
    let _ = fs::remove_file(trust_path);
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
fn plugin_registry_query_projects_only_active_host_registrations() {
    let mut core = ApplicationCore::default();
    let manifest = ExtensionManifest {
        id: ExtensionId("registry.reader".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: None,
        contributions: vec![ContributionDescriptor {
            id: "registry.reader.view".into(),
            kind: ContributionKind::View,
            scope: ContributionScope::Workbench,
            namespace: Some("workbench".into()),
            container: Some(HostContainer::PrimarySidebar),
            zone: Some(WorkbenchZone::Navigation),
            title: Some("Registry reader".into()),
            title_key: None,
            icon: None,
            order: None,
            default_visible: Some(true),
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
    .expect("registry plugin should register");

    let inactive = core
        .dispatch(Request::Query(Query::PluginRegistry {
            extension_id: Some(id.clone()),
        }))
        .expect("registry query should succeed before activation");
    assert!(
        matches!(inactive, Response::PluginRegistry(snapshot) if snapshot.contributions.is_empty())
    );

    core.dispatch(Request::Command(Command::ActivatePlugin {
        extension_id: id.clone(),
    }))
    .expect("registry plugin should activate");
    let active = core
        .dispatch(Request::Query(Query::PluginRegistry {
            extension_id: Some(id.clone()),
        }))
        .expect("registry query should project active registrations");
    assert!(matches!(active, Response::PluginRegistry(snapshot)
        if snapshot.schema_version == velvet_workbench_protocol::PLUGIN_REGISTRY_SCHEMA_VERSION
            && snapshot.contributions.len() == 1
            && snapshot.contributions[0].extension_id == id
            && snapshot.contributions[0].schema_version == velvet_plugin_host::CONTRIBUTION_SCHEMA_VERSION
    ));

    core.dispatch(Request::Command(Command::DisablePlugin {
        extension_id: id.clone(),
        reason: PluginDisableReason::User,
    }))
    .expect("registry plugin should disable");
    let disabled = core
        .dispatch(Request::Query(Query::PluginRegistry {
            extension_id: Some(id),
        }))
        .expect("registry query should remain available after disable");
    assert!(
        matches!(disabled, Response::PluginRegistry(snapshot) if snapshot.contributions.is_empty())
    );
}

#[test]
fn plugin_capability_grants_are_projected_and_required_denials_are_typed() {
    let mut core = ApplicationCore::default();
    let manifest = ExtensionManifest {
        id: ExtensionId("capability.reader".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        required_capabilities: vec!["network".into()],
        ..ExtensionManifest::default()
    };
    let id = manifest.id.clone();
    core.dispatch(Request::Command(Command::RegisterPlugin {
        manifest: Box::new(manifest.into()),
    }))
    .expect("capability plugin should register");

    let denied = core
        .dispatch(Request::Command(Command::ActivatePlugin {
            extension_id: id.clone(),
        }))
        .expect_err("required capability should require approval");
    assert!(matches!(
        denied,
        CoreError::PluginCapabilityDenied {
            extension_id,
            capability: Capability::Network,
            reason: velvet_workbench_protocol::PluginCapabilityDenyReason::UserApprovalRequired,
            retryable: false,
        } if extension_id == id.0
    ));

    let snapshot = match core
        .dispatch(Request::Query(Query::PluginCapabilities {
            extension_id: id.clone(),
        }))
        .expect("capability projection should be available")
    {
        Response::PluginCapabilities(snapshot) => snapshot,
        other => panic!("unexpected capability response: {other:?}"),
    };
    assert!(!snapshot.active);
    assert_eq!(snapshot.requested, vec![Capability::Network]);

    let granted = core
        .dispatch(Request::Command(Command::GrantPluginCapabilities {
            extension_id: id.clone(),
            capabilities: vec![Capability::Network],
            expected_generation: Some(snapshot.generation),
        }))
        .expect("user approval should be projected");
    let Response::PluginCapabilities(granted) = granted else {
        panic!("grant should return a capability projection")
    };
    assert!(!granted.active);

    core.dispatch(Request::Command(Command::ActivatePlugin {
        extension_id: id.clone(),
    }))
    .expect("approved capability should allow activation");
    let active = core
        .dispatch(Request::Query(Query::PluginCapabilities {
            extension_id: id.clone(),
        }))
        .expect("active capability projection should be available");
    let Response::PluginCapabilities(active) = active else {
        panic!("query should return a capability projection")
    };
    assert!(active.active);
    assert_eq!(active.granted, vec![Capability::Network]);

    let revoked = core
        .dispatch(Request::Command(Command::RevokePluginCapabilities {
            extension_id: id.clone(),
            capabilities: vec![Capability::Network],
            expected_generation: Some(active.generation),
        }))
        .expect("revocation should be projected");
    let Response::PluginCapabilities(revoked) = revoked else {
        panic!("revoke should return a capability projection")
    };
    assert!(revoked.granted.is_empty());
    assert!(revoked
        .denied
        .iter()
        .any(|denial| denial.capability == Capability::Network));
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
            policy: velvet_job_system::JobExecutionPolicy::default(),
            budget: velvet_job_system::JobBudgetRequest::default(),
            checkpoint: None,
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
fn portable_import_composition_persists_failed_extension_bytes() {
    struct FailingExtension;

    impl BundleImportParticipant for FailingExtension {
        fn owner_id(&self) -> &str {
            "sample.extension"
        }

        fn snapshot_scope(&self) -> &str {
            "settings"
        }

        fn bundle_scope(&self) -> PortableBundleScope {
            PortableBundleScope::Extension
        }

        fn schema_version(&self) -> u16 {
            1
        }

        fn import_payload(
            &self,
            _section: &PortableBundleSection,
            _existing: Option<&StateSnapshot>,
            _migration: Option<&velvet_workbench_protocol::ImportMigration>,
        ) -> Result<Vec<u8>, BundleImportParticipantError> {
            Err(BundleImportParticipantError::Unavailable(
                "extension is disabled".into(),
            ))
        }
    }

    let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
    let recovery_sets = Arc::new(InMemoryRecoverySetStore::default());
    let quarantine = Arc::new(InMemoryImportQuarantineStore::default());
    let plugin_root = temporary_layout_path().with_extension("plugins");
    let payload = br#"{"settings":{"theme":"dark"}}"#.to_vec();
    let bundle = PortableBundle {
        schema_version: PORTABLE_BUNDLE_SCHEMA_VERSION,
        bundle_id: "bundle-composition-1".into(),
        created_at_ms: 1,
        source_build: "0.0.4".into(),
        sections: vec![PortableBundleSection {
            section_id: "extension-settings".into(),
            scope: PortableBundleScope::Extension,
            owner_id: "sample.extension".into(),
            schema_version: 1,
            required: false,
            extension_id: Some("sample.extension".into()),
            sha256: format!("{:x}", Sha256::digest(&payload)),
            payload,
        }],
    };
    let plan = PortableBundleInspector::inspect(
        &bundle,
        "plan-composition-1",
        &["extension-settings".into()],
        &BundleInspectionContext {
            now_ms: 10,
            plan_ttl_ms: 100,
            supported_owner_schemas: BTreeMap::from([("sample.extension".into(), 1)]),
            trusted_extensions: BTreeSet::from(["sample.extension".into()]),
        },
    )
    .expect("extension plan should be valid");
    let mut core = ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store_and_recovery_stores(
            Arc::new(SnapshotLayoutStateStore::new(
                snapshots.clone(),
                Some(recovery_sets.clone()),
            )),
            &plugin_root,
            Arc::new(SnapshotJobStateStore::new(
                snapshots.clone(),
                Some(recovery_sets.clone()),
            )),
            Arc::new(SnapshotProfileStateStore::new(
                snapshots.clone(),
                Some(recovery_sets.clone()),
            )),
            Arc::new(InMemoryAssetSourceLocationStore::default()),
            ApplicationRecoveryStores::new(
                Arc::new(InMemoryRecoveryJournalStore::default()),
                Some(snapshots),
                Some(recovery_sets),
            )
            .with_import_quarantine(quarantine.clone()),
        );

    let participant = FailingExtension;
    let receipt = core
        .import_portable_bundle(&bundle, &plan, &[&participant], 20)
        .expect("extension failure should be isolated");
    assert_eq!(
        receipt.status,
        velvet_workbench_protocol::ImportReceiptStatus::Partial
    );
    let quarantined = core
        .import_quarantine_entries()
        .expect("quarantine should load");
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].payload, bundle.sections[0].payload);
    match core
        .dispatch(Request::Query(Query::ImportQuarantine))
        .expect("quarantine query should succeed")
    {
        Response::ImportQuarantine(entries) => {
            assert_eq!(entries.len(), 1);
            assert_eq!(
                entries[0].payload_bytes,
                bundle.sections[0].payload.len() as u64
            );
        }
        other => panic!("expected quarantine summaries, got {other:?}"),
    }
    assert_eq!(
        core.dispatch(Request::Command(Command::DiscardImportQuarantine {
            bundle_id: "bundle-composition-1".into(),
            plan_id: "plan-composition-1".into(),
            section_id: "extension-settings".into(),
        }))
        .expect("discard should succeed"),
        Response::Accepted
    );
    assert!(core
        .import_quarantine_entries()
        .expect("quarantine should reload")
        .is_empty());
    assert!(!quarantine
        .remove(
            "bundle-composition-1",
            "plan-composition-1",
            "extension-settings",
        )
        .expect("repeated discard should be a no-op"));
    assert!(matches!(
        core.discard_import_quarantine("bundle/escape", "plan-composition-1", "extension-settings"),
        Err(CoreError::InvalidCommand(_))
    ));
    let _ = fs::remove_dir_all(plugin_root);
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
                )),
                &plugin_root,
                Arc::new(SnapshotJobStateStore::new(
                    snapshots.clone(),
                    Some(recovery_sets.clone()),
                )),
                Arc::new(SnapshotProfileStateStore::new(
                    snapshots.clone(),
                    Some(recovery_sets.clone()),
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

#[test]
fn corrupt_snapshot_generation_is_targeted_and_quarantine_restores_previous_set() {
    let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
    let recovery_sets = Arc::new(InMemoryRecoverySetStore::default());
    let plugin_root = temporary_layout_path().with_extension("plugins");
    let asset_sources = Arc::new(InMemoryAssetSourceLocationStore::default());
    let build_core = |snapshot_store: Arc<dyn StateSnapshotStore>| {
        ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store_and_recovery_stores(
                Arc::new(SnapshotLayoutStateStore::new(
                    snapshot_store.clone(),
                    Some(recovery_sets.clone()),
                )),
                &plugin_root,
                Arc::new(SnapshotJobStateStore::new(
                    snapshot_store.clone(),
                    Some(recovery_sets.clone()),
                )),
                Arc::new(SnapshotProfileStateStore::new(
                    snapshot_store.clone(),
                    Some(recovery_sets.clone()),
                )),
                asset_sources.clone(),
                ApplicationRecoveryStores::new(
                    Arc::new(InMemoryRecoveryJournalStore::default()),
                    Some(snapshot_store),
                    Some(recovery_sets.clone()),
                ),
            )
    };

    let mut first = build_core(snapshots.clone());
    first
        .dispatch(Request::Command(Command::Layout(
            LayoutCommand::SetPartVisibility {
                part: WorkbenchPart::PrimarySidebar,
                visible: false,
            },
        )))
        .expect("first layout should persist");
    first
        .dispatch(Request::Command(Command::SetLocale {
            locale: "zh-Hans".into(),
            expected_revision: Some(0),
        }))
        .expect("first profile should persist");

    let mut second = build_core(snapshots.clone());
    second
        .dispatch(Request::Command(Command::Layout(
            LayoutCommand::ActivateApp(WorkbenchAppId::FeedWorkbench),
        )))
        .expect("second layout should persist");
    second
        .dispatch(Request::Command(Command::SetLocale {
            locale: "en-US".into(),
            expected_revision: Some(1),
        }))
        .expect("second profile should persist");
    let current = recovery_sets
        .load_latest(CORE_RECOVERY_SET_ID)
        .expect("current recovery set should load")
        .expect("current recovery set should exist");
    let target_generation = current
        .members
        .get("layout:workspace")
        .expect("layout should be indexed")
        .generation;
    let previous_set = recovery_sets
        .load_previous(CORE_RECOVERY_SET_ID)
        .expect("previous recovery set should load")
        .expect("previous recovery set should exist");
    let previous_layout_generation = previous_set
        .members
        .get("layout:workspace")
        .expect("previous layout should be indexed")
        .generation;

    let corrupt_store: Arc<dyn StateSnapshotStore> = Arc::new(CorruptGenerationStore {
        inner: snapshots.clone(),
        owner_id: "layout",
        scope: "workspace",
        generation: target_generation,
    });
    let mut third = build_core(corrupt_store);
    let projection = third.recovery_projection();
    let target = projection
        .last_failure
        .as_ref()
        .and_then(|failure| failure.target.as_ref())
        .expect("corrupt generation should be targeted");
    assert_eq!(target.owner_id, "layout");
    assert_eq!(target.scope, "workspace");
    assert_eq!(target.generation, target_generation);
    assert!(projection
        .actions
        .contains(&velvet_workbench_protocol::RecoveryAction::QuarantineState));

    third
        .dispatch(Request::Command(Command::QuarantineRecoveryFailure))
        .expect("quarantine should restore the previous set");
    let restored_set = recovery_sets
        .load_latest(CORE_RECOVERY_SET_ID)
        .unwrap()
        .unwrap();
    assert_eq!(
        restored_set
            .members
            .get("layout:workspace")
            .expect("restored layout should be indexed")
            .generation,
        previous_layout_generation
    );
    assert!(snapshots
        .load_generation("layout", "workspace", target_generation)
        .expect("quarantined generation lookup should work")
        .is_none());
    let restored = build_core(snapshots.clone());
    assert_eq!(restored.profile_state().locale, "zh-Hans");
    assert_eq!(
        restored.layout_snapshot().active_app,
        WorkbenchAppId::AssetManager
    );
    let _ = fs::remove_dir_all(plugin_root);
}

#[test]
fn durability_report_is_present_in_recovery_projection_and_diagnostics() {
    let plugin_root = temporary_layout_path().with_extension("plugins");
    let report = velvet_workbench_protocol::DurabilityReport {
        schema_version: 1,
        mode: velvet_workbench_protocol::DurabilityMode::RequireFsync,
        atomic_replace: true,
        file_sync: true,
        directory_sync: true,
        crash_durable: true,
        detail: "release fixture".into(),
    };
    let core = ApplicationCore::with_store_and_plugin_directory_and_job_store_and_profile_store_and_asset_source_store_and_recovery_stores(
            Arc::new(InMemoryLayoutStateStore::default()),
            &plugin_root,
            Arc::new(velvet_job_system::InMemoryJobStateStore::default()),
            Arc::new(InMemoryProfileStateStore::default()),
            Arc::new(InMemoryAssetSourceLocationStore::default()),
            ApplicationRecoveryStores::new(
                Arc::new(InMemoryRecoveryJournalStore::default()),
                None,
                None,
            )
            .with_durability_report(report.clone()),
        );
    assert_eq!(
        core.recovery_projection().durability_report,
        Some(report.clone())
    );
    assert_eq!(
        core.diagnostic_bundle().projection.durability_report,
        Some(report)
    );
    let _ = fs::remove_dir_all(plugin_root);
}
