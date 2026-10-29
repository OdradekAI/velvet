use std::collections::{BTreeMap, BTreeSet};
use std::process::{Child, Command, Stdio};

use velvet_layout_model::WorkbenchZone;
use velvet_plugin_api::{
    AppTarget, ContributionKind, ContributionScope, ExtensionId, ExtensionManifest, ExtensionState,
    ExtensionSummary, HostContainer, Permission, RuntimeKind, VersionRange,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeHealth {
    Running,
    Crashed(String),
}

pub trait RuntimeProcess: Send {
    fn poll(&mut self) -> RuntimeHealth;
    fn shutdown(&mut self) -> Result<(), RuntimeError>;
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
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command
            .spawn()
            .map_err(|error| RuntimeError::StartFailed(error.to_string()))?;

        Ok(Box::new(ChildRuntimeProcess { child }))
    }
}

struct ChildRuntimeProcess {
    child: Child,
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
        match self.child.try_wait() {
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
        }
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
}

pub trait PluginHost {
    fn register(&mut self, manifest: ExtensionManifest) -> Result<(), PluginError>;
    fn discover(&self) -> Vec<ExtensionSummary>;
    fn activate(&mut self, id: &ExtensionId) -> Result<(), PluginError>;
    fn disable(&mut self, id: &ExtensionId, reason: DisableReason) -> Result<(), PluginError>;
    fn active_contributions(
        &self,
        id: &ExtensionId,
    ) -> Result<Vec<velvet_plugin_api::ContributionDescriptor>, PluginError>;
    fn check_health(&mut self, id: &ExtensionId) -> Result<ExtensionState, PluginError>;
    fn recover(&mut self, id: &ExtensionId) -> Result<(), PluginError>;
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
                self.processes.remove(id);
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
    use std::sync::{Arc, Mutex};

    use super::*;
    use velvet_layout_model::WorkbenchZone;
    use velvet_plugin_api::{
        ContributionDescriptor, Permission, RuntimeKind, RuntimeLaunchSpec, ZonePermissionSet,
    };

    enum StartPlan {
        Fail(String),
        Process(Vec<RuntimeHealth>),
    }

    struct FakeRuntime {
        plans: VecDeque<StartPlan>,
        grants: Arc<Mutex<Vec<PermissionGrant>>>,
        shutdowns: Arc<Mutex<usize>>,
    }

    struct FakeProcess {
        health: VecDeque<RuntimeHealth>,
        shutdowns: Arc<Mutex<usize>>,
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
        assert_eq!(*shutdowns.lock().unwrap(), 1);
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
}
