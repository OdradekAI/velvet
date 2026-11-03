use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use velvet_persistence::DurabilityMode;

use super::{
    is_safe_extension_id, PluginTrustRecord, PluginTrustState, PLUGIN_TRUST_RECORD_SCHEMA_VERSION,
};

pub const PLUGIN_TRUST_STORE_SCHEMA_VERSION: u16 = 1;
pub const MAX_PLUGIN_TRUST_RECORDS: usize = 256;
pub const MAX_PLUGIN_TRUST_STORE_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginTrustStoreError {
    Unavailable(String),
    Corrupt(String),
    Invalid(String),
}

pub trait PluginTrustStore: Send + Sync {
    fn load(&self) -> Result<Vec<PluginTrustRecord>, PluginTrustStoreError>;
    fn save(&self, records: &[PluginTrustRecord]) -> Result<(), PluginTrustStoreError>;
}

#[derive(Default)]
pub struct InMemoryPluginTrustStore {
    records: Mutex<BTreeMap<super::ExtensionId, PluginTrustRecord>>,
}

impl InMemoryPluginTrustStore {
    pub fn records(&self) -> Result<Vec<PluginTrustRecord>, PluginTrustStoreError> {
        self.load()
    }
}

impl PluginTrustStore for InMemoryPluginTrustStore {
    fn load(&self) -> Result<Vec<PluginTrustRecord>, PluginTrustStoreError> {
        self.records
            .lock()
            .map(|records| records.values().cloned().collect())
            .map_err(|_| PluginTrustStoreError::Unavailable("trust store lock poisoned".into()))
    }

    fn save(&self, records: &[PluginTrustRecord]) -> Result<(), PluginTrustStoreError> {
        validate_records(records)?;
        let mut next = BTreeMap::new();
        for record in records {
            next.insert(record.extension_id.clone(), record.clone());
        }
        self.records
            .lock()
            .map(|mut current| *current = next)
            .map_err(|_| PluginTrustStoreError::Unavailable("trust store lock poisoned".into()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFilePluginTrustStore {
    path: PathBuf,
    durability: DurabilityMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PluginTrustStoreState {
    schema_version: u16,
    records: Vec<PluginTrustRecord>,
}

impl JsonFilePluginTrustStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_durability(path, DurabilityMode::AtomicRenameOnly)
    }

    pub fn with_durability(path: impl Into<PathBuf>, durability: DurabilityMode) -> Self {
        Self {
            path: path.into(),
            durability,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read_state(&self) -> Result<PluginTrustStoreState, PluginTrustStoreError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(PluginTrustStoreState {
                    schema_version: PLUGIN_TRUST_STORE_SCHEMA_VERSION,
                    records: Vec::new(),
                })
            }
            Err(error) => return Err(PluginTrustStoreError::Unavailable(error.to_string())),
        };
        if metadata.len() > MAX_PLUGIN_TRUST_STORE_BYTES {
            return Err(PluginTrustStoreError::Corrupt(
                "plugin trust store exceeds its size limit".into(),
            ));
        }
        let bytes = fs::read(&self.path)
            .map_err(|error| PluginTrustStoreError::Unavailable(error.to_string()))?;
        let state = serde_json::from_slice::<PluginTrustStoreState>(&bytes)
            .map_err(|error| PluginTrustStoreError::Corrupt(error.to_string()))?;
        validate_state(&state)?;
        Ok(state)
    }

    fn write_state(&self, state: &PluginTrustStoreState) -> Result<(), PluginTrustStoreError> {
        validate_state(state)?;
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|error| PluginTrustStoreError::Corrupt(error.to_string()))?;
        if bytes.len() as u64 > MAX_PLUGIN_TRUST_STORE_BYTES {
            return Err(PluginTrustStoreError::Invalid(
                "plugin trust store exceeds its size limit".into(),
            ));
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| PluginTrustStoreError::Unavailable(error.to_string()))?;
        }
        static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin-trust.json");
        let temporary = self.path.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        fs::write(&temporary, bytes)
            .map_err(|error| PluginTrustStoreError::Unavailable(error.to_string()))?;
        if self.durability == DurabilityMode::RequireFsync {
            File::open(&temporary)
                .and_then(|file| file.sync_all())
                .map_err(|error| PluginTrustStoreError::Unavailable(error.to_string()))?;
        }
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(PluginTrustStoreError::Unavailable(error.to_string()));
        }
        if self.durability == DurabilityMode::RequireFsync {
            let parent = self.path.parent().ok_or_else(|| {
                PluginTrustStoreError::Unavailable("trust path has no parent".into())
            })?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| PluginTrustStoreError::Unavailable(error.to_string()))?;
        }
        Ok(())
    }
}

impl PluginTrustStore for JsonFilePluginTrustStore {
    fn load(&self) -> Result<Vec<PluginTrustRecord>, PluginTrustStoreError> {
        Ok(self.read_state()?.records)
    }

    fn save(&self, records: &[PluginTrustRecord]) -> Result<(), PluginTrustStoreError> {
        self.write_state(&PluginTrustStoreState {
            schema_version: PLUGIN_TRUST_STORE_SCHEMA_VERSION,
            records: records.to_vec(),
        })
    }
}

pub(crate) fn validate_records(records: &[PluginTrustRecord]) -> Result<(), PluginTrustStoreError> {
    if records.len() > MAX_PLUGIN_TRUST_RECORDS {
        return Err(PluginTrustStoreError::Invalid(format!(
            "plugin trust record limit has been exceeded: {} > {}",
            records.len(),
            MAX_PLUGIN_TRUST_RECORDS
        )));
    }
    let mut ids = BTreeMap::new();
    for record in records {
        if record.schema_version != PLUGIN_TRUST_RECORD_SCHEMA_VERSION {
            return Err(PluginTrustStoreError::Invalid(format!(
                "trust record for {} uses unsupported schema {}",
                record.extension_id.0, record.schema_version
            )));
        }
        if !is_safe_extension_id(&record.extension_id.0) {
            return Err(PluginTrustStoreError::Invalid(format!(
                "trust record extension id {} is invalid",
                record.extension_id.0
            )));
        }
        if record.version.is_empty() || record.version.len() > 256 {
            return Err(PluginTrustStoreError::Invalid(format!(
                "trust record for {} has an invalid version",
                record.extension_id.0
            )));
        }
        if record.package_sha256.len() != 64
            || !record
                .package_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(PluginTrustStoreError::Invalid(format!(
                "trust record for {} has an invalid package digest",
                record.extension_id.0
            )));
        }
        if record.state == PluginTrustState::DeveloperUnsigned {
            if record.publisher_key_id.is_some() || record.generation != 0 {
                return Err(PluginTrustStoreError::Invalid(format!(
                    "developer-unsigned trust record for {} has publisher provenance",
                    record.extension_id.0
                )));
            }
        } else if record.generation == 0 {
            return Err(PluginTrustStoreError::Invalid(format!(
                "trust record for {} has no generation",
                record.extension_id.0
            )));
        }
        if ids.insert(record.extension_id.clone(), ()).is_some() {
            return Err(PluginTrustStoreError::Invalid(format!(
                "duplicate trust record for {}",
                record.extension_id.0
            )));
        }
    }
    Ok(())
}

fn validate_state(state: &PluginTrustStoreState) -> Result<(), PluginTrustStoreError> {
    if state.schema_version != PLUGIN_TRUST_STORE_SCHEMA_VERSION {
        return Err(PluginTrustStoreError::Corrupt(format!(
            "plugin trust store schema {} is unsupported",
            state.schema_version
        )));
    }
    validate_records(&state.records)
}
