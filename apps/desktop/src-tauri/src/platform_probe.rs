use std::env;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use velvet_workbench_protocol::{
    PlatformCapability, PlatformCapabilityStatus, PlatformPreflightEvidence,
};

const MAX_DETAIL_BYTES: usize = 512;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlatformPreflightReport {
    pub evidence: Vec<PlatformPreflightEvidence>,
}

impl PlatformPreflightReport {
    pub fn webview_unavailable(&self) -> bool {
        self.evidence.iter().any(|evidence| {
            evidence.capability == PlatformCapability::Webview
                && evidence.status == PlatformCapabilityStatus::Unavailable
        })
    }
}

pub trait PlatformPreflightProbe: Send + Sync {
    fn probe(&self) -> PlatformPreflightReport;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HostPlatformPreflightProbe;

impl PlatformPreflightProbe for HostPlatformPreflightProbe {
    fn probe(&self) -> PlatformPreflightReport {
        PlatformPreflightReport {
            evidence: vec![probe_webview(), probe_gpu()],
        }
    }
}

fn probe_webview() -> PlatformPreflightEvidence {
    let status = env_override("VELVET_WEBVIEW_PREFLIGHT").unwrap_or_else(|| {
        #[cfg(target_os = "macos")]
        {
            if Path::new("/System/Library/Frameworks/WebKit.framework").exists() {
                return PlatformCapabilityStatus::Available;
            }
            return PlatformCapabilityStatus::Unavailable;
        }
        #[cfg(target_os = "windows")]
        {
            if env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER").is_some()
                || env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_some()
                || windows_webview2_runtime_is_registered()
            {
                return PlatformCapabilityStatus::Available;
            }
            return PlatformCapabilityStatus::Unknown;
        }
        #[cfg(target_os = "linux")]
        {
            if env::var_os("WAYLAND_DISPLAY").is_some() || env::var_os("DISPLAY").is_some() {
                return PlatformCapabilityStatus::Available;
            }
            return PlatformCapabilityStatus::Unavailable;
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            PlatformCapabilityStatus::Unknown
        }
    });
    PlatformPreflightEvidence {
        capability: PlatformCapability::Webview,
        status: status.clone(),
        detail: bounded_detail(webview_detail(&status)),
        checked_at_ms: now_ms(),
    }
}

fn probe_gpu() -> PlatformPreflightEvidence {
    let status = env_override("VELVET_GPU_PREFLIGHT").unwrap_or_else(|| {
        #[cfg(target_os = "macos")]
        {
            if Path::new("/System/Library/Frameworks/Metal.framework").exists() {
                return PlatformCapabilityStatus::Available;
            }
            return PlatformCapabilityStatus::Degraded;
        }
        #[cfg(target_os = "linux")]
        {
            if Path::new("/dev/dri/renderD128").exists() || Path::new("/dev/dri/card0").exists() {
                return PlatformCapabilityStatus::Available;
            }
            return PlatformCapabilityStatus::Degraded;
        }
        #[cfg(target_os = "windows")]
        {
            PlatformCapabilityStatus::Unknown
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            PlatformCapabilityStatus::Unknown
        }
    });
    PlatformPreflightEvidence {
        capability: PlatformCapability::Gpu,
        status: status.clone(),
        detail: bounded_detail(gpu_detail(&status)),
        checked_at_ms: now_ms(),
    }
}

fn env_override(name: &str) -> Option<PlatformCapabilityStatus> {
    match env::var(name).ok()?.to_ascii_lowercase().as_str() {
        "available" => Some(PlatformCapabilityStatus::Available),
        "degraded" => Some(PlatformCapabilityStatus::Degraded),
        "unavailable" => Some(PlatformCapabilityStatus::Unavailable),
        "unknown" => Some(PlatformCapabilityStatus::Unknown),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn windows_webview2_runtime_is_registered() -> bool {
    [
        r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients",
        r"HKLM\SOFTWARE\Microsoft\EdgeUpdate\Clients",
        r"HKCU\Software\Microsoft\EdgeUpdate\Clients",
    ]
    .iter()
    .any(|key| {
        let Ok(output) = std::process::Command::new("reg")
            .args(["query", key, "/s", "/f", "WebView2", "/d"])
            .output()
        else {
            return false;
        };
        output.status.success()
            && String::from_utf8_lossy(&output.stdout)
                .to_ascii_lowercase()
                .contains("webview2")
    })
}

fn webview_detail(status: &PlatformCapabilityStatus) -> String {
    match status {
        PlatformCapabilityStatus::Available => "host WebView prerequisites detected".into(),
        PlatformCapabilityStatus::Degraded => "host WebView is available with reduced features".into(),
        PlatformCapabilityStatus::Unavailable => "host WebView prerequisites are unavailable".into(),
        PlatformCapabilityStatus::Unknown => "host WebView prerequisites could not be verified".into(),
    }
}

fn gpu_detail(status: &PlatformCapabilityStatus) -> String {
    match status {
        PlatformCapabilityStatus::Available => "hardware graphics prerequisites detected".into(),
        PlatformCapabilityStatus::Degraded => "hardware graphics are unavailable; software fallback is required".into(),
        PlatformCapabilityStatus::Unavailable => "hardware graphics prerequisites are unavailable".into(),
        PlatformCapabilityStatus::Unknown => "hardware graphics prerequisites could not be verified".into(),
    }
}

fn bounded_detail(value: String) -> String {
    value.chars().take(MAX_DETAIL_BYTES).collect()
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
    fn preflight_report_marks_webview_failure_as_fatal_but_gpu_degradation_as_recoverable() {
        let report = PlatformPreflightReport {
            evidence: vec![
                PlatformPreflightEvidence {
                    capability: PlatformCapability::Webview,
                    status: PlatformCapabilityStatus::Unavailable,
                    detail: "missing WebView".into(),
                    checked_at_ms: 1,
                },
                PlatformPreflightEvidence {
                    capability: PlatformCapability::Gpu,
                    status: PlatformCapabilityStatus::Degraded,
                    detail: "software fallback".into(),
                    checked_at_ms: 1,
                },
            ],
        };
        assert!(report.webview_unavailable());
    }

    #[test]
    fn unknown_status_is_not_treated_as_a_webview_failure() {
        let report = PlatformPreflightReport {
            evidence: vec![PlatformPreflightEvidence {
                capability: PlatformCapability::Webview,
                status: PlatformCapabilityStatus::Unknown,
                detail: "not verified".into(),
                checked_at_ms: 1,
            }],
        };
        assert!(!report.webview_unavailable());
    }
}
