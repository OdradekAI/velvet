use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use velvet_persistence::{
    RecoverySetError, RecoverySetStore, RecoverySetTransaction, SnapshotError, StateSnapshot,
    StateSnapshotStore, CORE_RECOVERY_SET_ID, MAX_STATE_SNAPSHOT_BYTES,
};
use velvet_workbench_protocol::{
    ImportConflict, ImportMigration, ImportOwnerReceipt, ImportOwnerStatus, ImportPlan,
    ImportPlanError, ImportReceipt, ImportReceiptStatus, PortableBundle, PortableBundleError,
    PortableBundleScope, PortableBundleSection, IMPORT_PLAN_SCHEMA_VERSION,
    IMPORT_RECEIPT_SCHEMA_VERSION,
};

const DEFAULT_IMPORT_PLAN_TTL_MS: u64 = 5 * 60 * 1000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BundleInspectionError {
    Bundle(PortableBundleError),
    DigestMismatch {
        section_id: String,
    },
    UnknownSelection {
        section_id: String,
    },
    UnsupportedCoreSchema {
        owner_id: String,
        received: u16,
        supported: u16,
    },
    NoImportableSections,
    InvalidPlan(velvet_workbench_protocol::ImportPlanError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleInspectionContext {
    pub now_ms: u64,
    pub plan_ttl_ms: u64,
    pub supported_owner_schemas: BTreeMap<String, u16>,
    pub trusted_extensions: BTreeSet<String>,
}

impl Default for BundleInspectionContext {
    fn default() -> Self {
        Self {
            now_ms: 0,
            plan_ttl_ms: DEFAULT_IMPORT_PLAN_TTL_MS,
            supported_owner_schemas: BTreeMap::new(),
            trusted_extensions: BTreeSet::new(),
        }
    }
}

/// Read-only bundle inspection and plan generation.
///
/// This stage verifies the container and section integrity, applies only the
/// host's compatibility policy, and produces an expiring plan. It does not
/// decode owner payloads, publish snapshots, access paths, or run commands.
pub struct PortableBundleInspector;

impl PortableBundleInspector {
    pub fn inspect(
        bundle: &PortableBundle,
        plan_id: impl Into<String>,
        selected_section_ids: &[String],
        context: &BundleInspectionContext,
    ) -> Result<ImportPlan, BundleInspectionError> {
        bundle.validate().map_err(BundleInspectionError::Bundle)?;
        let by_id = bundle
            .sections
            .iter()
            .map(|section| (section.section_id.as_str(), section))
            .collect::<BTreeMap<_, _>>();
        let mut selected = Vec::new();
        let mut migrations = Vec::new();
        let mut conflicts = Vec::new();
        let mut skipped_extension_sections = Vec::new();
        let mut estimated_bytes = 0u64;

        for section_id in selected_section_ids {
            let Some(section) = by_id.get(section_id.as_str()) else {
                return Err(BundleInspectionError::UnknownSelection {
                    section_id: section_id.clone(),
                });
            };
            if matches!(section.scope, PortableBundleScope::Extension) {
                let extension_id = section.extension_id.as_deref().unwrap_or_default();
                let Some(supported_schema) = context.supported_owner_schemas.get(extension_id)
                else {
                    skipped_extension_sections.push(section.section_id.clone());
                    continue;
                };
                if !context.trusted_extensions.contains(extension_id) {
                    skipped_extension_sections.push(section.section_id.clone());
                    continue;
                }
                let digest = Sha256::digest(&section.payload);
                if encode_hex(&digest) != section.sha256.to_ascii_lowercase() {
                    skipped_extension_sections.push(section.section_id.clone());
                    conflicts.push(ImportConflict {
                        section_id: section.section_id.clone(),
                        reason: "extension section integrity check failed".into(),
                    });
                    continue;
                }
                if section.schema_version > *supported_schema {
                    skipped_extension_sections.push(section.section_id.clone());
                    conflicts.push(ImportConflict {
                        section_id: section.section_id.clone(),
                        reason: "extension schema is newer than the active host".into(),
                    });
                    continue;
                }
                if section.schema_version < *supported_schema {
                    migrations.push(ImportMigration {
                        section_id: section.section_id.clone(),
                        from_schema_version: section.schema_version,
                        to_schema_version: *supported_schema,
                    });
                }
            } else {
                let digest = Sha256::digest(&section.payload);
                if encode_hex(&digest) != section.sha256.to_ascii_lowercase() {
                    return Err(BundleInspectionError::DigestMismatch {
                        section_id: section.section_id.clone(),
                    });
                }
                let Some(supported_schema) = context
                    .supported_owner_schemas
                    .get(&section.owner_id)
                    .copied()
                else {
                    return Err(BundleInspectionError::UnsupportedCoreSchema {
                        owner_id: section.owner_id.clone(),
                        received: section.schema_version,
                        supported: 0,
                    });
                };
                if section.schema_version > supported_schema {
                    return Err(BundleInspectionError::UnsupportedCoreSchema {
                        owner_id: section.owner_id.clone(),
                        received: section.schema_version,
                        supported: supported_schema,
                    });
                }
                if section.schema_version < supported_schema {
                    migrations.push(ImportMigration {
                        section_id: section.section_id.clone(),
                        from_schema_version: section.schema_version,
                        to_schema_version: supported_schema,
                    });
                }
            }

            selected.push(section.section_id.clone());
            estimated_bytes = estimated_bytes.saturating_add(section.payload.len() as u64);
        }

        if selected.is_empty() {
            return Err(BundleInspectionError::NoImportableSections);
        }
        let plan = ImportPlan {
            schema_version: IMPORT_PLAN_SCHEMA_VERSION,
            plan_id: plan_id.into(),
            bundle_id: bundle.bundle_id.clone(),
            created_at_ms: context.now_ms,
            expires_at_ms: context.now_ms.saturating_add(if context.plan_ttl_ms == 0 {
                DEFAULT_IMPORT_PLAN_TTL_MS
            } else {
                context.plan_ttl_ms
            }),
            selected_section_ids: selected,
            migrations,
            conflicts,
            unavailable_sources: Vec::new(),
            skipped_extension_sections,
            estimated_bytes,
        };
        plan.validate(context.now_ms)
            .map_err(BundleInspectionError::InvalidPlan)?;
        Ok(plan)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BundleImportParticipantError {
    Invalid(String),
    Conflict(String),
    Unavailable(String),
}

/// Owner-specific import seam. A participant decodes and migrates its own
/// payload, while the host supplies the current snapshot and controls the
/// snapshot metadata, staging and RecoverySet publication.
pub trait BundleImportParticipant: Send + Sync {
    fn owner_id(&self) -> &str;
    fn snapshot_scope(&self) -> &str;
    fn bundle_scope(&self) -> PortableBundleScope;
    fn schema_version(&self) -> u16;
    fn import_payload(
        &self,
        section: &PortableBundleSection,
        existing: Option<&StateSnapshot>,
        migration: Option<&ImportMigration>,
    ) -> Result<Vec<u8>, BundleImportParticipantError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BundleImportError {
    Bundle(PortableBundleError),
    Plan(ImportPlanError),
    BundleIdMismatch,
    UnknownSelection {
        section_id: String,
    },
    DigestMismatch {
        section_id: String,
    },
    DuplicateOwner {
        owner_id: String,
        scope: String,
    },
    MissingParticipant {
        owner_id: String,
        scope: String,
    },
    UnsupportedCoreSchema {
        owner_id: String,
        received: u16,
        supported: u16,
    },
    Snapshot(SnapshotError),
    RecoverySet(RecoverySetError),
    Participant {
        owner_id: String,
        scope: String,
        error: BundleImportParticipantError,
    },
    PayloadTooLarge {
        owner_id: String,
        scope: String,
    },
}

struct PreparedSnapshot {
    owner_id: String,
    scope: String,
    snapshot: StateSnapshot,
}

/// Publishes selected core owner sections through the existing snapshot and
/// RecoverySet seams. Extensions are independent: their participant output is
/// committed after the core pointer and a failure becomes a quarantined
/// receipt entry instead of invalidating the selected core import.
pub struct PortableBundleImporter<'a> {
    snapshots: &'a dyn StateSnapshotStore,
    recovery_sets: &'a dyn RecoverySetStore,
}

impl<'a> PortableBundleImporter<'a> {
    pub fn new(
        snapshots: &'a dyn StateSnapshotStore,
        recovery_sets: &'a dyn RecoverySetStore,
    ) -> Self {
        Self {
            snapshots,
            recovery_sets,
        }
    }

    pub fn import(
        &self,
        bundle: &PortableBundle,
        plan: &ImportPlan,
        participants: &[&dyn BundleImportParticipant],
        now_ms: u64,
    ) -> Result<ImportReceipt, BundleImportError> {
        bundle.validate().map_err(BundleImportError::Bundle)?;
        plan.validate(now_ms).map_err(BundleImportError::Plan)?;
        if plan.bundle_id != bundle.bundle_id {
            return Err(BundleImportError::BundleIdMismatch);
        }
        let sections = bundle
            .sections
            .iter()
            .map(|section| (section.section_id.as_str(), section))
            .collect::<BTreeMap<_, _>>();
        let migrations = plan
            .migrations
            .iter()
            .map(|migration| (migration.section_id.as_str(), migration))
            .collect::<BTreeMap<_, _>>();
        let mut seen_owners = BTreeSet::new();
        let mut core_snapshots = Vec::new();
        let mut extension_snapshots = Vec::new();
        let mut owner_receipts = Vec::new();

        for section_id in &plan.selected_section_ids {
            let Some(section) = sections.get(section_id.as_str()) else {
                return Err(BundleImportError::UnknownSelection {
                    section_id: section_id.clone(),
                });
            };
            let is_extension = matches!(section.scope, PortableBundleScope::Extension);
            let participant = participants
                .iter()
                .find(|participant| {
                    participant.owner_id() == section.owner_id
                        && participant.bundle_scope() == section.scope.clone()
                })
                .copied();

            if !is_extension {
                let digest = Sha256::digest(&section.payload);
                if encode_hex(&digest) != section.sha256.to_ascii_lowercase() {
                    return Err(BundleImportError::DigestMismatch {
                        section_id: section.section_id.clone(),
                    });
                }
                let Some(participant) = participant else {
                    return Err(BundleImportError::MissingParticipant {
                        owner_id: section.owner_id.clone(),
                        scope: bundle_scope_label(&section.scope).into(),
                    });
                };
                let owner_scope = (
                    participant.owner_id().to_owned(),
                    participant.snapshot_scope().to_owned(),
                );
                if !seen_owners.insert(owner_scope.clone()) {
                    return Err(BundleImportError::DuplicateOwner {
                        owner_id: owner_scope.0,
                        scope: owner_scope.1,
                    });
                }
                let supported_schema = participant.schema_version();
                if section.schema_version > supported_schema {
                    return Err(BundleImportError::UnsupportedCoreSchema {
                        owner_id: section.owner_id.clone(),
                        received: section.schema_version,
                        supported: supported_schema,
                    });
                }
                let existing = self
                    .snapshots
                    .load_latest(participant.owner_id(), participant.snapshot_scope())
                    .map_err(BundleImportError::Snapshot)?;
                let payload = participant
                    .import_payload(
                        section,
                        existing.as_ref(),
                        migrations.get(section_id.as_str()).copied(),
                    )
                    .map_err(|error| BundleImportError::Participant {
                        owner_id: participant.owner_id().into(),
                        scope: participant.snapshot_scope().into(),
                        error,
                    })?;
                core_snapshots.push(PreparedSnapshot {
                    owner_id: participant.owner_id().into(),
                    scope: participant.snapshot_scope().into(),
                    snapshot: make_snapshot(participant, existing.as_ref(), now_ms, payload)?,
                });
                continue;
            }

            let Some(participant) = participant else {
                owner_receipts.push(skipped_extension_receipt(
                    section,
                    "extension participant is unavailable",
                ));
                continue;
            };
            if !verify_digest(section) {
                owner_receipts.push(ImportOwnerReceipt {
                    owner_id: participant.owner_id().into(),
                    scope: participant.snapshot_scope().into(),
                    status: ImportOwnerStatus::Quarantined,
                    generation: None,
                    message: Some("extension section integrity check failed".into()),
                });
                continue;
            }
            if section.schema_version > participant.schema_version() {
                owner_receipts.push(ImportOwnerReceipt {
                    owner_id: participant.owner_id().into(),
                    scope: participant.snapshot_scope().into(),
                    status: ImportOwnerStatus::Quarantined,
                    generation: None,
                    message: Some("extension schema is newer than the participant".into()),
                });
                continue;
            }
            let owner_scope = (
                participant.owner_id().to_owned(),
                participant.snapshot_scope().to_owned(),
            );
            if !seen_owners.insert(owner_scope.clone()) {
                return Err(BundleImportError::DuplicateOwner {
                    owner_id: owner_scope.0,
                    scope: owner_scope.1,
                });
            }
            let existing = match self
                .snapshots
                .load_latest(participant.owner_id(), participant.snapshot_scope())
            {
                Ok(existing) => existing,
                Err(error) => {
                    owner_receipts.push(ImportOwnerReceipt {
                        owner_id: participant.owner_id().into(),
                        scope: participant.snapshot_scope().into(),
                        status: ImportOwnerStatus::Quarantined,
                        generation: None,
                        message: Some(format!("extension state could not be loaded: {error:?}")),
                    });
                    continue;
                }
            };
            let payload = match participant.import_payload(
                section,
                existing.as_ref(),
                migrations.get(section_id.as_str()).copied(),
            ) {
                Ok(payload) => payload,
                Err(error) => {
                    owner_receipts.push(ImportOwnerReceipt {
                        owner_id: participant.owner_id().into(),
                        scope: participant.snapshot_scope().into(),
                        status: ImportOwnerStatus::Quarantined,
                        generation: None,
                        message: Some(format!("extension import failed: {error:?}")),
                    });
                    continue;
                }
            };
            match make_snapshot(participant, existing.as_ref(), now_ms, payload) {
                Ok(snapshot) => extension_snapshots.push(PreparedSnapshot {
                    owner_id: participant.owner_id().into(),
                    scope: participant.snapshot_scope().into(),
                    snapshot,
                }),
                Err(BundleImportError::PayloadTooLarge { .. }) => {
                    owner_receipts.push(ImportOwnerReceipt {
                        owner_id: participant.owner_id().into(),
                        scope: participant.snapshot_scope().into(),
                        status: ImportOwnerStatus::Quarantined,
                        generation: None,
                        message: Some("extension payload exceeds snapshot limit".into()),
                    });
                }
                Err(error) => return Err(error),
            }
        }

        let recovery_set = if core_snapshots.is_empty() {
            None
        } else {
            let current = self
                .recovery_sets
                .load_latest(CORE_RECOVERY_SET_ID)
                .map_err(BundleImportError::RecoverySet)?
                .ok_or(BundleImportError::RecoverySet(RecoverySetError::NotFound))?;
            let scopes = current
                .members
                .values()
                .map(|member| (member.owner_id.as_str(), member.scope.as_str()))
                .collect::<Vec<_>>();
            for prepared in &core_snapshots {
                if !current
                    .members
                    .contains_key(&format!("{}:{}", prepared.owner_id, prepared.scope))
                {
                    return Err(BundleImportError::RecoverySet(
                        RecoverySetError::Incomplete(format!(
                            "owner {}:{} is not in the current recovery set",
                            prepared.owner_id, prepared.scope
                        )),
                    ));
                }
            }
            let mut transaction = RecoverySetTransaction::new(
                self.snapshots,
                self.recovery_sets,
                CORE_RECOVERY_SET_ID,
                now_ms,
                &scopes,
            )
            .map_err(BundleImportError::RecoverySet)?;
            for prepared in &core_snapshots {
                transaction
                    .stage(&prepared.snapshot)
                    .map_err(BundleImportError::RecoverySet)?;
            }
            let committed = transaction
                .commit()
                .map_err(BundleImportError::RecoverySet)?;
            for prepared in &core_snapshots {
                owner_receipts.push(ImportOwnerReceipt {
                    owner_id: prepared.owner_id.clone(),
                    scope: prepared.scope.clone(),
                    status: ImportOwnerStatus::Committed,
                    generation: Some(prepared.snapshot.metadata.generation),
                    message: None,
                });
            }
            Some(committed)
        };

        for prepared in extension_snapshots {
            let owner_id = prepared.owner_id.clone();
            let scope = prepared.scope.clone();
            let generation = prepared.snapshot.metadata.generation;
            let result = self
                .snapshots
                .stage(&prepared.snapshot)
                .and_then(|_| self.snapshots.commit(&owner_id, &scope, generation));
            owner_receipts.push(match result {
                Ok(()) => ImportOwnerReceipt {
                    owner_id,
                    scope,
                    status: ImportOwnerStatus::Committed,
                    generation: Some(generation),
                    message: None,
                },
                Err(error) => ImportOwnerReceipt {
                    owner_id,
                    scope,
                    status: ImportOwnerStatus::Quarantined,
                    generation: None,
                    message: Some(format!(
                        "extension snapshot could not be committed: {error:?}"
                    )),
                },
            });
        }

        let status = if owner_receipts
            .iter()
            .any(|owner| !matches!(owner.status, ImportOwnerStatus::Committed))
        {
            ImportReceiptStatus::Partial
        } else {
            ImportReceiptStatus::Committed
        };
        Ok(ImportReceipt {
            schema_version: IMPORT_RECEIPT_SCHEMA_VERSION,
            plan_id: plan.plan_id.clone(),
            bundle_id: plan.bundle_id.clone(),
            status,
            recorded_at_ms: now_ms,
            recovery_set_generation: recovery_set.map(|set| set.generation),
            owners: owner_receipts,
        })
    }
}

fn make_snapshot(
    participant: &dyn BundleImportParticipant,
    existing: Option<&StateSnapshot>,
    now_ms: u64,
    payload: Vec<u8>,
) -> Result<StateSnapshot, BundleImportError> {
    if payload.len() > MAX_STATE_SNAPSHOT_BYTES {
        return Err(BundleImportError::PayloadTooLarge {
            owner_id: participant.owner_id().into(),
            scope: participant.snapshot_scope().into(),
        });
    }
    let generation = existing
        .map(|snapshot| snapshot.metadata.generation.saturating_add(1))
        .unwrap_or(1)
        .max(1);
    Ok(StateSnapshot::new(
        participant.owner_id(),
        participant.snapshot_scope(),
        participant.schema_version(),
        generation,
        now_ms,
        payload,
    ))
}

fn verify_digest(section: &PortableBundleSection) -> bool {
    let digest = Sha256::digest(&section.payload);
    encode_hex(&digest) == section.sha256.to_ascii_lowercase()
}

fn skipped_extension_receipt(section: &PortableBundleSection, message: &str) -> ImportOwnerReceipt {
    ImportOwnerReceipt {
        owner_id: section
            .extension_id
            .clone()
            .unwrap_or_else(|| section.owner_id.clone()),
        scope: "extension".into(),
        status: ImportOwnerStatus::Skipped,
        generation: None,
        message: Some(message.into()),
    }
}

fn bundle_scope_label(scope: &PortableBundleScope) -> &'static str {
    match scope {
        PortableBundleScope::Profile => "profile",
        PortableBundleScope::Workspace => "workspace",
        PortableBundleScope::Asset => "asset",
        PortableBundleScope::Extension => "extension",
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_persistence::{
        commit_current_recovery_set, InMemoryRecoverySetStore, InMemoryStateSnapshotStore,
        StateSnapshotStore, CORE_RECOVERY_SET_ID, JOB_SNAPSHOT_OWNER, JOB_SNAPSHOT_SCOPE,
        LAYOUT_SNAPSHOT_OWNER, LAYOUT_SNAPSHOT_SCOPE, PROFILE_SNAPSHOT_OWNER,
        PROFILE_SNAPSHOT_SCOPE,
    };
    use velvet_workbench_protocol::{PortableBundleSection, PORTABLE_BUNDLE_SCHEMA_VERSION};

    struct FakeParticipant {
        owner_id: &'static str,
        snapshot_scope: &'static str,
        bundle_scope: PortableBundleScope,
        schema_version: u16,
        failure: Option<BundleImportParticipantError>,
    }

    impl BundleImportParticipant for FakeParticipant {
        fn owner_id(&self) -> &str {
            self.owner_id
        }

        fn snapshot_scope(&self) -> &str {
            self.snapshot_scope
        }

        fn bundle_scope(&self) -> PortableBundleScope {
            self.bundle_scope.clone()
        }

        fn schema_version(&self) -> u16 {
            self.schema_version
        }

        fn import_payload(
            &self,
            section: &PortableBundleSection,
            _existing: Option<&StateSnapshot>,
            _migration: Option<&ImportMigration>,
        ) -> Result<Vec<u8>, BundleImportParticipantError> {
            self.failure
                .clone()
                .map_or_else(|| Ok(section.payload.clone()), Err)
        }
    }

    fn section(
        section_id: &str,
        scope: PortableBundleScope,
        owner_id: &str,
        schema_version: u16,
        payload: &[u8],
        extension_id: Option<&str>,
        required: bool,
    ) -> PortableBundleSection {
        PortableBundleSection {
            section_id: section_id.into(),
            scope,
            owner_id: owner_id.into(),
            schema_version,
            required,
            extension_id: extension_id.map(str::to_owned),
            sha256: encode_hex(&Sha256::digest(payload)),
            payload: payload.to_vec(),
        }
    }

    fn bundle() -> PortableBundle {
        PortableBundle {
            schema_version: PORTABLE_BUNDLE_SCHEMA_VERSION,
            bundle_id: "bundle-1".into(),
            created_at_ms: 1,
            source_build: "0.0.4".into(),
            sections: vec![
                section(
                    "profile",
                    PortableBundleScope::Profile,
                    "profile",
                    1,
                    br#"{"locale":"zh-Hans"}"#,
                    None,
                    true,
                ),
                section(
                    "extension-sample",
                    PortableBundleScope::Extension,
                    "sample.extension",
                    1,
                    br#"{"settings":{}}"#,
                    Some("sample.extension"),
                    false,
                ),
            ],
        }
    }

    fn context() -> BundleInspectionContext {
        BundleInspectionContext {
            now_ms: 10,
            plan_ttl_ms: 100,
            supported_owner_schemas: BTreeMap::from([
                ("profile".into(), 2),
                ("sample.extension".into(), 1),
            ]),
            trusted_extensions: BTreeSet::from(["sample.extension".into()]),
        }
    }

    fn seed_core_recovery_set(
        snapshots: &InMemoryStateSnapshotStore,
        recovery_sets: &InMemoryRecoverySetStore,
    ) {
        for (owner_id, scope, payload) in [
            (
                LAYOUT_SNAPSHOT_OWNER,
                LAYOUT_SNAPSHOT_SCOPE,
                b"layout-v1".to_vec(),
            ),
            (
                PROFILE_SNAPSHOT_OWNER,
                PROFILE_SNAPSHOT_SCOPE,
                b"profile-v1".to_vec(),
            ),
            (JOB_SNAPSHOT_OWNER, JOB_SNAPSHOT_SCOPE, b"jobs-v1".to_vec()),
        ] {
            let snapshot = StateSnapshot::new(owner_id, scope, 1, 1, 1, payload);
            snapshots.stage(&snapshot).expect("seed should stage");
            snapshots
                .commit(owner_id, scope, 1)
                .expect("seed should commit");
        }
        commit_current_recovery_set(
            snapshots,
            recovery_sets,
            CORE_RECOVERY_SET_ID,
            1,
            &[
                (LAYOUT_SNAPSHOT_OWNER, LAYOUT_SNAPSHOT_SCOPE),
                (PROFILE_SNAPSHOT_OWNER, PROFILE_SNAPSHOT_SCOPE),
                (JOB_SNAPSHOT_OWNER, JOB_SNAPSHOT_SCOPE),
            ],
        )
        .expect("seed recovery set should commit");
    }

    #[test]
    fn inspection_verifies_digest_and_emits_expiring_migration_plan() {
        let bundle = bundle();
        let plan = PortableBundleInspector::inspect(
            &bundle,
            "import-1",
            &["profile".into(), "extension-sample".into()],
            &context(),
        )
        .expect("bundle should produce a plan");
        assert_eq!(
            plan.selected_section_ids,
            vec!["profile", "extension-sample"]
        );
        assert_eq!(plan.estimated_bytes, 35);
        assert_eq!(plan.expires_at_ms, 110);
        assert_eq!(plan.migrations.len(), 1);
        assert_eq!(plan.migrations[0].section_id, "profile");
    }

    #[test]
    fn digest_mismatch_blocks_plan_before_owner_decode() {
        let mut bundle = bundle();
        bundle.sections[0].payload = br#"{"locale":"en-US"}"#.to_vec();
        assert_eq!(
            PortableBundleInspector::inspect(&bundle, "import-1", &["profile".into()], &context(),),
            Err(BundleInspectionError::DigestMismatch {
                section_id: "profile".into()
            })
        );
    }

    #[test]
    fn untrusted_extension_is_skipped_without_blocking_core_import() {
        let mut context = context();
        context.trusted_extensions.clear();
        let plan = PortableBundleInspector::inspect(
            &bundle(),
            "import-1",
            &["profile".into(), "extension-sample".into()],
            &context,
        )
        .expect("core section should remain importable");
        assert_eq!(plan.selected_section_ids, vec!["profile"]);
        assert_eq!(plan.skipped_extension_sections, vec!["extension-sample"]);
    }

    #[test]
    fn corrupt_extension_is_quarantined_without_blocking_core_import() {
        let mut bundle = bundle();
        bundle.sections[1].payload = b"corrupt".to_vec();
        let plan = PortableBundleInspector::inspect(
            &bundle,
            "import-1",
            &["profile".into(), "extension-sample".into()],
            &context(),
        )
        .expect("core section should remain importable");
        assert_eq!(plan.selected_section_ids, vec!["profile"]);
        assert_eq!(plan.skipped_extension_sections, vec!["extension-sample"]);
        assert_eq!(plan.conflicts[0].section_id, "extension-sample");
    }

    #[test]
    fn newer_core_schema_is_a_hard_failure_and_unknown_selection_is_typed() {
        let mut newer_bundle = bundle();
        newer_bundle.sections[0].schema_version = 3;
        assert!(matches!(
            PortableBundleInspector::inspect(
                &newer_bundle,
                "import-1",
                &["profile".into()],
                &context(),
            ),
            Err(BundleInspectionError::UnsupportedCoreSchema {
                owner_id,
                received: 3,
                supported: 2,
            }) if owner_id == "profile"
        ));
        assert_eq!(
            PortableBundleInspector::inspect(
                &bundle(),
                "import-1",
                &["missing".into()],
                &context(),
            ),
            Err(BundleInspectionError::UnknownSelection {
                section_id: "missing".into()
            })
        );
    }

    #[test]
    fn importer_stages_core_payload_and_publishes_one_recovery_set_generation() {
        let snapshots = InMemoryStateSnapshotStore::default();
        let recovery_sets = InMemoryRecoverySetStore::default();
        seed_core_recovery_set(&snapshots, &recovery_sets);
        let bundle = bundle();
        let plan =
            PortableBundleInspector::inspect(&bundle, "import-1", &["profile".into()], &context())
                .expect("plan should be valid");
        let profile = FakeParticipant {
            owner_id: PROFILE_SNAPSHOT_OWNER,
            snapshot_scope: PROFILE_SNAPSHOT_SCOPE,
            bundle_scope: PortableBundleScope::Profile,
            schema_version: 2,
            failure: None,
        };
        let receipt = PortableBundleImporter::new(&snapshots, &recovery_sets)
            .import(&bundle, &plan, &[&profile], 20)
            .expect("core import should commit");
        assert_eq!(receipt.status, ImportReceiptStatus::Committed);
        assert_eq!(receipt.recovery_set_generation, Some(2));
        assert_eq!(receipt.owners[0].generation, Some(2));
        assert_eq!(
            snapshots
                .load_latest(PROFILE_SNAPSHOT_OWNER, PROFILE_SNAPSHOT_SCOPE)
                .expect("profile should load")
                .expect("profile should exist")
                .payload,
            br#"{"locale":"zh-Hans"}"#
        );
        assert_eq!(
            snapshots
                .load_latest(LAYOUT_SNAPSHOT_OWNER, LAYOUT_SNAPSHOT_SCOPE)
                .expect("layout should load")
                .expect("layout should exist")
                .metadata
                .generation,
            1
        );
    }

    #[test]
    fn failed_core_participant_does_not_publish_a_new_recovery_set() {
        let snapshots = InMemoryStateSnapshotStore::default();
        let recovery_sets = InMemoryRecoverySetStore::default();
        seed_core_recovery_set(&snapshots, &recovery_sets);
        let bundle = bundle();
        let plan =
            PortableBundleInspector::inspect(&bundle, "import-1", &["profile".into()], &context())
                .expect("plan should be valid");
        let profile = FakeParticipant {
            owner_id: PROFILE_SNAPSHOT_OWNER,
            snapshot_scope: PROFILE_SNAPSHOT_SCOPE,
            bundle_scope: PortableBundleScope::Profile,
            schema_version: 2,
            failure: Some(BundleImportParticipantError::Conflict(
                "profile revision changed".into(),
            )),
        };
        assert!(matches!(
            PortableBundleImporter::new(&snapshots, &recovery_sets).import(
                &bundle,
                &plan,
                &[&profile],
                20
            ),
            Err(BundleImportError::Participant { .. })
        ));
        assert_eq!(
            recovery_sets
                .load_latest(CORE_RECOVERY_SET_ID)
                .expect("recovery set should load")
                .expect("recovery set should exist")
                .generation,
            1
        );
        assert_eq!(
            snapshots
                .load_latest(PROFILE_SNAPSHOT_OWNER, PROFILE_SNAPSHOT_SCOPE)
                .expect("profile should load")
                .expect("profile should exist")
                .metadata
                .generation,
            1
        );
    }

    #[test]
    fn extension_participant_failure_is_partial_after_core_commit() {
        let snapshots = InMemoryStateSnapshotStore::default();
        let recovery_sets = InMemoryRecoverySetStore::default();
        seed_core_recovery_set(&snapshots, &recovery_sets);
        let bundle = bundle();
        let plan = PortableBundleInspector::inspect(
            &bundle,
            "import-1",
            &["profile".into(), "extension-sample".into()],
            &context(),
        )
        .expect("plan should be valid");
        let profile = FakeParticipant {
            owner_id: PROFILE_SNAPSHOT_OWNER,
            snapshot_scope: PROFILE_SNAPSHOT_SCOPE,
            bundle_scope: PortableBundleScope::Profile,
            schema_version: 2,
            failure: None,
        };
        let extension = FakeParticipant {
            owner_id: "sample.extension",
            snapshot_scope: "storage",
            bundle_scope: PortableBundleScope::Extension,
            schema_version: 1,
            failure: Some(BundleImportParticipantError::Unavailable(
                "extension is disabled".into(),
            )),
        };
        let receipt = PortableBundleImporter::new(&snapshots, &recovery_sets)
            .import(&bundle, &plan, &[&profile, &extension], 20)
            .expect("core import should survive extension failure");
        assert_eq!(receipt.status, ImportReceiptStatus::Partial);
        assert_eq!(receipt.recovery_set_generation, Some(2));
        assert!(receipt
            .owners
            .iter()
            .any(|owner| owner.owner_id == "sample.extension"
                && owner.status == ImportOwnerStatus::Quarantined));
    }
}
