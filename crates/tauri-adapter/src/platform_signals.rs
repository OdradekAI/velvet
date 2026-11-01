//! Host-owned adapters for operating-system lifecycle and network signals.
//!
//! The adapter intentionally has no Tauri types.  It samples only fixed,
//! allow-listed platform commands and forwards a typed environment command to
//! the Rust-owned JobSystem through `TauriTransport`.  It never owns Job state
//! and it does not execute a shell command supplied by a manifest or user.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use velvet_job_system::{JobExecutionEnvironment, JobNetworkState};
use velvet_workbench_protocol::{Command as CoreCommand, CoreTransport, Request, RequestId};

use crate::TauriTransport;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
// Keep this above the normal polling interval so an ordinary poll is not
// mistaken for a suspend. Explicit Tauri resume events still recover shorter
// sleeps; the gap fallback covers event-loop resumes that provide no signal.
const SLEEP_GAP: Duration = Duration::from_secs(10);
const CHECKPOINT_GRACE: Duration = Duration::from_millis(250);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(1);
const REQUEST_ID_START: u64 = 0x4000_0000;

#[derive(Clone, Copy, Debug)]
enum PlatformSignal {
    Resumed,
    Stop,
}

/// A small, stoppable host worker that converts platform observations into
/// typed Job execution-environment updates.
pub struct PlatformSignalController {
    sender: Sender<PlatformSignal>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl PlatformSignalController {
    pub fn start(transport: TauriTransport) -> Self {
        Self::start_with_source(transport, Box::new(DefaultPlatformSignalSource))
    }

    pub fn start_with_source(
        transport: TauriTransport,
        source: Box<dyn PlatformSignalSource>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let join = thread::Builder::new()
            .name("velvet-platform-signals".into())
            .spawn(move || run_worker(transport, source, receiver, worker_stop))
            .ok();
        Self { sender, stop, join }
    }

    /// Called by the Tauri run loop when the host resumes.  The worker emits a
    /// short suspend transition so active Jobs can checkpoint before they are
    /// resumed with the current network sample.
    pub fn on_resumed(&self) {
        let _ = self.sender.send(PlatformSignal::Resumed);
    }
}

impl Drop for PlatformSignalController {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.sender.send(PlatformSignal::Stop);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Injectable platform source used by release tests and by the native
/// adapter.  It deliberately exposes only the network classification needed
/// by Job policy; sleep/wake is supplied by the host run loop or gap detector.
pub trait PlatformSignalSource: Send {
    fn network_state(&mut self) -> JobNetworkState;
}

struct DefaultPlatformSignalSource;

impl PlatformSignalSource for DefaultPlatformSignalSource {
    fn network_state(&mut self) -> JobNetworkState {
        detect_network_state()
    }
}

fn run_worker(
    transport: TauriTransport,
    mut source: Box<dyn PlatformSignalSource>,
    receiver: mpsc::Receiver<PlatformSignal>,
    stop: Arc<AtomicBool>,
) {
    let mut last_environment = None;
    let mut request_id = REQUEST_ID_START;
    let mut last_tick = Instant::now();

    apply_environment(
        &transport,
        &mut last_environment,
        &mut request_id,
        JobExecutionEnvironment {
            suspended: false,
            network: source.network_state(),
        },
        false,
    );

    while !stop.load(Ordering::Acquire) {
        match receiver.recv_timeout(POLL_INTERVAL) {
            Ok(PlatformSignal::Resumed) => {
                apply_environment(
                    &transport,
                    &mut last_environment,
                    &mut request_id,
                    JobExecutionEnvironment {
                        suspended: true,
                        network: source.network_state(),
                    },
                    true,
                );
                thread::sleep(CHECKPOINT_GRACE);
                apply_environment(
                    &transport,
                    &mut last_environment,
                    &mut request_id,
                    JobExecutionEnvironment {
                        suspended: false,
                        network: source.network_state(),
                    },
                    true,
                );
                last_tick = Instant::now();
            }
            Ok(PlatformSignal::Stop) => break,
            Err(RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                let woke_from_gap = is_sleep_gap(now.duration_since(last_tick));
                if woke_from_gap {
                    apply_environment(
                        &transport,
                        &mut last_environment,
                        &mut request_id,
                        JobExecutionEnvironment {
                            suspended: true,
                            network: source.network_state(),
                        },
                        true,
                    );
                    thread::sleep(CHECKPOINT_GRACE);
                }
                apply_environment(
                    &transport,
                    &mut last_environment,
                    &mut request_id,
                    JobExecutionEnvironment {
                        suspended: false,
                        network: source.network_state(),
                    },
                    woke_from_gap,
                );
                last_tick = now;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn is_sleep_gap(elapsed: Duration) -> bool {
    elapsed >= SLEEP_GAP
}

fn apply_environment(
    transport: &TauriTransport,
    last_environment: &mut Option<JobExecutionEnvironment>,
    request_id: &mut u64,
    environment: JobExecutionEnvironment,
    force: bool,
) {
    if !force && last_environment.is_some_and(|last| last == environment) {
        return;
    }
    *request_id = request_id.saturating_add(1);
    let _ = transport.dispatch(
        RequestId(*request_id),
        Request::Command(CoreCommand::SetJobExecutionEnvironment { environment }),
    );
    *last_environment = Some(environment);
}

fn detect_network_state() -> JobNetworkState {
    #[cfg(target_os = "linux")]
    {
        if let Some(output) = run_fixed_command("/usr/bin/nmcli", &["-t", "device", "show"]) {
            if let Some(state) = parse_linux_nmcli(&output) {
                return state;
            }
        }
        if let Some(output) = run_fixed_command("/usr/sbin/ip", &["route", "show", "default"]) {
            return parse_default_route(&output);
        }
        JobNetworkState::Unknown
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(output) = run_fixed_command("/usr/sbin/scutil", &["--nwi"]) {
            if let Some(state) = parse_macos_scutil(&output) {
                return state;
            }
        }
        if let Some(output) = run_fixed_command("/sbin/route", &["-n", "get", "default"]) {
            return parse_default_route(&output);
        }
        JobNetworkState::Unknown
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        JobNetworkState::Unknown
    }
}

fn run_fixed_command(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    loop {
        match child.try_wait().ok()? {
            Some(status) if status.success() => {
                let output = child.wait_with_output().ok()?;
                return Some(
                    String::from_utf8_lossy(&output.stdout)
                        .chars()
                        .take(16 * 1024)
                        .collect(),
                );
            }
            Some(_) => return None,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn parse_default_route(output: &str) -> JobNetworkState {
    if output.lines().any(|line| {
        let lower = line.trim().to_ascii_lowercase();
        lower.split_whitespace().next() == Some("default")
            || lower.contains("route to: default")
            || lower.contains("destination: default")
    }) {
        JobNetworkState::Online
    } else {
        JobNetworkState::Offline
    }
}

#[cfg(target_os = "linux")]
fn parse_linux_nmcli(output: &str) -> Option<JobNetworkState> {
    let lower = output.to_ascii_lowercase();
    let disconnected = lower.contains("disconnected") || lower.contains("unavailable");
    let connected = lower.lines().any(|line| {
        line.contains("(connected") || line.contains(":connected") || line.contains(" activated")
    });
    if disconnected && !connected {
        return Some(JobNetworkState::Offline);
    }
    if connected && (lower.contains("metered:yes") || lower.contains("metered yes")) {
        return Some(JobNetworkState::Metered);
    }
    if connected {
        return Some(JobNetworkState::Online);
    }
    None
}

#[cfg(target_os = "macos")]
fn parse_macos_scutil(output: &str) -> Option<JobNetworkState> {
    let lower = output.to_ascii_lowercase();
    if lower.contains("not reachable") || lower.contains("unreachable") {
        return Some(JobNetworkState::Offline);
    }
    if lower.contains("metered") || lower.contains("expensive") || lower.contains("low data") {
        return Some(JobNetworkState::Metered);
    }
    lower
        .contains("reachable")
        .then_some(JobNetworkState::Online)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_route_parser_distinguishes_offline_without_shelling_out() {
        assert_eq!(
            parse_default_route("default via 192.168.1.1 dev en0"),
            JobNetworkState::Online
        );
        assert_eq!(
            parse_default_route("route to: default\ndestination: default"),
            JobNetworkState::Online
        );
        assert_eq!(parse_default_route(""), JobNetworkState::Offline);
    }

    #[test]
    fn normal_poll_interval_is_not_treated_as_a_sleep_gap() {
        assert!(!is_sleep_gap(Duration::from_secs(5)));
        assert!(is_sleep_gap(Duration::from_secs(10)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_nmcli_parser_handles_metered_and_disconnected() {
        assert_eq!(
            parse_linux_nmcli("GENERAL.STATE:100 (connected)\nGENERAL.METERED:yes"),
            Some(JobNetworkState::Metered)
        );
        assert_eq!(
            parse_linux_nmcli("GENERAL.STATE:30 (disconnected)"),
            Some(JobNetworkState::Offline)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_scutil_parser_handles_reachability() {
        assert_eq!(
            parse_macos_scutil("Reachable\nFlags: 0x00000002"),
            Some(JobNetworkState::Online)
        );
        assert_eq!(
            parse_macos_scutil("Not Reachable"),
            Some(JobNetworkState::Offline)
        );
    }
}
