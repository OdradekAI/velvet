use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use velvet_workbench_protocol::{
    ImportConflict, ImportMigration, ImportPlan, PortableBundle, PortableBundleError,
    PortableBundleScope, IMPORT_PLAN_SCHEMA_VERSION,
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
    use velvet_workbench_protocol::{PortableBundleSection, PORTABLE_BUNDLE_SCHEMA_VERSION};

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
}
