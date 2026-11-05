use std::sync::Arc;

use super::*;
use crate::platform_signals::{PlatformSignalController, PlatformSignalSource};
use velvet_layout_model::{LayoutCommand, WorkbenchPart};
use velvet_persistence::InMemoryLayoutStateStore;
use velvet_plugin_api::{
    ContributionDescriptor, ContributionKind, ContributionScope, ExtensionId, ExtensionManifest,
    RuntimeKind, RuntimeLaunchSpec, VersionRange,
};
use velvet_plugin_host::{
    PluginInvocation, PluginInvocationResult, PluginRuntime, RuntimeError, RuntimeHealth,
    RuntimeProcess,
};
use velvet_workbench_protocol::{PluginDisableReason, Query};

struct OfflineSource;

impl PlatformSignalSource for OfflineSource {
    fn network_state(&mut self) -> velvet_job_system::JobNetworkState {
        velvet_job_system::JobNetworkState::Offline
    }
}

struct CrashingRuntime;

struct CrashingProcess;

impl RuntimeProcess for CrashingProcess {
    fn poll(&mut self) -> RuntimeHealth {
        RuntimeHealth::Running
    }

    fn shutdown(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn invoke(
        &mut self,
        _invocation: &PluginInvocation,
    ) -> Result<PluginInvocationResult, RuntimeError> {
        Err(RuntimeError::InvocationFailed("sidecar crashed".into()))
    }
}

impl PluginRuntime for CrashingRuntime {
    fn start(
        &mut self,
        _manifest: &ExtensionManifest,
        _permissions: &velvet_plugin_host::PermissionGrant,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
        Ok(Box::new(CrashingProcess))
    }
}

#[test]
fn successful_layout_commands_emit_authoritative_events() {
    let sink = InMemoryEventSink::default();
    let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
    let response = transport
        .dispatch(
            RequestId(1),
            Request::Command(Command::LayoutAt {
                expected_revision: Some(0),
                command: LayoutCommand::SetPartVisibility {
                    part: WorkbenchPart::PrimarySidebar,
                    visible: false,
                },
            }),
        )
        .expect("layout command should succeed");

    assert!(matches!(response, Response::Layout(state) if state.revision == 1));
    assert!(matches!(
        sink.events().as_slice(),
        [Event::LayoutChanged { revision: 1, .. }]
    ));
}

#[test]
fn rejected_layout_commands_do_not_emit_events() {
    let sink = InMemoryEventSink::default();
    let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
    let error = transport
        .dispatch(
            RequestId(1),
            Request::Command(Command::LayoutAt {
                expected_revision: Some(9),
                command: LayoutCommand::ResetLayout,
            }),
        )
        .expect_err("stale layout command should fail");

    assert!(matches!(error, CoreError::StaleRevision { .. }));
    assert!(sink.events().is_empty());
}

#[test]
fn locale_commands_emit_authoritative_profile_events() {
    let sink = InMemoryEventSink::default();
    let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
    let response = transport
        .dispatch(
            RequestId(1),
            Request::Command(Command::SetLocale {
                locale: "zh-Hans".into(),
                expected_revision: Some(0),
            }),
        )
        .expect("locale command should succeed");

    assert!(matches!(
        response,
        Response::Profile(state) if state.locale == "zh-Hans" && state.revision == 1
    ));
    assert!(matches!(
        sink.events().as_slice(),
        [Event::ProfileChanged { revision: 1, state }] if state.locale == "zh-Hans"
    ));
}

#[test]
fn plugin_developer_mode_commands_emit_authoritative_profile_events() {
    let sink = InMemoryEventSink::default();
    let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
    let response = transport
        .dispatch(
            RequestId(2),
            Request::Command(Command::SetPluginDeveloperMode {
                enabled: false,
                expected_revision: Some(0),
            }),
        )
        .expect("Profile trust mode command should succeed");

    assert!(matches!(
        response,
        Response::Profile(state)
            if state.plugin_developer_mode == Some(false) && state.revision == 1
    ));
    assert!(matches!(
        sink.events().as_slice(),
        [Event::ProfileChanged { revision: 1, state }]
            if state.plugin_developer_mode == Some(false)
    ));
}

#[test]
fn recovery_query_projects_the_same_typed_contract_as_rust_core() {
    let sink = InMemoryEventSink::default();
    let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
    let response = transport
        .dispatch(RequestId(10), Request::Query(Query::Recovery))
        .expect("recovery query should succeed");

    assert!(matches!(response, Response::Recovery(projection) if projection.schema_version == 1));
    assert!(matches!(
        sink.events().as_slice(),
        [Event::RecoveryChanged { projection }] if projection.schema_version == 1
    ));
}

#[test]
fn shutdown_hooks_close_the_core_launch_receipt() {
    let transport = TauriTransport::new(ApplicationCore::default());
    transport
        .record_shutdown_intent()
        .expect("shutdown intent should be accepted");
    transport
        .record_shutdown_complete()
        .expect("shutdown completion should be accepted");

    let response = transport
        .dispatch(RequestId(11), Request::Query(Query::Recovery))
        .expect("recovery query should succeed");
    assert!(matches!(
        response,
        Response::Recovery(projection)
            if projection.mode == velvet_workbench_protocol::RecoveryMode::Ready
    ));
}

#[test]
fn disabling_a_plugin_emits_a_lifecycle_event() {
    let sink = InMemoryEventSink::default();
    let mut core = ApplicationCore::default();
    let manifest = ExtensionManifest {
        id: ExtensionId("sample.reader".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        launch: None,
        target_app: None,
        contributions: vec![],
        permission_sets: vec![],
        ..ExtensionManifest::default()
    };
    let id = manifest.id.clone();
    core.register_plugin(manifest)
        .expect("plugin should register");
    core.activate_plugin(&id).expect("plugin should activate");
    let transport = TauriTransport::with_event_sink(core, sink.clone());

    transport
        .dispatch(
            RequestId(2),
            Request::Command(Command::DisablePlugin {
                extension_id: id,
                reason: PluginDisableReason::User,
            }),
        )
        .expect("plugin disable should succeed");

    assert!(matches!(
        sink.events().as_slice(),
        [Event::PluginDisabled { extension_id, reason }] if extension_id == "sample.reader"
            && reason == "user"
    ));
}

#[test]
fn runtime_command_failure_emits_a_crash_event_for_frontend_cleanup() {
    let sink = InMemoryEventSink::default();
    let mut core = ApplicationCore::with_plugin_runtime(
        Arc::new(InMemoryLayoutStateStore::default()),
        Box::new(CrashingRuntime),
    );
    let manifest = ExtensionManifest {
        id: ExtensionId("sample.crashing-sidecar".into()),
        version: "1.0.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::Sidecar,
        launch: Some(RuntimeLaunchSpec {
            program: "test-sidecar".into(),
            args: Vec::new(),
        }),
        target_app: None,
        contributions: vec![ContributionDescriptor {
            id: "echo.crash".into(),
            kind: ContributionKind::Command,
            scope: ContributionScope::Workbench,
            namespace: Some("workbench".into()),
            container: None,
            zone: None,
            title: Some("Crash Echo Runtime".into()),
            title_key: None,
            icon: None,
            order: None,
            default_visible: None,
            view_container_id: None,
            activity_bar: None,
            label: None,
            label_key: None,
            command_id: None,
            group: None,
            when: None,
            anchor: None,
            ui: None,
            preview: None,
        }],
        permission_sets: vec![],
        ..ExtensionManifest::default()
    };
    let id = manifest.id.clone();
    core.register_plugin(manifest)
        .expect("the crashing sidecar should register");
    core.activate_plugin(&id)
        .expect("the crashing sidecar should start");
    let transport = TauriTransport::with_event_sink(core, sink.clone());

    let error = transport
        .dispatch(
            RequestId(3),
            Request::Command(Command::InvokePluginCommand {
                extension_id: id.clone(),
                command_id: "echo.crash".into(),
                args: serde_json::Value::Null,
                context: serde_json::Value::Null,
            }),
        )
        .expect_err("the crashing sidecar should fail its command");
    assert!(matches!(
        error,
        CoreError::PluginRuntimeFailed { extension_id, .. } if extension_id == id.0
    ));
    let events = sink.events();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::PluginDisabled { extension_id, reason }
            if extension_id == "sample.crashing-sidecar" && reason == "crash"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        Event::RecoveryChanged { projection }
            if projection.mode == velvet_workbench_protocol::RecoveryMode::Degraded
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        Event::PluginDiagnostic { diagnostic }
            if diagnostic.extension_id == "sample.crashing-sidecar"
                && diagnostic.kind == velvet_workbench_protocol::PluginDiagnosticKind::Runtime
                && diagnostic.retryable
    )));
}

#[test]
fn job_commands_emit_progress_and_terminal_events() {
    let sink = InMemoryEventSink::default();
    let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
    let response = transport
        .dispatch(
            RequestId(4),
            Request::Command(Command::CreateJob {
                kind: "sample".into(),
                duration_ms: 200,
                will_fail: false,
                input: serde_json::Value::Null,
                policy: velvet_job_system::JobExecutionPolicy::default(),
                budget: velvet_job_system::JobBudgetRequest::default(),
            }),
        )
        .expect("job should be accepted");
    let job_id = match response {
        Response::Job(snapshot) => snapshot.id,
        other => panic!("expected job response, got {other:?}"),
    };
    std::thread::sleep(std::time::Duration::from_millis(80));
    let _ = transport.dispatch(RequestId(5), Request::Query(Query::Jobs));
    let events = sink.events();
    assert!(events.iter().any(|event| matches!(event, Event::JobChanged { snapshot } if snapshot.id == job_id && snapshot.progress > 0)));
    for request_id in 6..=20 {
        std::thread::sleep(std::time::Duration::from_millis(40));
        let _ = transport.dispatch(RequestId(request_id), Request::Query(Query::Jobs));
        if sink.events().iter().any(|event| matches!(event, Event::JobChanged { snapshot } if snapshot.id == job_id && snapshot.status == velvet_job_system::JobStatus::Completed)) {
                return;
            }
    }
    panic!("job never emitted a completed event: {:?}", sink.events());
}

#[test]
fn job_environment_command_pauses_policy_blocked_work_and_emits_projection() {
    let sink = InMemoryEventSink::default();
    let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
    let response = transport
        .dispatch(
            RequestId(21),
            Request::Command(Command::CreateJob {
                kind: "networked".into(),
                duration_ms: 30_000,
                will_fail: false,
                input: serde_json::Value::Null,
                policy: velvet_job_system::JobExecutionPolicy {
                    network: velvet_job_system::JobNetworkPolicy::Unmetered,
                    ..velvet_job_system::JobExecutionPolicy::default()
                },
                budget: velvet_job_system::JobBudgetRequest::default(),
            }),
        )
        .expect("networked Job should be accepted");
    let job_id = match response {
        Response::Job(snapshot) => snapshot.id,
        other => panic!("expected job response, got {other:?}"),
    };
    let mut response = Response::Accepted;
    for attempt in 0..20 {
        response = transport
            .dispatch(
                RequestId(22 + attempt),
                Request::Command(Command::SetJobExecutionEnvironment {
                    environment: velvet_job_system::JobExecutionEnvironment {
                        suspended: false,
                        network: velvet_job_system::JobNetworkState::Offline,
                        ..velvet_job_system::JobExecutionEnvironment::default()
                    },
                }),
            )
            .expect("offline environment should be accepted");
        if matches!(
            response,
            Response::Jobs(ref jobs)
                if jobs.iter().any(|snapshot| snapshot.id == job_id
                    && snapshot.status == velvet_job_system::JobStatus::Paused)
        ) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(matches!(
        response,
        Response::Jobs(ref jobs)
            if jobs.iter().any(|snapshot| snapshot.id == job_id
                && snapshot.status == velvet_job_system::JobStatus::Paused)
    ));
    assert!(sink.events().iter().any(|event| matches!(
        event,
        Event::JobChanged { snapshot }
            if snapshot.id == job_id
                && snapshot.status == velvet_job_system::JobStatus::Paused
    )));
}

#[test]
fn platform_signal_controller_routes_fake_network_state_to_job_environment() {
    let sink = InMemoryEventSink::default();
    let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink);
    let controller =
        PlatformSignalController::start_with_source(transport.clone(), Box::new(OfflineSource));
    std::thread::sleep(std::time::Duration::from_millis(20));
    let response = transport
        .dispatch(
            RequestId(30),
            Request::Command(Command::CreateJob {
                kind: "networked.fake".into(),
                duration_ms: 200,
                will_fail: false,
                input: serde_json::Value::Null,
                policy: velvet_job_system::JobExecutionPolicy {
                    network: velvet_job_system::JobNetworkPolicy::Unmetered,
                    ..velvet_job_system::JobExecutionPolicy::default()
                },
                budget: velvet_job_system::JobBudgetRequest::default(),
            }),
        )
        .expect("the fake platform signal should not break dispatch");
    assert!(
        matches!(response, Response::Job(snapshot) if snapshot.status == velvet_job_system::JobStatus::Paused)
    );
    drop(controller);
}

#[test]
fn handshake_issues_a_session_and_rejects_unknown_sessions() {
    let transport = TauriTransport::new(ApplicationCore::default());
    let response = transport
        .negotiate_handshake(ProtocolHandshakeRequest {
            client_id: "desktop-test".into(),
            protocol_major: velvet_workbench_protocol::PROTOCOL_MAJOR,
            protocol_minor: velvet_workbench_protocol::PROTOCOL_MINOR,
            features: velvet_workbench_protocol::supported_transport_features(),
            schema_versions: velvet_workbench_protocol::supported_schema_versions(),
        })
        .expect("handshake should succeed");
    assert!(response.session_id.starts_with("core-"));
    transport
        .validate_session(Some(&response.session_id))
        .expect("issued session should be accepted");
    let replacement = transport
        .negotiate_handshake(ProtocolHandshakeRequest {
            client_id: "desktop-test".into(),
            protocol_major: velvet_workbench_protocol::PROTOCOL_MAJOR,
            protocol_minor: velvet_workbench_protocol::PROTOCOL_MINOR,
            features: Vec::new(),
            schema_versions: std::collections::BTreeMap::new(),
        })
        .expect("reconnect handshake should succeed");
    assert!(matches!(
        transport.validate_session(Some(&response.session_id)),
        Err(CoreError::SessionExpired { .. })
    ));
    transport
        .validate_session(Some(&replacement.session_id))
        .expect("replacement session should be accepted");
    assert!(matches!(
        transport.validate_session(Some("core-unknown")),
        Err(CoreError::SessionExpired { .. })
    ));
}

#[test]
fn plugin_envelope_uses_the_same_session_dispatch_path() {
    let transport = TauriTransport::new(ApplicationCore::default());
    let session = transport
        .negotiate_handshake(ProtocolHandshakeRequest {
            client_id: "plugin-contract-test".into(),
            protocol_major: velvet_workbench_protocol::PROTOCOL_MAJOR,
            protocol_minor: velvet_workbench_protocol::PROTOCOL_MINOR,
            features: Vec::new(),
            schema_versions: std::collections::BTreeMap::new(),
        })
        .expect("handshake should succeed")
        .session_id;
    transport
        .bind_plugin_session(&session, "missing.extension".into())
        .expect("plugin runtime identity should bind before dispatch");

    let error = transport
        .dispatch_plugin_session(velvet_workbench_protocol::PluginRequestEnvelope {
            protocol_version: velvet_workbench_protocol::PLUGIN_PROTOCOL_SCHEMA_VERSION,
            session_id: session,
            caller_extension_id: ExtensionId("missing.extension".into()),
            request_id: RequestId(90),
            message: velvet_workbench_protocol::PluginRequest::QueryProjection {
                extension_id: ExtensionId("missing.extension".into()),
            },
        })
        .expect_err("unknown projection should stay inside the plugin boundary");
    assert!(matches!(error, CoreError::Plugin(_)));

    let unbound_transport = TauriTransport::new(ApplicationCore::default());
    let unbound_session = unbound_transport
        .negotiate_handshake(ProtocolHandshakeRequest {
            client_id: "unbound-plugin-contract-test".into(),
            protocol_major: velvet_workbench_protocol::PROTOCOL_MAJOR,
            protocol_minor: velvet_workbench_protocol::PROTOCOL_MINOR,
            features: Vec::new(),
            schema_versions: std::collections::BTreeMap::new(),
        })
        .expect("handshake should succeed")
        .session_id;
    let unbound_error = unbound_transport
        .dispatch_plugin_session(velvet_workbench_protocol::PluginRequestEnvelope {
            protocol_version: velvet_workbench_protocol::PLUGIN_PROTOCOL_SCHEMA_VERSION,
            session_id: unbound_session,
            caller_extension_id: ExtensionId("missing.extension".into()),
            request_id: RequestId(91),
            message: velvet_workbench_protocol::PluginRequest::QueryProjection {
                extension_id: ExtensionId("missing.extension".into()),
            },
        })
        .expect_err("unbound plugin sessions must fail closed");
    assert!(matches!(unbound_error, CoreError::SessionExpired { .. }));
}

#[test]
fn session_dispatch_replays_declared_idempotent_results_and_reports_outcomes() {
    let sink = InMemoryEventSink::default();
    let transport = TauriTransport::with_event_sink(ApplicationCore::default(), sink.clone());
    let session = transport
        .negotiate_handshake(ProtocolHandshakeRequest {
            client_id: "desktop-test".into(),
            protocol_major: velvet_workbench_protocol::PROTOCOL_MAJOR,
            protocol_minor: velvet_workbench_protocol::PROTOCOL_MINOR,
            features: Vec::new(),
            schema_versions: std::collections::BTreeMap::new(),
        })
        .expect("handshake should succeed")
        .session_id;
    let request = Request::Command(Command::LayoutAt {
        expected_revision: Some(0),
        command: LayoutCommand::SetPartVisibility {
            part: WorkbenchPart::PrimarySidebar,
            visible: false,
        },
    });
    let first = transport
        .dispatch_session(&session, RequestId(44), request.clone())
        .expect("idempotent command should succeed");
    let duplicate = transport
        .dispatch_session(&session, RequestId(44), request)
        .expect("duplicate should return the cached result");
    assert_eq!(first, duplicate);
    assert_eq!(sink.events().len(), 1);

    let mutation = Request::Command(Command::SetLocale {
        locale: "zh-CN".into(),
        expected_revision: None,
    });
    transport
        .dispatch_session(&session, RequestId(48), mutation.clone())
        .expect("unscoped mutation should succeed once");
    assert!(matches!(
        transport.dispatch_session(&session, RequestId(48), mutation),
        Err(CoreError::RequestIdReuse { request_id: 48 })
    ));

    let failed = Request::Command(Command::LayoutAt {
        expected_revision: Some(99),
        command: LayoutCommand::ResetLayout,
    });
    assert!(matches!(
        transport.dispatch_session(&session, RequestId(49), failed.clone()),
        Err(CoreError::StaleRevision { .. })
    ));
    assert!(matches!(
        transport.dispatch_session(
            &session,
            RequestId(50),
            Request::Query(Query::RequestOutcome {
                request_id: RequestId(49),
            })
        ),
        Ok(Response::RequestOutcome(RequestOutcome {
            status: RequestOutcomeStatus::Unknown,
            ..
        }))
    ));
    assert!(matches!(
        transport.dispatch_session(&session, RequestId(49), failed),
        Err(CoreError::RequestIdReuse { request_id: 49 })
    ));

    assert!(matches!(
        transport.dispatch_session(
            &session,
            RequestId(44),
            Request::Command(Command::CloseQuarantinedRuntimeLease {
                extension_id: velvet_plugin_api::ExtensionId("sample.reader".into()),
                runtime_nonce: "nonce-1".into(),
            })
        ),
        Err(CoreError::RequestIdReuse { request_id: 44 })
    ));
    assert!(matches!(
        transport.dispatch_session(
            &session,
            RequestId(45),
            Request::Query(Query::RequestOutcome {
                request_id: RequestId(44),
            })
        ),
        Ok(Response::RequestOutcome(RequestOutcome {
            status: RequestOutcomeStatus::Completed,
            ..
        }))
    ));
    assert!(matches!(
        transport.dispatch_session(
            &session,
            RequestId(46),
            Request::Query(Query::RequestOutcome {
                request_id: RequestId(999),
            })
        ),
        Ok(Response::RequestOutcome(RequestOutcome {
            status: RequestOutcomeStatus::Unknown,
            ..
        }))
    ));
    let replacement = transport
        .negotiate_handshake(ProtocolHandshakeRequest {
            client_id: "desktop-test".into(),
            protocol_major: velvet_workbench_protocol::PROTOCOL_MAJOR,
            protocol_minor: velvet_workbench_protocol::PROTOCOL_MINOR,
            features: Vec::new(),
            schema_versions: std::collections::BTreeMap::new(),
        })
        .expect("reconnect should issue a replacement session")
        .session_id;
    assert!(matches!(
        transport.dispatch_session(
            &replacement,
            RequestId(47),
            Request::Query(Query::RequestOutcome {
                request_id: RequestId(44),
            })
        ),
        Ok(Response::RequestOutcome(RequestOutcome {
            status: RequestOutcomeStatus::Completed,
            ..
        }))
    ));
}

#[test]
fn evicted_command_outcomes_keep_old_request_ids_consumed() {
    let transport = TauriTransport::new(ApplicationCore::default());
    let session = transport
        .negotiate_handshake(ProtocolHandshakeRequest {
            client_id: "eviction-test".into(),
            protocol_major: velvet_workbench_protocol::PROTOCOL_MAJOR,
            protocol_minor: velvet_workbench_protocol::PROTOCOL_MINOR,
            features: Vec::new(),
            schema_versions: std::collections::BTreeMap::new(),
        })
        .expect("handshake should succeed")
        .session_id;

    let mutation = || {
        Request::Command(Command::SetLocale {
            locale: "en-US".into(),
            expected_revision: None,
        })
    };
    transport
        .dispatch_session(&session, RequestId(1), mutation())
        .expect("first mutation should succeed");
    for request_id in 2..=(MAX_REQUEST_OUTCOMES as u64 + 1) {
        transport
            .dispatch_session(&session, RequestId(request_id), mutation())
            .expect("mutation should remain dispatchable within the window");
    }

    assert!(matches!(
        transport.dispatch_session(&session, RequestId(1), mutation()),
        Err(CoreError::RequestIdReuse { request_id: 1 })
    ));
    assert!(matches!(
        transport.dispatch_session(
            &session,
            RequestId(MAX_REQUEST_OUTCOMES as u64 + 2),
            Request::Query(Query::RequestOutcome {
                request_id: RequestId(1),
            })
        ),
        Ok(Response::RequestOutcome(RequestOutcome {
            status: RequestOutcomeStatus::Unknown,
            ..
        }))
    ));
}
