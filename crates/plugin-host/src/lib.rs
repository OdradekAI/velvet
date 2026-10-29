use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use velvet_layout_model::WorkbenchZone;
use velvet_plugin_api::{
    AppTarget, ContributionKind, ContributionScope, ExtensionId, ExtensionManifest,
    ExtensionProjection, ExtensionState, ExtensionSummary, HostContainer, Permission, RuntimeKind,
    VersionRange,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PluginInvocation {
    pub extension_id: ExtensionId,
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
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ManifestLoadReport::default());
            }
            Err(error) => {
                return Err(ManifestDiscoveryError::Unavailable {
                    root: self.root.clone(),
                    message: error.to_string(),
                });
            }
        };

        for entry in entries {
            let entry = entry.map_err(|error| ManifestDiscoveryError::Unavailable {
                root: self.root.clone(),
                message: error.to_string(),
            })?;
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            {
                paths.push(path);
            }
        }
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
}

pub struct InMemoryPluginHost {
    manifests: BTreeMap<ExtensionId, ExtensionManifest>,
    active: BTreeSet<ExtensionId>,
    states: BTreeMap<ExtensionId, ExtensionState>,
    processes: BTreeMap<ExtensionId, Box<dyn RuntimeProcess>>,
    runtime: Box<dyn PluginRuntime>,
    host_api: VersionRange,
    apps: BTreeMap<String, VersionRange>,
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
            if contribution.namespace != expected_namespace {
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
}

impl PluginHost for InMemoryPluginHost {
    fn register(&mut self, manifest: ExtensionManifest) -> Result<(), PluginError> {
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
            target_app: manifest.target_app.clone(),
            contributions: manifest.contributions.clone(),
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
            contribution.id == command_id && contribution.kind == ContributionKind::Command
        }) {
            return Err(PluginError::InvalidContribution(format!(
                "command {} is not declared by extension {}",
                command_id, id.0
            )));
        }

        let invocation = PluginInvocation {
            extension_id: id.clone(),
            command_id,
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

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use velvet_layout_model::WorkbenchZone;
    use velvet_plugin_api::{
        ContextAnchor, ContributionDescriptor, Permission, RuntimeKind, RuntimeLaunchSpec,
        UiSurfaceDescriptor, UiSurfaceKind, ZonePermissionSet,
    };

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
        std::env::temp_dir().join(format!(
            "velvet-plugin-manifests-{}-{nonce}",
            std::process::id()
        ))
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
                namespace: "workbench".into(),
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
            }],
            permission_sets: vec![],
        }
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
        assert_eq!(projection.contributions[0].namespace, "workbench");
        assert_eq!(
            projection.contributions[0]
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
                namespace: "workbench.app.feed-workbench".into(),
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
            }],
            permission_sets: vec![],
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
        };

        let mut process = runtime
            .start(&manifest, &PermissionGrant::default())
            .expect("the declared sidecar should start");
        let result = process
            .invoke(&PluginInvocation {
                extension_id: manifest.id.clone(),
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
        assert!(projection.contributions.iter().any(|contribution| {
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
}
