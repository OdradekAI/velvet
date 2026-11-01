use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const LAUNCHER_FAILURE_SCHEMA_VERSION: u16 = 1;
pub const LAUNCHER_FAILURE_FILE: &str = "launcher-failure.json";
pub const LAUNCHER_ATTEMPT_SCHEMA_VERSION: u16 = 1;
pub const LAUNCHER_ATTEMPT_FILE: &str = "launcher-attempt.json";
const LAUNCHER_ATTEMPT_PREFIX: &str = "launcher-attempt-";
const MAX_BYTES: u64 = 16 * 1024;
const MAX_TEXT: usize = 8 * 1024;
const MAX_LAUNCHER_ATTEMPTS: usize = 8;
static LAUNCHER_ATTEMPT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LauncherFailureRecord {
    pub schema_version: u16,
    pub phase: String,
    pub message: String,
    pub exit_code: Option<i32>,
    pub stderr_tail: Option<String>,
    pub occurred_at_ms: u64,
}

/// A release wrapper writes this before spawning the application. It is kept
/// until the child exits successfully so a killed/loader-failed wrapper can be
/// classified by the next Core launch without owning business state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LauncherAttemptRecord {
    pub schema_version: u16,
    pub correlation_id: String,
    pub launcher_process_id: u32,
    #[serde(default)]
    pub child_process_id: Option<u32>,
    pub started_at_ms: u64,
}

pub fn record_launcher_failure(
    root: &Path,
    phase: impl Into<String>,
    message: impl Into<String>,
    exit_code: Option<i32>,
    stderr_tail: Option<String>,
) -> Result<(), String> {
    let record = LauncherFailureRecord {
        schema_version: LAUNCHER_FAILURE_SCHEMA_VERSION,
        phase: bounded(phase.into(), 128),
        message: bounded(message.into(), MAX_TEXT),
        exit_code,
        stderr_tail: stderr_tail.map(|tail| bounded(tail, MAX_TEXT)),
        occurred_at_ms: now_ms(),
    };
    write_json(root.join(LAUNCHER_FAILURE_FILE), &record)
}

pub fn consume_launcher_failure(root: &Path) -> Option<LauncherFailureRecord> {
    let path = root.join(LAUNCHER_FAILURE_FILE);
    let record = read_json::<LauncherFailureRecord>(&path).ok()?;
    if record.schema_version != LAUNCHER_FAILURE_SCHEMA_VERSION {
        return None;
    }
    let _ = fs::remove_file(path);
    Some(record)
}

pub fn arm_launcher_attempt(root: &Path) -> Result<LauncherAttemptRecord, String> {
    let path = root.join(LAUNCHER_ATTEMPT_FILE);
    if path.exists() {
        let stale_path = root.join(format!(
            "launcher-attempt-stale-{}-{}.json",
            std::process::id(),
            LAUNCHER_ATTEMPT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::rename(&path, stale_path).map_err(|error| error.to_string())?;
    }
    let record = LauncherAttemptRecord {
        schema_version: LAUNCHER_ATTEMPT_SCHEMA_VERSION,
        correlation_id: format!("launcher-{}-{}", std::process::id(), now_ms()),
        launcher_process_id: std::process::id(),
        child_process_id: None,
        started_at_ms: now_ms(),
    };
    write_json(path, &record)?;
    prune_launcher_attempts(root);
    Ok(record)
}

pub fn record_launcher_child(root: &Path, child_process_id: u32) -> Result<(), String> {
    let path = root.join(LAUNCHER_ATTEMPT_FILE);
    let mut record = read_json::<LauncherAttemptRecord>(&path)?;
    if record.schema_version != LAUNCHER_ATTEMPT_SCHEMA_VERSION {
        return Err("launcher attempt schema is unsupported".into());
    }
    record.child_process_id = Some(child_process_id);
    write_json(path, &record)
}

pub fn complete_launcher_attempt(root: &Path) -> Result<(), String> {
    let path = root.join(LAUNCHER_ATTEMPT_FILE);
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn consume_launcher_attempt(root: &Path) -> Option<LauncherAttemptRecord> {
    let path = root.join(LAUNCHER_ATTEMPT_FILE);
    let record = read_json::<LauncherAttemptRecord>(&path).ok()?;
    if record.schema_version != LAUNCHER_ATTEMPT_SCHEMA_VERSION {
        return None;
    }
    let _ = fs::remove_file(path);
    Some(record)
}

pub fn read_launcher_attempt(root: &Path) -> Option<LauncherAttemptRecord> {
    let record = read_json::<LauncherAttemptRecord>(&root.join(LAUNCHER_ATTEMPT_FILE)).ok()?;
    (record.schema_version == LAUNCHER_ATTEMPT_SCHEMA_VERSION).then_some(record)
}

pub fn read_launcher_attempts(root: &Path) -> Vec<(PathBuf, LauncherAttemptRecord)> {
    let mut paths = Vec::new();
    let primary = root.join(LAUNCHER_ATTEMPT_FILE);
    if primary.is_file() {
        paths.push(primary);
    }
    if let Ok(entries) = fs::read_dir(root) {
        paths.extend(entries.flatten().filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            (name.starts_with(LAUNCHER_ATTEMPT_PREFIX)
                && name != LAUNCHER_ATTEMPT_FILE
                && path.is_file())
            .then_some(path)
        }));
    }
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let record = read_json::<LauncherAttemptRecord>(&path).ok()?;
            (record.schema_version == LAUNCHER_ATTEMPT_SCHEMA_VERSION).then_some((path, record))
        })
        .collect()
}

pub fn consume_launcher_attempt_at(path: &Path) -> Option<LauncherAttemptRecord> {
    let record = read_json::<LauncherAttemptRecord>(path).ok()?;
    if record.schema_version != LAUNCHER_ATTEMPT_SCHEMA_VERSION {
        return None;
    }
    let _ = fs::remove_file(path);
    Some(record)
}

fn prune_launcher_attempts(root: &Path) {
    let mut stale_paths = fs::read_dir(root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            (name.starts_with(LAUNCHER_ATTEMPT_PREFIX)
                && name != LAUNCHER_ATTEMPT_FILE
                && path.is_file())
            .then_some(path)
        })
        .collect::<Vec<_>>();
    stale_paths.sort();
    let keep = MAX_LAUNCHER_ATTEMPTS.saturating_sub(1);
    let remove_count = stale_paths.len().saturating_sub(keep);
    for path in stale_paths.into_iter().take(remove_count) {
        let _ = fs::remove_file(path);
    }
}

fn bounded(value: String, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "launcher failure path has no parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    let mut file = fs::File::create(&temporary).map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(temporary, &path).map_err(|error| error.to_string())?;
    if let Ok(directory) = fs::File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_BYTES {
        return Err("launcher failure marker exceeds its size limit".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_failure_is_bounded_and_one_shot() {
        let root = std::env::temp_dir().join(format!("velvet-launcher-failure-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        record_launcher_failure(
            &root,
            "loader",
            "x".repeat(20_000),
            Some(127),
            Some("stderr".into()),
        )
        .expect("marker should be written");
        let record = consume_launcher_failure(&root).expect("marker should be consumed");
        assert_eq!(record.phase, "loader");
        assert_eq!(record.message.chars().count(), MAX_TEXT);
        assert_eq!(record.exit_code, Some(127));
        assert!(consume_launcher_failure(&root).is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn launcher_attempt_is_armed_updated_and_consumed_once() {
        let root = std::env::temp_dir().join(format!("velvet-launcher-attempt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let record = arm_launcher_attempt(&root).expect("attempt should be armed");
        assert_eq!(record.child_process_id, None);
        record_launcher_child(&root, 42).expect("child identity should be recorded");
        let consumed = consume_launcher_attempt(&root).expect("attempt should be consumed");
        assert_eq!(consumed.child_process_id, Some(42));
        assert!(consume_launcher_attempt(&root).is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn arming_a_new_attempt_preserves_the_previous_attempt() {
        let root = std::env::temp_dir().join(format!(
            "velvet-launcher-attempt-history-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        arm_launcher_attempt(&root).expect("first attempt should be armed");
        arm_launcher_attempt(&root).expect("second attempt should be armed");
        assert_eq!(read_launcher_attempts(&root).len(), 2);
        let _ = fs::remove_dir_all(root);
    }
}
