use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const NATIVE_RECOVERY_SCHEMA_VERSION: u16 = 1;
pub const NATIVE_RECOVERY_REQUEST_FILE: &str = "native-recovery-request.json";
pub const NATIVE_RECOVERY_INTENT_FILE: &str = "native-recovery-intent.json";
const MAX_RECOVERY_WINDOW_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeRecoveryAction {
    Retry,
    RestoreRecoverySet,
    ContinueSafeMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeRecoveryRequest {
    pub schema_version: u16,
    pub title: String,
    pub message: String,
    pub actions: Vec<NativeRecoveryAction>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeRecoveryIntent {
    pub schema_version: u16,
    pub action: NativeRecoveryAction,
    pub selected_at_ms: u64,
}

pub fn launch_native_recovery_window(root: &Path, request: NativeRecoveryRequest) -> Result<(), String> {
    write_json(root.join(NATIVE_RECOVERY_REQUEST_FILE), &request)?;
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    Command::new(executable)
        .arg("--velvet-recovery-window")
        .arg("--velvet-recovery-root")
        .arg(root)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("native recovery window could not start: {error}"))
}

pub fn run_native_recovery_window(root: &Path) -> Result<(), String> {
    let request = read_json::<NativeRecoveryRequest>(root.join(NATIVE_RECOVERY_REQUEST_FILE))?;
    if request.schema_version != NATIVE_RECOVERY_SCHEMA_VERSION {
        return Err("native recovery request schema is unsupported".into());
    }
    let action = show_native_dialog(&request).unwrap_or(NativeRecoveryAction::ContinueSafeMode);
    write_json(
        root.join(NATIVE_RECOVERY_INTENT_FILE),
        &NativeRecoveryIntent {
            schema_version: NATIVE_RECOVERY_SCHEMA_VERSION,
            action,
            selected_at_ms: now_ms(),
        },
    )?;
    let _ = fs::remove_file(root.join(NATIVE_RECOVERY_REQUEST_FILE));
    Ok(())
}

pub fn consume_native_recovery_intent(root: &Path) -> Option<NativeRecoveryIntent> {
    let path = root.join(NATIVE_RECOVERY_INTENT_FILE);
    let intent = read_json::<NativeRecoveryIntent>(path.clone()).ok()?;
    if intent.schema_version != NATIVE_RECOVERY_SCHEMA_VERSION {
        return None;
    }
    let _ = fs::remove_file(path);
    Some(intent)
}

fn show_native_dialog(request: &NativeRecoveryRequest) -> Option<NativeRecoveryAction> {
    #[cfg(target_os = "macos")]
    {
        return show_macos_dialog(request);
    }
    #[cfg(target_os = "linux")]
    {
        return show_linux_dialog(request);
    }
    #[cfg(target_os = "windows")]
    {
        return show_windows_dialog(request);
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(target_os = "macos")]
fn show_macos_dialog(request: &NativeRecoveryRequest) -> Option<NativeRecoveryAction> {
    let script = r#"
on run argv
  set dialogTitle to item 1 of argv
  set dialogMessage to item 2 of argv
  set result to display dialog dialogMessage with title dialogTitle buttons {"Continue Safe Mode", "Restore validated state", "Retry"} default button "Retry" cancel button "Continue Safe Mode"
  return button returned of result
end run
"#;
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script, "--", &request.title, &request.message])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    action_from_label(String::from_utf8_lossy(&output.stdout).trim())
}

#[cfg(target_os = "linux")]
fn show_linux_dialog(request: &NativeRecoveryRequest) -> Option<NativeRecoveryAction> {
    let zenity = Command::new("zenity")
        .args([
            "--question",
            "--title",
            &request.title,
            "--text",
            &request.message,
            "--ok-label",
            "Retry",
            "--cancel-label",
            "Continue Safe Mode",
            "--extra-button",
            "Restore validated state",
        ])
        .output();
    if let Ok(output) = zenity {
        if output.status.success() {
            let label = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            return Some(if label.is_empty() {
                NativeRecoveryAction::Retry
            } else {
                action_from_label(&label)?
            });
        }
        return Some(NativeRecoveryAction::ContinueSafeMode);
    }
    let status = Command::new("kdialog")
        .args(["--title", &request.title, "--yesno", &request.message])
        .status()
        .ok()?;
    Some(if status.success() {
        NativeRecoveryAction::Retry
    } else {
        NativeRecoveryAction::ContinueSafeMode
    })
}

#[cfg(target_os = "windows")]
fn show_windows_dialog(request: &NativeRecoveryRequest) -> Option<NativeRecoveryAction> {
    let script = r#"
Add-Type -AssemblyName PresentationFramework
$result = [System.Windows.MessageBox]::Show($env:VELVET_RECOVERY_MESSAGE, $env:VELVET_RECOVERY_TITLE, [System.Windows.MessageBoxButton]::YesNoCancel, [System.Windows.MessageBoxImage]::Warning)
if ($result -eq [System.Windows.MessageBoxResult]::Yes) { exit 0 }
if ($result -eq [System.Windows.MessageBoxResult]::No) { exit 2 }
exit 1
"#;
    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("VELVET_RECOVERY_TITLE", &request.title)
        .env("VELVET_RECOVERY_MESSAGE", &request.message)
        .status()
        .ok()?;
    Some(match status.code() {
        Some(0) => NativeRecoveryAction::Retry,
        Some(2) => NativeRecoveryAction::RestoreRecoverySet,
        _ => NativeRecoveryAction::ContinueSafeMode,
    })
}

fn action_from_label(label: &str) -> Option<NativeRecoveryAction> {
    match label {
        "Retry" => Some(NativeRecoveryAction::Retry),
        "Restore validated state" => Some(NativeRecoveryAction::RestoreRecoverySet),
        "Continue Safe Mode" => Some(NativeRecoveryAction::ContinueSafeMode),
        _ => None,
    }
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "native recovery path has no parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary, &path).map_err(|error| error.to_string())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, String> {
    let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_RECOVERY_WINDOW_BYTES {
        return Err("native recovery file exceeds its size limit".into());
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
    fn native_recovery_intent_is_bounded_and_one_shot() {
        let root = std::env::temp_dir().join(format!(
            "velvet-native-recovery-window-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        write_json(
            root.join(NATIVE_RECOVERY_INTENT_FILE),
            &NativeRecoveryIntent {
                schema_version: NATIVE_RECOVERY_SCHEMA_VERSION,
                action: NativeRecoveryAction::RestoreRecoverySet,
                selected_at_ms: 1,
            },
        )
        .expect("intent should be written");
        assert_eq!(
            consume_native_recovery_intent(&root).map(|intent| intent.action),
            Some(NativeRecoveryAction::RestoreRecoverySet)
        );
        assert!(consume_native_recovery_intent(&root).is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn action_labels_are_closed_over_typed_recovery_actions() {
        assert_eq!(action_from_label("Retry"), Some(NativeRecoveryAction::Retry));
        assert_eq!(
            action_from_label("Restore validated state"),
            Some(NativeRecoveryAction::RestoreRecoverySet)
        );
        assert_eq!(action_from_label("unknown"), None);
    }
}
