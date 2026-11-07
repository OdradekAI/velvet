use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use super::platform_process;
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
    Quarantined,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProcessIdentity {
    pub pid: u32,
    #[serde(default)]
    pub process_group_id: Option<u32>,
    /// Linux `/proc/<pid>/stat` start time in clock ticks, macOS process start
    /// time, or Windows process creation time, depending on the host platform.
    #[serde(default)]
    pub start_token: Option<String>,
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
    #[serde(default)]
    pub host_instance_id: Option<String>,
    #[serde(default)]
    pub launch_attempt_id: Option<u64>,
    pub started_at_ms: u64,
}

/// Safe, user-facing evidence for a quarantined lease. Process identity and
/// package paths stay inside the host so a recovery surface cannot turn this
/// projection into a process-control API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantinedRuntimeLease {
    pub extension_id: ExtensionId,
    pub runtime_nonce: String,
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
            host_instance_id: None,
            launch_attempt_id: None,
            started_at_ms: runtime_timestamp_ms(),
        }
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
    fn quarantine(
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

    fn quarantine(
        &self,
        extension_id: &ExtensionId,
        runtime_nonce: &str,
    ) -> Result<(), RuntimeLeaseError> {
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| RuntimeLeaseError::Unavailable("lease store lock poisoned".into()))?;
        let Some(lease) = leases.get_mut(&(extension_id.clone(), runtime_nonce.to_owned())) else {
            return Err(RuntimeLeaseError::Invalid(
                "runtime lease was not found".into(),
            ));
        };
        lease.state = RuntimeLeaseState::Quarantined;
        Ok(())
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

    fn quarantine(
        &self,
        extension_id: &ExtensionId,
        runtime_nonce: &str,
    ) -> Result<(), RuntimeLeaseError> {
        let mut state = self.read_state()?;
        let Some(lease) = state.leases.iter_mut().find(|candidate| {
            &candidate.extension_id == extension_id && candidate.runtime_nonce == runtime_nonce
        }) else {
            return Err(RuntimeLeaseError::Invalid(
                "runtime lease was not found".into(),
            ));
        };
        lease.state = RuntimeLeaseState::Quarantined;
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

/// Uses the strongest identity primitive available on the current platform.
/// Linux verifies PID reuse with `/proc`; macOS uses `proc_pidinfo`; Windows
/// uses the process creation time and Job Object lifetime adapter.
#[derive(Default)]
pub struct PlatformRuntimeOrphanReconciler;

impl RuntimeOrphanReconciler for PlatformRuntimeOrphanReconciler {
    fn observe(&self, lease: &RuntimeLease) -> RuntimeOrphanObservation {
        let Some(expected) = lease.process.as_ref() else {
            return RuntimeOrphanObservation::IdentityUncertain;
        };
        match inspect_process(expected.pid) {
            None => RuntimeOrphanObservation::Absent,
            Some(actual) if process_identity_matches(expected, &actual) => {
                RuntimeOrphanObservation::Matching
            }
            Some(actual) if actual.start_token.is_none() => {
                RuntimeOrphanObservation::IdentityUncertain
            }
            Some(_) if expected.start_token.is_some() => RuntimeOrphanObservation::DifferentProcess,
            Some(_) => RuntimeOrphanObservation::IdentityUncertain,
        }
    }

    fn terminate(&self, lease: &RuntimeLease) -> Result<(), RuntimeLeaseError> {
        if self.observe(lease) != RuntimeOrphanObservation::Matching {
            return Err(RuntimeLeaseError::Invalid(
                "runtime identity changed before termination".into(),
            ));
        }
        let Some(process) = lease.process.as_ref() else {
            return Err(RuntimeLeaseError::Invalid(
                "runtime lease has no process identity".into(),
            ));
        };
        #[cfg(unix)]
        {
            let group = process.process_group_id.ok_or_else(|| {
                RuntimeLeaseError::Unavailable(
                    "runtime process group is unavailable on this platform".into(),
                )
            })?;
            if group == 0 || group > i32::MAX as u32 {
                return Err(RuntimeLeaseError::Invalid(
                    "runtime process group id is outside the safe kill range".into(),
                ));
            }
            let result = unsafe { libc::kill(-(group as i32), libc::SIGKILL) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::ESRCH) {
                    return Ok(());
                }
                return Err(RuntimeLeaseError::Unavailable(error.to_string()));
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            platform_process::terminate_process(process.pid).map_err(RuntimeLeaseError::Unavailable)
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            let _ = process;
            Err(RuntimeLeaseError::Unavailable(
                "no platform runtime termination primitive is configured".into(),
            ))
        }
    }
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

fn process_identity_matches(
    expected: &RuntimeProcessIdentity,
    actual: &RuntimeProcessIdentity,
) -> bool {
    expected.pid == actual.pid
        && expected.process_group_id == actual.process_group_id
        && expected.start_token.is_some()
        && expected.start_token == actual.start_token
}

#[cfg(target_os = "linux")]
fn inspect_process(pid: u32) -> Option<RuntimeProcessIdentity> {
    let contents = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, fields) = contents.rsplit_once(") ")?;
    let fields = fields.split_whitespace().collect::<Vec<_>>();
    let process_group_id = fields.get(2)?.parse::<u32>().ok()?;
    let start_token = fields.get(19)?.to_string();
    Some(RuntimeProcessIdentity {
        pid,
        process_group_id: Some(process_group_id),
        start_token: Some(start_token),
    })
}

#[cfg(all(unix, not(target_os = "linux")))]
fn inspect_process(pid: u32) -> Option<RuntimeProcessIdentity> {
    #[cfg(target_os = "macos")]
    return super::platform_process::inspect_process(pid);

    #[cfg(not(target_os = "macos"))]
    {
        let result = unsafe { libc::kill(pid as i32, 0) };
        if result != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            return None;
        }
        Some(RuntimeProcessIdentity {
            pid,
            process_group_id: None,
            start_token: None,
        })
    }
}

#[cfg(windows)]
fn inspect_process(pid: u32) -> Option<RuntimeProcessIdentity> {
    super::platform_process::inspect_process(pid)
}

#[cfg(all(not(unix), not(windows)))]
fn inspect_process(_pid: u32) -> Option<RuntimeProcessIdentity> {
    None
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
            start_token: Some("100".into()),
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

        let quarantined =
            RuntimeLease::starting(id.clone(), "1.2.3".into(), None, "nonce-quarantined".into());
        store
            .publish(quarantined)
            .expect("the second lease should be persisted");
        store
            .quarantine(&id, "nonce-quarantined")
            .expect("the lease should be quarantined");
        assert_eq!(
            store.load().unwrap()[0].state,
            RuntimeLeaseState::Quarantined
        );
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
            host_instance_id: None,
            launch_attempt_id: None,
            started_at_ms: 1,
        };
        assert!(matches!(
            store.publish(lease),
            Err(RuntimeLeaseError::Invalid(_))
        ));
    }

    #[test]
    fn platform_reconciler_never_kills_an_absent_process() {
        let lease = RuntimeLease::starting(
            ExtensionId("example.plugin".into()),
            "1.0.0".into(),
            None,
            "nonce-absent".into(),
        )
        .running(RuntimeProcessIdentity {
            pid: u32::MAX,
            process_group_id: Some(u32::MAX),
            start_token: Some("missing".into()),
        });
        let reconciler = PlatformRuntimeOrphanReconciler;
        assert!(matches!(
            reconciler.observe(&lease),
            RuntimeOrphanObservation::Absent | RuntimeOrphanObservation::IdentityUncertain
        ));
        assert!(matches!(
            reconciler.terminate(&lease),
            Err(RuntimeLeaseError::Invalid(_))
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_reconciler_uses_process_start_token_to_reject_pid_reuse() {
        let process = inspect_process(std::process::id()).expect("the test process should exist");
        let lease = RuntimeLease::starting(
            ExtensionId("example.plugin".into()),
            "1.0.0".into(),
            None,
            "nonce-current".into(),
        )
        .running(process.clone());
        let reconciler = PlatformRuntimeOrphanReconciler;
        assert_eq!(
            reconciler.observe(&lease),
            RuntimeOrphanObservation::Matching
        );

        let mut reused = process;
        reused.start_token = Some("different-start-time".into());
        let reused_lease = RuntimeLease::starting(
            ExtensionId("example.plugin".into()),
            "1.0.0".into(),
            None,
            "nonce-reused".into(),
        )
        .running(reused);
        assert_eq!(
            reconciler.observe(&reused_lease),
            RuntimeOrphanObservation::DifferentProcess
        );
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn native_reconciler_uses_platform_start_identity() {
        let process = inspect_process(std::process::id()).expect("the test process should exist");
        assert!(process.start_token.is_some());
        let lease = RuntimeLease::starting(
            ExtensionId("example.plugin".into()),
            "1.0.0".into(),
            None,
            "nonce-native".into(),
        )
        .running(process);
        assert_eq!(
            PlatformRuntimeOrphanReconciler.observe(&lease),
            RuntimeOrphanObservation::Matching
        );
    }

    #[cfg(unix)]
    #[test]
    fn native_reconciler_terminates_confirmed_orphan_process_group() {
        use std::os::unix::process::CommandExt;
        use std::process::Command;

        let mut child = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .expect("the release qualification child should start");
        let process = inspect_process(child.id()).expect("the child identity should be readable");
        let lease = RuntimeLease::starting(
            ExtensionId("example.plugin".into()),
            "1.0.0".into(),
            None,
            "nonce-orphan-process".into(),
        )
        .running(process);
        let reconciler = PlatformRuntimeOrphanReconciler;

        assert_eq!(
            reconciler.observe(&lease),
            RuntimeOrphanObservation::Matching
        );
        reconciler
            .terminate(&lease)
            .expect("a confirmed orphan process group should be terminable");
        let status = child.wait().expect("the child status should be reapable");
        assert!(!status.success(), "the orphan child should be terminated");
    }
}
