use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use velvet_workbench_protocol::{
    ReleaseStatusDto, ReleaseTransactionState, RELEASE_TRANSACTION_SCHEMA_VERSION,
};

use crate::durability::{write_atomic, DurabilityMode};

/// Storage adapter for the application-owned release transaction journal.
///
/// The adapter persists only the signed manifest and transaction metadata. It
/// does not decide whether a release is trusted or when a health checkpoint is
/// sufficient to commit; those decisions stay in `ApplicationCore`.
pub trait ReleaseUpdateStore: Send + Sync {
    fn load(&self) -> Result<ReleaseStatusDto, ReleaseStoreError>;
    fn save(&self, status: &ReleaseStatusDto) -> Result<(), ReleaseStoreError>;
}

/// Native package staging seam. The updater owns the source transport; this
/// adapter owns bounded copy, digest verification and generation publication.
pub trait ReleasePackageStore: Send + Sync {
    fn stage_package(
        &self,
        transaction_id: &str,
        source: &Path,
        expected_sha256: &str,
    ) -> Result<String, ReleaseStoreError>;
    fn is_staged(&self, staged_package_id: &str) -> Result<bool, ReleaseStoreError>;
    fn commit_package(
        &self,
        staged_package_id: &str,
        package_generation: &str,
    ) -> Result<(), ReleaseStoreError>;
    fn rollback_package(&self, staged_package_id: &str) -> Result<(), ReleaseStoreError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseStoreError {
    Unavailable(String),
    Corrupt(String),
}

pub struct InMemoryReleaseUpdateStore {
    status: Mutex<ReleaseStatusDto>,
}

impl Default for InMemoryReleaseUpdateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryReleaseUpdateStore {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(default_release_status()),
        }
    }
}

impl ReleaseUpdateStore for InMemoryReleaseUpdateStore {
    fn load(&self) -> Result<ReleaseStatusDto, ReleaseStoreError> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| ReleaseStoreError::Unavailable("release store lock poisoned".into()))
    }

    fn save(&self, status: &ReleaseStatusDto) -> Result<(), ReleaseStoreError> {
        validate_status(status)?;
        self.status
            .lock()
            .map(|mut current| *current = status.clone())
            .map_err(|_| ReleaseStoreError::Unavailable("release store lock poisoned".into()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileReleaseUpdateStore {
    path: PathBuf,
    durability: DurabilityMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileReleasePackageStore {
    root: PathBuf,
    durability: DurabilityMode,
}

impl JsonFileReleasePackageStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_durability(root, DurabilityMode::AtomicRenameOnly)
    }

    pub fn with_durability(root: impl Into<PathBuf>, durability: DurabilityMode) -> Self {
        Self {
            root: root.into(),
            durability,
        }
    }

    fn staging_path(&self, transaction_id: &str) -> PathBuf {
        self.root
            .join("staging")
            .join(transaction_id)
            .join("package")
    }

    fn package_path(&self, generation: &str) -> PathBuf {
        self.root.join("packages").join(generation).join("package")
    }
}

impl ReleasePackageStore for JsonFileReleasePackageStore {
    fn stage_package(
        &self,
        transaction_id: &str,
        source: &Path,
        expected_sha256: &str,
    ) -> Result<String, ReleaseStoreError> {
        validate_token(transaction_id, "transaction id")?;
        if expected_sha256.len() != 64
            || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ReleaseStoreError::Corrupt(
                "package digest must be a 64 character hexadecimal string".into(),
            ));
        }
        let metadata = fs::symlink_metadata(source)
            .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        if !metadata.file_type().is_file() {
            return Err(ReleaseStoreError::Corrupt(
                "release package source must be a regular file".into(),
            ));
        }
        let destination = self.staging_path(transaction_id);
        let parent = destination
            .parent()
            .ok_or_else(|| ReleaseStoreError::Unavailable("staging path has no parent".into()))?;
        fs::create_dir_all(parent)
            .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        let temporary = destination.with_extension("tmp");
        let mut input = File::open(source)
            .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        let mut output = File::create(&temporary)
            .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = input
                .read(&mut buffer)
                .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
            std::io::Write::write_all(&mut output, &buffer[..read])
                .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        }
        if encode_hex(&digest.finalize()) != expected_sha256.to_ascii_lowercase() {
            let _ = fs::remove_file(&temporary);
            return Err(ReleaseStoreError::Corrupt(
                "package digest does not match the release manifest".into(),
            ));
        }
        if self.durability == DurabilityMode::RequireFsync {
            output
                .sync_all()
                .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        }
        drop(output);
        fs::rename(&temporary, &destination)
            .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        if self.durability == DurabilityMode::RequireFsync {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        }
        Ok(transaction_id.into())
    }

    fn is_staged(&self, staged_package_id: &str) -> Result<bool, ReleaseStoreError> {
        validate_token(staged_package_id, "staged package id")?;
        Ok(self.staging_path(staged_package_id).is_file())
    }

    fn commit_package(
        &self,
        staged_package_id: &str,
        package_generation: &str,
    ) -> Result<(), ReleaseStoreError> {
        validate_token(staged_package_id, "staged package id")?;
        validate_token(package_generation, "package generation")?;
        let source = self.staging_path(staged_package_id);
        if !source.is_file() {
            return Err(ReleaseStoreError::Unavailable(
                "staged package is missing".into(),
            ));
        }
        let destination = self.package_path(package_generation);
        let parent = destination
            .parent()
            .ok_or_else(|| ReleaseStoreError::Unavailable("package path has no parent".into()))?;
        fs::create_dir_all(parent)
            .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        fs::rename(&source, &destination)
            .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        if self.durability == DurabilityMode::RequireFsync {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        }
        Ok(())
    }

    fn rollback_package(&self, staged_package_id: &str) -> Result<(), ReleaseStoreError> {
        validate_token(staged_package_id, "staged package id")?;
        let staging_path = self.staging_path(staged_package_id);
        let staging = staging_path
            .parent()
            .ok_or_else(|| ReleaseStoreError::Unavailable("staging path has no parent".into()))?;
        match fs::remove_dir_all(staging) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ReleaseStoreError::Unavailable(error.to_string())),
        }
    }
}

impl JsonFileReleaseUpdateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_durability(path, DurabilityMode::AtomicRenameOnly)
    }

    pub fn with_durability(path: impl Into<PathBuf>, durability: DurabilityMode) -> Self {
        Self {
            path: path.into(),
            durability,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ReleaseUpdateStore for JsonFileReleaseUpdateStore {
    fn load(&self) -> Result<ReleaseStatusDto, ReleaseStoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(default_release_status())
            }
            Err(error) => return Err(ReleaseStoreError::Unavailable(error.to_string())),
        };
        let status = serde_json::from_slice::<ReleaseStatusDto>(&bytes)
            .map_err(|error| ReleaseStoreError::Corrupt(error.to_string()))?;
        validate_status(&status)?;
        Ok(status)
    }

    fn save(&self, status: &ReleaseStatusDto) -> Result<(), ReleaseStoreError> {
        validate_status(status)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))?;
        }
        let bytes = serde_json::to_vec_pretty(status)
            .map_err(|error| ReleaseStoreError::Corrupt(error.to_string()))?;
        write_atomic(&self.path, &bytes, self.durability)
            .map_err(|error| ReleaseStoreError::Unavailable(error.to_string()))
    }
}

pub fn default_release_status() -> ReleaseStatusDto {
    ReleaseStatusDto {
        schema_version: RELEASE_TRANSACTION_SCHEMA_VERSION,
        committed_sequence: 0,
        committed_package_generation: "bootstrap".into(),
        pending: None,
        last_transaction: None,
    }
}

fn validate_status(status: &ReleaseStatusDto) -> Result<(), ReleaseStoreError> {
    if status.schema_version != RELEASE_TRANSACTION_SCHEMA_VERSION {
        return Err(ReleaseStoreError::Corrupt(format!(
            "unsupported release transaction schema {}",
            status.schema_version
        )));
    }
    if status.committed_package_generation.is_empty() {
        return Err(ReleaseStoreError::Corrupt(
            "committed package generation is empty".into(),
        ));
    }
    for transaction in status.pending.iter().chain(status.last_transaction.iter()) {
        if transaction.schema_version != RELEASE_TRANSACTION_SCHEMA_VERSION
            || transaction.transaction_id.is_empty()
            || transaction.staged_package_id.is_empty()
            || transaction.package_generation.is_empty()
        {
            return Err(ReleaseStoreError::Corrupt(
                "release transaction has invalid identity fields".into(),
            ));
        }
    }
    if status
        .pending
        .as_ref()
        .is_some_and(|transaction| transaction.state != ReleaseTransactionState::Staged)
    {
        return Err(ReleaseStoreError::Corrupt(
            "pending release transaction is not staged".into(),
        ));
    }
    if status
        .last_transaction
        .as_ref()
        .is_some_and(|transaction| transaction.state == ReleaseTransactionState::Staged)
    {
        return Err(ReleaseStoreError::Corrupt(
            "last release transaction is still staged".into(),
        ));
    }
    Ok(())
}

fn validate_token(value: &str, field: &str) -> Result<(), ReleaseStoreError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ReleaseStoreError::Corrupt(format!(
            "{field} must be a bounded opaque token"
        )));
    }
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_workbench_protocol::{
        ReleaseChannel, ReleaseManifestDto, ReleaseTransactionDto, ReleaseTransactionState,
    };

    #[test]
    fn in_memory_store_starts_at_bootstrap_generation() {
        let store = InMemoryReleaseUpdateStore::new();
        let status = store.load().expect("status");
        assert_eq!(status.committed_sequence, 0);
        assert_eq!(status.committed_package_generation, "bootstrap");
        assert!(status.pending.is_none());
    }

    #[test]
    fn file_store_rejects_unknown_schema() {
        let root =
            std::env::temp_dir().join(format!("velvet-release-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        let path = root.join("release.json");
        fs::write(
            &path,
            serde_json::json!({
                "schemaVersion": 99,
                "committedSequence": 0,
                "committedPackageGeneration": "bootstrap",
                "pending": null,
                "lastTransaction": null
            })
            .to_string(),
        )
        .expect("write");
        let error = JsonFileReleaseUpdateStore::new(&path)
            .load()
            .expect_err("unknown schemas must fail closed");
        assert!(matches!(error, ReleaseStoreError::Corrupt(_)));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_store_round_trips_pending_transaction() {
        let root = std::env::temp_dir().join(format!(
            "velvet-release-store-roundtrip-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let path = root.join("nested/release.json");
        let store = JsonFileReleaseUpdateStore::new(&path);
        let mut status = default_release_status();
        status.pending = Some(ReleaseTransactionDto {
            schema_version: RELEASE_TRANSACTION_SCHEMA_VERSION,
            transaction_id: "txn-1".into(),
            manifest: ReleaseManifestDto {
                schema_version: 1,
                channel: ReleaseChannel::Preview,
                version: "0.0.5".into(),
                target: "test".into(),
                artifact_sha256: "0".repeat(64),
                updater_schema: 1,
                min_data_schema: 1,
                max_data_schema: 1,
                min_client_version: "0.0.1".into(),
                release_sequence: 1,
                issued_at_ms: 1,
                expires_at_ms: 2,
                signing_key_id: "test".into(),
                signature: "0".repeat(128),
            },
            staged_package_id: "stage-1".into(),
            package_generation: "pkg-1".into(),
            base_data_generation: Some(1),
            committed_data_generation: None,
            state: ReleaseTransactionState::Staged,
            health_checkpoint: None,
            failure_reason: None,
            created_at_ms: 1,
        });
        store.save(&status).expect("save");
        assert_eq!(store.load().expect("load"), status);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn package_store_verifies_digest_before_publication() {
        let root = std::env::temp_dir().join(format!(
            "velvet-release-package-store-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        let source = root.join("source.bin");
        fs::write(&source, b"package-bytes").expect("source");
        let digest = encode_hex(&Sha256::digest(b"package-bytes"));
        let store = JsonFileReleasePackageStore::new(root.join("store"));
        let id = store
            .stage_package("txn-1", &source, &digest)
            .expect("stage package");
        assert!(store.is_staged(&id).expect("staged"));
        store.commit_package(&id, "pkg-1").expect("commit package");
        assert!(!store.is_staged(&id).expect("staging removed"));
        let _ = fs::remove_dir_all(root);
    }
}
