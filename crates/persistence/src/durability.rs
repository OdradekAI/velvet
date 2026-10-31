use std::fs::{self, File};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurabilityMode {
    AtomicRenameOnly,
    RequireFsync,
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
