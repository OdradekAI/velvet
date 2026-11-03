use serde_json::Error as JsonError;
use velvet_layout_model::{InMemoryLayoutService, LayoutService, LayoutState};
use velvet_persistence::StateSnapshot;
use velvet_workbench_protocol::{
    ImportMigration, PortableAssetMetadata, PortableBundleScope, PortableBundleSection,
    ProfileState, SourceAvailability,
    ASSET_BUNDLE_SCHEMA_VERSION as PROTOCOL_ASSET_BUNDLE_SCHEMA_VERSION, PROFILE_SCHEMA_VERSION,
};

use crate::{BundleImportParticipant, BundleImportParticipantError};

pub const LAYOUT_BUNDLE_OWNER: &str = "layout";
pub const LAYOUT_BUNDLE_SCOPE: &str = "workspace";
pub const PROFILE_BUNDLE_OWNER: &str = "profile";
pub const PROFILE_BUNDLE_SCOPE: &str = "user";
pub const LAYOUT_BUNDLE_SCHEMA_VERSION: u16 = 1;
pub const ASSET_BUNDLE_OWNER: &str = "asset";
pub const ASSET_BUNDLE_SCOPE: &str = "metadata";
pub const ASSET_BUNDLE_SCHEMA_VERSION: u16 = PROTOCOL_ASSET_BUNDLE_SCHEMA_VERSION;

/// Imports the host-owned Profile section while preserving the monotonic
/// Profile revision used by optimistic transport commands.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProfileBundleParticipant;

impl BundleImportParticipant for ProfileBundleParticipant {
    fn owner_id(&self) -> &str {
        PROFILE_BUNDLE_OWNER
    }

    fn snapshot_scope(&self) -> &str {
        PROFILE_BUNDLE_SCOPE
    }

    fn bundle_scope(&self) -> PortableBundleScope {
        PortableBundleScope::Profile
    }

    fn schema_version(&self) -> u16 {
        PROFILE_SCHEMA_VERSION
    }

    fn import_payload(
        &self,
        section: &PortableBundleSection,
        existing: Option<&StateSnapshot>,
        _migration: Option<&ImportMigration>,
    ) -> Result<Vec<u8>, BundleImportParticipantError> {
        let mut state = decode_json::<ProfileState>(&section.payload, "profile")?;
        if state.schema_version != PROFILE_SCHEMA_VERSION || !super::valid_profile_state(&state) {
            return Err(BundleImportParticipantError::Invalid(
                "profile payload failed schema or locale validation".into(),
            ));
        }
        if let Some(existing_state) = existing
            .and_then(|snapshot| serde_json::from_slice::<ProfileState>(&snapshot.payload).ok())
        {
            state.revision = existing_state.revision.saturating_add(1);
            // Developer trust is a local authorization, not portable user
            // content. Keep the current Profile decision across imports so a
            // bundle cannot silently enable unsigned package installation.
            state.plugin_developer_mode = existing_state.plugin_developer_mode;
        } else {
            state.plugin_developer_mode = None;
        }
        serde_json::to_vec(&state).map_err(|error| {
            BundleImportParticipantError::Invalid(format!(
                "profile payload could not be encoded: {error}"
            ))
        })
    }
}

/// Imports the Workbench layout through the layout model's migration and
/// validation rules rather than copying arbitrary layout bytes into a snapshot.
#[derive(Clone, Copy, Debug, Default)]
pub struct LayoutBundleParticipant;

impl BundleImportParticipant for LayoutBundleParticipant {
    fn owner_id(&self) -> &str {
        LAYOUT_BUNDLE_OWNER
    }

    fn snapshot_scope(&self) -> &str {
        LAYOUT_BUNDLE_SCOPE
    }

    fn bundle_scope(&self) -> PortableBundleScope {
        PortableBundleScope::Workspace
    }

    fn schema_version(&self) -> u16 {
        LAYOUT_BUNDLE_SCHEMA_VERSION
    }

    fn import_payload(
        &self,
        section: &PortableBundleSection,
        existing: Option<&StateSnapshot>,
        _migration: Option<&ImportMigration>,
    ) -> Result<Vec<u8>, BundleImportParticipantError> {
        let state = decode_json::<LayoutState>(&section.payload, "layout")?;
        let mut state = InMemoryLayoutService::default()
            .migrate(state)
            .map_err(|error| {
                BundleImportParticipantError::Invalid(format!(
                    "layout payload failed migration: {error:?}"
                ))
            })?;
        if let Some(existing_revision) = existing
            .and_then(|snapshot| serde_json::from_slice::<LayoutState>(&snapshot.payload).ok())
            .map(|state| state.revision)
        {
            state.revision = existing_revision.saturating_add(1);
        }
        serde_json::to_vec(&state).map_err(|error| {
            BundleImportParticipantError::Invalid(format!(
                "layout payload could not be encoded: {error}"
            ))
        })
    }
}

/// Imports path-free Asset metadata without touching the source-location
/// registry. A source path, approved root or volume identity from another
/// machine is never a portable capability; local availability must be
/// re-established by the Asset source owner after import.
#[derive(Clone, Copy, Debug, Default)]
pub struct AssetBundleParticipant;

impl BundleImportParticipant for AssetBundleParticipant {
    fn owner_id(&self) -> &str {
        ASSET_BUNDLE_OWNER
    }

    fn snapshot_scope(&self) -> &str {
        ASSET_BUNDLE_SCOPE
    }

    fn bundle_scope(&self) -> PortableBundleScope {
        PortableBundleScope::Asset
    }

    fn schema_version(&self) -> u16 {
        ASSET_BUNDLE_SCHEMA_VERSION
    }

    fn import_payload(
        &self,
        section: &PortableBundleSection,
        _existing: Option<&StateSnapshot>,
        _migration: Option<&ImportMigration>,
    ) -> Result<Vec<u8>, BundleImportParticipantError> {
        let mut metadata =
            decode_json::<PortableAssetMetadata>(&section.payload, "asset metadata")?;
        metadata.validate().map_err(|error| {
            BundleImportParticipantError::Invalid(format!(
                "asset metadata failed schema validation: {error:?}"
            ))
        })?;

        // Availability is an observation of the exporting environment. Since
        // the bundle intentionally carries no path/root/volume identity, an
        // import must never claim that a local source is currently available.
        for asset in &mut metadata.assets {
            asset.source_availability = SourceAvailability::Unknown;
        }

        serde_json::to_vec(&metadata).map_err(|error| {
            BundleImportParticipantError::Invalid(format!(
                "asset metadata could not be encoded: {error}"
            ))
        })
    }
}

fn decode_json<T: serde::de::DeserializeOwned>(
    payload: &[u8],
    owner: &str,
) -> Result<T, BundleImportParticipantError> {
    serde_json::from_slice(payload).map_err(|error: JsonError| {
        BundleImportParticipantError::Invalid(format!("{owner} payload is not valid JSON: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use velvet_layout_model::LayoutState;

    #[test]
    fn profile_participant_validates_and_preserves_monotonic_revision() {
        let participant = ProfileBundleParticipant;
        let section = PortableBundleSection {
            section_id: "profile".into(),
            scope: PortableBundleScope::Profile,
            owner_id: PROFILE_BUNDLE_OWNER.into(),
            schema_version: PROFILE_SCHEMA_VERSION,
            required: true,
            extension_id: None,
            sha256: String::new(),
            payload: serde_json::to_vec(&ProfileState {
                schema_version: PROFILE_SCHEMA_VERSION,
                revision: 2,
                locale: "zh-Hans".into(),
                plugin_developer_mode: Some(true),
            })
            .expect("profile should encode"),
        };
        let existing = StateSnapshot::new(
            PROFILE_BUNDLE_OWNER,
            PROFILE_BUNDLE_SCOPE,
            PROFILE_SCHEMA_VERSION,
            1,
            1,
            serde_json::to_vec(&ProfileState {
                schema_version: PROFILE_SCHEMA_VERSION,
                revision: 9,
                locale: "en-US".into(),
                plugin_developer_mode: Some(false),
            })
            .expect("existing profile should encode"),
        );
        let payload = participant
            .import_payload(&section, Some(&existing), None)
            .expect("profile should import");
        let imported: ProfileState =
            serde_json::from_slice(&payload).expect("profile should decode");
        assert_eq!(imported.locale, "zh-Hans");
        assert_eq!(imported.revision, 10);
        assert_eq!(imported.plugin_developer_mode, Some(false));
    }

    #[test]
    fn profile_participant_rejects_invalid_locale() {
        let participant = ProfileBundleParticipant;
        let section = PortableBundleSection {
            section_id: "profile".into(),
            scope: PortableBundleScope::Profile,
            owner_id: PROFILE_BUNDLE_OWNER.into(),
            schema_version: PROFILE_SCHEMA_VERSION,
            required: true,
            extension_id: None,
            sha256: String::new(),
            payload: br#"{"schemaVersion":1,"revision":1,"locale":"../tmp"}"#.to_vec(),
        };
        assert!(matches!(
            participant.import_payload(&section, None, None),
            Err(BundleImportParticipantError::Invalid(_))
        ));
    }

    #[test]
    fn layout_participant_migrates_and_preserves_revision() {
        let participant = LayoutBundleParticipant;
        let state = LayoutState {
            revision: 3,
            ..LayoutState::default()
        };
        let section = PortableBundleSection {
            section_id: "layout".into(),
            scope: PortableBundleScope::Workspace,
            owner_id: LAYOUT_BUNDLE_OWNER.into(),
            schema_version: LAYOUT_BUNDLE_SCHEMA_VERSION,
            required: true,
            extension_id: None,
            sha256: String::new(),
            payload: serde_json::to_vec(&state).expect("layout should encode"),
        };
        let existing = StateSnapshot::new(
            LAYOUT_BUNDLE_OWNER,
            LAYOUT_BUNDLE_SCOPE,
            LAYOUT_BUNDLE_SCHEMA_VERSION,
            2,
            1,
            serde_json::to_vec(&LayoutState {
                revision: 12,
                ..LayoutState::default()
            })
            .expect("existing layout should encode"),
        );
        let payload = participant
            .import_payload(&section, Some(&existing), None)
            .expect("layout should import");
        let imported: LayoutState = serde_json::from_slice(&payload).expect("layout should decode");
        assert_eq!(imported.schema_version, 1);
        assert_eq!(imported.revision, 13);
    }

    #[test]
    fn layout_participant_rejects_future_schema() {
        let participant = LayoutBundleParticipant;
        let section = PortableBundleSection {
            section_id: "layout".into(),
            scope: PortableBundleScope::Workspace,
            owner_id: LAYOUT_BUNDLE_OWNER.into(),
            schema_version: LAYOUT_BUNDLE_SCHEMA_VERSION,
            required: true,
            extension_id: None,
            sha256: String::new(),
            payload: br#"{"schemaVersion":99}"#.to_vec(),
        };
        assert!(matches!(
            participant.import_payload(&section, None, None),
            Err(BundleImportParticipantError::Invalid(_))
        ));
    }

    #[test]
    fn asset_participant_rejects_paths_and_normalizes_foreign_availability() {
        let participant = AssetBundleParticipant;
        let section = PortableBundleSection {
            section_id: "asset-metadata".into(),
            scope: PortableBundleScope::Asset,
            owner_id: ASSET_BUNDLE_OWNER.into(),
            schema_version: ASSET_BUNDLE_SCHEMA_VERSION,
            required: true,
            extension_id: None,
            sha256: String::new(),
            payload: serde_json::to_vec(&PortableAssetMetadata {
                schema_version: ASSET_BUNDLE_SCHEMA_VERSION,
                assets: vec![velvet_workbench_protocol::PortableAssetRecord {
                    asset_id: "asset-1".into(),
                    content_kind: "text/plain".into(),
                    fingerprint: Some("sha256:asset-1".into()),
                    source_availability: SourceAvailability::Available,
                }],
            })
            .expect("asset metadata should encode"),
        };
        let payload = participant
            .import_payload(&section, None, None)
            .expect("asset metadata should import");
        let imported: PortableAssetMetadata =
            serde_json::from_slice(&payload).expect("asset metadata should decode");
        assert_eq!(
            imported.assets[0].source_availability,
            SourceAvailability::Unknown
        );

        let path_payload = br#"{
            "schemaVersion": 1,
            "assets": [{
                "assetId": "asset-1",
                "contentKind": "text/plain",
                "sourcePath": "/tmp/foreign"
            }]
        }"#
        .to_vec();
        let path_section = PortableBundleSection {
            payload: path_payload,
            ..section
        };
        assert!(matches!(
            participant.import_payload(&path_section, None, None),
            Err(BundleImportParticipantError::Invalid(_))
        ));
    }
}
