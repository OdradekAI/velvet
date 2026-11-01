use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use velvet_workbench_protocol::{ActivationRequest, CoreError};

const LOCK_SCHEMA_VERSION: u16 = 1;
const LOCK_FILE_NAME: &str = "profile.lock";
const SOCKET_FILE_NAME: &str = "profile-instance.sock";
const MAX_FRAME_BYTES: usize = 64 * 1024;
const MAX_CONNECT_ATTEMPTS: usize = 20;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileLockRecord {
    pub schema_version: u16,
    pub pid: u32,
    pub started_at_ms: u64,
    pub instance_nonce: String,
    #[serde(default)]
    pub process_start_token: Option<String>,
    pub socket_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LockOwnerState {
    Alive,
    Stale,
    Unknown,
}

#[derive(Debug)]
pub enum AcquireOutcome {
    Primary(ProfileInstanceCoordinator),
    Forwarded,
    Locked(ProfileLockRecord),
}

#[derive(Debug)]
pub struct ProfileInstanceCoordinator {
    lock_path: PathBuf,
    socket_path: PathBuf,
    lock_file: Option<File>,
    record: ProfileLockRecord,
    stop: Arc<AtomicBool>,
    listener: Option<JoinHandle<()>>,
}

impl ProfileInstanceCoordinator {
    pub fn acquire(root: impl Into<PathBuf>, request: &ActivationRequest) -> io::Result<AcquireOutcome> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        request
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error_message(error)))?;

        let lock_path = root.join(LOCK_FILE_NAME);
        let socket_path = root.join(SOCKET_FILE_NAME);
        let record = ProfileLockRecord {
            schema_version: LOCK_SCHEMA_VERSION,
            pid: std::process::id(),
            started_at_ms: now_ms(),
            instance_nonce: format!("{}-{}", std::process::id(), now_ms()),
            process_start_token: process_start_token(std::process::id()),
            socket_path: Some(socket_path.to_string_lossy().into_owned()),
        };

        match OpenOptions::new().write(true).create_new(true).open(&lock_path) {
            Ok(mut lock_file) => {
                write_json(&mut lock_file, &record)?;
                Ok(AcquireOutcome::Primary(Self {
                    lock_path,
                    socket_path,
                    lock_file: Some(lock_file),
                    record,
                    stop: Arc::new(AtomicBool::new(false)),
                    listener: None,
                }))
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let existing = read_record(&lock_path)?;
                match lock_owner_state(&existing) {
                    LockOwnerState::Stale => {
                        if remove_lock_if_same(&lock_path, &existing)? {
                            return Self::acquire(root, request);
                        }
                        Ok(AcquireOutcome::Locked(read_record(&lock_path)?))
                    }
                    LockOwnerState::Alive | LockOwnerState::Unknown => {
                        if forward_to_primary(&existing, request).unwrap_or(false) {
                            Ok(AcquireOutcome::Forwarded)
                        } else {
                            Ok(AcquireOutcome::Locked(existing))
                        }
                    }
                }
            }
            Err(error) => Err(error),
        }
    }

    pub fn record(&self) -> &ProfileLockRecord {
        &self.record
    }

    pub fn start_listener<F>(&mut self, handler: F) -> io::Result<()>
    where
        F: Fn(ActivationRequest) + Send + 'static,
    {
        #[cfg(not(unix))]
        {
            let _ = handler;
            return Ok(());
        }

        #[cfg(unix)]
        {
            use std::os::unix::net::UnixListener;

            let _ = fs::remove_file(&self.socket_path);
            let listener = UnixListener::bind(&self.socket_path)?;
            listener.set_nonblocking(true)?;
            let socket_path = self.socket_path.clone();
            let stop = self.stop.clone();
            self.listener = Some(thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            if let Ok(request) = read_frame(&mut stream) {
                                if request.validate().is_ok() {
                                    handler(request);
                                    let _ = write_ack(&mut stream, true);
                                } else {
                                    let _ = write_ack(&mut stream, false);
                                }
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(25));
                        }
                        Err(_) => break,
                    }
                }
                let _ = fs::remove_file(socket_path);
            }));
            Ok(())
        }
    }
}

impl Drop for ProfileInstanceCoordinator {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        #[cfg(unix)]
        let _ = std::os::unix::net::UnixStream::connect(&self.socket_path);
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
        let _ = fs::remove_file(&self.socket_path);
        let _ = self.lock_file.take();
        if read_record(&self.lock_path)
            .map(|record| {
                record.pid == self.record.pid
                    && record.instance_nonce == self.record.instance_nonce
            })
            .unwrap_or(false)
        {
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

fn forward_to_primary(record: &ProfileLockRecord, request: &ActivationRequest) -> io::Result<bool> {
    #[cfg(not(unix))]
    {
        let _ = (record, request);
        return Ok(false);
    }

    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;

        let Some(socket_path) = record.socket_path.as_deref() else {
            return Ok(false);
        };
        for _ in 0..MAX_CONNECT_ATTEMPTS {
            match UnixStream::connect(socket_path) {
                Ok(mut stream) => {
                    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
                    write_frame(&mut stream, request)?;
                    return read_ack(&mut stream);
                }
                Err(error) if matches!(error.kind(), io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound) => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(false)
    }
}

fn lock_owner_state(record: &ProfileLockRecord) -> LockOwnerState {
    let Some(expected) = record.process_start_token.as_deref() else {
        return LockOwnerState::Unknown;
    };
    match inspect_process_start_token(record.pid) {
        ProcessObservation::Absent => LockOwnerState::Stale,
        ProcessObservation::Unknown => LockOwnerState::Unknown,
        ProcessObservation::Present(actual) if actual == expected => LockOwnerState::Alive,
        ProcessObservation::Present(_) => LockOwnerState::Stale,
    }
}

fn remove_lock_if_same(path: &Path, expected: &ProfileLockRecord) -> io::Result<bool> {
    match read_record(path) {
        Ok(actual) if actual == *expected => match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
            Err(error) => Err(error),
        },
        Ok(_) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

enum ProcessObservation {
    Absent,
    Present(String),
    Unknown,
}

#[cfg(target_os = "linux")]
fn inspect_process_start_token(pid: u32) -> ProcessObservation {
    let path = format!("/proc/{pid}/stat");
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return ProcessObservation::Absent;
        }
        Err(_) => return ProcessObservation::Unknown,
    };
    let Some((_, fields)) = contents.rsplit_once(") ") else {
        return ProcessObservation::Unknown;
    };
    let Some(start_token) = fields.split_whitespace().nth(19) else {
        return ProcessObservation::Unknown;
    };
    ProcessObservation::Present(start_token.to_owned())
}

#[cfg(target_os = "macos")]
fn inspect_process_start_token(pid: u32) -> ProcessObservation {
    let info_size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let result = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            info_size,
        )
    };
    if result <= 0 {
        return if io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            ProcessObservation::Absent
        } else {
            ProcessObservation::Unknown
        };
    }
    if result < info_size {
        return ProcessObservation::Unknown;
    }
    let info = unsafe { info.assume_init() };
    ProcessObservation::Present(format!("{}:{}", info.pbi_start_tvsec, info.pbi_start_tvusec))
}

#[cfg(all(not(target_os = "linux"), not(target_os = "macos")))]
fn inspect_process_start_token(_pid: u32) -> ProcessObservation {
    ProcessObservation::Unknown
}

#[cfg(target_os = "linux")]
fn process_start_token(pid: u32) -> Option<String> {
    match inspect_process_start_token(pid) {
        ProcessObservation::Present(token) => Some(token),
        ProcessObservation::Absent | ProcessObservation::Unknown => None,
    }
}

#[cfg(target_os = "macos")]
fn process_start_token(pid: u32) -> Option<String> {
    match inspect_process_start_token(pid) {
        ProcessObservation::Present(token) => Some(token),
        ProcessObservation::Absent | ProcessObservation::Unknown => None,
    }
}

#[cfg(all(not(target_os = "linux"), not(target_os = "macos")))]
fn process_start_token(_pid: u32) -> Option<String> {
    None
}

fn read_record(path: &Path) -> io::Result<ProfileLockRecord> {
    let bytes = fs::read(path)?;
    let record: ProfileLockRecord = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if record.schema_version != LOCK_SCHEMA_VERSION || record.pid == 0 || record.instance_nonce.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid profile lock record"));
    }
    Ok(record)
}

fn write_json<T: Serialize>(file: &mut File, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    file.write_all(&bytes)?;
    file.sync_all()
}

fn write_frame(stream: &mut impl Write, request: &ActivationRequest) -> io::Result<()> {
    let bytes = serde_json::to_vec(request).map_err(io::Error::other)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "activation frame is too large"));
    }
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)
}

fn read_frame(stream: &mut impl Read) -> io::Result<ActivationRequest> {
    let mut length = [0_u8; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "activation frame is invalid"));
    }
    let mut bytes = vec![0_u8; length];
    stream.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn write_ack(stream: &mut impl Write, accepted: bool) -> io::Result<()> {
    stream.write_all(&[u8::from(accepted)])
}

fn read_ack(stream: &mut impl Read) -> io::Result<bool> {
    let mut ack = [0_u8; 1];
    stream.read_exact(&mut ack)?;
    Ok(ack[0] == 1)
}

fn error_message(error: CoreError) -> String {
    format!("{error:?}")
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
    use std::sync::mpsc;

    fn request(value: &str) -> ActivationRequest {
        ActivationRequest {
            schema_version: 1,
            request_id: "test-request".into(),
            targets: vec![velvet_workbench_protocol::ActivationTarget {
                kind: velvet_workbench_protocol::ActivationTargetKind::DeepLink,
                value: value.into(),
            }],
        }
    }

    #[cfg(unix)]
    #[test]
    fn lock_forwards_validated_request_to_primary_on_unix() {
        let root = std::env::temp_dir().join(format!("velvet-profile-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let primary = match ProfileInstanceCoordinator::acquire(&root, &request("velvet://asset/1"))
            .expect("primary lock")
        {
            AcquireOutcome::Primary(primary) => primary,
            other => panic!("unexpected primary outcome: {other:?}"),
        };
        let (sender, receiver) = mpsc::channel();
        let mut primary = primary;
        primary
            .start_listener(move |request| sender.send(request).expect("send activation"))
            .expect("listener");
        assert!(matches!(
            ProfileInstanceCoordinator::acquire(&root, &request("velvet://asset/2"))
                .expect("forward request"),
            AcquireOutcome::Forwarded
        ));
        assert_eq!(receiver.recv_timeout(Duration::from_secs(1)).expect("activation"), request("velvet://asset/2"));
        drop(primary);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_deep_link_is_rejected_before_forwarding() {
        let root = std::env::temp_dir().join(format!("velvet-profile-lock-invalid-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let primary = ProfileInstanceCoordinator::acquire(&root, &request("velvet://asset/1"))
            .expect("primary lock");
        assert!(ActivationRequest {
            schema_version: 1,
            request_id: "invalid".into(),
            targets: vec![velvet_workbench_protocol::ActivationTarget {
                kind: velvet_workbench_protocol::ActivationTargetKind::DeepLink,
                value: "https://example.invalid".into(),
            }],
        }
        .validate()
        .is_err());
        drop(primary);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_existing_lock_without_a_listener_fails_closed() {
        let root = std::env::temp_dir().join(format!("velvet-profile-lock-closed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let primary = match ProfileInstanceCoordinator::acquire(&root, &request("velvet://asset/1"))
            .expect("primary lock")
        {
            AcquireOutcome::Primary(primary) => primary,
            other => panic!("unexpected primary outcome: {other:?}"),
        };
        let outcome = ProfileInstanceCoordinator::acquire(&root, &request("velvet://asset/2"))
            .expect("lock contention should be classified");
        assert!(matches!(outcome, AcquireOutcome::Locked(_)));
        drop(primary);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_lock_is_not_deleted_or_taken_over() {
        let root = std::env::temp_dir().join(format!("velvet-profile-lock-malformed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("profile root");
        fs::write(root.join(LOCK_FILE_NAME), b"not-json").expect("lock marker");
        let error = ProfileInstanceCoordinator::acquire(&root, &request("velvet://asset/1"))
            .expect_err("malformed lock must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(root.join(LOCK_FILE_NAME).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn verified_absent_owner_is_recovered_before_acquiring_the_lock() {
        let root = std::env::temp_dir().join(format!("velvet-profile-lock-stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("profile root");
        let stale = ProfileLockRecord {
            schema_version: LOCK_SCHEMA_VERSION,
            pid: u32::MAX,
            started_at_ms: 1,
            instance_nonce: "stale-instance".into(),
            process_start_token: Some("stale-token".into()),
            socket_path: None,
        };
        fs::write(
            root.join(LOCK_FILE_NAME),
            serde_json::to_vec(&stale).expect("stale lock json"),
        )
        .expect("stale lock");
        let primary = match ProfileInstanceCoordinator::acquire(&root, &request("velvet://asset/1"))
            .expect("stale lock recovery")
        {
            AcquireOutcome::Primary(primary) => primary,
            other => panic!("unexpected stale-lock outcome: {other:?}"),
        };
        assert_ne!(primary.record().instance_nonce, stale.instance_nonce);
        drop(primary);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lock_without_identity_token_remains_fail_closed() {
        let root = std::env::temp_dir().join(format!("velvet-profile-lock-uncertain-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("profile root");
        let uncertain = ProfileLockRecord {
            schema_version: LOCK_SCHEMA_VERSION,
            pid: u32::MAX,
            started_at_ms: 1,
            instance_nonce: "uncertain-instance".into(),
            process_start_token: None,
            socket_path: None,
        };
        fs::write(
            root.join(LOCK_FILE_NAME),
            serde_json::to_vec(&uncertain).expect("uncertain lock json"),
        )
        .expect("uncertain lock");
        let outcome = ProfileInstanceCoordinator::acquire(&root, &request("velvet://asset/1"))
            .expect("uncertain lock classification");
        assert!(matches!(outcome, AcquireOutcome::Locked(_)));
        assert!(root.join(LOCK_FILE_NAME).exists());
        let _ = fs::remove_dir_all(root);
    }
}
