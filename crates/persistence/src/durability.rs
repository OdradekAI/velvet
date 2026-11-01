use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub const DURABILITY_REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DurabilityMode {
    AtomicRenameOnly,
    RequireFsync,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DurabilityReport {
    pub schema_version: u16,
    pub mode: DurabilityMode,
    pub atomic_replace: bool,
    pub file_sync: bool,
    pub directory_sync: bool,
    pub crash_durable: bool,
    pub detail: String,
}

impl DurabilityReport {
    pub fn for_mode(mode: DurabilityMode) -> Self {
        let directory_sync = mode == DurabilityMode::RequireFsync && directory_sync_supported();
        let file_sync = mode == DurabilityMode::RequireFsync && directory_sync;
        Self {
            schema_version: DURABILITY_REPORT_SCHEMA_VERSION,
            mode,
            atomic_replace: true,
            file_sync,
            directory_sync,
            crash_durable: file_sync && directory_sync,
            detail: if mode == DurabilityMode::RequireFsync && !directory_sync_supported() {
                "required fsync is unavailable on this platform; atomic rename only".into()
            } else if mode == DurabilityMode::RequireFsync {
                "file and parent-directory fsync are required before publication".into()
            } else {
                "same-directory atomic rename is enabled without fsync".into()
            },
        }
    }
}

pub trait DurabilityReportStore: Send + Sync {
    fn load(&self) -> io::Result<Option<DurabilityReport>>;
    fn save(&self, report: &DurabilityReport) -> io::Result<()>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFileDurabilityReportStore {
    path: PathBuf,
}

impl JsonFileDurabilityReportStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl DurabilityReportStore for JsonFileDurabilityReportStore {
    fn load(&self) -> io::Result<Option<DurabilityReport>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let report: DurabilityReport = serde_json::from_slice(&bytes).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("durability report is invalid: {error}"),
            )
        })?;
        if report.schema_version != DURABILITY_REPORT_SCHEMA_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "durability report schema is unsupported",
            ));
        }
        Ok(Some(report))
    }

    fn save(&self, report: &DurabilityReport) -> io::Result<()> {
        if report.schema_version != DURABILITY_REPORT_SCHEMA_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "durability report schema is unsupported",
            ));
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(report).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("durability report could not be encoded: {error}"),
            )
        })?;
        write_atomic(&self.path, &bytes, DurabilityMode::AtomicRenameOnly)
    }
}

#[derive(Default)]
pub struct InMemoryDurabilityReportStore {
    report: std::sync::Mutex<Option<DurabilityReport>>,
}

impl DurabilityReportStore for InMemoryDurabilityReportStore {
    fn load(&self) -> io::Result<Option<DurabilityReport>> {
        self.report
            .lock()
            .map(|report| report.clone())
            .map_err(|_| io::Error::other("durability report lock poisoned"))
    }

    fn save(&self, report: &DurabilityReport) -> io::Result<()> {
        self.report
            .lock()
            .map(|mut current| *current = Some(report.clone()))
            .map_err(|_| io::Error::other("durability report lock poisoned"))
    }
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8], mode: DurabilityMode) -> io::Result<()> {
    if mode == DurabilityMode::RequireFsync && !directory_sync_supported() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "directory fsync is not supported on this platform",
        ));
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    fs::write(&temporary, bytes)?;
    if mode == DurabilityMode::RequireFsync {
        File::open(&temporary)?.sync_all()?;
    }
    fs::rename(&temporary, path)?;
    if mode == DurabilityMode::RequireFsync {
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "atomic path has no parent")
        })?;
        if let Err(error) = File::open(parent).and_then(|directory| directory.sync_all()) {
            let _ = fs::remove_file(path);
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(unix)]
const fn directory_sync_supported() -> bool {
    true
}

#[cfg(not(unix))]
const fn directory_sync_supported() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "velvet-durability-report-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn report_records_effective_atomic_and_fsync_capabilities() {
        let atomic = DurabilityReport::for_mode(DurabilityMode::AtomicRenameOnly);
        assert!(atomic.atomic_replace);
        assert!(!atomic.file_sync);
        assert!(!atomic.directory_sync);
        assert!(!atomic.crash_durable);

        let fsync = DurabilityReport::for_mode(DurabilityMode::RequireFsync);
        assert!(fsync.atomic_replace);
        assert_eq!(fsync.crash_durable, fsync.file_sync && fsync.directory_sync);
    }

    #[test]
    fn json_report_store_round_trips_and_rejects_unknown_schema() {
        let path = temporary_path();
        let store = JsonFileDurabilityReportStore::new(&path);
        let report = DurabilityReport::for_mode(DurabilityMode::AtomicRenameOnly);
        store.save(&report).expect("durability report should save");
        assert_eq!(
            store.load().expect("durability report should load"),
            Some(report.clone())
        );

        let mut invalid = report;
        invalid.schema_version = DURABILITY_REPORT_SCHEMA_VERSION + 1;
        fs::write(
            &path,
            serde_json::to_vec(&invalid).expect("report should encode"),
        )
        .expect("invalid report should write");
        assert_eq!(
            store
                .load()
                .expect_err("unknown report schema should fail")
                .kind(),
            io::ErrorKind::InvalidData
        );
        let _ = fs::remove_file(path);
    }
}
