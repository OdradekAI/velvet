use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;
use velvet_tauri_adapter::TauriTransport;
use velvet_workbench_protocol::{ReleaseChannel, ReleaseManifestDto};

pub const UPDATE_COORDINATOR_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeUpdateState {
    Disabled,
    NoUpdate,
    Available,
    Installing,
    RestartRequired,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeUpdateStatus {
    pub schema_version: u16,
    pub state: NativeUpdateState,
    pub channel: ReleaseChannel,
    pub current_version: String,
    pub version: Option<String>,
    pub target: Option<String>,
    pub release_sequence: Option<u64>,
    pub transaction_id: Option<String>,
    pub notes: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateCoordinatorError {
    Disabled(String),
    Updater(String),
    Manifest(String),
    Core(String),
    Stale(String),
    Busy,
    Install(String),
}

impl std::fmt::Display for UpdateCoordinatorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled(message) => write!(formatter, "updater is disabled: {message}"),
            Self::Updater(message) => write!(formatter, "updater request failed: {message}"),
            Self::Manifest(message) => write!(formatter, "release manifest is invalid: {message}"),
            Self::Core(message) => write!(formatter, "release core rejected the update: {message}"),
            Self::Stale(message) => write!(formatter, "update is stale: {message}"),
            Self::Busy => formatter.write_str("an update installation is already in progress"),
            Self::Install(message) => write!(formatter, "update installation failed: {message}"),
        }
    }
}

impl std::error::Error for UpdateCoordinatorError {}

#[derive(Clone)]
pub struct UpdateCoordinator {
    channel: ReleaseChannel,
    current_version: String,
    next_transaction_id: Arc<AtomicU64>,
    installing: Arc<AtomicBool>,
}

impl UpdateCoordinator {
    pub fn new(current_version: impl Into<String>, channel: ReleaseChannel) -> Self {
        Self {
            channel,
            current_version: current_version.into(),
            next_transaction_id: Arc::new(AtomicU64::new(1)),
            installing: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn channel(&self) -> &ReleaseChannel {
        &self.channel
    }

    pub async fn check<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        transport: &TauriTransport,
    ) -> Result<NativeUpdateStatus, UpdateCoordinatorError> {
        match self.discover(app, transport).await {
            Ok(Some((update, manifest))) => Ok(self.available_status(&update, &manifest)),
            Ok(None) => Ok(self.base_status(NativeUpdateState::NoUpdate, None)),
            Err(UpdateCoordinatorError::Disabled(message)) => Ok(self.base_status(
                NativeUpdateState::Disabled,
                Some(message),
            )),
            Err(error) => Ok(self.base_status(
                NativeUpdateState::Failed,
                Some(error.to_string()),
            )),
        }
    }

    pub async fn install<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        transport: &TauriTransport,
        expected_version: &str,
    ) -> Result<NativeUpdateStatus, UpdateCoordinatorError> {
        if self.installing.swap(true, Ordering::AcqRel) {
            return Err(UpdateCoordinatorError::Busy);
        }
        let _install_guard = InstallGuard(self.installing.clone());
        let Some((update, manifest)) = self.discover(app, transport).await? else {
            return Err(UpdateCoordinatorError::Stale(
                "no update is currently available".into(),
            ));
        };
        if normalize_version(&update.version) != normalize_version(expected_version)
            || normalize_version(&manifest.version) != normalize_version(expected_version)
        {
            return Err(UpdateCoordinatorError::Stale(format!(
                "requested {}, discovered {}",
                expected_version, manifest.version
            )));
        }

        let bytes = update
            .download(|_, _| {}, || {})
            .await
            .map_err(|error| UpdateCoordinatorError::Updater(error.to_string()))?;
        let now_ms = now_ms();
        transport
            .validate_release_manifest(&manifest, now_ms, &release_target())
            .map_err(|error| UpdateCoordinatorError::Core(format!("{error:?}")))?;

        let transaction_id = self.next_transaction_id();
        let staged_package_id = transport
            .stage_release_package_bytes(
                &transaction_id,
                &bytes,
                &manifest.artifact_sha256,
            )
            .map_err(|error| UpdateCoordinatorError::Core(format!("{error:?}")))?;
        let package_generation = format!(
            "release-{}-{}",
            manifest.release_sequence,
            normalize_token(&manifest.version)
        );
        if let Err(error) = transport.stage_release(
            transaction_id.clone(),
            manifest.clone(),
            staged_package_id.clone(),
            package_generation,
        ) {
            let _ = transport.discard_staged_release_package(&staged_package_id);
            return Err(UpdateCoordinatorError::Core(format!("{error:?}")));
        }

        if let Err(error) = update.install(&bytes) {
            let rollback_reason = format!("Tauri updater install failed: {error}");
            let _ = transport.rollback_release(&transaction_id, rollback_reason.clone());
            return Err(UpdateCoordinatorError::Install(rollback_reason));
        }

        // The journal is committed only by the next process after it reaches
        // the native readiness barrier. This request lets Tauri relaunch the
        // newly installed image on platforms where install() does not exit.
        app.request_restart();
        #[allow(unreachable_code)]
        Ok(NativeUpdateStatus {
            schema_version: UPDATE_COORDINATOR_SCHEMA_VERSION,
            state: NativeUpdateState::RestartRequired,
            channel: self.channel.clone(),
            current_version: self.current_version.clone(),
            version: Some(manifest.version),
            target: Some(manifest.target),
            release_sequence: Some(manifest.release_sequence),
            transaction_id: Some(transaction_id),
            notes: update.body,
            message: Some("The update was installed; Velvet will restart and commit health after workbench-ready.".into()),
        })
    }

    async fn discover<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        transport: &TauriTransport,
    ) -> Result<Option<(tauri_plugin_updater::Update, ReleaseManifestDto)>, UpdateCoordinatorError>
    {
        let updater = app
            .updater_builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| UpdateCoordinatorError::Disabled(error.to_string()))?;
        let Some(update) = updater
            .check()
            .await
            .map_err(|error| match error {
                tauri_plugin_updater::Error::EmptyEndpoints => {
                    UpdateCoordinatorError::Disabled(error.to_string())
                }
                other => UpdateCoordinatorError::Updater(other.to_string()),
            })?
        else {
            return Ok(None);
        };
        let manifest = parse_release_manifest(
            &update.raw_json,
            &update.target,
            Some(update.download_url.as_str()),
        )?;
        if normalize_version(&update.version) != normalize_version(&manifest.version) {
            return Err(UpdateCoordinatorError::Manifest(
                "Tauri update version and Velvet manifest version differ".into(),
            ));
        }
        if manifest.channel != self.channel {
            return Err(UpdateCoordinatorError::Manifest(
                "the release channel is not enabled for this installation".into(),
            ));
        }
        transport
            .validate_release_manifest(&manifest, now_ms(), &release_target())
            .map_err(|error| UpdateCoordinatorError::Core(format!("{error:?}")))?;
        Ok(Some((update, manifest)))
    }

    fn base_status(&self, state: NativeUpdateState, message: Option<String>) -> NativeUpdateStatus {
        NativeUpdateStatus {
            schema_version: UPDATE_COORDINATOR_SCHEMA_VERSION,
            state,
            channel: self.channel.clone(),
            current_version: self.current_version.clone(),
            version: None,
            target: None,
            release_sequence: None,
            transaction_id: None,
            notes: None,
            message,
        }
    }

    fn available_status(
        &self,
        update: &tauri_plugin_updater::Update,
        manifest: &ReleaseManifestDto,
    ) -> NativeUpdateStatus {
        NativeUpdateStatus {
            schema_version: UPDATE_COORDINATOR_SCHEMA_VERSION,
            state: NativeUpdateState::Available,
            channel: self.channel.clone(),
            current_version: self.current_version.clone(),
            version: Some(manifest.version.clone()),
            target: Some(manifest.target.clone()),
            release_sequence: Some(manifest.release_sequence),
            transaction_id: None,
            notes: update.body.clone(),
            message: None,
        }
    }

    fn next_transaction_id(&self) -> String {
        let sequence = self.next_transaction_id.fetch_add(1, Ordering::Relaxed);
        format!("updater-{}-{sequence}", std::process::id())
    }
}

pub fn parse_release_manifest(
    raw_json: &serde_json::Value,
    updater_target: &str,
    download_url: Option<&str>,
) -> Result<ReleaseManifestDto, UpdateCoordinatorError> {
    let manifests = raw_json.get("velvetManifests");
    let selected_platform = download_url.and_then(|url| {
        raw_json
            .get("platforms")
            .and_then(serde_json::Value::as_object)
            .and_then(|platforms| {
                platforms.iter().find_map(|(platform, value)| {
                    (value.get("url").and_then(serde_json::Value::as_str) == Some(url))
                        .then_some(platform.as_str())
                })
            })
    });
    let value = selected_platform
        .and_then(|platform| manifests.and_then(|entries| entries.get(platform)))
        .or_else(|| manifests.and_then(|entries| entries.get(updater_target)))
        .ok_or_else(|| {
            UpdateCoordinatorError::Manifest(format!(
                "latest.json has no velvetManifests entry for {updater_target}"
            ))
        })?;
    serde_json::from_value(value.clone())
        .map_err(|error| UpdateCoordinatorError::Manifest(error.to_string()))
}

fn normalize_version(version: &str) -> &str {
    version.strip_prefix('v').unwrap_or(version)
}

fn normalize_token(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn release_target() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

struct InstallGuard(Arc<AtomicBool>);

impl Drop for InstallGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_read_from_tauri_static_json_extension() {
        let raw = serde_json::json!({
            "version": "0.0.5",
            "platforms": {},
            "velvetManifests": {
              "test-x86_64": {
                "schemaVersion": 1,
                "channel": "preview",
                "version": "0.0.5",
                "target": "test-x86_64",
                "artifactSha256": "a".repeat(64),
                "updaterSchema": 1,
                "minDataSchema": 1,
                "maxDataSchema": 1,
                "minClientVersion": "0.0.1",
                "releaseSequence": 2,
                "issuedAtMs": 1,
                "expiresAtMs": 2,
                "signingKeyId": "preview",
                "signature": "b".repeat(128)
              }
            }
        });
        let manifest = parse_release_manifest(&raw, "test-x86_64", None).expect("manifest");
        assert_eq!(manifest.version, "0.0.5");
        assert_eq!(manifest.release_sequence, 2);
    }

    #[test]
    fn missing_manifest_fails_closed() {
        let error = parse_release_manifest(
            &serde_json::json!({"version": "0.0.5"}),
            "darwin-aarch64",
            None,
        )
            .expect_err("missing manifest should fail");
        assert!(error.to_string().contains("no velvetManifests entry"));
    }

    #[test]
    fn wrong_target_does_not_fall_back_to_another_platform_manifest() {
        let raw = serde_json::json!({
            "velvetManifests": {
                "darwin-aarch64": {
                    "schemaVersion": 1,
                    "channel": "preview",
                    "version": "0.0.5",
                    "target": "macos-aarch64",
                    "artifactSha256": "a".repeat(64),
                    "updaterSchema": 1,
                    "minDataSchema": 1,
                    "maxDataSchema": 1,
                    "minClientVersion": "0.0.1",
                    "releaseSequence": 2,
                    "issuedAtMs": 1,
                    "expiresAtMs": 2,
                    "signingKeyId": "preview",
                    "signature": "b".repeat(128)
                }
            }
        });
        let error = parse_release_manifest(&raw, "linux-x86_64", None)
            .expect_err("target must be bound");
        assert!(error.to_string().contains("linux-x86_64"));
    }

    #[test]
    fn selected_installer_manifest_is_bound_to_the_download_url() {
        let raw = serde_json::json!({
            "platforms": {
                "windows-x86_64": {"url": "https://example.test/msi.zip"},
                "windows-x86_64-msi": {"url": "https://example.test/msi.zip"},
                "windows-x86_64-nsis": {"url": "https://example.test/nsis.zip"}
            },
            "velvetManifests": {
                "windows-x86_64": manifest_json("windows-x86_64", "base"),
                "windows-x86_64-msi": manifest_json("windows-x86_64", "msi"),
                "windows-x86_64-nsis": manifest_json("windows-x86_64", "nsis")
            }
        });
        let manifest = parse_release_manifest(
            &raw,
            "windows-x86_64",
            Some("https://example.test/nsis.zip"),
        )
        .expect("installer manifest");
        assert_eq!(manifest.signing_key_id, "nsis");
    }

    fn manifest_json(target: &str, signing_key_id: &str) -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": 1,
            "channel": "preview",
            "version": "0.0.5",
            "target": target,
            "artifactSha256": "a".repeat(64),
            "updaterSchema": 1,
            "minDataSchema": 1,
            "maxDataSchema": 1,
            "minClientVersion": "0.0.1",
            "releaseSequence": 2,
            "issuedAtMs": 1,
            "expiresAtMs": 2,
            "signingKeyId": signing_key_id,
            "signature": "b".repeat(128)
        })
    }

    #[test]
    fn status_has_stable_wire_shape() {
        let status = UpdateCoordinator::new("0.0.5", ReleaseChannel::Preview)
            .base_status(NativeUpdateState::Disabled, Some("not configured".into()));
        let json = serde_json::to_value(status).expect("status should serialize");
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["state"], "disabled");
        assert_eq!(json["channel"], "preview");
    }
}
