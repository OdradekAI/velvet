use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use velvet_application_core::{CrashCaptureAdapter, RecoveryError};
use velvet_workbench_protocol::{
    CrashCaptureEvidence, CrashCaptureKind, StartupPhase, CRASH_CAPTURE_SCHEMA_VERSION,
};

pub const ACTIVE_PROCESS_RECEIPT_DIR: &str = "active-process-receipts";
pub const CRASH_RECEIPTS_FILE: &str = "crash-receipts.json";
const MAX_CRASH_RECEIPTS_BYTES: u64 = 256 * 1024;
const MAX_CRASH_RECEIPTS: usize = 32;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActiveProcessReceipt {
    schema_version: u16,
    correlation_id: String,
    attempt_id: u64,
    process_id: u32,
    phase: StartupPhase,
    started_at_ms: u64,
    clean_exit: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CrashReceiptFile {
    schema_version: u16,
    receipts: Vec<CrashCaptureEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileCrashCaptureAdapter {
    root: PathBuf,
}

impl FileCrashCaptureAdapter {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl CrashCaptureAdapter for FileCrashCaptureAdapter {
    fn consume(&self) -> Result<Vec<CrashCaptureEvidence>, RecoveryError> {
        let mut evidence = read_external_receipts(&self.root)?;
        if let Ok(entries) = fs::read_dir(self.root.join(ACTIVE_PROCESS_RECEIPT_DIR)) {
            for entry in entries.flatten() {
                let active_path = entry.path();
                let Ok(bytes) = fs::read(&active_path) else {
                    continue;
                };
                let Ok(receipt) = serde_json::from_slice::<ActiveProcessReceipt>(&bytes) else {
                    continue;
                };
                if receipt.process_id == std::process::id() {
                    continue;
                }
                if receipt.schema_version == CRASH_CAPTURE_SCHEMA_VERSION && !receipt.clean_exit {
                    evidence.push(CrashCaptureEvidence {
                        schema_version: CRASH_CAPTURE_SCHEMA_VERSION,
                        correlation_id: receipt.correlation_id,
                        process_id: receipt.process_id,
                        parent_process_id: None,
                        attempt_id: receipt.attempt_id,
                        component: "core".into(),
                        kind: CrashCaptureKind::ProcessExit,
                        code: "unclean-process-exit".into(),
                        message: format!(
                            "previous Velvet process ended during {}",
                            startup_phase_name(&receipt.phase)
                        ),
                        runtime_nonce: None,
                        log_tail: None,
                        occurred_at_ms: now_ms(),
                    });
                }
                let _ = fs::remove_file(active_path);
            }
        }
        Ok(evidence)
    }
}

pub fn arm_process_receipt(root: &Path) -> Result<String, String> {
    let correlation_id = format!("pid-{}-{}", std::process::id(), now_ms());
    write_json(
        active_process_receipt_path(root),
        &ActiveProcessReceipt {
            schema_version: CRASH_CAPTURE_SCHEMA_VERSION,
            correlation_id: correlation_id.clone(),
            attempt_id: 0,
            process_id: std::process::id(),
            phase: StartupPhase::Preflight,
            started_at_ms: now_ms(),
            clean_exit: false,
        },
    )?;
    Ok(correlation_id)
}

pub fn update_process_receipt(
    root: &Path,
    correlation_id: &str,
    attempt_id: u64,
    phase: StartupPhase,
) -> Result<(), String> {
    let path = active_process_receipt_path(root);
    let mut receipt = read_json::<ActiveProcessReceipt>(&path)?;
    receipt.correlation_id = correlation_id.to_owned();
    receipt.attempt_id = attempt_id;
    receipt.phase = phase;
    write_json(path, &receipt)
}

pub fn complete_process_receipt(root: &Path) -> Result<(), String> {
    let path = active_process_receipt_path(root);
    if !path.exists() {
        return Ok(());
    }
    let _ = fs::remove_file(path);
    Ok(())
}

/// File protocol for a future OS crash handler or launcher wrapper. The
/// current alpha consumes receipts written by that adapter when present.
#[allow(dead_code)]
pub fn append_external_crash_receipt(
    root: &Path,
    evidence: CrashCaptureEvidence,
) -> Result<(), String> {
    if evidence.schema_version != CRASH_CAPTURE_SCHEMA_VERSION {
        return Err("crash receipt schema is unsupported".into());
    }
    let path = root.join(CRASH_RECEIPTS_FILE);
    let mut state = if path.exists() {
        read_json::<CrashReceiptFile>(&path)?
    } else {
        CrashReceiptFile {
            schema_version: CRASH_CAPTURE_SCHEMA_VERSION,
            receipts: Vec::new(),
        }
    };
    if state.schema_version != CRASH_CAPTURE_SCHEMA_VERSION {
        return Err("crash receipt store schema is unsupported".into());
    }
    state.receipts.push(evidence);
    if state.receipts.len() > MAX_CRASH_RECEIPTS {
        let remove = state.receipts.len() - MAX_CRASH_RECEIPTS;
        state.receipts.drain(0..remove);
    }
    write_json(path, &state)
}

fn read_external_receipts(root: &Path) -> Result<Vec<CrashCaptureEvidence>, RecoveryError> {
    let path = root.join(CRASH_RECEIPTS_FILE);
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(RecoveryError::Store(error.to_string())),
    };
    if metadata.len() > MAX_CRASH_RECEIPTS_BYTES {
        return Err(RecoveryError::Store(
            "crash receipt store exceeds its size limit".into(),
        ));
    }
    let state = read_json::<CrashReceiptFile>(&path).map_err(RecoveryError::Store)?;
    if state.schema_version != CRASH_CAPTURE_SCHEMA_VERSION
        || state.receipts.len() > MAX_CRASH_RECEIPTS
        || state
            .receipts
            .iter()
            .any(|receipt| receipt.schema_version != CRASH_CAPTURE_SCHEMA_VERSION)
    {
        return Err(RecoveryError::Store("crash receipt store is invalid".into()));
    }
    let _ = fs::remove_file(path);
    Ok(state.receipts)
}

fn startup_phase_name(phase: &StartupPhase) -> &'static str {
    match phase {
        StartupPhase::Preflight => "preflight",
        StartupPhase::RecoveringState => "recovering-state",
        StartupPhase::CoreReady => "core-ready",
        StartupPhase::WorkbenchReady => "workbench-ready",
        StartupPhase::OptionalActivation => "optional-activation",
        StartupPhase::Ready => "ready",
    }
}

fn active_process_receipt_path(root: &Path) -> PathBuf {
    root.join(ACTIVE_PROCESS_RECEIPT_DIR)
        .join(format!("{}.json", std::process::id()))
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "crash receipt path has no parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_CRASH_RECEIPTS_BYTES {
        return Err("crash receipt exceeds its size limit".into());
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
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
    fn unclean_process_receipt_is_correlated_once() {
        let root = std::env::temp_dir().join(format!(
            "velvet-crash-capture-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        write_json(
            active_process_receipt_path(&root),
            &ActiveProcessReceipt {
                schema_version: CRASH_CAPTURE_SCHEMA_VERSION,
                correlation_id: "instance-1".into(),
                attempt_id: 9,
                process_id: 77,
                phase: StartupPhase::WorkbenchReady,
                started_at_ms: 1,
                clean_exit: false,
            },
        )
        .expect("receipt should be written");
        let adapter = FileCrashCaptureAdapter::new(&root);
        let evidence = adapter.consume().expect("receipt should be consumed");
        assert_eq!(evidence[0].correlation_id, "instance-1");
        assert_eq!(evidence[0].attempt_id, 9);
        assert!(adapter.consume().expect("receipt should be one-shot").is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn external_receipts_are_bounded_and_consumed() {
        let root = std::env::temp_dir().join(format!(
            "velvet-crash-receipts-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        append_external_crash_receipt(
            &root,
            CrashCaptureEvidence {
                schema_version: CRASH_CAPTURE_SCHEMA_VERSION,
                correlation_id: "instance-2".into(),
                process_id: 88,
                parent_process_id: Some(77),
                attempt_id: 10,
                component: "sample.plugin".into(),
                kind: CrashCaptureKind::Runtime,
                code: "signal".into(),
                message: "sidecar failed".into(),
                runtime_nonce: Some("nonce".into()),
                log_tail: Some("tail".into()),
                occurred_at_ms: 2,
            },
        )
        .expect("external receipt should be written");
        let evidence = FileCrashCaptureAdapter::new(&root)
            .consume()
            .expect("external receipt should be consumed");
        assert_eq!(evidence[0].runtime_nonce.as_deref(), Some("nonce"));
        let _ = fs::remove_dir_all(root);
    }
}
