use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::*;
use crate::{CONTRIBUTION_REGISTRY_SCHEMA_VERSION, CONTRIBUTION_SCHEMA_VERSION};
use ed25519_dalek::{Signer, SigningKey};
use velvet_layout_model::WorkbenchZone;
use velvet_persistence::{
    InMemoryStateSnapshotStore, JsonFileStateSnapshotStore, StateSnapshotStore,
};
use velvet_plugin_api::{
    Capability, ContextAnchor, ContributionDescriptor, ContributionScope, ExtensionId,
    ExtensionStorageDeclaration, LocalizationManifest, Permission, PreviewAugmentationSlot,
    PreviewContributionDescriptor, PreviewIntent, PreviewSurface, RuntimeKind, RuntimeLaunchSpec,
    UiSurfaceDescriptor, UiSurfaceKind, VersionRange, ZonePermissionSet,
};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

enum StartPlan {
    Fail(String),
    Process(Vec<RuntimeHealth>),
    ProcessWithInvocation(Vec<RuntimeHealth>, RuntimeError),
}

struct FakeRuntime {
    plans: VecDeque<StartPlan>,
    grants: Arc<Mutex<Vec<PermissionGrant>>>,
    shutdowns: Arc<Mutex<usize>>,
}

struct FakeProcess {
    health: VecDeque<RuntimeHealth>,
    shutdowns: Arc<Mutex<usize>>,
    invocation_error: Option<RuntimeError>,
}

struct JobBoundaryRuntime {
    observed_deadline: Arc<Mutex<Option<u64>>>,
    cancel_job: bool,
    result_schema_version: u16,
}

struct JobBoundaryProcess {
    observed_deadline: Arc<Mutex<Option<u64>>>,
    cancel_job: bool,
    result_schema_version: u16,
}

struct LeaseRuntime {
    shutdowns: Arc<Mutex<usize>>,
}

struct LeaseProcess {
    shutdowns: Arc<Mutex<usize>>,
}

struct StorageRuntime {
    observed: Arc<Mutex<Vec<PluginInvocation>>>,
    shutdowns: Arc<Mutex<usize>>,
}

struct StorageProcess {
    observed: Arc<Mutex<Vec<PluginInvocation>>>,
    shutdowns: Arc<Mutex<usize>>,
}

struct ConfirmProvider {
    accepted: bool,
}

impl PluginConfirmationProvider for ConfirmProvider {
    fn confirm(
        &self,
        _extension_id: &ExtensionId,
        _request: &PluginConfirmationRequest,
    ) -> Result<bool, String> {
        Ok(self.accepted)
    }
}

fn temporary_manifest_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after the Unix epoch")
        .as_nanos();
    let sequence = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "velvet-plugin-manifests-{}-{nonce}-{sequence}",
        std::process::id(),
    ))
}

fn write_package(root: &PathBuf, manifest: &ExtensionManifest) {
    fs::create_dir_all(root).expect("the package directory should be writable");
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(manifest).expect("the manifest should serialize"),
    )
    .expect("the manifest should be written");
}

fn write_signed_package(
    root: &PathBuf,
    manifest: &ExtensionManifest,
    publisher_key_id: &str,
    signing_key: &SigningKey,
    generation: u64,
) {
    write_package(root, manifest);
    let public_key = encode_hex(signing_key.verifying_key().as_bytes());
    let mut envelope = PluginSignatureEnvelope {
        schema_version: PLUGIN_SIGNATURE_SCHEMA_VERSION,
        publisher_key_id: publisher_key_id.into(),
        publisher_key: public_key,
        signature: String::new(),
        package_sha256: package_sha256(root).expect("package digest should be computable"),
        extension_id: manifest.id.clone(),
        version: manifest.version.clone(),
        runtime: manifest.runtime.clone(),
        manifest_schema: manifest.manifest_schema,
        host_api: manifest.host_api.clone(),
        generation,
    };
    envelope.signature = encode_hex(
        &signing_key
            .sign(&canonical_plugin_signature_payload(&envelope))
            .to_bytes(),
    );
    fs::write(
        root.join(PLUGIN_SIGNATURE_FILE),
        serde_json::to_vec_pretty(&envelope).expect("the package signature should serialize"),
    )
    .expect("the package signature should be written");
}

fn signed_key_rotation(
    old_key_id: &str,
    old_key: &SigningKey,
    new_key_id: &str,
    new_key: &SigningKey,
    sequence: u64,
) -> PluginPublisherKeyRotation {
    let mut rotation = PluginPublisherKeyRotation {
        schema_version: PLUGIN_KEY_ROTATION_SCHEMA_VERSION,
        old_key_id: old_key_id.into(),
        old_public_key: encode_hex(old_key.verifying_key().as_bytes()),
        new_key_id: new_key_id.into(),
        new_public_key: encode_hex(new_key.verifying_key().as_bytes()),
        sequence,
        old_signature: String::new(),
        new_signature: String::new(),
    };
    let payload = rotation.canonical_payload();
    rotation.old_signature = encode_hex(&old_key.sign(&payload).to_bytes());
    rotation.new_signature = encode_hex(&new_key.sign(&payload).to_bytes());
    rotation
}

fn write_zip_package(path: &PathBuf, manifest: &ExtensionManifest, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("the archive should be writable");
    let mut archive = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    archive
        .start_file("manifest.json", options)
        .expect("the manifest entry should start");
    archive
        .write_all(&serde_json::to_vec(manifest).expect("the manifest should serialize"))
        .expect("the manifest entry should be written");
    for (name, bytes) in entries {
        archive
            .start_file(*name, options)
            .expect("the archive entry should start");
        archive
            .write_all(bytes)
            .expect("the archive entry should be written");
    }
    archive.finish().expect("the archive should finish");
}

#[test]
fn resource_loader_reads_only_declared_bounded_localization_bundles() {
    let install_root = temporary_manifest_dir();
    let package_root = install_root.join("example");
    let mut manifest = view_manifest(None);
    manifest.localization = Some(LocalizationManifest {
        default_locale: "en".into(),
        supported_locales: vec!["en".into(), "zh-Hans".into()],
        resource_root: "l10n".into(),
        bundle_schema: 1,
    });
    write_package(&package_root, &manifest);
    fs::create_dir_all(package_root.join("l10n")).expect("resource directory should exist");
    fs::write(package_root.join("l10n/en.json"), br#"{"ready":"Ready"}"#)
        .expect("resource bundle should be written");

    let loader = PluginResourceLoader::new(&install_root);
    let resource = loader
        .read_localization_bundle(&ExtensionId("example".into()), &manifest, "en")
        .expect("declared bundle should load");
    assert_eq!(resource.content_type, "application/json");
    assert_eq!(resource.bytes, br#"{"ready":"Ready"}"#);
    assert!(matches!(
        loader.read_localization_bundle(&manifest.id, &manifest, "fr"),
        Err(PluginError::ResourceNotFound { .. })
    ));

    let _ = fs::remove_dir_all(install_root);
}

#[test]
fn resource_loader_rejects_package_escape_and_oversized_bundle() {
    let install_root = temporary_manifest_dir();
    let package_root = install_root.join("example");
    let mut manifest = view_manifest(None);
    manifest.localization = Some(LocalizationManifest {
        default_locale: "en".into(),
        supported_locales: vec!["en".into()],
        resource_root: "../outside".into(),
        bundle_schema: 1,
    });
    write_package(&package_root, &manifest);
    let loader = PluginResourceLoader::new(&install_root);
    assert!(matches!(
        loader.read_localization_bundle(&manifest.id, &manifest, "en"),
        Err(PluginError::ResourceDenied { .. })
    ));

    manifest.localization.as_mut().unwrap().resource_root = "l10n".into();
    fs::create_dir_all(package_root.join("l10n")).expect("resource directory should exist");
    fs::write(
        package_root.join("l10n/en.json"),
        vec![b'x'; MAX_PLUGIN_LOCALIZATION_BYTES + 1],
    )
    .expect("large resource bundle should be written");
    assert!(matches!(
        loader.read_localization_bundle(&manifest.id, &manifest, "en"),
        Err(PluginError::ResourceTooLarge { .. })
    ));

    let _ = fs::remove_dir_all(install_root);
}

#[test]
fn zip_archive_adapter_extracts_a_directory_package_and_cleans_rejected_archives() {
    let root = temporary_manifest_dir();
    fs::create_dir_all(&root).expect("the archive root should be writable");
    let archive_path = root.join("example.zip");
    let manifest = view_manifest(Some(WorkbenchZone::Navigation));
    write_zip_package(
        &archive_path,
        &manifest,
        &[("l10n/en.json", br#"{"ready":"Ready"}"#)],
    );

    let extraction = root.join("extracted");
    ZipPluginArchiveAdapter
        .extract_to(&archive_path, &extraction)
        .expect("a valid archive should extract");
    assert_eq!(
        fs::read(extraction.join("manifest.json")).expect("manifest should be extracted"),
        serde_json::to_vec(&manifest).expect("manifest should serialize")
    );
    assert_eq!(
        fs::read(extraction.join("l10n/en.json")).expect("bundle should be extracted"),
        br#"{"ready":"Ready"}"#
    );

    let rejected_archive = root.join("rejected.zip");
    let file = fs::File::create(&rejected_archive).expect("the archive should be writable");
    let mut archive = zip::ZipWriter::new(file);
    archive
        .start_file("../escape.txt", zip::write::SimpleFileOptions::default())
        .expect("the unsafe entry should be written");
    archive
        .write_all(b"escape")
        .expect("the unsafe entry should contain data");
    archive.finish().expect("the archive should finish");
    let rejected_extraction = root.join("rejected");
    assert!(matches!(
        ZipPluginArchiveAdapter.extract_to(&rejected_archive, &rejected_extraction),
        Err(PluginArchiveError::EntryRejected(_))
    ));
    assert!(!rejected_extraction.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn plugin_host_installs_and_previews_zip_packages_through_the_existing_transaction() {
    let source_root = temporary_manifest_dir();
    let install_root = temporary_manifest_dir();
    fs::create_dir_all(&source_root).expect("source root should be writable");
    let archive_path = source_root.join("example.zip");
    let manifest = ExtensionManifest {
        id: ExtensionId("example".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        contributions: Vec::new(),
        permission_sets: Vec::new(),
        ..ExtensionManifest::default()
    };
    write_zip_package(&archive_path, &manifest, &[]);

    let host = InMemoryPluginHost::default();
    let preview = host
        .inspect_from_archive(&archive_path)
        .expect("archive preview should validate the manifest");
    assert_eq!(preview.manifest.id, manifest.id);

    let mut host = host;
    let installed = host
        .install_from_archive(&archive_path, &install_root)
        .expect("archive install should use the directory transaction");
    assert_eq!(installed.id, manifest.id);
    assert!(install_root.join("example/manifest.json").is_file());

    let _ = fs::remove_dir_all(source_root);
    let _ = fs::remove_dir_all(install_root);
}

impl RuntimeProcess for FakeProcess {
    fn poll(&mut self) -> RuntimeHealth {
        self.health.pop_front().unwrap_or(RuntimeHealth::Running)
    }

    fn shutdown(&mut self) -> Result<(), RuntimeError> {
        *self
            .shutdowns
            .lock()
            .expect("shutdown counter should not be poisoned") += 1;
        Ok(())
    }

    fn invoke(
        &mut self,
        invocation: &PluginInvocation,
    ) -> Result<PluginInvocationResult, RuntimeError> {
        if let Some(error) = self.invocation_error.clone() {
            return Err(error);
        }
        if invocation.method == "job.run" {
            return Ok(PluginInvocationResult {
                result: Some(serde_json::json!({
                    "contentType": "application/json",
                    "payload": { "jobKind": invocation.command_id.clone() },
                })),
                error: None,
                events: vec![PluginInvocationEvent {
                    kind: "progress".into(),
                    progress: Some(50),
                    message: Some("Halfway".into()),
                    checkpoint: None,
                }],
            });
        }
        Ok(PluginInvocationResult {
            result: Some(serde_json::json!({ "accepted": true })),
            error: None,
            events: Vec::new(),
        })
    }
}

impl RuntimeProcess for JobBoundaryProcess {
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
        *self
            .observed_deadline
            .lock()
            .expect("deadline capture should not be poisoned") = invocation.deadline_ms;
        Ok(PluginInvocationResult {
            result: Some(serde_json::json!({
                "contentType": "application/json",
                "schemaVersion": self.result_schema_version,
                "payload": { "accepted": true },
            })),
            error: None,
            events: if self.cancel_job {
                vec![PluginInvocationEvent {
                    kind: "cancelled".into(),
                    progress: None,
                    message: Some("plugin stopped after checking its input".into()),
                    checkpoint: None,
                }]
            } else {
                vec![PluginInvocationEvent {
                    kind: "progress".into(),
                    progress: Some(100),
                    message: Some("Completed".into()),
                    checkpoint: None,
                }]
            },
        })
    }
}

impl PluginRuntime for JobBoundaryRuntime {
    fn start(
        &mut self,
        _manifest: &ExtensionManifest,
        _permissions: &PermissionGrant,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
        Ok(Box::new(JobBoundaryProcess {
            observed_deadline: self.observed_deadline.clone(),
            cancel_job: self.cancel_job,
            result_schema_version: self.result_schema_version,
        }))
    }
}

impl RuntimeProcess for LeaseProcess {
    fn poll(&mut self) -> RuntimeHealth {
        RuntimeHealth::Running
    }

    fn shutdown(&mut self) -> Result<(), RuntimeError> {
        *self
            .shutdowns
            .lock()
            .expect("shutdown counter should not be poisoned") += 1;
        Ok(())
    }

    fn identity(&self) -> Option<RuntimeProcessIdentity> {
        Some(RuntimeProcessIdentity {
            pid: 41_001,
            process_group_id: Some(41_001),
            start_token: Some("test-token".into()),
        })
    }

    fn invoke(
        &mut self,
        _invocation: &PluginInvocation,
    ) -> Result<PluginInvocationResult, RuntimeError> {
        Ok(PluginInvocationResult {
            result: Some(serde_json::json!({ "accepted": true })),
            error: None,
            events: Vec::new(),
        })
    }
}

impl RuntimeProcess for StorageProcess {
    fn poll(&mut self) -> RuntimeHealth {
        RuntimeHealth::Running
    }

    fn shutdown(&mut self) -> Result<(), RuntimeError> {
        *self
            .shutdowns
            .lock()
            .expect("shutdown counter should not be poisoned") += 1;
        Ok(())
    }

    fn invoke(
        &mut self,
        invocation: &PluginInvocation,
    ) -> Result<PluginInvocationResult, RuntimeError> {
        self.observed
            .lock()
            .expect("invocation log should not be poisoned")
            .push(invocation.clone());
        let request = serde_json::from_value::<StorageRuntimeRequest>(invocation.args.clone())
            .map_err(|error| RuntimeError::InvocationFailed(error.to_string()))?;
        let target_schema_version = request.target_schema_version;
        let entries = if invocation.method == STORAGE_MIGRATE_METHOD {
            request
                .entries
                .into_iter()
                .map(|(key, value)| (format!("{key}.v2"), value))
                .collect()
        } else {
            request.entries
        };
        Ok(PluginInvocationResult {
            result: Some(
                serde_json::to_value(StorageRuntimeResult {
                    schema_version: target_schema_version,
                    entries,
                })
                .expect("storage result should serialize"),
            ),
            error: None,
            events: Vec::new(),
        })
    }
}

impl PluginRuntime for StorageRuntime {
    fn start(
        &mut self,
        _manifest: &ExtensionManifest,
        _permissions: &PermissionGrant,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
        Ok(Box::new(StorageProcess {
            observed: self.observed.clone(),
            shutdowns: self.shutdowns.clone(),
        }))
    }
}

impl PluginRuntime for LeaseRuntime {
    fn start(
        &mut self,
        _manifest: &ExtensionManifest,
        _permissions: &PermissionGrant,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
        Ok(Box::new(LeaseProcess {
            shutdowns: self.shutdowns.clone(),
        }))
    }

    fn requires_package_launch_context(&self) -> bool {
        true
    }
}

impl PluginRuntime for FakeRuntime {
    fn start(
        &mut self,
        _manifest: &ExtensionManifest,
        permissions: &PermissionGrant,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
        self.grants
            .lock()
            .expect("grant log should not be poisoned")
            .push(permissions.clone());
        match self
            .plans
            .pop_front()
            .expect("a runtime plan should be configured")
        {
            StartPlan::Fail(message) => Err(RuntimeError::StartFailed(message)),
            StartPlan::Process(health) => Ok(Box::new(FakeProcess {
                health: health.into(),
                shutdowns: self.shutdowns.clone(),
                invocation_error: None,
            })),
            StartPlan::ProcessWithInvocation(health, error) => Ok(Box::new(FakeProcess {
                health: health.into(),
                shutdowns: self.shutdowns.clone(),
                invocation_error: Some(error),
            })),
        }
    }
}

fn view_manifest(zone: Option<WorkbenchZone>) -> ExtensionManifest {
    ExtensionManifest {
        id: ExtensionId("example".into()),
        version: "0.1.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: None,
        contributions: vec![ContributionDescriptor {
            id: "example.view".into(),
            kind: ContributionKind::View,
            scope: ContributionScope::Workbench,
            namespace: Some("workbench".into()),
            container: Some(HostContainer::PrimarySidebar),
            zone,
            title: Some("Example view".into()),
            title_key: None,
            icon: Some("document".into()),
            order: None,
            default_visible: Some(true),
            view_container_id: None,
            activity_bar: None,
            label: None,
            label_key: None,
            command_id: None,
            group: None,
            when: None,
            enablement: None,
            keybinding: None,
            keybinding_platform: None,
            menu_location: None,
            required_capabilities: Vec::new(),
            anchor: None,
            ui: None,
            preview: None,
        }],
        permission_sets: vec![],
        ..ExtensionManifest::default()
    }
}

fn preview_manifest(with_permission: bool) -> ExtensionManifest {
    ExtensionManifest {
        id: ExtensionId("preview.example".into()),
        version: "0.1.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: None,
        contributions: vec![ContributionDescriptor {
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
            enablement: None,
            keybinding: None,
            keybinding_platform: None,
            menu_location: None,
            required_capabilities: Vec::new(),
            anchor: None,
            ui: None,
            preview: Some(PreviewContributionDescriptor {
                content_kinds: vec!["image".into()],
                intents: vec![PreviewIntent::Quick],
                surfaces: vec![PreviewSurface::Workspace],
                priority: 20,
                slot: None,
            }),
        }],
        permission_sets: if with_permission {
            vec![ZonePermissionSet {
                zone: WorkbenchZone::Contextual,
                permissions: vec![Permission::PreviewRender],
            }]
        } else {
            vec![]
        },
        ..ExtensionManifest::default()
    }
}

fn job_manifest(with_permission: bool) -> ExtensionManifest {
    ExtensionManifest {
        id: ExtensionId("job.example".into()),
        version: "0.1.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: None,
        contributions: vec![ContributionDescriptor {
            id: "job.example.analyze".into(),
            kind: ContributionKind::Job,
            scope: ContributionScope::Capability,
            namespace: Some("capability.job.example".into()),
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
            enablement: None,
            keybinding: None,
            keybinding_platform: None,
            menu_location: None,
            required_capabilities: Vec::new(),
            anchor: None,
            ui: None,
            preview: None,
        }],
        permission_sets: if with_permission {
            vec![ZonePermissionSet {
                zone: WorkbenchZone::Operations,
                permissions: vec![Permission::JobsCreate],
            }]
        } else {
            vec![]
        },
        ..ExtensionManifest::default()
    }
}

#[test]
fn host_capability_services_require_active_grants_and_preserve_plugin_source() {
    let grants = Arc::new(Mutex::new(Vec::new()));
    let shutdowns = Arc::new(Mutex::new(0));
    let mut host = InMemoryPluginHost::with_runtime(Box::new(FakeRuntime {
        plans: vec![StartPlan::Process(vec![RuntimeHealth::Running])].into(),
        grants,
        shutdowns,
    }));
    let manifest = ExtensionManifest {
        id: ExtensionId("services.example".into()),
        optional_capabilities: vec![
            Capability::LogsWrite.name().into(),
            Capability::NotificationsShow.name().into(),
            Capability::DialogsShow.name().into(),
        ],
        ..ExtensionManifest::default()
    };
    let id = manifest.id.clone();
    host.register(manifest)
        .expect("service manifest should register");
    let generation = host
        .capability_snapshot(&id)
        .expect("capability registration should exist")
        .generation;
    host.grant_capabilities(
        &id,
        &[
            Capability::LogsWrite,
            Capability::NotificationsShow,
            Capability::DialogsShow,
        ],
        Some(generation),
    )
    .expect("service capabilities should be granted before activation");
    host.activate(&id)
        .expect("service extension should activate");

    host.emit_log(
        &id,
        PluginLogInput {
            extension_id: id.clone(),
            level: PluginLogLevel::Info,
            category: "service".into(),
            message: "hello".into(),
            session_id: Some("session-1".into()),
            job_id: None,
            operation_id: None,
            retryable: false,
        },
    )
    .expect("active extension should be able to log");
    host.emit_notification(
        &id,
        PluginNotificationInput {
            extension_id: id.clone(),
            severity: PluginNotificationSeverity::Info,
            title: "Ready".into(),
            message: "The host is ready".into(),
            session_id: None,
            job_id: None,
            operation_id: None,
            retryable: false,
        },
    )
    .expect("active extension should be able to notify");
    host.set_confirmation_provider(Arc::new(ConfirmProvider { accepted: true }));
    assert!(host
        .request_confirmation(
            &id,
            &PluginConfirmationRequest {
                title: "Confirm".into(),
                message: "Continue?".into(),
                confirm_label: "Continue".into(),
                cancel_label: "Cancel".into(),
                context_revision: 3,
            },
        )
        .expect("confirmation provider should be called"));
    assert_eq!(host.logs_snapshot().len(), 1);
    assert_eq!(host.notifications_snapshot().len(), 1);

    host.disable(&id, DisableReason::User)
        .expect("service extension should disable");
    assert!(matches!(
        host.emit_log(
            &id,
            PluginLogInput {
                extension_id: id.clone(),
                level: PluginLogLevel::Info,
                category: "service".into(),
                message: "after disable".into(),
                session_id: None,
                job_id: None,
                operation_id: None,
                retryable: false,
            },
        ),
        Err(PluginError::InvalidState { .. })
    ));
}

#[test]
fn notification_rate_limit_is_a_typed_failure() {
    let grants = Arc::new(Mutex::new(Vec::new()));
    let shutdowns = Arc::new(Mutex::new(0));
    let mut host = InMemoryPluginHost::with_runtime(Box::new(FakeRuntime {
        plans: vec![StartPlan::Process(vec![RuntimeHealth::Running])].into(),
        grants,
        shutdowns,
    }));
    let manifest = ExtensionManifest {
        id: ExtensionId("notify.example".into()),
        optional_capabilities: vec![Capability::NotificationsShow.name().into()],
        ..ExtensionManifest::default()
    };
    let id = manifest.id.clone();
    host.register(manifest)
        .expect("notification manifest should register");
    let generation = host
        .capability_snapshot(&id)
        .expect("capability registration should exist")
        .generation;
    host.grant_capabilities(&id, &[Capability::NotificationsShow], Some(generation))
        .expect("notification capability should be granted before activation");
    host.activate(&id)
        .expect("notification extension should activate");
    for index in 0..MAX_PLUGIN_NOTIFICATIONS_PER_MINUTE {
        host.emit_notification(
            &id,
            PluginNotificationInput {
                extension_id: id.clone(),
                severity: PluginNotificationSeverity::Info,
                title: format!("Notification {index}"),
                message: "bounded".into(),
                session_id: None,
                job_id: None,
                operation_id: None,
                retryable: false,
            },
        )
        .expect("notifications below the rate limit should succeed");
    }
    assert!(matches!(
        host.emit_notification(
            &id,
            PluginNotificationInput {
                extension_id: id.clone(),
                severity: PluginNotificationSeverity::Info,
                title: "overflow".into(),
                message: "bounded".into(),
                session_id: None,
                job_id: None,
                operation_id: None,
                retryable: false,
            },
        ),
        Err(PluginError::HostCapabilityRateLimited { .. })
    ));
}

#[test]
fn job_handles_use_a_thread_safe_runtime_boundary_and_project_progress() {
    let mut host = InMemoryPluginHost::default();
    let manifest = job_manifest(true);
    let id = manifest.id.clone();
    host.register(manifest)
        .expect("Job manifest should register");
    host.activate(&id).expect("Job extension should activate");
    let handle = host
        .job_handles(&id)
        .expect("active Job contribution should expose a handle")
        .pop()
        .expect("Job contribution should have a handle");
    let execution = handle
        .invoke(PluginJobInvocation {
            job_id: "job-000001".into(),
            job_kind: "job.example.analyze".into(),
            attempt: 1,
            input: serde_json::json!({ "assetId": "asset-1" }),
            provenance: JobProvenance {
                input_fingerprint: Some("sha256:asset-1".into()),
                ..JobProvenance::for_owner("job.example")
            },
            budget: JobBudgetRequest::default(),
            deadline_ms: None,
            cancellation_grace_ms: default_job_cancellation_grace_ms(),
            checkpoint: None,
        })
        .expect("Job invocation should reach the runtime");
    assert_eq!(execution.result.content_type, "application/json");
    assert_eq!(execution.result.payload["jobKind"], "job.example.analyze");
    assert_eq!(execution.events[0].progress, Some(100));
    assert!(host.drain_diagnostics().is_empty());
}

#[test]
fn plugin_job_deadline_and_self_cancellation_reach_the_runtime_boundary() {
    let observed_deadline = Arc::new(Mutex::new(None));
    let mut host = InMemoryPluginHost::with_runtime(Box::new(JobBoundaryRuntime {
        observed_deadline: observed_deadline.clone(),
        cancel_job: true,
        result_schema_version: JOB_SCHEMA_VERSION,
    }));
    let manifest = job_manifest(true);
    let id = manifest.id.clone();
    host.register(manifest)
        .expect("Job manifest should register");
    host.activate(&id).expect("Job extension should activate");
    let handle = host
        .job_handles(&id)
        .expect("active Job contribution should expose a handle")
        .pop()
        .expect("Job contribution should have a handle");

    let execution = handle
        .invoke(PluginJobInvocation {
            job_id: "job-cancelled-by-plugin".into(),
            job_kind: "job.example.analyze".into(),
            attempt: 1,
            input: serde_json::json!({ "assetId": "asset-1" }),
            provenance: JobProvenance::for_owner("job.example"),
            budget: JobBudgetRequest::default(),
            deadline_ms: Some(37),
            cancellation_grace_ms: default_job_cancellation_grace_ms(),
            checkpoint: None,
        })
        .expect("Job invocation should reach the runtime");

    assert_eq!(
        *observed_deadline.lock().expect("deadline capture"),
        Some(37)
    );
    assert!(execution.cancelled);
    assert_eq!(execution.events[0].kind, "cancelled");
}

#[test]
fn plugin_job_rejects_a_result_from_an_unsupported_schema() {
    let mut host = InMemoryPluginHost::with_runtime(Box::new(JobBoundaryRuntime {
        observed_deadline: Arc::new(Mutex::new(None)),
        cancel_job: false,
        result_schema_version: JOB_SCHEMA_VERSION + 1,
    }));
    let manifest = job_manifest(true);
    let id = manifest.id.clone();
    host.register(manifest)
        .expect("Job manifest should register");
    host.activate(&id).expect("Job extension should activate");
    let handle = host
        .job_handles(&id)
        .expect("active Job contribution should expose a handle")
        .pop()
        .expect("Job contribution should have a handle");

    let error = handle
        .invoke(PluginJobInvocation {
            job_id: "job-stale-result".into(),
            job_kind: "job.example.analyze".into(),
            attempt: 1,
            input: serde_json::json!({ "assetId": "asset-1" }),
            provenance: JobProvenance::for_owner("job.example"),
            budget: JobBudgetRequest::default(),
            deadline_ms: None,
            cancellation_grace_ms: default_job_cancellation_grace_ms(),
            checkpoint: None,
        })
        .expect_err("unsupported result schema must be rejected");
    assert!(matches!(
        error,
        PluginError::RuntimeInvocation { message, .. }
            if message.contains("result schema") && message.contains("unsupported")
    ));
}

#[test]
fn runtime_job_request_serializes_deadline_and_schema_reference() {
    let request = RuntimeInvocationRequest {
        protocol_version: RUNTIME_PROTOCOL_VERSION,
        kind: "invoke-job",
        method: "job.run",
        extension_id: "job.example",
        command_id: "job.example.analyze",
        args: &serde_json::json!({ "assetId": "asset-1" }),
        context: &serde_json::json!({ "attempt": 1 }),
        schema_id: Some("velvet.job.job.example.job.example.analyze"),
        schema_version: Some(JOB_SCHEMA_VERSION),
        deadline_ms: Some(37),
    };
    let value = serde_json::to_value(request).expect("runtime request should serialize");
    assert_eq!(value["kind"], "invoke-job");
    assert_eq!(value["deadlineMs"], 37);
    assert_eq!(value["schemaVersion"], JOB_SCHEMA_VERSION);
}

#[test]
fn job_handles_reject_stale_provenance_schema_before_runtime_invocation() {
    let mut host = InMemoryPluginHost::default();
    let manifest = job_manifest(true);
    let id = manifest.id.clone();
    host.register(manifest)
        .expect("Job manifest should register");
    host.activate(&id).expect("Job extension should activate");
    let handle = host
        .job_handles(&id)
        .expect("active Job contribution should expose a handle")
        .pop()
        .expect("Job contribution should have a handle");
    let error = handle
        .invoke(PluginJobInvocation {
            job_id: "job-stale-schema".into(),
            job_kind: "job.example.analyze".into(),
            attempt: 1,
            input: serde_json::json!({ "assetId": "asset-1" }),
            provenance: JobProvenance {
                schema_version: JOB_SCHEMA_VERSION + 1,
                ..JobProvenance::for_owner("job.example")
            },
            budget: JobBudgetRequest::default(),
            deadline_ms: None,
            cancellation_grace_ms: default_job_cancellation_grace_ms(),
            checkpoint: None,
        })
        .expect_err("stale Job provenance must fail before runtime invocation");
    assert!(matches!(
        error,
        PluginError::RuntimeInvocation { message, .. }
            if message.contains("schema") && message.contains("unsupported")
    ));

    let owner_error = handle
        .invoke(PluginJobInvocation {
            job_id: "job-cross-owner".into(),
            job_kind: "job.example.analyze".into(),
            attempt: 1,
            input: serde_json::json!({ "assetId": "asset-1" }),
            provenance: JobProvenance::for_owner("other.extension"),
            budget: JobBudgetRequest::default(),
            deadline_ms: None,
            cancellation_grace_ms: default_job_cancellation_grace_ms(),
            checkpoint: None,
        })
        .expect_err("cross-owner Job provenance must fail before runtime invocation");
    assert!(matches!(
        owner_error,
        PluginError::RuntimeInvocation { message, .. }
            if message.contains("owner") && message.contains("does not match")
    ));
}

#[test]
fn plugin_job_invocation_and_checkpoint_event_are_bounded_wire_shapes() {
    let invocation: PluginJobInvocation = serde_json::from_value(serde_json::json!({
        "jobId": "job-1",
        "jobKind": "job.example.analyze",
        "attempt": 2,
        "input": { "assetId": "asset-1" },
        "provenance": {
            "ownerId": "job.example",
            "inputFingerprint": "sha256:asset-1",
            "schemaVersion": 1
        },
        "budget": {
            "memoryBytes": 1024,
            "diskBytes": 2048,
            "childProcesses": 1,
            "concurrency": 1,
            "cpuMillisPerMinute": 100,
            "networkBytesPerMinute": 200
        },
        "deadlineMs": 5000,
        "cancellationGraceMs": 250,
        "checkpoint": { "cursor": 4 }
    }))
    .expect("job invocation should decode");
    assert_eq!(invocation.attempt, 2);
    assert_eq!(invocation.budget.memory_bytes, 1024);
    assert_eq!(invocation.checkpoint.unwrap()["cursor"], 4);

    let event: PluginInvocationEvent = serde_json::from_value(serde_json::json!({
        "kind": "checkpoint",
        "checkpoint": { "cursor": 5 }
    }))
    .expect("checkpoint event should decode");
    assert_eq!(event.checkpoint.unwrap()["cursor"], 5);
}

#[test]
fn preview_provider_requires_render_permission_and_uses_the_runtime_boundary() {
    let mut denied_host = InMemoryPluginHost::default();
    let denied_id = preview_manifest(false).id.clone();
    denied_host
        .register(preview_manifest(false))
        .expect("preview manifest should validate before permission check");
    denied_host
        .activate(&denied_id)
        .expect("built-in preview provider should activate");
    assert!(matches!(
        denied_host.invoke_preview(
            &denied_id,
            "preview.image".into(),
            serde_json::json!({"assetId":"asset-1"}),
        ),
        Err(PluginError::PermissionDenied {
            permission: Permission::PreviewRender,
            ..
        })
    ));

    let mut host = InMemoryPluginHost::default();
    let manifest = preview_manifest(true);
    let id = manifest.id.clone();
    host.register(manifest)
        .expect("preview manifest with permission should register");
    host.activate(&id)
        .expect("preview provider should activate");
    let result = host
        .invoke_preview(
            &id,
            "preview.image".into(),
            serde_json::json!({"assetId":"asset-1"}),
        )
        .expect("preview invocation should reach the runtime process");
    assert_eq!(result.error, None);
    assert_eq!(result.result, Some(serde_json::json!({"accepted": true})));
}

#[test]
fn preview_augmentation_uses_the_same_permission_and_runtime_boundary() {
    let mut manifest = preview_manifest(true);
    manifest.contributions[0].id = "preview.metadata".into();
    manifest.contributions[0].kind = ContributionKind::PreviewAugmentation;
    manifest.contributions[0]
        .preview
        .as_mut()
        .expect("preview metadata should exist")
        .slot = Some(PreviewAugmentationSlot::Metadata);

    let mut host = InMemoryPluginHost::default();
    let id = manifest.id.clone();
    host.register(manifest)
        .expect("preview augmentation manifest should register");
    host.activate(&id)
        .expect("preview augmentation should activate");
    let result = host
        .invoke_preview_augmentation(
            &id,
            "preview.metadata".into(),
            serde_json::json!({"assetId":"asset-1"}),
        )
        .expect("preview augmentation should reach the runtime process");
    assert_eq!(result.error, None);
    assert_eq!(result.result, Some(serde_json::json!({"accepted": true})));
}

#[test]
fn view_contributions_must_declare_a_zone() {
    let mut host = InMemoryPluginHost::default();

    assert!(matches!(
        host.register(view_manifest(None)),
        Err(PluginError::InvalidContribution(_))
    ));
}

#[test]
fn manifests_with_an_unknown_schema_are_rejected_before_registration() {
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    manifest.manifest_schema = MANIFEST_SCHEMA_VERSION + 1;
    let mut host = InMemoryPluginHost::default();

    assert!(matches!(
        host.register(manifest),
        Err(PluginError::InvalidContribution(message))
            if message.contains("manifest schema")
    ));
}

#[test]
fn bridge_and_runtime_protocol_ranges_fail_closed_when_the_host_cannot_negotiate_them() {
    let mut bridge_incompatible = view_manifest(Some(WorkbenchZone::Navigation));
    bridge_incompatible.bridge_api = VersionRange("^2".into());
    let mut host = InMemoryPluginHost::default();
    assert!(matches!(
        host.register(bridge_incompatible),
        Err(PluginError::InvalidContribution(message))
            if message.contains("bridge API range")
    ));

    let mut runtime_incompatible = view_manifest(Some(WorkbenchZone::Navigation));
    runtime_incompatible.runtime_protocol = VersionRange("^2".into());
    assert!(matches!(
        host.register(runtime_incompatible),
        Err(PluginError::InvalidContribution(message))
            if message.contains("runtime protocol range")
    ));
}

#[test]
fn host_storage_is_manifest_declared_namespaced_and_failure_isolated() {
    struct FailingMigration;
    impl MigrationParticipant for FailingMigration {
        fn migrate(
            &self,
            _source: &ExtensionStorageView,
            _destination: &mut ExtensionStorageTransaction,
        ) -> Result<(), ExtensionStorageError> {
            Err(ExtensionStorageError::Unavailable(
                "injected failure".into(),
            ))
        }
    }

    let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
    let mut host = InMemoryPluginHost::default();
    host.set_extension_storage_store(snapshots);

    let storage = Some(ExtensionStorageDeclaration {
        schema_version: 1,
        quota_bytes: 1024,
    });
    let first = ExtensionManifest {
        id: ExtensionId("sample.one".into()),
        storage: storage.clone(),
        ..ExtensionManifest::default()
    };
    let second = ExtensionManifest {
        id: ExtensionId("sample.two".into()),
        storage,
        ..ExtensionManifest::default()
    };
    host.register(first).unwrap();
    host.register(second).unwrap();

    let first_storage = host
        .extension_storage(&ExtensionId("sample.one".into()))
        .unwrap();
    let second_storage = host
        .extension_storage(&ExtensionId("sample.two".into()))
        .unwrap();
    let mut transaction = first_storage.begin_transaction().unwrap();
    transaction.put("answer", b"42".to_vec()).unwrap();
    let committed = transaction.commit().unwrap();
    assert_eq!(committed.generation, 1);
    assert!(second_storage.load().unwrap().entries.is_empty());
    assert!(matches!(
        host.migrate_extension_state(&ExtensionId("sample.one".into()), &FailingMigration),
        Err(PluginError::ExtensionStorage { .. })
    ));
    assert_eq!(first_storage.load().unwrap().generation, 1);
    assert!(second_storage.load().unwrap().entries.is_empty());
}

#[test]
fn runtime_storage_export_is_host_validated_and_does_not_publish_state() {
    let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let shutdowns = Arc::new(Mutex::new(0));
    let mut host = InMemoryPluginHost::with_runtime(Box::new(StorageRuntime {
        observed: observed.clone(),
        shutdowns,
    }));
    host.set_extension_storage_store(snapshots);
    let manifest = ExtensionManifest {
        id: ExtensionId("runtime.backup".into()),
        runtime: RuntimeKind::Wasm,
        launch: Some(RuntimeLaunchSpec {
            program: "runner".into(),
            args: Vec::new(),
        }),
        storage: Some(ExtensionStorageDeclaration {
            schema_version: 1,
            quota_bytes: 1024,
        }),
        ..ExtensionManifest::default()
    };
    let id = manifest.id.clone();
    host.register(manifest).unwrap();
    let storage = host.extension_storage(&id).unwrap();
    let mut transaction = storage.begin_transaction().unwrap();
    transaction.put("answer", b"42".to_vec()).unwrap();
    transaction.commit().unwrap();
    host.activate(&id).unwrap();

    let export = host.export_extension_state_from_runtime(&id).unwrap();
    assert_eq!(export.schema_version, 1);
    assert_eq!(export.source_generation, 1);
    assert_eq!(export.entries["answer"], b"42");
    assert_eq!(storage.load().unwrap().generation, 1);
    let invocation = &observed.lock().unwrap()[0];
    assert_eq!(invocation.method, STORAGE_EXPORT_METHOD);
    assert_eq!(invocation.command_id, "storage");
    assert_eq!(invocation.context["capability"], "extension-storage");
}

#[test]
fn runtime_storage_migration_commits_only_the_new_extension_generation() {
    let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut host = InMemoryPluginHost::with_runtime(Box::new(StorageRuntime {
        observed: observed.clone(),
        shutdowns: Arc::new(Mutex::new(0)),
    }));
    host.set_extension_storage_store(snapshots.clone());
    let first = ExtensionManifest {
        id: ExtensionId("runtime.migration".into()),
        runtime: RuntimeKind::Wasm,
        launch: Some(RuntimeLaunchSpec {
            program: "runner".into(),
            args: Vec::new(),
        }),
        storage: Some(ExtensionStorageDeclaration {
            schema_version: 1,
            quota_bytes: 1024,
        }),
        ..ExtensionManifest::default()
    };
    let id = first.id.clone();
    host.register(first).unwrap();
    let storage = host.extension_storage(&id).unwrap();
    let mut transaction = storage.begin_transaction().unwrap();
    transaction.put("answer", b"42".to_vec()).unwrap();
    transaction.commit().unwrap();
    host.activate(&id).unwrap();

    host.manifests.get_mut(&id).unwrap().storage = Some(ExtensionStorageDeclaration {
        schema_version: 2,
        quota_bytes: 1024,
    });
    let migrated = host.migrate_extension_state_from_runtime(&id).unwrap();
    assert_eq!(migrated.schema_version, 2);
    assert_eq!(migrated.generation, 2);
    assert_eq!(migrated.entries["answer.v2"], b"42");
    assert_eq!(storage.load_for_migration().unwrap().generation, 2);
    let invocation = &observed.lock().unwrap()[0];
    let request: StorageRuntimeRequest = serde_json::from_value(invocation.args.clone()).unwrap();
    assert_eq!(invocation.method, STORAGE_MIGRATE_METHOD);
    assert_eq!(request.source_schema_version, 1);
    assert_eq!(request.target_schema_version, 2);
    assert_eq!(request.source_generation, 1);
    assert_eq!(
        snapshots
            .load_latest("extension.runtime.migration", "storage")
            .unwrap()
            .unwrap()
            .metadata
            .generation,
        2
    );
}

#[test]
fn external_activation_migrates_an_existing_storage_generation_before_ready() {
    let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut host = InMemoryPluginHost::with_runtime(Box::new(StorageRuntime {
        observed: observed.clone(),
        shutdowns: Arc::new(Mutex::new(0)),
    }));
    host.set_extension_storage_store(snapshots);
    let manifest = ExtensionManifest {
        id: ExtensionId("runtime.activation-migration".into()),
        runtime: RuntimeKind::Wasm,
        launch: Some(RuntimeLaunchSpec {
            program: "runner".into(),
            args: Vec::new(),
        }),
        storage: Some(ExtensionStorageDeclaration {
            schema_version: 1,
            quota_bytes: 1024,
        }),
        ..ExtensionManifest::default()
    };
    let id = manifest.id.clone();
    host.register(manifest).unwrap();
    let storage = host.extension_storage(&id).unwrap();
    let mut transaction = storage.begin_transaction().unwrap();
    transaction.put("answer", b"42".to_vec()).unwrap();
    transaction.commit().unwrap();
    host.activate(&id).unwrap();
    host.disable(&id, DisableReason::User).unwrap();

    host.manifests.get_mut(&id).unwrap().storage = Some(ExtensionStorageDeclaration {
        schema_version: 2,
        quota_bytes: 1024,
    });
    host.activate(&id)
        .expect("activation should migrate before exposing the runtime as ready");
    let migrated = host
        .extension_storage(&id)
        .unwrap()
        .load_for_migration()
        .unwrap();
    assert_eq!(migrated.schema_version, 2);
    assert_eq!(migrated.entries["answer.v2"], b"42");
    assert_eq!(
        observed
            .lock()
            .unwrap()
            .iter()
            .filter(|invocation| invocation.method == STORAGE_MIGRATE_METHOD)
            .count(),
        1
    );
}

#[test]
fn runtime_storage_failure_disables_only_the_failing_extension_and_keeps_state() {
    let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
    let shutdowns = Arc::new(Mutex::new(0));
    let mut host = InMemoryPluginHost::with_runtime(Box::new(FakeRuntime {
        plans: VecDeque::from([StartPlan::ProcessWithInvocation(
            vec![RuntimeHealth::Running],
            RuntimeError::InvocationFailed("migration crashed".into()),
        )]),
        grants: Arc::new(Mutex::new(Vec::new())),
        shutdowns: shutdowns.clone(),
    }));
    host.set_extension_storage_store(snapshots);
    let id = ExtensionId("runtime.failed-migration".into());
    let manifest = ExtensionManifest {
        id: id.clone(),
        runtime: RuntimeKind::Wasm,
        launch: Some(RuntimeLaunchSpec {
            program: "runner".into(),
            args: Vec::new(),
        }),
        storage: Some(ExtensionStorageDeclaration {
            schema_version: 2,
            quota_bytes: 1024,
        }),
        ..ExtensionManifest::default()
    };
    host.register(manifest).unwrap();
    let storage = host.extension_storage(&id).unwrap();
    let mut transaction = storage.begin_transaction().unwrap();
    transaction.put("answer", b"42".to_vec()).unwrap();
    transaction.commit().unwrap();
    host.activate(&id).unwrap();

    assert!(matches!(
        host.migrate_extension_state_from_runtime(&id),
        Err(PluginError::ExtensionStorage { .. })
    ));
    assert_eq!(host.discover()[0].state, ExtensionState::Failed);
    assert_eq!(storage.load_for_migration().unwrap().generation, 1);
    assert_eq!(
        storage.load_for_migration().unwrap().entries["answer"],
        b"42"
    );
    assert_eq!(*shutdowns.lock().unwrap(), 1);
    assert!(host
        .drain_diagnostics()
        .iter()
        .any(|record| record.code == "storage-runtime-failed"));
}

#[test]
fn storage_recovery_projects_generations_and_restores_without_reactivating() {
    let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
    let mut host = InMemoryPluginHost::default();
    host.set_extension_storage_store(snapshots);
    let id = ExtensionId("storage.recovery".into());
    host.register(ExtensionManifest {
        id: id.clone(),
        storage: Some(ExtensionStorageDeclaration {
            schema_version: 1,
            quota_bytes: 1024,
        }),
        ..ExtensionManifest::default()
    })
    .unwrap();
    let storage = host.extension_storage(&id).unwrap();
    let mut first = storage.begin_transaction().unwrap();
    first.put("value", b"one".to_vec()).unwrap();
    first.commit().unwrap();
    let mut second = storage.begin_transaction().unwrap();
    second.put("value", b"two".to_vec()).unwrap();
    second.commit().unwrap();
    host.activate(&id).unwrap();

    let projection = host.extension_storage_recovery(&id).unwrap();
    assert_eq!(projection.status, ExtensionStorageRecoveryStatus::Available);
    assert_eq!(projection.current_generation, Some(2));
    assert_eq!(projection.previous_generation, Some(1));
    assert_eq!(projection.quarantine_generation, Some(2));

    let restored = host.restore_extension_state(&id).unwrap();
    assert_eq!(restored.generation, 3);
    assert_eq!(restored.entries["value"], b"one");
    assert_eq!(host.discover()[0].state, ExtensionState::Disabled);
    assert_eq!(
        host.extension_storage_recovery(&id)
            .unwrap()
            .current_generation,
        Some(3)
    );
}

#[test]
fn storage_recovery_quarantine_is_compare_and_swap_and_isolated() {
    let snapshots = Arc::new(InMemoryStateSnapshotStore::default());
    let mut host = InMemoryPluginHost::default();
    host.set_extension_storage_store(snapshots.clone());
    let id = ExtensionId("storage.quarantine".into());
    host.register(ExtensionManifest {
        id: id.clone(),
        storage: Some(ExtensionStorageDeclaration {
            schema_version: 1,
            quota_bytes: 1024,
        }),
        ..ExtensionManifest::default()
    })
    .unwrap();
    let storage = host.extension_storage(&id).unwrap();
    let mut first = storage.begin_transaction().unwrap();
    first.put("value", b"one".to_vec()).unwrap();
    first.commit().unwrap();
    let mut second = storage.begin_transaction().unwrap();
    second.put("value", b"two".to_vec()).unwrap();
    second.commit().unwrap();
    host.activate(&id).unwrap();

    assert!(matches!(
        host.quarantine_extension_state(&id, 1),
        Err(PluginError::ExtensionStorage { message, .. }) if message.contains("Conflict")
    ));
    assert_eq!(storage.load().unwrap().generation, 2);
    assert_eq!(host.discover()[0].state, ExtensionState::Active);

    host.quarantine_extension_state(&id, 2).unwrap();
    let current = storage.load().unwrap();
    assert_eq!(current.generation, 1);
    assert_eq!(current.entries["value"], b"one");
    assert_eq!(
        snapshots
            .load_generation("storage.quarantine", "storage", 2)
            .unwrap(),
        None
    );
    assert_eq!(host.discover()[0].state, ExtensionState::Disabled);
}

#[test]
fn corrupt_unknown_storage_generation_can_be_quarantined_by_filename_metadata() {
    let root = temporary_manifest_dir();
    let snapshot_dir = root.join("extension.storage.corrupt/storage");
    fs::create_dir_all(&snapshot_dir).unwrap();
    fs::write(snapshot_dir.join("7.snapshot.json"), b"future bytes").unwrap();

    let mut host = InMemoryPluginHost::default();
    host.set_extension_storage_store(Arc::new(JsonFileStateSnapshotStore::new(&root)));
    let id = ExtensionId("storage.corrupt".into());
    host.register(ExtensionManifest {
        id: id.clone(),
        storage: Some(ExtensionStorageDeclaration {
            schema_version: 1,
            quota_bytes: 1024,
        }),
        ..ExtensionManifest::default()
    })
    .unwrap();

    let projection = host.extension_storage_recovery(&id).unwrap();
    assert_eq!(projection.status, ExtensionStorageRecoveryStatus::Corrupt);
    assert_eq!(projection.current_generation, Some(7));
    assert_eq!(projection.quarantine_generation, Some(7));

    host.quarantine_extension_state(&id, 7).unwrap();
    assert_eq!(
        host.extension_storage_recovery(&id).unwrap().status,
        ExtensionStorageRecoveryStatus::Empty
    );
    assert!(snapshot_dir.join("7.quarantine.json").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn undeclared_or_oversized_extension_storage_is_rejected() {
    let mut host = InMemoryPluginHost::default();
    host.register(ExtensionManifest {
        id: ExtensionId("sample.no-storage".into()),
        ..ExtensionManifest::default()
    })
    .unwrap();
    assert!(matches!(
        host.extension_storage(&ExtensionId("sample.no-storage".into())),
        Err(PluginError::ExtensionStorage { .. })
    ));

    let invalid = ExtensionManifest {
        id: ExtensionId("sample.too-large".into()),
        storage: Some(ExtensionStorageDeclaration {
            schema_version: 1,
            quota_bytes: MAX_EXTENSION_STORAGE_BYTES as u64 + 1,
        }),
        ..ExtensionManifest::default()
    };
    assert!(matches!(
        host.register(invalid),
        Err(PluginError::InvalidContribution(message))
            if message.contains("quotaBytes")
    ));
}

#[test]
fn diagnostic_sink_is_bounded_ordered_and_redacts_paths() {
    let mut sink = PluginDiagnosticSink::default();
    sink.record(PluginDiagnosticInput {
        extension_id: ExtensionId("sample.reader".into()),
        kind: PluginDiagnosticKind::Runtime,
        code: "runtime-crashed".into(),
        message:
            "failed at /Users/example/private/file.txt path=/Users/example/private/file.txt\nsecret"
                .into(),
        job_id: None,
        attempt: None,
        retryable: true,
    });
    let records = sink.drain();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].sequence, 1);
    assert_eq!(
        records[0].message,
        "failed at <redacted-path> <redacted-path> secret"
    );
    assert!(records[0].message.len() <= MAX_PLUGIN_DIAGNOSTIC_MESSAGE_BYTES);

    for index in 0..=MAX_PLUGIN_DIAGNOSTICS {
        sink.record(PluginDiagnosticInput {
            extension_id: ExtensionId("sample.reader".into()),
            kind: PluginDiagnosticKind::Job,
            code: format!("job-{index}"),
            message: "bounded".into(),
            job_id: None,
            attempt: None,
            retryable: true,
        });
    }
    let records = sink.drain();
    assert_eq!(records.len(), MAX_PLUGIN_DIAGNOSTICS);
    assert_eq!(records.first().map(|record| record.sequence), Some(3));
    assert_eq!(records.last().map(|record| record.sequence), Some(258));
}

#[test]
fn diagnostic_messages_are_bounded_at_a_utf8_boundary() {
    let mut sink = PluginDiagnosticSink::default();
    sink.record(PluginDiagnosticInput {
        extension_id: ExtensionId("sample.reader".into()),
        kind: PluginDiagnosticKind::Runtime,
        code: "runtime-crashed".into(),
        message: "界".repeat(MAX_PLUGIN_DIAGNOSTIC_MESSAGE_BYTES),
        job_id: None,
        attempt: None,
        retryable: true,
    });

    let records = sink.drain();
    assert!(records[0].message.len() <= MAX_PLUGIN_DIAGNOSTIC_MESSAGE_BYTES);
    assert!(records[0]
        .message
        .is_char_boundary(records[0].message.len()));
}

#[test]
fn structured_logs_and_notifications_are_redacted_bounded_and_rate_limited() {
    let host = InMemoryPluginHost::default();
    let extension_id = ExtensionId("sample.diagnostics".into());
    for _ in 0..(MAX_PLUGIN_LOGS_PER_MINUTE + 5) {
        host.record_log(PluginLogInput {
            extension_id: extension_id.clone(),
            level: PluginLogLevel::Info,
            category: "runtime".into(),
            message: "token=/Users/private/asset.png".into(),
            session_id: Some("session-1".into()),
            job_id: Some("job-1".into()),
            operation_id: Some("operation-1".into()),
            retryable: true,
        });
    }
    for _ in 0..(MAX_PLUGIN_NOTIFICATIONS_PER_MINUTE + 5) {
        host.record_notification(PluginNotificationInput {
            extension_id: extension_id.clone(),
            severity: PluginNotificationSeverity::Warning,
            title: "Plugin warning".into(),
            message: "path=/Users/private/asset.png".into(),
            session_id: Some("session-1".into()),
            job_id: Some("job-1".into()),
            operation_id: Some("operation-1".into()),
            retryable: true,
        });
    }
    let mut host = host;
    let logs = host.drain_logs();
    let notifications = host.drain_notifications();
    assert_eq!(logs.len(), MAX_PLUGIN_LOGS_PER_MINUTE);
    assert_eq!(notifications.len(), MAX_PLUGIN_NOTIFICATIONS_PER_MINUTE);
    assert!(logs[0].message.contains("<redacted-path>"));
    assert!(notifications[0].message.contains("<redacted-path>"));
    assert_eq!(logs[0].session_id.as_deref(), Some("session-1"));
    assert_eq!(notifications[0].job_id.as_deref(), Some("job-1"));
}

#[test]
fn valid_view_contributions_are_discoverable_and_cleaned_up() {
    let mut host = InMemoryPluginHost::default();
    host.register(view_manifest(Some(WorkbenchZone::Navigation)))
        .unwrap();
    host.activate(&ExtensionId("example".into())).unwrap();

    assert_eq!(
        host.active_contributions(&ExtensionId("example".into()))
            .unwrap()
            .len(),
        1
    );
    host.disable(&ExtensionId("example".into()), DisableReason::Crash)
        .unwrap();
    assert!(host
        .active_contributions(&ExtensionId("example".into()))
        .unwrap()
        .is_empty());
}

#[test]
fn registry_projection_has_owner_metadata_and_disposable_handles() {
    let mut host = InMemoryPluginHost::default();
    let id = ExtensionId("example".into());
    host.register(view_manifest(Some(WorkbenchZone::Navigation)))
        .unwrap();
    host.activate(&id).unwrap();

    let snapshot = host.registry_snapshot();
    assert_eq!(
        snapshot.schema_version,
        CONTRIBUTION_REGISTRY_SCHEMA_VERSION
    );
    assert_eq!(snapshot.generation, 1);
    let registration = snapshot
        .contributions
        .first()
        .expect("active contribution should be projected");
    assert_eq!(registration.extension_id, id);
    assert_eq!(registration.contribution_id, "example.view");
    assert_eq!(registration.namespace, "workbench");
    assert_eq!(registration.scope, ContributionScope::Workbench);
    assert_eq!(registration.zone, Some(WorkbenchZone::Navigation));
    assert_eq!(registration.schema_version, CONTRIBUTION_SCHEMA_VERSION);

    let handle = host
        .contribution_handles(&id)
        .unwrap()
        .pop()
        .expect("active contribution should expose a handle");
    assert!(host.dispose_contribution(&handle).unwrap());
    assert!(!host.dispose_contribution(&handle).unwrap());
    assert!(host.registry_snapshot().contributions.is_empty());
    assert!(host.active_contributions(&id).unwrap().is_empty());
    host.disable(&id, DisableReason::User).unwrap();
    assert!(host.registry_snapshot().contributions.is_empty());
}

#[test]
fn registry_cleanup_is_isolated_when_one_runtime_fails() {
    let grants = Arc::new(Mutex::new(Vec::new()));
    let runtime = FakeRuntime {
        plans: VecDeque::from([
            StartPlan::ProcessWithInvocation(
                vec![RuntimeHealth::Running],
                RuntimeError::InvocationTimeout,
            ),
            StartPlan::Process(vec![RuntimeHealth::Running]),
        ]),
        grants,
        shutdowns: Arc::new(Mutex::new(0)),
    };
    let mut host = InMemoryPluginHost::with_runtime(Box::new(runtime));
    let mut first = view_manifest(Some(WorkbenchZone::Navigation));
    first.id = ExtensionId("first.example".into());
    first.contributions[0].id = "first.example.view".into();
    first.contributions[0].kind = velvet_plugin_api::ContributionKind::Command;
    first.contributions[0].container = None;
    first.contributions[0].zone = None;
    first.contributions[0].icon = None;
    first.contributions[0].default_visible = None;
    let first_id = first.id.clone();
    let mut second = view_manifest(Some(WorkbenchZone::Navigation));
    second.id = ExtensionId("second.example".into());
    second.contributions[0].id = "second.example.view".into();
    let second_id = second.id.clone();
    host.register(first).unwrap();
    host.register(second).unwrap();
    host.activate(&first_id).unwrap();
    host.activate(&second_id).unwrap();
    assert_eq!(host.registry_snapshot().contributions.len(), 2);

    let _ = host.invoke_command(
        &first_id,
        "first.example.view".into(),
        Value::Null,
        Value::Null,
    );
    assert!(host.active_contributions(&first_id).unwrap().is_empty());
    assert_eq!(host.active_contributions(&second_id).unwrap().len(), 1);
    assert_eq!(host.registry_snapshot().contributions.len(), 1);
}

#[test]
fn activation_resolves_extension_dependencies_before_the_requested_extension() {
    let runtime = FakeRuntime {
        plans: VecDeque::from([
            StartPlan::Process(vec![RuntimeHealth::Running]),
            StartPlan::Fail("dependent runtime failed".into()),
        ]),
        grants: Arc::new(Mutex::new(Vec::new())),
        shutdowns: Arc::new(Mutex::new(0)),
    };
    let mut host = InMemoryPluginHost::with_runtime(Box::new(runtime));

    let mut dependency = view_manifest(Some(WorkbenchZone::Navigation));
    dependency.id = ExtensionId("base.example".into());
    dependency.contributions[0].id = "base.example.view".into();
    let dependency_id = dependency.id.clone();

    let mut feature = view_manifest(Some(WorkbenchZone::Contextual));
    feature.id = ExtensionId("feature.example".into());
    feature.contributions[0].id = "feature.example.view".into();
    feature.extension_dependencies = vec![dependency_id.clone()];
    let feature_id = feature.id.clone();

    host.register(dependency).unwrap();
    host.register(feature).unwrap();

    assert!(matches!(
        host.activate(&feature_id),
        Err(PluginError::RuntimeStart { extension_id, .. }) if extension_id == feature_id
    ));
    assert_eq!(
        host.discover()
            .into_iter()
            .find(|summary| summary.id == dependency_id)
            .map(|summary| summary.state),
        Some(ExtensionState::Active)
    );
    assert_eq!(
        host.discover()
            .into_iter()
            .find(|summary| summary.id == feature_id)
            .map(|summary| summary.state),
        Some(ExtensionState::Failed)
    );
    assert_eq!(host.registry_snapshot().contributions.len(), 1);
    assert_eq!(
        host.registry_snapshot().contributions[0].extension_id,
        dependency_id
    );
}

#[test]
fn activation_rejects_dependency_cycles_without_starting_a_runtime() {
    let shutdowns = Arc::new(Mutex::new(0));
    let mut host = InMemoryPluginHost::with_runtime(Box::new(FakeRuntime {
        plans: VecDeque::from([StartPlan::Process(vec![RuntimeHealth::Running])]),
        grants: Arc::new(Mutex::new(Vec::new())),
        shutdowns,
    }));

    let mut first = view_manifest(Some(WorkbenchZone::Navigation));
    first.id = ExtensionId("first.example".into());
    first.contributions[0].id = "first.example.view".into();
    first.extension_dependencies = vec![ExtensionId("second.example".into())];

    let mut second = view_manifest(Some(WorkbenchZone::Contextual));
    second.id = ExtensionId("second.example".into());
    second.contributions[0].id = "second.example.view".into();
    second.extension_dependencies = vec![ExtensionId("first.example".into())];
    let first_id = first.id.clone();

    host.register(first).unwrap();
    host.register(second).unwrap();

    assert!(matches!(
        host.activate(&first_id),
        Err(PluginError::InvalidContribution(message))
            if message.contains("cyclic extension dependency")
    ));
    assert!(host.registry_snapshot().contributions.is_empty());
    assert!(host
        .discover()
        .into_iter()
        .all(|summary| { summary.state == ExtensionState::Registered }));
}

#[test]
fn activation_candidates_cover_each_declared_lazy_event_shape() {
    let mut host = InMemoryPluginHost::default();
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    manifest.activation_events = vec![
        ActivationEvent::WorkbenchReady,
        ActivationEvent::App {
            app_id: "asset-manager".into(),
        },
        ActivationEvent::View {
            contribution_id: "example.view".into(),
        },
        ActivationEvent::Command {
            contribution_id: "example.view".into(),
        },
        ActivationEvent::Preview {
            selector: "image/*".into(),
        },
        ActivationEvent::Job {
            job_kind: "asset-analysis".into(),
        },
    ];
    let id = manifest.id.clone();
    host.register(manifest).unwrap();

    let events = [
        ActivationEvent::WorkbenchReady,
        ActivationEvent::App {
            app_id: "asset-manager".into(),
        },
        ActivationEvent::View {
            contribution_id: "example.view".into(),
        },
        ActivationEvent::Command {
            contribution_id: "example.view".into(),
        },
        ActivationEvent::Preview {
            selector: "image/*".into(),
        },
        ActivationEvent::Job {
            job_kind: "asset-analysis".into(),
        },
    ];
    for event in events {
        assert_eq!(host.activation_candidates(&event), vec![id.clone()]);
    }
}

#[test]
fn active_declared_commands_are_invoked_through_the_runtime_boundary() {
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    let command = &mut manifest.contributions[0];
    command.id = "example.open".into();
    command.kind = ContributionKind::Command;
    command.container = None;
    command.zone = None;
    command.icon = None;
    command.default_visible = None;
    let id = manifest.id.clone();

    let mut host = InMemoryPluginHost::default();
    host.register(manifest).unwrap();
    host.activate(&id).unwrap();

    let result = host
        .invoke_command(
            &id,
            "example.open".into(),
            serde_json::json!({ "source": "test" }),
            serde_json::json!({ "activeAppId": null }),
        )
        .expect("an active declared command should reach the runtime");
    assert_eq!(result.error, None);
    assert_eq!(result.result, Some(serde_json::json!({ "accepted": true })));

    assert!(matches!(
        host.invoke_command(&id, "example.missing".into(), Value::Null, Value::Null),
        Err(PluginError::InvalidContribution(_))
    ));

    host.disable(&id, DisableReason::User).unwrap();
    assert!(matches!(
        host.invoke_command(&id, "example.open".into(), Value::Null, Value::Null),
        Err(PluginError::InvalidState {
            state: ExtensionState::Disabled,
            ..
        })
    ));
}

#[test]
fn preview_actions_require_render_permission_and_use_the_command_boundary() {
    let mut manifest = view_manifest(None);
    let command = &mut manifest.contributions[0];
    command.id = "example.analyze".into();
    command.kind = ContributionKind::Command;
    command.scope = ContributionScope::Workbench;
    command.namespace = Some("workbench".into());
    command.container = None;
    command.zone = None;
    command.icon = None;
    command.default_visible = None;
    manifest.permission_sets = vec![ZonePermissionSet {
        zone: WorkbenchZone::Contextual,
        permissions: vec![Permission::PreviewRender],
    }];
    let id = manifest.id.clone();

    let mut host = InMemoryPluginHost::default();
    host.register(manifest)
        .expect("preview action should register");
    host.activate(&id).expect("preview action should activate");
    assert!(host
        .has_permission(&id, Permission::PreviewRender)
        .expect("permission lookup should succeed"));
    let result = host
        .invoke_preview_action(
            &id,
            "example.analyze".into(),
            serde_json::json!({ "scope": "metadata" }),
            velvet_plugin_api::PreviewActionContext {
                session_id: "preview.example:asset-1".into(),
                provider_id: "example.preview".into(),
                asset_id: "asset-1".into(),
                content_kind: "image".into(),
                intent: velvet_plugin_api::PreviewIntent::Analyze,
                surface: velvet_plugin_api::PreviewSurface::Workspace,
            },
        )
        .expect("preview action should reach the runtime");
    assert_eq!(result.result, Some(serde_json::json!({ "accepted": true })));

    let mut denied = view_manifest(None);
    denied.id = ExtensionId("example.denied".into());
    denied.contributions[0].id = "example.denied".into();
    denied.contributions[0].kind = ContributionKind::Command;
    denied.contributions[0].scope = ContributionScope::Workbench;
    denied.contributions[0].namespace = Some("workbench".into());
    denied.contributions[0].container = None;
    denied.contributions[0].zone = None;
    denied.contributions[0].icon = None;
    denied.contributions[0].default_visible = None;
    let denied_id = denied.id.clone();
    host.register(denied)
        .expect("denied action should register");
    host.activate(&denied_id)
        .expect("denied action should activate");
    assert!(matches!(
        host.invoke_preview_action(
            &denied_id,
            "example.denied".into(),
            Value::Null,
            velvet_plugin_api::PreviewActionContext {
                session_id: "preview.example:asset-1".into(),
                provider_id: "example.preview".into(),
                asset_id: "asset-1".into(),
                content_kind: "image".into(),
                intent: velvet_plugin_api::PreviewIntent::Quick,
                surface: velvet_plugin_api::PreviewSurface::Workspace,
            },
        ),
        Err(PluginError::PermissionDenied {
            permission: Permission::PreviewRender,
            ..
        })
    ));
}

#[test]
fn invocation_failure_isolated_to_one_extension_and_recoverable() {
    let grants = Arc::new(Mutex::new(Vec::new()));
    let shutdowns = Arc::new(Mutex::new(0));
    let runtime = FakeRuntime {
        plans: VecDeque::from([
            StartPlan::ProcessWithInvocation(
                vec![RuntimeHealth::Running],
                RuntimeError::InvocationTimeout,
            ),
            StartPlan::Process(vec![RuntimeHealth::Running]),
        ]),
        grants,
        shutdowns: shutdowns.clone(),
    };
    let mut host = InMemoryPluginHost::with_runtime(Box::new(runtime));
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    manifest.id = ExtensionId("timed-out.command".into());
    manifest.contributions[0].id = "timed-out.command.run".into();
    manifest.contributions[0].kind = ContributionKind::Command;
    manifest.contributions[0].container = None;
    manifest.contributions[0].zone = None;
    manifest.contributions[0].icon = None;
    manifest.contributions[0].default_visible = None;
    let id = manifest.id.clone();
    host.register(manifest).unwrap();
    host.activate(&id).unwrap();

    let error = host
        .invoke_command(
            &id,
            "timed-out.command.run".into(),
            Value::Null,
            Value::Null,
        )
        .expect_err("a timed-out runtime should reject the invocation");
    assert!(matches!(
        error,
        PluginError::RuntimeTimeout {
            command_id,
            deadline_ms: 5_000,
            ..
        } if command_id == "timed-out.command.run"
    ));
    assert_eq!(host.discover()[0].state, ExtensionState::Failed);
    assert!(host.active_contributions(&id).unwrap().is_empty());

    host.recover(&id)
        .expect("the extension should be restartable");
    host.invoke_command(
        &id,
        "timed-out.command.run".into(),
        Value::Null,
        Value::Null,
    )
    .expect("a recovered runtime should accept the command");
    let diagnostics = host.drain_diagnostics();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.extension_id == id
            && diagnostic.kind == PluginDiagnosticKind::Runtime
            && diagnostic.code == "plugin-runtime-timeout"
            && diagnostic.retryable
    }));
    assert_eq!(*shutdowns.lock().unwrap(), 1);
}

#[test]
fn projections_preserve_declared_ui_metadata_and_namespace() {
    let mut host = InMemoryPluginHost::default();
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    manifest.contributions[0].ui = Some(UiSurfaceDescriptor {
        kind: UiSurfaceKind::View,
        entrypoint: "wasm://example/view".into(),
        bridge_version: "1".into(),
    });
    host.register(manifest).unwrap();
    host.activate(&ExtensionId("example".into())).unwrap();

    let projection = host
        .projection(&ExtensionId("example".into()))
        .expect("active plugin projection should be available");
    assert_eq!(projection.summary.state, ExtensionState::Active);
    assert_eq!(
        projection.manifest.contributions[0].namespace.as_deref(),
        Some("workbench")
    );
    assert_eq!(
        projection.manifest.contributions[0]
            .ui
            .as_ref()
            .expect("view UI metadata should be preserved")
            .entrypoint,
        "wasm://example/view"
    );

    host.disable(&ExtensionId("example".into()), DisableReason::User)
        .unwrap();
    assert_eq!(
        host.projection(&ExtensionId("example".into()))
            .unwrap()
            .summary
            .state,
        ExtensionState::Disabled
    );
}

#[test]
fn app_namespace_requires_a_matching_installed_target() {
    let manifest = ExtensionManifest {
        id: ExtensionId("feed-tools".into()),
        version: "0.1.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: Some(AppTarget {
            app_id: "feed-workbench".into(),
            api: VersionRange("^1".into()),
        }),
        contributions: vec![ContributionDescriptor {
            id: "feed-tools.menu".into(),
            kind: ContributionKind::ContextMenu,
            scope: ContributionScope::WorkbenchApp {
                app_id: "feed-workbench".into(),
            },
            namespace: Some("workbench.app.feed-workbench".into()),
            container: Some(HostContainer::ContextMenu),
            zone: Some(WorkbenchZone::Navigation),
            title: Some("Open feed tool".into()),
            title_key: None,
            icon: Some("command".into()),
            order: None,
            default_visible: Some(true),
            view_container_id: None,
            activity_bar: None,
            label: Some("Open feed tool".into()),
            label_key: None,
            command_id: Some("feed-tools.menu".into()),
            group: None,
            when: None,
            enablement: None,
            keybinding: None,
            keybinding_platform: None,
            menu_location: None,
            required_capabilities: Vec::new(),
            anchor: Some(ContextAnchor::Asset),
            ui: None,
            preview: None,
        }],
        permission_sets: vec![],
        ..ExtensionManifest::default()
    };
    let mut host = InMemoryPluginHost::default();

    host.register(manifest).unwrap();
    host.activate(&ExtensionId("feed-tools".into())).unwrap();
    assert_eq!(
        host.active_contributions(&ExtensionId("feed-tools".into()))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn permission_sets_are_rejected_when_the_zone_does_not_allow_them() {
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    manifest.permission_sets = vec![ZonePermissionSet {
        zone: WorkbenchZone::Navigation,
        permissions: vec![Permission::MetadataWrite],
    }];
    let mut host = InMemoryPluginHost::default();

    assert!(matches!(
        host.register(manifest),
        Err(PluginError::PermissionDenied {
            zone: WorkbenchZone::Navigation,
            permission: Permission::MetadataWrite,
            ..
        })
    ));
}

#[test]
fn install_preview_exposes_permissions_and_rejects_incompatible_hosts() {
    let source = temporary_manifest_dir();
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    manifest.id = ExtensionId("preview.permissions".into());
    manifest.permission_sets = vec![ZonePermissionSet {
        zone: WorkbenchZone::Navigation,
        permissions: vec![Permission::LibraryRead, Permission::SourceConnect],
    }];
    write_package(&source, &manifest);

    let host = InMemoryPluginHost::default();
    let preview = host
        .inspect_from_directory(&source)
        .expect("a valid package should produce an install preview");
    assert_eq!(preview.operation, ExtensionInstallOperation::Install);
    assert!(preview.compatible);
    assert!(preview.can_install);
    assert_eq!(preview.manifest.permission_sets[0].permissions.len(), 2);

    manifest.host_api = VersionRange("^2".into());
    write_package(&source, &manifest);
    let incompatible = host
        .inspect_from_directory(&source)
        .expect("an incompatible package should still produce a review");
    assert!(!incompatible.compatible);
    assert!(!incompatible.can_install);
    assert!(incompatible.issues[0].contains("incompatible"));

    fs::remove_dir_all(source).expect("the preview package should be removed");
}

#[test]
fn external_runtime_start_failure_is_isolated_and_recoverable() {
    let grants = Arc::new(Mutex::new(Vec::new()));
    let shutdowns = Arc::new(Mutex::new(0));
    let runtime = FakeRuntime {
        plans: VecDeque::from([
            StartPlan::Fail("runner unavailable".into()),
            StartPlan::Process(vec![RuntimeHealth::Running]),
        ]),
        grants: grants.clone(),
        shutdowns: shutdowns.clone(),
    };
    let mut host = InMemoryPluginHost::with_runtime(Box::new(runtime));
    let manifest = ExtensionManifest {
        id: ExtensionId("external.reader".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::Sidecar,
        launch: Some(RuntimeLaunchSpec {
            program: "reader-sidecar".into(),
            args: vec!["--stdio".into()],
        }),
        target_app: None,
        contributions: vec![],
        permission_sets: vec![ZonePermissionSet {
            zone: WorkbenchZone::Contextual,
            permissions: vec![Permission::AssetRead, Permission::MetadataRead],
        }],
        ..ExtensionManifest::default()
    };
    let id = manifest.id.clone();
    host.register(manifest).unwrap();

    let error = host
        .activate(&id)
        .expect_err("the first runtime start should fail");
    assert!(matches!(error, PluginError::RuntimeStart { .. }));
    assert_eq!(host.active_contributions(&id).unwrap(), Vec::new());
    assert_eq!(host.discover()[0].state, ExtensionState::Failed);

    host.recover(&id)
        .expect("a failed runtime should be restartable");
    assert_eq!(host.discover()[0].state, ExtensionState::Active);
    assert!(grants.lock().unwrap()[0].contains(&Permission::AssetRead));
    assert!(grants.lock().unwrap()[0].contains(&Permission::MetadataRead));
    host.disable(&id, DisableReason::User).unwrap();
    assert_eq!(*shutdowns.lock().unwrap(), 1);
}

#[test]
fn crashed_runtime_is_removed_from_active_contributions_and_can_restart() {
    let grants = Arc::new(Mutex::new(Vec::new()));
    let shutdowns = Arc::new(Mutex::new(0));
    let runtime = FakeRuntime {
        plans: VecDeque::from([
            StartPlan::Process(vec![RuntimeHealth::Crashed("panic".into())]),
            StartPlan::Process(vec![RuntimeHealth::Running]),
            StartPlan::Process(vec![RuntimeHealth::Running]),
        ]),
        grants,
        shutdowns: shutdowns.clone(),
    };
    let mut host = InMemoryPluginHost::with_runtime(Box::new(runtime));
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    manifest.id = ExtensionId("crashing.navigation".into());
    manifest.runtime = RuntimeKind::Wasm;
    manifest.launch = Some(RuntimeLaunchSpec {
        program: "wasm-runner".into(),
        args: vec!["plugin.wasm".into()],
    });
    let id = manifest.id.clone();
    host.register(manifest).unwrap();
    let mut stable_manifest = view_manifest(Some(WorkbenchZone::Navigation));
    stable_manifest.id = ExtensionId("stable.navigation".into());
    let stable_id = stable_manifest.id.clone();
    host.register(stable_manifest).unwrap();
    host.activate(&id).unwrap();
    host.activate(&stable_id).unwrap();

    let error = host
        .check_health(&id)
        .expect_err("the runtime crash should be reported");
    assert!(matches!(error, PluginError::RuntimeCrashed { .. }));
    assert_eq!(host.discover()[0].state, ExtensionState::Failed);
    assert!(host.active_contributions(&id).unwrap().is_empty());
    assert_eq!(host.active_contributions(&stable_id).unwrap().len(), 1);
    let crash_evidence = host.drain_crash_evidence();
    assert_eq!(crash_evidence.len(), 1);
    assert_eq!(crash_evidence[0].extension_id, id);
    assert_eq!(crash_evidence[0].message, "panic");

    host.recover(&id).unwrap();
    assert_eq!(host.discover()[0].state, ExtensionState::Active);
    host.disable(&id, DisableReason::User).unwrap();
    assert_eq!(*shutdowns.lock().unwrap(), 2);
}

#[cfg(any(target_os = "macos", windows))]
#[test]
fn process_runtime_records_native_start_identity() {
    let token = process_start_token(std::process::id());
    assert!(
        token.is_some(),
        "the platform start identity should be readable"
    );
}

#[test]
fn process_runtime_starts_a_declared_external_program() {
    let mut runtime = ProcessPluginRuntime;
    let manifest = ExtensionManifest {
        id: ExtensionId("process.runtime".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::Sidecar,
        launch: Some(RuntimeLaunchSpec {
            program: std::env::current_exe()
                .expect("the test executable should be discoverable")
                .display()
                .to_string(),
            args: vec!["--help".into()],
        }),
        target_app: None,
        contributions: vec![],
        permission_sets: vec![],
        ..ExtensionManifest::default()
    };

    let mut process = runtime
        .start(&manifest, &PermissionGrant::default())
        .expect("the declared external program should start");
    process
        .shutdown()
        .expect("the external process should be stoppable");
}

#[test]
fn package_launch_policy_rejects_escape_and_unlisted_host_commands() {
    let root = temporary_manifest_dir();
    fs::create_dir_all(&root).expect("the package root should be writable");
    fs::write(root.join("sidecar"), b"#!/bin/sh\n").expect("the sidecar fixture should exist");
    let extension_id = ExtensionId("package.policy".into());
    let context = RuntimeLaunchContext::package_with_allowlist(&root, ["node"]);

    let relative = RuntimeLaunchSpec {
        program: "./sidecar".into(),
        args: Vec::new(),
    };
    let resolved = resolve_launch_program(&extension_id, &relative, &context)
        .expect("a package-relative executable should resolve");
    assert_eq!(resolved.0, fs::canonicalize(root.join("sidecar")).unwrap());

    let unlisted = RuntimeLaunchSpec {
        program: "sh".into(),
        args: Vec::new(),
    };
    let error = resolve_launch_program(&extension_id, &unlisted, &context)
        .expect_err("an unlisted host command should be rejected");
    assert!(format!("{error:?}").contains("allow-list"));

    let escaped = RuntimeLaunchSpec {
        program: "../sidecar".into(),
        args: Vec::new(),
    };
    let error = resolve_launch_program(&extension_id, &escaped, &context)
        .expect_err("a package escape should be rejected");
    assert!(format!("{error:?}").contains("could not be resolved"));

    let absolute = RuntimeLaunchSpec {
        program: std::env::current_exe()
            .expect("the test executable should be discoverable")
            .display()
            .to_string(),
        args: Vec::new(),
    };
    let error = resolve_launch_program(&extension_id, &absolute, &context)
        .expect_err("an absolute executable should be rejected");
    assert!(format!("{error:?}").contains("package-relative"));

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn process_runtime_executes_a_package_relative_sidecar() {
    use std::os::unix::fs::PermissionsExt;

    let root = temporary_manifest_dir();
    fs::create_dir_all(&root).expect("the package root should be writable");
    let executable = root.join("runner.sh");
    fs::write(
            &executable,
            b"#!/bin/sh\nIFS= read -r line\nprintf '%s\\n' '{\"protocolVersion\":1,\"ok\":true,\"result\":{\"relative\":true}}'\n",
        )
        .expect("the relative sidecar should be written");
    let mut permissions = fs::metadata(&executable)
        .expect("the relative sidecar metadata should be available")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions)
        .expect("the relative sidecar should be executable");

    let mut runtime = ProcessPluginRuntime;
    let manifest = ExtensionManifest {
        id: ExtensionId("process.relative-sidecar".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::Sidecar,
        launch: Some(RuntimeLaunchSpec {
            program: "./runner.sh".into(),
            args: Vec::new(),
        }),
        target_app: None,
        contributions: vec![],
        permission_sets: vec![],
        ..ExtensionManifest::default()
    };
    let context = RuntimeLaunchContext::package(&root);
    let mut process = runtime
        .start_with_context(&manifest, &PermissionGrant::default(), &context)
        .expect("the package-relative sidecar should start");
    let result = process
        .invoke(&PluginInvocation {
            extension_id: manifest.id.clone(),
            method: "command.execute".into(),
            command_id: "relative.open".into(),
            args: serde_json::json!({}),
            context: serde_json::json!({}),
            deadline_ms: None,
        })
        .expect("the package-relative sidecar should answer");
    assert_eq!(result.result, Some(serde_json::json!({ "relative": true })));
    process
        .shutdown()
        .expect("the package-relative sidecar should stop");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn process_runtime_invokes_a_versioned_jsonl_command() {
    let mut runtime = ProcessPluginRuntime;
    let manifest = ExtensionManifest {
            id: ExtensionId("process.invocation".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Sidecar,
            launch: Some(RuntimeLaunchSpec {
                program: "sh".into(),
                args: vec![
                    "-c".into(),
                    "IFS= read -r line; if printf '%s' \"$line\" | grep -q 'process.invocation.open'; then printf '%s\\n' '{\"protocolVersion\":1,\"ok\":true,\"result\":{\"accepted\":true}}'; else printf '%s\\n' '{\"protocolVersion\":1,\"ok\":false,\"error\":\"unexpected command\"}'; fi; sleep 5".into(),
                ],
            }),
            target_app: None,
            contributions: vec![],
            permission_sets: vec![],
            ..ExtensionManifest::default()
            };

    let mut process = runtime
        .start(&manifest, &PermissionGrant::default())
        .expect("the declared sidecar should start");
    let result = process
        .invoke(&PluginInvocation {
            extension_id: manifest.id.clone(),
            method: "command.execute".into(),
            command_id: "process.invocation.open".into(),
            args: serde_json::json!({ "source": "test" }),
            context: serde_json::json!({ "activeAppId": null }),
            deadline_ms: None,
        })
        .expect("the sidecar should answer the JSONL invocation");
    assert_eq!(result.error, None);
    assert_eq!(result.result, Some(serde_json::json!({ "accepted": true })));
    process
        .shutdown()
        .expect("the sidecar should be stoppable after invocation");
}

#[test]
fn process_runtime_carries_job_payload_schema_reference() {
    let mut runtime = ProcessPluginRuntime;
    let manifest = ExtensionManifest {
            id: ExtensionId("process.job-schema".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Sidecar,
            launch: Some(RuntimeLaunchSpec {
                program: "sh".into(),
                args: vec![
                    "-c".into(),
                    "IFS= read -r line; if printf '%s' \"$line\" | grep -q '\"kind\":\"invoke-job\"' && printf '%s' \"$line\" | grep -q '\"schemaId\":\"velvet.job.process.job-schema.metadata\"' && printf '%s' \"$line\" | grep -q '\"schemaVersion\":1'; then printf '%s\\n' '{\"protocolVersion\":1,\"ok\":true,\"result\":{\"contentType\":\"application/json\",\"payload\":{\"accepted\":true}}}'; else printf '%s\\n' '{\"protocolVersion\":1,\"ok\":false,\"error\":\"missing job schema reference\"}'; fi; sleep 5".into(),
                ],
            }),
            target_app: None,
            contributions: vec![],
            permission_sets: vec![],
            ..ExtensionManifest::default()
        };
    let mut process = runtime
        .start(&manifest, &PermissionGrant::default())
        .expect("the sidecar should start");
    let result = process
        .invoke(&PluginInvocation {
            extension_id: manifest.id.clone(),
            method: "job.run".into(),
            command_id: "metadata".into(),
            args: serde_json::json!({ "assetId": "asset-1" }),
            context: serde_json::json!({
                "jobId": "job-1",
                "jobKind": "metadata",
                "provenance": {
                    "ownerId": "process.job-schema",
                    "schemaVersion": JOB_SCHEMA_VERSION
                }
            }),
            deadline_ms: None,
        })
        .expect("the sidecar should receive the Job schema reference");
    assert_eq!(result.error, None);
    assert_eq!(
        result.result,
        Some(serde_json::json!({
            "contentType": "application/json",
            "payload": { "accepted": true }
        }))
    );
    process
        .shutdown()
        .expect("the sidecar should be stoppable after invocation");
}

#[test]
fn process_runtime_carries_preview_action_schema_reference() {
    let mut runtime = ProcessPluginRuntime;
    let manifest = ExtensionManifest {
            id: ExtensionId("process.preview-schema".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Sidecar,
            launch: Some(RuntimeLaunchSpec {
                program: "sh".into(),
                args: vec![
                    "-c".into(),
                    "IFS= read -r line; if printf '%s' \"$line\" | grep -q '\"method\":\"preview.action\"' && printf '%s' \"$line\" | grep -q '\"schemaId\":\"velvet.preview.process.preview-schema.preview-open\"' && printf '%s' \"$line\" | grep -q '\"schemaVersion\":1'; then printf '%s\\n' '{\"protocolVersion\":1,\"ok\":true,\"result\":{\"kind\":\"accepted\"}}'; else printf '%s\\n' '{\"protocolVersion\":1,\"ok\":false,\"error\":\"missing preview schema reference\"}'; fi; sleep 5".into(),
                ],
            }),
            target_app: None,
            contributions: vec![],
            permission_sets: vec![],
            ..ExtensionManifest::default()
        };
    let mut process = runtime
        .start(&manifest, &PermissionGrant::default())
        .expect("the sidecar should start");
    let result = process
        .invoke(&PluginInvocation {
            extension_id: manifest.id.clone(),
            method: "preview.action".into(),
            command_id: "preview-open".into(),
            args: serde_json::json!({ "scope": "metadata" }),
            context: serde_json::json!({
                "sessionId": "preview-session",
                "providerId": "preview-provider",
                "assetId": "asset-1",
                "contentKind": "image",
                "intent": "analyze",
                "surface": "workspace"
            }),
            deadline_ms: None,
        })
        .expect("the sidecar should receive the Preview action schema reference");
    assert_eq!(result.error, None);
    assert_eq!(
        result.result,
        Some(serde_json::json!({ "kind": "accepted" }))
    );
    process
        .shutdown()
        .expect("the sidecar should be stoppable after invocation");
}

#[test]
fn process_runtime_rejects_unknown_response_fields() {
    let mut runtime = ProcessPluginRuntime;
    let manifest = ExtensionManifest {
            id: ExtensionId("process.unknown-response-field".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Sidecar,
            launch: Some(RuntimeLaunchSpec {
                program: "sh".into(),
                args: vec![
                    "-c".into(),
                    "IFS= read -r line; printf '%s\\n' '{\"protocolVersion\":1,\"ok\":true,\"result\":null,\"unowned\":true}'".into(),
                ],
            }),
            target_app: None,
            contributions: vec![],
            permission_sets: vec![],
            ..ExtensionManifest::default()
        };
    let mut process = runtime
        .start(&manifest, &PermissionGrant::default())
        .expect("the sidecar should start");
    let error = process
        .invoke(&PluginInvocation {
            extension_id: manifest.id.clone(),
            method: "command.execute".into(),
            command_id: "unknown.response.field".into(),
            args: serde_json::json!({}),
            context: serde_json::json!({}),
            deadline_ms: None,
        })
        .expect_err("unknown runtime response fields must fail closed");
    assert!(matches!(
        error,
        RuntimeError::InvocationFailed(message) if message.contains("unknown field")
    ));
    process
        .shutdown()
        .expect("the malformed sidecar should still be stoppable");
}

#[test]
fn process_runtime_rejects_oversized_request_payloads() {
    let mut runtime = ProcessPluginRuntime;
    let manifest = ExtensionManifest {
        id: ExtensionId("process.oversized-request".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::Sidecar,
        launch: Some(RuntimeLaunchSpec {
            program: "sh".into(),
            args: vec!["-c".into(), "sleep 5".into()],
        }),
        target_app: None,
        contributions: vec![],
        permission_sets: vec![],
        ..ExtensionManifest::default()
    };
    let mut process = runtime
        .start(&manifest, &PermissionGrant::default())
        .expect("the sidecar should start");
    let error = process
        .invoke(&PluginInvocation {
            extension_id: manifest.id.clone(),
            method: "command.execute".into(),
            command_id: "oversized.request".into(),
            args: serde_json::json!("x".repeat(MAX_RUNTIME_REQUEST_BYTES)),
            context: serde_json::json!({}),
            deadline_ms: None,
        })
        .expect_err("oversized runtime requests must fail before IPC");
    assert!(matches!(
        error,
        RuntimeError::InvocationFailed(message)
            if message.contains("runtime invocation request exceeded")
    ));
    process
        .shutdown()
        .expect("the runtime should remain stoppable after a rejected request");
}

#[test]
fn process_runtime_marks_storage_participants_with_the_storage_invocation_kind() {
    let mut runtime = ProcessPluginRuntime;
    let manifest = ExtensionManifest {
            id: ExtensionId("process.storage-invocation".into()),
            version: "1.0.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Sidecar,
            launch: Some(RuntimeLaunchSpec {
                program: "sh".into(),
                args: vec![
                    "-c".into(),
                    "IFS= read -r line; if printf '%s' \"$line\" | grep -q '\"kind\":\"invoke-storage\"' && printf '%s' \"$line\" | grep -q 'storage.migrate'; then printf '%s\\n' '{\"protocolVersion\":1,\"ok\":true,\"result\":{\"schemaVersion\":2,\"entries\":{}}}'; else printf '%s\\n' '{\"protocolVersion\":1,\"ok\":false,\"error\":\"unexpected storage request\"}'; fi; sleep 5".into(),
                ],
            }),
            target_app: None,
            contributions: vec![],
            permission_sets: vec![],
            ..ExtensionManifest::default()
        };
    let mut process = runtime
        .start(&manifest, &PermissionGrant::default())
        .expect("the declared sidecar should start");
    let result = process
        .invoke(&PluginInvocation {
            extension_id: manifest.id.clone(),
            method: STORAGE_MIGRATE_METHOD.into(),
            command_id: "storage".into(),
            args: serde_json::json!({
                "sourceSchemaVersion": 1,
                "targetSchemaVersion": 2,
                "sourceGeneration": 1,
                "entries": {}
            }),
            context: serde_json::json!({
                "capability": "extension-storage",
                "operation": STORAGE_MIGRATE_METHOD,
            }),
            deadline_ms: None,
        })
        .expect("the storage participant should answer the JSONL invocation");
    assert_eq!(result.error, None);
    assert_eq!(
        result.result,
        Some(serde_json::json!({
            "schemaVersion": 2,
            "entries": {}
        }))
    );
    process
        .shutdown()
        .expect("the storage participant should be stoppable after invocation");
}

#[test]
fn process_runtime_reports_bounded_stderr_and_exit_status() {
    let mut runtime = ProcessPluginRuntime;
    let manifest = ExtensionManifest {
        id: ExtensionId("process.failure-evidence".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::Sidecar,
        launch: Some(RuntimeLaunchSpec {
            program: "sh".into(),
            args: vec!["-c".into(), "yes x | head -c 20000 >&2; exit 7".into()],
        }),
        target_app: None,
        contributions: vec![],
        permission_sets: vec![],
        ..ExtensionManifest::default()
    };

    let mut process = runtime
        .start(&manifest, &PermissionGrant::default())
        .expect("the sidecar should start");
    let deadline = Instant::now() + Duration::from_secs(2);
    let health = loop {
        match process.poll() {
            RuntimeHealth::Running if Instant::now() < deadline => std::thread::yield_now(),
            health => break health,
        }
    };

    match health {
        RuntimeHealth::Crashed(message) => {
            assert!(message.contains("exit status: 7"));
            assert!(message.contains("stderr"));
            assert!(message.len() <= MAX_RUNTIME_STDERR_BYTES + 128);
        }
        RuntimeHealth::Running => panic!("the sidecar should have exited"),
    }
    process
        .shutdown()
        .expect("the exited sidecar should be cleanly reaped");
}

#[cfg(unix)]
#[test]
fn process_runtime_shutdown_terminates_sidecar_process_group() {
    let root = temporary_manifest_dir();
    fs::create_dir_all(&root).expect("the process-group fixture should be writable");
    let started = root.join("started");
    let descendant_marker = root.join("descendant-alive");
    let shell_quote = |path: &PathBuf| format!("'{}'", path.display());
    let command = format!(
        "(sleep 1; printf alive > {}) & child=$!; printf started > {}; wait $child",
        shell_quote(&descendant_marker),
        shell_quote(&started),
    );
    let manifest = ExtensionManifest {
        id: ExtensionId("process.group-cleanup".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::Sidecar,
        launch: Some(RuntimeLaunchSpec {
            program: "sh".into(),
            args: vec!["-c".into(), command],
        }),
        target_app: None,
        contributions: vec![],
        permission_sets: vec![],
        ..ExtensionManifest::default()
    };

    let mut runtime = ProcessPluginRuntime;
    let mut process = runtime
        .start(&manifest, &PermissionGrant::default())
        .expect("the sidecar should start");
    let deadline = Instant::now() + Duration::from_secs(2);
    while !started.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(started.exists(), "the descendant should have started");
    assert!(!descendant_marker.exists());
    process
        .shutdown()
        .expect("shutdown should terminate the process group");
    std::thread::sleep(Duration::from_millis(1200));
    assert!(!descendant_marker.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn json_manifest_discovery_isolates_invalid_files() {
    let root = temporary_manifest_dir();
    fs::create_dir_all(&root).expect("the manifest directory should be writable");

    let valid = view_manifest(Some(WorkbenchZone::Navigation));
    fs::write(
        root.join("a-valid.json"),
        serde_json::to_vec_pretty(&valid).expect("the valid manifest should serialize"),
    )
    .expect("the valid manifest should be written");
    fs::write(root.join("b-invalid.json"), b"{ not json")
        .expect("the invalid manifest should be written");

    let mut rejected = view_manifest(Some(WorkbenchZone::Navigation));
    rejected.id = ExtensionId("rejected.permissions".into());
    rejected.permission_sets = vec![ZonePermissionSet {
        zone: WorkbenchZone::Navigation,
        permissions: vec![Permission::Network],
    }];
    fs::write(
        root.join("c-rejected.json"),
        serde_json::to_vec_pretty(&rejected).expect("the rejected manifest should serialize"),
    )
    .expect("the rejected manifest should be written");

    let mut host = InMemoryPluginHost::default();
    let report = JsonManifestDiscovery::new(&root)
        .load_into(&mut host)
        .expect("the manifest directory should be scanned");
    assert_eq!(report.loaded, vec![root.join("a-valid.json")]);
    assert_eq!(report.failures.len(), 2);
    assert!(matches!(
        report.failures[0].error,
        ManifestLoadError::InvalidJson(_)
    ));
    assert!(matches!(
        report.failures[1].error,
        ManifestLoadError::Rejected(PluginError::PermissionDenied { .. })
    ));
    assert_eq!(host.discover().len(), 1);

    fs::remove_dir_all(root).expect("the manifest directory should be removed");
}

#[test]
fn package_discovery_rejects_unlisted_launch_commands_before_activation() {
    let root = temporary_manifest_dir();
    fs::create_dir_all(&root).expect("the package directory should be writable");
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    manifest.id = ExtensionId("rejected.launch-command".into());
    manifest.runtime = RuntimeKind::Sidecar;
    manifest.launch = Some(RuntimeLaunchSpec {
        program: "sh".into(),
        args: Vec::new(),
    });
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("the manifest should serialize"),
    )
    .expect("the manifest should be written");

    let mut host = InMemoryPluginHost::default();
    let report = JsonManifestDiscovery::new(&root)
        .load_into(&mut host)
        .expect("package discovery should complete");
    assert!(report.loaded.is_empty());
    assert_eq!(report.failures.len(), 1);
    match &report.failures[0].error {
        ManifestLoadError::Rejected(PluginError::InvalidContribution(message)) => {
            assert!(message.contains("allow-list"));
        }
        other => panic!("expected an allow-list rejection, got {other:?}"),
    }
    assert!(host.discover().is_empty());
    fs::remove_dir_all(root).expect("the package directory should be removed");
}

#[test]
fn process_host_rejects_unscoped_external_registration() {
    let mut manifest = view_manifest(Some(WorkbenchZone::Navigation));
    manifest.id = ExtensionId("rejected.unscoped-runtime".into());
    manifest.runtime = RuntimeKind::Sidecar;
    manifest.launch = Some(RuntimeLaunchSpec {
        program: std::env::current_exe()
            .expect("the test executable should be discoverable")
            .display()
            .to_string(),
        args: Vec::new(),
    });

    let mut host = InMemoryPluginHost::default();
    let error = host
        .register(manifest)
        .expect_err("external runtimes need a committed package root");
    assert!(matches!(
        error,
        PluginError::InvalidContribution(message)
            if message.contains("committed package")
    ));
}

#[test]
fn bundled_echo_sidecar_runs_the_full_lifecycle() {
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/plugins/echo-sidecar");
    let root = temporary_manifest_dir();
    fs::create_dir_all(&root).expect("the example plugin directory should be writable");
    fs::copy(source.join("manifest.json"), root.join("manifest.json"))
        .expect("the example manifest should be copied");
    fs::copy(source.join("sidecar.mjs"), root.join("sidecar.mjs"))
        .expect("the example sidecar should be copied");

    let mut host = InMemoryPluginHost::default();
    let report = JsonManifestDiscovery::new(&root)
        .load_into(&mut host)
        .expect("the example manifest should be discovered");
    assert_eq!(report.loaded, vec![root.join("manifest.json")]);
    assert!(report.failures.is_empty());

    let id = ExtensionId("velvet.example.echo".into());
    host.activate(&id)
        .expect("the example sidecar should start");
    let projection = host
        .projection(&id)
        .expect("the active example should expose a projection");
    assert!(projection
        .manifest
        .contributions
        .iter()
        .any(|contribution| {
            contribution.id == "echo.view"
                && contribution
                    .ui
                    .as_ref()
                    .is_some_and(|surface| surface.entrypoint.starts_with("data:text/html"))
        }));

    let echoed = host
        .invoke_command(
            &id,
            "echo.open".into(),
            serde_json::json!({ "source": "integration-test" }),
            serde_json::json!({
                "activeAppId": "asset-manager",
                "keys": { "resourceKind": "asset" }
            }),
        )
        .expect("the example command should reach the sidecar");
    assert_eq!(
        echoed
            .result
            .as_ref()
            .and_then(|result| result.get("commandId")),
        Some(&serde_json::json!("echo.open"))
    );

    let job_handle = host
        .job_handles(&id)
        .expect("the active sidecar should expose its Job handle")
        .into_iter()
        .find(|handle| handle.job_kind() == "echo.metadata-analysis")
        .expect("the declared analysis Job should be registered");
    let job = job_handle
        .invoke(PluginJobInvocation {
            job_id: "job-echo-000001".into(),
            job_kind: "echo.metadata-analysis".into(),
            attempt: 1,
            input: serde_json::json!({ "scope": "metadata" }),
            provenance: JobProvenance {
                input_fingerprint: Some("sha256:echo".into()),
                ..JobProvenance::for_owner("velvet.example.echo")
            },
            budget: JobBudgetRequest::default(),
            deadline_ms: None,
            cancellation_grace_ms: default_job_cancellation_grace_ms(),
            checkpoint: None,
        })
        .expect("the sidecar Job should reach the runtime");
    assert_eq!(job.events[0].progress, Some(50));
    assert_eq!(job.result.payload["jobKind"], "echo.metadata-analysis");

    let crash = host
        .invoke_command(&id, "echo.crash".into(), Value::Null, Value::Null)
        .expect_err("the crash command should fail only its extension");
    assert!(matches!(crash, PluginError::RuntimeInvocation { .. }));
    assert_eq!(host.discover()[0].state, ExtensionState::Failed);
    assert!(host.active_contributions(&id).unwrap().is_empty());

    host.recover(&id)
        .expect("the crashed example should be recoverable");
    assert_eq!(host.discover()[0].state, ExtensionState::Active);
    host.invoke_command(&id, "echo.open".into(), Value::Null, Value::Null)
        .expect("the recovered sidecar should accept commands");
    host.disable(&id, DisableReason::User)
        .expect("the example should be stoppable");

    fs::remove_dir_all(root).expect("the temporary example directory should be removed");
}

#[test]
fn unsigned_packages_are_developer_only_and_signed_packages_record_trust() {
    let source = temporary_manifest_dir();
    let install_root = temporary_manifest_dir();
    let manifest = view_manifest(Some(WorkbenchZone::Navigation));
    write_package(&source, &manifest);

    let mut developer_host = InMemoryPluginHost::default();
    developer_host
        .install_from_directory(&source, &install_root)
        .expect("developer mode should admit an unsigned local package");
    assert_eq!(
        developer_host
            .plugin_trust_record(&manifest.id)
            .unwrap()
            .state,
        PluginTrustState::DeveloperUnsigned
    );

    let mut release_host = InMemoryPluginHost::default();
    release_host.set_plugin_trust_policy(PluginTrustPolicy::require_signed());
    let preview = release_host
        .inspect_from_directory(&source)
        .expect("trust failures should be reviewable before copy");
    assert!(!preview.can_install);
    assert!(preview
        .issues
        .iter()
        .any(|issue| issue.contains("unsigned package")));
    assert!(matches!(
        release_host.install_from_directory(&source, &temporary_manifest_dir()),
        Err(PluginError::TrustDenied {
            state: PluginTrustState::Unknown,
            ..
        })
    ));

    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    write_signed_package(&source, &manifest, "publisher.test", &signing_key, 1);
    let policy = PluginTrustPolicy::require_signed().trust_publisher(
        "publisher.test",
        encode_hex(signing_key.verifying_key().as_bytes()),
    );
    release_host.set_plugin_trust_policy(policy);
    let signed_install_root = temporary_manifest_dir();
    release_host
        .install_from_directory(&source, &signed_install_root)
        .expect("a trusted signed package should install");
    let record = release_host
        .plugin_trust_record(&manifest.id)
        .expect("the signed package should have a trust record");
    assert_eq!(record.state, PluginTrustState::Trusted);
    assert_eq!(record.generation, 1);

    fs::write(source.join("tampered.txt"), b"changed")
        .expect("the package should be mutable for the tamper fixture");
    let mut tampered_host = InMemoryPluginHost::default();
    tampered_host.set_plugin_trust_policy(PluginTrustPolicy::require_signed().trust_publisher(
        "publisher.test",
        encode_hex(signing_key.verifying_key().as_bytes()),
    ));
    assert!(matches!(
        tampered_host.install_from_directory(&source, &temporary_manifest_dir()),
        Err(PluginError::TrustDenied {
            state: PluginTrustState::Incompatible,
            ..
        })
    ));

    let _ = fs::remove_dir_all(source);
    let _ = fs::remove_dir_all(install_root);
    let _ = fs::remove_dir_all(signed_install_root);
}

#[test]
fn signed_release_policy_cannot_be_overridden_by_developer_mode() {
    let mut host = InMemoryPluginHost::default();
    host.set_plugin_trust_policy(PluginTrustPolicy::require_signed());
    assert!(matches!(
        host.set_plugin_developer_mode(true),
        Err(PluginError::TrustPolicy(_))
    ));
}

#[test]
fn revoked_or_downgraded_signed_packages_stay_inactive() {
    let source = temporary_manifest_dir();
    let install_root = temporary_manifest_dir();
    let manifest = view_manifest(Some(WorkbenchZone::Navigation));
    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    write_signed_package(&source, &manifest, "publisher.revoked", &signing_key, 2);
    let public_key = encode_hex(signing_key.verifying_key().as_bytes());

    let mut revoked_host = InMemoryPluginHost::default();
    revoked_host.set_plugin_trust_policy(
        PluginTrustPolicy::require_signed()
            .trust_publisher("publisher.revoked", public_key.clone())
            .revoke_publisher("publisher.revoked"),
    );
    assert!(matches!(
        revoked_host.install_from_directory(&source, &install_root),
        Err(PluginError::TrustDenied {
            state: PluginTrustState::Revoked,
            ..
        })
    ));

    let mut downgraded_host = InMemoryPluginHost::default();
    downgraded_host.set_plugin_trust_policy(
        PluginTrustPolicy::require_signed()
            .trust_publisher("publisher.revoked", public_key)
            .require_minimum_generation(manifest.id.clone(), 3),
    );
    assert!(matches!(
        downgraded_host.install_from_directory(&source, &temporary_manifest_dir()),
        Err(PluginError::TrustDenied {
            state: PluginTrustState::Blocked,
            ..
        })
    ));

    let _ = fs::remove_dir_all(source);
    let _ = fs::remove_dir_all(install_root);
}

#[test]
fn publisher_key_rotation_requires_overlap_signatures_and_preserves_old_key_overlap() {
    let source = temporary_manifest_dir();
    let install_root = temporary_manifest_dir();
    let manifest = view_manifest(Some(WorkbenchZone::Navigation));
    let old_key = SigningKey::from_bytes(&[21u8; 32]);
    let new_key = SigningKey::from_bytes(&[22u8; 32]);
    write_signed_package(&source, &manifest, "publisher.next", &new_key, 4);

    let mut policy = PluginTrustPolicy::require_signed().trust_publisher(
        "publisher.current",
        encode_hex(old_key.verifying_key().as_bytes()),
    );
    policy
        .register_publisher_key_rotation(signed_key_rotation(
            "publisher.current",
            &old_key,
            "publisher.next",
            &new_key,
            1,
        ))
        .expect("the overlap-signed successor should be accepted");

    let mut host = InMemoryPluginHost::default();
    host.set_plugin_trust_policy(policy.clone());
    host.install_from_directory(&source, &install_root)
        .expect("the successor package should be trusted during overlap");
    assert_eq!(
        host.plugin_trust_record(&manifest.id)
            .expect("the successor package should record trust")
            .publisher_key_id,
        Some("publisher.next".into())
    );

    let old_source = temporary_manifest_dir();
    let old_install_root = temporary_manifest_dir();
    write_signed_package(&old_source, &manifest, "publisher.current", &old_key, 3);
    let mut overlap_host = InMemoryPluginHost::default();
    overlap_host.set_plugin_trust_policy(policy);
    overlap_host
        .install_from_directory(&old_source, &old_install_root)
        .expect("the previous key should remain valid during overlap");

    let _ = fs::remove_dir_all(source);
    let _ = fs::remove_dir_all(install_root);
    let _ = fs::remove_dir_all(old_source);
    let _ = fs::remove_dir_all(old_install_root);
}

#[test]
fn publisher_key_rotation_rejects_forgery_replay_and_revoked_successors() {
    let old_key = SigningKey::from_bytes(&[23u8; 32]);
    let new_key = SigningKey::from_bytes(&[24u8; 32]);
    let unrelated_key = SigningKey::from_bytes(&[25u8; 32]);
    let mut policy = PluginTrustPolicy::require_signed().trust_publisher(
        "publisher.current",
        encode_hex(old_key.verifying_key().as_bytes()),
    );

    let mut forged =
        signed_key_rotation("publisher.current", &old_key, "publisher.next", &new_key, 1);
    forged.new_signature = encode_hex(&unrelated_key.sign(&forged.canonical_payload()).to_bytes());
    assert!(matches!(
        policy.register_publisher_key_rotation(forged),
        Err(PluginError::TrustPolicy(message)) if message.contains("successor key")
    ));

    let rotation =
        signed_key_rotation("publisher.current", &old_key, "publisher.next", &new_key, 1);
    policy
        .register_publisher_key_rotation(rotation.clone())
        .expect("the first rotation should be accepted");
    policy
        .register_publisher_key_rotation(rotation)
        .expect("replaying the exact rotation should be idempotent");

    let mut conflicting = signed_key_rotation(
        "publisher.current",
        &old_key,
        "publisher.next",
        &unrelated_key,
        2,
    );
    conflicting.old_signature =
        encode_hex(&old_key.sign(&conflicting.canonical_payload()).to_bytes());
    conflicting.new_signature = encode_hex(
        &unrelated_key
            .sign(&conflicting.canonical_payload())
            .to_bytes(),
    );
    assert!(matches!(
        policy.register_publisher_key_rotation(conflicting),
        Err(PluginError::TrustPolicy(message)) if message.contains("different rotation")
    ));

    let mut stale = signed_key_rotation(
        "publisher.current",
        &old_key,
        "publisher.stale",
        &unrelated_key,
        1,
    );
    stale.old_signature = encode_hex(&old_key.sign(&stale.canonical_payload()).to_bytes());
    stale.new_signature = encode_hex(&unrelated_key.sign(&stale.canonical_payload()).to_bytes());
    assert!(matches!(
        policy.register_publisher_key_rotation(stale),
        Err(PluginError::TrustPolicy(message)) if message.contains("not newer")
    ));

    let mut revoked_policy = policy.revoke_publisher("publisher.next");
    let revoked_source = temporary_manifest_dir();
    let revoked_install_root = temporary_manifest_dir();
    let revoked_manifest = view_manifest(Some(WorkbenchZone::Navigation));
    write_signed_package(
        &revoked_source,
        &revoked_manifest,
        "publisher.next",
        &new_key,
        5,
    );
    let mut revoked_host = InMemoryPluginHost::default();
    revoked_host.set_plugin_trust_policy(revoked_policy.clone());
    assert!(matches!(
        revoked_host.install_from_directory(&revoked_source, &revoked_install_root),
        Err(PluginError::TrustDenied {
            state: PluginTrustState::Revoked,
            ..
        })
    ));
    revoked_policy
        .register_publisher_key_rotation(signed_key_rotation(
            "publisher.current",
            &old_key,
            "publisher.third",
            &unrelated_key,
            2,
        ))
        .expect("the still-active old key should authorize a later successor");
    assert!(revoked_policy
        .trusted_publishers
        .contains_key("publisher.third"));

    let _ = fs::remove_dir_all(revoked_source);
    let _ = fs::remove_dir_all(revoked_install_root);
}

#[test]
fn publisher_key_rotation_is_versioned_and_bounded() {
    let old_key = SigningKey::from_bytes(&[26u8; 32]);
    let new_key = SigningKey::from_bytes(&[27u8; 32]);
    let mut policy = PluginTrustPolicy::require_signed().trust_publisher(
        "publisher.current",
        encode_hex(old_key.verifying_key().as_bytes()),
    );

    let mut unsupported =
        signed_key_rotation("publisher.current", &old_key, "publisher.next", &new_key, 1);
    unsupported.schema_version += 1;
    assert!(matches!(
        policy.register_publisher_key_rotation(unsupported),
        Err(PluginError::TrustPolicy(message)) if message.contains("unsupported")
    ));

    let mut oversized =
        signed_key_rotation("publisher.current", &old_key, "publisher.next", &new_key, 1);
    oversized.new_key_id = "oversized-".repeat(MAX_PLUGIN_KEY_ROTATION_BYTES);
    assert!(matches!(
        policy.register_publisher_key_rotation(oversized),
        Err(PluginError::TrustPolicy(message)) if message.contains("exceeds")
    ));
}

#[test]
fn plugin_trust_records_survive_restart_and_uninstall() {
    let source = temporary_manifest_dir();
    let install_root = temporary_manifest_dir();
    let trust_root = temporary_manifest_dir();
    let trust_path = trust_root.join("plugin-trust.json");
    let manifest = view_manifest(Some(WorkbenchZone::Navigation));
    let signing_key = SigningKey::from_bytes(&[11u8; 32]);
    write_signed_package(&source, &manifest, "publisher.persisted", &signing_key, 3);
    let policy = PluginTrustPolicy::require_signed().trust_publisher(
        "publisher.persisted",
        encode_hex(signing_key.verifying_key().as_bytes()),
    );

    let store = Arc::new(JsonFilePluginTrustStore::new(&trust_path));
    let mut first_host = InMemoryPluginHost::default();
    first_host.set_plugin_trust_policy(policy.clone());
    first_host
        .set_plugin_trust_store(store.clone())
        .expect("an empty trust store should load");
    first_host
        .install_from_directory(&source, &install_root)
        .expect("the signed package should install");
    first_host
        .uninstall_from_directory(&manifest.id, &install_root)
        .expect("uninstall should remove only the package");
    assert_eq!(
        first_host
            .plugin_trust_record(&manifest.id)
            .expect("uninstall should retain trust evidence")
            .state,
        PluginTrustState::Trusted
    );

    let mut restarted_host = InMemoryPluginHost::default();
    restarted_host.set_plugin_trust_policy(policy);
    restarted_host
        .set_plugin_trust_store(store)
        .expect("the persisted trust record should load after restart");
    assert_eq!(
        restarted_host
            .plugin_trust_record(&manifest.id)
            .expect("the restarted host should see trust evidence")
            .generation,
        3
    );

    let _ = fs::remove_dir_all(source);
    let _ = fs::remove_dir_all(install_root);
    let _ = fs::remove_dir_all(trust_root);
}

#[test]
fn trust_store_write_failure_does_not_publish_registration() {
    struct FailingStore;

    impl PluginTrustStore for FailingStore {
        fn load(&self) -> Result<Vec<PluginTrustRecord>, PluginTrustStoreError> {
            Ok(Vec::new())
        }

        fn save(&self, _records: &[PluginTrustRecord]) -> Result<(), PluginTrustStoreError> {
            Err(PluginTrustStoreError::Unavailable(
                "test store is read-only".into(),
            ))
        }
    }

    let source = temporary_manifest_dir();
    let install_root = temporary_manifest_dir();
    let manifest = view_manifest(Some(WorkbenchZone::Navigation));
    let signing_key = SigningKey::from_bytes(&[13u8; 32]);
    write_signed_package(&source, &manifest, "publisher.read-only", &signing_key, 1);
    let mut host = InMemoryPluginHost::default();
    host.set_plugin_trust_policy(PluginTrustPolicy::require_signed().trust_publisher(
        "publisher.read-only",
        encode_hex(signing_key.verifying_key().as_bytes()),
    ));
    host.set_plugin_trust_store(Arc::new(FailingStore))
        .expect("the empty read-only store should load");

    assert!(matches!(
        host.install_from_directory(&source, &install_root),
        Err(PluginError::TrustStore(_))
    ));
    assert!(host.discover().is_empty());

    let _ = fs::remove_dir_all(source);
    let _ = fs::remove_dir_all(install_root);
}

#[test]
fn corrupt_plugin_trust_store_fails_closed_without_blocking_host() {
    let root = temporary_manifest_dir();
    fs::create_dir_all(&root).expect("the trust store directory should be writable");
    let path = root.join("plugin-trust.json");
    fs::write(&path, b"{\"schemaVersion\":999,\"records\":[]}")
        .expect("the corrupt trust store should be writable");
    let mut host = InMemoryPluginHost::default();
    let error = host
        .set_plugin_trust_store(Arc::new(JsonFilePluginTrustStore::new(&path)))
        .expect_err("an unsupported trust store must fail closed");
    assert!(matches!(error, PluginError::TrustStore(_)));
    assert!(host.discover().is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bundled_echo_sidecar_installs_and_is_rediscovered_after_restart() {
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/plugins/echo-sidecar");
    let root = temporary_manifest_dir();
    let mut host = InMemoryPluginHost::default();
    let installed = host
        .install_from_directory(&source, &root)
        .expect("the example package should install");

    assert_eq!(installed.id.0, "velvet.example.echo");
    assert_eq!(installed.state, ExtensionState::Registered);
    assert!(root.join("velvet.example.echo/manifest.json").is_file());
    assert!(root.join("velvet.example.echo/sidecar.mjs").is_file());

    drop(host);
    let mut restarted = InMemoryPluginHost::default();
    let report = JsonManifestDiscovery::new(&root)
        .load_into(&mut restarted)
        .expect("the installed package should be found on restart");
    assert_eq!(report.failures, Vec::new());
    assert_eq!(report.loaded.len(), 1);
    assert_eq!(restarted.discover()[0].state, ExtensionState::Registered);
    restarted
        .activate(&ExtensionId("velvet.example.echo".into()))
        .expect("the first discovered startup should activate the sidecar");
    restarted
        .invoke_command(
            &ExtensionId("velvet.example.echo".into()),
            "echo.open".into(),
            Value::Null,
            Value::Null,
        )
        .expect("the first started sidecar should accept a command");

    restarted
        .uninstall_from_directory(&ExtensionId("velvet.example.echo".into()), &root)
        .expect("the installed sidecar should be uninstallable");
    assert!(restarted.discover().is_empty());
    assert!(restarted.registry_snapshot().contributions.is_empty());
    assert!(!root.join("velvet.example.echo").exists());

    fs::remove_dir_all(root).expect("the temporary installation should be removed");
}

#[test]
fn plugin_upgrade_can_be_previewed_committed_or_rolled_back() {
    let root = temporary_manifest_dir();
    let first_source = temporary_manifest_dir();
    let upgrade_source = temporary_manifest_dir();
    let mut first = view_manifest(Some(WorkbenchZone::Navigation));
    first.id = ExtensionId("transactional.plugin".into());
    first.version = "0.1.0".into();
    first.contributions[0].id = "transactional.plugin.view".into();
    write_package(&first_source, &first);
    let mut host = InMemoryPluginHost::default();
    host.install_from_directory(&first_source, &root)
        .expect("the initial package should install");
    host.commit_install(&first.id, &root)
        .expect("the initial package should be committed");
    host.activate(&first.id)
        .expect("the initial package should activate");
    assert_eq!(host.registry_snapshot().contributions.len(), 1);

    let conflict = host
        .inspect_from_directory(&first_source)
        .expect("an installed package should still produce a version review");
    assert_eq!(conflict.operation, ExtensionInstallOperation::Upgrade);
    assert!(!conflict.can_install);
    assert!(conflict.issues[0].contains("must be newer"));

    first.version = "0.2.0".into();
    write_package(&upgrade_source, &first);
    let preview = host
        .inspect_from_directory(&upgrade_source)
        .expect("the newer package should produce an upgrade preview");
    assert_eq!(preview.operation, ExtensionInstallOperation::Upgrade);
    assert!(preview.can_install);
    assert_eq!(
        preview
            .existing
            .expect("the installed package should be shown")
            .version,
        "0.1.0"
    );

    host.install_from_directory(&upgrade_source, &root)
        .expect("the upgrade should be staged");
    assert_eq!(host.projection(&first.id).unwrap().summary.version, "0.2.0");
    assert!(host.registry_snapshot().contributions.is_empty());
    assert!(
        root.join(".transactional.plugin.rollback").exists()
            || root
                .read_dir()
                .expect("the install root should be readable")
                .any(|entry| entry
                    .expect("the rollback entry should be readable")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".transactional.plugin.rollback-"))
    );

    host.rollback_install(&first.id, &root)
        .expect("the staged upgrade should be reversible");
    let restored = host
        .projection(&first.id)
        .expect("the previous package should be restored");
    assert_eq!(restored.summary.version, "0.1.0");
    assert_eq!(restored.summary.state, ExtensionState::Active);
    assert_eq!(host.registry_snapshot().contributions.len(), 1);

    host.install_from_directory(&upgrade_source, &root)
        .expect("the upgrade should be stageable again");
    host.commit_install(&first.id, &root)
        .expect("the upgrade should be committed");
    assert_eq!(host.projection(&first.id).unwrap().summary.version, "0.2.0");
    assert!(host.registry_snapshot().contributions.is_empty());
    assert!(!root
        .read_dir()
        .expect("the install root should be readable")
        .any(|entry| entry
            .expect("the rollback entry should be readable")
            .file_name()
            .to_string_lossy()
            .starts_with(".transactional.plugin.rollback-")));

    fs::remove_dir_all(root).expect("the transactional installation should be removed");
    fs::remove_dir_all(first_source).expect("the initial package should be removed");
    fs::remove_dir_all(upgrade_source).expect("the upgrade package should be removed");
}

#[test]
fn interrupted_install_recovery_is_discoverable_and_resolvable_after_restart() {
    let root = temporary_manifest_dir();
    let id = ExtensionId("recoverable.plugin".into());
    let previous = ExtensionManifest {
        id: id.clone(),
        version: "0.1.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: None,
        contributions: Vec::new(),
        permission_sets: Vec::new(),
        ..ExtensionManifest::default()
    };
    let mut current = previous.clone();
    current.version = "0.2.0".into();
    fs::create_dir_all(root.join(&id.0)).expect("the current package should be writable");
    write_package(&root.join(&id.0), &current);
    let rollback = root.join(format!(".{}.rollback-crash", id.0));
    write_package(&rollback, &previous);

    let mut restarted = InMemoryPluginHost::default();
    JsonManifestDiscovery::new(&root)
        .load_into(&mut restarted)
        .expect("the current package should be discovered");
    let recoveries = restarted
        .inspect_install_recoveries(&root)
        .expect("the interrupted upgrade should be discoverable");
    assert_eq!(recoveries.len(), 1);
    assert_eq!(recoveries[0].kind, ExtensionInstallRecoveryKind::Rollback);
    assert_eq!(recoveries[0].candidate_version.as_deref(), Some("0.2.0"));
    assert_eq!(recoveries[0].rollback_version.as_deref(), Some("0.1.0"));
    assert!(recoveries[0].can_commit);
    assert!(recoveries[0].can_rollback);

    restarted
        .resolve_install_recovery(&id, ExtensionInstallRecoveryAction::Rollback, &root)
        .expect("the previous package should be restorable after restart");
    assert_eq!(restarted.discover()[0].version, "0.1.0");
    assert_eq!(
        restarted.inspect_install_recoveries(&root).unwrap(),
        Vec::new()
    );
    assert_eq!(
        serde_json::from_slice::<ExtensionManifest>(
            &fs::read(root.join(&id.0).join("manifest.json")).unwrap(),
        )
        .unwrap()
        .version,
        "0.1.0"
    );

    let staged_id = ExtensionId("staged.plugin".into());
    let staging = root.join(format!(".{}.staging-crash", staged_id.0));
    let staged = ExtensionManifest {
        id: staged_id.clone(),
        version: "0.1.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: None,
        contributions: Vec::new(),
        permission_sets: Vec::new(),
        ..ExtensionManifest::default()
    };
    write_package(&staging, &staged);
    let staged_recovery = restarted
        .inspect_install_recoveries(&root)
        .expect("the staged package should be discoverable")
        .into_iter()
        .find(|recovery| recovery.extension_id == staged_id)
        .expect("the staged recovery should be listed");
    assert!(staged_recovery.can_resume);
    restarted
        .resolve_install_recovery(&staged_id, ExtensionInstallRecoveryAction::Resume, &root)
        .expect("the staged package should resume after restart");
    assert_eq!(restarted.discover().len(), 2);
    assert!(root.join(&staged_id.0).is_dir());

    fs::remove_dir_all(root).expect("the recovery fixture should be removed");
}

#[test]
fn recovery_action_failure_is_logged_and_retryable() {
    let root = temporary_manifest_dir();
    let id = ExtensionId("retryable.recovery".into());
    let previous = ExtensionManifest {
        id: id.clone(),
        version: "0.1.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: None,
        contributions: Vec::new(),
        permission_sets: Vec::new(),
        ..ExtensionManifest::default()
    };
    let mut current = previous.clone();
    current.version = "0.2.0".into();
    write_package(&root.join(&id.0), &current);
    write_package(&root.join(format!(".{}.rollback-crash", id.0)), &previous);

    let mut host = InMemoryPluginHost::default();
    JsonManifestDiscovery::new(&root)
        .load_into(&mut host)
        .expect("the current package should be discovered");
    let recovery_current = root.join(format!(".{}.recovery-current-{}", id.0, std::process::id()));
    fs::write(&recovery_current, b"temporary blocker")
        .expect("the retry blocker should be writable");
    host.resolve_install_recovery(&id, ExtensionInstallRecoveryAction::Rollback, &root)
        .expect_err("the occupied recovery path should make the first attempt fail");

    let failed_log = host
        .inspect_install_recovery_log(&root)
        .expect("the failed recovery should be logged");
    assert_eq!(failed_log.len(), 1);
    assert_eq!(failed_log[0].attempt, 1);
    assert_eq!(
        failed_log[0].status,
        ExtensionInstallRecoveryLogStatus::Failed
    );

    fs::remove_file(&recovery_current).expect("the temporary blocker should be removable");
    host.resolve_install_recovery(&id, ExtensionInstallRecoveryAction::Rollback, &root)
        .expect("the same recovery action should be retryable");
    let log = host
        .inspect_install_recovery_log(&root)
        .expect("the recovery log should be readable");
    assert_eq!(log.len(), 2);
    assert_eq!(log[1].attempt, 2);
    assert_eq!(log[1].status, ExtensionInstallRecoveryLogStatus::Succeeded);
    assert_eq!(host.discover()[0].version, "0.1.0");

    let export_path = root.with_extension("recovery-log.json");
    host.export_install_recovery_log(&root, &export_path)
        .expect("the recovery log should export");
    let exported: Vec<ExtensionInstallRecoveryLogEntry> = serde_json::from_slice(
        &fs::read(&export_path).expect("the exported recovery log should be readable"),
    )
    .expect("the exported recovery log should be valid JSON");
    assert_eq!(exported.len(), 2);
    host.clear_install_recovery_log(&root)
        .expect("the recovery log should clear");
    assert!(host
        .inspect_install_recovery_log(&root)
        .expect("the cleared recovery log should remain readable")
        .is_empty());
    assert!(!recovery_log_path(&root).exists());

    fs::remove_dir_all(root).expect("the retry fixture should be removed");
    fs::remove_file(export_path).expect("the exported recovery log should be removed");
}

#[test]
fn orphan_recovery_directories_are_cleaned_and_logged() {
    let root = temporary_manifest_dir();
    let orphan_staging = root.join(".orphan.plugin.staging-empty");
    let orphan_rollback = root.join(".orphan.plugin.rollback-empty");
    fs::create_dir_all(&orphan_staging).expect("the orphan staging directory should exist");
    fs::create_dir_all(&orphan_rollback).expect("the orphan rollback directory should exist");

    let stale_id = ExtensionId("stale.plugin".into());
    let stale = ExtensionManifest {
        id: stale_id.clone(),
        version: "0.1.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: None,
        contributions: Vec::new(),
        permission_sets: Vec::new(),
        ..ExtensionManifest::default()
    };
    write_package(&root.join(&stale_id.0), &stale);
    write_package(&root.join(format!(".{}.staging-stale", stale_id.0)), &stale);

    let missing_id = ExtensionId("missing.plugin".into());
    write_package(
        &root.join(format!(".{}.rollback-stale", missing_id.0)),
        &ExtensionManifest {
            id: missing_id,
            ..stale.clone()
        },
    );

    let mut host = InMemoryPluginHost::default();
    JsonManifestDiscovery::new(&root)
        .load_into(&mut host)
        .expect("the stale package should be discovered");
    host.cleanup_install_recovery_orphans(&root)
        .expect("orphan recovery directories should be cleaned");

    assert!(!orphan_staging.exists());
    assert!(!orphan_rollback.exists());
    assert!(!root.join(format!(".{}.staging-stale", stale_id.0)).exists());
    assert!(!root
        .join(format!(".{}.rollback-stale", "missing.plugin"))
        .exists());
    assert!(root.join(&stale_id.0).exists());
    let log = host
        .inspect_install_recovery_log(&root)
        .expect("orphan cleanup should be logged");
    assert_eq!(log.len(), 4);
    assert!(log.iter().all(|entry| {
        entry.action == ExtensionInstallRecoveryAction::Discard
            && entry.status == ExtensionInstallRecoveryLogStatus::Succeeded
            && entry.removed_paths.len() == 1
    }));

    fs::remove_dir_all(root).expect("the orphan fixture should be removed");
}

#[test]
fn runtime_lease_is_published_during_activation_and_closed_after_disable() {
    let package_root = temporary_manifest_dir();
    fs::create_dir_all(&package_root).expect("the package root should exist");
    let manifest = ExtensionManifest {
        id: ExtensionId("lease.plugin".into()),
        runtime: RuntimeKind::Sidecar,
        launch: Some(RuntimeLaunchSpec {
            program: "node".into(),
            args: Vec::new(),
        }),
        ..ExtensionManifest::default()
    };
    let shutdowns = Arc::new(Mutex::new(0));
    let mut host = InMemoryPluginHost::with_runtime(Box::new(LeaseRuntime {
        shutdowns: shutdowns.clone(),
    }));
    let leases = Arc::new(InMemoryRuntimeLeaseStore::default());
    host.set_runtime_lease_store(leases.clone());
    host.register_from_package(manifest.clone(), &package_root)
        .expect("the package should register");
    host.activate(&manifest.id)
        .expect("the runtime should activate");

    let active_leases = leases.leases().expect("the lease store should be readable");
    assert_eq!(active_leases.len(), 1);
    assert_eq!(active_leases[0].extension_id, manifest.id);
    assert_eq!(active_leases[0].state, RuntimeLeaseState::Running);
    assert_eq!(active_leases[0].process.as_ref().unwrap().pid, 41_001);

    host.disable(&manifest.id, DisableReason::User)
        .expect("the runtime should disable");
    assert!(leases
        .leases()
        .expect("the lease store should be readable")
        .is_empty());
    assert_eq!(*shutdowns.lock().unwrap(), 1);
    fs::remove_dir_all(package_root).expect("the package root should be removed");
}

struct MatchingOrphanReconciler {
    terminations: Arc<Mutex<usize>>,
}

impl RuntimeOrphanReconciler for MatchingOrphanReconciler {
    fn observe(&self, _lease: &RuntimeLease) -> RuntimeOrphanObservation {
        RuntimeOrphanObservation::Matching
    }

    fn terminate(&self, _lease: &RuntimeLease) -> Result<(), RuntimeLeaseError> {
        *self
            .terminations
            .lock()
            .expect("termination counter should not be poisoned") += 1;
        Ok(())
    }
}

#[test]
fn confirmed_orphan_is_terminated_and_quarantined_state_stays_recoverable() {
    let id = ExtensionId("orphan.plugin".into());
    let manifest = ExtensionManifest {
        id: id.clone(),
        ..ExtensionManifest::default()
    };
    let store = Arc::new(InMemoryRuntimeLeaseStore::default());
    store
        .publish(
            RuntimeLease::starting(
                id.clone(),
                manifest.version.clone(),
                None,
                "orphan-nonce".into(),
            )
            .running(RuntimeProcessIdentity {
                pid: 42_001,
                process_group_id: Some(42_001),
                start_token: Some("orphan-token".into()),
            }),
        )
        .expect("the orphan lease should publish");
    let terminations = Arc::new(Mutex::new(0));
    let mut host = InMemoryPluginHost::default();
    host.set_runtime_lease_store(store.clone());
    host.set_runtime_orphan_reconciler(Arc::new(MatchingOrphanReconciler {
        terminations: terminations.clone(),
    }));
    host.register(manifest)
        .expect("the extension should register");

    let reports = host
        .reconcile_runtime_orphans()
        .expect("orphan reconciliation should complete");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].observation, RuntimeOrphanObservation::Matching);
    assert!(reports[0].lease_closed);
    assert!(!reports[0].quarantined);
    assert_eq!(*terminations.lock().unwrap(), 1);
    assert!(store
        .leases()
        .expect("the lease store should be readable")
        .is_empty());
    assert_eq!(host.discover()[0].state, ExtensionState::Failed);
    assert!(host
        .drain_diagnostics()
        .iter()
        .any(|record| record.code == "runtime-orphan-terminated"));
}

#[test]
fn uncertain_orphan_is_quarantined_without_termination() {
    let id = ExtensionId("uncertain.plugin".into());
    let store = Arc::new(InMemoryRuntimeLeaseStore::default());
    store
        .publish(RuntimeLease::starting(
            id.clone(),
            "0.1.0".into(),
            None,
            "uncertain-nonce".into(),
        ))
        .expect("the uncertain lease should publish");
    let mut host = InMemoryPluginHost::default();
    host.set_runtime_lease_store(store.clone());
    host.set_runtime_orphan_reconciler(Arc::new(ConservativeRuntimeOrphanReconciler));
    host.register(ExtensionManifest {
        id: id.clone(),
        ..ExtensionManifest::default()
    })
    .expect("the quarantined extension should remain projected");

    let reports = host
        .reconcile_runtime_orphans()
        .expect("uncertain reconciliation should complete");
    assert_eq!(
        reports[0].observation,
        RuntimeOrphanObservation::IdentityUncertain
    );
    assert!(!reports[0].lease_closed);
    assert!(reports[0].quarantined);
    assert_eq!(
        store
            .leases()
            .expect("the lease store should be readable")
            .len(),
        1
    );
    assert_eq!(
        store.leases().unwrap()[0].state,
        RuntimeLeaseState::Quarantined
    );
    let second_reports = host
        .reconcile_runtime_orphans()
        .expect("a quarantined orphan should remain inert");
    assert!(second_reports[0].quarantined);
    assert_eq!(host.discover()[0].state, ExtensionState::Quarantined);
    let recoveries = host
        .quarantined_runtime_leases()
        .expect("quarantined lease evidence should be queryable");
    assert_eq!(recoveries.len(), 1);
    host.close_quarantined_runtime_lease(&id, "uncertain-nonce")
        .expect("the user should be able to close quarantined evidence");
    assert!(host
        .quarantined_runtime_leases()
        .expect("the lease store should remain readable")
        .is_empty());
    assert!(host
        .drain_diagnostics()
        .iter()
        .any(|record| record.code == "runtime-orphan-identity-uncertain"));
}
