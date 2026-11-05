//! Host-owned contribution registry.
//!
//! The registry is deliberately independent from runtime implementations and
//! frontend registries. It stores only validated, declarative contribution
//! projections and opaque registration handles. A runtime never receives a
//! reference to this map or to a renderer object.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use velvet_layout_model::WorkbenchZone;
use velvet_plugin_api::{ContributionDescriptor, ContributionKind, ContributionScope, ExtensionId};

/// Version of the host-owned contribution projection.
pub const CONTRIBUTION_REGISTRY_SCHEMA_VERSION: u16 = 1;

/// Version of a single contribution descriptor registration.
pub const CONTRIBUTION_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct ContributionRegistrationHandle {
    extension_id: ExtensionId,
    contribution_id: String,
    namespace: String,
    /// Opaque to callers. It is only useful when passed back to the owning
    /// `PluginHost` and is never interpreted by a plugin runtime.
    token: u64,
}

impl std::fmt::Debug for ContributionRegistrationHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContributionRegistrationHandle")
            .field("extension_id", &self.extension_id)
            .field("contribution_id", &self.contribution_id)
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}

impl ContributionRegistrationHandle {
    pub fn extension_id(&self) -> &ExtensionId {
        &self.extension_id
    }

    pub fn contribution_id(&self) -> &str {
        &self.contribution_id
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContributionRegistration {
    pub extension_id: ExtensionId,
    pub contribution_id: String,
    pub namespace: String,
    pub kind: ContributionKind,
    pub scope: ContributionScope,
    pub zone: Option<WorkbenchZone>,
    pub schema_version: u16,
    pub descriptor: ContributionDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContributionRegistrySnapshot {
    pub schema_version: u16,
    pub generation: u64,
    pub contributions: Vec<ContributionRegistration>,
}

impl Default for ContributionRegistrySnapshot {
    fn default() -> Self {
        Self {
            schema_version: CONTRIBUTION_REGISTRY_SCHEMA_VERSION,
            generation: 0,
            contributions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RegistryError {
    Duplicate(String),
    InvalidNamespace(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(key) => write!(formatter, "duplicate contribution registration {key}"),
            Self::InvalidNamespace(id) => write!(formatter, "contribution {id} has no namespace"),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RegistrationKey {
    extension_id: ExtensionId,
    namespace: String,
    contribution_id: String,
}

#[derive(Clone, Debug)]
struct RegisteredContribution {
    handle: ContributionRegistrationHandle,
    registration: ContributionRegistration,
}

impl RegistrationKey {
    fn new(extension_id: &ExtensionId, namespace: &str, contribution_id: &str) -> Self {
        Self {
            extension_id: extension_id.clone(),
            namespace: namespace.into(),
            contribution_id: contribution_id.into(),
        }
    }
}

#[derive(Default)]
pub(crate) struct ContributionRegistry {
    generation: u64,
    next_token: u64,
    entries: BTreeMap<RegistrationKey, RegisteredContribution>,
}

impl ContributionRegistry {
    pub(crate) fn register_extension(
        &mut self,
        extension_id: &ExtensionId,
        descriptors: &[ContributionDescriptor],
    ) -> Result<Vec<ContributionRegistrationHandle>, RegistryError> {
        let mut keys = Vec::with_capacity(descriptors.len());
        for descriptor in descriptors {
            let Some(namespace) = descriptor.namespace.as_deref() else {
                return Err(RegistryError::InvalidNamespace(descriptor.id.clone()));
            };
            let key = RegistrationKey::new(extension_id, namespace, &descriptor.id);
            if self.entries.contains_key(&key) || keys.iter().any(|candidate| candidate == &key) {
                return Err(RegistryError::Duplicate(format!(
                    "{}:{}:{}",
                    extension_id.0, namespace, descriptor.id
                )));
            }
            keys.push(key);
        }

        if descriptors.is_empty() {
            return Ok(Vec::new());
        }

        self.generation = self.generation.saturating_add(1);
        let mut handles = Vec::with_capacity(descriptors.len());
        for (descriptor, key) in descriptors.iter().zip(keys) {
            self.next_token = self.next_token.saturating_add(1);
            let namespace = key.namespace.clone();
            let handle = ContributionRegistrationHandle {
                extension_id: extension_id.clone(),
                contribution_id: descriptor.id.clone(),
                namespace: namespace.clone(),
                token: self.next_token,
            };
            let registration = ContributionRegistration {
                extension_id: extension_id.clone(),
                contribution_id: descriptor.id.clone(),
                namespace,
                kind: descriptor.kind.clone(),
                scope: descriptor.scope.clone(),
                zone: descriptor.zone.clone(),
                schema_version: CONTRIBUTION_SCHEMA_VERSION,
                descriptor: descriptor.clone(),
            };
            self.entries.insert(
                key,
                RegisteredContribution {
                    handle: handle.clone(),
                    registration,
                },
            );
            handles.push(handle);
        }
        Ok(handles)
    }

    pub(crate) fn revoke_extension(&mut self, extension_id: &ExtensionId) -> usize {
        let keys = self
            .entries
            .keys()
            .filter(|key| &key.extension_id == extension_id)
            .cloned()
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return 0;
        }
        for key in &keys {
            self.entries.remove(key);
        }
        self.generation = self.generation.saturating_add(1);
        keys.len()
    }

    pub(crate) fn dispose(&mut self, handle: &ContributionRegistrationHandle) -> bool {
        let key = RegistrationKey::new(
            &handle.extension_id,
            &handle.namespace,
            &handle.contribution_id,
        );
        let matches_handle = self
            .entries
            .get(&key)
            .is_some_and(|entry| entry.handle == *handle);
        if !matches_handle {
            return false;
        }
        self.entries.remove(&key);
        self.generation = self.generation.saturating_add(1);
        true
    }

    pub(crate) fn contains(
        &self,
        extension_id: &ExtensionId,
        contribution_id: &str,
        kind: &ContributionKind,
    ) -> bool {
        self.entries.values().any(|entry| {
            &entry.registration.extension_id == extension_id
                && entry.registration.contribution_id == contribution_id
                && &entry.registration.kind == kind
        })
    }

    pub(crate) fn for_extension(
        &self,
        extension_id: &ExtensionId,
    ) -> Vec<ContributionRegistration> {
        self.entries
            .values()
            .filter(|entry| &entry.registration.extension_id == extension_id)
            .map(|entry| entry.registration.clone())
            .collect()
    }

    pub(crate) fn snapshot(&self) -> ContributionRegistrySnapshot {
        ContributionRegistrySnapshot {
            schema_version: CONTRIBUTION_REGISTRY_SCHEMA_VERSION,
            generation: self.generation,
            contributions: self
                .entries
                .values()
                .map(|entry| entry.registration.clone())
                .collect(),
        }
    }
}
