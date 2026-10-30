use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use velvet_layout_model::WorkbenchZone;
use velvet_plugin_api::{
    validate_reserved_metadata, AppTarget, ContributionKind, ContributionScope, ExtensionId,
    ExtensionInstallOperation, ExtensionInstallPreview, ExtensionInstallRecovery,
    ExtensionInstallRecoveryAction, ExtensionInstallRecoveryKind, ExtensionInstallRecoveryLogEntry,
    ExtensionInstallRecoveryLogStatus, ExtensionManifest, ExtensionProjection, ExtensionState,
    ExtensionSummary, HostContainer, Permission, RuntimeKind, VersionRange,
    MANIFEST_SCHEMA_VERSION,
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
    InstallFailed(String),
    InstallConflict(ExtensionId),
    VersionConflict {
        extension_id: ExtensionId,
        installed: String,
        requested: String,
    },
    UninstallFailed(String),
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

const RUNTIME_PROTOCOL_VERSION: u16 = 1;
const RUNTIME_INVOCATION_TIMEOUT: Duration = Duration::from_secs(5);
const RECOVERY_LOG_FILE: &str = ".plugin-recovery-log.json";
const RECOVERY_LOG_LIMIT: usize = 100;

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
}

pub trait RuntimeProcess: Send {
    fn poll(&mut self) -> RuntimeHealth;
    fn shutdown(&mut self) -> Result<(), RuntimeError>;
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
        if manifest.runtime == RuntimeKind::BuiltIn {
            return Ok(Box::new(NoopRuntimeProcess));
        }

        let Some(spec) = manifest.launch.as_ref() else {
            return Err(RuntimeError::MissingLaunchSpec(manifest.id.clone()));
        };

        if spec.program.trim().is_empty() {
            return Err(RuntimeError::InvalidLaunchSpec(manifest.id.clone()));
        }

        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .env("VELVET_EXTENSION_ID", &manifest.id.0)
            .env("VELVET_PLUGIN_PERMISSIONS", permissions.as_env_value())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|error| RuntimeError::StartFailed(error.to_string()))?;

        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RuntimeError::StartFailed(
                    "external runtime did not expose stdin".into(),
                ));
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RuntimeError::StartFailed(
                    "external runtime did not expose stdout".into(),
                ));
            }
        };
        let (requests, receiver) = mpsc::channel();
        let worker = match thread::Builder::new()
            .name(format!("velvet-plugin-{}", manifest.id.0))
            .spawn(move || runtime_io_loop(stdin, stdout, receiver))
        {
            Ok(worker) => worker,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RuntimeError::StartFailed(error.to_string()));
            }
        };

        Ok(Box::new(ChildRuntimeProcess {
            child,
            requests: Some(requests),
            worker: Some(worker),
        }))
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
    child: Child,
    requests: Option<Sender<RuntimeIoRequest>>,
    worker: Option<JoinHandle<()>>,
}

impl RuntimeProcess for ChildRuntimeProcess {
    fn poll(&mut self) -> RuntimeHealth {
        match self.child.try_wait() {
            Ok(Some(status)) => RuntimeHealth::Crashed(format!("process exited with {status}")),
            Ok(None) => RuntimeHealth::Running,
            Err(error) => RuntimeHealth::Crashed(error.to_string()),
        }
    }

    fn shutdown(&mut self) -> Result<(), RuntimeError> {
        if let Some(requests) = self.requests.take() {
            let _ = requests.send(RuntimeIoRequest::Stop);
        }

        let shutdown_result = match self.child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => {
                self.child
                    .kill()
                    .map_err(|error| RuntimeError::ShutdownFailed(error.to_string()))?;
                self.child
                    .wait()
                    .map(|_| ())
                    .map_err(|error| RuntimeError::ShutdownFailed(error.to_string()))
            }
            Err(error) => Err(RuntimeError::ShutdownFailed(error.to_string())),
        };

        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() && shutdown_result.is_ok() {
                return Err(RuntimeError::ShutdownFailed(
                    "runtime I/O worker panicked".into(),
                ));
            }
        }

        shutdown_result
    }

    fn invoke(
        &mut self,
        invocation: &PluginInvocation,
    ) -> Result<PluginInvocationResult, RuntimeError> {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                return Err(RuntimeError::InvocationFailed(format!(
                    "runtime exited before invocation with {status}"
                )))
            }
            Ok(None) => {}
            Err(error) => return Err(RuntimeError::InvocationFailed(error.to_string())),
        }

        let request = RuntimeInvocationRequest {
            protocol_version: RUNTIME_PROTOCOL_VERSION,
            kind: "invoke-command",
            method: &invocation.method,
            extension_id: &invocation.extension_id.0,
            command_id: &invocation.command_id,
            args: &invocation.args,
            context: &invocation.context,
        };
        let line = serde_json::to_string(&request)
            .map_err(|error| RuntimeError::InvocationFailed(error.to_string()))?;
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
        })
    }
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
        _invocation: &PluginInvocation,
    ) -> Result<PluginInvocationResult, RuntimeError> {
        Ok(PluginInvocationResult {
            result: Some(serde_json::json!({ "accepted": true })),
            error: None,
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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInvocationResponse {
    protocol_version: u16,
    ok: bool,
    result: Option<Value>,
    error: Option<String>,
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
                    let mut response_line = String::new();
                    let read = stdout
                        .read_line(&mut response_line)
                        .map_err(|error| RuntimeError::InvocationFailed(error.to_string()))?;
                    if read == 0 {
                        return Err(RuntimeError::InvocationFailed(
                            "runtime closed stdout without a response".into(),
                        ));
                    }
                    Ok(response_line)
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

            match host.register(manifest) {
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
    fn discover(&self) -> Vec<ExtensionSummary>;
    fn projection(&self, id: &ExtensionId) -> Result<ExtensionProjection, PluginError>;
    fn activate(&mut self, id: &ExtensionId) -> Result<(), PluginError>;
    fn disable(&mut self, id: &ExtensionId, reason: DisableReason) -> Result<(), PluginError>;
    fn active_contributions(
        &self,
        id: &ExtensionId,
    ) -> Result<Vec<velvet_plugin_api::ContributionDescriptor>, PluginError>;
    fn check_health(&mut self, id: &ExtensionId) -> Result<ExtensionState, PluginError>;
    fn recover(&mut self, id: &ExtensionId) -> Result<(), PluginError>;
    fn invoke_command(
        &mut self,
        id: &ExtensionId,
        command_id: String,
        args: Value,
        context: Value,
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
    fn install_from_directory(
        &mut self,
        source_dir: &Path,
        install_root: &Path,
    ) -> Result<ExtensionSummary, PluginError>;
    fn inspect_from_directory(
        &self,
        source_dir: &Path,
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
}

pub struct InMemoryPluginHost {
    manifests: BTreeMap<ExtensionId, ExtensionManifest>,
    active: BTreeSet<ExtensionId>,
    states: BTreeMap<ExtensionId, ExtensionState>,
    processes: BTreeMap<ExtensionId, Box<dyn RuntimeProcess>>,
    runtime: Box<dyn PluginRuntime>,
    host_api: VersionRange,
    apps: BTreeMap<String, VersionRange>,
    pending_installs: BTreeMap<ExtensionId, InstallBackup>,
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
            active: BTreeSet::new(),
            states: BTreeMap::new(),
            processes: BTreeMap::new(),
            runtime: Box::new(ProcessPluginRuntime),
            host_api: VersionRange("1".into()),
            apps,
            pending_installs: BTreeMap::new(),
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

    pub fn register_app(&mut self, app_id: String, api: VersionRange) {
        self.apps.insert(app_id, api);
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
        self.manifests.remove(id);
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
        let result = self
            .processes
            .get_mut(id)
            .ok_or_else(|| PluginError::InvalidState {
                extension_id: id.clone(),
                state: ExtensionState::Failed,
            })?
            .invoke(&invocation);

        match result {
            Ok(result) => {
                if let Some(error) = &result.error {
                    return Err(PluginError::RuntimeInvocation {
                        extension_id: id.clone(),
                        message: error.clone(),
                    });
                }
                Ok(result)
            }
            Err(RuntimeError::InvocationRejected(message)) => Err(PluginError::RuntimeInvocation {
                extension_id: id.clone(),
                message,
            }),
            Err(error) => {
                let message = runtime_error_message(error);
                if let Some(mut process) = self.processes.remove(id) {
                    let _ = process.shutdown();
                }
                self.active.remove(id);
                self.states.insert(id.clone(), ExtensionState::Failed);
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
}

impl PluginHost for InMemoryPluginHost {
    fn register(&mut self, manifest: ExtensionManifest) -> Result<(), PluginError> {
        let manifest = self.normalize_manifest(manifest)?;
        if self.manifests.contains_key(&manifest.id) {
            return Err(PluginError::AlreadyRegistered(manifest.id));
        }

        self.validate_manifest(&manifest)?;
        self.states
            .insert(manifest.id.clone(), ExtensionState::Registered);
        self.manifests.insert(manifest.id.clone(), manifest);
        Ok(())
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

        self.states.insert(id.clone(), ExtensionState::Starting);
        let permissions = PermissionGrant::from_manifest(&manifest);
        match self.runtime.start(&manifest, &permissions) {
            Ok(process) => {
                self.processes.insert(id.clone(), process);
                self.active.insert(id.clone());
                self.states.insert(id.clone(), ExtensionState::Active);
                Ok(())
            }
            Err(error) => {
                self.states.insert(id.clone(), ExtensionState::Failed);
                Err(PluginError::RuntimeStart {
                    extension_id: id.clone(),
                    message: runtime_error_message(error),
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
            .and_then(|mut process| process.shutdown().err());
        self.states.insert(id.clone(), ExtensionState::Disabled);
        if let Some(error) = shutdown_error {
            return Err(PluginError::RuntimeShutdown {
                extension_id: id.clone(),
                message: runtime_error_message(error),
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

    fn check_health(&mut self, id: &ExtensionId) -> Result<ExtensionState, PluginError> {
        if !self.manifests.contains_key(id) {
            return Err(PluginError::NotFound(id.clone()));
        }

        let Some(process) = self.processes.get_mut(id) else {
            return Ok(self
                .states
                .get(id)
                .cloned()
                .unwrap_or(ExtensionState::Registered));
        };

        match process.poll() {
            RuntimeHealth::Running => Ok(ExtensionState::Active),
            RuntimeHealth::Crashed(message) => {
                if let Some(mut process) = self.processes.remove(id) {
                    let _ = process.shutdown();
                }
                self.active.remove(id);
                self.states.insert(id.clone(), ExtensionState::Failed);
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
        if let Err(error) = self.register(manifest) {
            let _ = fs::remove_dir_all(&destination);
            if let Some(backup_path) = &backup_path {
                let _ = fs::rename(backup_path, &destination);
                if let Some(previous_manifest) = previous_manifest {
                    let _ = self.register(previous_manifest);
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
            self.register(previous_manifest)?;
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
                    if let Err(error) = self.register(manifest) {
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
                        self.register(manifest)?;
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
                    if let Err(error) = self.register(previous_manifest) {
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
        Ok(())
    }
}

fn namespace_for(scope: &ContributionScope, extension_id: &ExtensionId) -> String {
    match scope {
        ContributionScope::Workbench => "workbench".into(),
        ContributionScope::WorkbenchApp { app_id } => format!("workbench.app.{app_id}"),
        ContributionScope::Capability => format!("capability.{}", extension_id.0),
    }
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
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use velvet_layout_model::WorkbenchZone;
    use velvet_plugin_api::{
        ContextAnchor, ContributionDescriptor, Permission, PreviewAugmentationSlot,
        PreviewContributionDescriptor, PreviewIntent, PreviewSurface, RuntimeKind,
        RuntimeLaunchSpec, UiSurfaceDescriptor, UiSurfaceKind, ZonePermissionSet,
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
            _invocation: &PluginInvocation,
        ) -> Result<PluginInvocationResult, RuntimeError> {
            if let Some(error) = self.invocation_error.clone() {
                return Err(error);
            }
            Ok(PluginInvocationResult {
                result: Some(serde_json::json!({ "accepted": true })),
                error: None,
            })
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

        host.recover(&id).unwrap();
        assert_eq!(host.discover()[0].state, ExtensionState::Active);
        host.disable(&id, DisableReason::User).unwrap();
        assert_eq!(*shutdowns.lock().unwrap(), 2);
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
}
