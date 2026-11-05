//! Framework-free CoreTransport adapter with session and event policy.
//!
//! Native platform signals remain in the sibling platform_signals module; Tauri
//! framework types belong to the desktop application, not this adapter.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use velvet_application_core::{ApplicationCore, InProcessCoreTransport};
use velvet_workbench_protocol::{
    negotiate_protocol, request_idempotency_scope, Command, CoreError, CoreTransport, Event,
    PluginRequestEnvelope, PluginResponseEnvelope, ProtocolHandshakeRequest,
    ProtocolHandshakeResponse, Query, Request, RequestId, RequestOutcome, RequestOutcomeStatus,
    Response,
};

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
const MAX_REQUEST_OUTCOMES: usize = 128;
type RequestOutcomeKey = (String, u64);
type RequestOutcomeCache = VecDeque<(RequestOutcomeKey, Option<&'static str>, Option<Response>)>;

#[path = "platform_signals.rs"]
pub mod platform_signals;

pub trait CoreEventSink: Send + Sync {
    fn emit(&self, event: Event);
}

#[derive(Clone, Default)]
pub struct InMemoryEventSink {
    events: Arc<Mutex<Vec<Event>>>,
}

impl InMemoryEventSink {
    pub fn events(&self) -> Vec<Event> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }
}

impl CoreEventSink for InMemoryEventSink {
    fn emit(&self, event: Event) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

/// Adapter seam for Tauri commands and events.
///
/// The actual `tauri::State` and command macros belong in the desktop app.
/// Keeping this adapter free of Tauri types lets protocol tests run without a webview.
#[derive(Clone)]
pub struct TauriTransport {
    inner: InProcessCoreTransport,
    event_sink: Arc<dyn CoreEventSink>,
    sessions: Arc<Mutex<HashMap<String, String>>>,
    plugin_sessions: Arc<Mutex<HashMap<String, String>>>,
    request_outcomes: Arc<Mutex<RequestOutcomeCache>>,
    evicted_request_floor: Arc<Mutex<HashMap<String, u64>>>,
}

impl TauriTransport {
    pub fn new(core: ApplicationCore) -> Self {
        Self::with_event_sink(core, InMemoryEventSink::default())
    }

    pub fn with_event_sink<S>(core: ApplicationCore, event_sink: S) -> Self
    where
        S: CoreEventSink + 'static,
    {
        Self {
            inner: InProcessCoreTransport::new(core),
            event_sink: Arc::new(event_sink),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            plugin_sessions: Arc::new(Mutex::new(HashMap::new())),
            request_outcomes: Arc::new(Mutex::new(VecDeque::new())),
            evicted_request_floor: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn negotiate_handshake(
        &self,
        request: ProtocolHandshakeRequest,
    ) -> Result<ProtocolHandshakeResponse, CoreError> {
        let sequence = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
        let session_id = format!("core-{}-{sequence}", std::process::id());
        let client_id = request.client_id.clone();
        let response = negotiate_protocol(request, session_id)?;
        self.sessions
            .lock()
            .map_err(|_| CoreError::InvalidCommand("session registry is unavailable".into()))?
            .insert(client_id, response.session_id.clone());
        Ok(response)
    }

    pub fn validate_session(&self, session_id: Option<&str>) -> Result<(), CoreError> {
        let Some(session_id) = session_id else {
            // Keep the adapter usable for native recovery and in-process tests
            // that intentionally exercise the legacy envelope directly.
            return Ok(());
        };
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| CoreError::InvalidCommand("session registry is unavailable".into()))?;
        if sessions.values().any(|active| active == session_id) {
            Ok(())
        } else {
            Err(CoreError::SessionExpired {
                session_id: session_id.into(),
            })
        }
    }

    pub fn dispatch_session(
        &self,
        session_id: &str,
        request_id: RequestId,
        request: Request,
    ) -> Result<Response, CoreError> {
        self.validate_session(Some(session_id))?;
        let client_id = self
            .sessions
            .lock()
            .map_err(|_| CoreError::InvalidCommand("session registry is unavailable".into()))?
            .iter()
            .find_map(|(client, active)| (active == session_id).then_some(client.clone()))
            .ok_or_else(|| CoreError::SessionExpired {
                session_id: session_id.into(),
            })?;

        if let Request::Query(Query::RequestOutcome { request_id: target }) = &request {
            let completed = self
                .request_outcomes
                .lock()
                .map_err(|_| {
                    CoreError::InvalidCommand("request outcome store is unavailable".into())
                })?
                .iter()
                .any(|((owner, id), _, response)| {
                    owner == &client_id && *id == target.0 && response.is_some()
                });
            return Ok(Response::RequestOutcome(RequestOutcome {
                request_id: target.clone(),
                status: if completed {
                    RequestOutcomeStatus::Completed
                } else {
                    RequestOutcomeStatus::Unknown
                },
            }));
        }

        let cache_key = (client_id, request_id.0);
        let scope = request_idempotency_scope(&request);
        if let Some((_, existing_scope, response)) = self
            .request_outcomes
            .lock()
            .map_err(|_| CoreError::InvalidCommand("request outcome store is unavailable".into()))?
            .iter()
            .find(|((owner, id), _, _)| owner == &cache_key.0 && *id == request_id.0)
        {
            if let (Some(current_scope), Some(previous_scope), Some(response)) =
                (scope, *existing_scope, response.as_ref())
            {
                if current_scope == previous_scope {
                    return Ok(response.clone());
                }
            }
            // A request id is still a correlation key for commands that are
            // not safe to replay. Seeing it again must never dispatch the
            // command a second time, even when neither request declares an
            // idempotency scope.
            return Err(CoreError::RequestIdReuse {
                request_id: request_id.0,
            });
        }

        if matches!(&request, Request::Command(_))
            && self
                .evicted_request_floor
                .lock()
                .map_err(|_| {
                    CoreError::InvalidCommand("request outcome store is unavailable".into())
                })?
                .get(&cache_key.0)
                .is_some_and(|floor| request_id.0 <= *floor)
        {
            // Request ids are monotonically allocated by the desktop client.
            // Once the bounded result window evicts an older command, that id
            // remains consumed even though its detailed outcome is gone.
            return Err(CoreError::RequestIdReuse {
                request_id: request_id.0,
            });
        }

        let record_outcome = matches!(&request, Request::Command(_));
        let response = match self.dispatch(request_id, request) {
            Ok(response) => {
                if record_outcome {
                    self.remember_outcome(cache_key, scope, Some(response.clone()))?;
                }
                response
            }
            Err(error) => {
                if record_outcome {
                    // A command that crossed the Core boundary but did not
                    // produce a response is consumed as an unknown outcome.
                    // Retrying the same request id must not invoke it again.
                    let _ = self.remember_outcome(cache_key, scope, None);
                }
                return Err(error);
            }
        };
        Ok(response)
    }

    /// Dispatches a plugin-only envelope through the same session, outcome and
    /// event policy as every other CoreTransport request.
    pub fn dispatch_plugin_session(
        &self,
        envelope: PluginRequestEnvelope,
    ) -> Result<PluginResponseEnvelope, CoreError> {
        envelope.validate()?;
        let session_id = envelope.session_id.clone();
        self.validate_session(Some(&session_id))?;
        let bound_extension_id = self
            .plugin_sessions
            .lock()
            .map_err(|_| {
                CoreError::InvalidCommand("plugin session registry is unavailable".into())
            })?
            .get(&session_id)
            .cloned()
            .ok_or_else(|| CoreError::SessionExpired {
                session_id: session_id.clone(),
            })?;
        if bound_extension_id != envelope.caller_extension_id.0 {
            return Err(CoreError::InvalidCommand(
                "plugin session identity does not match the caller extension".into(),
            ));
        }
        let request_id = envelope.request_id.clone();
        let response =
            self.dispatch_session(&session_id, request_id.clone(), envelope.into_request())?;
        PluginResponseEnvelope::from_core(
            session_id,
            velvet_workbench_protocol::ResponseEnvelope {
                protocol_version: velvet_workbench_protocol::PROTOCOL_VERSION,
                request_id: request_id.clone(),
                response,
            },
        )
    }

    /// Binds a trusted runtime session to one extension before accepting
    /// plugin-only requests. Runtime adapters must derive the extension id
    /// from their host-owned lease/package identity rather than an untrusted
    /// plugin payload.
    pub fn bind_plugin_session(
        &self,
        session_id: &str,
        extension_id: String,
    ) -> Result<(), CoreError> {
        self.validate_session(Some(session_id))?;
        self.plugin_sessions
            .lock()
            .map_err(|_| {
                CoreError::InvalidCommand("plugin session registry is unavailable".into())
            })?
            .insert(session_id.into(), extension_id);
        Ok(())
    }

    pub fn unbind_plugin_session(&self, session_id: &str) -> Result<(), CoreError> {
        self.plugin_sessions
            .lock()
            .map_err(|_| {
                CoreError::InvalidCommand("plugin session registry is unavailable".into())
            })?
            .remove(session_id);
        Ok(())
    }

    fn remember_outcome(
        &self,
        cache_key: RequestOutcomeKey,
        scope: Option<&'static str>,
        response: Option<Response>,
    ) -> Result<(), CoreError> {
        let mut outcomes = self.request_outcomes.lock().map_err(|_| {
            CoreError::InvalidCommand("request outcome store is unavailable".into())
        })?;
        outcomes.retain(|(key, _, _)| key != &cache_key);
        outcomes.push_back((cache_key, scope, response));
        while outcomes.len() > MAX_REQUEST_OUTCOMES {
            if let Some(((owner, request_id), _, _)) = outcomes.pop_front() {
                let mut floors = self.evicted_request_floor.lock().map_err(|_| {
                    CoreError::InvalidCommand("request outcome store is unavailable".into())
                })?;
                floors
                    .entry(owner)
                    .and_modify(|floor| *floor = (*floor).max(request_id))
                    .or_insert(request_id);
            }
        }
        Ok(())
    }

    pub fn record_shutdown_intent(&self) -> Result<(), velvet_application_core::RecoveryError> {
        self.inner.record_shutdown_intent()
    }

    pub fn record_shutdown_complete(&self) -> Result<(), velvet_application_core::RecoveryError> {
        self.inner.record_shutdown_complete()
    }

    pub fn stage_release_package_bytes(
        &self,
        transaction_id: &str,
        bytes: &[u8],
        expected_sha256: &str,
    ) -> Result<String, CoreError> {
        self.inner.with_core(|core| {
            core.stage_release_package_bytes(transaction_id, bytes, expected_sha256)
        })
    }

    pub fn stage_release(
        &self,
        transaction_id: String,
        manifest: velvet_workbench_protocol::ReleaseManifestDto,
        staged_package_id: String,
        package_generation: String,
    ) -> Result<velvet_workbench_protocol::ReleaseStatusDto, CoreError> {
        self.inner.with_core(|core| {
            core.stage_release(
                transaction_id,
                manifest,
                staged_package_id,
                package_generation,
            )
        })
    }

    pub fn validate_release_manifest(
        &self,
        manifest: &velvet_workbench_protocol::ReleaseManifestDto,
        now_ms: u64,
        expected_target: &str,
    ) -> Result<(), CoreError> {
        self.inner
            .with_core(|core| core.validate_release_manifest(manifest, now_ms, expected_target))
    }

    pub fn discard_staged_release_package(&self, staged_package_id: &str) -> Result<(), CoreError> {
        self.inner
            .with_core(|core| core.discard_staged_release_package(staged_package_id))
    }

    pub fn commit_release_health(
        &self,
        transaction_id: &str,
    ) -> Result<velvet_workbench_protocol::ReleaseStatusDto, CoreError> {
        self.inner
            .with_core(|core| core.commit_release_health(transaction_id, "workbench-ready"))
    }

    pub fn rollback_release(
        &self,
        transaction_id: &str,
        reason: String,
    ) -> Result<velvet_workbench_protocol::ReleaseStatusDto, CoreError> {
        self.inner
            .with_core(|core| core.rollback_release(transaction_id, reason))
    }

    pub fn release_status(&self) -> Result<velvet_workbench_protocol::ReleaseStatusDto, CoreError> {
        self.inner.with_core(|core| core.release_status())
    }
}

impl CoreTransport for TauriTransport {
    fn dispatch(&self, request_id: RequestId, request: Request) -> Result<Response, CoreError> {
        let emits_layout_event = matches!(
            &request,
            Request::Command(Command::Layout(_)) | Request::Command(Command::LayoutAt { .. })
        );
        let emits_profile_event = matches!(
            &request,
            Request::Command(Command::SetLocale { .. })
                | Request::Command(Command::SetPluginDeveloperMode { .. })
        );
        let emits_recovery_event = matches!(
            &request,
            Request::Query(velvet_workbench_protocol::Query::Recovery)
        );
        let disabled_extension = match &request {
            Request::Command(Command::DisablePlugin {
                extension_id,
                reason,
            }) => Some((extension_id.0.clone(), disable_reason_name(reason))),
            _ => None,
        };
        let response = match self.inner.dispatch(request_id, request) {
            Ok(response) => response,
            Err(error) => {
                self.emit_pending_diagnostics();
                if let CoreError::PluginRuntimeFailed { extension_id, .. } = &error {
                    self.event_sink.emit(Event::PluginDisabled {
                        extension_id: extension_id.clone(),
                        reason: "crash".into(),
                    });
                    self.event_sink.emit(Event::RecoveryChanged {
                        projection: self.inner.recovery_projection(),
                    });
                }
                return Err(error);
            }
        };

        for snapshot in self.inner.drain_job_events() {
            self.event_sink.emit(Event::JobChanged {
                snapshot: Box::new(snapshot),
            });
        }
        self.emit_pending_diagnostics();

        if emits_layout_event {
            if let Response::Layout(state) = &response {
                self.event_sink.emit(Event::LayoutChanged {
                    revision: state.revision,
                    state: state.clone(),
                });
            }
        }

        if emits_profile_event {
            if let Response::Profile(state) = &response {
                self.event_sink.emit(Event::ProfileChanged {
                    revision: state.revision,
                    state: state.clone(),
                });
            }
        }

        if emits_recovery_event {
            if let Response::Recovery(projection) = &response {
                self.event_sink.emit(Event::RecoveryChanged {
                    projection: projection.clone(),
                });
            }
        }

        if let (Some((extension_id, reason)), Response::Extension(summary)) =
            (disabled_extension, &response)
        {
            if !summary.active {
                self.event_sink.emit(Event::PluginDisabled {
                    extension_id,
                    reason,
                });
            }
        }

        Ok(response)
    }
}

impl TauriTransport {
    fn emit_pending_diagnostics(&self) {
        for diagnostic in self.inner.drain_plugin_diagnostics() {
            self.event_sink.emit(Event::PluginDiagnostic { diagnostic });
        }
    }
}

fn disable_reason_name(reason: &velvet_workbench_protocol::PluginDisableReason) -> String {
    match reason {
        velvet_workbench_protocol::PluginDisableReason::User => "user".into(),
        velvet_workbench_protocol::PluginDisableReason::Crash => "crash".into(),
        velvet_workbench_protocol::PluginDisableReason::Incompatible => "incompatible".into(),
    }
}

#[cfg(test)]
#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
