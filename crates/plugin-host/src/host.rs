//! PluginHost implementation and host-owned lifecycle coordination.
//!
//! Package trust, extension storage, runtime leases and platform process
//! supervision stay in sibling owner modules and are exposed through this
//! private implementation module's public facade.

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

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use platform_process::ProcessSupervisor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use velvet_job_system::{JobBudgetRequest, JobProvenance, JOB_SCHEMA_VERSION};
use velvet_layout_model::WorkbenchZone;
use velvet_plugin_api::{
    validate_reserved_metadata, ActivationEvent, AppTarget, Capability, ContributionKind,
    ContributionScope, ExtensionId, ExtensionInstallOperation, ExtensionInstallPreview,
    ExtensionInstallRecovery, ExtensionInstallRecoveryAction, ExtensionInstallRecoveryKind,
    ExtensionInstallRecoveryLogEntry, ExtensionInstallRecoveryLogStatus, ExtensionManifest,
    ExtensionProjection, ExtensionState, ExtensionSummary, HostContainer, Permission,
    PreviewActionContext, RuntimeKind, RuntimeLaunchSpec, VersionRange, MANIFEST_SCHEMA_VERSION,
    PREVIEW_ACTION_SCHEMA_VERSION,
};

use crate::registry::{
    ContributionRegistrationHandle, ContributionRegistry, ContributionRegistrySnapshot,
    RegistryError,
};

#[path = "capabilities.rs"]
mod capabilities;
#[path = "extension_storage.rs"]
mod extension_storage;
#[path = "platform_process.rs"]
mod platform_process;
#[path = "publisher_trust.rs"]
mod publisher_trust;
#[path = "runtime_lease.rs"]
mod runtime_lease;
#[path = "trust_store.rs"]
mod trust_store;

pub use self::capabilities::{
    CapabilityAuditAction, CapabilityAuditRecord, CapabilityBroker, CapabilityBrokerError,
    CapabilityDenial, CapabilityDenyReason, CapabilityGrantSnapshot, CapabilityHandle,
    CapabilityPolicy, CAPABILITY_AUDIT_SCHEMA_VERSION, CAPABILITY_SCHEMA_VERSION,
};
pub use extension_storage::{
    BackupParticipant, ExtensionStorage, ExtensionStorageError, ExtensionStorageExport,
    ExtensionStorageRecovery, ExtensionStorageRecoveryStatus, ExtensionStorageTransaction,
    ExtensionStorageView, MigrationParticipant, EXTENSION_STORAGE_SCOPE,
    MAX_EXTENSION_STORAGE_BYTES, MAX_EXTENSION_STORAGE_KEY_BYTES,
};
pub use publisher_trust::{
    InMemoryPluginPublisherTrustStore, JsonFilePluginPublisherTrustStore,
    PluginPublisherTrustDocument, PluginPublisherTrustError, PluginPublisherTrustStore,
    MAX_PLUGIN_PUBLISHER_KEYS, MAX_PLUGIN_PUBLISHER_ROTATIONS,
    MAX_PLUGIN_PUBLISHER_TRUST_DOCUMENT_BYTES, MAX_PLUGIN_PUBLISHER_TRUST_LIFETIME_SECONDS,
    PLUGIN_PUBLISHER_TRUST_DOCUMENT_SCHEMA_VERSION,
};
pub use runtime_lease::{
    ConservativeRuntimeOrphanReconciler, InMemoryRuntimeLeaseStore, JsonFileRuntimeLeaseStore,
    PlatformRuntimeOrphanReconciler, QuarantinedRuntimeLease, RuntimeLease, RuntimeLeaseError,
    RuntimeLeaseState, RuntimeLeaseStore, RuntimeOrphanObservation, RuntimeOrphanReconciler,
    RuntimeOrphanReport, RuntimeProcessIdentity, MAX_RUNTIME_LEASES, MAX_RUNTIME_LEASE_STORE_BYTES,
    RUNTIME_LEASE_SCHEMA_VERSION,
};
pub use trust_store::{
    InMemoryPluginTrustStore, JsonFilePluginTrustStore, PluginTrustStore, PluginTrustStoreError,
    MAX_PLUGIN_TRUST_RECORDS, MAX_PLUGIN_TRUST_STORE_BYTES, PLUGIN_TRUST_STORE_SCHEMA_VERSION,
};

pub const PLUGIN_SIGNATURE_SCHEMA_VERSION: u16 = 1;
pub const PLUGIN_KEY_ROTATION_SCHEMA_VERSION: u16 = 1;
pub const PLUGIN_TRUST_RECORD_SCHEMA_VERSION: u16 = 1;
pub const MAX_PLUGIN_SIGNATURE_BYTES: usize = 64 * 1024;
pub const MAX_PLUGIN_KEY_ROTATION_BYTES: usize = 16 * 1024;
const PLUGIN_SIGNATURE_FILE: &str = "signature.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginTrustState {
    Trusted,
    DeveloperUnsigned,
    Unknown,
    Revoked,
    Incompatible,
    Blocked,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginTrustPolicy {
    require_signature: bool,
    developer_mode: bool,
    trusted_publishers: BTreeMap<String, String>,
    publisher_rotations: BTreeMap<String, PluginPublisherKeyRotation>,
    revoked_publishers: BTreeSet<String>,
    minimum_generations: BTreeMap<ExtensionId, u64>,
}

/// A publisher key transition that is authorized by both sides of the
/// transition. The signatures cover every field except the two signature
/// fields themselves, so a successor cannot be introduced by editing local
/// policy text or by possessing only the new key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginPublisherKeyRotation {
    pub schema_version: u16,
    pub old_key_id: String,
    pub old_public_key: String,
    pub new_key_id: String,
    pub new_public_key: String,
    pub sequence: u64,
    pub old_signature: String,
    pub new_signature: String,
}

impl PluginPublisherKeyRotation {
    /// Returns the canonical bytes that both keys must sign.
    pub fn canonical_payload(&self) -> Vec<u8> {
        canonical_plugin_publisher_key_rotation_payload(self)
    }
}

impl PluginTrustPolicy {
    pub fn developer_mode() -> Self {
        Self {
            require_signature: false,
            developer_mode: true,
            trusted_publishers: BTreeMap::new(),
            publisher_rotations: BTreeMap::new(),
            revoked_publishers: BTreeSet::new(),
            minimum_generations: BTreeMap::new(),
        }
    }

    pub fn require_signed() -> Self {
        Self {
            require_signature: true,
            developer_mode: false,
            trusted_publishers: BTreeMap::new(),
            publisher_rotations: BTreeMap::new(),
            revoked_publishers: BTreeSet::new(),
            minimum_generations: BTreeMap::new(),
        }
    }

    pub fn trust_publisher(
        mut self,
        publisher_key_id: impl Into<String>,
        public_key_hex: impl Into<String>,
    ) -> Self {
        self.trusted_publishers
            .insert(publisher_key_id.into(), public_key_hex.into());
        self
    }

    /// Registers a successor only when the currently trusted key and the
    /// successor key both sign the exact transition statement.
    pub fn register_publisher_key_rotation(
        &mut self,
        rotation: PluginPublisherKeyRotation,
    ) -> Result<(), PluginError> {
        let serialized = serde_json::to_vec(&rotation).map_err(|error| {
            PluginError::TrustPolicy(format!(
                "publisher key rotation is not serializable: {error}"
            ))
        })?;
        if serialized.len() > MAX_PLUGIN_KEY_ROTATION_BYTES {
            return Err(PluginError::TrustPolicy(format!(
                "publisher key rotation exceeds {MAX_PLUGIN_KEY_ROTATION_BYTES} bytes"
            )));
        }
        if rotation.schema_version != PLUGIN_KEY_ROTATION_SCHEMA_VERSION {
            return Err(PluginError::TrustPolicy(format!(
                "unsupported publisher key rotation schema {}",
                rotation.schema_version
            )));
        }
        if rotation.sequence == 0 {
            return Err(PluginError::TrustPolicy(
                "publisher key rotation sequence must be positive".into(),
            ));
        }
        if rotation.old_key_id.is_empty()
            || rotation.new_key_id.is_empty()
            || rotation.old_key_id == rotation.new_key_id
        {
            return Err(PluginError::TrustPolicy(
                "publisher key rotation ids must be distinct and non-empty".into(),
            ));
        }
        if self.revoked_publishers.contains(&rotation.old_key_id) {
            return Err(PluginError::TrustPolicy(
                "a revoked publisher key cannot authorize a rotation".into(),
            ));
        }
        let Some(current_public_key) = self.trusted_publishers.get(&rotation.old_key_id) else {
            return Err(PluginError::TrustPolicy(format!(
                "publisher key {} is not trusted",
                rotation.old_key_id
            )));
        };
        if current_public_key != &rotation.old_public_key {
            return Err(PluginError::TrustPolicy(
                "rotation old key does not match the trusted publisher key".into(),
            ));
        }
        if self.revoked_publishers.contains(&rotation.new_key_id) {
            return Err(PluginError::TrustPolicy(
                "a revoked successor key cannot be registered".into(),
            ));
        }
        if let Some(existing) = self.publisher_rotations.get(&rotation.new_key_id) {
            if existing == &rotation {
                return Ok(());
            }
            return Err(PluginError::TrustPolicy(format!(
                "publisher key {} already has a different rotation",
                rotation.new_key_id
            )));
        }
        if let Some(previous_sequence) = self
            .publisher_rotations
            .values()
            .filter(|existing| existing.old_key_id == rotation.old_key_id)
            .map(|existing| existing.sequence)
            .max()
        {
            if rotation.sequence <= previous_sequence {
                return Err(PluginError::TrustPolicy(format!(
                    "publisher key rotation sequence {} is not newer than {}",
                    rotation.sequence, previous_sequence
                )));
            }
        }
        if let Some(parent) = self.publisher_rotations.get(&rotation.old_key_id) {
            if rotation.sequence <= parent.sequence {
                return Err(PluginError::TrustPolicy(format!(
                    "publisher key rotation sequence {} does not advance from {}",
                    rotation.sequence, parent.sequence
                )));
            }
        }
        if self.trusted_publishers.contains_key(&rotation.new_key_id) {
            return Err(PluginError::TrustPolicy(format!(
                "publisher key {} is already trusted",
                rotation.new_key_id
            )));
        }

        let old_public_key = decode_verifying_key(&rotation.old_public_key).map_err(|message| {
            PluginError::TrustPolicy(format!("rotation old public key is invalid: {message}"))
        })?;
        let new_public_key = decode_verifying_key(&rotation.new_public_key).map_err(|message| {
            PluginError::TrustPolicy(format!("rotation new public key is invalid: {message}"))
        })?;
        if rotation.old_public_key == rotation.new_public_key {
            return Err(PluginError::TrustPolicy(
                "publisher key rotation must change the public key".into(),
            ));
        }
        let old_signature = decode_signature(&rotation.old_signature).map_err(|message| {
            PluginError::TrustPolicy(format!("rotation old signature is invalid: {message}"))
        })?;
        let new_signature = decode_signature(&rotation.new_signature).map_err(|message| {
            PluginError::TrustPolicy(format!("rotation new signature is invalid: {message}"))
        })?;
        let payload = rotation.canonical_payload();
        old_public_key
            .verify(&payload, &old_signature)
            .map_err(|_| {
                PluginError::TrustPolicy(
                    "publisher key rotation is not authorized by the old key".into(),
                )
            })?;
        new_public_key
            .verify(&payload, &new_signature)
            .map_err(|_| {
                PluginError::TrustPolicy(
                    "publisher key rotation is not confirmed by the successor key".into(),
                )
            })?;

        self.trusted_publishers
            .insert(rotation.new_key_id.clone(), rotation.new_public_key.clone());
        self.publisher_rotations
            .insert(rotation.new_key_id.clone(), rotation);
        Ok(())
    }

    pub fn revoke_publisher(mut self, publisher_key_id: impl Into<String>) -> Self {
        self.revoked_publishers.insert(publisher_key_id.into());
        self
    }

    pub fn require_minimum_generation(
        mut self,
        extension_id: ExtensionId,
        generation: u64,
    ) -> Self {
        self.minimum_generations.insert(extension_id, generation);
        self
    }

    pub fn is_developer_mode(&self) -> bool {
        self.developer_mode
    }

    #[cfg(test)]
    pub(crate) fn trusts_publisher_key(&self, publisher_key_id: &str, public_key: &str) -> bool {
        self.trusted_publishers
            .get(publisher_key_id)
            .is_some_and(|trusted_key| trusted_key == public_key)
    }

    pub fn allows_developer_mode(&self) -> bool {
        !self.require_signature
    }

    pub fn set_developer_mode(&mut self, enabled: bool) -> Result<(), PluginError> {
        if enabled && self.require_signature {
            return Err(PluginError::TrustPolicy(
                "the active release policy requires signed packages".into(),
            ));
        }
        self.developer_mode = enabled;
        Ok(())
    }
}

impl Default for PluginTrustPolicy {
    fn default() -> Self {
        Self::developer_mode()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginSignatureEnvelope {
    pub schema_version: u16,
    pub publisher_key_id: String,
    pub publisher_key: String,
    pub signature: String,
    pub package_sha256: String,
    pub extension_id: ExtensionId,
    pub version: String,
    pub runtime: RuntimeKind,
    pub manifest_schema: u16,
    pub host_api: VersionRange,
    pub generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginTrustRecord {
    pub schema_version: u16,
    pub extension_id: ExtensionId,
    pub version: String,
    pub package_sha256: String,
    pub publisher_key_id: Option<String>,
    pub generation: u64,
    pub state: PluginTrustState,
}

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
    TrustDenied {
        extension_id: ExtensionId,
        state: PluginTrustState,
        message: String,
    },
    TrustStore(String),
    TrustPolicy(String),
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
    RegistryUnavailable,
    CapabilityDenied {
        extension_id: ExtensionId,
        capability: Capability,
        reason: CapabilityDenyReason,
        retryable: bool,
    },
    CapabilityGenerationConflict {
        extension_id: ExtensionId,
        expected: u64,
        actual: u64,
    },
    InvalidState {
        extension_id: ExtensionId,
        state: ExtensionState,
    },
}

fn plugin_trust_store_error(error: PluginTrustStoreError) -> PluginError {
    PluginError::TrustStore(format!("{error:?}"))
}

fn capability_error(id: &ExtensionId, error: CapabilityBrokerError) -> PluginError {
    match error {
        CapabilityBrokerError::UnknownExtension(extension_id) => {
            PluginError::NotFound(extension_id)
        }
        CapabilityBrokerError::UnknownCapability(name) => {
            PluginError::InvalidContribution(format!("unsupported capability identifier {name}"))
        }
        CapabilityBrokerError::NotRequested(capability) => PluginError::CapabilityDenied {
            extension_id: id.clone(),
            capability,
            reason: CapabilityDenyReason::NotRequested,
            retryable: false,
        },
        CapabilityBrokerError::HostPolicyDenied(capability) => PluginError::CapabilityDenied {
            extension_id: id.clone(),
            capability,
            reason: CapabilityDenyReason::HostPolicy,
            retryable: false,
        },
        CapabilityBrokerError::UserApprovalRequired(capability) => PluginError::CapabilityDenied {
            extension_id: id.clone(),
            capability,
            reason: CapabilityDenyReason::UserApprovalRequired,
            retryable: false,
        },
        CapabilityBrokerError::CapabilityRevoked(capability) => PluginError::CapabilityDenied {
            extension_id: id.clone(),
            capability,
            reason: CapabilityDenyReason::Revoked,
            retryable: false,
        },
        CapabilityBrokerError::Inactive {
            extension_id,
            capability,
        } => PluginError::CapabilityDenied {
            extension_id,
            capability,
            reason: CapabilityDenyReason::Inactive,
            retryable: true,
        },
        CapabilityBrokerError::InvalidHandle | CapabilityBrokerError::SessionMismatch => {
            PluginError::InvalidContribution("capability handle is no longer valid".into())
        }
        CapabilityBrokerError::GenerationConflict { expected, actual } => {
            PluginError::CapabilityGenerationConflict {
                extension_id: id.clone(),
                expected,
                actual,
            }
        }
        CapabilityBrokerError::RequiredDenied(capability) => PluginError::CapabilityDenied {
            extension_id: id.clone(),
            capability,
            reason: CapabilityDenyReason::UserApprovalRequired,
            retryable: false,
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisableReason {
    User,
    Crash,
    Incompatible,
}

/// Result of one lazy activation request. A failure is recorded for the
/// extension only; callers can continue processing the remaining outcomes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginActivationOutcome {
    pub extension_id: ExtensionId,
    pub activated: bool,
    pub error: Option<String>,
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
pub const MAX_PLUGIN_LOGS: usize = 256;
pub const MAX_PLUGIN_NOTIFICATIONS: usize = 64;
pub const MAX_PLUGIN_LOGS_PER_MINUTE: usize = 120;
pub const MAX_PLUGIN_NOTIFICATIONS_PER_MINUTE: usize = 16;
pub const MAX_PLUGIN_DIAGNOSTIC_MESSAGE_BYTES: usize = 512;
const RECOVERY_LOG_FILE: &str = ".plugin-recovery-log.json";
const SUPPORTED_BRIDGE_API: &str = "1";
const SUPPORTED_RUNTIME_PROTOCOL: &str = "1";
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginLogRecord {
    pub sequence: u64,
    pub extension_id: ExtensionId,
    pub level: PluginLogLevel,
    pub category: String,
    pub message: String,
    pub session_id: Option<String>,
    pub job_id: Option<String>,
    pub operation_id: Option<String>,
    pub retryable: bool,
    pub occurred_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginNotificationSeverity {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginNotificationRecord {
    pub sequence: u64,
    pub extension_id: ExtensionId,
    pub severity: PluginNotificationSeverity,
    pub title: String,
    pub message: String,
    pub session_id: Option<String>,
    pub job_id: Option<String>,
    pub operation_id: Option<String>,
    pub retryable: bool,
    pub occurred_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginLogInput {
    pub extension_id: ExtensionId,
    pub level: PluginLogLevel,
    pub category: String,
    pub message: String,
    pub session_id: Option<String>,
    pub job_id: Option<String>,
    pub operation_id: Option<String>,
    pub retryable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginNotificationInput {
    pub extension_id: ExtensionId,
    pub severity: PluginNotificationSeverity,
    pub title: String,
    pub message: String,
    pub session_id: Option<String>,
    pub job_id: Option<String>,
    pub operation_id: Option<String>,
    pub retryable: bool,
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

#[derive(Debug, Default)]
struct PluginLogSink {
    next_sequence: u64,
    records: std::collections::VecDeque<PluginLogRecord>,
}

impl PluginLogSink {
    fn record(&mut self, input: PluginLogInput) {
        let now = diagnostic_timestamp_ms();
        let cutoff = now.saturating_sub(60_000);
        let recent = self
            .records
            .iter()
            .filter(|record| {
                record.extension_id == input.extension_id && record.occurred_at_ms >= cutoff
            })
            .count();
        if recent >= MAX_PLUGIN_LOGS_PER_MINUTE {
            return;
        }
        let record = PluginLogRecord {
            sequence: self.next_sequence,
            extension_id: input.extension_id,
            level: input.level,
            category: bounded_category(&sanitize_plugin_diagnostic_message(&input.category)),
            message: sanitize_plugin_diagnostic_message(&input.message),
            session_id: input.session_id,
            job_id: input.job_id,
            operation_id: input.operation_id,
            retryable: input.retryable,
            occurred_at_ms: now,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        if self.records.len() == MAX_PLUGIN_LOGS {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }

    fn drain(&mut self) -> Vec<PluginLogRecord> {
        self.records.drain(..).collect()
    }

    fn snapshot(&self) -> Vec<PluginLogRecord> {
        self.records.iter().cloned().collect()
    }
}

#[derive(Debug, Default)]
struct PluginNotificationSink {
    next_sequence: u64,
    records: std::collections::VecDeque<PluginNotificationRecord>,
}

impl PluginNotificationSink {
    fn record(&mut self, input: PluginNotificationInput) {
        let now = diagnostic_timestamp_ms();
        let cutoff = now.saturating_sub(60_000);
        let recent = self
            .records
            .iter()
            .filter(|record| {
                record.extension_id == input.extension_id && record.occurred_at_ms >= cutoff
            })
            .count();
        if recent >= MAX_PLUGIN_NOTIFICATIONS_PER_MINUTE {
            return;
        }
        let record = PluginNotificationRecord {
            sequence: self.next_sequence,
            extension_id: input.extension_id,
            severity: input.severity,
            title: bounded_category(&sanitize_plugin_diagnostic_message(&input.title)),
            message: sanitize_plugin_diagnostic_message(&input.message),
            session_id: input.session_id,
            job_id: input.job_id,
            operation_id: input.operation_id,
            retryable: input.retryable,
            occurred_at_ms: now,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        if self.records.len() == MAX_PLUGIN_NOTIFICATIONS {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }

    fn drain(&mut self) -> Vec<PluginNotificationRecord> {
        self.records.drain(..).collect()
    }

    fn snapshot(&self) -> Vec<PluginNotificationRecord> {
        self.records.iter().cloned().collect()
    }
}

fn bounded_category(value: &str) -> String {
    value.chars().take(64).collect()
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
    #[serde(default)]
    pub deadline_ms: Option<u64>,
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
    #[serde(default = "default_plugin_job_schema_version")]
    pub schema_version: u16,
    pub content_type: String,
    pub payload: Value,
}

fn default_plugin_job_schema_version() -> u16 {
    JOB_SCHEMA_VERSION
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginJobExecution {
    pub result: PluginJobResult,
    pub events: Vec<PluginInvocationEvent>,
    /// The plugin may cooperatively stop its own work by returning a bounded
    /// `cancelled` event. The host still owns the durable Job transition.
    pub cancelled: bool,
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
            deadline_ms: invocation.deadline_ms,
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
        if job_result.schema_version != JOB_SCHEMA_VERSION {
            self.record_diagnostic(
                "job-result-schema-unsupported",
                &format!(
                    "plugin Job result schema {} is unsupported; expected {}",
                    job_result.schema_version, JOB_SCHEMA_VERSION
                ),
                &invocation,
                false,
            );
            return Err(PluginError::RuntimeInvocation {
                extension_id: self.extension_id.clone(),
                message: format!(
                    "plugin Job result schema {} is unsupported; expected {}",
                    job_result.schema_version, JOB_SCHEMA_VERSION
                ),
            });
        }
        Ok(PluginJobExecution {
            result: job_result,
            cancelled: result.events.iter().any(|event| event.kind == "cancelled"),
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
    capabilities: BTreeSet<Capability>,
}

impl PermissionGrant {
    pub fn contains(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    pub fn permissions(&self) -> impl Iterator<Item = &Permission> {
        self.permissions.iter()
    }

    pub fn capabilities(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter()
    }

    fn from_permissions(permissions: impl IntoIterator<Item = Permission>) -> Self {
        let permissions = permissions.into_iter().collect::<BTreeSet<_>>();
        Self {
            capabilities: permissions
                .iter()
                .map(|permission| {
                    Capability::from_name(permission.capability_name())
                        .expect("legacy permission identifiers map to capabilities")
                })
                .collect(),
            permissions,
        }
    }

    fn from_capabilities(capabilities: impl IntoIterator<Item = Capability>) -> Self {
        let capabilities = capabilities.into_iter().collect::<BTreeSet<_>>();
        let permissions = capabilities
            .iter()
            .filter_map(Capability::legacy_permission)
            .collect();
        Self {
            permissions,
            capabilities,
        }
    }

    fn as_env_value(&self) -> String {
        self.permissions
            .iter()
            .map(permission_name)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn as_capability_env_value(&self) -> String {
        self.capabilities
            .iter()
            .map(Capability::name)
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
            .env(
                "VELVET_PLUGIN_CAPABILITIES",
                permissions.as_capability_env_value(),
            )
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

        let (schema_id, schema_version) = match invocation.method.as_str() {
            "job.run" => (
                Some(format!(
                    "velvet.job.{}.{}",
                    invocation.extension_id.0, invocation.command_id
                )),
                Some(JOB_SCHEMA_VERSION),
            ),
            "preview.action" => (
                Some(format!(
                    "velvet.preview.{}.{}",
                    invocation.extension_id.0, invocation.command_id
                )),
                Some(PREVIEW_ACTION_SCHEMA_VERSION),
            ),
            _ => (None, None),
        };
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
            schema_version,
            deadline_ms: invocation.deadline_ms,
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
            .recv_timeout(
                invocation
                    .deadline_ms
                    .map(Duration::from_millis)
                    .unwrap_or(RUNTIME_INVOCATION_TIMEOUT),
            )
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
    #[serde(skip_serializing_if = "Option::is_none")]
    deadline_ms: Option<u64>,
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
    /// Returns the host-owned contribution projection used by transport
    /// adapters and reconnecting clients. Implementations that do not expose
    /// a registry report `RegistryUnavailable` rather than inventing a second
    /// empty state owner.
    fn registry_snapshot(&self) -> Result<ContributionRegistrySnapshot, PluginError> {
        Err(PluginError::RegistryUnavailable)
    }
    fn contribution_handles(
        &self,
        _id: &ExtensionId,
    ) -> Result<Vec<ContributionRegistrationHandle>, PluginError> {
        Err(PluginError::RegistryUnavailable)
    }
    fn dispose_contribution(
        &mut self,
        _handle: &ContributionRegistrationHandle,
    ) -> Result<bool, PluginError> {
        Err(PluginError::RegistryUnavailable)
    }
    fn capability_snapshot(
        &self,
        _id: &ExtensionId,
    ) -> Result<CapabilityGrantSnapshot, PluginError> {
        Err(PluginError::RegistryUnavailable)
    }
    fn grant_capabilities(
        &mut self,
        _id: &ExtensionId,
        _capabilities: &[Capability],
        _expected_generation: Option<u64>,
    ) -> Result<CapabilityGrantSnapshot, PluginError> {
        Err(PluginError::RegistryUnavailable)
    }
    fn revoke_capabilities(
        &mut self,
        _id: &ExtensionId,
        _capabilities: &[Capability],
        _expected_generation: Option<u64>,
    ) -> Result<CapabilityGrantSnapshot, PluginError> {
        Err(PluginError::RegistryUnavailable)
    }
    fn capability_audit(&self) -> Vec<CapabilityAuditRecord> {
        Vec::new()
    }
    fn drain_capability_audit(&mut self) -> Vec<CapabilityAuditRecord> {
        Vec::new()
    }
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
    fn drain_logs(&mut self) -> Vec<PluginLogRecord> {
        Vec::new()
    }
    fn logs_snapshot(&self) -> Vec<PluginLogRecord> {
        Vec::new()
    }
    fn drain_notifications(&mut self) -> Vec<PluginNotificationRecord> {
        Vec::new()
    }
    fn notifications_snapshot(&self) -> Vec<PluginNotificationRecord> {
        Vec::new()
    }
    fn drain_crash_evidence(&mut self) -> Vec<RuntimeCrashEvidence> {
        Vec::new()
    }
}

pub struct InMemoryPluginHost {
    manifests: BTreeMap<ExtensionId, ExtensionManifest>,
    package_roots: BTreeMap<ExtensionId, PathBuf>,
    trust_policy: PluginTrustPolicy,
    trust_records: BTreeMap<ExtensionId, PluginTrustRecord>,
    trust_store: Option<Arc<dyn PluginTrustStore>>,
    active: BTreeSet<ExtensionId>,
    capabilities: CapabilityBroker,
    logs: Arc<Mutex<PluginLogSink>>,
    notifications: Arc<Mutex<PluginNotificationSink>>,
    registry: ContributionRegistry,
    registrations: BTreeMap<ExtensionId, Vec<ContributionRegistrationHandle>>,
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
            trust_policy: PluginTrustPolicy::default(),
            trust_records: BTreeMap::new(),
            trust_store: None,
            active: BTreeSet::new(),
            capabilities: CapabilityBroker::default(),
            logs: Arc::new(Mutex::new(PluginLogSink::default())),
            notifications: Arc::new(Mutex::new(PluginNotificationSink::default())),
            registry: ContributionRegistry::default(),
            registrations: BTreeMap::new(),
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

    /// Returns the authoritative host-owned contribution projection.
    ///
    /// Frontend registries may cache this projection, but they do not own it.
    /// A new generation is published whenever a contribution is registered or
    /// revoked, which gives reconnecting clients a cheap resync check.
    pub fn registry_snapshot(&self) -> ContributionRegistrySnapshot {
        self.registry.snapshot()
    }

    /// Returns opaque disposable handles for one active extension.
    pub fn contribution_handles(
        &self,
        id: &ExtensionId,
    ) -> Result<Vec<ContributionRegistrationHandle>, PluginError> {
        if !self.manifests.contains_key(id) {
            return Err(PluginError::NotFound(id.clone()));
        }
        Ok(self.registrations.get(id).cloned().unwrap_or_default())
    }

    /// Disposes one registration without changing the extension lifecycle.
    /// Repeating the same handle is intentionally idempotent.
    pub fn dispose_contribution(
        &mut self,
        handle: &ContributionRegistrationHandle,
    ) -> Result<bool, PluginError> {
        if !self.manifests.contains_key(handle.extension_id()) {
            return Err(PluginError::NotFound(handle.extension_id().clone()));
        }
        let removed = self.registry.dispose(handle);
        if removed {
            if let Some(handles) = self.registrations.get_mut(handle.extension_id()) {
                handles.retain(|candidate| candidate != handle);
            }
        }
        Ok(removed)
    }

    /// Replaces host policy for capability recomputation. User grants remain
    /// owned by the broker; changing host policy can only reduce effective
    /// grants and never creates a new user approval. Active extensions are
    /// disabled when the new policy invalidates their effective session so a
    /// running runtime cannot continue with stale capability assumptions.
    pub fn set_capability_policy(&mut self, policy: CapabilityPolicy) {
        let active = self
            .active
            .iter()
            .cloned()
            .map(|id| {
                let granted = self
                    .capabilities
                    .snapshot(&id)
                    .map(|snapshot| snapshot.granted.into_iter().collect::<BTreeSet<_>>())
                    .unwrap_or_default();
                (id, granted)
            })
            .collect::<Vec<_>>();
        self.capabilities.set_policy(policy);
        for (id, previous_granted) in active {
            let invalid = self
                .capabilities
                .snapshot(&id)
                .map(|snapshot| {
                    !snapshot.active
                        || previous_granted
                            .iter()
                            .any(|capability| !snapshot.granted.contains(capability))
                })
                .unwrap_or(true);
            if invalid {
                let _ = self.disable(&id, DisableReason::Incompatible);
            }
        }
    }

    pub fn capability_snapshot(
        &self,
        id: &ExtensionId,
    ) -> Result<CapabilityGrantSnapshot, PluginError> {
        self.capabilities
            .snapshot(id)
            .map_err(|error| capability_error(id, error))
    }

    pub fn grant_capabilities(
        &mut self,
        id: &ExtensionId,
        capabilities: &[Capability],
        expected_generation: Option<u64>,
    ) -> Result<CapabilityGrantSnapshot, PluginError> {
        self.capabilities
            .grant(id, capabilities, expected_generation)
            .map_err(|error| capability_error(id, error))
    }

    pub fn revoke_capabilities(
        &mut self,
        id: &ExtensionId,
        capabilities: &[Capability],
        expected_generation: Option<u64>,
    ) -> Result<CapabilityGrantSnapshot, PluginError> {
        self.capabilities
            .revoke(id, capabilities, expected_generation)
            .map_err(|error| capability_error(id, error))
    }

    pub fn capability_handle(
        &self,
        id: &ExtensionId,
        capability: &Capability,
    ) -> Result<CapabilityHandle, PluginError> {
        self.capabilities
            .handle(id, capability)
            .map_err(|error| capability_error(id, error))
    }

    pub fn check_capability_handle(
        &self,
        id: &ExtensionId,
        handle: &CapabilityHandle,
    ) -> Result<(), PluginError> {
        self.capabilities
            .check_handle(id, handle)
            .map_err(|error| capability_error(id, error))
    }

    pub fn drain_capability_audit(&mut self) -> Vec<CapabilityAuditRecord> {
        self.capabilities.drain_audit()
    }

    pub fn capability_audit(&self) -> Vec<CapabilityAuditRecord> {
        self.capabilities.audit_snapshot()
    }

    /// Returns deterministic candidates for a lazy activation trigger.
    /// Matching is deliberately exact; selector expansion belongs to the
    /// owner that defines the event topic, not to the registry.
    pub fn activation_candidates(&self, event: &ActivationEvent) -> Vec<ExtensionId> {
        self.manifests
            .values()
            .filter(|manifest| {
                manifest
                    .activation_events
                    .iter()
                    .any(|candidate| candidate == event)
            })
            .map(|manifest| manifest.id.clone())
            .collect()
    }

    /// Activates every candidate for a trigger while isolating one extension's
    /// failure from the other candidates.
    pub fn activate_for_event(&mut self, event: &ActivationEvent) -> Vec<PluginActivationOutcome> {
        self.activation_candidates(event)
            .into_iter()
            .map(|extension_id| match self.activate(&extension_id) {
                Ok(()) => PluginActivationOutcome {
                    extension_id,
                    activated: true,
                    error: None,
                },
                Err(error) => PluginActivationOutcome {
                    extension_id,
                    activated: false,
                    error: Some(format!("{error:?}")),
                },
            })
            .collect()
    }

    fn activation_order(&self, id: &ExtensionId) -> Result<Vec<ExtensionId>, PluginError> {
        fn visit(
            host: &InMemoryPluginHost,
            id: &ExtensionId,
            visiting: &mut BTreeSet<ExtensionId>,
            visited: &mut BTreeSet<ExtensionId>,
            order: &mut Vec<ExtensionId>,
        ) -> Result<(), PluginError> {
            if visited.contains(id) {
                return Ok(());
            }
            if !visiting.insert(id.clone()) {
                return Err(PluginError::InvalidContribution(format!(
                    "cyclic extension dependency involving {}",
                    id.0
                )));
            }
            let manifest = host
                .manifests
                .get(id)
                .ok_or_else(|| PluginError::NotFound(id.clone()))?;
            let mut dependencies = manifest.extension_dependencies.clone();
            dependencies.sort();
            for dependency in dependencies {
                visit(host, &dependency, visiting, visited, order)?;
            }
            visiting.remove(id);
            visited.insert(id.clone());
            order.push(id.clone());
            Ok(())
        }

        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();
        visit(self, id, &mut visiting, &mut visited, &mut order)?;
        Ok(order)
    }

    fn activate_runtime(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        let Some(manifest) = self.manifests.get(id).cloned() else {
            return Err(PluginError::NotFound(id.clone()));
        };

        if self.active.contains(id) {
            return Ok(());
        }

        let storage_migration_required = match self.runtime_storage_migration_required(&manifest) {
            Ok(required) => required,
            Err(error) => {
                self.capabilities.deactivate(id);
                self.states.insert(id.clone(), ExtensionState::Failed);
                return Err(error);
            }
        };

        self.states.insert(id.clone(), ExtensionState::Starting);
        let (capability_snapshot, granted_permissions) = match self.capabilities.activate(id) {
            Ok(grant) => grant,
            Err(error) => {
                self.capabilities.deactivate(id);
                self.states.insert(id.clone(), ExtensionState::Failed);
                let plugin_error = capability_error(id, error);
                self.record_diagnostic(PluginDiagnosticInput {
                    extension_id: id.clone(),
                    kind: PluginDiagnosticKind::Activation,
                    code: "capability-grant-denied".into(),
                    message: format!("{plugin_error:?}"),
                    job_id: None,
                    attempt: None,
                    retryable: matches!(
                        plugin_error,
                        PluginError::CapabilityDenied {
                            retryable: true,
                            ..
                        }
                    ),
                });
                return Err(plugin_error);
            }
        };
        let mut permissions = PermissionGrant::from_capabilities(capability_snapshot.granted);
        if permissions.permissions.is_empty() && !granted_permissions.is_empty() {
            permissions = PermissionGrant::from_permissions(granted_permissions);
        }
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
                self.capabilities.deactivate(id);
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
                        self.capabilities.deactivate(id);
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
                        self.capabilities.deactivate(id);
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
                if let Err(error) = self.register_contributions(id, &manifest.contributions) {
                    if let Some(process) = self.processes.remove(id) {
                        if let Ok(mut process) = process.lock() {
                            let _ = process.shutdown();
                        }
                    }
                    let runtime_nonce = self.runtime_nonces.remove(id);
                    self.close_runtime_lease(id, runtime_nonce.as_deref());
                    self.capabilities.deactivate(id);
                    self.states.insert(id.clone(), ExtensionState::Failed);
                    self.record_diagnostic(PluginDiagnosticInput {
                        extension_id: id.clone(),
                        kind: PluginDiagnosticKind::Activation,
                        code: "registry-registration-failed".into(),
                        message: format!("contributions could not be registered: {error:?}"),
                        job_id: None,
                        attempt: None,
                        retryable: false,
                    });
                    return Err(error);
                }
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
                self.capabilities.deactivate(id);
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

    pub fn set_plugin_trust_policy(&mut self, policy: PluginTrustPolicy) {
        self.trust_policy = policy;
    }

    pub fn developer_mode_allowed(&self) -> bool {
        self.trust_policy.allows_developer_mode()
    }

    pub fn set_plugin_developer_mode(&mut self, enabled: bool) -> Result<(), PluginError> {
        self.trust_policy.set_developer_mode(enabled)
    }

    pub fn set_plugin_trust_store(
        &mut self,
        store: Arc<dyn PluginTrustStore>,
    ) -> Result<(), PluginError> {
        let records = store.load().map_err(plugin_trust_store_error)?;
        trust_store::validate_records(&records).map_err(plugin_trust_store_error)?;
        self.trust_records = records
            .into_iter()
            .map(|record| (record.extension_id.clone(), record))
            .collect();
        self.trust_store = Some(store);
        Ok(())
    }

    pub fn plugin_trust_record(&self, id: &ExtensionId) -> Result<PluginTrustRecord, PluginError> {
        self.trust_records
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::NotFound(id.clone()))
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

    pub fn record_log(&self, input: PluginLogInput) {
        if let Ok(mut sink) = self.logs.lock() {
            sink.record(input);
        }
    }

    pub fn record_notification(&self, input: PluginNotificationInput) {
        if let Ok(mut sink) = self.notifications.lock() {
            sink.record(input);
        }
    }

    pub fn drain_logs(&mut self) -> Vec<PluginLogRecord> {
        self.logs
            .lock()
            .map(|mut sink| sink.drain())
            .unwrap_or_default()
    }

    pub fn logs_snapshot(&self) -> Vec<PluginLogRecord> {
        self.logs
            .lock()
            .map(|sink| sink.snapshot())
            .unwrap_or_default()
    }

    pub fn drain_notifications(&mut self) -> Vec<PluginNotificationRecord> {
        self.notifications
            .lock()
            .map(|mut sink| sink.drain())
            .unwrap_or_default()
    }

    pub fn notifications_snapshot(&self) -> Vec<PluginNotificationRecord> {
        self.notifications
            .lock()
            .map(|sink| sink.snapshot())
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
        self.active.remove(extension_id);
        self.revoke_contributions(extension_id);
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
            deadline_ms: None,
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
        self.revoke_contributions(id);
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
        self.revoke_contributions(id);
        self.states.insert(id.clone(), ExtensionState::Failed);
    }

    pub fn register_app(&mut self, app_id: String, api: VersionRange) {
        self.apps.insert(app_id, api);
    }

    pub fn reconcile_runtime_orphans(&mut self) -> Result<Vec<RuntimeOrphanReport>, PluginError> {
        let (Some(store), Some(reconciler)) =
            (self.runtime_leases.clone(), self.orphan_reconciler.clone())
        else {
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
                self.active.remove(&lease.extension_id);
                self.revoke_contributions(&lease.extension_id);
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
                    self.active.remove(&lease.extension_id);
                    self.revoke_contributions(&lease.extension_id);
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
                        self.active.remove(&lease.extension_id);
                        self.revoke_contributions(&lease.extension_id);
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
                        self.active.remove(&lease.extension_id);
                        self.revoke_contributions(&lease.extension_id);
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
                    self.active.remove(&lease.extension_id);
                    self.revoke_contributions(&lease.extension_id);
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
        let trust_record = package_root
            .map(|package_root| self.verify_package_trust(&manifest, package_root))
            .transpose()?;
        self.capabilities
            .register(&manifest)
            .map_err(|error| capability_error(&manifest.id, error))?;
        if let Some(trust_record) = &trust_record {
            let mut records = self.trust_records.clone();
            records.insert(manifest.id.clone(), trust_record.clone());
            if let Err(error) = self.persist_trust_records(&records) {
                self.capabilities.unregister(&manifest.id);
                return Err(error);
            }
        }
        if let Some(package_root) = package_root {
            self.package_roots
                .insert(manifest.id.clone(), package_root.to_path_buf());
        } else {
            self.package_roots.remove(&manifest.id);
        }
        self.states
            .insert(manifest.id.clone(), ExtensionState::Registered);
        if let Some(trust_record) = trust_record {
            self.trust_records.insert(manifest.id.clone(), trust_record);
        }
        self.manifests.insert(manifest.id.clone(), manifest);
        Ok(())
    }

    fn persist_trust_records(
        &self,
        records: &BTreeMap<ExtensionId, PluginTrustRecord>,
    ) -> Result<(), PluginError> {
        let Some(store) = self.trust_store.as_ref() else {
            return Ok(());
        };
        let records = records.values().cloned().collect::<Vec<_>>();
        store.save(&records).map_err(plugin_trust_store_error)
    }

    fn verify_package_trust(
        &self,
        manifest: &ExtensionManifest,
        package_root: &Path,
    ) -> Result<PluginTrustRecord, PluginError> {
        let package_sha256 = package_sha256(package_root)?;
        let signature_path = package_root.join(PLUGIN_SIGNATURE_FILE);
        if !signature_path.exists() {
            if self.trust_policy.developer_mode && !self.trust_policy.require_signature {
                return Ok(PluginTrustRecord {
                    schema_version: PLUGIN_TRUST_RECORD_SCHEMA_VERSION,
                    extension_id: manifest.id.clone(),
                    version: manifest.version.clone(),
                    package_sha256,
                    publisher_key_id: None,
                    generation: 0,
                    state: PluginTrustState::DeveloperUnsigned,
                });
            }
            return Err(PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Unknown,
                message: "unsigned package requires explicit developer mode".into(),
            });
        }

        let bytes = fs::read(&signature_path).map_err(|error| PluginError::TrustDenied {
            extension_id: manifest.id.clone(),
            state: PluginTrustState::Unknown,
            message: format!("package signature could not be read: {error}"),
        })?;
        if bytes.len() > MAX_PLUGIN_SIGNATURE_BYTES {
            return Err(PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Blocked,
                message: format!("package signature exceeds {MAX_PLUGIN_SIGNATURE_BYTES} bytes"),
            });
        }
        let envelope =
            serde_json::from_slice::<PluginSignatureEnvelope>(&bytes).map_err(|error| {
                PluginError::TrustDenied {
                    extension_id: manifest.id.clone(),
                    state: PluginTrustState::Blocked,
                    message: format!("package signature is invalid: {error}"),
                }
            })?;
        if envelope.schema_version != PLUGIN_SIGNATURE_SCHEMA_VERSION
            || envelope.extension_id != manifest.id
            || envelope.version != manifest.version
            || envelope.runtime != manifest.runtime
            || envelope.manifest_schema != manifest.manifest_schema
            || envelope.host_api != manifest.host_api
            || envelope.package_sha256 != package_sha256
            || envelope.generation == 0
        {
            return Err(PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Incompatible,
                message: "package signature identity or digest does not match the manifest".into(),
            });
        }
        if self
            .trust_policy
            .revoked_publishers
            .contains(&envelope.publisher_key_id)
        {
            return Err(PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Revoked,
                message: format!("publisher {} is revoked", envelope.publisher_key_id),
            });
        }
        if self
            .trust_policy
            .minimum_generations
            .get(&manifest.id)
            .is_some_and(|minimum| envelope.generation < *minimum)
        {
            return Err(PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Blocked,
                message: "package generation is below the accepted minimum".into(),
            });
        }
        let Some(expected_key) = self
            .trust_policy
            .trusted_publishers
            .get(&envelope.publisher_key_id)
        else {
            return Err(PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Unknown,
                message: format!("publisher {} is not trusted", envelope.publisher_key_id),
            });
        };
        if expected_key != &envelope.publisher_key {
            return Err(PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Blocked,
                message: "package publisher key does not match the trusted key".into(),
            });
        }
        let public_key = decode_hex(&envelope.publisher_key, 32).map_err(|message| {
            PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Blocked,
                message: format!("publisher key is invalid: {message}"),
            }
        })?;
        let public_key: [u8; 32] = public_key.try_into().expect("key length is checked");
        let verifying_key =
            VerifyingKey::from_bytes(&public_key).map_err(|error| PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Blocked,
                message: format!("publisher key is invalid: {error}"),
            })?;
        let signature =
            decode_hex(&envelope.signature, 64).map_err(|message| PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Blocked,
                message: format!("package signature is invalid: {message}"),
            })?;
        let signature =
            Signature::from_slice(&signature).map_err(|error| PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Blocked,
                message: format!("package signature is invalid: {error}"),
            })?;
        verifying_key
            .verify(&canonical_plugin_signature_payload(&envelope), &signature)
            .map_err(|_| PluginError::TrustDenied {
                extension_id: manifest.id.clone(),
                state: PluginTrustState::Blocked,
                message: "package signature verification failed".into(),
            })?;
        Ok(PluginTrustRecord {
            schema_version: PLUGIN_TRUST_RECORD_SCHEMA_VERSION,
            extension_id: manifest.id.clone(),
            version: manifest.version.clone(),
            package_sha256,
            publisher_key_id: Some(envelope.publisher_key_id),
            generation: envelope.generation,
            state: PluginTrustState::Trusted,
        })
    }

    fn validate_manifest(&self, manifest: &ExtensionManifest) -> Result<(), PluginError> {
        manifest
            .validate_contract()
            .map_err(|error| PluginError::InvalidContribution(error.to_string()))?;
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
        if !api_compatible(
            &manifest.bridge_api,
            &VersionRange(SUPPORTED_BRIDGE_API.into()),
        ) {
            return Err(PluginError::InvalidContribution(format!(
                "bridge API range {} is incompatible with host {}",
                manifest.bridge_api.0, SUPPORTED_BRIDGE_API
            )));
        }
        if !api_compatible(
            &manifest.runtime_protocol,
            &VersionRange(SUPPORTED_RUNTIME_PROTOCOL.into()),
        ) {
            return Err(PluginError::InvalidContribution(format!(
                "runtime protocol range {} is incompatible with host {}",
                manifest.runtime_protocol.0, SUPPORTED_RUNTIME_PROTOCOL
            )));
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
        self.revoke_contributions(id);
        self.capabilities.unregister(id);
        let runtime_nonce = self.runtime_nonces.remove(id);
        self.close_runtime_lease(id, runtime_nonce.as_deref());
        self.manifests.remove(id);
        self.package_roots.remove(id);
        self.active.remove(id);
        self.states.remove(id);
        self.processes.remove(id);
    }

    fn register_contributions(
        &mut self,
        id: &ExtensionId,
        contributions: &[velvet_plugin_api::ContributionDescriptor],
    ) -> Result<(), PluginError> {
        for contribution in contributions {
            for capability_name in &contribution.required_capabilities {
                let capability = Capability::from_name(capability_name).ok_or_else(|| {
                    PluginError::InvalidContribution(format!(
                        "contribution {} declares unknown capability {}",
                        contribution.id, capability_name
                    ))
                })?;
                if !self
                    .capabilities
                    .is_granted(id, &capability)
                    .map_err(|error| capability_error(id, error))?
                {
                    return Err(PluginError::CapabilityDenied {
                        extension_id: id.clone(),
                        capability,
                        reason: CapabilityDenyReason::UserApprovalRequired,
                        retryable: false,
                    });
                }
            }
        }
        let handles = self
            .registry
            .register_extension(id, contributions)
            .map_err(|error| match error {
                RegistryError::Duplicate(key)
                | RegistryError::InvalidNamespace(key)
                | RegistryError::InvalidCommandSurface(key)
                | RegistryError::KeybindingConflict(key) => PluginError::InvalidContribution(key),
            })?;
        self.registrations.insert(id.clone(), handles);
        Ok(())
    }

    fn revoke_contributions(&mut self, id: &ExtensionId) {
        self.capabilities.deactivate(id);
        self.registry.revoke_extension(id);
        self.registrations.remove(id);
    }

    fn contribution_is_active(
        &self,
        id: &ExtensionId,
        contribution_id: &str,
        kind: &ContributionKind,
    ) -> bool {
        self.registry.contains(id, contribution_id, kind)
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

        if !self.contribution_is_active(id, &contribution_id, &expected_kind) {
            return Err(PluginError::InvalidContribution(format!(
                "contribution {} is not declared by extension {}",
                contribution_id, id.0
            )));
        }

        let deadline_ms = command_deadline_ms(&context);
        let invocation = PluginInvocation {
            extension_id: id.clone(),
            method: method.into(),
            command_id: contribution_id,
            args,
            context,
            deadline_ms,
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
                self.revoke_contributions(id);
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
        if !expected_kinds.contains(&contribution.kind)
            || contribution.preview.is_none()
            || !self.contribution_is_active(id, &contribution_id, &contribution.kind)
        {
            return Err(PluginError::InvalidContribution(format!(
                "{} is not a {label} contribution",
                contribution_id,
            )));
        }
        if !self.has_permission(id, Permission::PreviewRender)? {
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
        let capability = Capability::from_name(permission.capability_name())
            .expect("every legacy permission has a capability identifier");
        self.capabilities
            .is_granted(id, &capability)
            .map_err(|error| capability_error(id, error))
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
        let order = self.activation_order(id)?;
        for dependency in order {
            if !self.active.contains(&dependency) {
                self.activate_runtime(&dependency)?;
            }
        }
        Ok(())
    }

    fn disable(&mut self, id: &ExtensionId, _reason: DisableReason) -> Result<(), PluginError> {
        if !self.manifests.contains_key(id) {
            return Err(PluginError::NotFound(id.clone()));
        }

        self.active.remove(id);
        self.revoke_contributions(id);
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
        if !self.manifests.contains_key(id) {
            return Err(PluginError::NotFound(id.clone()));
        }
        Ok(self
            .registry
            .for_extension(id)
            .into_iter()
            .map(|registration| registration.descriptor)
            .collect())
    }

    fn registry_snapshot(&self) -> Result<ContributionRegistrySnapshot, PluginError> {
        Ok(InMemoryPluginHost::registry_snapshot(self))
    }

    fn contribution_handles(
        &self,
        id: &ExtensionId,
    ) -> Result<Vec<ContributionRegistrationHandle>, PluginError> {
        InMemoryPluginHost::contribution_handles(self, id)
    }

    fn dispose_contribution(
        &mut self,
        handle: &ContributionRegistrationHandle,
    ) -> Result<bool, PluginError> {
        InMemoryPluginHost::dispose_contribution(self, handle)
    }

    fn capability_snapshot(
        &self,
        id: &ExtensionId,
    ) -> Result<CapabilityGrantSnapshot, PluginError> {
        InMemoryPluginHost::capability_snapshot(self, id)
    }

    fn grant_capabilities(
        &mut self,
        id: &ExtensionId,
        capabilities: &[Capability],
        expected_generation: Option<u64>,
    ) -> Result<CapabilityGrantSnapshot, PluginError> {
        InMemoryPluginHost::grant_capabilities(self, id, capabilities, expected_generation)
    }

    fn revoke_capabilities(
        &mut self,
        id: &ExtensionId,
        capabilities: &[Capability],
        expected_generation: Option<u64>,
    ) -> Result<CapabilityGrantSnapshot, PluginError> {
        InMemoryPluginHost::revoke_capabilities(self, id, capabilities, expected_generation)
    }

    fn capability_audit(&self) -> Vec<CapabilityAuditRecord> {
        InMemoryPluginHost::capability_audit(self)
    }

    fn drain_capability_audit(&mut self) -> Vec<CapabilityAuditRecord> {
        InMemoryPluginHost::drain_capability_audit(self)
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
        let job_kinds = self
            .active_contributions(id)?
            .iter()
            .filter(|contribution| contribution.kind == ContributionKind::Job)
            .map(|contribution| contribution.id.clone())
            .collect::<Vec<_>>();
        if job_kinds.is_empty() {
            return Ok(Vec::new());
        }
        if !self.has_permission(id, Permission::JobsCreate)? {
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
                self.revoke_contributions(id);
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
            if let Err(error) = self.verify_package_trust(&manifest, source_dir) {
                issues.push(format!("package trust is not ready: {error:?}"));
            }
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
        self.verify_package_trust(&manifest, source_dir)?;
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
        self.revoke_contributions(id);
        self.capabilities.unregister(id);
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

    fn drain_logs(&mut self) -> Vec<PluginLogRecord> {
        InMemoryPluginHost::drain_logs(self)
    }

    fn logs_snapshot(&self) -> Vec<PluginLogRecord> {
        InMemoryPluginHost::logs_snapshot(self)
    }

    fn drain_notifications(&mut self) -> Vec<PluginNotificationRecord> {
        InMemoryPluginHost::drain_notifications(self)
    }

    fn notifications_snapshot(&self) -> Vec<PluginNotificationRecord> {
        InMemoryPluginHost::notifications_snapshot(self)
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

fn command_deadline_ms(context: &Value) -> Option<u64> {
    context
        .get("_velvet")
        .and_then(|metadata| metadata.get("deadlineMs"))
        .and_then(Value::as_u64)
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
    required.is_compatible_with(provided)
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginSignaturePayload<'a> {
    schema_version: u16,
    publisher_key_id: &'a str,
    publisher_key: &'a str,
    package_sha256: &'a str,
    extension_id: &'a ExtensionId,
    version: &'a str,
    runtime: &'a RuntimeKind,
    manifest_schema: u16,
    host_api: &'a VersionRange,
    generation: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginPublisherKeyRotationPayload<'a> {
    schema_version: u16,
    old_key_id: &'a str,
    old_public_key: &'a str,
    new_key_id: &'a str,
    new_public_key: &'a str,
    sequence: u64,
}

fn canonical_plugin_publisher_key_rotation_payload(
    rotation: &PluginPublisherKeyRotation,
) -> Vec<u8> {
    serde_json::to_vec(&PluginPublisherKeyRotationPayload {
        schema_version: rotation.schema_version,
        old_key_id: &rotation.old_key_id,
        old_public_key: &rotation.old_public_key,
        new_key_id: &rotation.new_key_id,
        new_public_key: &rotation.new_public_key,
        sequence: rotation.sequence,
    })
    .expect("publisher key rotation payload should serialize")
}

fn decode_verifying_key(value: &str) -> Result<VerifyingKey, String> {
    let bytes = decode_hex(value, 32)?;
    let bytes: [u8; 32] = bytes.try_into().expect("key length is checked");
    VerifyingKey::from_bytes(&bytes).map_err(|error| error.to_string())
}

fn decode_signature(value: &str) -> Result<Signature, String> {
    let bytes = decode_hex(value, 64)?;
    Signature::from_slice(&bytes).map_err(|error| error.to_string())
}

fn canonical_plugin_signature_payload(envelope: &PluginSignatureEnvelope) -> Vec<u8> {
    serde_json::to_vec(&PluginSignaturePayload {
        schema_version: envelope.schema_version,
        publisher_key_id: &envelope.publisher_key_id,
        publisher_key: &envelope.publisher_key,
        package_sha256: &envelope.package_sha256,
        extension_id: &envelope.extension_id,
        version: &envelope.version,
        runtime: &envelope.runtime,
        manifest_schema: envelope.manifest_schema,
        host_api: &envelope.host_api,
        generation: envelope.generation,
    })
    .expect("plugin signature payload should serialize")
}

fn package_sha256(root: &Path) -> Result<String, PluginError> {
    let mut files = Vec::new();
    collect_package_files(root, Path::new(""), &mut files)?;
    files.sort();
    let mut digest = Sha256::new();
    for relative in files {
        let relative_text = relative.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(root.join(&relative)).map_err(|error| {
            PluginError::InstallFailed(format!(
                "package file {} could not be read: {error}",
                relative_text
            ))
        })?;
        digest.update((relative_text.len() as u64).to_be_bytes());
        digest.update(relative_text.as_bytes());
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    Ok(encode_hex(&digest.finalize()))
}

fn collect_package_files(
    root: &Path,
    relative_root: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), PluginError> {
    let entries = fs::read_dir(root).map_err(|error| {
        PluginError::InstallFailed(format!("package directory could not be read: {error}"))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            PluginError::InstallFailed(format!("package entry could not be read: {error}"))
        })?;
        let path = entry.path();
        let relative = relative_root.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| {
            PluginError::InstallFailed(format!("package entry type could not be read: {error}"))
        })?;
        if file_type.is_dir() {
            collect_package_files(&path, &relative, files)?;
        } else if file_type.is_file() {
            if relative != Path::new(PLUGIN_SIGNATURE_FILE) {
                files.push(relative);
            }
        } else {
            return Err(PluginError::InstallFailed(
                "plugin packages cannot contain symbolic links or special files".into(),
            ));
        }
    }
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn decode_hex(value: &str, expected_bytes: usize) -> Result<Vec<u8>, String> {
    if value.len() != expected_bytes * 2 {
        return Err(format!("expected {expected_bytes} bytes"));
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(expected_bytes);
    for index in (0..bytes.len()).step_by(2) {
        let high = hex_digit(bytes[index]).ok_or_else(|| "non-hex digit".to_owned())?;
        let low = hex_digit(bytes[index + 1]).ok_or_else(|| "non-hex digit".to_owned())?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
