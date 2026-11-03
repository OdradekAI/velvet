use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use platform_process::ProcessSupervisor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use velvet_job_system::{JobBudgetRequest, JobProvenance, JOB_SCHEMA_VERSION};
use velvet_layout_model::WorkbenchZone;
use velvet_plugin_api::{
    validate_reserved_metadata, AppTarget, ContributionKind, ContributionScope, ExtensionId,
    ExtensionInstallOperation, ExtensionInstallPreview, ExtensionInstallRecovery,
    ExtensionInstallRecoveryAction, ExtensionInstallRecoveryKind, ExtensionInstallRecoveryLogEntry,
    ExtensionInstallRecoveryLogStatus, ExtensionManifest, ExtensionProjection, ExtensionState,
    ExtensionSummary, HostContainer, Permission, PreviewActionContext, RuntimeKind,
    RuntimeLaunchSpec, VersionRange, MANIFEST_SCHEMA_VERSION,
};

mod extension_storage;
mod platform_process;
mod runtime_lease;

pub use extension_storage::{
    BackupParticipant, ExtensionStorage, ExtensionStorageError, ExtensionStorageExport,
    ExtensionStorageRecovery, ExtensionStorageRecoveryStatus, ExtensionStorageTransaction,
    ExtensionStorageView, MigrationParticipant, EXTENSION_STORAGE_SCOPE,
    MAX_EXTENSION_STORAGE_BYTES, MAX_EXTENSION_STORAGE_KEY_BYTES,
};
pub use runtime_lease::{
    ConservativeRuntimeOrphanReconciler, InMemoryRuntimeLeaseStore, JsonFileRuntimeLeaseStore,
    PlatformRuntimeOrphanReconciler, QuarantinedRuntimeLease, RuntimeLease, RuntimeLeaseError,
    RuntimeLeaseState, RuntimeLeaseStore, RuntimeOrphanObservation, RuntimeOrphanReconciler,
    RuntimeOrphanReport, RuntimeProcessIdentity, MAX_RUNTIME_LEASES, MAX_RUNTIME_LEASE_STORE_BYTES,
    RUNTIME_LEASE_SCHEMA_VERSION,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginError {
    NotFound(ExtensionId),
    AlreadyRegistered(ExtensionId),
    IncompatibleHostApi(ExtensionId),
    UnknownWorkbenchApp(String),
    InvalidNamespace(String),
    InvalidContribution(String),
    PermissionDenied {
        extension_id: ExtensionId,
        zone: WorkbenchZone,
        permission: Permission,
    },
    RuntimeStart {
        extension_id: ExtensionId,
        message: String,
    },
    RuntimeCrashed {
        extension_id: ExtensionId,
        message: String,
    },
    RuntimeShutdown {
        extension_id: ExtensionId,
        message: String,
    },
    RuntimeInvocation {
        extension_id: ExtensionId,
        message: String,
    },
    RuntimeLease {
        extension_id: ExtensionId,
        message: String,
    },
    InstallFailed(String),
    InstallConflict(ExtensionId),
    VersionConflict {
        extension_id: ExtensionId,
        installed: String,
        requested: String,
    },
    UninstallFailed(String),
    ResourceNotFound {
        extension_id: ExtensionId,
        locale: String,
    },
    ResourceDenied {
        extension_id: ExtensionId,
        message: String,
    },
    ResourceTooLarge {
        extension_id: ExtensionId,
        locale: String,
        limit: usize,
    },
    ExtensionStorage {
        extension_id: ExtensionId,
        message: String,
    },
    InvalidState {
        extension_id: ExtensionId,
        state: ExtensionState,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisableReason {
    User,
    Crash,
    Incompatible,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    MissingLaunchSpec(ExtensionId),
    InvalidLaunchSpec(ExtensionId),
    StartFailed(String),
    ShutdownFailed(String),
    InvocationFailed(String),
    InvocationTimeout,
    InvocationRejected(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeHealth {
    Running,
    Crashed(String),
}

/// Controls how a runtime launch target is resolved.
///
/// `Direct` exists for trusted built-in/test composition. Installed external
/// extensions use `PackageRelative`, which allows only a host-approved bare
/// command or an executable that resolves inside the committed package root.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum RuntimeLaunchPolicy {
    #[default]
    Direct,
    PackageRelative {
        package_root: PathBuf,
        allowed_programs: BTreeSet<String>,
    },
}

/// Host-owned launch context. It is deliberately separate from the manifest
/// and never crosses the Rust/TypeScript/Tauri protocol boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeLaunchContext {
    pub policy: RuntimeLaunchPolicy,
    pub runtime_nonce: Option<String>,
    pub host_instance_id: Option<String>,
    pub launch_attempt_id: Option<u64>,
    pub crash_receipt_path: Option<PathBuf>,
}

impl RuntimeLaunchContext {
    pub fn package(package_root: impl Into<PathBuf>) -> Self {
        Self::package_with_allowlist(package_root, default_allowed_programs())
    }

    pub fn package_with_allowlist(
        package_root: impl Into<PathBuf>,
        allowed_programs: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            policy: RuntimeLaunchPolicy::PackageRelative {
                package_root: package_root.into(),
                allowed_programs: allowed_programs.into_iter().map(Into::into).collect(),
            },
            runtime_nonce: None,
            host_instance_id: None,
            launch_attempt_id: None,
            crash_receipt_path: None,
        }
    }

    pub fn with_runtime_nonce(mut self, runtime_nonce: impl Into<String>) -> Self {
        self.runtime_nonce = Some(runtime_nonce.into());
        self
    }

    pub fn with_host_context(
        mut self,
        host_instance_id: impl Into<String>,
        launch_attempt_id: u64,
    ) -> Self {
        self.host_instance_id = Some(host_instance_id.into());
        self.launch_attempt_id = Some(launch_attempt_id);
        self
    }

    pub fn with_crash_receipt_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.crash_receipt_path = Some(path.into());
        self
    }
}

pub const DEFAULT_ALLOWED_RUNTIME_PROGRAMS: &[&str] =
    &["bun", "deno", "node", "nodejs", "wasmer", "wasmtime"];

fn default_allowed_programs() -> BTreeSet<String> {
    DEFAULT_ALLOWED_RUNTIME_PROGRAMS
        .iter()
        .map(|program| (*program).to_owned())
        .collect()
}

const RUNTIME_PROTOCOL_VERSION: u16 = 1;
const RUNTIME_INVOCATION_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_JOB_EVENTS: usize = 128;
pub const MAX_RUNTIME_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_RUNTIME_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const STORAGE_EXPORT_METHOD: &str = "storage.export";
const STORAGE_MIGRATE_METHOD: &str = "storage.migrate";
pub const MAX_RUNTIME_STDERR_BYTES: usize = 16 * 1024;
pub const MAX_PLUGIN_LOCALIZATION_BYTES: usize = 1024 * 1024;
pub const MAX_PLUGIN_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PLUGIN_ARCHIVE_UNCOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PLUGIN_ARCHIVE_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_PLUGIN_ARCHIVE_ENTRIES: usize = 4096;
pub const MAX_PLUGIN_DIAGNOSTICS: usize = 256;
pub const MAX_PLUGIN_DIAGNOSTIC_MESSAGE_BYTES: usize = 512;
const RECOVERY_LOG_FILE: &str = ".plugin-recovery-log.json";
const RECOVERY_LOG_LIMIT: usize = 100;
static RUNTIME_NONCE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginLocalizationResource {
    pub extension_id: ExtensionId,
    pub locale: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginDiagnosticKind {
    Activation,
    Runtime,
    Invocation,
    Job,
    Storage,
    Migration,
    Shutdown,
}

/// A bounded, local-first diagnostic record owned by PluginHost.
///
/// Messages are normalized and path-like tokens are redacted before they
/// enter this queue. The queue is evidence for projection/export; it is not a
/// remote telemetry channel and never contains a plugin's raw payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginDiagnosticRecord {
    pub sequence: u64,
    pub extension_id: ExtensionId,
    pub kind: PluginDiagnosticKind,
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub attempt: Option<u32>,
    pub retryable: bool,
    pub occurred_at_ms: u64,
}

#[derive(Debug)]
pub struct PluginDiagnosticSink {
    next_sequence: u64,
    records: std::collections::VecDeque<PluginDiagnosticRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginDiagnosticInput {
    pub extension_id: ExtensionId,
    pub kind: PluginDiagnosticKind,
    pub code: String,
    pub message: String,
    pub job_id: Option<String>,
    pub attempt: Option<u32>,
    pub retryable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCrashEvidence {
    pub extension_id: ExtensionId,
    pub runtime_nonce: Option<String>,
    pub process_id: Option<u32>,
    pub host_instance_id: Option<String>,
    pub launch_attempt_id: Option<u64>,
    pub message: String,
    pub occurred_at_ms: u64,
}

impl Default for PluginDiagnosticSink {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            records: std::collections::VecDeque::new(),
        }
    }
}

impl PluginDiagnosticSink {
    pub fn record(&mut self, input: PluginDiagnosticInput) {
        let record = PluginDiagnosticRecord {
            sequence: self.next_sequence,
            extension_id: input.extension_id,
            kind: input.kind,
            code: input.code,
            message: sanitize_plugin_diagnostic_message(&input.message),
            job_id: input.job_id,
            attempt: input.attempt,
            retryable: input.retryable,
            occurred_at_ms: diagnostic_timestamp_ms(),
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        if self.records.len() == MAX_PLUGIN_DIAGNOSTICS {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }

    fn drain(&mut self) -> Vec<PluginDiagnosticRecord> {
        self.records.drain(..).collect()
    }
}

fn diagnostic_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

pub fn sanitize_plugin_diagnostic_message(message: &str) -> String {
    let normalized = message
        .chars()
        .map(|character| match character {
            '\r' | '\n' | '\t' => ' ',
            character if character.is_control() => ' ',
            character => character,
        })
        .collect::<String>();
    let redacted = normalized
        .split_whitespace()
        .map(|token| {
            let candidate = token.trim_start_matches(['(', '[', '{', '<']);
            if candidate.starts_with('/')
                || candidate.starts_with("~/")
                || candidate.contains('\\')
                || candidate.starts_with("file://")
                || candidate.contains("=/")
                || candidate
                    .as_bytes()
                    .get(1)
                    .is_some_and(|character| *character == b':')
            {
                "<redacted-path>"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if redacted.len() <= MAX_PLUGIN_DIAGNOSTIC_MESSAGE_BYTES {
        return redacted;
    }
    let mut end = MAX_PLUGIN_DIAGNOSTIC_MESSAGE_BYTES;
    while !redacted.is_char_boundary(end) {
        end -= 1;
    }
    redacted[..end].to_owned()
}

/// Package resource adapter for host-owned, bounded reads.
///
/// The loader never accepts an arbitrary filesystem path. It derives the
/// installed package from the validated extension id, checks the declared
/// localization root and locale, rejects symlink escapes, and enforces a
/// bounded payload before returning bytes to the protocol layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginResourceLoader {
    install_root: PathBuf,
}

impl PluginResourceLoader {
    pub fn new(install_root: impl Into<PathBuf>) -> Self {
        Self {
            install_root: install_root.into(),
        }
    }

    pub fn read_localization_bundle(
        &self,
        extension_id: &ExtensionId,
        manifest: &ExtensionManifest,
        locale: &str,
    ) -> Result<PluginLocalizationResource, PluginError> {
        let localization =
            manifest
                .localization
                .as_ref()
                .ok_or_else(|| PluginError::ResourceDenied {
                    extension_id: extension_id.clone(),
                    message: "extension does not declare localization resources".into(),
                })?;
        if !localization
            .supported_locales
            .iter()
            .any(|supported| supported == locale)
        {
            return Err(PluginError::ResourceNotFound {
                extension_id: extension_id.clone(),
                locale: locale.into(),
            });
        }
        if !is_safe_extension_id(&extension_id.0)
            || !is_safe_resource_root(&localization.resource_root)
            || !is_safe_locale_segment(locale)
        {
            return Err(PluginError::ResourceDenied {
                extension_id: extension_id.clone(),
                message: "localization resource path is invalid".into(),
            });
        }

        let install_root =
            fs::canonicalize(&self.install_root).map_err(|error| PluginError::ResourceDenied {
                extension_id: extension_id.clone(),
                message: format!("plugin install root is unavailable: {error}"),
            })?;
        let package_path = self.install_root.join(&extension_id.0);
        let package_root =
            fs::canonicalize(&package_path).map_err(|_| PluginError::ResourceNotFound {
                extension_id: extension_id.clone(),
                locale: locale.into(),
            })?;
        if !package_root.starts_with(&install_root) {
            return Err(PluginError::ResourceDenied {
                extension_id: extension_id.clone(),
                message: "plugin package escapes the install root".into(),
            });
        }

        let resource_path = package_root
            .join(&localization.resource_root)
            .join(format!("{locale}.json"));
        let canonical_resource =
            fs::canonicalize(&resource_path).map_err(|_| PluginError::ResourceNotFound {
                extension_id: extension_id.clone(),
                locale: locale.into(),
            })?;
        if !canonical_resource.starts_with(&package_root) {
            return Err(PluginError::ResourceDenied {
                extension_id: extension_id.clone(),
                message: "localization resource escapes the plugin package".into(),
            });
        }

        let file =
            fs::File::open(&canonical_resource).map_err(|_| PluginError::ResourceNotFound {
                extension_id: extension_id.clone(),
                locale: locale.into(),
            })?;
        let metadata = file
            .metadata()
            .map_err(|error| PluginError::ResourceDenied {
                extension_id: extension_id.clone(),
                message: format!("localization resource metadata is unavailable: {error}"),
            })?;
        if !metadata.is_file() {
            return Err(PluginError::ResourceNotFound {
                extension_id: extension_id.clone(),
                locale: locale.into(),
            });
        }
        if metadata.len() > MAX_PLUGIN_LOCALIZATION_BYTES as u64 {
            return Err(PluginError::ResourceTooLarge {
                extension_id: extension_id.clone(),
                locale: locale.into(),
                limit: MAX_PLUGIN_LOCALIZATION_BYTES,
            });
        }

        let mut bytes =
            Vec::with_capacity(metadata.len().min(MAX_PLUGIN_LOCALIZATION_BYTES as u64) as usize);
        file.take((MAX_PLUGIN_LOCALIZATION_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| PluginError::ResourceDenied {
                extension_id: extension_id.clone(),
                message: format!("localization resource could not be read: {error}"),
            })?;
        if bytes.len() > MAX_PLUGIN_LOCALIZATION_BYTES {
            return Err(PluginError::ResourceTooLarge {
                extension_id: extension_id.clone(),
                locale: locale.into(),
                limit: MAX_PLUGIN_LOCALIZATION_BYTES,
            });
        }

        Ok(PluginLocalizationResource {
            extension_id: extension_id.clone(),
            locale: locale.into(),
            content_type: "application/json".into(),
            bytes,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginArchiveError {
    ReadFailed(String),
    InvalidArchive(String),
    EntryRejected(String),
    TooLarge(String),
    MissingManifest,
    DestinationExists,
}

/// Extracts a ZIP plugin package into a caller-owned temporary directory.
///
/// The adapter is deliberately separate from `PluginResourceLoader`: archive
/// format handling ends after a validated directory package exists, while
/// resource reads continue to use the same package-root containment checks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ZipPluginArchiveAdapter;

impl ZipPluginArchiveAdapter {
    pub fn extract_to(
        &self,
        archive_path: &Path,
        destination: &Path,
    ) -> Result<(), PluginArchiveError> {
        if destination.exists() {
            return Err(PluginArchiveError::DestinationExists);
        }
        let file = fs::File::open(archive_path)
            .map_err(|error| PluginArchiveError::ReadFailed(error.to_string()))?;
        let archive_size = file
            .metadata()
            .map_err(|error| PluginArchiveError::ReadFailed(error.to_string()))?
            .len();
        if archive_size > MAX_PLUGIN_ARCHIVE_BYTES {
            return Err(PluginArchiveError::TooLarge(format!(
                "archive exceeds the {MAX_PLUGIN_ARCHIVE_BYTES} byte limit"
            )));
        }

        let mut archive = zip::ZipArchive::new(file)
            .map_err(|error| PluginArchiveError::InvalidArchive(error.to_string()))?;
        if archive.len() > MAX_PLUGIN_ARCHIVE_ENTRIES {
            return Err(PluginArchiveError::TooLarge(format!(
                "archive contains more than {MAX_PLUGIN_ARCHIVE_ENTRIES} entries"
            )));
        }

        fs::create_dir_all(destination)
            .map_err(|error| PluginArchiveError::ReadFailed(error.to_string()))?;
        let result = self.extract_entries(&mut archive, destination);
        if result.is_ok() && !destination.join("manifest.json").is_file() {
            let _ = fs::remove_dir_all(destination);
            return Err(PluginArchiveError::MissingManifest);
        }
        if result.is_err() {
            let _ = fs::remove_dir_all(destination);
        }
        result
    }

    fn extract_entries<R: Read + std::io::Seek>(
        &self,
        archive: &mut zip::ZipArchive<R>,
        destination: &Path,
    ) -> Result<(), PluginArchiveError> {
        let mut seen = BTreeSet::new();
        let mut total_uncompressed = 0_u64;
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|error| PluginArchiveError::InvalidArchive(error.to_string()))?;
            let raw_name = entry
                .name()
                .map_err(|error| PluginArchiveError::InvalidArchive(error.to_string()))?
                .into_owned();
            let relative = safe_archive_entry_path(&raw_name).ok_or_else(|| {
                PluginArchiveError::EntryRejected("archive entry path is unsafe".into())
            })?;
            if !seen.insert(relative.clone()) {
                return Err(PluginArchiveError::EntryRejected(
                    "archive contains duplicate entry paths".into(),
                ));
            }
            if entry.encrypted() || entry.is_symlink() {
                return Err(PluginArchiveError::EntryRejected(
                    "encrypted and symbolic-link entries are not supported".into(),
                ));
            }

            let target = destination.join(&relative);
            if !target.starts_with(destination) {
                return Err(PluginArchiveError::EntryRejected(
                    "archive entry escapes the extraction root".into(),
                ));
            }
            if entry.is_dir() {
                fs::create_dir_all(&target)
                    .map_err(|error| PluginArchiveError::ReadFailed(error.to_string()))?;
                continue;
            }
            if !entry.is_file() {
                return Err(PluginArchiveError::EntryRejected(
                    "archive contains an unsupported special entry".into(),
                ));
            }

            let declared_size = entry.size();
            if declared_size > MAX_PLUGIN_ARCHIVE_ENTRY_BYTES {
                return Err(PluginArchiveError::TooLarge(format!(
                    "archive entry exceeds the {MAX_PLUGIN_ARCHIVE_ENTRY_BYTES} byte limit"
                )));
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| PluginArchiveError::ReadFailed(error.to_string()))?;
            }
            let mut output = fs::File::create(&target)
                .map_err(|error| PluginArchiveError::ReadFailed(error.to_string()))?;
            let mut bytes = Vec::with_capacity(declared_size as usize);
            entry
                .take(MAX_PLUGIN_ARCHIVE_ENTRY_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| PluginArchiveError::ReadFailed(error.to_string()))?;
            if bytes.len() as u64 > MAX_PLUGIN_ARCHIVE_ENTRY_BYTES {
                return Err(PluginArchiveError::TooLarge(format!(
                    "archive entry exceeds the {MAX_PLUGIN_ARCHIVE_ENTRY_BYTES} byte limit"
                )));
            }
            total_uncompressed = total_uncompressed
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| {
                    PluginArchiveError::TooLarge("archive uncompressed size overflowed".into())
                })?;
            if total_uncompressed > MAX_PLUGIN_ARCHIVE_UNCOMPRESSED_BYTES {
                return Err(PluginArchiveError::TooLarge(format!(
                    "archive exceeds the {MAX_PLUGIN_ARCHIVE_UNCOMPRESSED_BYTES} byte unpacked limit"
                )));
            }
            output
                .write_all(&bytes)
                .map_err(|error| PluginArchiveError::ReadFailed(error.to_string()))?;
        }
        Ok(())
    }
}

fn safe_archive_entry_path(raw_name: &str) -> Option<PathBuf> {
    let trimmed = raw_name.strip_suffix('/').unwrap_or(raw_name);
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
    {
        return None;
    }

    let mut path = PathBuf::new();
    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.contains(':') {
            return None;
        }
        path.push(segment);
    }
    Some(path)
}

fn temporary_archive_directory(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "velvet-plugin-archive-{prefix}-{}-{nonce}",
        std::process::id()
    ))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginInvocation {
    pub extension_id: ExtensionId,
    pub method: String,
    pub command_id: String,
    pub args: Value,
    pub context: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginInvocationResult {
    pub result: Option<Value>,
    pub error: Option<String>,
    #[serde(default)]
    pub events: Vec<PluginInvocationEvent>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginInvocationEvent {
    pub kind: String,
    #[serde(default)]
    pub progress: Option<u8>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub checkpoint: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StorageRuntimeRequest {
    source_schema_version: u16,
    target_schema_version: u16,
    source_generation: u64,
    entries: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StorageRuntimeResult {
    schema_version: u16,
    entries: BTreeMap<String, Vec<u8>>,
}

/// Runtime input for a declared Job contribution. The host owns Job
/// lifecycle and provenance; the plugin receives a bounded JSON projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginJobInvocation {
    pub job_id: String,
    pub job_kind: String,
    pub attempt: u32,
    pub input: Value,
    pub provenance: JobProvenance,
    #[serde(default)]
    pub budget: JobBudgetRequest,
    #[serde(default)]
    pub deadline_ms: Option<u64>,
    #[serde(default = "default_job_cancellation_grace_ms")]
    pub cancellation_grace_ms: u64,
    #[serde(default)]
    pub checkpoint: Option<Value>,
}

fn default_job_cancellation_grace_ms() -> u64 {
    1_000
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginJobResult {
    pub content_type: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginJobExecution {
    pub result: PluginJobResult,
    pub events: Vec<PluginInvocationEvent>,
}

/// A thread-safe handle to one active plugin runtime process. It is an
/// adapter seam only; PluginHost remains responsible for activation,
/// permissions and lifecycle state.
#[derive(Clone)]
pub struct PluginJobHandle {
    extension_id: ExtensionId,
    job_kind: String,
    process: Arc<Mutex<Box<dyn RuntimeProcess>>>,
    diagnostics: Arc<Mutex<PluginDiagnosticSink>>,
}

impl PluginJobHandle {
    pub fn extension_id(&self) -> &ExtensionId {
        &self.extension_id
    }

    pub fn job_kind(&self) -> &str {
        &self.job_kind
    }

    pub fn invoke(
        &self,
        invocation: PluginJobInvocation,
    ) -> Result<PluginJobExecution, PluginError> {
        if invocation.job_kind != self.job_kind {
            return Err(PluginError::InvalidContribution(format!(
                "Job invocation kind {} does not match {}",
                invocation.job_kind, self.job_kind
            )));
        }
        validate_job_provenance(&self.extension_id, &invocation.provenance)?;
        let args = invocation.input.clone();
        let context =
            serde_json::to_value(&invocation).map_err(|error| PluginError::RuntimeInvocation {
                extension_id: self.extension_id.clone(),
                message: format!("Job invocation context could not be encoded: {error}"),
            })?;
        let plugin_invocation = PluginInvocation {
            extension_id: self.extension_id.clone(),
            method: "job.run".into(),
            command_id: self.job_kind.clone(),
            args,
            context,
        };
        validate_runtime_payload(
            &plugin_invocation.extension_id,
            &plugin_invocation.args,
            &plugin_invocation.context,
        )?;
        let result = self
            .process
            .lock()
            .map_err(|_| {
                self.record_diagnostic(
                    "job-runtime-lock-poisoned",
                    "plugin runtime lock poisoned",
                    &invocation,
                    true,
                );
                PluginError::RuntimeInvocation {
                    extension_id: self.extension_id.clone(),
                    message: "plugin runtime lock poisoned".into(),
                }
            })?
            .invoke(&plugin_invocation)
            .map_err(|error| {
                let message = runtime_error_message(error);
                self.record_diagnostic("job-runtime-failed", &message, &invocation, true);
                PluginError::RuntimeInvocation {
                    extension_id: self.extension_id.clone(),
                    message,
                }
            })?;
        if let Some(error) = result.error {
            self.record_diagnostic("job-rejected", &error, &invocation, true);
            return Err(PluginError::RuntimeInvocation {
                extension_id: self.extension_id.clone(),
                message: error,
            });
        }
        if result.events.len() > MAX_JOB_EVENTS {
            self.record_diagnostic(
                "job-events-overflow",
                &format!("job runtime returned too many progress events (max {MAX_JOB_EVENTS})"),
                &invocation,
                false,
            );
            return Err(PluginError::RuntimeInvocation {
                extension_id: self.extension_id.clone(),
                message: format!(
                    "job runtime returned too many progress events (max {MAX_JOB_EVENTS})"
                ),
            });
        }
        let payload = result.result.ok_or_else(|| {
            self.record_diagnostic(
                "job-missing-result",
                "job runtime returned no result",
                &invocation,
                true,
            );
            PluginError::RuntimeInvocation {
                extension_id: self.extension_id.clone(),
                message: "job runtime returned no result".into(),
            }
        })?;
        let job_result = serde_json::from_value::<PluginJobResult>(payload).map_err(|error| {
            self.record_diagnostic(
                "job-invalid-result",
                &format!("invalid job result: {error}"),
                &invocation,
                false,
            );
            PluginError::RuntimeInvocation {
                extension_id: self.extension_id.clone(),
                message: format!("invalid job result: {error}"),
            }
        })?;
        Ok(PluginJobExecution {
            result: job_result,
            events: result.events,
        })
    }

    fn record_diagnostic(
        &self,
        code: &str,
        message: &str,
        invocation: &PluginJobInvocation,
        retryable: bool,
    ) {
        if let Ok(mut sink) = self.diagnostics.lock() {
            sink.record(PluginDiagnosticInput {
                extension_id: self.extension_id.clone(),
                kind: PluginDiagnosticKind::Job,
                code: code.into(),
                message: message.into(),
                job_id: Some(invocation.job_id.clone()),
                attempt: Some(invocation.attempt),
                retryable,
            });
        }
    }
}

pub trait RuntimeProcess: Send {
    fn poll(&mut self) -> RuntimeHealth;
    fn shutdown(&mut self) -> Result<(), RuntimeError>;
    fn identity(&self) -> Option<RuntimeProcessIdentity> {
        None
    }
    fn invoke(
        &mut self,
        invocation: &PluginInvocation,
    ) -> Result<PluginInvocationResult, RuntimeError>;
}

pub trait PluginRuntime: Send {
    fn start(
        &mut self,
        manifest: &ExtensionManifest,
        permissions: &PermissionGrant,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError>;

    fn start_with_context(
        &mut self,
        manifest: &ExtensionManifest,
        permissions: &PermissionGrant,
        _context: &RuntimeLaunchContext,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
        self.start(manifest, permissions)
    }

    fn requires_package_launch_context(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PermissionGrant {
    permissions: BTreeSet<Permission>,
}

impl PermissionGrant {
    pub fn contains(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    pub fn permissions(&self) -> impl Iterator<Item = &Permission> {
        self.permissions.iter()
    }

    fn from_manifest(manifest: &ExtensionManifest) -> Self {
        Self {
            permissions: manifest
                .permission_sets
                .iter()
                .flat_map(|set| set.permissions.iter().cloned())
                .collect(),
        }
    }

    fn as_env_value(&self) -> String {
        self.permissions
            .iter()
            .map(permission_name)
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn resolve_launch_program(
    extension_id: &ExtensionId,
    spec: &RuntimeLaunchSpec,
    context: &RuntimeLaunchContext,
) -> Result<(PathBuf, Option<PathBuf>), RuntimeError> {
    match &context.policy {
        RuntimeLaunchPolicy::Direct => Ok((PathBuf::from(&spec.program), None)),
        RuntimeLaunchPolicy::PackageRelative {
            package_root,
            allowed_programs,
        } => {
            let canonical_root = fs::canonicalize(package_root).map_err(|error| {
                RuntimeError::StartFailed(format!(
                    "runtime package root for {} is unavailable: {error}",
                    extension_id.0
                ))
            })?;
            let raw_program = Path::new(&spec.program);
            if raw_program.is_absolute() {
                return Err(RuntimeError::StartFailed(format!(
                    "runtime {} must use a package-relative executable or an allowed host command",
                    extension_id.0
                )));
            }

            if is_bare_program(&spec.program) {
                if allowed_programs.contains(&spec.program) {
                    return Ok((PathBuf::from(&spec.program), Some(canonical_root)));
                }
                return Err(RuntimeError::StartFailed(format!(
                    "runtime command {} is not in the host allow-list",
                    spec.program
                )));
            }

            if !is_explicit_relative_path(&spec.program) {
                return Err(RuntimeError::StartFailed(
                    "runtime executable must start with ./ or ../ when it is package-relative"
                        .into(),
                ));
            }
            let candidate =
                fs::canonicalize(canonical_root.join(raw_program)).map_err(|error| {
                    RuntimeError::StartFailed(format!(
                        "runtime executable could not be resolved inside its package: {error}"
                    ))
                })?;
            if !candidate.starts_with(&canonical_root) || !candidate.is_file() {
                return Err(RuntimeError::StartFailed(
                    "runtime executable resolves outside its committed package".into(),
                ));
            }
            Ok((candidate, Some(canonical_root)))
        }
    }
}

fn validate_launch_context(
    manifest: &ExtensionManifest,
    context: &RuntimeLaunchContext,
) -> Result<(), RuntimeError> {
    let Some(spec) = manifest.launch.as_ref() else {
        return Err(RuntimeError::MissingLaunchSpec(manifest.id.clone()));
    };
    if spec.program.trim().is_empty() {
        return Err(RuntimeError::InvalidLaunchSpec(manifest.id.clone()));
    }
    resolve_launch_program(&manifest.id, spec, context).map(|_| ())
}

fn is_bare_program(program: &str) -> bool {
    !program.is_empty() && !program.contains(['/', '\\']) && !matches!(program, "." | "..")
}

fn is_explicit_relative_path(program: &str) -> bool {
    program.starts_with("./")
        || program.starts_with("../")
        || program.starts_with(".\\")
        || program.starts_with("..\\")
}

/// Starts the declared isolated process for WASM runners and sidecars.
/// Built-in extensions receive a no-op process so the host uses one lifecycle
/// path for trusted and external extensions.
#[derive(Clone, Debug, Default)]
pub struct ProcessPluginRuntime;

impl PluginRuntime for ProcessPluginRuntime {
    fn start(
        &mut self,
        manifest: &ExtensionManifest,
        permissions: &PermissionGrant,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
        self.start_with_context(manifest, permissions, &RuntimeLaunchContext::default())
    }

    fn start_with_context(
        &mut self,
        manifest: &ExtensionManifest,
        permissions: &PermissionGrant,
        context: &RuntimeLaunchContext,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
        if manifest.runtime == RuntimeKind::BuiltIn {
            return Ok(Box::new(NoopRuntimeProcess));
        }

        let Some(spec) = manifest.launch.as_ref() else {
            return Err(RuntimeError::MissingLaunchSpec(manifest.id.clone()));
        };

        if spec.program.trim().is_empty() {
            return Err(RuntimeError::InvalidLaunchSpec(manifest.id.clone()));
        }

        let (program, package_root) = resolve_launch_program(&manifest.id, spec, context)?;
        let mut command = Command::new(program);
        command
            .args(&spec.args)
            .env("VELVET_EXTENSION_ID", &manifest.id.0)
            .env("VELVET_PLUGIN_PERMISSIONS", permissions.as_env_value())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(runtime_nonce) = &context.runtime_nonce {
            command.env("VELVET_RUNTIME_LEASE_ID", runtime_nonce);
        }
        if let Some(host_instance_id) = &context.host_instance_id {
            command.env("VELVET_HOST_INSTANCE_ID", host_instance_id);
        }
        if let Some(launch_attempt_id) = context.launch_attempt_id {
            command.env("VELVET_LAUNCH_ATTEMPT_ID", launch_attempt_id.to_string());
        }
        if let Some(crash_receipt_path) = &context.crash_receipt_path {
            command.env("VELVET_CRASH_RECEIPT_PATH", crash_receipt_path);
        }
        if let Some(package_root) = package_root {
            command.current_dir(package_root);
        }
        #[cfg(unix)]
        command.process_group(0);
        #[cfg(target_os = "linux")]
        configure_linux_parent_death(&mut command);
        let process_supervisor = ProcessSupervisor::new().map_err(RuntimeError::StartFailed)?;
        let mut child = command
            .spawn()
            .map_err(|error| RuntimeError::StartFailed(error.to_string()))?;
        if let Err(error) = process_supervisor.assign(child.id()) {
            let _ = terminate_child(&mut child);
            let _ = child.wait();
            return Err(RuntimeError::StartFailed(error));
        }

        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                let _ = terminate_child(&mut child);
                let _ = child.wait();
                return Err(RuntimeError::StartFailed(
                    "external runtime did not expose stdin".into(),
                ));
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = terminate_child(&mut child);
                let _ = child.wait();
                return Err(RuntimeError::StartFailed(
                    "external runtime did not expose stdout".into(),
                ));
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                let _ = terminate_child(&mut child);
                let _ = child.wait();
                return Err(RuntimeError::StartFailed(
                    "external runtime did not expose stderr".into(),
                ));
            }
        };
        let stderr_tail = Arc::new(Mutex::new(Vec::new()));
        let stderr_worker_tail = Arc::clone(&stderr_tail);
        let stderr_worker = match thread::Builder::new()
            .name(format!("velvet-plugin-stderr-{}", manifest.id.0))
            .spawn(move || read_bounded_stderr(stderr, stderr_worker_tail))
        {
            Ok(worker) => worker,
            Err(error) => {
                let _ = terminate_child(&mut child);
                let _ = child.wait();
                return Err(RuntimeError::StartFailed(error.to_string()));
            }
        };
        let (requests, receiver) = mpsc::channel();
        let worker = match thread::Builder::new()
            .name(format!("velvet-plugin-{}", manifest.id.0))
            .spawn(move || runtime_io_loop(stdin, stdout, receiver))
        {
            Ok(worker) => worker,
            Err(error) => {
                let _ = terminate_child(&mut child);
                let _ = child.wait();
                let _ = stderr_worker.join();
                return Err(RuntimeError::StartFailed(error.to_string()));
            }
        };

        Ok(Box::new(ChildRuntimeProcess {
            process_supervisor,
            start_token: process_start_token(child.id()),
            child,
            requests: Some(requests),
            worker: Some(worker),
            stderr_worker: Some(stderr_worker),
            stderr_tail,
        }))
    }

    fn requires_package_launch_context(&self) -> bool {
        true
    }
}

enum RuntimeIoRequest {
    Invoke {
        line: String,
        response: Sender<Result<String, RuntimeError>>,
    },
    Stop,
}

struct ChildRuntimeProcess {
    process_supervisor: ProcessSupervisor,
    start_token: Option<String>,
    child: Child,
    requests: Option<Sender<RuntimeIoRequest>>,
    worker: Option<JoinHandle<()>>,
    stderr_worker: Option<JoinHandle<()>>,
    stderr_tail: Arc<Mutex<Vec<u8>>>,
}

impl RuntimeProcess for ChildRuntimeProcess {
    fn poll(&mut self) -> RuntimeHealth {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                cleanup_exited_child_group(&self.child);
                let _ = self.child.wait();
                self.join_stderr_worker();
                RuntimeHealth::Crashed(exit_evidence(&status, &self.stderr_snapshot()))
            }
            Ok(None) => RuntimeHealth::Running,
            Err(error) => RuntimeHealth::Crashed(error.to_string()),
        }
    }

    fn shutdown(&mut self) -> Result<(), RuntimeError> {
        if let Some(requests) = self.requests.take() {
            let _ = requests.send(RuntimeIoRequest::Stop);
        }

        let shutdown_result = match self.child.try_wait() {
            Ok(Some(_)) => {
                cleanup_exited_child_group(&self.child);
                self.join_stderr_worker();
                Ok(())
            }
            Ok(None) => {
                self.process_supervisor
                    .terminate()
                    .map_err(RuntimeError::ShutdownFailed)?;
                #[cfg(not(windows))]
                terminate_child(&mut self.child)?;
                self.child
                    .wait()
                    .map(|_| ())
                    .map_err(|error| RuntimeError::ShutdownFailed(error.to_string()))
            }
            Err(error) => Err(RuntimeError::ShutdownFailed(error.to_string())),
        };

        self.join_stderr_worker();

        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() && shutdown_result.is_ok() {
                return Err(RuntimeError::ShutdownFailed(
                    "runtime I/O worker panicked".into(),
                ));
            }
        }

        shutdown_result
    }

    fn identity(&self) -> Option<RuntimeProcessIdentity> {
        Some(RuntimeProcessIdentity {
            pid: self.child.id(),
            process_group_id: process_group_id(self.child.id()),
            start_token: self.start_token.clone(),
        })
    }

    fn invoke(
        &mut self,
        invocation: &PluginInvocation,
    ) -> Result<PluginInvocationResult, RuntimeError> {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                cleanup_exited_child_group(&self.child);
                let _ = self.child.wait();
                self.join_stderr_worker();
                return Err(RuntimeError::InvocationFailed(format!(
                    "runtime exited before invocation: {}",
                    exit_evidence(&status, &self.stderr_snapshot())
                )));
            }
            Ok(None) => {}
            Err(error) => return Err(RuntimeError::InvocationFailed(error.to_string())),
        }

        let schema_id = (invocation.method == "job.run").then(|| {
            format!(
                "velvet.job.{}.{}",
                invocation.extension_id.0, invocation.command_id
            )
        });
        let request = RuntimeInvocationRequest {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
            kind: match invocation.method.as_str() {
                "job.run" => "invoke-job",
                STORAGE_EXPORT_METHOD | STORAGE_MIGRATE_METHOD => "invoke-storage",
                _ => "invoke-command",
            },
            method: &invocation.method,
            extension_id: &invocation.extension_id.0,
            command_id: &invocation.command_id,
            args: &invocation.args,
            context: &invocation.context,
            schema_id: schema_id.as_deref(),
            schema_version: schema_id.as_ref().map(|_| JOB_SCHEMA_VERSION),
        };
        let line = serde_json::to_string(&request)
            .map_err(|error| RuntimeError::InvocationFailed(error.to_string()))?;
        if line.len() > MAX_RUNTIME_REQUEST_BYTES {
            return Err(RuntimeError::InvocationFailed(format!(
                "runtime invocation request exceeded {MAX_RUNTIME_REQUEST_BYTES} bytes"
            )));
        }
        let (response_sender, response_receiver) = mpsc::channel();
        self.requests
            .as_ref()
            .ok_or_else(|| RuntimeError::InvocationFailed("runtime is shut down".into()))?
            .send(RuntimeIoRequest::Invoke {
                line,
                response: response_sender,
            })
            .map_err(|_| RuntimeError::InvocationFailed("runtime I/O worker stopped".into()))?;

        let response = response_receiver
            .recv_timeout(RUNTIME_INVOCATION_TIMEOUT)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => RuntimeError::InvocationTimeout,
                mpsc::RecvTimeoutError::Disconnected => {
                    RuntimeError::InvocationFailed("runtime I/O worker disconnected".into())
                }
            })??;
        let response = serde_json::from_str::<RuntimeInvocationResponse>(&response)
            .map_err(|error| RuntimeError::InvocationFailed(error.to_string()))?;
        if response.protocol_version != RUNTIME_PROTOCOL_VERSION {
            return Err(RuntimeError::InvocationFailed(format!(
                "runtime protocol version {} is incompatible with {}",
                response.protocol_version, RUNTIME_PROTOCOL_VERSION
            )));
        }
        if !response.ok {
            return Err(RuntimeError::InvocationRejected(
                response
                    .error
                    .unwrap_or_else(|| "plugin rejected the command".into()),
            ));
        }

        Ok(PluginInvocationResult {
            result: response.result,
            error: response.error,
            events: response.events,
        })
    }
}

impl ChildRuntimeProcess {
    fn stderr_snapshot(&self) -> Vec<u8> {
        self.stderr_tail
            .lock()
            .map(|tail| tail.clone())
            .unwrap_or_default()
    }

    fn join_stderr_worker(&mut self) {
        if let Some(worker) = self.stderr_worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_bounded_stderr(mut stderr: ChildStderr, tail: Arc<Mutex<Vec<u8>>>) {
    let mut chunk = [0_u8; 4096];
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => {
                if let Ok(mut tail) = tail.lock() {
                    tail.extend_from_slice(&chunk[..count]);
                    if tail.len() > MAX_RUNTIME_STDERR_BYTES {
                        let remove = tail.len() - MAX_RUNTIME_STDERR_BYTES;
                        tail.drain(..remove);
                    }
                }
            }
            Err(_) => break,
        }
    }
}

fn exit_evidence(status: &std::process::ExitStatus, stderr: &[u8]) -> String {
    let mut message = format!("process exited with {status}");
    #[cfg(unix)]
    if let Some(signal) = status.signal() {
        message.push_str(&format!(" (signal {signal})"));
    }
    if !stderr.is_empty() {
        message.push_str(": stderr: ");
        message.push_str(&sanitize_plugin_diagnostic_message(
            &String::from_utf8_lossy(stderr),
        ));
    }
    message
}

fn terminate_child(child: &mut Child) -> Result<(), RuntimeError> {
    #[cfg(unix)]
    {
        let process_group = -(child.id() as i32);
        let result = unsafe { libc::kill(process_group, libc::SIGKILL) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            return child.kill().map_err(|fallback| {
                RuntimeError::ShutdownFailed(format!(
                    "process-group kill failed: {error}; direct kill failed: {fallback}"
                ))
            });
        }
        Ok(())
    }

    #[cfg(not(unix))]
    child
        .kill()
        .map_err(|error| RuntimeError::ShutdownFailed(error.to_string()))
}

#[cfg(unix)]
fn process_group_id(pid: u32) -> Option<u32> {
    Some(pid)
}

#[cfg(not(unix))]
fn process_group_id(_pid: u32) -> Option<u32> {
    None
}

#[cfg(target_os = "linux")]
fn process_start_token(pid: u32) -> Option<String> {
    let contents = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, fields) = contents.rsplit_once(") ")?;
    fields.split_whitespace().nth(19).map(ToOwned::to_owned)
}

#[cfg(any(target_os = "macos", windows))]
fn process_start_token(pid: u32) -> Option<String> {
    platform_process::inspect_process(pid).and_then(|identity| identity.start_token)
}

#[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows)))]
fn process_start_token(_pid: u32) -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
fn configure_linux_parent_death(command: &mut Command) {
    // The kernel delivers SIGKILL if Core disappears before the sidecar can
    // be reconciled. This is an availability guard; it does not sandbox the
    // child or replace the restart/orphan policy owned by PluginHost.
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn cleanup_exited_child_group(child: &Child) {
    #[cfg(unix)]
    {
        let process_group = -(child.id() as i32);
        let _ = unsafe { libc::kill(process_group, libc::SIGKILL) };
    }
    #[cfg(not(unix))]
    let _ = child;
}

struct NoopRuntimeProcess;

impl RuntimeProcess for NoopRuntimeProcess {
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
        if invocation.method == "job.run" {
            return Ok(PluginInvocationResult {
                result: Some(serde_json::json!({
                    "contentType": "application/json",
                    "payload": {
                        "jobKind": invocation.command_id.clone(),
                        "accepted": true,
                    }
                })),
                error: None,
                events: vec![PluginInvocationEvent {
                    kind: "progress".into(),
                    progress: Some(100),
                    message: Some("Completed".into()),
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInvocationRequest<'a> {
    protocol_version: u16,
    kind: &'static str,
    method: &'a str,
    extension_id: &'a str,
    command_id: &'a str,
    args: &'a Value,
    context: &'a Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<u16>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeInvocationResponse {
    protocol_version: u16,
    ok: bool,
    result: Option<Value>,
    error: Option<String>,
    #[serde(default)]
    events: Vec<PluginInvocationEvent>,
}

fn validate_runtime_payload(
    extension_id: &ExtensionId,
    args: &Value,
    context: &Value,
) -> Result<(), PluginError> {
    let bytes =
        serde_json::to_vec(&(args, context)).map_err(|error| PluginError::RuntimeInvocation {
            extension_id: extension_id.clone(),
            message: format!("runtime invocation payload could not be encoded: {error}"),
        })?;
    if bytes.len() > MAX_RUNTIME_REQUEST_BYTES {
        return Err(PluginError::RuntimeInvocation {
            extension_id: extension_id.clone(),
            message: format!(
                "runtime invocation request exceeded {MAX_RUNTIME_REQUEST_BYTES} bytes"
            ),
        });
    }
    Ok(())
}

fn validate_job_provenance(
    extension_id: &ExtensionId,
    provenance: &JobProvenance,
) -> Result<(), PluginError> {
    if provenance.owner_id != extension_id.0 {
        return Err(PluginError::RuntimeInvocation {
            extension_id: extension_id.clone(),
            message: format!(
                "Job provenance owner {} does not match extension {}",
                provenance.owner_id, extension_id.0
            ),
        });
    }
    if provenance.schema_version != JOB_SCHEMA_VERSION {
        return Err(PluginError::RuntimeInvocation {
            extension_id: extension_id.clone(),
            message: format!(
                "Job provenance schema {} is unsupported; expected {}",
                provenance.schema_version, JOB_SCHEMA_VERSION
            ),
        });
    }
    Ok(())
}

fn runtime_io_loop(
    stdin: std::process::ChildStdin,
    stdout: std::process::ChildStdout,
    receiver: Receiver<RuntimeIoRequest>,
) {
    let mut stdin = stdin;
    let mut stdout = BufReader::new(stdout);
    while let Ok(request) = receiver.recv() {
        match request {
            RuntimeIoRequest::Stop => break,
            RuntimeIoRequest::Invoke { line, response } => {
                let result = (|| {
                    stdin
                        .write_all(line.as_bytes())
                        .and_then(|_| stdin.write_all(b"\n"))
                        .and_then(|_| stdin.flush())
                        .map_err(|error| RuntimeError::InvocationFailed(error.to_string()))?;
                    let mut response_line = Vec::new();
                    loop {
                        let available = stdout
                            .fill_buf()
                            .map_err(|error| RuntimeError::InvocationFailed(error.to_string()))?;
                        if available.is_empty() {
                            if response_line.is_empty() {
                                return Err(RuntimeError::InvocationFailed(
                                    "runtime closed stdout without a response".into(),
                                ));
                            }
                            break;
                        }
                        let newline = available.iter().position(|byte| *byte == b'\n');
                        let take = newline.map_or(available.len(), |index| index + 1);
                        if response_line.len().saturating_add(take) > MAX_RUNTIME_RESPONSE_BYTES {
                            return Err(RuntimeError::InvocationFailed(format!(
                                "runtime response exceeded {MAX_RUNTIME_RESPONSE_BYTES} bytes"
                            )));
                        }
                        response_line.extend_from_slice(&available[..take]);
                        stdout.consume(take);
                        if newline.is_some() {
                            break;
                        }
                    }
                    String::from_utf8(response_line)
                        .map_err(|error| RuntimeError::InvocationFailed(error.to_string()))
                })();
                let _ = response.send(result);
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestDiscoveryError {
    Unavailable { root: PathBuf, message: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestLoadError {
    ReadFailed(String),
    InvalidJson(String),
    Rejected(PluginError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestLoadFailure {
    pub path: PathBuf,
    pub error: ManifestLoadError,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ManifestLoadReport {
    pub loaded: Vec<PathBuf>,
    pub failures: Vec<ManifestLoadFailure>,
}

/// Reads local JSON manifests without owning plugin lifecycle state. Valid
/// files are handed to `PluginHost::register`; invalid files are reported
/// individually so one broken package cannot hide other installed plugins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonManifestDiscovery {
    root: PathBuf,
}

impl JsonManifestDiscovery {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load_into<H: PluginHost>(
        &self,
        host: &mut H,
    ) -> Result<ManifestLoadReport, ManifestDiscoveryError> {
        let mut paths = Vec::new();
        if !self.root.exists() {
            return Ok(ManifestLoadReport::default());
        }
        collect_manifest_paths(&self.root, &mut paths).map_err(|error| {
            ManifestDiscoveryError::Unavailable {
                root: self.root.clone(),
                message: error.to_string(),
            }
        })?;
        paths.sort();

        let mut report = ManifestLoadReport::default();
        for path in paths {
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    report.failures.push(ManifestLoadFailure {
                        path,
                        error: ManifestLoadError::ReadFailed(error.to_string()),
                    });
                    continue;
                }
            };
            let mut manifest = match serde_json::from_slice::<ExtensionManifest>(&bytes) {
                Ok(manifest) => manifest,
                Err(error) => {
                    report.failures.push(ManifestLoadFailure {
                        path,
                        error: ManifestLoadError::InvalidJson(error.to_string()),
                    });
                    continue;
                }
            };
            resolve_relative_launch_args(&path, &mut manifest);

            let package_root = path.parent().unwrap_or(self.root.as_path());
            match host.register_from_package(manifest, package_root) {
                Ok(()) => report.loaded.push(path),
                Err(error) => report.failures.push(ManifestLoadFailure {
                    path,
                    error: ManifestLoadError::Rejected(error),
                }),
            }
        }

        Ok(report)
    }
}

fn collect_manifest_paths(root: &Path, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
            collect_manifest_paths(&path, paths)?;
        } else if file_type.is_file()
            && !entry.file_name().to_string_lossy().starts_with('.')
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            paths.push(path);
        }
    }
    Ok(())
}

/// Package manifests may use `./` or `../` launch arguments for files shipped
/// beside the manifest. Resolve only those explicitly relative arguments so
/// executable names such as `node` and arbitrary user arguments retain their
/// normal process lookup semantics.
fn resolve_relative_launch_args(manifest_path: &Path, manifest: &mut ExtensionManifest) {
    let Some(launch) = manifest.launch.as_mut() else {
        return;
    };
    let Some(base) = manifest_path.parent() else {
        return;
    };

    for argument in &mut launch.args {
        if argument.starts_with("./") || argument.starts_with("../") {
            *argument = base.join(&*argument).to_string_lossy().into_owned();
        }
    }
}

fn copy_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(source_path, destination_path)?;
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "plugin packages cannot contain symbolic links or special files",
            ));
        }
    }
    Ok(())
}

fn is_safe_extension_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric() || (matches!(character, '.' | '_' | '-') && index > 0)
        })
}

fn is_safe_resource_root(root: &str) -> bool {
    !root.is_empty()
        && !root.starts_with('/')
        && !root.contains('\\')
        && root.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
                })
        })
}

fn is_safe_locale_segment(locale: &str) -> bool {
    !locale.is_empty()
        && locale.len() <= 32
        && locale
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn scan_recovery_paths(
    install_root: &Path,
) -> Result<BTreeMap<ExtensionId, RecoveryPaths>, PluginError> {
    let mut recoveries = BTreeMap::new();
    if !install_root.exists() {
        return Ok(recoveries);
    }

    for entry in
        fs::read_dir(install_root).map_err(|error| PluginError::InstallFailed(error.to_string()))?
    {
        let entry = entry.map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        if !entry
            .file_type()
            .map_err(|error| PluginError::InstallFailed(error.to_string()))?
            .is_dir()
        {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(name) = name.strip_prefix('.') else {
            continue;
        };
        let (id, kind) = if let Some((id, _nonce)) = name.split_once(".staging-") {
            (id, ExtensionInstallRecoveryKind::Staging)
        } else if let Some((id, _nonce)) = name.split_once(".rollback-") {
            (id, ExtensionInstallRecoveryKind::Rollback)
        } else {
            continue;
        };
        if !is_safe_extension_id(id) {
            continue;
        }

        let paths = recoveries.entry(ExtensionId(id.to_owned())).or_default();
        match kind {
            ExtensionInstallRecoveryKind::Staging => paths.staging.push(entry.path()),
            ExtensionInstallRecoveryKind::Rollback => paths.rollback.push(entry.path()),
        }
    }

    for paths in recoveries.values_mut() {
        paths.staging.sort();
        paths.rollback.sort();
    }
    Ok(recoveries)
}

fn recovery_manifest(path: &Path) -> Result<ExtensionManifest, String> {
    let manifest_path = path.join("manifest.json");
    let bytes = fs::read(&manifest_path).map_err(|error| error.to_string())?;
    let mut manifest =
        serde_json::from_slice::<ExtensionManifest>(&bytes).map_err(|error| error.to_string())?;
    resolve_relative_launch_args(&manifest_path, &mut manifest);
    Ok(manifest)
}

fn recovery_log_path(install_root: &Path) -> PathBuf {
    install_root.join(RECOVERY_LOG_FILE)
}

fn recovery_log_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn read_recovery_log(
    install_root: &Path,
) -> Result<Vec<ExtensionInstallRecoveryLogEntry>, PluginError> {
    let path = recovery_log_path(install_root);
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(PluginError::InstallFailed(error.to_string())),
    };
    serde_json::from_slice(&bytes).map_err(|error| PluginError::InstallFailed(error.to_string()))
}

fn append_recovery_log(
    install_root: &Path,
    entry: ExtensionInstallRecoveryLogEntry,
) -> Result<(), PluginError> {
    fs::create_dir_all(install_root)
        .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
    let mut entries = read_recovery_log(install_root).unwrap_or_default();
    entries.push(entry);
    if entries.len() > RECOVERY_LOG_LIMIT {
        let start = entries.len() - RECOVERY_LOG_LIMIT;
        entries.drain(..start);
    }
    let bytes = serde_json::to_vec_pretty(&entries)
        .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
    let temporary = install_root.join(format!(
        ".plugin-recovery-log.json.tmp-{}-{}",
        std::process::id(),
        recovery_log_timestamp_ms()
    ));
    fs::write(&temporary, bytes).map_err(|error| PluginError::InstallFailed(error.to_string()))?;
    if let Err(error) = fs::rename(&temporary, recovery_log_path(install_root)) {
        let _ = fs::remove_file(&temporary);
        return Err(PluginError::InstallFailed(error.to_string()));
    }
    Ok(())
}

fn next_recovery_attempt(
    install_root: &Path,
    id: &ExtensionId,
    action: &ExtensionInstallRecoveryAction,
) -> u32 {
    read_recovery_log(install_root)
        .unwrap_or_default()
        .iter()
        .filter(|entry| &entry.extension_id == id && &entry.action == action)
        .map(|entry| entry.attempt)
        .max()
        .unwrap_or_default()
        .saturating_add(1)
}

pub trait PluginHost {
    fn register(&mut self, manifest: ExtensionManifest) -> Result<(), PluginError>;
    fn register_from_package(
        &mut self,
        manifest: ExtensionManifest,
        _package_root: &Path,
    ) -> Result<(), PluginError> {
        self.register(manifest)
    }
    fn discover(&self) -> Vec<ExtensionSummary>;
    fn projection(&self, id: &ExtensionId) -> Result<ExtensionProjection, PluginError>;
    fn activate(&mut self, id: &ExtensionId) -> Result<(), PluginError>;
    fn disable(&mut self, id: &ExtensionId, reason: DisableReason) -> Result<(), PluginError>;
    fn active_contributions(
        &self,
        id: &ExtensionId,
    ) -> Result<Vec<velvet_plugin_api::ContributionDescriptor>, PluginError>;
    fn job_handles(&self, id: &ExtensionId) -> Result<Vec<PluginJobHandle>, PluginError>;
    fn check_health(&mut self, id: &ExtensionId) -> Result<ExtensionState, PluginError>;
    fn recover(&mut self, id: &ExtensionId) -> Result<(), PluginError>;
    fn invoke_command(
        &mut self,
        id: &ExtensionId,
        command_id: String,
        args: Value,
        context: Value,
    ) -> Result<PluginInvocationResult, PluginError>;
    fn invoke_preview_action(
        &mut self,
        id: &ExtensionId,
        command_id: String,
        args: Value,
        context: PreviewActionContext,
    ) -> Result<PluginInvocationResult, PluginError>;
    fn invoke_preview(
        &mut self,
        id: &ExtensionId,
        contribution_id: String,
        context: Value,
    ) -> Result<PluginInvocationResult, PluginError>;
    fn invoke_preview_augmentation(
        &mut self,
        id: &ExtensionId,
        contribution_id: String,
        context: Value,
    ) -> Result<PluginInvocationResult, PluginError>;
    fn has_permission(&self, id: &ExtensionId, permission: Permission)
        -> Result<bool, PluginError>;
    fn install_from_directory(
        &mut self,
        source_dir: &Path,
        install_root: &Path,
    ) -> Result<ExtensionSummary, PluginError>;
    fn install_from_archive(
        &mut self,
        source_archive: &Path,
        install_root: &Path,
    ) -> Result<ExtensionSummary, PluginError>;
    fn inspect_from_directory(
        &self,
        source_dir: &Path,
    ) -> Result<ExtensionInstallPreview, PluginError>;
    fn inspect_from_archive(
        &self,
        source_archive: &Path,
    ) -> Result<ExtensionInstallPreview, PluginError>;
    fn commit_install(&mut self, id: &ExtensionId, install_root: &Path) -> Result<(), PluginError>;
    fn rollback_install(
        &mut self,
        id: &ExtensionId,
        install_root: &Path,
    ) -> Result<(), PluginError>;
    fn inspect_install_recoveries(
        &self,
        install_root: &Path,
    ) -> Result<Vec<ExtensionInstallRecovery>, PluginError>;
    fn inspect_install_recovery_log(
        &self,
        install_root: &Path,
    ) -> Result<Vec<ExtensionInstallRecoveryLogEntry>, PluginError>;
    fn clear_install_recovery_log(&mut self, install_root: &Path) -> Result<(), PluginError>;
    fn export_install_recovery_log(
        &self,
        install_root: &Path,
        destination: &Path,
    ) -> Result<(), PluginError>;
    fn cleanup_install_recovery_orphans(&mut self, install_root: &Path) -> Result<(), PluginError>;
    fn resolve_install_recovery(
        &mut self,
        id: &ExtensionId,
        action: ExtensionInstallRecoveryAction,
        install_root: &Path,
    ) -> Result<(), PluginError>;
    fn uninstall_from_directory(
        &mut self,
        id: &ExtensionId,
        install_root: &Path,
    ) -> Result<(), PluginError>;
    fn extension_storage(&self, id: &ExtensionId) -> Result<ExtensionStorage, PluginError>;
    fn export_extension_state(
        &self,
        id: &ExtensionId,
        participant: &dyn BackupParticipant,
    ) -> Result<ExtensionStorageExport, PluginError>;
    fn migrate_extension_state(
        &self,
        id: &ExtensionId,
        participant: &dyn MigrationParticipant,
    ) -> Result<ExtensionStorageView, PluginError>;
    fn export_extension_state_from_runtime(
        &mut self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageExport, PluginError>;
    fn migrate_extension_state_from_runtime(
        &mut self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageView, PluginError>;
    fn extension_storage_recovery(
        &self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageRecovery, PluginError>;
    fn restore_extension_state(
        &mut self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageView, PluginError>;
    fn quarantine_extension_state(
        &mut self,
        id: &ExtensionId,
        generation: u64,
    ) -> Result<(), PluginError>;
    fn quarantined_runtime_leases(&self) -> Result<Vec<QuarantinedRuntimeLease>, PluginError>;
    fn close_quarantined_runtime_lease(
        &mut self,
        extension_id: &ExtensionId,
        runtime_nonce: &str,
    ) -> Result<(), PluginError>;
    fn set_runtime_context(
        &mut self,
        host_instance_id: String,
        launch_attempt_id: u64,
        crash_receipt_path: Option<PathBuf>,
    ) {
        let _ = (host_instance_id, launch_attempt_id, crash_receipt_path);
    }
    fn reconcile_runtime_orphans(&mut self) -> Result<Vec<RuntimeOrphanReport>, PluginError> {
        Ok(Vec::new())
    }
    fn drain_diagnostics(&mut self) -> Vec<PluginDiagnosticRecord>;
    fn drain_crash_evidence(&mut self) -> Vec<RuntimeCrashEvidence> {
        Vec::new()
    }
}

pub struct InMemoryPluginHost {
    manifests: BTreeMap<ExtensionId, ExtensionManifest>,
    package_roots: BTreeMap<ExtensionId, PathBuf>,
    active: BTreeSet<ExtensionId>,
    states: BTreeMap<ExtensionId, ExtensionState>,
    processes: BTreeMap<ExtensionId, Arc<Mutex<Box<dyn RuntimeProcess>>>>,
    runtime_nonces: BTreeMap<ExtensionId, String>,
    runtime: Box<dyn PluginRuntime>,
    host_instance_id: Option<String>,
    launch_attempt_id: Option<u64>,
    crash_receipt_path: Option<PathBuf>,
    host_api: VersionRange,
    apps: BTreeMap<String, VersionRange>,
    pending_installs: BTreeMap<ExtensionId, InstallBackup>,
    extension_snapshots: Option<Arc<dyn velvet_persistence::StateSnapshotStore>>,
    runtime_leases: Option<Arc<dyn RuntimeLeaseStore>>,
    orphan_reconciler: Option<Arc<dyn RuntimeOrphanReconciler>>,
    diagnostics: Arc<Mutex<PluginDiagnosticSink>>,
    crash_evidence: VecDeque<RuntimeCrashEvidence>,
}

struct InstallBackup {
    previous_manifest: Option<ExtensionManifest>,
    previous_state: Option<ExtensionState>,
    backup_path: Option<PathBuf>,
}

#[derive(Default)]
struct RecoveryPaths {
    staging: Vec<PathBuf>,
    rollback: Vec<PathBuf>,
}

impl Default for InMemoryPluginHost {
    fn default() -> Self {
        let mut apps = BTreeMap::new();
        apps.insert("asset-manager".into(), VersionRange("1".into()));
        apps.insert("feed-workbench".into(), VersionRange("1".into()));

        Self {
            manifests: BTreeMap::new(),
            package_roots: BTreeMap::new(),
            active: BTreeSet::new(),
            states: BTreeMap::new(),
            processes: BTreeMap::new(),
            runtime_nonces: BTreeMap::new(),
            runtime: Box::new(ProcessPluginRuntime),
            host_instance_id: None,
            launch_attempt_id: None,
            crash_receipt_path: None,
            host_api: VersionRange("1".into()),
            apps,
            pending_installs: BTreeMap::new(),
            extension_snapshots: None,
            runtime_leases: None,
            orphan_reconciler: None,
            diagnostics: Arc::new(Mutex::new(PluginDiagnosticSink::default())),
            crash_evidence: VecDeque::new(),
        }
    }
}

impl InMemoryPluginHost {
    pub fn with_host_api(host_api: VersionRange) -> Self {
        Self {
            host_api,
            ..Self::default()
        }
    }

    pub fn with_runtime(runtime: Box<dyn PluginRuntime>) -> Self {
        Self {
            runtime,
            ..Self::default()
        }
    }

    pub fn set_extension_storage_store(
        &mut self,
        snapshots: Arc<dyn velvet_persistence::StateSnapshotStore>,
    ) {
        self.extension_snapshots = Some(snapshots);
    }

    pub fn set_runtime_lease_store(&mut self, leases: Arc<dyn RuntimeLeaseStore>) {
        self.runtime_leases = Some(leases);
    }

    pub fn set_runtime_context(
        &mut self,
        host_instance_id: String,
        launch_attempt_id: u64,
        crash_receipt_path: Option<PathBuf>,
    ) {
        self.host_instance_id = Some(host_instance_id);
        self.launch_attempt_id = Some(launch_attempt_id);
        self.crash_receipt_path = crash_receipt_path;
    }

    pub fn set_runtime_orphan_reconciler(&mut self, reconciler: Arc<dyn RuntimeOrphanReconciler>) {
        self.orphan_reconciler = Some(reconciler);
    }

    pub fn drain_diagnostics(&mut self) -> Vec<PluginDiagnosticRecord> {
        self.diagnostics
            .lock()
            .map(|mut sink| sink.drain())
            .unwrap_or_default()
    }

    fn record_diagnostic(&self, input: PluginDiagnosticInput) {
        if let Ok(mut sink) = self.diagnostics.lock() {
            sink.record(input);
        }
    }

    fn record_runtime_crash_evidence(&mut self, id: &ExtensionId, message: String) {
        let lease = self
            .runtime_leases
            .as_ref()
            .and_then(|store| store.load().ok())
            .and_then(|leases| {
                leases.into_iter().find(|lease| {
                    &lease.extension_id == id
                        && self
                            .runtime_nonces
                            .get(id)
                            .is_some_and(|nonce| nonce == &lease.runtime_nonce)
                })
            });
        if self.crash_evidence.len() == MAX_PLUGIN_DIAGNOSTICS {
            self.crash_evidence.pop_front();
        }
        self.crash_evidence.push_back(RuntimeCrashEvidence {
            extension_id: id.clone(),
            runtime_nonce: lease.as_ref().map(|lease| lease.runtime_nonce.clone()),
            process_id: lease
                .as_ref()
                .and_then(|lease| lease.process.as_ref().map(|process| process.pid)),
            host_instance_id: lease
                .as_ref()
                .and_then(|lease| lease.host_instance_id.clone()),
            launch_attempt_id: lease.as_ref().and_then(|lease| lease.launch_attempt_id),
            message: sanitize_plugin_diagnostic_message(&message),
            occurred_at_ms: diagnostic_timestamp_ms(),
        });
    }

    pub fn drain_crash_evidence(&mut self) -> Vec<RuntimeCrashEvidence> {
        self.crash_evidence.drain(..).collect()
    }

    pub fn extension_storage(&self, id: &ExtensionId) -> Result<ExtensionStorage, PluginError> {
        let manifest = self
            .manifests
            .get(id)
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        let declaration =
            manifest
                .storage
                .as_ref()
                .ok_or_else(|| PluginError::ExtensionStorage {
                    extension_id: id.clone(),
                    message: "extension did not declare host-owned storage".into(),
                })?;
        let snapshots =
            self.extension_snapshots
                .as_ref()
                .ok_or_else(|| PluginError::ExtensionStorage {
                    extension_id: id.clone(),
                    message: "extension storage is unavailable before persistence is configured"
                        .into(),
                })?;
        ExtensionStorage::new(
            id.clone(),
            declaration.schema_version,
            declaration.quota_bytes,
            snapshots.clone(),
        )
        .map_err(|error| {
            self.record_diagnostic(PluginDiagnosticInput {
                extension_id: id.clone(),
                kind: PluginDiagnosticKind::Storage,
                code: "storage-unavailable".into(),
                message: format!("{error:?}"),
                job_id: None,
                attempt: None,
                retryable: true,
            });
            PluginError::ExtensionStorage {
                extension_id: id.clone(),
                message: format!("{error:?}"),
            }
        })
    }

    pub fn export_extension_state(
        &self,
        id: &ExtensionId,
        participant: &dyn BackupParticipant,
    ) -> Result<ExtensionStorageExport, PluginError> {
        self.extension_storage(id)?
            .export_with(participant)
            .map_err(|error| {
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Storage,
                    code: "backup-export-failed".into(),
                    message: format!("{error:?}"),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                PluginError::ExtensionStorage {
                    extension_id: id.clone(),
                    message: format!("{error:?}"),
                }
            })
    }

    pub fn migrate_extension_state(
        &self,
        id: &ExtensionId,
        participant: &dyn MigrationParticipant,
    ) -> Result<ExtensionStorageView, PluginError> {
        self.extension_storage(id)?
            .migrate_with(participant)
            .map_err(|error| {
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Migration,
                    code: "migration-failed".into(),
                    message: format!("{error:?}"),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                PluginError::ExtensionStorage {
                    extension_id: id.clone(),
                    message: format!("{error:?}"),
                }
            })
    }

    /// Runs a plugin-owned logical backup through the isolated runtime. The
    /// runtime only receives a bounded source snapshot and returns bounded
    /// entries; the host remains the owner of export validation and metadata.
    pub fn export_extension_state_from_runtime(
        &mut self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageExport, PluginError> {
        let storage = self.extension_storage(id)?;
        let source = storage
            .load()
            .map_err(|error| self.storage_error(id, "storage-load-failed", error))?;
        let request = StorageRuntimeRequest {
            source_schema_version: source.schema_version,
            target_schema_version: source.schema_version,
            source_generation: source.generation,
            entries: source.entries.clone(),
        };
        let result = self.invoke_storage_runtime(id, STORAGE_EXPORT_METHOD, &request)?;
        let runtime_result =
            self.decode_storage_runtime_result(id, result, source.schema_version)?;
        storage
            .export_entries(&source, runtime_result.entries)
            .map_err(|error| self.storage_error(id, "backup-export-failed", error))
    }

    /// Runs a plugin-owned schema migration through the isolated runtime. The
    /// returned entries are committed against the exact source generation; a
    /// concurrent writer therefore wins or produces a typed conflict, never a
    /// lost update. No core RecoverySet pointer is touched here.
    pub fn migrate_extension_state_from_runtime(
        &mut self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageView, PluginError> {
        let storage = self.extension_storage(id)?;
        let source = storage
            .load_for_migration()
            .map_err(|error| self.storage_error(id, "migration-source-failed", error))?;
        let request = StorageRuntimeRequest {
            source_schema_version: source.schema_version,
            target_schema_version: storage.schema_version(),
            source_generation: source.generation,
            entries: source.entries.clone(),
        };
        let result = self.invoke_storage_runtime(id, STORAGE_MIGRATE_METHOD, &request)?;
        let runtime_result =
            self.decode_storage_runtime_result(id, result, storage.schema_version())?;
        storage
            .commit_migrated_entries(&source, runtime_result.entries)
            .map_err(|error| self.storage_error(id, "migration-commit-failed", error))
    }

    pub fn extension_storage_recovery(
        &self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageRecovery, PluginError> {
        let manifest = self
            .manifests
            .get(id)
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        let Some(declaration) = manifest.storage.as_ref() else {
            return Ok(ExtensionStorageRecovery {
                extension_id: id.clone(),
                status: ExtensionStorageRecoveryStatus::Unavailable,
                current_generation: None,
                previous_generation: None,
                schema_version: None,
                previous_schema_version: None,
                quarantine_generation: None,
                reason: Some("extension did not declare host-owned storage".into()),
            });
        };
        if self.extension_snapshots.is_none() {
            return Ok(ExtensionStorageRecovery {
                extension_id: id.clone(),
                status: ExtensionStorageRecoveryStatus::Unavailable,
                current_generation: None,
                previous_generation: None,
                schema_version: Some(declaration.schema_version),
                previous_schema_version: None,
                quarantine_generation: None,
                reason: Some(
                    "extension storage is unavailable before persistence is configured".into(),
                ),
            });
        }
        let storage = self.extension_storage(id)?;
        let current = match storage.load() {
            Ok(current) if current.generation == 0 => {
                return Ok(ExtensionStorageRecovery {
                    extension_id: id.clone(),
                    status: ExtensionStorageRecoveryStatus::Empty,
                    current_generation: None,
                    previous_generation: None,
                    schema_version: Some(storage.schema_version()),
                    previous_schema_version: None,
                    quarantine_generation: None,
                    reason: None,
                });
            }
            Ok(current) => current,
            Err(error) => {
                let quarantine_generation = storage
                    .candidate_generations()
                    .ok()
                    .and_then(|generations| generations.last().copied());
                return Ok(ExtensionStorageRecovery {
                    extension_id: id.clone(),
                    status: ExtensionStorageRecoveryStatus::Corrupt,
                    current_generation: quarantine_generation,
                    previous_generation: None,
                    schema_version: Some(storage.schema_version()),
                    previous_schema_version: None,
                    quarantine_generation,
                    reason: Some(storage_recovery_reason(error)),
                });
            }
        };
        let previous = match storage.previous() {
            Ok(previous) => previous,
            Err(error) => {
                return Ok(ExtensionStorageRecovery {
                    extension_id: id.clone(),
                    status: ExtensionStorageRecoveryStatus::Corrupt,
                    current_generation: Some(current.generation),
                    previous_generation: None,
                    schema_version: Some(current.schema_version),
                    previous_schema_version: None,
                    quarantine_generation: Some(current.generation),
                    reason: Some(storage_recovery_reason(error)),
                });
            }
        };
        Ok(ExtensionStorageRecovery {
            extension_id: id.clone(),
            status: ExtensionStorageRecoveryStatus::Available,
            current_generation: Some(current.generation),
            previous_generation: previous.as_ref().map(|view| view.generation),
            schema_version: Some(current.schema_version),
            previous_schema_version: previous.as_ref().map(|view| view.schema_version),
            quarantine_generation: Some(current.generation),
            reason: None,
        })
    }

    pub fn restore_extension_state(
        &mut self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageView, PluginError> {
        let storage = self.extension_storage(id)?;
        let previous = storage
            .previous()
            .map_err(|error| self.storage_error(id, "storage-restore-failed", error))?;
        let Some(previous) = previous else {
            return Err(self.storage_error(
                id,
                "storage-restore-failed",
                ExtensionStorageError::Unavailable(
                    "no previous extension storage generation exists".into(),
                ),
            ));
        };
        if previous.schema_version != storage.schema_version() {
            return Err(self.storage_error(
                id,
                "storage-restore-failed",
                ExtensionStorageError::SchemaMismatch {
                    expected: storage.schema_version(),
                    actual: previous.schema_version,
                },
            ));
        }
        if self.active.contains(id) {
            self.disable(id, DisableReason::User)?;
        }
        let restored = storage
            .restore_previous()
            .map_err(|error| self.storage_error(id, "storage-restore-failed", error))?;
        self.states.insert(id.clone(), ExtensionState::Disabled);
        self.record_diagnostic(PluginDiagnosticInput {
            extension_id: id.clone(),
            kind: PluginDiagnosticKind::Storage,
            code: "storage-restored".into(),
            message: format!(
                "restored extension storage generation {}",
                restored.generation
            ),
            job_id: None,
            attempt: None,
            retryable: false,
        });
        Ok(restored)
    }

    pub fn quarantine_extension_state(
        &mut self,
        id: &ExtensionId,
        generation: u64,
    ) -> Result<(), PluginError> {
        let storage = self.extension_storage(id)?;
        let current_generation = storage
            .candidate_generations()
            .map_err(|error| self.storage_error(id, "storage-quarantine-failed", error))?
            .last()
            .copied()
            .unwrap_or(0);
        if current_generation != generation {
            return Err(self.storage_error(
                id,
                "storage-quarantine-failed",
                ExtensionStorageError::Conflict {
                    expected: generation,
                    actual: current_generation,
                },
            ));
        }
        if self.active.contains(id) {
            self.disable(id, DisableReason::User)?;
        }
        storage
            .quarantine_generation(generation)
            .map_err(|error| self.storage_error(id, "storage-quarantine-failed", error))?;
        self.states.insert(id.clone(), ExtensionState::Disabled);
        self.record_diagnostic(PluginDiagnosticInput {
            extension_id: id.clone(),
            kind: PluginDiagnosticKind::Storage,
            code: "storage-quarantined".into(),
            message: format!("quarantined extension storage generation {generation}"),
            job_id: None,
            attempt: None,
            retryable: false,
        });
        Ok(())
    }

    pub fn quarantined_runtime_leases(&self) -> Result<Vec<QuarantinedRuntimeLease>, PluginError> {
        let Some(store) = self.runtime_leases.as_ref() else {
            return Ok(Vec::new());
        };
        store
            .load()
            .map_err(|error| PluginError::RuntimeLease {
                extension_id: ExtensionId("runtime".into()),
                message: format!("runtime leases could not be loaded: {error:?}"),
            })
            .map(|leases| {
                leases
                    .into_iter()
                    .filter(|lease| matches!(lease.state, RuntimeLeaseState::Quarantined))
                    .map(|lease| QuarantinedRuntimeLease {
                        extension_id: lease.extension_id,
                        runtime_nonce: lease.runtime_nonce,
                        started_at_ms: lease.started_at_ms,
                    })
                    .collect()
            })
    }

    pub fn close_quarantined_runtime_lease(
        &mut self,
        extension_id: &ExtensionId,
        runtime_nonce: &str,
    ) -> Result<(), PluginError> {
        let Some(store) = self.runtime_leases.as_ref() else {
            return Err(PluginError::RuntimeLease {
                extension_id: extension_id.clone(),
                message: "runtime lease store is unavailable".into(),
            });
        };
        let lease = store
            .load()
            .map_err(|error| PluginError::RuntimeLease {
                extension_id: extension_id.clone(),
                message: format!("runtime leases could not be loaded: {error:?}"),
            })?
            .into_iter()
            .find(|lease| {
                lease.extension_id == *extension_id && lease.runtime_nonce == runtime_nonce
            })
            .ok_or_else(|| PluginError::RuntimeLease {
                extension_id: extension_id.clone(),
                message: "quarantined runtime lease was not found".into(),
            })?;
        if !matches!(lease.state, RuntimeLeaseState::Quarantined) {
            return Err(PluginError::RuntimeLease {
                extension_id: extension_id.clone(),
                message: "only a quarantined runtime lease can be closed".into(),
            });
        }
        store
            .close(extension_id, runtime_nonce)
            .map_err(|error| PluginError::RuntimeLease {
                extension_id: extension_id.clone(),
                message: format!("quarantined runtime lease could not be closed: {error:?}"),
            })?;
        if let Some(state) = self.states.get_mut(extension_id) {
            *state = ExtensionState::Failed;
        }
        self.record_diagnostic(PluginDiagnosticInput {
            extension_id: extension_id.clone(),
            kind: PluginDiagnosticKind::Runtime,
            code: "runtime-orphan-quarantine-closed".into(),
            message:
                "quarantined runtime evidence was explicitly closed; no process was terminated"
                    .into(),
            job_id: None,
            attempt: None,
            retryable: false,
        });
        Ok(())
    }

    fn storage_error(
        &self,
        id: &ExtensionId,
        code: &str,
        error: ExtensionStorageError,
    ) -> PluginError {
        self.record_diagnostic(PluginDiagnosticInput {
            extension_id: id.clone(),
            kind: if code.starts_with("migration") {
                PluginDiagnosticKind::Migration
            } else {
                PluginDiagnosticKind::Storage
            },
            code: code.into(),
            message: format!("{error:?}"),
            job_id: None,
            attempt: None,
            retryable: true,
        });
        PluginError::ExtensionStorage {
            extension_id: id.clone(),
            message: format!("{error:?}"),
        }
    }

    fn invoke_storage_runtime(
        &mut self,
        id: &ExtensionId,
        method: &str,
        request: &StorageRuntimeRequest,
    ) -> Result<PluginInvocationResult, PluginError> {
        let state = self
            .states
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        if !self.active.contains(id) {
            return Err(PluginError::InvalidState {
                extension_id: id.clone(),
                state,
            });
        }
        let process = self
            .processes
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::InvalidState {
                extension_id: id.clone(),
                state: ExtensionState::Failed,
            })?;
        let args =
            serde_json::to_value(request).map_err(|error| PluginError::ExtensionStorage {
                extension_id: id.clone(),
                message: format!("storage runtime request could not be encoded: {error}"),
            })?;
        let invocation = PluginInvocation {
            extension_id: id.clone(),
            method: method.into(),
            command_id: "storage".into(),
            args,
            context: serde_json::json!({
                "capability": "extension-storage",
                "operation": method,
            }),
        };
        validate_runtime_payload(
            &invocation.extension_id,
            &invocation.args,
            &invocation.context,
        )?;
        let result = match process.lock() {
            Ok(mut process) => process.invoke(&invocation),
            Err(_) => Err(RuntimeError::InvocationFailed(
                "plugin runtime lock poisoned".into(),
            )),
        };
        match result {
            Ok(result) if result.error.is_none() => Ok(result),
            Ok(result) => {
                let message = result
                    .error
                    .unwrap_or_else(|| "plugin rejected storage operation".into());
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: storage_diagnostic_kind(method),
                    code: "storage-runtime-rejected".into(),
                    message: message.clone(),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                Err(PluginError::ExtensionStorage {
                    extension_id: id.clone(),
                    message,
                })
            }
            Err(error) => {
                let message = runtime_error_message(error);
                Err(self.fail_storage_runtime(id, method, "storage-runtime-failed", message))
            }
        }
    }

    fn decode_storage_runtime_result(
        &mut self,
        id: &ExtensionId,
        result: PluginInvocationResult,
        expected_schema_version: u16,
    ) -> Result<StorageRuntimeResult, PluginError> {
        let payload = result.result.ok_or_else(|| {
            self.fail_storage_runtime(
                id,
                "storage",
                "storage-runtime-invalid-result",
                "plugin returned no storage result".into(),
            )
        })?;
        let runtime_result =
            serde_json::from_value::<StorageRuntimeResult>(payload).map_err(|error| {
                self.fail_storage_runtime(
                    id,
                    "storage",
                    "storage-runtime-invalid-result",
                    format!("plugin storage result is invalid: {error}"),
                )
            })?;
        if runtime_result.schema_version != expected_schema_version {
            return Err(self.fail_storage_runtime(
                id,
                "storage",
                "storage-runtime-schema-mismatch",
                format!(
                    "plugin returned schema {} but host expected {}",
                    runtime_result.schema_version, expected_schema_version
                ),
            ));
        }
        Ok(runtime_result)
    }

    fn fail_storage_runtime(
        &mut self,
        id: &ExtensionId,
        method: &str,
        code: &str,
        message: String,
    ) -> PluginError {
        if let Some(process) = self.processes.remove(id) {
            if let Ok(mut process) = process.lock() {
                let _ = process.shutdown();
            }
        }
        let runtime_nonce = self.runtime_nonces.remove(id);
        self.close_runtime_lease(id, runtime_nonce.as_deref());
        self.active.remove(id);
        self.states.insert(id.clone(), ExtensionState::Failed);
        self.record_diagnostic(PluginDiagnosticInput {
            extension_id: id.clone(),
            kind: storage_diagnostic_kind(method),
            code: code.into(),
            message: message.clone(),
            job_id: None,
            attempt: None,
            retryable: true,
        });
        PluginError::ExtensionStorage {
            extension_id: id.clone(),
            message,
        }
    }

    fn runtime_storage_migration_required(
        &self,
        manifest: &ExtensionManifest,
    ) -> Result<bool, PluginError> {
        if manifest.runtime == RuntimeKind::BuiltIn
            || manifest.storage.is_none()
            || self.extension_snapshots.is_none()
        {
            return Ok(false);
        }
        let storage = self.extension_storage(&manifest.id)?;
        let source = storage
            .load_for_migration()
            .map_err(|error| self.storage_error(&manifest.id, "migration-source-failed", error))?;
        Ok(source.generation > 0 && source.schema_version != storage.schema_version())
    }

    fn abort_runtime_activation(&mut self, id: &ExtensionId) {
        if let Some(process) = self.processes.remove(id) {
            if let Ok(mut process) = process.lock() {
                let _ = process.shutdown();
            }
        }
        let runtime_nonce = self.runtime_nonces.remove(id);
        self.close_runtime_lease(id, runtime_nonce.as_deref());
        self.active.remove(id);
        self.states.insert(id.clone(), ExtensionState::Failed);
    }

    pub fn register_app(&mut self, app_id: String, api: VersionRange) {
        self.apps.insert(app_id, api);
    }

    pub fn reconcile_runtime_orphans(&mut self) -> Result<Vec<RuntimeOrphanReport>, PluginError> {
        let (Some(store), Some(reconciler)) = (
            self.runtime_leases.as_ref(),
            self.orphan_reconciler.as_ref(),
        ) else {
            return Ok(Vec::new());
        };
        let leases = store.load().map_err(|error| PluginError::RuntimeLease {
            extension_id: ExtensionId("runtime".into()),
            message: format!("runtime leases could not be loaded: {error:?}"),
        })?;
        let mut reports = Vec::with_capacity(leases.len());
        for lease in leases {
            if matches!(lease.state, RuntimeLeaseState::Quarantined) {
                if let Some(state) = self.states.get_mut(&lease.extension_id) {
                    *state = ExtensionState::Failed;
                }
                reports.push(RuntimeOrphanReport {
                    extension_id: lease.extension_id,
                    runtime_nonce: lease.runtime_nonce,
                    observation: RuntimeOrphanObservation::IdentityUncertain,
                    lease_closed: false,
                    quarantined: true,
                });
                continue;
            }
            let observation = reconciler.observe(&lease);
            let mut lease_closed = false;
            let mut quarantined = false;
            match observation {
                RuntimeOrphanObservation::Absent | RuntimeOrphanObservation::DifferentProcess => {
                    lease_closed = store
                        .close(&lease.extension_id, &lease.runtime_nonce)
                        .is_ok();
                    if let Some(state) = self.states.get_mut(&lease.extension_id) {
                        *state = ExtensionState::Failed;
                    }
                    self.record_diagnostic(PluginDiagnosticInput {
                        extension_id: lease.extension_id.clone(),
                        kind: PluginDiagnosticKind::Runtime,
                        code: "runtime-orphan-reconciled".into(),
                        message: format!("orphan lease observed as {observation:?}"),
                        job_id: None,
                        attempt: None,
                        retryable: true,
                    });
                }
                RuntimeOrphanObservation::Matching => match reconciler.terminate(&lease) {
                    Ok(()) => {
                        lease_closed = store
                            .close(&lease.extension_id, &lease.runtime_nonce)
                            .is_ok();
                        if let Some(state) = self.states.get_mut(&lease.extension_id) {
                            *state = ExtensionState::Failed;
                        }
                        self.record_diagnostic(PluginDiagnosticInput {
                            extension_id: lease.extension_id.clone(),
                            kind: PluginDiagnosticKind::Runtime,
                            code: "runtime-orphan-terminated".into(),
                            message: "confirmed orphan runtime was terminated".into(),
                            job_id: None,
                            attempt: None,
                            retryable: true,
                        });
                    }
                    Err(error) => {
                        quarantined = store
                            .quarantine(&lease.extension_id, &lease.runtime_nonce)
                            .is_ok();
                        if let Some(state) = self.states.get_mut(&lease.extension_id) {
                            *state = ExtensionState::Failed;
                        }
                        self.record_diagnostic(PluginDiagnosticInput {
                            extension_id: lease.extension_id.clone(),
                            kind: PluginDiagnosticKind::Runtime,
                            code: "runtime-orphan-quarantined".into(),
                            message: format!("confirmed orphan could not be terminated: {error:?}"),
                            job_id: None,
                            attempt: None,
                            retryable: true,
                        });
                    }
                },
                RuntimeOrphanObservation::IdentityUncertain => {
                    quarantined = store
                        .quarantine(&lease.extension_id, &lease.runtime_nonce)
                        .is_ok();
                    if let Some(state) = self.states.get_mut(&lease.extension_id) {
                        *state = ExtensionState::Failed;
                    }
                    self.record_diagnostic(PluginDiagnosticInput {
                        extension_id: lease.extension_id.clone(),
                        kind: PluginDiagnosticKind::Runtime,
                        code: "runtime-orphan-identity-uncertain".into(),
                        message: "runtime identity could not be verified; no process was killed"
                            .into(),
                        job_id: None,
                        attempt: None,
                        retryable: true,
                    });
                }
            }
            reports.push(RuntimeOrphanReport {
                extension_id: lease.extension_id,
                runtime_nonce: lease.runtime_nonce,
                observation,
                lease_closed,
                quarantined,
            });
        }
        Ok(reports)
    }

    fn runtime_lease_enabled(&self, manifest: &ExtensionManifest) -> bool {
        manifest.runtime != RuntimeKind::BuiltIn
            && self.runtime.requires_package_launch_context()
            && self.runtime_leases.is_some()
    }

    fn close_runtime_lease(&self, id: &ExtensionId, runtime_nonce: Option<&str>) {
        let (Some(store), Some(runtime_nonce)) = (self.runtime_leases.as_ref(), runtime_nonce)
        else {
            return;
        };
        if let Err(error) = store.close(id, runtime_nonce) {
            self.record_diagnostic(PluginDiagnosticInput {
                extension_id: id.clone(),
                kind: PluginDiagnosticKind::Runtime,
                code: "runtime-lease-close-failed".into(),
                message: format!("runtime lease could not be closed: {error:?}"),
                job_id: None,
                attempt: None,
                retryable: true,
            });
        }
    }

    fn register_manifest(
        &mut self,
        manifest: ExtensionManifest,
        package_root: Option<&Path>,
    ) -> Result<(), PluginError> {
        let manifest = self.normalize_manifest(manifest)?;
        if self.manifests.contains_key(&manifest.id) {
            return Err(PluginError::AlreadyRegistered(manifest.id));
        }

        if package_root.is_none()
            && manifest.runtime != RuntimeKind::BuiltIn
            && self.runtime.requires_package_launch_context()
        {
            return Err(PluginError::InvalidContribution(
                "external runtimes must be discovered or installed from a committed package".into(),
            ));
        }

        if let Some(package_root) = package_root {
            if manifest.runtime != RuntimeKind::BuiltIn {
                let context = RuntimeLaunchContext::package(package_root);
                if let Err(error) = validate_launch_context(&manifest, &context) {
                    return Err(PluginError::InvalidContribution(runtime_error_message(
                        error,
                    )));
                }
            }
        }

        self.validate_manifest(&manifest)?;
        if let Some(package_root) = package_root {
            self.package_roots
                .insert(manifest.id.clone(), package_root.to_path_buf());
        } else {
            self.package_roots.remove(&manifest.id);
        }
        self.states
            .insert(manifest.id.clone(), ExtensionState::Registered);
        self.manifests.insert(manifest.id.clone(), manifest);
        Ok(())
    }

    fn validate_manifest(&self, manifest: &ExtensionManifest) -> Result<(), PluginError> {
        validate_reserved_metadata(&manifest.extensions)
            .map_err(PluginError::InvalidContribution)?;
        if manifest.manifest_schema != MANIFEST_SCHEMA_VERSION {
            return Err(PluginError::InvalidContribution(format!(
                "manifest schema {} is unsupported; expected {}",
                manifest.manifest_schema, MANIFEST_SCHEMA_VERSION
            )));
        }

        if !is_safe_extension_id(&manifest.id.0) {
            return Err(PluginError::InvalidContribution(format!(
                "extension id {} is invalid",
                manifest.id.0
            )));
        }

        if let Some(storage) = &manifest.storage {
            if storage.schema_version == 0 {
                return Err(PluginError::InvalidContribution(
                    "storage schemaVersion must be non-zero".into(),
                ));
            }
            if storage.quota_bytes == 0 || storage.quota_bytes > MAX_EXTENSION_STORAGE_BYTES as u64
            {
                return Err(PluginError::InvalidContribution(format!(
                    "storage quotaBytes must be between 1 and {MAX_EXTENSION_STORAGE_BYTES}"
                )));
            }
        }

        if manifest.extension_kind.is_empty() {
            return Err(PluginError::InvalidContribution(
                "extensionKind must declare at least one kind".into(),
            ));
        }

        if manifest.required_capabilities.iter().any(|required| {
            manifest
                .optional_capabilities
                .iter()
                .any(|optional| optional == required)
        }) {
            return Err(PluginError::InvalidContribution(
                "a capability cannot be both required and optional".into(),
            ));
        }

        if manifest
            .extension_dependencies
            .iter()
            .any(|dependency| dependency == &manifest.id)
        {
            return Err(PluginError::InvalidContribution(
                "an extension cannot depend on itself".into(),
            ));
        }

        match manifest.runtime {
            RuntimeKind::BuiltIn if manifest.launch.is_some() => {
                return Err(PluginError::InvalidContribution(
                    "built-in extensions cannot declare a launch process".into(),
                ));
            }
            RuntimeKind::Wasm | RuntimeKind::Sidecar if manifest.launch.is_none() => {
                return Err(PluginError::InvalidContribution(
                    "isolated extensions must declare a launch process".into(),
                ));
            }
            _ => {}
        }

        if let Some(localization) = &manifest.localization {
            if localization.default_locale.trim().is_empty()
                || localization.supported_locales.is_empty()
                || !localization
                    .supported_locales
                    .iter()
                    .any(|locale| locale == &localization.default_locale)
                || localization.resource_root.trim().is_empty()
                || localization.resource_root.starts_with('/')
                || localization
                    .resource_root
                    .split('/')
                    .any(|segment| segment == "..")
                || localization.bundle_schema == 0
            {
                return Err(PluginError::InvalidContribution(
                    "localization metadata is invalid".into(),
                ));
            }
        }

        if !api_compatible(&manifest.host_api, &self.host_api) {
            return Err(PluginError::IncompatibleHostApi(manifest.id.clone()));
        }

        for permission_set in &manifest.permission_sets {
            for permission in &permission_set.permissions {
                if !permission_allowed(permission_set.zone.clone(), permission) {
                    return Err(PluginError::PermissionDenied {
                        extension_id: manifest.id.clone(),
                        zone: permission_set.zone.clone(),
                        permission: permission.clone(),
                    });
                }
            }
        }

        let mut app_ids = BTreeSet::new();
        for contribution in &manifest.contributions {
            let expected_namespace = namespace_for(&contribution.scope, &manifest.id);
            if contribution
                .namespace
                .as_deref()
                .is_some_and(|namespace| namespace != expected_namespace)
            {
                return Err(PluginError::InvalidNamespace(format!(
                    "{} must use namespace {}",
                    contribution.id, expected_namespace
                )));
            }

            if matches!(
                contribution.kind,
                ContributionKind::View
                    | ContributionKind::ViewContainer
                    | ContributionKind::ActivityBarEntry
            ) && contribution.zone.is_none()
            {
                return Err(PluginError::InvalidContribution(format!(
                    "contribution {} must declare a zone",
                    contribution.id
                )));
            }

            if matches!(contribution.scope, ContributionScope::Capability)
                && (contribution.container.is_some() || contribution.zone.is_some())
            {
                return Err(PluginError::InvalidContribution(format!(
                    "capability {} cannot target a host container or zone",
                    contribution.id
                )));
            }

            match contribution.kind {
                ContributionKind::Job => {
                    if !matches!(contribution.scope, ContributionScope::Capability) {
                        return Err(PluginError::InvalidContribution(format!(
                            "job {} must be a capability contribution",
                            contribution.id
                        )));
                    }
                }
                ContributionKind::PreviewProvider => {
                    let Some(preview) = contribution.preview.as_ref() else {
                        return Err(PluginError::InvalidContribution(format!(
                            "preview provider {} must declare preview metadata",
                            contribution.id
                        )));
                    };
                    if !matches!(contribution.scope, ContributionScope::Capability)
                        || preview.slot.is_some()
                        || preview.content_kinds.is_empty()
                        || preview.intents.is_empty()
                        || preview.surfaces.is_empty()
                    {
                        return Err(PluginError::InvalidContribution(format!(
                            "preview provider {} must be a capability with non-empty selectors and no slot",
                            contribution.id
                        )));
                    }
                }
                ContributionKind::PreviewAugmentation | ContributionKind::Inspector => {
                    let Some(preview) = contribution.preview.as_ref() else {
                        return Err(PluginError::InvalidContribution(format!(
                            "preview augmentation {} must declare preview metadata",
                            contribution.id
                        )));
                    };
                    if !matches!(contribution.scope, ContributionScope::Capability)
                        || preview.slot.is_none()
                        || preview.content_kinds.is_empty()
                        || preview.intents.is_empty()
                        || preview.surfaces.is_empty()
                    {
                        return Err(PluginError::InvalidContribution(format!(
                            "preview augmentation {} must be a capability with a slot",
                            contribution.id
                        )));
                    }
                }
                _ if contribution.preview.is_some() => {
                    return Err(PluginError::InvalidContribution(format!(
                        "contribution {} cannot declare preview metadata",
                        contribution.id
                    )));
                }
                _ => {}
            }

            if matches!(contribution.scope, ContributionScope::WorkbenchApp { .. })
                && matches!(
                    contribution.container,
                    Some(HostContainer::AuxiliarySidebar | HostContainer::OperationsPanel)
                )
            {
                return Err(PluginError::InvalidContribution(format!(
                    "app contribution {} cannot claim a global container",
                    contribution.id
                )));
            }

            match (
                &contribution.kind,
                &contribution.scope,
                &contribution.container,
            ) {
                (
                    ContributionKind::CompanionView,
                    ContributionScope::Workbench,
                    Some(HostContainer::AuxiliarySidebar),
                )
                | (
                    ContributionKind::OperationView,
                    ContributionScope::Workbench,
                    Some(HostContainer::OperationsPanel),
                ) => {}
                (ContributionKind::CompanionView, _, _)
                | (ContributionKind::OperationView, _, _) => {
                    return Err(PluginError::InvalidContribution(format!(
                        "{} must use its reserved global container",
                        contribution.id
                    )));
                }
                _ => {}
            }

            if let ContributionScope::WorkbenchApp { app_id } = &contribution.scope {
                app_ids.insert(app_id.clone());
            }
        }

        if app_ids.len() > 1 {
            return Err(PluginError::InvalidContribution(
                "one extension cannot target multiple WorkbenchApp namespaces".into(),
            ));
        }

        if let Some(target) = &manifest.target_app {
            self.validate_target(target, &app_ids)?;
        } else if !app_ids.is_empty() {
            return Err(PluginError::InvalidContribution(
                "WorkbenchApp contributions require an explicit target app".into(),
            ));
        }

        Ok(())
    }

    fn normalize_manifest(
        &self,
        mut manifest: ExtensionManifest,
    ) -> Result<ExtensionManifest, PluginError> {
        for contribution in &mut manifest.contributions {
            let expected_namespace = namespace_for(&contribution.scope, &manifest.id);
            if contribution
                .namespace
                .as_deref()
                .is_some_and(|namespace| namespace != expected_namespace)
            {
                return Err(PluginError::InvalidNamespace(format!(
                    "{} must use namespace {}",
                    contribution.id, expected_namespace
                )));
            }
            contribution.namespace = Some(expected_namespace);
        }
        self.validate_manifest(&manifest)?;
        Ok(manifest)
    }

    fn validate_target(
        &self,
        target: &AppTarget,
        contribution_apps: &BTreeSet<String>,
    ) -> Result<(), PluginError> {
        let Some(app_api) = self.apps.get(&target.app_id) else {
            return Err(PluginError::UnknownWorkbenchApp(target.app_id.clone()));
        };

        if !api_compatible(&target.api, app_api) {
            return Err(PluginError::InvalidContribution(format!(
                "WorkbenchApp {} does not satisfy target API {}",
                target.app_id, target.api.0
            )));
        }

        if let Some(contribution_app) = contribution_apps.iter().next() {
            if contribution_app != &target.app_id {
                return Err(PluginError::InvalidContribution(format!(
                    "contribution namespace targets {}, manifest targets {}",
                    contribution_app, target.app_id
                )));
            }
        }

        Ok(())
    }

    fn remove_registration(&mut self, id: &ExtensionId) {
        let runtime_nonce = self.runtime_nonces.remove(id);
        self.close_runtime_lease(id, runtime_nonce.as_deref());
        self.manifests.remove(id);
        self.package_roots.remove(id);
        self.active.remove(id);
        self.states.remove(id);
        self.processes.remove(id);
    }

    fn restore_state(&mut self, id: &ExtensionId, state: ExtensionState) {
        if state != ExtensionState::Active {
            self.states.insert(id.clone(), state);
        }
    }
}

impl InMemoryPluginHost {
    fn invoke_declared(
        &mut self,
        id: &ExtensionId,
        contribution_id: String,
        expected_kind: ContributionKind,
        method: &str,
        args: Value,
        context: Value,
    ) -> Result<PluginInvocationResult, PluginError> {
        let state = self
            .states
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        if !self.active.contains(id) {
            return Err(PluginError::InvalidState {
                extension_id: id.clone(),
                state,
            });
        }

        let manifest = self
            .manifests
            .get(id)
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        if !manifest.contributions.iter().any(|contribution| {
            contribution.id == contribution_id && contribution.kind == expected_kind
        }) {
            return Err(PluginError::InvalidContribution(format!(
                "contribution {} is not declared by extension {}",
                contribution_id, id.0
            )));
        }

        let invocation = PluginInvocation {
            extension_id: id.clone(),
            method: method.into(),
            command_id: contribution_id,
            args,
            context,
        };
        validate_runtime_payload(
            &invocation.extension_id,
            &invocation.args,
            &invocation.context,
        )?;
        let process = self
            .processes
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::InvalidState {
                extension_id: id.clone(),
                state: ExtensionState::Failed,
            })?;
        let result = process
            .lock()
            .map_err(|_| {
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Invocation,
                    code: "invocation-lock-poisoned".into(),
                    message: "plugin runtime lock poisoned".into(),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                PluginError::RuntimeInvocation {
                    extension_id: id.clone(),
                    message: "plugin runtime lock poisoned".into(),
                }
            })?
            .invoke(&invocation);

        match result {
            Ok(result) => {
                if let Some(error) = &result.error {
                    self.record_diagnostic(PluginDiagnosticInput {
                        extension_id: id.clone(),
                        kind: PluginDiagnosticKind::Invocation,
                        code: "invocation-rejected".into(),
                        message: error.clone(),
                        job_id: None,
                        attempt: None,
                        retryable: true,
                    });
                    return Err(PluginError::RuntimeInvocation {
                        extension_id: id.clone(),
                        message: error.clone(),
                    });
                }
                Ok(result)
            }
            Err(RuntimeError::InvocationRejected(message)) => {
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Invocation,
                    code: "invocation-rejected".into(),
                    message: message.clone(),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                Err(PluginError::RuntimeInvocation {
                    extension_id: id.clone(),
                    message,
                })
            }
            Err(error) => {
                let message = runtime_error_message(error);
                if let Some(process) = self.processes.remove(id) {
                    if let Ok(mut process) = process.lock() {
                        let _ = process.shutdown();
                    }
                }
                let runtime_nonce = self.runtime_nonces.remove(id);
                self.close_runtime_lease(id, runtime_nonce.as_deref());
                self.active.remove(id);
                self.states.insert(id.clone(), ExtensionState::Failed);
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Runtime,
                    code: "invocation-runtime-failed".into(),
                    message: message.clone(),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                Err(PluginError::RuntimeInvocation {
                    extension_id: id.clone(),
                    message,
                })
            }
        }
    }

    fn invoke_preview_contribution(
        &mut self,
        id: &ExtensionId,
        contribution_id: String,
        expected_kinds: &[ContributionKind],
        method: &str,
        context: Value,
        label: &str,
    ) -> Result<PluginInvocationResult, PluginError> {
        let manifest = self
            .manifests
            .get(id)
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        let contribution = manifest
            .contributions
            .iter()
            .find(|contribution| contribution.id == contribution_id)
            .ok_or_else(|| {
                PluginError::InvalidContribution(format!(
                    "{label} {contribution_id} is not declared by extension {}",
                    id.0,
                ))
            })?;
        if !expected_kinds.contains(&contribution.kind) || contribution.preview.is_none() {
            return Err(PluginError::InvalidContribution(format!(
                "{} is not a {label} contribution",
                contribution_id,
            )));
        }
        if !manifest
            .permission_sets
            .iter()
            .flat_map(|set| set.permissions.iter())
            .any(|permission| permission == &Permission::PreviewRender)
        {
            return Err(PluginError::PermissionDenied {
                extension_id: id.clone(),
                zone: WorkbenchZone::Contextual,
                permission: Permission::PreviewRender,
            });
        }

        self.invoke_declared(
            id,
            contribution_id,
            contribution.kind.clone(),
            method,
            Value::Null,
            context,
        )
    }

    fn invoke_preview_action(
        &mut self,
        id: &ExtensionId,
        command_id: String,
        args: Value,
        context: PreviewActionContext,
    ) -> Result<PluginInvocationResult, PluginError> {
        if !self.has_permission(id, Permission::PreviewRender)? {
            return Err(PluginError::PermissionDenied {
                extension_id: id.clone(),
                zone: WorkbenchZone::Contextual,
                permission: Permission::PreviewRender,
            });
        }
        let context = serde_json::to_value(context).map_err(|error| {
            PluginError::InvalidContribution(format!(
                "preview action context could not be encoded: {error}"
            ))
        })?;
        self.invoke_declared(
            id,
            command_id,
            ContributionKind::Command,
            "preview.action",
            args,
            context,
        )
    }

    fn has_permission(
        &self,
        id: &ExtensionId,
        permission: Permission,
    ) -> Result<bool, PluginError> {
        let manifest = self
            .manifests
            .get(id)
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        Ok(manifest
            .permission_sets
            .iter()
            .flat_map(|set| set.permissions.iter())
            .any(|candidate| candidate == &permission))
    }
}

impl PluginHost for InMemoryPluginHost {
    fn register(&mut self, manifest: ExtensionManifest) -> Result<(), PluginError> {
        self.register_manifest(manifest, None)
    }

    fn register_from_package(
        &mut self,
        manifest: ExtensionManifest,
        package_root: &Path,
    ) -> Result<(), PluginError> {
        self.register_manifest(manifest, Some(package_root))
    }

    fn discover(&self) -> Vec<ExtensionSummary> {
        self.manifests
            .values()
            .map(|manifest| ExtensionSummary {
                id: manifest.id.clone(),
                version: manifest.version.clone(),
                active: self.active.contains(&manifest.id),
                state: self
                    .states
                    .get(&manifest.id)
                    .cloned()
                    .unwrap_or(ExtensionState::Registered),
            })
            .collect()
    }

    fn projection(&self, id: &ExtensionId) -> Result<ExtensionProjection, PluginError> {
        let Some(manifest) = self.manifests.get(id) else {
            return Err(PluginError::NotFound(id.clone()));
        };

        let summary = self
            .discover()
            .into_iter()
            .find(|candidate| &candidate.id == id)
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        Ok(ExtensionProjection {
            summary,
            manifest: manifest.clone(),
        })
    }

    fn activate(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        let Some(manifest) = self.manifests.get(id).cloned() else {
            return Err(PluginError::NotFound(id.clone()));
        };

        if self.active.contains(id) {
            return Ok(());
        }

        let storage_migration_required = match self.runtime_storage_migration_required(&manifest) {
            Ok(required) => required,
            Err(error) => {
                self.states.insert(id.clone(), ExtensionState::Failed);
                return Err(error);
            }
        };

        self.states.insert(id.clone(), ExtensionState::Starting);
        let permissions = PermissionGrant::from_manifest(&manifest);
        let package_root = self.package_roots.get(id).cloned();
        let runtime_lease = if self.runtime_lease_enabled(&manifest) {
            let runtime_nonce = next_runtime_nonce(id);
            let lease = RuntimeLease::starting(
                manifest.id.clone(),
                manifest.version.clone(),
                package_root.clone(),
                runtime_nonce.clone(),
            );
            let lease = if let (Some(instance_id), Some(attempt_id)) =
                (self.host_instance_id.clone(), self.launch_attempt_id)
            {
                lease.with_host_context(instance_id, attempt_id)
            } else {
                lease
            };
            let store = self
                .runtime_leases
                .as_ref()
                .expect("runtime lease is enabled only with a store")
                .clone();
            if let Err(error) = store.publish(lease.clone()) {
                self.states.insert(id.clone(), ExtensionState::Failed);
                let message = format!("runtime lease could not be published: {error:?}");
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Activation,
                    code: "runtime-lease-publish-failed".into(),
                    message: message.clone(),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                return Err(PluginError::RuntimeLease {
                    extension_id: id.clone(),
                    message,
                });
            }
            Some((store, lease))
        } else {
            None
        };
        let launch_context = package_root
            .as_deref()
            .map(RuntimeLaunchContext::package)
            .unwrap_or_default();
        let launch_context = runtime_lease
            .as_ref()
            .map(|(_, lease)| {
                launch_context
                    .clone()
                    .with_runtime_nonce(lease.runtime_nonce.clone())
            })
            .unwrap_or(launch_context);
        let launch_context = if let (Some(instance_id), Some(attempt_id)) =
            (self.host_instance_id.clone(), self.launch_attempt_id)
        {
            launch_context
                .with_host_context(instance_id, attempt_id)
                .with_crash_receipt_path(
                    self.crash_receipt_path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("crash-receipts.json")),
                )
        } else {
            launch_context
        };
        match self
            .runtime
            .start_with_context(&manifest, &permissions, &launch_context)
        {
            Ok(mut process) => {
                if let Some((store, lease)) = runtime_lease {
                    let Some(identity) = process.identity() else {
                        let _ = process.shutdown();
                        let _ = store.close(id, &lease.runtime_nonce);
                        self.states.insert(id.clone(), ExtensionState::Failed);
                        let message =
                            "external runtime did not provide a process identity for its lease"
                                .to_owned();
                        self.record_diagnostic(PluginDiagnosticInput {
                            extension_id: id.clone(),
                            kind: PluginDiagnosticKind::Activation,
                            code: "runtime-lease-identity-missing".into(),
                            message: message.clone(),
                            job_id: None,
                            attempt: None,
                            retryable: true,
                        });
                        return Err(PluginError::RuntimeLease {
                            extension_id: id.clone(),
                            message,
                        });
                    };
                    let runtime_nonce = lease.runtime_nonce.clone();
                    if let Err(error) = store.publish(lease.running(identity)) {
                        let _ = process.shutdown();
                        let _ = store.close(id, &runtime_nonce);
                        self.states.insert(id.clone(), ExtensionState::Failed);
                        let message = format!("runtime lease could not be committed: {error:?}");
                        self.record_diagnostic(PluginDiagnosticInput {
                            extension_id: id.clone(),
                            kind: PluginDiagnosticKind::Activation,
                            code: "runtime-lease-commit-failed".into(),
                            message: message.clone(),
                            job_id: None,
                            attempt: None,
                            retryable: true,
                        });
                        return Err(PluginError::RuntimeLease {
                            extension_id: id.clone(),
                            message,
                        });
                    }
                    self.runtime_nonces.insert(id.clone(), runtime_nonce);
                }
                self.processes
                    .insert(id.clone(), Arc::new(Mutex::new(process)));
                self.active.insert(id.clone());
                self.states.insert(id.clone(), ExtensionState::Active);
                if storage_migration_required {
                    if let Err(error) = self.migrate_extension_state_from_runtime(id) {
                        if self.active.contains(id) {
                            self.abort_runtime_activation(id);
                        }
                        return Err(error);
                    }
                }
                Ok(())
            }
            Err(error) => {
                if let Some((store, lease)) = runtime_lease {
                    if let Err(close_error) = store.close(id, &lease.runtime_nonce) {
                        self.record_diagnostic(PluginDiagnosticInput {
                            extension_id: id.clone(),
                            kind: PluginDiagnosticKind::Runtime,
                            code: "runtime-lease-close-failed".into(),
                            message: format!("failed to close start lease: {close_error:?}"),
                            job_id: None,
                            attempt: None,
                            retryable: true,
                        });
                    }
                }
                self.states.insert(id.clone(), ExtensionState::Failed);
                let message = runtime_error_message(error);
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Activation,
                    code: "activation-failed".into(),
                    message: message.clone(),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                Err(PluginError::RuntimeStart {
                    extension_id: id.clone(),
                    message,
                })
            }
        }
    }

    fn disable(&mut self, id: &ExtensionId, _reason: DisableReason) -> Result<(), PluginError> {
        if !self.manifests.contains_key(id) {
            return Err(PluginError::NotFound(id.clone()));
        }

        self.active.remove(id);
        let shutdown_error = self
            .processes
            .remove(id)
            .and_then(|process| process.lock().ok()?.shutdown().err());
        let runtime_nonce = self.runtime_nonces.remove(id);
        self.close_runtime_lease(id, runtime_nonce.as_deref());
        self.states.insert(id.clone(), ExtensionState::Disabled);
        if let Some(error) = shutdown_error {
            let message = runtime_error_message(error);
            self.record_diagnostic(PluginDiagnosticInput {
                extension_id: id.clone(),
                kind: PluginDiagnosticKind::Shutdown,
                code: "shutdown-failed".into(),
                message: message.clone(),
                job_id: None,
                attempt: None,
                retryable: true,
            });
            return Err(PluginError::RuntimeShutdown {
                extension_id: id.clone(),
                message,
            });
        }
        Ok(())
    }

    fn active_contributions(
        &self,
        id: &ExtensionId,
    ) -> Result<Vec<velvet_plugin_api::ContributionDescriptor>, PluginError> {
        let Some(manifest) = self.manifests.get(id) else {
            return Err(PluginError::NotFound(id.clone()));
        };

        if !self.active.contains(id) {
            return Ok(Vec::new());
        }

        Ok(manifest.contributions.clone())
    }

    fn job_handles(&self, id: &ExtensionId) -> Result<Vec<PluginJobHandle>, PluginError> {
        let state = self
            .states
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        if !self.active.contains(id) {
            return Err(PluginError::InvalidState {
                extension_id: id.clone(),
                state,
            });
        }
        let process = self
            .processes
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::InvalidState {
                extension_id: id.clone(),
                state: ExtensionState::Failed,
            })?;
        let manifest = self
            .manifests
            .get(id)
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        let job_kinds = manifest
            .contributions
            .iter()
            .filter(|contribution| contribution.kind == ContributionKind::Job)
            .map(|contribution| contribution.id.clone())
            .collect::<Vec<_>>();
        if job_kinds.is_empty() {
            return Ok(Vec::new());
        }
        if !manifest
            .permission_sets
            .iter()
            .flat_map(|set| set.permissions.iter())
            .any(|permission| permission == &Permission::JobsCreate)
        {
            return Err(PluginError::PermissionDenied {
                extension_id: id.clone(),
                zone: WorkbenchZone::Operations,
                permission: Permission::JobsCreate,
            });
        }
        Ok(job_kinds
            .into_iter()
            .map(|job_kind| PluginJobHandle {
                extension_id: id.clone(),
                job_kind,
                process: process.clone(),
                diagnostics: self.diagnostics.clone(),
            })
            .collect())
    }

    fn check_health(&mut self, id: &ExtensionId) -> Result<ExtensionState, PluginError> {
        if !self.manifests.contains_key(id) {
            return Err(PluginError::NotFound(id.clone()));
        }

        let Some(process) = self.processes.get(id).cloned() else {
            return Ok(self
                .states
                .get(id)
                .cloned()
                .unwrap_or(ExtensionState::Registered));
        };

        let health = process
            .lock()
            .map_err(|_| {
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Runtime,
                    code: "health-lock-poisoned".into(),
                    message: "plugin runtime lock poisoned".into(),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                PluginError::RuntimeCrashed {
                    extension_id: id.clone(),
                    message: "plugin runtime lock poisoned".into(),
                }
            })?
            .poll();
        match health {
            RuntimeHealth::Running => Ok(ExtensionState::Active),
            RuntimeHealth::Crashed(message) => {
                self.record_runtime_crash_evidence(id, message.clone());
                if let Some(process) = self.processes.remove(id) {
                    if let Ok(mut process) = process.lock() {
                        let _ = process.shutdown();
                    }
                }
                let runtime_nonce = self.runtime_nonces.remove(id);
                self.close_runtime_lease(id, runtime_nonce.as_deref());
                self.active.remove(id);
                self.states.insert(id.clone(), ExtensionState::Failed);
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Runtime,
                    code: "runtime-crashed".into(),
                    message: message.clone(),
                    job_id: None,
                    attempt: None,
                    retryable: true,
                });
                Err(PluginError::RuntimeCrashed {
                    extension_id: id.clone(),
                    message,
                })
            }
        }
    }

    fn recover(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        let state = self
            .states
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::NotFound(id.clone()))?;
        match state {
            ExtensionState::Failed | ExtensionState::Registered => self.activate(id),
            ExtensionState::Active => Ok(()),
            ExtensionState::Starting => Err(PluginError::InvalidState {
                extension_id: id.clone(),
                state,
            }),
            ExtensionState::Disabled => Err(PluginError::InvalidState {
                extension_id: id.clone(),
                state,
            }),
        }
    }

    fn invoke_command(
        &mut self,
        id: &ExtensionId,
        command_id: String,
        args: Value,
        context: Value,
    ) -> Result<PluginInvocationResult, PluginError> {
        self.invoke_declared(
            id,
            command_id,
            ContributionKind::Command,
            "command.execute",
            args,
            context,
        )
    }

    fn invoke_preview_action(
        &mut self,
        id: &ExtensionId,
        command_id: String,
        args: Value,
        context: PreviewActionContext,
    ) -> Result<PluginInvocationResult, PluginError> {
        InMemoryPluginHost::invoke_preview_action(self, id, command_id, args, context)
    }

    fn invoke_preview(
        &mut self,
        id: &ExtensionId,
        contribution_id: String,
        context: Value,
    ) -> Result<PluginInvocationResult, PluginError> {
        self.invoke_preview_contribution(
            id,
            contribution_id,
            &[ContributionKind::PreviewProvider],
            "preview.open",
            context,
            "preview provider",
        )
    }

    fn invoke_preview_augmentation(
        &mut self,
        id: &ExtensionId,
        contribution_id: String,
        context: Value,
    ) -> Result<PluginInvocationResult, PluginError> {
        self.invoke_preview_contribution(
            id,
            contribution_id,
            &[
                ContributionKind::PreviewAugmentation,
                ContributionKind::Inspector,
            ],
            "preview.augment",
            context,
            "preview augmentation",
        )
    }

    fn has_permission(
        &self,
        id: &ExtensionId,
        permission: Permission,
    ) -> Result<bool, PluginError> {
        InMemoryPluginHost::has_permission(self, id, permission)
    }

    fn install_from_archive(
        &mut self,
        source_archive: &Path,
        install_root: &Path,
    ) -> Result<ExtensionSummary, PluginError> {
        let extraction = install_root.join(format!(
            ".archive-staging-{}-{}",
            std::process::id(),
            recovery_log_timestamp_ms()
        ));
        ZipPluginArchiveAdapter
            .extract_to(source_archive, &extraction)
            .map_err(|error| {
                PluginError::InstallFailed(format!("archive extraction failed: {error:?}"))
            })?;
        let result = self.install_from_directory(&extraction, install_root);
        let _ = fs::remove_dir_all(&extraction);
        result
    }

    fn inspect_from_directory(
        &self,
        source_dir: &Path,
    ) -> Result<ExtensionInstallPreview, PluginError> {
        let source_manifest_path = source_dir.join("manifest.json");
        let bytes = fs::read(&source_manifest_path)
            .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        let manifest = serde_json::from_slice::<ExtensionManifest>(&bytes)
            .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        if !is_safe_extension_id(&manifest.id.0) {
            return Err(PluginError::InstallFailed(format!(
                "extension id {} cannot be used as an installation directory",
                manifest.id.0
            )));
        }
        let compatible = api_compatible(&manifest.host_api, &self.host_api);
        let mut issues = Vec::new();
        if !compatible {
            issues.push(format!(
                "host API {} is incompatible with host {}",
                manifest.host_api.0, self.host_api.0
            ));
        } else {
            self.validate_manifest(&manifest)?;
        }

        let existing = self
            .discover()
            .into_iter()
            .find(|summary| summary.id == manifest.id);
        let operation = if existing.is_some() {
            ExtensionInstallOperation::Upgrade
        } else {
            ExtensionInstallOperation::Install
        };
        if let Some(current) = &existing {
            if !version_is_newer(&manifest.version, &current.version) {
                issues.push(format!(
                    "version {} must be newer than installed version {}",
                    manifest.version, current.version
                ));
            }
        }

        Ok(ExtensionInstallPreview {
            manifest,
            existing,
            operation,
            compatible,
            can_install: compatible && issues.is_empty(),
            issues,
        })
    }

    fn inspect_from_archive(
        &self,
        source_archive: &Path,
    ) -> Result<ExtensionInstallPreview, PluginError> {
        let extraction = temporary_archive_directory("inspect");
        ZipPluginArchiveAdapter
            .extract_to(source_archive, &extraction)
            .map_err(|error| {
                PluginError::InstallFailed(format!("archive extraction failed: {error:?}"))
            })?;
        let result = self.inspect_from_directory(&extraction);
        let _ = fs::remove_dir_all(&extraction);
        result
    }

    fn install_from_directory(
        &mut self,
        source_dir: &Path,
        install_root: &Path,
    ) -> Result<ExtensionSummary, PluginError> {
        let source_manifest_path = source_dir.join("manifest.json");
        let bytes = fs::read(&source_manifest_path)
            .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        let mut manifest = serde_json::from_slice::<ExtensionManifest>(&bytes)
            .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        let id = manifest.id.clone();
        if !is_safe_extension_id(&id.0) {
            return Err(PluginError::InstallFailed(format!(
                "extension id {} cannot be used as an installation directory",
                id.0
            )));
        }
        let previous_manifest = self.manifests.get(&id).cloned();
        let previous_state = self.states.get(&id).cloned();
        if self.pending_installs.contains_key(&id) {
            return Err(PluginError::InstallFailed(format!(
                "extension {} already has a pending installation",
                id.0
            )));
        }

        fs::create_dir_all(install_root)
            .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        let destination = install_root.join(&id.0);
        if previous_manifest.is_none() && destination.exists() {
            return Err(PluginError::InstallConflict(id));
        }
        if let Some(current) = self.discover().into_iter().find(|summary| summary.id == id) {
            if !version_is_newer(&manifest.version, &current.version) {
                return Err(PluginError::VersionConflict {
                    extension_id: id,
                    installed: current.version,
                    requested: manifest.version,
                });
            }
        }
        let staging = install_root.join(format!(".{}.staging-{}", id.0, std::process::id()));
        if staging.exists() {
            fs::remove_dir_all(&staging)
                .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        }
        if let Err(error) = copy_directory(source_dir, &staging) {
            let _ = fs::remove_dir_all(&staging);
            return Err(PluginError::InstallFailed(error.to_string()));
        }

        resolve_relative_launch_args(&destination.join("manifest.json"), &mut manifest);
        if let Err(error) = self.validate_manifest(&manifest) {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }

        let backup_path = previous_manifest
            .as_ref()
            .map(|_| install_root.join(format!(".{}.rollback-{}", id.0, std::process::id())));
        if let Some(backup_path) = &backup_path {
            if backup_path.exists() {
                fs::remove_dir_all(backup_path)
                    .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
            }
            if self.active.contains(&id) {
                self.disable(&id, DisableReason::User)?;
            }
            fs::rename(&destination, backup_path)
                .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
            self.remove_registration(&id);
        }
        if let Err(error) = fs::rename(&staging, &destination) {
            if let Some(backup_path) = &backup_path {
                let _ = fs::rename(backup_path, &destination);
            }
            return Err(PluginError::InstallFailed(error.to_string()));
        }
        if let Err(error) = self.register_from_package(manifest, &destination) {
            let _ = fs::remove_dir_all(&destination);
            if let Some(backup_path) = &backup_path {
                let _ = fs::rename(backup_path, &destination);
                if let Some(previous_manifest) = previous_manifest {
                    let _ = self.register_from_package(previous_manifest, &destination);
                    if let Some(state) = previous_state {
                        self.restore_state(&id, state);
                    }
                }
            }
            return Err(error);
        }

        self.pending_installs.insert(
            id.clone(),
            InstallBackup {
                previous_manifest,
                previous_state,
                backup_path,
            },
        );

        self.discover()
            .into_iter()
            .find(|summary| summary.id == id)
            .ok_or_else(|| PluginError::InstallFailed("installed plugin disappeared".into()))
    }

    fn commit_install(
        &mut self,
        id: &ExtensionId,
        _install_root: &Path,
    ) -> Result<(), PluginError> {
        let Some(backup) = self.pending_installs.remove(id) else {
            return Ok(());
        };
        if let Some(backup_path) = backup.backup_path {
            fs::remove_dir_all(&backup_path)
                .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        }
        Ok(())
    }

    fn rollback_install(
        &mut self,
        id: &ExtensionId,
        install_root: &Path,
    ) -> Result<(), PluginError> {
        let backup = self
            .pending_installs
            .remove(id)
            .ok_or_else(|| PluginError::InvalidState {
                extension_id: id.clone(),
                state: self
                    .states
                    .get(id)
                    .cloned()
                    .unwrap_or(ExtensionState::Registered),
            })?;
        if self.active.contains(id) {
            self.disable(id, DisableReason::Incompatible)?;
        }
        let destination = install_root.join(&id.0);
        if destination.exists() {
            fs::remove_dir_all(&destination)
                .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        }
        self.remove_registration(id);
        if let Some(backup_path) = backup.backup_path {
            fs::rename(&backup_path, &destination)
                .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        }
        if let Some(previous_manifest) = backup.previous_manifest {
            self.register_from_package(previous_manifest, &destination)?;
            if let Some(previous_state) = backup.previous_state {
                if previous_state == ExtensionState::Active {
                    self.activate(id)?;
                } else {
                    self.restore_state(id, previous_state);
                }
            }
        }
        Ok(())
    }

    fn inspect_install_recoveries(
        &self,
        install_root: &Path,
    ) -> Result<Vec<ExtensionInstallRecovery>, PluginError> {
        let paths_by_id = scan_recovery_paths(install_root)?;
        let summaries = self.discover();
        let mut recoveries = Vec::new();

        for (id, paths) in paths_by_id {
            let current = summaries.iter().find(|summary| summary.id == id).cloned();
            let destination = install_root.join(&id.0);

            if let Some(staging_path) = paths.staging.last() {
                let mut issues = Vec::new();
                let candidate = match recovery_manifest(staging_path) {
                    Ok(manifest) => {
                        if manifest.id != id {
                            issues.push(format!(
                                "staging manifest id {} does not match {}",
                                manifest.id.0, id.0
                            ));
                        }
                        if let Err(error) = self.validate_manifest(&manifest) {
                            issues.push(format!("staging manifest is invalid: {error:?}"));
                        }
                        Some(manifest)
                    }
                    Err(error) => {
                        issues.push(format!("staging manifest cannot be read: {error}"));
                        None
                    }
                };
                if destination.exists() {
                    issues.push("the installed destination already exists".into());
                }
                if paths.staging.len() > 1 {
                    issues.push(
                        "multiple staging directories were found; the newest is selected".into(),
                    );
                }
                recoveries.push(ExtensionInstallRecovery {
                    extension_id: id.clone(),
                    kind: ExtensionInstallRecoveryKind::Staging,
                    operation: if current.is_some() {
                        ExtensionInstallOperation::Upgrade
                    } else {
                        ExtensionInstallOperation::Install
                    },
                    candidate_version: candidate.as_ref().map(|manifest| manifest.version.clone()),
                    current: current.clone(),
                    rollback_version: None,
                    can_resume: !destination.exists()
                        && current.is_none()
                        && issues.is_empty()
                        && paths.staging.len() == 1,
                    can_commit: false,
                    can_rollback: false,
                    can_discard: true,
                    issues,
                });
            }

            if let Some(rollback_path) = paths.rollback.last() {
                let mut issues = Vec::new();
                let previous = match recovery_manifest(rollback_path) {
                    Ok(manifest) => {
                        if manifest.id != id {
                            issues.push(format!(
                                "rollback manifest id {} does not match {}",
                                manifest.id.0, id.0
                            ));
                        }
                        if let Err(error) = self.validate_manifest(&manifest) {
                            issues.push(format!("rollback manifest is invalid: {error:?}"));
                        }
                        Some(manifest)
                    }
                    Err(error) => {
                        issues.push(format!("rollback manifest cannot be read: {error}"));
                        None
                    }
                };
                let mut current_manifest_valid = false;
                let current_manifest = if destination.exists() {
                    match recovery_manifest(&destination) {
                        Ok(manifest) => {
                            match self.validate_manifest(&manifest) {
                                Ok(()) => current_manifest_valid = true,
                                Err(error) => {
                                    issues.push(format!("current manifest is invalid: {error:?}"));
                                }
                            }
                            Some(manifest)
                        }
                        Err(error) => {
                            issues.push(format!("current manifest cannot be read: {error}"));
                            None
                        }
                    }
                } else {
                    issues.push("the current installed destination is missing".into());
                    None
                };
                if paths.rollback.len() > 1 {
                    issues.push(
                        "multiple rollback directories were found; the newest is selected".into(),
                    );
                }
                recoveries.push(ExtensionInstallRecovery {
                    extension_id: id.clone(),
                    kind: ExtensionInstallRecoveryKind::Rollback,
                    operation: ExtensionInstallOperation::Upgrade,
                    candidate_version: current_manifest
                        .as_ref()
                        .map(|manifest| manifest.version.clone()),
                    current: current.clone(),
                    rollback_version: previous.as_ref().map(|manifest| manifest.version.clone()),
                    can_resume: false,
                    can_commit: current_manifest_valid && destination.exists(),
                    can_rollback: previous.is_some()
                        && !issues
                            .iter()
                            .any(|issue| issue.starts_with("rollback manifest")),
                    can_discard: true,
                    issues,
                });
            }
        }

        Ok(recoveries)
    }

    fn resolve_install_recovery(
        &mut self,
        id: &ExtensionId,
        action: ExtensionInstallRecoveryAction,
        install_root: &Path,
    ) -> Result<(), PluginError> {
        let attempt = next_recovery_attempt(install_root, id, &action);
        let action_for_log = action.clone();
        let result = (|| {
            let paths_by_id = scan_recovery_paths(install_root)?;
            let paths = paths_by_id
                .get(id)
                .ok_or_else(|| PluginError::InvalidState {
                    extension_id: id.clone(),
                    state: self
                        .states
                        .get(id)
                        .cloned()
                        .unwrap_or(ExtensionState::Registered),
                })?;
            let destination = install_root.join(&id.0);

            match action {
                ExtensionInstallRecoveryAction::Resume => {
                    let staging =
                        paths
                            .staging
                            .last()
                            .ok_or_else(|| PluginError::InvalidState {
                                extension_id: id.clone(),
                                state: ExtensionState::Registered,
                            })?;
                    if destination.exists() || self.manifests.contains_key(id) {
                        return Err(PluginError::InstallConflict(id.clone()));
                    }
                    let manifest =
                        recovery_manifest(staging).map_err(PluginError::InstallFailed)?;
                    if manifest.id != *id {
                        return Err(PluginError::InstallFailed(format!(
                            "staging manifest id {} does not match {}",
                            manifest.id.0, id.0
                        )));
                    }
                    self.validate_manifest(&manifest)?;
                    fs::rename(staging, &destination)
                        .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
                    if let Err(error) = self.register_from_package(manifest, &destination) {
                        let _ = fs::rename(&destination, staging);
                        return Err(error);
                    }
                    for stale in paths.staging.iter().filter(|path| *path != staging) {
                        let _ = fs::remove_dir_all(stale);
                    }
                }
                ExtensionInstallRecoveryAction::Commit => {
                    if paths.rollback.last().is_none() {
                        return Err(PluginError::InvalidState {
                            extension_id: id.clone(),
                            state: ExtensionState::Registered,
                        });
                    }
                    if !destination.exists() {
                        return Err(PluginError::InstallFailed(
                            "cannot keep an interrupted upgrade without its current package".into(),
                        ));
                    }
                    let manifest =
                        recovery_manifest(&destination).map_err(PluginError::InstallFailed)?;
                    self.validate_manifest(&manifest)?;
                    if !self.manifests.contains_key(id) {
                        self.register_from_package(manifest, &destination)?;
                    }
                    for rollback_path in &paths.rollback {
                        if rollback_path.exists() {
                            fs::remove_dir_all(rollback_path)
                                .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
                        }
                    }
                }
                ExtensionInstallRecoveryAction::Rollback => {
                    let rollback =
                        paths
                            .rollback
                            .last()
                            .ok_or_else(|| PluginError::InvalidState {
                                extension_id: id.clone(),
                                state: ExtensionState::Registered,
                            })?;
                    let previous_manifest =
                        recovery_manifest(rollback).map_err(PluginError::InstallFailed)?;
                    if previous_manifest.id != *id {
                        return Err(PluginError::InstallFailed(format!(
                            "rollback manifest id {} does not match {}",
                            previous_manifest.id.0, id.0
                        )));
                    }
                    self.validate_manifest(&previous_manifest)?;
                    if self.active.contains(id) {
                        self.disable(id, DisableReason::Incompatible)?;
                    }
                    let current_backup = install_root.join(format!(
                        ".{}.recovery-current-{}",
                        id.0,
                        std::process::id()
                    ));
                    if current_backup.exists() {
                        fs::remove_dir_all(&current_backup)
                            .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
                    }
                    if destination.exists() {
                        fs::rename(&destination, &current_backup)
                            .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
                    }
                    if let Err(error) = fs::rename(rollback, &destination) {
                        if current_backup.exists() {
                            let _ = fs::rename(&current_backup, &destination);
                        }
                        return Err(PluginError::InstallFailed(error.to_string()));
                    }
                    self.remove_registration(id);
                    if let Err(error) = self.register_from_package(previous_manifest, &destination)
                    {
                        let _ = fs::remove_dir_all(&destination);
                        if current_backup.exists() {
                            let _ = fs::rename(&current_backup, &destination);
                        }
                        return Err(error);
                    }
                    if current_backup.exists() {
                        fs::remove_dir_all(&current_backup)
                            .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
                    }
                    for stale in paths.rollback.iter().filter(|path| *path != rollback) {
                        let _ = fs::remove_dir_all(stale);
                    }
                }
                ExtensionInstallRecoveryAction::Discard => {
                    for staging in &paths.staging {
                        if staging.exists() {
                            fs::remove_dir_all(staging)
                                .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
                        }
                    }
                    for rollback in &paths.rollback {
                        if rollback.exists() {
                            fs::remove_dir_all(rollback)
                                .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
                        }
                    }
                }
            }
            Ok(())
        })();

        let (status, message) = match &result {
            Ok(()) => (
                ExtensionInstallRecoveryLogStatus::Succeeded,
                "recovery action completed".to_owned(),
            ),
            Err(error) => (
                ExtensionInstallRecoveryLogStatus::Failed,
                format!("recovery action failed: {error:?}"),
            ),
        };
        let _ = append_recovery_log(
            install_root,
            ExtensionInstallRecoveryLogEntry {
                extension_id: id.clone(),
                action: action_for_log,
                status,
                attempt,
                message,
                removed_paths: Vec::new(),
                occurred_at_ms: recovery_log_timestamp_ms(),
            },
        );
        result
    }

    fn inspect_install_recovery_log(
        &self,
        install_root: &Path,
    ) -> Result<Vec<ExtensionInstallRecoveryLogEntry>, PluginError> {
        Ok(read_recovery_log(install_root).unwrap_or_default())
    }

    fn clear_install_recovery_log(&mut self, install_root: &Path) -> Result<(), PluginError> {
        match fs::remove_file(recovery_log_path(install_root)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(PluginError::InstallFailed(error.to_string())),
        }
        let entries = match fs::read_dir(install_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(PluginError::InstallFailed(error.to_string())),
        };
        for entry in entries {
            let entry = entry.map_err(|error| PluginError::InstallFailed(error.to_string()))?;
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(".plugin-recovery-log.json.tmp-")
            {
                fs::remove_file(entry.path())
                    .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
            }
        }
        Ok(())
    }

    fn export_install_recovery_log(
        &self,
        install_root: &Path,
        destination: &Path,
    ) -> Result<(), PluginError> {
        let entries = read_recovery_log(install_root)?;
        let bytes = serde_json::to_vec_pretty(&entries)
            .map_err(|error| PluginError::InstallFailed(error.to_string()))?;
        fs::write(destination, bytes).map_err(|error| PluginError::InstallFailed(error.to_string()))
    }

    fn cleanup_install_recovery_orphans(&mut self, install_root: &Path) -> Result<(), PluginError> {
        let paths_by_id = scan_recovery_paths(install_root)?;
        for (id, paths) in paths_by_id {
            let destination = install_root.join(&id.0);
            let mut candidates = Vec::new();
            candidates.extend(
                paths
                    .staging
                    .iter()
                    .take(paths.staging.len().saturating_sub(1))
                    .cloned(),
            );
            candidates.extend(
                paths
                    .rollback
                    .iter()
                    .take(paths.rollback.len().saturating_sub(1))
                    .cloned(),
            );

            let staged_replacement_valid = paths
                .staging
                .last()
                .and_then(|staging| recovery_manifest(staging).ok())
                .is_some_and(|manifest| {
                    manifest.id == id && self.validate_manifest(&manifest).is_ok()
                });
            if let Some(staging) = paths.staging.last() {
                if !staged_replacement_valid || destination.exists() {
                    candidates.push(staging.clone());
                }
            }

            if let Some(rollback) = paths.rollback.last() {
                let valid = recovery_manifest(rollback)
                    .map(|manifest| manifest.id == id && self.validate_manifest(&manifest).is_ok())
                    .unwrap_or(false);
                let has_staged_replacement = staged_replacement_valid && !destination.exists();
                if !valid || (!destination.exists() && !has_staged_replacement) {
                    candidates.push(rollback.clone());
                }
            }

            candidates.sort();
            candidates.dedup();
            for path in candidates {
                let attempt = next_recovery_attempt(
                    install_root,
                    &id,
                    &ExtensionInstallRecoveryAction::Discard,
                );
                let result = fs::remove_dir_all(&path)
                    .map_err(|error| PluginError::InstallFailed(error.to_string()));
                let (status, message) = match &result {
                    Ok(()) => (
                        ExtensionInstallRecoveryLogStatus::Succeeded,
                        "orphan recovery directory removed".to_owned(),
                    ),
                    Err(error) => (
                        ExtensionInstallRecoveryLogStatus::Failed,
                        format!("orphan recovery directory could not be removed: {error:?}"),
                    ),
                };
                let _ = append_recovery_log(
                    install_root,
                    ExtensionInstallRecoveryLogEntry {
                        extension_id: id.clone(),
                        action: ExtensionInstallRecoveryAction::Discard,
                        status,
                        attempt,
                        message,
                        removed_paths: vec![path.to_string_lossy().into_owned()],
                        occurred_at_ms: recovery_log_timestamp_ms(),
                    },
                );
                result?;
            }
        }
        Ok(())
    }

    fn uninstall_from_directory(
        &mut self,
        id: &ExtensionId,
        install_root: &Path,
    ) -> Result<(), PluginError> {
        if !self.manifests.contains_key(id) {
            return Err(PluginError::NotFound(id.clone()));
        }
        if !is_safe_extension_id(&id.0) {
            return Err(PluginError::UninstallFailed(format!(
                "extension id {} cannot be used as an installation directory",
                id.0
            )));
        }

        if self.active.contains(id) {
            self.disable(id, DisableReason::User)?;
        }
        let destination = install_root.join(&id.0);
        if destination.exists() {
            fs::remove_dir_all(&destination)
                .map_err(|error| PluginError::UninstallFailed(error.to_string()))?;
        }

        self.manifests.remove(id);
        self.active.remove(id);
        self.states.remove(id);
        self.processes.remove(id);
        let runtime_nonce = self.runtime_nonces.remove(id);
        self.close_runtime_lease(id, runtime_nonce.as_deref());
        Ok(())
    }

    fn extension_storage(&self, id: &ExtensionId) -> Result<ExtensionStorage, PluginError> {
        InMemoryPluginHost::extension_storage(self, id)
    }

    fn export_extension_state(
        &self,
        id: &ExtensionId,
        participant: &dyn BackupParticipant,
    ) -> Result<ExtensionStorageExport, PluginError> {
        InMemoryPluginHost::export_extension_state(self, id, participant)
    }

    fn migrate_extension_state(
        &self,
        id: &ExtensionId,
        participant: &dyn MigrationParticipant,
    ) -> Result<ExtensionStorageView, PluginError> {
        InMemoryPluginHost::migrate_extension_state(self, id, participant)
    }

    fn export_extension_state_from_runtime(
        &mut self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageExport, PluginError> {
        InMemoryPluginHost::export_extension_state_from_runtime(self, id)
    }

    fn migrate_extension_state_from_runtime(
        &mut self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageView, PluginError> {
        InMemoryPluginHost::migrate_extension_state_from_runtime(self, id)
    }

    fn extension_storage_recovery(
        &self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageRecovery, PluginError> {
        InMemoryPluginHost::extension_storage_recovery(self, id)
    }

    fn restore_extension_state(
        &mut self,
        id: &ExtensionId,
    ) -> Result<ExtensionStorageView, PluginError> {
        InMemoryPluginHost::restore_extension_state(self, id)
    }

    fn quarantine_extension_state(
        &mut self,
        id: &ExtensionId,
        generation: u64,
    ) -> Result<(), PluginError> {
        InMemoryPluginHost::quarantine_extension_state(self, id, generation)
    }

    fn quarantined_runtime_leases(&self) -> Result<Vec<QuarantinedRuntimeLease>, PluginError> {
        InMemoryPluginHost::quarantined_runtime_leases(self)
    }

    fn close_quarantined_runtime_lease(
        &mut self,
        extension_id: &ExtensionId,
        runtime_nonce: &str,
    ) -> Result<(), PluginError> {
        InMemoryPluginHost::close_quarantined_runtime_lease(self, extension_id, runtime_nonce)
    }

    fn set_runtime_context(
        &mut self,
        host_instance_id: String,
        launch_attempt_id: u64,
        crash_receipt_path: Option<PathBuf>,
    ) {
        InMemoryPluginHost::set_runtime_context(
            self,
            host_instance_id,
            launch_attempt_id,
            crash_receipt_path,
        );
    }

    fn reconcile_runtime_orphans(&mut self) -> Result<Vec<RuntimeOrphanReport>, PluginError> {
        InMemoryPluginHost::reconcile_runtime_orphans(self)
    }

    fn drain_diagnostics(&mut self) -> Vec<PluginDiagnosticRecord> {
        InMemoryPluginHost::drain_diagnostics(self)
    }

    fn drain_crash_evidence(&mut self) -> Vec<RuntimeCrashEvidence> {
        InMemoryPluginHost::drain_crash_evidence(self)
    }
}

fn namespace_for(scope: &ContributionScope, extension_id: &ExtensionId) -> String {
    match scope {
        ContributionScope::Workbench => "workbench".into(),
        ContributionScope::WorkbenchApp { app_id } => format!("workbench.app.{app_id}"),
        ContributionScope::Capability => format!("capability.{}", extension_id.0),
    }
}

fn storage_diagnostic_kind(method: &str) -> PluginDiagnosticKind {
    if method == STORAGE_MIGRATE_METHOD {
        PluginDiagnosticKind::Migration
    } else {
        PluginDiagnosticKind::Storage
    }
}

fn storage_recovery_reason(error: ExtensionStorageError) -> String {
    sanitize_plugin_diagnostic_message(&format!(
        "extension storage recovery unavailable: {error:?}"
    ))
}

fn permission_allowed(zone: WorkbenchZone, permission: &Permission) -> bool {
    match zone {
        WorkbenchZone::Navigation => matches!(
            permission,
            Permission::WorkspaceOpen | Permission::LibraryRead | Permission::SourceConnect
        ),
        WorkbenchZone::Contextual => matches!(
            permission,
            Permission::AssetRead
                | Permission::MetadataRead
                | Permission::PreviewRender
                | Permission::MetadataWrite
        ),
        WorkbenchZone::Operations => matches!(
            permission,
            Permission::JobsCreate
                | Permission::JobsRead
                | Permission::LogsRead
                | Permission::ExternalProcess
                | Permission::Network
        ),
    }
}

fn permission_name(permission: &Permission) -> &'static str {
    match permission {
        Permission::WorkspaceOpen => "workspace.open",
        Permission::LibraryRead => "library.read",
        Permission::SourceConnect => "source.connect",
        Permission::AssetRead => "asset.read",
        Permission::MetadataRead => "metadata.read",
        Permission::MetadataWrite => "metadata.write",
        Permission::PreviewRender => "preview.render",
        Permission::JobsCreate => "jobs.create",
        Permission::JobsRead => "jobs.read",
        Permission::LogsRead => "logs.read",
        Permission::ExternalProcess => "external-process",
        Permission::Network => "network",
    }
}

fn runtime_error_message(error: RuntimeError) -> String {
    match error {
        RuntimeError::MissingLaunchSpec(id) => {
            format!("external runtime for {} has no launch specification", id.0)
        }
        RuntimeError::InvalidLaunchSpec(id) => {
            format!(
                "external runtime for {} has an invalid launch specification",
                id.0
            )
        }
        RuntimeError::StartFailed(message) | RuntimeError::ShutdownFailed(message) => message,
        RuntimeError::InvocationFailed(message) => message,
        RuntimeError::InvocationTimeout => "plugin command invocation timed out".into(),
        RuntimeError::InvocationRejected(message) => message,
    }
}

fn next_runtime_nonce(id: &ExtensionId) -> String {
    let sequence = RUNTIME_NONCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{}-{timestamp}-{sequence}", std::process::id(), id.0)
}

fn api_compatible(required: &VersionRange, provided: &VersionRange) -> bool {
    if required.0 == "*" || required.0 == provided.0 {
        return true;
    }

    required
        .0
        .strip_prefix('^')
        .and_then(|range| range.split('.').next())
        == provided.0.split('.').next()
}

fn version_is_newer(requested: &str, installed: &str) -> bool {
    let parse = |value: &str| {
        value
            .split('.')
            .map(|part| part.parse::<u64>().ok())
            .collect::<Option<Vec<_>>>()
    };
    match (parse(requested), parse(installed)) {
        (Some(requested), Some(installed)) => requested > installed,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use super::*;
    use velvet_layout_model::WorkbenchZone;
    use velvet_persistence::{
        InMemoryStateSnapshotStore, JsonFileStateSnapshotStore, StateSnapshotStore,
    };
    use velvet_plugin_api::{
        ContextAnchor, ContributionDescriptor, ExtensionId, ExtensionStorageDeclaration,
        LocalizationManifest, Permission, PreviewAugmentationSlot, PreviewContributionDescriptor,
        PreviewIntent, PreviewSurface, RuntimeKind, RuntimeLaunchSpec, UiSurfaceDescriptor,
        UiSurfaceKind, VersionRange, ZonePermissionSet,
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

    fn write_zip_package(path: &PathBuf, manifest: &ExtensionManifest, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("the archive should be writable");
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
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
        let manifest = view_manifest(None);
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
        let request: StorageRuntimeRequest =
            serde_json::from_value(invocation.args.clone()).unwrap();
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
            message: "failed at /Users/example/private/file.txt path=/Users/example/private/file.txt\nsecret".into(),
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
        assert!(matches!(error, PluginError::RuntimeInvocation { .. }));
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
                && diagnostic.code == "invocation-runtime-failed"
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
        assert!(!root.join("velvet.example.echo").exists());

        fs::remove_dir_all(root).expect("the temporary installation should be removed");
    }

    #[test]
    fn plugin_upgrade_can_be_previewed_committed_or_rolled_back() {
        let root = temporary_manifest_dir();
        let first_source = temporary_manifest_dir();
        let upgrade_source = temporary_manifest_dir();
        let mut first = ExtensionManifest {
            id: ExtensionId("transactional.plugin".into()),
            version: "0.1.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::BuiltIn,
            launch: None,
            target_app: None,
            contributions: Vec::new(),
            permission_sets: Vec::new(),
            ..ExtensionManifest::default()
        };
        write_package(&first_source, &first);
        let mut host = InMemoryPluginHost::default();
        host.install_from_directory(&first_source, &root)
            .expect("the initial package should install");
        host.commit_install(&first.id, &root)
            .expect("the initial package should be committed");
        host.activate(&first.id)
            .expect("the initial package should activate");

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

        host.install_from_directory(&upgrade_source, &root)
            .expect("the upgrade should be stageable again");
        host.commit_install(&first.id, &root)
            .expect("the upgrade should be committed");
        assert_eq!(host.projection(&first.id).unwrap().summary.version, "0.2.0");
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
        let recovery_current =
            root.join(format!(".{}.recovery-current-{}", id.0, std::process::id()));
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
}
