use std::collections::{BTreeMap, BTreeSet};

use velvet_plugin_api::{
    AppTarget, ContributionKind, ContributionScope, ExtensionId, ExtensionManifest,
    ExtensionSummary, HostContainer, VersionRange,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginError {
    NotFound(ExtensionId),
    AlreadyRegistered(ExtensionId),
    IncompatibleHostApi(ExtensionId),
    UnknownWorkbenchApp(String),
    InvalidNamespace(String),
    InvalidContribution(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisableReason {
    User,
    Crash,
    Incompatible,
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
}

pub struct InMemoryPluginHost {
    manifests: BTreeMap<ExtensionId, ExtensionManifest>,
    active: BTreeSet<ExtensionId>,
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

    pub fn register_app(&mut self, app_id: String, api: VersionRange) {
        self.apps.insert(app_id, api);
    }

    fn validate_manifest(&self, manifest: &ExtensionManifest) -> Result<(), PluginError> {
        if !api_compatible(&manifest.host_api, &self.host_api) {
            return Err(PluginError::IncompatibleHostApi(manifest.id.clone()));
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
            })
            .collect()
    }

    fn activate(&mut self, id: &ExtensionId) -> Result<(), PluginError> {
        if !self.manifests.contains_key(id) {
            return Err(PluginError::NotFound(id.clone()));
        }

        self.active.insert(id.clone());
        Ok(())
    }

    fn disable(&mut self, id: &ExtensionId, _reason: DisableReason) -> Result<(), PluginError> {
        if !self.manifests.contains_key(id) {
            return Err(PluginError::NotFound(id.clone()));
        }

        self.active.remove(id);
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
}

fn namespace_for(scope: &ContributionScope, extension_id: &ExtensionId) -> String {
    match scope {
        ContributionScope::Workbench => "workbench".into(),
        ContributionScope::WorkbenchApp { app_id } => format!("workbench.app.{app_id}"),
        ContributionScope::Capability => format!("capability.{}", extension_id.0),
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
    use super::*;
    use velvet_layout_model::WorkbenchZone;
    use velvet_plugin_api::{ContributionDescriptor, RuntimeKind};

    fn view_manifest(zone: Option<WorkbenchZone>) -> ExtensionManifest {
        ExtensionManifest {
            id: ExtensionId("example".into()),
            version: "0.1.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Wasm,
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
            runtime: RuntimeKind::Wasm,
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
}
