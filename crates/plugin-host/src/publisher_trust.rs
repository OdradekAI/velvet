use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use ed25519_dalek::{Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use velvet_persistence::DurabilityMode;

#[cfg(test)]
use super::encode_hex;
use super::{
    decode_hex, decode_signature, decode_verifying_key, PluginPublisherKeyRotation,
    PluginTrustPolicy, PLUGIN_KEY_ROTATION_SCHEMA_VERSION,
};

pub const PLUGIN_PUBLISHER_TRUST_DOCUMENT_SCHEMA_VERSION: u16 = 1;
pub const MAX_PLUGIN_PUBLISHER_TRUST_DOCUMENT_BYTES: u64 = 256 * 1024;
pub const MAX_PLUGIN_PUBLISHER_KEYS: usize = 256;
pub const MAX_PLUGIN_PUBLISHER_ROTATIONS: usize = 256;
pub const MAX_PLUGIN_PUBLISHER_TRUST_LIFETIME_SECONDS: u64 = 90 * 24 * 60 * 60;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginPublisherTrustError {
    Unavailable(String),
    Corrupt(String),
    Invalid(String),
    Untrusted(String),
    ClockSkew { issued_at: u64, now: u64 },
    Expired { expires_at: u64, now: u64 },
    Rollback { accepted: u64, candidate: u64 },
}

/// A root-signed publisher key set. The root key is pinned by native release
/// composition; this document can only add publisher keys through a verified
/// root signature and the existing dual-signed rotation contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginPublisherTrustDocument {
    pub schema_version: u16,
    pub root_key_id: String,
    pub sequence: u64,
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub trusted_publishers: BTreeMap<String, String>,
    pub rotations: Vec<PluginPublisherKeyRotation>,
    pub signature: String,
}

impl PluginPublisherTrustDocument {
    /// Returns the canonical bytes that the pinned publisher root signs.
    pub fn canonical_payload(&self) -> Vec<u8> {
        canonical_plugin_publisher_trust_payload(self)
    }

    /// Verifies the document and turns it into the host-owned package policy.
    pub fn verify(
        &self,
        expected_root_key_id: &str,
        root_key: &VerifyingKey,
        now_unix_seconds: u64,
    ) -> Result<PluginTrustPolicy, PluginPublisherTrustError> {
        validate_document(self)?;
        if self.root_key_id != expected_root_key_id {
            return Err(PluginPublisherTrustError::Untrusted(format!(
                "publisher trust document is for root {}, expected {}",
                self.root_key_id, expected_root_key_id
            )));
        }
        if now_unix_seconds < self.issued_at_unix_seconds {
            return Err(PluginPublisherTrustError::ClockSkew {
                issued_at: self.issued_at_unix_seconds,
                now: now_unix_seconds,
            });
        }
        if now_unix_seconds >= self.expires_at_unix_seconds {
            return Err(PluginPublisherTrustError::Expired {
                expires_at: self.expires_at_unix_seconds,
                now: now_unix_seconds,
            });
        }
        let signature = decode_signature(&self.signature).map_err(|message| {
            PluginPublisherTrustError::Invalid(format!(
                "publisher trust document signature is invalid: {message}"
            ))
        })?;
        root_key
            .verify(&self.canonical_payload(), &signature)
            .map_err(|_| {
                PluginPublisherTrustError::Untrusted(
                    "publisher trust document is not signed by the pinned root".into(),
                )
            })?;

        let mut policy = PluginTrustPolicy::require_signed();
        for (key_id, public_key) in &self.trusted_publishers {
            policy = policy.trust_publisher(key_id.clone(), public_key.clone());
        }
        let mut rotations = self.rotations.clone();
        rotations.sort_by(|left, right| {
            left.sequence
                .cmp(&right.sequence)
                .then_with(|| left.new_key_id.cmp(&right.new_key_id))
        });
        for rotation in rotations {
            policy
                .register_publisher_key_rotation(rotation)
                .map_err(|error| {
                    PluginPublisherTrustError::Invalid(format!(
                        "publisher trust rotation is invalid: {error:?}"
                    ))
                })?;
        }
        Ok(policy)
    }
}

pub trait PluginPublisherTrustStore: Send + Sync {
    fn load_verified(
        &self,
        root_key_id: &str,
        root_key: &VerifyingKey,
        now_unix_seconds: u64,
    ) -> Result<Option<PluginTrustPolicy>, PluginPublisherTrustError>;
    fn install_verified(
        &self,
        document: &PluginPublisherTrustDocument,
        root_key_id: &str,
        root_key: &VerifyingKey,
        now_unix_seconds: u64,
    ) -> Result<PluginTrustPolicy, PluginPublisherTrustError>;
}

#[derive(Default)]
pub struct InMemoryPluginPublisherTrustStore {
    state: Mutex<Option<PluginPublisherTrustStoreState>>,
}

impl InMemoryPluginPublisherTrustStore {
    fn load_document(
        &self,
    ) -> Result<Option<PluginPublisherTrustDocument>, PluginPublisherTrustError> {
        self.state
            .lock()
            .map_err(|_| PluginPublisherTrustError::Unavailable("trust store lock poisoned".into()))
            .map(|state| state.as_ref().map(|state| state.document.clone()))
    }

    fn install_document(
        &self,
        document: &PluginPublisherTrustDocument,
    ) -> Result<(), PluginPublisherTrustError> {
        validate_document(document)?;
        let mut state = self.state.lock().map_err(|_| {
            PluginPublisherTrustError::Unavailable("trust store lock poisoned".into())
        })?;
        check_sequence(state.as_ref(), document)?;
        *state = Some(PluginPublisherTrustStoreState {
            schema_version: PLUGIN_PUBLISHER_TRUST_DOCUMENT_SCHEMA_VERSION,
            accepted_sequence: document.sequence,
            document: document.clone(),
        });
        Ok(())
    }
}

impl PluginPublisherTrustStore for InMemoryPluginPublisherTrustStore {
    fn load_verified(
        &self,
        root_key_id: &str,
        root_key: &VerifyingKey,
        now_unix_seconds: u64,
    ) -> Result<Option<PluginTrustPolicy>, PluginPublisherTrustError> {
        self.load_document()?
            .map(|document| document.verify(root_key_id, root_key, now_unix_seconds))
            .transpose()
    }

    fn install_verified(
        &self,
        document: &PluginPublisherTrustDocument,
        root_key_id: &str,
        root_key: &VerifyingKey,
        now_unix_seconds: u64,
    ) -> Result<PluginTrustPolicy, PluginPublisherTrustError> {
        let policy = document.verify(root_key_id, root_key, now_unix_seconds)?;
        self.install_document(document)?;
        Ok(policy)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFilePluginPublisherTrustStore {
    path: PathBuf,
    durability: DurabilityMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PluginPublisherTrustStoreState {
    schema_version: u16,
    accepted_sequence: u64,
    document: PluginPublisherTrustDocument,
}

impl JsonFilePluginPublisherTrustStore {
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

    fn load_document(
        &self,
    ) -> Result<Option<PluginPublisherTrustDocument>, PluginPublisherTrustError> {
        Ok(self.read_state()?.map(|state| state.document))
    }

    fn install_document(
        &self,
        document: &PluginPublisherTrustDocument,
    ) -> Result<(), PluginPublisherTrustError> {
        validate_document(document)?;
        let current = self.read_state()?;
        check_sequence(current.as_ref(), document)?;
        self.write_state(&PluginPublisherTrustStoreState {
            schema_version: PLUGIN_PUBLISHER_TRUST_DOCUMENT_SCHEMA_VERSION,
            accepted_sequence: document.sequence,
            document: document.clone(),
        })
    }

    fn read_state(
        &self,
    ) -> Result<Option<PluginPublisherTrustStoreState>, PluginPublisherTrustError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(PluginPublisherTrustError::Unavailable(error.to_string())),
        };
        if metadata.len() > MAX_PLUGIN_PUBLISHER_TRUST_DOCUMENT_BYTES {
            return Err(PluginPublisherTrustError::Corrupt(
                "publisher trust store exceeds its size limit".into(),
            ));
        }
        let bytes = fs::read(&self.path)
            .map_err(|error| PluginPublisherTrustError::Unavailable(error.to_string()))?;
        let state = serde_json::from_slice::<PluginPublisherTrustStoreState>(&bytes)
            .map_err(|error| PluginPublisherTrustError::Corrupt(error.to_string()))?;
        validate_store_state(&state)?;
        Ok(Some(state))
    }

    fn write_state(
        &self,
        state: &PluginPublisherTrustStoreState,
    ) -> Result<(), PluginPublisherTrustError> {
        validate_store_state(state)?;
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|error| PluginPublisherTrustError::Corrupt(error.to_string()))?;
        if bytes.len() as u64 > MAX_PLUGIN_PUBLISHER_TRUST_DOCUMENT_BYTES {
            return Err(PluginPublisherTrustError::Invalid(
                "publisher trust store exceeds its size limit".into(),
            ));
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| PluginPublisherTrustError::Unavailable(error.to_string()))?;
        }
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin-publisher-trust.json");
        static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = self.path.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        fs::write(&temporary, bytes)
            .map_err(|error| PluginPublisherTrustError::Unavailable(error.to_string()))?;
        if self.durability == DurabilityMode::RequireFsync {
            File::open(&temporary)
                .and_then(|file| file.sync_all())
                .map_err(|error| PluginPublisherTrustError::Unavailable(error.to_string()))?;
        }
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(PluginPublisherTrustError::Unavailable(error.to_string()));
        }
        if self.durability == DurabilityMode::RequireFsync {
            let parent = self.path.parent().ok_or_else(|| {
                PluginPublisherTrustError::Unavailable("trust path has no parent".into())
            })?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| PluginPublisherTrustError::Unavailable(error.to_string()))?;
        }
        Ok(())
    }
}

impl PluginPublisherTrustStore for JsonFilePluginPublisherTrustStore {
    fn load_verified(
        &self,
        root_key_id: &str,
        root_key: &VerifyingKey,
        now_unix_seconds: u64,
    ) -> Result<Option<PluginTrustPolicy>, PluginPublisherTrustError> {
        self.load_document()?
            .map(|document| document.verify(root_key_id, root_key, now_unix_seconds))
            .transpose()
    }

    fn install_verified(
        &self,
        document: &PluginPublisherTrustDocument,
        root_key_id: &str,
        root_key: &VerifyingKey,
        now_unix_seconds: u64,
    ) -> Result<PluginTrustPolicy, PluginPublisherTrustError> {
        let policy = document.verify(root_key_id, root_key, now_unix_seconds)?;
        self.install_document(document)?;
        Ok(policy)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginPublisherTrustPayload<'a> {
    schema_version: u16,
    root_key_id: &'a str,
    sequence: u64,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    trusted_publishers: &'a BTreeMap<String, String>,
    rotations: &'a [PluginPublisherKeyRotation],
}

fn canonical_plugin_publisher_trust_payload(document: &PluginPublisherTrustDocument) -> Vec<u8> {
    serde_json::to_vec(&PluginPublisherTrustPayload {
        schema_version: document.schema_version,
        root_key_id: &document.root_key_id,
        sequence: document.sequence,
        issued_at_unix_seconds: document.issued_at_unix_seconds,
        expires_at_unix_seconds: document.expires_at_unix_seconds,
        trusted_publishers: &document.trusted_publishers,
        rotations: &document.rotations,
    })
    .expect("publisher trust payload should serialize")
}

fn validate_store_state(
    state: &PluginPublisherTrustStoreState,
) -> Result<(), PluginPublisherTrustError> {
    if state.schema_version != PLUGIN_PUBLISHER_TRUST_DOCUMENT_SCHEMA_VERSION {
        return Err(PluginPublisherTrustError::Corrupt(format!(
            "unsupported publisher trust store schema {}",
            state.schema_version
        )));
    }
    validate_document(&state.document)?;
    if state.accepted_sequence != state.document.sequence {
        return Err(PluginPublisherTrustError::Corrupt(
            "accepted publisher trust sequence does not match the document".into(),
        ));
    }
    Ok(())
}

fn check_sequence(
    current: Option<&PluginPublisherTrustStoreState>,
    candidate: &PluginPublisherTrustDocument,
) -> Result<(), PluginPublisherTrustError> {
    let Some(current) = current else {
        return Ok(());
    };
    if candidate.sequence < current.accepted_sequence {
        return Err(PluginPublisherTrustError::Rollback {
            accepted: current.accepted_sequence,
            candidate: candidate.sequence,
        });
    }
    if candidate.sequence == current.accepted_sequence && candidate != &current.document {
        return Err(PluginPublisherTrustError::Invalid(
            "publisher trust sequence is already occupied by a different document".into(),
        ));
    }
    Ok(())
}

fn validate_document(
    document: &PluginPublisherTrustDocument,
) -> Result<(), PluginPublisherTrustError> {
    let bytes = serde_json::to_vec(document)
        .map_err(|error| PluginPublisherTrustError::Invalid(error.to_string()))?;
    if bytes.len() as u64 > MAX_PLUGIN_PUBLISHER_TRUST_DOCUMENT_BYTES {
        return Err(PluginPublisherTrustError::Invalid(
            "publisher trust document exceeds its size limit".into(),
        ));
    }
    if document.schema_version != PLUGIN_PUBLISHER_TRUST_DOCUMENT_SCHEMA_VERSION {
        return Err(PluginPublisherTrustError::Invalid(format!(
            "unsupported publisher trust document schema {}",
            document.schema_version
        )));
    }
    validate_key_id(&document.root_key_id)?;
    if document.sequence == 0 {
        return Err(PluginPublisherTrustError::Invalid(
            "publisher trust sequence must be positive".into(),
        ));
    }
    if document.expires_at_unix_seconds <= document.issued_at_unix_seconds {
        return Err(PluginPublisherTrustError::Invalid(
            "publisher trust expiry must be after issuance".into(),
        ));
    }
    if document.expires_at_unix_seconds - document.issued_at_unix_seconds
        > MAX_PLUGIN_PUBLISHER_TRUST_LIFETIME_SECONDS
    {
        return Err(PluginPublisherTrustError::Invalid(
            "publisher trust document lifetime exceeds its limit".into(),
        ));
    }
    if document.trusted_publishers.len() > MAX_PLUGIN_PUBLISHER_KEYS {
        return Err(PluginPublisherTrustError::Invalid(
            "publisher trust key limit has been exceeded".into(),
        ));
    }
    if document.rotations.len() > MAX_PLUGIN_PUBLISHER_ROTATIONS {
        return Err(PluginPublisherTrustError::Invalid(
            "publisher trust rotation limit has been exceeded".into(),
        ));
    }
    for (key_id, public_key) in &document.trusted_publishers {
        validate_key_id(key_id)?;
        decode_verifying_key(public_key).map_err(|message| {
            PluginPublisherTrustError::Invalid(format!(
                "trusted publisher key {key_id} is invalid: {message}"
            ))
        })?;
    }
    for rotation in &document.rotations {
        if rotation.schema_version != PLUGIN_KEY_ROTATION_SCHEMA_VERSION {
            return Err(PluginPublisherTrustError::Invalid(format!(
                "publisher trust rotation uses unsupported schema {}",
                rotation.schema_version
            )));
        }
        validate_key_id(&rotation.old_key_id)?;
        validate_key_id(&rotation.new_key_id)?;
        decode_verifying_key(&rotation.old_public_key).map_err(|message| {
            PluginPublisherTrustError::Invalid(format!(
                "rotation old publisher key is invalid: {message}"
            ))
        })?;
        decode_verifying_key(&rotation.new_public_key).map_err(|message| {
            PluginPublisherTrustError::Invalid(format!(
                "rotation new publisher key is invalid: {message}"
            ))
        })?;
        decode_signature(&rotation.old_signature).map_err(|message| {
            PluginPublisherTrustError::Invalid(format!(
                "rotation old signature is invalid: {message}"
            ))
        })?;
        decode_signature(&rotation.new_signature).map_err(|message| {
            PluginPublisherTrustError::Invalid(format!(
                "rotation new signature is invalid: {message}"
            ))
        })?;
    }
    decode_hex(&document.signature, 64).map_err(|message| {
        PluginPublisherTrustError::Invalid(format!(
            "publisher trust document signature is invalid: {message}"
        ))
    })?;
    Ok(())
}

fn validate_key_id(value: &str) -> Result<(), PluginPublisherTrustError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(PluginPublisherTrustError::Invalid(
            "publisher key id contains unsupported characters".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn root_key() -> SigningKey {
        SigningKey::from_bytes(&[0x42; 32])
    }

    fn signed_document(
        root: &SigningKey,
        sequence: u64,
        issued_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
    ) -> PluginPublisherTrustDocument {
        let mut document = PluginPublisherTrustDocument {
            schema_version: PLUGIN_PUBLISHER_TRUST_DOCUMENT_SCHEMA_VERSION,
            root_key_id: "publisher-root".into(),
            sequence,
            issued_at_unix_seconds,
            expires_at_unix_seconds,
            trusted_publishers: BTreeMap::new(),
            rotations: Vec::new(),
            signature: String::new(),
        };
        document.signature = encode_hex(&root.sign(&document.canonical_payload()).to_bytes());
        document
    }

    fn signed_rotation(
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

    #[test]
    fn root_signed_documents_load_offline_and_apply_signed_rotations() {
        let root = root_key();
        let old_key = SigningKey::from_bytes(&[7u8; 32]);
        let new_key = SigningKey::from_bytes(&[8u8; 32]);
        let mut document = signed_document(&root, 1, 10, 100);
        document.trusted_publishers.insert(
            "publisher.current".into(),
            encode_hex(old_key.verifying_key().as_bytes()),
        );
        document.rotations.push(signed_rotation(
            "publisher.current",
            &old_key,
            "publisher.next",
            &new_key,
            1,
        ));
        document.signature = encode_hex(&root.sign(&document.canonical_payload()).to_bytes());

        let store = InMemoryPluginPublisherTrustStore::default();
        let policy = store
            .install_verified(&document, "publisher-root", &root.verifying_key(), 20)
            .expect("the root-signed document should install");
        assert!(!policy.is_developer_mode());
        assert!(policy.trusts_publisher_key(
            "publisher.current",
            &encode_hex(old_key.verifying_key().as_bytes())
        ));
        assert!(policy.trusts_publisher_key(
            "publisher.next",
            &encode_hex(new_key.verifying_key().as_bytes())
        ));
        assert!(store
            .load_verified("publisher-root", &root.verifying_key(), 90)
            .expect("the persisted document should be usable offline")
            .is_some());
        assert!(matches!(
            document.verify("other-root", &root.verifying_key(), 20),
            Err(PluginPublisherTrustError::Untrusted(_))
        ));
        let mut unsupported = document.clone();
        unsupported.schema_version += 1;
        assert!(matches!(
            unsupported.verify("publisher-root", &root.verifying_key(), 20),
            Err(PluginPublisherTrustError::Invalid(message))
                if message.contains("unsupported publisher trust document schema")
        ));

        let mut forged = document.clone();
        forged.trusted_publishers.insert(
            "publisher.forged".into(),
            encode_hex(
                SigningKey::from_bytes(&[9u8; 32])
                    .verifying_key()
                    .as_bytes(),
            ),
        );
        assert!(matches!(
            forged.verify("publisher-root", &root.verifying_key(), 20),
            Err(PluginPublisherTrustError::Untrusted(_))
        ));
    }

    #[test]
    fn publisher_trust_store_rejects_expiry_clock_skew_and_sequence_rollback() {
        let root = root_key();
        let store = InMemoryPluginPublisherTrustStore::default();
        let expiring = signed_document(&root, 1, 10, 20);
        assert!(matches!(
            expiring.verify("publisher-root", &root.verifying_key(), 9),
            Err(PluginPublisherTrustError::ClockSkew { .. })
        ));
        assert!(matches!(
            expiring.verify("publisher-root", &root.verifying_key(), 20),
            Err(PluginPublisherTrustError::Expired { .. })
        ));
        let document = signed_document(&root, 1, 10, 100);
        store
            .install_verified(&document, "publisher-root", &root.verifying_key(), 10)
            .expect("the first trust document should install");

        let mut conflicting = document.clone();
        conflicting.root_key_id = "publisher-root-alt".into();
        conflicting.signature = encode_hex(&root.sign(&conflicting.canonical_payload()).to_bytes());
        assert!(matches!(
            store.install_verified(&conflicting, "publisher-root-alt", &root.verifying_key(), 10),
            Err(PluginPublisherTrustError::Invalid(message))
                if message.contains("sequence is already occupied")
        ));

        let mut newer = signed_document(&root, 2, 20, 100);
        newer.signature = encode_hex(&root.sign(&newer.canonical_payload()).to_bytes());
        store
            .install_verified(&newer, "publisher-root", &root.verifying_key(), 20)
            .expect("the newer sequence should install");
        assert!(matches!(
            store.install_verified(&document, "publisher-root", &root.verifying_key(), 20),
            Err(PluginPublisherTrustError::Rollback {
                accepted: 2,
                candidate: 1
            })
        ));
        assert!(store
            .load_verified("publisher-root", &root.verifying_key(), 50)
            .expect("rollback must preserve the newer document")
            .is_some());
    }

    #[test]
    fn json_publisher_trust_store_is_atomic_and_fails_closed_on_corruption() {
        let root = root_key();
        let path = std::env::temp_dir().join(format!(
            "velvet-publisher-trust-{}-{}.json",
            std::process::id(),
            17
        ));
        let store = JsonFilePluginPublisherTrustStore::new(&path);
        let document = signed_document(&root, 1, 10, 100);
        store
            .install_verified(&document, "publisher-root", &root.verifying_key(), 20)
            .expect("the JSON trust document should install");
        let reopened = JsonFilePluginPublisherTrustStore::new(&path);
        assert!(reopened
            .load_verified("publisher-root", &root.verifying_key(), 90)
            .expect("the JSON trust document should survive restart")
            .is_some());

        std::fs::write(&path, b"{\"schemaVersion\":999}")
            .expect("the corrupt trust document should be writable");
        assert!(matches!(
            reopened.load_verified("publisher-root", &root.verifying_key(), 20),
            Err(PluginPublisherTrustError::Corrupt(_))
        ));
        let _ = std::fs::remove_file(path);
    }
}
