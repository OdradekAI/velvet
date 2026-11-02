use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use semver::Version;
use serde::Serialize;
use sha2::{Digest, Sha256};
use velvet_persistence::{
    RecoverySetStore, ReleasePackageStore, ReleaseStoreError, ReleaseUpdateStore,
    CORE_RECOVERY_SET_ID,
};
use velvet_workbench_protocol::{
    ReleaseChannel, ReleaseManifestDto, ReleaseStatusDto, ReleaseTransactionDto,
    ReleaseTransactionState, RELEASE_MANIFEST_SCHEMA_VERSION, RELEASE_TRANSACTION_SCHEMA_VERSION,
};

pub const RELEASE_HEALTH_CHECKPOINT: &str = "workbench-ready";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseVerificationError {
    Schema(u16),
    InvalidField(&'static str),
    InvalidSignature,
    UnknownSigningKey(String),
    TargetMismatch { expected: String, received: String },
    NotYetValid,
    Expired,
    StaleSequence { installed: u64, received: u64 },
    ArtifactDigestMismatch,
    ClientVersionTooOld { current: String, minimum: String },
}

impl std::fmt::Display for ReleaseVerificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schema(version) => {
                write!(formatter, "unsupported release manifest schema {version}")
            }
            Self::InvalidField(field) => {
                write!(formatter, "invalid release manifest field {field}")
            }
            Self::InvalidSignature => formatter.write_str("release manifest signature is invalid"),
            Self::UnknownSigningKey(key) => {
                write!(formatter, "release signing key {key} is not pinned")
            }
            Self::TargetMismatch { expected, received } => {
                write!(
                    formatter,
                    "release target {received} does not match {expected}"
                )
            }
            Self::NotYetValid => formatter.write_str("release manifest is not valid yet"),
            Self::Expired => formatter.write_str("release manifest has expired"),
            Self::StaleSequence {
                installed,
                received,
            } => {
                write!(
                    formatter,
                    "release sequence {received} is not newer than {installed}"
                )
            }
            Self::ArtifactDigestMismatch => {
                formatter.write_str("release artifact digest does not match the manifest")
            }
            Self::ClientVersionTooOld { current, minimum } => {
                write!(
                    formatter,
                    "client version {current} is below the release minimum {minimum}"
                )
            }
        }
    }
}

#[derive(Clone)]
pub struct ReleaseTrustPolicy {
    pub key_id: String,
    pub root_key: VerifyingKey,
    pub allowed_channel: ReleaseChannel,
    pub max_clock_skew_ms: u64,
    pub current_data_schema: u16,
    pub current_client_version: Version,
    pub supported_updater_schema: u16,
}

impl ReleaseTrustPolicy {
    pub fn new(
        key_id: impl Into<String>,
        root_key: VerifyingKey,
        allowed_channel: ReleaseChannel,
    ) -> Self {
        Self {
            key_id: key_id.into(),
            root_key,
            allowed_channel,
            max_clock_skew_ms: 5 * 60 * 1000,
            current_data_schema: 1,
            current_client_version: Version::parse(env!("CARGO_PKG_VERSION"))
                .expect("application core version must be semver"),
            supported_updater_schema: 1,
        }
    }

    pub fn verify_manifest(
        &self,
        manifest: &ReleaseManifestDto,
        now_ms: u64,
        expected_target: &str,
        installed_sequence: u64,
    ) -> Result<(), ReleaseVerificationError> {
        if manifest.schema_version != RELEASE_MANIFEST_SCHEMA_VERSION {
            return Err(ReleaseVerificationError::Schema(manifest.schema_version));
        }
        if manifest.version.is_empty()
            || manifest.target.is_empty()
            || manifest.min_client_version.is_empty()
            || manifest.signing_key_id.is_empty()
        {
            return Err(ReleaseVerificationError::InvalidField("identity"));
        }
        if manifest.artifact_sha256.len() != 64
            || !manifest
                .artifact_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ReleaseVerificationError::InvalidField("artifactSha256"));
        }
        if manifest.expires_at_ms <= manifest.issued_at_ms {
            return Err(ReleaseVerificationError::InvalidField("validity"));
        }
        if manifest.min_data_schema > manifest.max_data_schema
            || self.current_data_schema < manifest.min_data_schema
            || self.current_data_schema > manifest.max_data_schema
        {
            return Err(ReleaseVerificationError::InvalidField("dataSchemaRange"));
        }
        if manifest.updater_schema != self.supported_updater_schema {
            return Err(ReleaseVerificationError::InvalidField("updaterSchema"));
        }
        let minimum_client = Version::parse(&manifest.min_client_version)
            .map_err(|_| ReleaseVerificationError::InvalidField("minClientVersion"))?;
        if self.current_client_version < minimum_client {
            return Err(ReleaseVerificationError::ClientVersionTooOld {
                current: self.current_client_version.to_string(),
                minimum: manifest.min_client_version.clone(),
            });
        }
        if manifest.signing_key_id != self.key_id {
            return Err(ReleaseVerificationError::UnknownSigningKey(
                manifest.signing_key_id.clone(),
            ));
        }
        if manifest.channel != self.allowed_channel {
            return Err(ReleaseVerificationError::InvalidField("channel"));
        }
        if manifest.target != expected_target {
            return Err(ReleaseVerificationError::TargetMismatch {
                expected: expected_target.into(),
                received: manifest.target.clone(),
            });
        }
        if manifest.issued_at_ms > now_ms.saturating_add(self.max_clock_skew_ms) {
            return Err(ReleaseVerificationError::NotYetValid);
        }
        if manifest.expires_at_ms < now_ms.saturating_sub(self.max_clock_skew_ms) {
            return Err(ReleaseVerificationError::Expired);
        }
        if manifest.release_sequence <= installed_sequence {
            return Err(ReleaseVerificationError::StaleSequence {
                installed: installed_sequence,
                received: manifest.release_sequence,
            });
        }
        let signature = decode_hex(&manifest.signature)
            .and_then(|bytes| Signature::from_slice(&bytes).ok())
            .ok_or(ReleaseVerificationError::InvalidSignature)?;
        self.root_key
            .verify(&canonical_unsigned_manifest(manifest), &signature)
            .map_err(|_| ReleaseVerificationError::InvalidSignature)
    }

    pub fn verify_artifact_digest(
        &self,
        manifest: &ReleaseManifestDto,
        artifact: &[u8],
    ) -> Result<(), ReleaseVerificationError> {
        let digest = Sha256::digest(artifact);
        if encode_hex(&digest) == manifest.artifact_sha256.to_ascii_lowercase() {
            Ok(())
        } else {
            Err(ReleaseVerificationError::ArtifactDigestMismatch)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseUpdateError {
    Store(String),
    Verification(String),
    NoManager,
    NoPendingTransaction,
    TransactionMismatch,
    InvalidState(String),
}

impl From<ReleaseStoreError> for ReleaseUpdateError {
    fn from(error: ReleaseStoreError) -> Self {
        map_store_error(error)
    }
}

impl std::fmt::Display for ReleaseUpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(message) => write!(formatter, "release store: {message}"),
            Self::Verification(message) => write!(formatter, "release verification: {message}"),
            Self::NoManager => formatter.write_str("release update manager is unavailable"),
            Self::NoPendingTransaction => {
                formatter.write_str("there is no pending release transaction")
            }
            Self::TransactionMismatch => {
                formatter.write_str("release transaction id does not match the pending transaction")
            }
            Self::InvalidState(message) => {
                write!(formatter, "invalid release transaction state: {message}")
            }
        }
    }
}

pub struct ReleaseUpdateManager {
    store: Arc<dyn ReleaseUpdateStore>,
    packages: Option<Arc<dyn ReleasePackageStore>>,
    recovery_sets: Option<Arc<dyn RecoverySetStore>>,
    trust: ReleaseTrustPolicy,
}

impl ReleaseUpdateManager {
    pub fn new(
        store: Arc<dyn ReleaseUpdateStore>,
        packages: Option<Arc<dyn ReleasePackageStore>>,
        recovery_sets: Option<Arc<dyn RecoverySetStore>>,
        trust: ReleaseTrustPolicy,
    ) -> Self {
        Self {
            store,
            packages,
            recovery_sets,
            trust,
        }
    }

    pub fn status(&self) -> Result<ReleaseStatusDto, ReleaseUpdateError> {
        self.store.load().map_err(map_store_error)
    }

    /// Stage package bytes through the native adapter before sending the
    /// path-free `stage-release` transport command. The returned token is the
    /// only package identity that crosses the protocol boundary.
    pub fn stage_package(
        &self,
        transaction_id: &str,
        source: &std::path::Path,
        expected_sha256: &str,
    ) -> Result<String, ReleaseUpdateError> {
        self.packages
            .as_ref()
            .ok_or_else(|| {
                ReleaseUpdateError::InvalidState("package staging is unavailable".into())
            })?
            .stage_package(transaction_id, source, expected_sha256)
            .map_err(map_store_error)
    }

    pub fn stage_package_bytes(
        &self,
        transaction_id: &str,
        bytes: &[u8],
        expected_sha256: &str,
    ) -> Result<String, ReleaseUpdateError> {
        self.packages
            .as_ref()
            .ok_or_else(|| {
                ReleaseUpdateError::InvalidState("package staging is unavailable".into())
            })?
            .stage_package_bytes(transaction_id, bytes, expected_sha256)
            .map_err(map_store_error)
    }

    pub fn validate_manifest(
        &self,
        manifest: &ReleaseManifestDto,
        now_ms: u64,
        expected_target: &str,
    ) -> Result<(), ReleaseUpdateError> {
        let status = self.status()?;
        self.trust
            .verify_manifest(manifest, now_ms, expected_target, status.committed_sequence)
            .map_err(|error| ReleaseUpdateError::Verification(error.to_string()))
    }

    pub fn discard_package(&self, staged_package_id: &str) -> Result<(), ReleaseUpdateError> {
        self.packages
            .as_ref()
            .ok_or_else(|| {
                ReleaseUpdateError::InvalidState("package staging is unavailable".into())
            })?
            .rollback_package(staged_package_id)
            .map_err(map_store_error)
    }

    pub fn stage(
        &self,
        transaction_id: String,
        manifest: ReleaseManifestDto,
        staged_package_id: String,
        package_generation: String,
    ) -> Result<ReleaseStatusDto, ReleaseUpdateError> {
        let now_ms = now_ms();
        self.stage_at(
            transaction_id,
            manifest,
            staged_package_id,
            package_generation,
            now_ms,
            &format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        )
    }

    pub fn stage_at(
        &self,
        transaction_id: String,
        manifest: ReleaseManifestDto,
        staged_package_id: String,
        package_generation: String,
        now_ms: u64,
        expected_target: &str,
    ) -> Result<ReleaseStatusDto, ReleaseUpdateError> {
        validate_token(&transaction_id, "transaction id")?;
        validate_token(&staged_package_id, "staged package id")?;
        validate_token(&package_generation, "package generation")?;
        let mut status = self.status()?;
        if status.pending.is_some() {
            return Err(ReleaseUpdateError::InvalidState(
                "another release transaction is already staged".into(),
            ));
        }
        self.trust
            .verify_manifest(
                &manifest,
                now_ms,
                expected_target,
                status.committed_sequence,
            )
            .map_err(|error| ReleaseUpdateError::Verification(error.to_string()))?;
        if let Some(packages) = &self.packages {
            let staged = packages
                .is_staged(&staged_package_id)
                .map_err(map_store_error)?;
            if !staged {
                return Err(ReleaseUpdateError::InvalidState(
                    "the staged package token does not refer to a verified package".into(),
                ));
            }
        }
        let base_data_generation = match &self.recovery_sets {
            Some(sets) => sets
                .load_latest(CORE_RECOVERY_SET_ID)
                .map_err(|error| ReleaseUpdateError::Store(format!("recovery set: {error:?}")))?
                .map(|set| set.generation),
            None => None,
        };
        status.pending = Some(ReleaseTransactionDto {
            schema_version: RELEASE_TRANSACTION_SCHEMA_VERSION,
            transaction_id,
            manifest,
            staged_package_id,
            package_generation,
            base_data_generation,
            committed_data_generation: None,
            state: ReleaseTransactionState::Staged,
            health_checkpoint: None,
            failure_reason: None,
            created_at_ms: now_ms,
        });
        self.store.save(&status)?;
        Ok(status)
    }

    pub fn commit_health(
        &self,
        transaction_id: &str,
        checkpoint: &str,
    ) -> Result<ReleaseStatusDto, ReleaseUpdateError> {
        if checkpoint != RELEASE_HEALTH_CHECKPOINT {
            return Err(ReleaseUpdateError::InvalidState(format!(
                "health checkpoint must be {RELEASE_HEALTH_CHECKPOINT}"
            )));
        }
        let mut status = self.status()?;
        let mut transaction = status
            .pending
            .take()
            .ok_or(ReleaseUpdateError::NoPendingTransaction)?;
        if transaction.transaction_id != transaction_id {
            status.pending = Some(transaction);
            return Err(ReleaseUpdateError::TransactionMismatch);
        }
        if transaction.state != ReleaseTransactionState::Staged {
            return Err(ReleaseUpdateError::InvalidState(
                "transaction is not staged".into(),
            ));
        }
        transaction.state = ReleaseTransactionState::Committed;
        transaction.health_checkpoint = Some(checkpoint.into());
        transaction.committed_data_generation = match &self.recovery_sets {
            Some(sets) => {
                let current = sets.load_latest(CORE_RECOVERY_SET_ID).map_err(|error| {
                    ReleaseUpdateError::Store(format!("recovery set: {error:?}"))
                })?;
                if transaction.base_data_generation.is_some() && current.is_none() {
                    return Err(ReleaseUpdateError::InvalidState(
                        "the staged data generation is no longer available".into(),
                    ));
                }
                current.map(|set| set.generation)
            }
            None => transaction.base_data_generation,
        };
        if let Some(packages) = &self.packages {
            packages
                .commit_package(
                    &transaction.staged_package_id,
                    &transaction.package_generation,
                )
                .map_err(map_store_error)?;
        }
        status.committed_sequence = transaction.manifest.release_sequence;
        status.committed_package_generation = transaction.package_generation.clone();
        status.last_transaction = Some(transaction);
        self.store.save(&status)?;
        Ok(status)
    }

    pub fn rollback(
        &self,
        transaction_id: &str,
        reason: String,
    ) -> Result<ReleaseStatusDto, ReleaseUpdateError> {
        if reason.trim().is_empty() {
            return Err(ReleaseUpdateError::InvalidState(
                "rollback reason is required".into(),
            ));
        }
        let mut status = self.status()?;
        let mut transaction = status
            .pending
            .take()
            .ok_or(ReleaseUpdateError::NoPendingTransaction)?;
        if transaction.transaction_id != transaction_id {
            status.pending = Some(transaction);
            return Err(ReleaseUpdateError::TransactionMismatch);
        }
        transaction.state = ReleaseTransactionState::RolledBack;
        transaction.failure_reason = Some(reason);
        if let Some(packages) = &self.packages {
            packages
                .rollback_package(&transaction.staged_package_id)
                .map_err(map_store_error)?;
        }
        status.last_transaction = Some(transaction);
        self.store.save(&status)?;
        Ok(status)
    }
}

fn map_store_error(error: ReleaseStoreError) -> ReleaseUpdateError {
    match error {
        ReleaseStoreError::Unavailable(message) | ReleaseStoreError::Corrupt(message) => {
            ReleaseUpdateError::Store(message)
        }
    }
}

fn validate_token(value: &str, field: &'static str) -> Result<(), ReleaseUpdateError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ReleaseUpdateError::InvalidState(format!(
            "{field} must be a bounded opaque token"
        )));
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnsignedReleaseManifest<'a> {
    schema_version: u16,
    channel: &'a ReleaseChannel,
    version: &'a str,
    target: &'a str,
    artifact_sha256: &'a str,
    updater_schema: u16,
    min_data_schema: u16,
    max_data_schema: u16,
    min_client_version: &'a str,
    release_sequence: u64,
    issued_at_ms: u64,
    expires_at_ms: u64,
    signing_key_id: &'a str,
}

pub fn canonical_unsigned_manifest(manifest: &ReleaseManifestDto) -> Vec<u8> {
    serde_json::to_vec(&UnsignedReleaseManifest {
        schema_version: manifest.schema_version,
        channel: &manifest.channel,
        version: &manifest.version,
        target: &manifest.target,
        artifact_sha256: &manifest.artifact_sha256,
        updater_schema: manifest.updater_schema,
        min_data_schema: manifest.min_data_schema,
        max_data_schema: manifest.max_data_schema,
        min_client_version: &manifest.min_client_version,
        release_sequence: manifest.release_sequence,
        issued_at_ms: manifest.issued_at_ms,
        expires_at_ms: manifest.expires_at_ms,
        signing_key_id: &manifest.signing_key_id,
    })
    .expect("release manifest fields are serializable")
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len() / 2);
    for index in (0..bytes.len()).step_by(2) {
        let high = (bytes[index] as char).to_digit(16)? as u8;
        let low = (bytes[index + 1] as char).to_digit(16)? as u8;
        decoded.push((high << 4) | low);
    }
    Some(decoded)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use velvet_persistence::InMemoryReleaseUpdateStore;

    fn fixture() -> (ReleaseTrustPolicy, ReleaseManifestDto) {
        let signing_key = SigningKey::from_bytes(&[9; 32]);
        let verifying_key = signing_key.verifying_key();
        let mut manifest = ReleaseManifestDto {
            schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
            channel: ReleaseChannel::Preview,
            version: "0.0.5".into(),
            target: "test-target".into(),
            artifact_sha256: "0".repeat(64),
            updater_schema: 1,
            min_data_schema: 1,
            max_data_schema: 1,
            min_client_version: "0.0.1".into(),
            release_sequence: 1,
            issued_at_ms: 100,
            expires_at_ms: 1_000,
            signing_key_id: "root-1".into(),
            signature: String::new(),
        };
        let signature =
            ed25519_dalek::Signer::sign(&signing_key, &canonical_unsigned_manifest(&manifest));
        manifest.signature = encode_hex(&signature.to_bytes());
        (
            ReleaseTrustPolicy::new("root-1", verifying_key, ReleaseChannel::Preview),
            manifest,
        )
    }

    #[test]
    fn verifies_signature_target_freshness_and_artifact_digest() {
        let (trust, mut manifest) = fixture();
        let artifact = b"release-package";
        manifest.artifact_sha256 = encode_hex(&Sha256::digest(artifact));
        let signing_key = SigningKey::from_bytes(&[9; 32]);
        manifest.signature = encode_hex(
            &ed25519_dalek::Signer::sign(&signing_key, &canonical_unsigned_manifest(&manifest))
                .to_bytes(),
        );
        trust
            .verify_manifest(&manifest, 200, "test-target", 0)
            .expect("manifest should verify");
        trust
            .verify_artifact_digest(&manifest, artifact)
            .expect("artifact digest");
    }

    #[test]
    fn rejects_stale_manifest_and_bad_signature() {
        let (trust, manifest) = fixture();
        assert!(matches!(
            trust.verify_manifest(&manifest, 200, "test-target", 1),
            Err(ReleaseVerificationError::StaleSequence { .. })
        ));
        let mut tampered = manifest;
        tampered.version = "0.0.6".into();
        assert!(matches!(
            trust.verify_manifest(&tampered, 200, "test-target", 0),
            Err(ReleaseVerificationError::InvalidSignature)
        ));
    }

    #[test]
    fn rejects_incompatible_data_schema_and_client_floor() {
        let (mut trust, mut manifest) = fixture();
        manifest.min_data_schema = 2;
        manifest.max_data_schema = 3;
        assert!(matches!(
            trust.verify_manifest(&manifest, 200, "test-target", 0),
            Err(ReleaseVerificationError::InvalidField("dataSchemaRange"))
        ));
        manifest.min_data_schema = 1;
        manifest.max_data_schema = 1;
        manifest.min_client_version = "99.0.0".into();
        let signing_key = SigningKey::from_bytes(&[9; 32]);
        manifest.signature = encode_hex(
            &ed25519_dalek::Signer::sign(&signing_key, &canonical_unsigned_manifest(&manifest))
                .to_bytes(),
        );
        assert!(matches!(
            trust.verify_manifest(&manifest, 200, "test-target", 0),
            Err(ReleaseVerificationError::ClientVersionTooOld { .. })
        ));
        trust.current_data_schema = 2;
        assert!(matches!(
            trust.verify_manifest(&manifest, 200, "test-target", 0),
            Err(ReleaseVerificationError::InvalidField("dataSchemaRange"))
        ));
    }

    #[test]
    fn stages_commits_and_rolls_back_without_executing_a_package() {
        let (trust, manifest) = fixture();
        let store = Arc::new(InMemoryReleaseUpdateStore::new());
        let manager = ReleaseUpdateManager::new(store, None, None, trust);
        let status = manager
            .stage_at(
                "txn-1".into(),
                manifest,
                "stage-1".into(),
                "pkg-1".into(),
                200,
                "test-target",
            )
            .expect("stage");
        assert!(status.pending.is_some());
        let status = manager
            .commit_health("txn-1", RELEASE_HEALTH_CHECKPOINT)
            .expect("commit");
        assert_eq!(status.committed_package_generation, "pkg-1");
        let error = manager
            .rollback("txn-1", "too late".into())
            .expect_err("committed transaction cannot roll back");
        assert!(matches!(error, ReleaseUpdateError::NoPendingTransaction));
    }

    #[test]
    fn rollback_preserves_auditable_last_transaction() {
        let (trust, manifest) = fixture();
        let store = Arc::new(InMemoryReleaseUpdateStore::new());
        let manager = ReleaseUpdateManager::new(store, None, None, trust);
        manager
            .stage_at(
                "txn-2".into(),
                manifest,
                "stage-2".into(),
                "pkg-2".into(),
                200,
                "test-target",
            )
            .expect("stage");
        let status = manager
            .rollback("txn-2", "health check failed".into())
            .expect("rollback");
        assert!(status.pending.is_none());
        assert_eq!(
            status
                .last_transaction
                .as_ref()
                .map(|transaction| &transaction.state),
            Some(&ReleaseTransactionState::RolledBack)
        );
    }
}
