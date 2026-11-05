//! Host-owned capability grants, revocation and audit records.
//!
//! This module deliberately contains no filesystem, database, Tauri or
//! renderer handles. It turns manifest declarations and host/user policy into
//! bounded snapshots and opaque handles that the owning PluginHost can check
//! before every privileged operation.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use velvet_plugin_api::{Capability, ExtensionId, ExtensionManifest, Permission};

pub const CAPABILITY_SCHEMA_VERSION: u16 = 1;
pub const CAPABILITY_AUDIT_SCHEMA_VERSION: u16 = 1;
pub const MAX_CAPABILITY_AUDIT_RECORDS: usize = 256;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityDenyReason {
    NotRequested,
    HostPolicy,
    UserApprovalRequired,
    Revoked,
    Inactive,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityDenial {
    pub capability: Capability,
    pub reason: CapabilityDenyReason,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityAuditAction {
    Granted,
    Revoked,
    Denied,
    Cleared,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityAuditRecord {
    pub schema_version: u16,
    pub sequence: u64,
    pub extension_id: ExtensionId,
    pub capability: Capability,
    pub action: CapabilityAuditAction,
    pub reason: Option<CapabilityDenyReason>,
    pub generation: u64,
    pub occurred_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityGrantSnapshot {
    pub schema_version: u16,
    pub extension_id: ExtensionId,
    pub generation: u64,
    pub session_id: Option<String>,
    pub active: bool,
    pub requested: Vec<Capability>,
    pub granted: Vec<Capability>,
    pub denied: Vec<CapabilityDenial>,
}

impl Default for CapabilityGrantSnapshot {
    fn default() -> Self {
        Self {
            schema_version: CAPABILITY_SCHEMA_VERSION,
            extension_id: ExtensionId(String::new()),
            generation: 0,
            session_id: None,
            active: false,
            requested: Vec::new(),
            granted: Vec::new(),
            denied: Vec::new(),
        }
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct CapabilityHandle {
    extension_id: ExtensionId,
    capability: Capability,
    session_id: String,
    generation: u64,
    token: u64,
}

impl std::fmt::Debug for CapabilityHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapabilityHandle")
            .field("extension_id", &self.extension_id)
            .field("capability", &self.capability)
            .field("session_id", &self.session_id)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl CapabilityHandle {
    pub fn extension_id(&self) -> &ExtensionId {
        &self.extension_id
    }

    pub fn capability(&self) -> &Capability {
        &self.capability
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityBrokerError {
    UnknownExtension(ExtensionId),
    UnknownCapability(String),
    NotRequested(Capability),
    HostPolicyDenied(Capability),
    UserApprovalRequired(Capability),
    CapabilityRevoked(Capability),
    Inactive {
        extension_id: ExtensionId,
        capability: Capability,
    },
    InvalidHandle,
    SessionMismatch,
    GenerationConflict {
        expected: u64,
        actual: u64,
    },
    RequiredDenied(Capability),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityPolicy {
    host_allowed: BTreeSet<Capability>,
    require_user_grant: bool,
}

impl Default for CapabilityPolicy {
    fn default() -> Self {
        Self::legacy_compatible()
    }
}

impl CapabilityPolicy {
    /// Compatibility mode preserves the existing low-risk manifest checks;
    /// high-risk capabilities still require an explicit user grant.
    pub fn legacy_compatible() -> Self {
        Self {
            host_allowed: all_capabilities(),
            require_user_grant: false,
        }
    }

    /// Strict policy for tests and future production profiles. Every
    /// capability needs an explicit user grant, including read capabilities.
    pub fn deny_by_default() -> Self {
        Self {
            host_allowed: all_capabilities(),
            require_user_grant: true,
        }
    }

    pub fn with_host_allowed(mut self, capability: Capability, allowed: bool) -> Self {
        if allowed {
            self.host_allowed.insert(capability);
        } else {
            self.host_allowed.remove(&capability);
        }
        self
    }

    fn allows(&self, capability: &Capability) -> bool {
        self.host_allowed.contains(capability)
    }

    fn requires_user_grant(&self, capability: &Capability) -> bool {
        self.require_user_grant || capability.is_high_risk()
    }
}

#[derive(Clone, Debug)]
struct CapabilityRegistration {
    requested: BTreeSet<Capability>,
    required: BTreeSet<Capability>,
    explicit: BTreeSet<Capability>,
    user_grants: BTreeSet<Capability>,
    revoked: BTreeSet<Capability>,
    snapshot: CapabilityGrantSnapshot,
    handles: BTreeMap<Capability, CapabilityHandle>,
}

#[derive(Debug)]
pub struct CapabilityBroker {
    policy: CapabilityPolicy,
    registrations: BTreeMap<ExtensionId, CapabilityRegistration>,
    audit: VecDeque<CapabilityAuditRecord>,
    next_sequence: u64,
    next_token: u64,
}

impl Default for CapabilityBroker {
    fn default() -> Self {
        Self::new(CapabilityPolicy::default())
    }
}

impl CapabilityBroker {
    pub fn new(policy: CapabilityPolicy) -> Self {
        Self {
            policy,
            registrations: BTreeMap::new(),
            audit: VecDeque::new(),
            next_sequence: 1,
            next_token: 1,
        }
    }

    pub fn policy(&self) -> &CapabilityPolicy {
        &self.policy
    }

    pub fn set_policy(&mut self, policy: CapabilityPolicy) {
        self.policy = policy;
        let states = self
            .registrations
            .iter()
            .map(|(id, registration)| (id.clone(), registration.snapshot.active))
            .collect::<Vec<_>>();
        for (id, active) in states {
            let _ = self.recompute(&id, active);
        }
    }

    pub fn register(&mut self, manifest: &ExtensionManifest) -> Result<(), CapabilityBrokerError> {
        let (requested, required, explicit) = requested_capabilities(manifest)?;
        if self.registrations.contains_key(&manifest.id) {
            return Ok(());
        }
        self.registrations.insert(
            manifest.id.clone(),
            CapabilityRegistration {
                requested,
                required,
                explicit,
                user_grants: BTreeSet::new(),
                revoked: BTreeSet::new(),
                snapshot: CapabilityGrantSnapshot {
                    extension_id: manifest.id.clone(),
                    requested: requested_capabilities(manifest)?.0.into_iter().collect(),
                    ..CapabilityGrantSnapshot::default()
                },
                handles: BTreeMap::new(),
            },
        );
        Ok(())
    }

    pub fn unregister(&mut self, id: &ExtensionId) {
        if let Some(registration) = self.registrations.remove(id) {
            for capability in registration.snapshot.granted {
                self.record(
                    id,
                    capability,
                    CapabilityAuditAction::Cleared,
                    None,
                    registration.snapshot.generation,
                );
            }
        }
    }

    pub fn activate(
        &mut self,
        id: &ExtensionId,
    ) -> Result<(CapabilityGrantSnapshot, Vec<Permission>), CapabilityBrokerError> {
        let snapshot = self.recompute(id, true)?;
        let registration = self
            .registrations
            .get(id)
            .ok_or_else(|| CapabilityBrokerError::UnknownExtension(id.clone()))?;
        let denied_required = registration
            .required
            .iter()
            .find(|capability| !snapshot.granted.contains(capability));
        if let Some(capability) = denied_required {
            return Err(CapabilityBrokerError::RequiredDenied(capability.clone()));
        }
        let permissions = snapshot
            .granted
            .iter()
            .filter_map(Capability::legacy_permission)
            .collect();
        Ok((snapshot, permissions))
    }

    pub fn deactivate(&mut self, id: &ExtensionId) {
        let Some(registration) = self.registrations.get_mut(id) else {
            return;
        };
        let generation = registration.snapshot.generation.saturating_add(1);
        let cleared = registration.snapshot.granted.clone();
        registration.handles.clear();
        registration.user_grants.clear();
        registration.snapshot.generation = generation;
        registration.snapshot.session_id = None;
        registration.snapshot.active = false;
        registration.snapshot.granted.clear();
        registration.snapshot.denied = registration
            .requested
            .iter()
            .cloned()
            .map(|capability| CapabilityDenial {
                capability,
                reason: CapabilityDenyReason::Inactive,
            })
            .collect();
        for capability in cleared {
            self.record(
                id,
                capability,
                CapabilityAuditAction::Cleared,
                None,
                generation,
            );
        }
    }

    pub fn grant(
        &mut self,
        id: &ExtensionId,
        capabilities: &[Capability],
        expected_generation: Option<u64>,
    ) -> Result<CapabilityGrantSnapshot, CapabilityBrokerError> {
        self.check_generation(id, expected_generation)?;
        let active = {
            let registration = self
                .registrations
                .get_mut(id)
                .ok_or_else(|| CapabilityBrokerError::UnknownExtension(id.clone()))?;
            for capability in capabilities {
                if !registration.requested.contains(capability) {
                    return Err(CapabilityBrokerError::NotRequested(capability.clone()));
                }
                if !self.policy.allows(capability) {
                    return Err(CapabilityBrokerError::HostPolicyDenied(capability.clone()));
                }
            }
            for capability in capabilities {
                registration.user_grants.insert(capability.clone());
                registration.revoked.remove(capability);
            }
            registration.snapshot.active
        };
        let snapshot = self.recompute(id, active)?;
        for capability in capabilities {
            if snapshot.granted.contains(capability) {
                self.record(
                    id,
                    capability.clone(),
                    CapabilityAuditAction::Granted,
                    None,
                    snapshot.generation,
                );
            }
        }
        Ok(snapshot)
    }

    pub fn revoke(
        &mut self,
        id: &ExtensionId,
        capabilities: &[Capability],
        expected_generation: Option<u64>,
    ) -> Result<CapabilityGrantSnapshot, CapabilityBrokerError> {
        self.check_generation(id, expected_generation)?;
        let active = {
            let registration = self
                .registrations
                .get_mut(id)
                .ok_or_else(|| CapabilityBrokerError::UnknownExtension(id.clone()))?;
            for capability in capabilities {
                if !registration.requested.contains(capability) {
                    return Err(CapabilityBrokerError::NotRequested(capability.clone()));
                }
            }
            for capability in capabilities {
                registration.user_grants.remove(capability);
                registration.revoked.insert(capability.clone());
            }
            registration.snapshot.active
        };
        let snapshot = self.recompute(id, active)?;
        for capability in capabilities {
            self.record(
                id,
                capability.clone(),
                CapabilityAuditAction::Revoked,
                Some(CapabilityDenyReason::Revoked),
                snapshot.generation,
            );
        }
        Ok(snapshot)
    }

    pub fn snapshot(
        &self,
        id: &ExtensionId,
    ) -> Result<CapabilityGrantSnapshot, CapabilityBrokerError> {
        self.registrations
            .get(id)
            .map(|registration| registration.snapshot.clone())
            .ok_or_else(|| CapabilityBrokerError::UnknownExtension(id.clone()))
    }

    pub fn is_granted(
        &self,
        id: &ExtensionId,
        capability: &Capability,
    ) -> Result<bool, CapabilityBrokerError> {
        let snapshot = self.snapshot(id)?;
        Ok(snapshot.active && snapshot.granted.contains(capability))
    }

    pub fn handle(
        &self,
        id: &ExtensionId,
        capability: &Capability,
    ) -> Result<CapabilityHandle, CapabilityBrokerError> {
        let registration = self
            .registrations
            .get(id)
            .ok_or_else(|| CapabilityBrokerError::UnknownExtension(id.clone()))?;
        if !registration.snapshot.active {
            return Err(CapabilityBrokerError::Inactive {
                extension_id: id.clone(),
                capability: capability.clone(),
            });
        }
        registration
            .handles
            .get(capability)
            .cloned()
            .ok_or_else(|| {
                if registration.revoked.contains(capability) {
                    CapabilityBrokerError::CapabilityRevoked(capability.clone())
                } else {
                    CapabilityBrokerError::UserApprovalRequired(capability.clone())
                }
            })
    }

    pub fn check_handle(
        &self,
        id: &ExtensionId,
        handle: &CapabilityHandle,
    ) -> Result<(), CapabilityBrokerError> {
        let registration = self
            .registrations
            .get(id)
            .ok_or_else(|| CapabilityBrokerError::UnknownExtension(id.clone()))?;
        if handle.extension_id != *id {
            return Err(CapabilityBrokerError::SessionMismatch);
        }
        if registration.snapshot.session_id.as_deref() != Some(handle.session_id.as_str()) {
            return Err(CapabilityBrokerError::SessionMismatch);
        }
        let current = registration
            .handles
            .get(&handle.capability)
            .ok_or(CapabilityBrokerError::InvalidHandle)?;
        if current.session_id != handle.session_id {
            return Err(CapabilityBrokerError::SessionMismatch);
        }
        if current != handle {
            return Err(CapabilityBrokerError::InvalidHandle);
        }
        Ok(())
    }

    pub fn drain_audit(&mut self) -> Vec<CapabilityAuditRecord> {
        self.audit.drain(..).collect()
    }

    pub fn audit_snapshot(&self) -> Vec<CapabilityAuditRecord> {
        self.audit.iter().cloned().collect()
    }

    fn check_generation(
        &self,
        id: &ExtensionId,
        expected_generation: Option<u64>,
    ) -> Result<(), CapabilityBrokerError> {
        let actual = self.snapshot(id)?.generation;
        if let Some(expected) = expected_generation {
            if expected != actual {
                return Err(CapabilityBrokerError::GenerationConflict { expected, actual });
            }
        }
        Ok(())
    }

    fn recompute(
        &mut self,
        id: &ExtensionId,
        active: bool,
    ) -> Result<CapabilityGrantSnapshot, CapabilityBrokerError> {
        let policy = self.policy.clone();
        let mut audit = Vec::new();
        let mut next_token = self.next_token;
        let snapshot = {
            let registration = self
                .registrations
                .get_mut(id)
                .ok_or_else(|| CapabilityBrokerError::UnknownExtension(id.clone()))?;
            registration.handles.clear();
            registration.snapshot.generation = registration.snapshot.generation.saturating_add(1);
            registration.snapshot.active = active;
            registration.snapshot.granted.clear();
            registration.snapshot.denied.clear();
            registration.snapshot.session_id =
                active.then(|| format!("{}:capability:{}", id.0, registration.snapshot.generation));

            for capability in registration.requested.clone() {
                let denial = if registration.revoked.contains(&capability) {
                    Some(CapabilityDenyReason::Revoked)
                } else if !policy.allows(&capability) {
                    Some(CapabilityDenyReason::HostPolicy)
                } else if (policy.requires_user_grant(&capability)
                    || registration.explicit.contains(&capability))
                    && !registration.user_grants.contains(&capability)
                {
                    Some(CapabilityDenyReason::UserApprovalRequired)
                } else if !active {
                    Some(CapabilityDenyReason::Inactive)
                } else {
                    None
                };
                if let Some(reason) = denial {
                    registration.snapshot.denied.push(CapabilityDenial {
                        capability: capability.clone(),
                        reason: reason.clone(),
                    });
                    audit.push((capability, CapabilityAuditAction::Denied, Some(reason)));
                    continue;
                }
                let session_id = registration.snapshot.session_id.clone().unwrap_or_default();
                let handle = CapabilityHandle {
                    extension_id: id.clone(),
                    capability: capability.clone(),
                    session_id,
                    generation: registration.snapshot.generation,
                    token: next_token,
                };
                next_token = next_token.saturating_add(1);
                registration.snapshot.granted.push(capability.clone());
                registration.handles.insert(capability, handle);
            }
            registration.snapshot.granted.sort();
            registration
                .snapshot
                .denied
                .sort_by(|left, right| left.capability.cmp(&right.capability));
            registration.snapshot.clone()
        };
        self.next_token = next_token;
        for (capability, action, reason) in audit {
            self.record(id, capability, action, reason, snapshot.generation);
        }
        Ok(snapshot)
    }

    fn record(
        &mut self,
        extension_id: &ExtensionId,
        capability: Capability,
        action: CapabilityAuditAction,
        reason: Option<CapabilityDenyReason>,
        generation: u64,
    ) {
        let record = CapabilityAuditRecord {
            schema_version: CAPABILITY_AUDIT_SCHEMA_VERSION,
            sequence: self.next_sequence,
            extension_id: extension_id.clone(),
            capability,
            action,
            reason,
            generation,
            occurred_at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or_default(),
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        if self.audit.len() == MAX_CAPABILITY_AUDIT_RECORDS {
            self.audit.pop_front();
        }
        self.audit.push_back(record);
    }
}

type CapabilitySets = (
    BTreeSet<Capability>,
    BTreeSet<Capability>,
    BTreeSet<Capability>,
);

fn requested_capabilities(
    manifest: &ExtensionManifest,
) -> Result<CapabilitySets, CapabilityBrokerError> {
    let mut requested = manifest
        .permission_sets
        .iter()
        .flat_map(|set| set.permissions.iter())
        .map(permission_to_capability)
        .collect::<BTreeSet<_>>();
    let mut required = BTreeSet::new();
    let mut explicit = BTreeSet::new();
    for name in &manifest.required_capabilities {
        let capability = Capability::from_name(name)
            .ok_or_else(|| CapabilityBrokerError::UnknownCapability(name.clone()))?;
        requested.insert(capability.clone());
        required.insert(capability.clone());
        explicit.insert(capability);
    }
    for name in &manifest.optional_capabilities {
        let capability = Capability::from_name(name)
            .ok_or_else(|| CapabilityBrokerError::UnknownCapability(name.clone()))?;
        requested.insert(capability.clone());
        explicit.insert(capability);
    }
    Ok((requested, required, explicit))
}

fn permission_to_capability(permission: &Permission) -> Capability {
    Capability::from_name(permission.capability_name())
        .expect("legacy permissions are capabilities")
}

fn all_capabilities() -> BTreeSet<Capability> {
    [
        Capability::WorkspaceOpen,
        Capability::LibraryRead,
        Capability::SourceConnect,
        Capability::AssetRead,
        Capability::MetadataRead,
        Capability::MetadataWrite,
        Capability::PreviewRender,
        Capability::JobsCreate,
        Capability::JobsRead,
        Capability::LogsRead,
        Capability::ExternalProcess,
        Capability::Network,
        Capability::AssetFacetWrite,
        Capability::ConfigRead,
        Capability::ConfigWrite,
        Capability::StorageRead,
        Capability::StorageWrite,
        Capability::ViewRegister,
        Capability::MenuRegister,
        Capability::CommandRegister,
        Capability::NotificationsShow,
        Capability::LogsWrite,
        Capability::EventsSubscribe,
        Capability::Filesystem,
        Capability::Process,
        Capability::CoreMetadataWrite,
        Capability::Secret,
    ]
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_layout_model::WorkbenchZone;

    fn manifest(id: &str, required: &[&str], optional: &[&str]) -> ExtensionManifest {
        ExtensionManifest {
            id: ExtensionId(id.into()),
            required_capabilities: required.iter().map(|value| (*value).into()).collect(),
            optional_capabilities: optional.iter().map(|value| (*value).into()).collect(),
            ..ExtensionManifest::default()
        }
    }

    #[test]
    fn high_risk_capability_requires_user_grant_while_read_access_stays_compatible() {
        let mut broker = CapabilityBroker::default();
        let extension_id = ExtensionId("sample.reader".into());
        let mut manifest = manifest(&extension_id.0, &[], &["network"]);
        manifest.permission_sets = vec![velvet_plugin_api::ZonePermissionSet {
            zone: WorkbenchZone::Contextual,
            permissions: vec![Permission::AssetRead],
        }];
        broker
            .register(&manifest)
            .expect("manifest capabilities should register");

        let (snapshot, _) = broker
            .activate(&extension_id)
            .expect("optional denied capabilities must not block activation");
        assert_eq!(snapshot.granted, vec![Capability::AssetRead]);
        assert_eq!(snapshot.denied[0].capability, Capability::Network);
        assert_eq!(
            snapshot.denied[0].reason,
            CapabilityDenyReason::UserApprovalRequired
        );

        let generation = snapshot.generation;
        let snapshot = broker
            .grant(&extension_id, &[Capability::Network], Some(generation))
            .expect("explicit approval should grant network");
        assert!(snapshot.granted.contains(&Capability::Network));
    }

    #[test]
    fn required_denial_is_fail_closed_and_old_handles_are_invalidated() {
        let mut broker = CapabilityBroker::default();
        let extension_id = ExtensionId("sample.writer".into());
        broker
            .register(&manifest(&extension_id.0, &["metadata.write"], &[]))
            .expect("manifest capabilities should register");

        assert_eq!(
            broker.activate(&extension_id),
            Err(CapabilityBrokerError::RequiredDenied(
                Capability::MetadataWrite
            ))
        );
        let generation = broker.snapshot(&extension_id).unwrap().generation;
        broker
            .grant(
                &extension_id,
                &[Capability::MetadataWrite],
                Some(generation),
            )
            .expect("explicit approval should be accepted");
        let snapshot = broker
            .activate(&extension_id)
            .expect("activation should succeed")
            .0;
        let handle = broker
            .handle(&extension_id, &Capability::MetadataWrite)
            .expect("active capability should expose a handle");

        let snapshot = broker
            .revoke(
                &extension_id,
                &[Capability::MetadataWrite],
                Some(snapshot.generation),
            )
            .expect("revocation should update the grant generation");
        assert!(snapshot.granted.is_empty());
        assert_eq!(
            broker.check_handle(&extension_id, &handle),
            Err(CapabilityBrokerError::SessionMismatch)
        );
        assert_eq!(
            broker.activate(&extension_id),
            Err(CapabilityBrokerError::RequiredDenied(
                Capability::MetadataWrite
            ))
        );
    }

    #[test]
    fn strict_policy_and_host_policy_are_distinct_denials() {
        let extension_id = ExtensionId("sample.policy".into());
        let mut strict = CapabilityBroker::new(CapabilityPolicy::deny_by_default());
        strict
            .register(&manifest(&extension_id.0, &[], &["asset.read"]))
            .unwrap();
        let snapshot = strict.activate(&extension_id).unwrap().0;
        assert_eq!(
            snapshot.denied[0].reason,
            CapabilityDenyReason::UserApprovalRequired
        );

        let mut blocked = CapabilityBroker::new(
            CapabilityPolicy::legacy_compatible().with_host_allowed(Capability::AssetRead, false),
        );
        blocked
            .register(&manifest(&extension_id.0, &[], &["asset.read"]))
            .unwrap();
        let snapshot = blocked.activate(&extension_id).unwrap().0;
        assert_eq!(snapshot.denied[0].reason, CapabilityDenyReason::HostPolicy);
    }

    #[test]
    fn ui_zone_permissions_do_not_imply_process_filesystem_or_network() {
        let mut broker = CapabilityBroker::default();
        let extension_id = ExtensionId("sample.ui".into());
        let mut manifest = manifest(&extension_id.0, &[], &[]);
        manifest.permission_sets = vec![velvet_plugin_api::ZonePermissionSet {
            zone: WorkbenchZone::Contextual,
            permissions: vec![Permission::AssetRead],
        }];
        broker.register(&manifest).unwrap();
        let snapshot = broker.activate(&extension_id).unwrap().0;
        assert_eq!(snapshot.granted, vec![Capability::AssetRead]);
        assert!(!snapshot.requested.contains(&Capability::Network));
        assert!(!snapshot.requested.contains(&Capability::Process));
        assert!(!snapshot.requested.contains(&Capability::Filesystem));
    }

    #[test]
    fn audit_records_are_bounded_and_do_not_capture_paths_or_secrets() {
        let mut broker = CapabilityBroker::default();
        let extension_id = ExtensionId("sample.audit".into());
        broker
            .register(&manifest(&extension_id.0, &[], &["network"]))
            .unwrap();
        broker.activate(&extension_id).unwrap();
        let records = broker.drain_audit();
        assert!(!records.is_empty());
        assert!(records.iter().all(|record| {
            record.schema_version == CAPABILITY_AUDIT_SCHEMA_VERSION
                && record.extension_id == extension_id
                && record.occurred_at_ms > 0
        }));
        let encoded = serde_json::to_string(&records).unwrap();
        assert!(!encoded.contains("/"));
        assert!(!encoded.contains("secret"));
    }

    #[test]
    fn batch_grants_validate_before_mutating_any_capability() {
        let extension_id = ExtensionId("sample.batch".into());
        let mut broker = CapabilityBroker::new(
            CapabilityPolicy::legacy_compatible().with_host_allowed(Capability::Network, false),
        );
        broker
            .register(&manifest(&extension_id.0, &[], &["asset.read", "network"]))
            .unwrap();
        let error = broker.grant(
            &extension_id,
            &[Capability::AssetRead, Capability::Network],
            Some(0),
        );
        assert_eq!(
            error,
            Err(CapabilityBrokerError::HostPolicyDenied(Capability::Network))
        );
        assert!(broker.snapshot(&extension_id).unwrap().granted.is_empty());
    }

    #[test]
    fn policy_replacement_preserves_active_state_until_host_handles_invalidation() {
        let extension_id = ExtensionId("sample.policy-live".into());
        let mut broker = CapabilityBroker::default();
        let mut manifest = manifest(&extension_id.0, &[], &[]);
        manifest.permission_sets = vec![velvet_plugin_api::ZonePermissionSet {
            zone: WorkbenchZone::Contextual,
            permissions: vec![Permission::AssetRead],
        }];
        broker.register(&manifest).unwrap();
        broker.activate(&extension_id).unwrap();
        broker.set_policy(CapabilityPolicy::legacy_compatible());
        assert!(broker.snapshot(&extension_id).unwrap().active);
    }
}
