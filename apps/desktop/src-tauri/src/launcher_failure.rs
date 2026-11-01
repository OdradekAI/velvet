use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const LAUNCHER_FAILURE_SCHEMA_VERSION: u16 = 1;
pub const LAUNCHER_FAILURE_FILE: &str = "launcher-failure.json";
const MAX_BYTES: u64 = 16 * 1024;
const MAX_TEXT: usize = 8 * 1024;

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
    let record = read_json(&path).ok()?;
    if record.schema_version != LAUNCHER_FAILURE_SCHEMA_VERSION {
        return None;
    }
    let _ = fs::remove_file(path);
    Some(record)
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
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn read_json(path: &Path) -> Result<LauncherFailureRecord, String> {
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
}
