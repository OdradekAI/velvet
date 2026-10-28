use std::collections::{BTreeMap, BTreeSet};

use velvet_plugin_api::{ContributionKind, ExtensionId, ExtensionManifest, ExtensionSummary};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginError {
    NotFound(ExtensionId),
    AlreadyRegistered(ExtensionId),
    IncompatibleHostApi(ExtensionId),
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
}

#[derive(Default)]
pub struct InMemoryPluginHost {
    manifests: BTreeMap<ExtensionId, ExtensionManifest>,
    active: BTreeSet<ExtensionId>,
}

impl PluginHost for InMemoryPluginHost {
    fn register(&mut self, manifest: ExtensionManifest) -> Result<(), PluginError> {
        if self.manifests.contains_key(&manifest.id) {
            return Err(PluginError::AlreadyRegistered(manifest.id));
        }

        for contribution in &manifest.contributions {
            if contribution.kind == ContributionKind::View && contribution.zone.is_none() {
                return Err(PluginError::InvalidContribution(format!(
                    "view contribution {} must declare a zone",
                    contribution.id
                )));
            }
        }

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_layout_model::WorkbenchZone;
    use velvet_plugin_api::{ContributionDescriptor, ContributionKind, RuntimeKind, VersionRange};

    #[test]
    fn view_contributions_must_declare_a_zone() {
        let manifest = ExtensionManifest {
            id: ExtensionId("example".into()),
            version: "0.1.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Wasm,
            contributions: vec![ContributionDescriptor {
                id: "example.view".into(),
                kind: ContributionKind::View,
                zone: None,
            }],
            permission_sets: vec![],
        };
        let mut host = InMemoryPluginHost::default();

        assert!(matches!(
            host.register(manifest),
            Err(PluginError::InvalidContribution(_))
        ));
    }

    #[test]
    fn valid_view_contributions_are_discoverable() {
        let manifest = ExtensionManifest {
            id: ExtensionId("example".into()),
            version: "0.1.0".into(),
            host_api: VersionRange("^1".into()),
            runtime: RuntimeKind::Wasm,
            contributions: vec![ContributionDescriptor {
                id: "example.view".into(),
                kind: ContributionKind::View,
                zone: Some(WorkbenchZone::Contextual),
            }],
            permission_sets: vec![],
        };
        let mut host = InMemoryPluginHost::default();

        host.register(manifest).unwrap();
        assert_eq!(host.discover().len(), 1);
    }
}
