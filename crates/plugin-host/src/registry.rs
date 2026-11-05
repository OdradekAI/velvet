//! Host-owned contribution registry.
//!
//! The registry is deliberately independent from runtime implementations and
//! frontend registries. It stores only validated, declarative contribution
//! projections and opaque registration handles. A runtime never receives a
//! reference to this map or to a renderer object.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use velvet_layout_model::WorkbenchZone;
use velvet_plugin_api::{
    Capability, ContributionDescriptor, ContributionKind, ContributionScope, ExtensionId,
    KeybindingPlatform, MenuLocation,
};

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
    InvalidCommandSurface(String),
    KeybindingConflict(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(key) => write!(formatter, "duplicate contribution registration {key}"),
            Self::InvalidNamespace(id) => write!(formatter, "contribution {id} has no namespace"),
            Self::InvalidCommandSurface(message) => {
                write!(formatter, "invalid command surface: {message}")
            }
            Self::KeybindingConflict(key) => write!(formatter, "keybinding conflict {key}"),
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
        validate_command_surfaces(descriptors, &self.entries)?;
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

fn validate_command_surfaces(
    descriptors: &[ContributionDescriptor],
    existing: &BTreeMap<RegistrationKey, RegisteredContribution>,
) -> Result<(), RegistryError> {
    let command_ids = descriptors
        .iter()
        .filter(|descriptor| descriptor.kind == ContributionKind::Command)
        .map(|descriptor| descriptor.id.as_str())
        .collect::<BTreeSet<_>>();
    let command_scopes = descriptors
        .iter()
        .filter(|descriptor| descriptor.kind == ContributionKind::Command)
        .map(|descriptor| {
            (
                descriptor.id.as_str(),
                (&descriptor.scope, descriptor.namespace.as_deref()),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut keybindings = BTreeSet::new();
    for entry in existing.values() {
        if entry.registration.kind == ContributionKind::Keybinding {
            if let Some(keybinding) = &entry.registration.descriptor.keybinding {
                keybindings.extend(keybinding_keys(&entry.registration.descriptor, keybinding));
            }
        }
    }

    for descriptor in descriptors {
        for capability in &descriptor.required_capabilities {
            if Capability::from_name(capability).is_none() {
                return Err(RegistryError::InvalidCommandSurface(format!(
                    "contribution {} declares unknown capability {}",
                    descriptor.id, capability
                )));
            }
        }
        match descriptor.kind {
            ContributionKind::Command => {
                if descriptor.title.as_deref().is_none_or(str::is_empty) {
                    return Err(RegistryError::InvalidCommandSurface(format!(
                        "command {} must declare title metadata",
                        descriptor.id
                    )));
                }
            }
            ContributionKind::Menu | ContributionKind::ContextMenu => {
                if descriptor
                    .label
                    .as_deref()
                    .or(descriptor.title.as_deref())
                    .is_none_or(str::is_empty)
                {
                    return Err(RegistryError::InvalidCommandSurface(format!(
                        "menu {} must declare label or title metadata",
                        descriptor.id
                    )));
                }
                let Some(command_id) = descriptor.command_id.as_deref() else {
                    return Err(RegistryError::InvalidCommandSurface(format!(
                        "menu {} must reference a command",
                        descriptor.id
                    )));
                };
                if !command_ids.contains(command_id)
                    && !(descriptor.kind == ContributionKind::ContextMenu
                        && command_id == descriptor.id)
                {
                    return Err(RegistryError::InvalidCommandSurface(format!(
                        "menu {} references undeclared command {}",
                        descriptor.id, command_id
                    )));
                }
                if let Some((command_scope, command_namespace)) = command_scopes.get(command_id) {
                    if *command_scope != &descriptor.scope
                        || *command_namespace != descriptor.namespace.as_deref()
                    {
                        return Err(RegistryError::InvalidCommandSurface(format!(
                            "menu {} references command {} from another scope",
                            descriptor.id, command_id
                        )));
                    }
                }
                let inferred_location = (descriptor.kind == ContributionKind::ContextMenu)
                    .then_some(MenuLocation::ContextMenu);
                let Some(location) = descriptor
                    .menu_location
                    .as_ref()
                    .or(inferred_location.as_ref())
                else {
                    return Err(RegistryError::InvalidCommandSurface(format!(
                        "menu {} must declare a menu location",
                        descriptor.id
                    )));
                };
                if descriptor.kind == ContributionKind::ContextMenu
                    && !matches!(location, MenuLocation::ContextMenu)
                {
                    return Err(RegistryError::InvalidCommandSurface(format!(
                        "context menu {} must use the context-menu location",
                        descriptor.id
                    )));
                }
            }
            ContributionKind::Keybinding => {
                let Some(command_id) = descriptor.command_id.as_deref() else {
                    return Err(RegistryError::InvalidCommandSurface(format!(
                        "keybinding {} must reference a command",
                        descriptor.id
                    )));
                };
                if !command_ids.contains(command_id) {
                    return Err(RegistryError::InvalidCommandSurface(format!(
                        "keybinding {} references undeclared command {}",
                        descriptor.id, command_id
                    )));
                }
                if let Some((command_scope, command_namespace)) = command_scopes.get(command_id) {
                    if *command_scope != &descriptor.scope
                        || *command_namespace != descriptor.namespace.as_deref()
                    {
                        return Err(RegistryError::InvalidCommandSurface(format!(
                            "keybinding {} references command {} from another scope",
                            descriptor.id, command_id
                        )));
                    }
                }
                let Some(keybinding) = descriptor.keybinding.as_deref() else {
                    return Err(RegistryError::InvalidCommandSurface(format!(
                        "keybinding {} must declare a key sequence",
                        descriptor.id
                    )));
                };
                let keys = keybinding_keys(descriptor, keybinding);
                if let Some(conflict) = keys.iter().find(|key| keybindings.contains(*key)) {
                    return Err(RegistryError::KeybindingConflict(conflict.clone()));
                }
                keybindings.extend(keys);
            }
            _ => {}
        }
    }
    Ok(())
}

fn keybinding_keys(descriptor: &ContributionDescriptor, keybinding: &str) -> Vec<String> {
    let platforms = match descriptor.keybinding_platform.as_ref() {
        Some(KeybindingPlatform::Macos) => vec!["macos"],
        Some(KeybindingPlatform::Windows) => vec!["windows"],
        Some(KeybindingPlatform::Linux) => vec!["linux"],
        Some(KeybindingPlatform::Any) | None => vec!["macos", "windows", "linux"],
    };
    platforms
        .into_iter()
        .map(|platform| {
            format!(
                "{}:{}:{}",
                platform,
                normalize_keybinding(keybinding),
                serde_json::to_string(&descriptor.scope).unwrap_or_default()
            )
        })
        .collect()
}

fn normalize_keybinding(value: &str) -> String {
    value
        .split_whitespace()
        .map(|chord| {
            chord
                .split('+')
                .map(|part| -> String {
                    match part.trim().to_ascii_lowercase().as_str() {
                        "command" | "cmd" => "meta".into(),
                        "control" => "ctrl".into(),
                        "option" => "alt".into(),
                        "return" => "enter".into(),
                        "esc" => "escape".into(),
                        part => part.into(),
                    }
                })
                .collect::<Vec<_>>()
                .join("+")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_plugin_api::ContributionScope;

    fn command(id: &str) -> ContributionDescriptor {
        ContributionDescriptor {
            id: id.into(),
            namespace: Some("workbench".into()),
            title: Some(id.into()),
            ..ContributionDescriptor::default()
        }
    }

    #[test]
    fn command_surface_requires_declared_command_references() {
        let mut registry = ContributionRegistry::default();
        let mut menu = command("sample.menu");
        menu.kind = ContributionKind::ContextMenu;
        menu.command_id = Some("sample.missing".into());
        menu.menu_location = Some(MenuLocation::ContextMenu);
        assert!(matches!(
            registry.register_extension(&ExtensionId("sample".into()), &[menu]),
            Err(RegistryError::InvalidCommandSurface(message))
                if message.contains("undeclared command")
        ));
    }

    #[test]
    fn keybinding_normalization_detects_conflicts_across_extensions() {
        let mut registry = ContributionRegistry::default();
        let mut first = command("sample.keybinding");
        first.keybinding = Some("Ctrl+Shift+I".into());
        first.kind = ContributionKind::Keybinding;
        first.command_id = Some("sample.command".into());
        first.keybinding_platform = Some(KeybindingPlatform::Any);
        let mut declared_command = command("sample.command");
        declared_command.kind = ContributionKind::Command;
        registry
            .register_extension(&ExtensionId("sample".into()), &[declared_command, first])
            .expect("first command surface should register");

        let mut second_command = command("other.command");
        second_command.kind = ContributionKind::Command;
        let mut second = command("other.keybinding");
        second.kind = ContributionKind::Keybinding;
        second.command_id = Some("other.command".into());
        second.keybinding = Some("ctrl+shift+i".into());
        second.keybinding_platform = Some(KeybindingPlatform::Any);
        assert!(matches!(
            registry.register_extension(&ExtensionId("other".into()), &[second_command, second],),
            Err(RegistryError::KeybindingConflict(_))
        ));
    }

    #[test]
    fn app_scoped_command_surface_keeps_scope_in_the_conflict_key() {
        let mut registry = ContributionRegistry::default();
        let mut command = command("sample.command");
        command.scope = ContributionScope::WorkbenchApp {
            app_id: "asset-manager".into(),
        };
        command.namespace = Some("workbench.app.asset-manager".into());
        registry
            .register_extension(&ExtensionId("sample".into()), &[command])
            .expect("app-scoped command should register");
    }
}
