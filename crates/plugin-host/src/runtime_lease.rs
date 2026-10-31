use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use velvet_persistence::DurabilityMode;
use velvet_plugin_api::ExtensionId;

pub const RUNTIME_LEASE_SCHEMA_VERSION: u16 = 1;
pub const MAX_RUNTIME_LEASES: usize = 128;
pub const MAX_RUNTIME_LEASE_STORE_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeLeaseState {
    Starting,
    Running,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProcessIdentity {
    pub pid: u32,
    #[serde(default)]
    pub process_group_id: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeLease {
    pub schema_version: u16,
    pub extension_id: ExtensionId,
    pub package_version: String,
    #[serde(default)]
    pub package_root: Option<PathBuf>,
    pub runtime_nonce: String,
    pub state: RuntimeLeaseState,
    #[serde(default)]
    pub process: Option<RuntimeProcessIdentity>,
    pub started_at_ms: u64,
}

impl RuntimeLease {
    pub fn starting(
        extension_id: ExtensionId,
        package_version: String,
        package_root: Option<PathBuf>,
        runtime_nonce: String,
    ) -> Self {
        Self {
            schema_version: RUNTIME_LEASE_SCHEMA_VERSION,
            extension_id,
            package_version,
            package_root,
            runtime_nonce,
            state: RuntimeLeaseState::Starting,
            process: None,
            started_at_ms: runtime_timestamp_ms(),
        }
    }

    pub fn running(mut self, process: RuntimeProcessIdentity) -> Self {
        self.state = RuntimeLeaseState::Running;
        self.process = Some(process);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeLeaseError {
    Unavailable(String),
    Corrupt(String),
    Invalid(String),
}

pub trait RuntimeLeaseStore: Send + Sync {
    fn load(&self) -> Result<Vec<RuntimeLease>, RuntimeLeaseError>;
    fn publish(&self, lease: RuntimeLease) -> Result<(), RuntimeLeaseError>;
    fn close(
        &self,
        extension_id: &ExtensionId,
        runtime_nonce: &str,
    ) -> Result<(), RuntimeLeaseError>;
}

#[derive(Default)]
pub struct InMemoryRuntimeLeaseStore {
    leases: Mutex<BTreeMap<(ExtensionId, String), RuntimeLease>>,
}

impl InMemoryRuntimeLeaseStore {
    pub fn leases(&self) -> Result<Vec<RuntimeLease>, RuntimeLeaseError> {
        self.load()
    }
}

impl RuntimeLeaseStore for InMemoryRuntimeLeaseStore {
    fn load(&self) -> Result<Vec<RuntimeLease>, RuntimeLeaseError> {
        self.leases
            .lock()
            .map(|leases| leases.values().cloned().collect())
            .map_err(|_| RuntimeLeaseError::Unavailable("lease store lock poisoned".into()))
    }

    fn publish(&self, lease: RuntimeLease) -> Result<(), RuntimeLeaseError> {
        validate_lease(&lease)?;
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| RuntimeLeaseError::Unavailable("lease store lock poisoned".into()))?;
        let key = (lease.extension_id.clone(), lease.runtime_nonce.clone());
        if !leases.contains_key(&key) && leases.len() >= MAX_RUNTIME_LEASES {
            return Err(RuntimeLeaseError::Invalid(
                "runtime lease limit has been reached".into(),
            ));
        }
        leases.insert(key, lease);
        Ok(())
    }

    fn close(
        &self,
        extension_id: &ExtensionId,
        runtime_nonce: &str,
    ) -> Result<(), RuntimeLeaseError> {
        self.leases
            .lock()
            .map(|mut leases| {
                leases.remove(&(extension_id.clone(), runtime_nonce.to_owned()));
            })
            .map_err(|_| RuntimeLeaseError::Unavailable("lease store lock poisoned".into()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileRuntimeLeaseStore {
    path: PathBuf,
    durability: DurabilityMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeLeaseStoreState {
    schema_version: u16,
    leases: Vec<RuntimeLease>,
}

impl JsonFileRuntimeLeaseStore {
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

    fn read_state(&self) -> Result<RuntimeLeaseStoreState, RuntimeLeaseError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(RuntimeLeaseStoreState {
                    schema_version: RUNTIME_LEASE_SCHEMA_VERSION,
                    leases: Vec::new(),
                })
            }
            Err(error) => return Err(RuntimeLeaseError::Unavailable(error.to_string())),
        };
        if metadata.len() > MAX_RUNTIME_LEASE_STORE_BYTES {
            return Err(RuntimeLeaseError::Corrupt(
                "runtime lease store exceeds its size limit".into(),
            ));
        }
        let bytes = fs::read(&self.path)
            .map_err(|error| RuntimeLeaseError::Unavailable(error.to_string()))?;
        let state = serde_json::from_slice::<RuntimeLeaseStoreState>(&bytes)
            .map_err(|error| RuntimeLeaseError::Corrupt(error.to_string()))?;
        validate_store_state(&state)?;
        Ok(state)
    }

    fn write_state(&self, state: &RuntimeLeaseStoreState) -> Result<(), RuntimeLeaseError> {
        validate_store_state(state)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| RuntimeLeaseError::Unavailable(error.to_string()))?;
        }
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|error| RuntimeLeaseError::Corrupt(error.to_string()))?;
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("runtime-leases.json");
        let temporary = self.path.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            runtime_timestamp_ms()
        ));
        fs::write(&temporary, bytes)
            .map_err(|error| RuntimeLeaseError::Unavailable(error.to_string()))?;
        if self.durability == DurabilityMode::RequireFsync {
            File::open(&temporary)
                .and_then(|file| file.sync_all())
                .map_err(|error| RuntimeLeaseError::Unavailable(error.to_string()))?;
        }
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(RuntimeLeaseError::Unavailable(error.to_string()));
        }
        if self.durability == DurabilityMode::RequireFsync {
            let parent = self
                .path
                .parent()
                .ok_or_else(|| RuntimeLeaseError::Unavailable("lease path has no parent".into()))?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| RuntimeLeaseError::Unavailable(error.to_string()))?;
        }
        Ok(())
    }
}

impl RuntimeLeaseStore for JsonFileRuntimeLeaseStore {
    fn load(&self) -> Result<Vec<RuntimeLease>, RuntimeLeaseError> {
        Ok(self.read_state()?.leases)
    }

    fn publish(&self, lease: RuntimeLease) -> Result<(), RuntimeLeaseError> {
        validate_lease(&lease)?;
        let mut state = self.read_state()?;
        if !state.leases.iter().any(|candidate| {
            candidate.extension_id == lease.extension_id
                && candidate.runtime_nonce == lease.runtime_nonce
        }) && state.leases.len() >= MAX_RUNTIME_LEASES
        {
            return Err(RuntimeLeaseError::Invalid(
                "runtime lease limit has been reached".into(),
            ));
        }
        state.leases.retain(|candidate| {
            candidate.extension_id != lease.extension_id
                || candidate.runtime_nonce != lease.runtime_nonce
        });
        state.leases.push(lease);
        self.write_state(&state)
    }

    fn close(
        &self,
        extension_id: &ExtensionId,
        runtime_nonce: &str,
    ) -> Result<(), RuntimeLeaseError> {
        let mut state = self.read_state()?;
        state.leases.retain(|candidate| {
            &candidate.extension_id != extension_id || candidate.runtime_nonce != runtime_nonce
        });
        self.write_state(&state)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeOrphanObservation {
    Absent,
    Matching,
    DifferentProcess,
    IdentityUncertain,
}

pub trait RuntimeOrphanReconciler: Send + Sync {
    fn observe(&self, lease: &RuntimeLease) -> RuntimeOrphanObservation;
    fn terminate(&self, lease: &RuntimeLease) -> Result<(), RuntimeLeaseError>;
}

#[derive(Default)]
pub struct ConservativeRuntimeOrphanReconciler;

impl RuntimeOrphanReconciler for ConservativeRuntimeOrphanReconciler {
    fn observe(&self, _lease: &RuntimeLease) -> RuntimeOrphanObservation {
        RuntimeOrphanObservation::IdentityUncertain
    }

    fn terminate(&self, _lease: &RuntimeLease) -> Result<(), RuntimeLeaseError> {
        Err(RuntimeLeaseError::Unavailable(
            "no platform runtime termination primitive is configured".into(),
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeOrphanReport {
    pub extension_id: ExtensionId,
    pub runtime_nonce: String,
    pub observation: RuntimeOrphanObservation,
    pub lease_closed: bool,
    pub quarantined: bool,
}

fn validate_store_state(state: &RuntimeLeaseStoreState) -> Result<(), RuntimeLeaseError> {
    if state.schema_version != RUNTIME_LEASE_SCHEMA_VERSION {
        return Err(RuntimeLeaseError::Corrupt(format!(
            "runtime lease store schema {} is unsupported; expected {}",
            state.schema_version, RUNTIME_LEASE_SCHEMA_VERSION
        )));
    }
    if state.leases.len() > MAX_RUNTIME_LEASES {
        return Err(RuntimeLeaseError::Corrupt(
            "runtime lease store contains too many leases".into(),
        ));
    }
    for lease in &state.leases {
        validate_lease(lease)?;
    }
    Ok(())
}

fn validate_lease(lease: &RuntimeLease) -> Result<(), RuntimeLeaseError> {
    if lease.schema_version != RUNTIME_LEASE_SCHEMA_VERSION
        || lease.extension_id.0.trim().is_empty()
        || lease.package_version.trim().is_empty()
        || lease.runtime_nonce.trim().is_empty()
        || matches!(lease.state, RuntimeLeaseState::Running) && lease.process.is_none()
    {
        return Err(RuntimeLeaseError::Invalid(
            "runtime lease fields are invalid".into(),
        ));
    }
    Ok(())
}

fn runtime_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEMP_PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_path() -> PathBuf {
        let sequence = TEMP_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "velvet-runtime-leases-{}-{sequence}.json",
            std::process::id()
        ))
    }

    #[test]
    fn json_store_round_trips_and_closes_a_running_lease() {
        let path = temporary_path();
        let store =
            JsonFileRuntimeLeaseStore::with_durability(&path, DurabilityMode::AtomicRenameOnly);
        let id = ExtensionId("example.plugin".into());
        let lease = RuntimeLease::starting(
            id.clone(),
            "1.2.3".into(),
            Some(PathBuf::from("/plugins/example.plugin")),
            "nonce-1".into(),
        )
        .running(RuntimeProcessIdentity {
            pid: 100,
            process_group_id: Some(100),
        });
        store
            .publish(lease.clone())
            .expect("the lease should be persisted");
        assert_eq!(store.load().expect("the lease should load"), vec![lease]);
        store.close(&id, "nonce-1").expect("the lease should close");
        assert!(store
            .load()
            .expect("the empty store should load")
            .is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn running_lease_without_process_identity_is_rejected() {
        let store = InMemoryRuntimeLeaseStore::default();
        let lease = RuntimeLease {
            schema_version: RUNTIME_LEASE_SCHEMA_VERSION,
            extension_id: ExtensionId("example.plugin".into()),
            package_version: "1.0.0".into(),
            package_root: None,
            runtime_nonce: "nonce-1".into(),
            state: RuntimeLeaseState::Running,
            process: None,
            started_at_ms: 1,
        };
        assert!(matches!(
            store.publish(lease),
            Err(RuntimeLeaseError::Invalid(_))
        ));
    }
}
