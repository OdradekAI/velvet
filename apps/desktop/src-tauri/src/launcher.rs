#[path = "launcher_failure.rs"]
#[allow(dead_code)]
mod launcher_failure;

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const MAX_STDERR_BYTES: usize = 8 * 1024;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let root = app_data_root();
    let executable = std::env::var_os("VELVET_APP_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(default_application_binary);
    let mut command = Command::new(&executable);
    command
        .args(std::env::args_os().skip(1))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = launcher_failure::record_launcher_failure(
                &root,
                "loader",
                format!("could not launch Velvet application: {error}"),
                None,
                None,
            );
            return 1;
        }
    };
    let stderr = child.stderr.take().map(read_bounded_tail).unwrap_or_default();
    match child.wait() {
        Ok(status) if status.success() => 0,
        Ok(status) => {
            let tail = bounded_tail(&stderr);
            let _ = launcher_failure::record_launcher_failure(
                &root,
                "exit",
                "Velvet application exited before the launcher observed a healthy startup",
                status.code(),
                (!tail.is_empty()).then_some(tail),
            );
            status.code().unwrap_or(1)
        }
        Err(error) => {
            let _ = launcher_failure::record_launcher_failure(
                &root,
                "loader",
                format!("could not wait for Velvet application: {error}"),
                None,
                (!stderr.is_empty()).then_some(bounded_tail(&stderr)),
            );
            1
        }
    }
}

fn read_bounded_tail<R: Read>(mut reader: R) -> Vec<u8> {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 4096];
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
        retained.extend_from_slice(&buffer[..read]);
        if retained.len() > MAX_STDERR_BYTES {
            let drop_count = retained.len() - MAX_STDERR_BYTES;
            retained.drain(..drop_count);
        }
    }
    retained
}

fn bounded_tail(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn app_data_root() -> PathBuf {
    if let Some(path) = std::env::var_os("VELVET_APP_DATA_DIR") {
        return PathBuf::from(path);
    }
    platform_application_data_root().unwrap_or_else(default_application_data_fallback)
}

#[cfg(target_os = "windows")]
fn platform_application_data_root() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("com.velvet.desktop"))
}

#[cfg(target_os = "macos")]
fn platform_application_data_root() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|path| path.join("Library/Application Support/com.velvet.desktop"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn platform_application_data_root() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|path| path.join(".local/share"))
        })
        .map(|path| path.join("com.velvet.desktop"))
}

fn default_application_data_fallback() -> PathBuf {
    default_application_binary()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("velvet-data")
}

fn default_application_binary() -> PathBuf {
    let name = if cfg!(windows) { "velvet-desktop.exe" } else { "velvet-desktop" };
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_failure_writes_a_marker_without_starting_core() {
        let root = std::env::temp_dir().join(format!("velvet-launcher-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::env::set_var("VELVET_APP_DATA_DIR", &root);
        std::env::set_var("VELVET_APP_BINARY", root.join("missing-velvet-desktop"));
        assert_eq!(run(), 1);
        let record = launcher_failure::consume_launcher_failure(&root)
            .expect("loader failure should be recorded");
        assert_eq!(record.phase, "loader");
        std::env::remove_var("VELVET_APP_DATA_DIR");
        std::env::remove_var("VELVET_APP_BINARY");
        let _ = std::fs::remove_dir_all(root);
    }
}
