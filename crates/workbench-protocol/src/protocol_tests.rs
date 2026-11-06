use super::*;

#[test]
fn canonical_manifest_dto_round_trips_without_becoming_the_domain_owner() {
    let manifest = ExtensionManifest {
        id: ExtensionId("sample.reader".into()),
        extension_kind: vec![ExtensionKind::Ui, ExtensionKind::Capability],
        activation_events: vec![velvet_plugin_api::ActivationEvent::WorkbenchReady],
        storage: Some(ExtensionStorageDeclaration {
            schema_version: 1,
            quota_bytes: 4096,
        }),
        ..ExtensionManifest::default()
    };

    let dto = PluginManifestDto::from(manifest.clone());
    let json = serde_json::to_value(&dto).expect("protocol DTO should serialize");
    assert_eq!(json["manifestSchema"], 1);
    assert_eq!(json["bridgeApi"], "1");
    assert_eq!(json["activationEvents"][0]["kind"], "onWorkbenchReady");
    assert_eq!(json["storage"]["schemaVersion"], 1);
    assert_eq!(json["storage"]["quotaBytes"], 4096);

    let decoded: PluginManifestDto =
        serde_json::from_value(json).expect("protocol DTO should deserialize");
    assert_eq!(ExtensionManifest::from(decoded), manifest);
}

#[test]
fn plugin_registry_snapshot_round_trips_as_a_declarative_projection() {
    let descriptor = ContributionDescriptor {
        id: "sample.reader.view".into(),
        kind: velvet_plugin_api::ContributionKind::View,
        scope: velvet_plugin_api::ContributionScope::Workbench,
        namespace: Some("workbench".into()),
        container: Some(velvet_plugin_api::HostContainer::PrimarySidebar),
        zone: Some(velvet_layout_model::WorkbenchZone::Navigation),
        title: Some("Reader".into()),
        title_key: None,
        icon: None,
        order: None,
        default_visible: Some(true),
        view_container_id: None,
        activity_bar: None,
        label: None,
        label_key: None,
        command_id: None,
        group: None,
        when: None,
        enablement: None,
        keybinding: None,
        keybinding_platform: None,
        menu_location: None,
        required_capabilities: Vec::new(),
        anchor: None,
        ui: None,
        preview: None,
    };
    let snapshot = PluginRegistrySnapshotDto {
        schema_version: PLUGIN_REGISTRY_SCHEMA_VERSION,
        generation: 2,
        contributions: vec![PluginContributionRegistrationDto {
            extension_id: ExtensionId("sample.reader".into()),
            contribution_id: descriptor.id.clone(),
            namespace: "workbench".into(),
            kind: descriptor.kind.clone(),
            scope: descriptor.scope.clone(),
            zone: descriptor.zone.clone(),
            schema_version: 1,
            descriptor,
        }],
    };
    let value = serde_json::to_value(&snapshot).expect("registry snapshot should serialize");
    assert_eq!(value["schemaVersion"], PLUGIN_REGISTRY_SCHEMA_VERSION);
    assert_eq!(value["contributions"][0]["schemaVersion"], 1);
    let decoded: PluginRegistrySnapshotDto =
        serde_json::from_value(value).expect("registry snapshot should deserialize");
    assert_eq!(decoded, snapshot);
}

#[test]
fn plugin_envelope_round_trips_and_maps_to_core_request() {
    let envelope = PluginRequestEnvelope {
        protocol_version: PLUGIN_PROTOCOL_SCHEMA_VERSION,
        session_id: "core-session".into(),
        caller_extension_id: ExtensionId("publisher.reader".into()),
        runtime_nonce: "test-runtime-nonce".into(),
        request_id: RequestId(7),
        message: PluginRequest::InvokeCommand {
            extension_id: ExtensionId("publisher.reader".into()),
            command_id: "reader.inspect".into(),
            args: serde_json::json!({ "assetId": "asset-1" }),
            context: serde_json::json!({ "zone": "contextual" }),
        },
    };

    envelope
        .validate()
        .expect("valid plugin envelope should pass");
    let mut missing_nonce = envelope.clone();
    missing_nonce.runtime_nonce.clear();
    assert!(matches!(
        missing_nonce.validate(),
        Err(CoreError::InvalidCommand(message)) if message.contains("runtime nonce")
    ));
    assert_eq!(envelope.idempotency_scope(), None);
    let json = serde_json::to_value(&envelope).expect("plugin envelope should serialize");
    assert_eq!(json["protocolVersion"], PLUGIN_PROTOCOL_SCHEMA_VERSION);
    assert_eq!(json["requestId"], 7);
    assert_eq!(json["callerExtensionId"], "publisher.reader");
    assert_eq!(json["runtimeNonce"], "test-runtime-nonce");
    assert_eq!(json["message"]["kind"], "invoke-command");
    assert_eq!(json["message"]["extensionId"], "publisher.reader");

    let decoded: PluginRequestEnvelope =
        serde_json::from_value(json).expect("plugin envelope should deserialize");
    assert_eq!(decoded, envelope);
    assert_eq!(
        decoded.into_request(),
        Request::Command(Command::InvokePluginCommand {
            extension_id: ExtensionId("publisher.reader".into()),
            command_id: "reader.inspect".into(),
            args: serde_json::json!({ "assetId": "asset-1" }),
            context: serde_json::json!({ "zone": "contextual" }),
            context_revision: 0,
            deadline_ms: None,
            idempotency_key: None,
        })
    );
}

#[test]
fn plugin_host_capability_requests_round_trip_without_widening_the_core_boundary() {
    let envelope = PluginRequestEnvelope {
        protocol_version: PLUGIN_PROTOCOL_SCHEMA_VERSION,
        session_id: "core-session".into(),
        caller_extension_id: ExtensionId("sample.services".into()),
        runtime_nonce: "test-runtime-nonce".into(),
        request_id: RequestId(9),
        message: PluginRequest::SubmitJob {
            extension_id: ExtensionId("sample.services".into()),
            job_kind: "sample.services.inspect".into(),
            input: serde_json::json!({"selectionRevision": 4}),
            provenance: JobProvenance::for_owner("untrusted-owner"),
            policy: JobExecutionPolicy::default(),
            budget: JobBudgetRequest::default(),
        },
    };
    envelope.validate().expect("host request should validate");
    let json = serde_json::to_value(&envelope).expect("host request should serialize");
    let decoded: PluginRequestEnvelope =
        serde_json::from_value(json).expect("host request should deserialize");
    assert_eq!(decoded, envelope);
    assert!(matches!(
        decoded.into_request(),
        Request::Command(Command::SubmitPluginJob {
            extension_id,
            job_kind,
            ..
        }) if extension_id == ExtensionId("sample.services".into())
            && job_kind == "sample.services.inspect"
    ));

    let response = PluginResponseEnvelope::from_core(
        "core-session".into(),
        ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id: RequestId(9),
            response: Response::PluginConfirmation { accepted: true },
        },
    )
    .expect("confirmation should remain a plugin response");
    assert_eq!(
        response.response,
        PluginResponse::Confirmation { accepted: true }
    );
}

#[test]
fn plugin_query_is_repeatable_and_session_mismatch_is_rejected() {
    let envelope = PluginRequestEnvelope {
        protocol_version: PLUGIN_PROTOCOL_SCHEMA_VERSION,
        session_id: "core-session".into(),
        caller_extension_id: ExtensionId("publisher.reader".into()),
        runtime_nonce: "test-runtime-nonce".into(),
        request_id: RequestId(8),
        message: PluginRequest::QueryProjection {
            extension_id: ExtensionId("publisher.reader".into()),
        },
    };

    assert_eq!(envelope.idempotency_scope(), Some("plugin-projection"));
    envelope
        .validate_session("core-session")
        .expect("matching session should pass");
    assert_eq!(
        envelope.validate_session("replacement-session"),
        Err(CoreError::SessionExpired {
            session_id: "core-session".into()
        })
    );
    assert_eq!(
        envelope.into_request(),
        Request::Query(Query::PluginProjection {
            extension_id: ExtensionId("publisher.reader".into()),
        })
    );
}

#[test]
fn plugin_envelope_cannot_address_another_extension_namespace() {
    let envelope = PluginRequestEnvelope {
        protocol_version: PLUGIN_PROTOCOL_SCHEMA_VERSION,
        session_id: "core-session".into(),
        caller_extension_id: ExtensionId("publisher.reader".into()),
        runtime_nonce: "test-runtime-nonce".into(),
        request_id: RequestId(9),
        message: PluginRequest::QueryCapabilities {
            extension_id: ExtensionId("other.publisher".into()),
        },
    };
    assert_eq!(
        envelope.validate(),
        Err(CoreError::InvalidCommand(
            "plugin caller cannot address another extension namespace".into()
        ))
    );
}

#[test]
fn plugin_response_and_event_envelopes_keep_only_plugin_messages() {
    let response = PluginResponseEnvelope {
        protocol_version: PLUGIN_PROTOCOL_SCHEMA_VERSION,
        session_id: "core-session".into(),
        request_id: RequestId(9),
        response: PluginResponse::Accepted,
    };
    let response_json = serde_json::to_value(&response).expect("response should serialize");
    assert_eq!(response_json["response"]["kind"], "accepted");
    assert_eq!(
        PluginResponseEnvelope::from_core(
            "core-session".into(),
            ResponseEnvelope {
                protocol_version: PROTOCOL_VERSION,
                request_id: RequestId(9),
                response: Response::Accepted,
            },
        )
        .expect("accepted response should map"),
        response
    );

    let event = PluginEventEnvelope::from_core(
        "core-session".into(),
        3,
        Event::PluginDisabled {
            extension_id: "publisher.reader".into(),
            reason: "crash".into(),
        },
    )
    .expect("plugin event should map");
    assert_eq!(event.sequence, 3);
    assert_eq!(
        serde_json::to_value(&event).unwrap()["event"]["kind"],
        "disabled"
    );
}

#[test]
fn core_transport_plugin_dispatch_uses_the_public_envelope_only() {
    struct FakeTransport;

    impl CoreTransport for FakeTransport {
        fn dispatch(
            &self,
            _request_id: RequestId,
            request: Request,
        ) -> Result<Response, CoreError> {
            match request {
                Request::Command(Command::InvokePluginCommand { .. }) => Ok(Response::Accepted),
                Request::Query(Query::PluginProjection { extension_id }) => {
                    Ok(Response::PluginProjection(PluginProjectionDto {
                        summary: ExtensionSummary {
                            id: extension_id,
                            version: "1.0.0".into(),
                            active: true,
                            state: velvet_plugin_api::ExtensionState::Active,
                        },
                        manifest: PluginManifestDto::from(ExtensionManifest::default()),
                    }))
                }
                _ => Err(CoreError::InvalidCommand("unexpected request".into())),
            }
        }
    }

    let response = FakeTransport
        .dispatch_plugin(PluginRequestEnvelope {
            protocol_version: PLUGIN_PROTOCOL_SCHEMA_VERSION,
            session_id: "fake-session".into(),
            caller_extension_id: ExtensionId("publisher.reader".into()),
            runtime_nonce: "test-runtime-nonce".into(),
            request_id: RequestId(10),
            message: PluginRequest::InvokeCommand {
                extension_id: ExtensionId("publisher.reader".into()),
                command_id: "reader.inspect".into(),
                args: serde_json::Value::Null,
                context: serde_json::Value::Null,
            },
        })
        .expect("plugin invocation should use the CoreTransport seam");
    assert_eq!(response.response, PluginResponse::Accepted);
}

#[test]
fn profile_state_round_trips_with_canonical_wire_names() {
    let state = ProfileState {
        schema_version: PROFILE_SCHEMA_VERSION,
        revision: 4,
        locale: "zh-Hans".into(),
        plugin_developer_mode: Some(true),
    };

    let json = serde_json::to_value(&state).expect("profile state should serialize");
    assert_eq!(json["schemaVersion"], PROFILE_SCHEMA_VERSION);
    assert_eq!(json["revision"], 4);
    assert_eq!(json["locale"], "zh-Hans");
    assert_eq!(json["pluginDeveloperMode"], true);
    assert!(json.get("schema_version").is_none());

    let decoded: ProfileState =
        serde_json::from_value(json).expect("profile state should deserialize");
    assert_eq!(decoded, state);
    let legacy: ProfileState = serde_json::from_value(serde_json::json!({
        "schemaVersion": PROFILE_SCHEMA_VERSION,
        "revision": 2,
        "locale": "en"
    }))
    .expect("legacy profiles should default the optional trust setting");
    assert_eq!(legacy.plugin_developer_mode, None);
    assert!(serde_json::from_value::<ProfileState>(serde_json::json!({
        "schemaVersion": PROFILE_SCHEMA_VERSION,
        "revision": 4,
        "locale": "zh-Hans",
        "pluginDeveloperMode": true,
        "unexpected": true
    }))
    .is_err());
}

#[test]
fn plugin_resource_dto_round_trips_with_bounded_payload_shape() {
    let resource = PluginResourceDto {
        schema_version: PLUGIN_RESOURCE_SCHEMA_VERSION,
        extension_id: ExtensionId("sample.localized".into()),
        locale: "zh-Hans".into(),
        content_type: "application/json".into(),
        bytes: "{\"ready\":\"就绪\"}".as_bytes().to_vec(),
    };

    let json = serde_json::to_value(&resource).expect("resource should serialize");
    assert_eq!(json["schemaVersion"], PLUGIN_RESOURCE_SCHEMA_VERSION);
    assert_eq!(json["extensionId"], "sample.localized");
    assert_eq!(json["locale"], "zh-Hans");
    assert_eq!(json["contentType"], "application/json");
    assert!(json["bytes"].is_array());
    let decoded: PluginResourceDto =
        serde_json::from_value(json).expect("resource should deserialize");
    assert_eq!(decoded, resource);
}

#[test]
fn recovery_projection_uses_stable_wire_names() {
    let projection = RecoveryProjection {
        schema_version: RECOVERY_SCHEMA_VERSION,
        mode: RecoveryMode::Safe,
        phase: StartupPhase::Preflight,
        attempt_id: 2,
        consecutive_failures: 2,
        last_failure: Some(RecoveryFailure {
            class: RecoveryFailureClass::AbnormalExit,
            phase: StartupPhase::Preflight,
            message: "previous launch did not complete".into(),
            at_ms: 42,
            target: None,
        }),
        last_good: None,
        actions: vec![RecoveryAction::Retry, RecoveryAction::RestoreSnapshot],
        platform_preflight: Vec::new(),
        durability_report: Some(DurabilityReport {
            schema_version: 1,
            mode: DurabilityMode::RequireFsync,
            atomic_replace: true,
            file_sync: true,
            directory_sync: true,
            crash_durable: true,
            detail: "fsync".into(),
        }),
        storage: Some(velvet_job_system::StorageAvailability::Writable),
    };
    let json = serde_json::to_value(&projection).expect("recovery should serialize");
    assert_eq!(json["schemaVersion"], RECOVERY_SCHEMA_VERSION);
    assert_eq!(json["mode"], "safe");
    assert_eq!(json["phase"], "preflight");
    assert_eq!(json["lastFailure"]["class"], "abnormal-exit");
    assert_eq!(json["actions"][1], "restore-snapshot");
    assert_eq!(json["durabilityReport"]["mode"], "require-fsync");
    let decoded: RecoveryProjection =
        serde_json::from_value(json).expect("recovery should deserialize");
    assert_eq!(decoded, projection);
}

#[test]
fn recovery_diagnostic_bundle_uses_stable_wire_names() {
    let bundle = RecoveryDiagnosticBundle {
        schema_version: RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
        exported_at_ms: 42,
        projection: RecoveryProjection {
            schema_version: RECOVERY_SCHEMA_VERSION,
            mode: RecoveryMode::Degraded,
            phase: StartupPhase::OptionalActivation,
            attempt_id: 2,
            consecutive_failures: 0,
            last_failure: None,
            last_good: None,
            actions: vec![RecoveryAction::Retry, RecoveryAction::ExportDiagnostics],
            platform_preflight: Vec::new(),
            durability_report: None,
            storage: None,
        },
        plugin_diagnostics: Vec::new(),
        diagnostics_degraded: false,
        crash_evidence: Vec::new(),
    };
    let json = serde_json::to_value(&bundle).expect("diagnostic bundle should serialize");
    assert_eq!(json["schemaVersion"], RECOVERY_DIAGNOSTIC_SCHEMA_VERSION);
    assert_eq!(json["exportedAtMs"], 42);
    assert_eq!(json["projection"]["mode"], "degraded");
    assert_eq!(json["projection"]["actions"][1], "export-diagnostics");
    assert_eq!(json["pluginDiagnostics"], serde_json::json!([]));
    assert_eq!(json["diagnosticsDegraded"], false);
    assert_eq!(json["crashEvidence"], serde_json::json!([]));

    let legacy = serde_json::json!({
        "schemaVersion": RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
        "exportedAtMs": 7,
        "projection": bundle.projection.clone(),
    });
    let decoded: RecoveryDiagnosticBundle =
        serde_json::from_value(legacy).expect("legacy diagnostic bundle should decode");
    assert!(decoded.plugin_diagnostics.is_empty());
    assert!(!decoded.diagnostics_degraded);
}

#[test]
fn crash_artifact_attachment_round_trips_without_a_path() {
    let evidence = CrashCaptureEvidence {
        schema_version: CRASH_CAPTURE_SCHEMA_VERSION,
        correlation_id: "launch-1".into(),
        process_id: 7,
        parent_process_id: None,
        attempt_id: 2,
        component: "core".into(),
        kind: CrashCaptureKind::Exception,
        code: "system-crash-artifact".into(),
        message: "artifact attached".into(),
        runtime_nonce: None,
        log_tail: None,
        artifact: Some(CrashArtifactAttachment {
            artifact_id: "0123456789abcdef01234567".into(),
            format: "dmp".into(),
            size_bytes: 42,
            sha256: "a".repeat(64),
            collected_at_ms: 9,
            metadata: Some(CrashArtifactMetadata {
                process_name: Some("Velvet".into()),
                process_id: Some(7),
                exception_type: Some("EXC_CRASH".into()),
                signal: Some("SIGABRT".into()),
                reason: Some("abort".into()),
            }),
        }),
        occurred_at_ms: 10,
    };
    let json = serde_json::to_value(&evidence).expect("crash evidence should serialize");
    assert_eq!(json["artifact"]["format"], "dmp");
    assert_eq!(json["artifact"]["metadata"]["signal"], "SIGABRT");
    assert!(json.get("path").is_none());
    let decoded: CrashCaptureEvidence =
        serde_json::from_value(json).expect("crash evidence should deserialize");
    assert_eq!(decoded, evidence);
}

#[test]
fn plugin_diagnostic_event_uses_a_bounded_typed_wire_shape() {
    let event = PluginDiagnosticEvent {
        schema_version: PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
        sequence: 4,
        extension_id: "sample.reader".into(),
        kind: PluginDiagnosticKind::Job,
        code: "job-failed".into(),
        message: "runtime exited".into(),
        job_id: Some("job-000001".into()),
        attempt: Some(2),
        retryable: true,
        occurred_at_ms: 12,
    };
    let json = serde_json::to_value(&event).expect("plugin diagnostic should serialize");
    assert_eq!(json["schemaVersion"], PLUGIN_DIAGNOSTIC_SCHEMA_VERSION);
    assert_eq!(json["kind"], "job");
    assert_eq!(json["jobId"], "job-000001");
    let decoded: PluginDiagnosticEvent =
        serde_json::from_value(json).expect("plugin diagnostic should deserialize");
    assert_eq!(decoded, event);
}

#[test]
fn recovery_quarantine_target_uses_stable_wire_names() {
    let failure = RecoveryFailure {
        class: RecoveryFailureClass::CoreMigration,
        phase: StartupPhase::RecoveringState,
        message: "migration failed".into(),
        at_ms: 9,
        target: Some(RecoveryTarget {
            owner_id: "layout".into(),
            scope: "workspace".into(),
            generation: 3,
        }),
    };
    let json = serde_json::to_value(&failure).expect("recovery failure should serialize");
    assert_eq!(json["target"]["ownerId"], "layout");
    assert_eq!(json["target"]["generation"], 3);
    assert_eq!(
        serde_json::to_value(RecoveryAction::QuarantineState)
            .expect("quarantine action should serialize"),
        "quarantine-state"
    );
}

#[test]
fn job_governance_contract_round_trips_policy_checkpoint_and_environment() {
    let policy = JobExecutionPolicy {
        deadline_ms: Some(5_000),
        cancellation_grace_ms: 250,
        retry: velvet_job_system::JobRetryPolicy {
            max_attempts: 4,
            initial_backoff_ms: 50,
            max_backoff_ms: 500,
        },
        network: velvet_job_system::JobNetworkPolicy::Unmetered,
        background: true,
    };
    let request = Request::Command(Command::CreateJob {
        kind: "index".into(),
        duration_ms: 1_000,
        will_fail: false,
        input: Value::Null,
        policy: policy.clone(),
        budget: JobBudgetRequest::default(),
    });
    assert_eq!(
        match request {
            Request::Command(Command::CreateJob { policy: actual, .. }) => actual,
            _ => unreachable!(),
        },
        policy
    );
    let environment = JobExecutionEnvironment {
        suspended: true,
        network: velvet_job_system::JobNetworkState::Unknown,
        storage: velvet_job_system::StorageAvailability::ReadOnly,
        power: velvet_job_system::PowerPressure::Critical,
        ..velvet_job_system::JobExecutionEnvironment::default()
    };
    let command = Command::SetJobExecutionEnvironment { environment };
    assert_eq!(command, Command::SetJobExecutionEnvironment { environment });
    let environment_json = serde_json::to_value(environment)
        .expect("job environment should serialize with its schema");
    assert_eq!(
        environment_json["schemaVersion"],
        velvet_job_system::JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION
    );
    assert_eq!(environment_json["storage"], "read-only");
    assert_eq!(environment_json["power"], "critical");
    let legacy: JobExecutionEnvironment = serde_json::from_value(serde_json::json!({
        "suspended": false,
        "network": "online"
    }))
    .expect("legacy environment should use conservative compatibility defaults");
    assert_eq!(legacy, JobExecutionEnvironment::default());
    let checkpoint = velvet_job_system::JobCheckpoint {
        schema_version: velvet_job_system::JOB_CHECKPOINT_SCHEMA_VERSION,
        payload: serde_json::json!({ "cursor": 4 }),
    };
    let json = serde_json::to_value(&checkpoint).expect("checkpoint should serialize");
    assert_eq!(json["schemaVersion"], 1);
    assert_eq!(json["payload"]["cursor"], 4);
}

#[test]
fn operation_plan_rejects_stale_targets_and_unknown_retries() {
    let plan = OperationPlan {
        schema_version: OPERATION_SCHEMA_VERSION,
        plan_id: "plan-1".into(),
        operation_id: "operation-1".into(),
        owner_id: "asset-core".into(),
        kind: OperationKind::Delete,
        targets: vec![OperationTarget {
            target_id: "asset-1".into(),
            expected_revision: Some(7),
            expected_fingerprint: Some("sha256:before".into()),
        }],
        created_at_ms: 100,
        expires_at_ms: 500,
        retry_policy: OperationRetryPolicy::ReconcileBeforeRetry,
        undo: OperationUndoCapability::BoundedBackup,
    };
    plan.validate(200)
        .expect("fresh operation plan should validate");
    assert_eq!(
        plan.revalidate(
            200,
            &[OperationTarget {
                target_id: "asset-1".into(),
                expected_revision: Some(8),
                expected_fingerprint: Some("sha256:after".into()),
            }]
        ),
        Err(OperationPlanError::StaleTarget("asset-1".into()))
    );
    assert_eq!(plan.validate(500), Err(OperationPlanError::Expired));
    let preview_plan = OperationPlan {
        targets: vec![OperationTarget {
            target_id: "asset-1".into(),
            expected_revision: None,
            expected_fingerprint: None,
        }],
        ..plan.clone()
    };
    preview_plan
        .validate_for_preview(200)
        .expect("preview envelope should allow provider-owned evidence");
    assert_eq!(
        preview_plan.validate(200),
        Err(OperationPlanError::MissingTargetEvidence("asset-1".into()))
    );

    let receipt = OperationReceipt {
        schema_version: OPERATION_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        operation_id: plan.operation_id.clone(),
        status: OperationReceiptStatus::Unknown,
        reconciled: false,
        recorded_at_ms: 250,
        items: vec![OperationItemReceipt {
            target_id: "asset-1".into(),
            status: OperationReceiptStatus::Unknown,
            message: None,
        }],
        undo_token: None,
    };
    assert!(receipt.retry_requires_reconciliation());
    let json = serde_json::to_value(&plan).expect("operation plan should serialize");
    assert_eq!(json["schemaVersion"], OPERATION_SCHEMA_VERSION);
    assert_eq!(json["kind"], "delete");
    assert_eq!(json["targets"][0]["expectedRevision"], 7);
}

#[test]
fn portable_bundle_and_import_plan_are_bounded_and_path_free_at_the_contract() {
    let section = PortableBundleSection {
        section_id: "profile".into(),
        scope: PortableBundleScope::Profile,
        owner_id: "profile".into(),
        schema_version: 1,
        required: true,
        extension_id: None,
        sha256: "a".repeat(64),
        payload: vec![1, 2, 3],
    };
    let bundle = PortableBundle {
        schema_version: PORTABLE_BUNDLE_SCHEMA_VERSION,
        bundle_id: "bundle-1".into(),
        created_at_ms: 42,
        source_build: "0.0.4".into(),
        sections: vec![section],
    };
    bundle.validate().expect("bundle metadata should validate");
    let json = serde_json::to_value(&bundle).expect("bundle should serialize");
    assert_eq!(json["schemaVersion"], PORTABLE_BUNDLE_SCHEMA_VERSION);
    assert_eq!(json["sections"][0]["scope"], "profile");
    assert!(json.get("path").is_none());
    assert!(serde_json::from_value::<PortableBundle>(serde_json::json!({
        "schemaVersion": PORTABLE_BUNDLE_SCHEMA_VERSION,
        "bundleId": "bundle-1",
        "createdAtMs": 42,
        "sourceBuild": "0.0.4",
        "sections": [{
            "sectionId": "profile",
            "scope": "profile",
            "ownerId": "profile",
            "schemaVersion": 1,
            "required": true,
            "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "payload": [],
            "path": "/tmp/unsafe"
        }]
    }))
    .is_err());

    let plan = ImportPlan {
        schema_version: IMPORT_PLAN_SCHEMA_VERSION,
        plan_id: "import-1".into(),
        bundle_id: "bundle-1".into(),
        created_at_ms: 50,
        expires_at_ms: 100,
        selected_section_ids: vec!["profile".into()],
        migrations: vec![ImportMigration {
            section_id: "profile".into(),
            from_schema_version: 1,
            to_schema_version: 2,
        }],
        conflicts: Vec::new(),
        unavailable_sources: Vec::new(),
        skipped_extension_sections: Vec::new(),
        estimated_bytes: 3,
    };
    plan.validate(60).expect("import plan should validate");
    let plan_json = serde_json::to_value(&plan).expect("plan should serialize");
    assert_eq!(plan_json["schemaVersion"], IMPORT_PLAN_SCHEMA_VERSION);
    assert_eq!(plan_json["migrations"][0]["fromSchemaVersion"], 1);

    let receipt = ImportReceipt {
        schema_version: IMPORT_RECEIPT_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        bundle_id: plan.bundle_id.clone(),
        status: ImportReceiptStatus::Partial,
        recorded_at_ms: 70,
        recovery_set_generation: Some(4),
        owners: vec![ImportOwnerReceipt {
            owner_id: "profile".into(),
            scope: "user".into(),
            status: ImportOwnerStatus::Committed,
            generation: Some(2),
            message: None,
        }],
    };
    receipt.validate().expect("import receipt should validate");
    let receipt_json = serde_json::to_value(&receipt).expect("receipt should serialize");
    assert_eq!(receipt_json["status"], "partial");
    assert_eq!(receipt_json["owners"][0]["status"], "committed");
}

#[test]
fn portable_asset_metadata_is_path_free_and_bounded() {
    let metadata = PortableAssetMetadata {
        schema_version: ASSET_BUNDLE_SCHEMA_VERSION,
        assets: vec![PortableAssetRecord {
            asset_id: "asset-1".into(),
            content_kind: "text/plain".into(),
            fingerprint: Some("sha256:asset-1".into()),
            source_availability: SourceAvailability::Available,
        }],
    };
    metadata.validate().expect("asset metadata should validate");
    let json = serde_json::to_value(&metadata).expect("asset metadata should serialize");
    assert_eq!(json["schemaVersion"], ASSET_BUNDLE_SCHEMA_VERSION);
    assert_eq!(json["assets"][0]["sourceAvailability"], "available");
    assert!(json["assets"][0].get("sourcePath").is_none());

    assert!(
        serde_json::from_value::<PortableAssetMetadata>(serde_json::json!({
            "schemaVersion": ASSET_BUNDLE_SCHEMA_VERSION,
            "assets": [{
                "assetId": "asset-1",
                "contentKind": "text/plain",
                "sourcePath": "/tmp/should-not-cross-the-boundary"
            }]
        }))
        .is_err()
    );
    assert_eq!(
        PortableAssetMetadata {
            schema_version: ASSET_BUNDLE_SCHEMA_VERSION,
            assets: vec![
                PortableAssetRecord {
                    asset_id: "asset-1".into(),
                    content_kind: "text/plain".into(),
                    fingerprint: None,
                    source_availability: SourceAvailability::Unknown,
                },
                PortableAssetRecord {
                    asset_id: "asset-1".into(),
                    content_kind: "image/png".into(),
                    fingerprint: None,
                    source_availability: SourceAvailability::Unknown,
                },
            ],
        }
        .validate(),
        Err(PortableAssetMetadataError::DuplicateAsset("asset-1".into()))
    );
}

#[test]
fn import_quarantine_entry_is_bounded_and_path_free() {
    let entry = ImportQuarantineEntry {
        schema_version: IMPORT_QUARANTINE_SCHEMA_VERSION,
        bundle_id: "bundle-1".into(),
        plan_id: "plan-1".into(),
        section_id: "extension-sample".into(),
        owner_id: "sample.extension".into(),
        section_schema_version: 1,
        quarantined_at_ms: 42,
        reason: "participant unavailable".into(),
        payload: vec![1, 2, 3],
    };
    entry.validate().expect("quarantine entry should validate");
    assert_eq!(
        entry.summary(),
        ImportQuarantineSummary {
            schema_version: IMPORT_QUARANTINE_SCHEMA_VERSION,
            bundle_id: "bundle-1".into(),
            plan_id: "plan-1".into(),
            section_id: "extension-sample".into(),
            owner_id: "sample.extension".into(),
            section_schema_version: 1,
            quarantined_at_ms: 42,
            reason: "participant unavailable".into(),
            payload_bytes: 3,
        }
    );
    let json = serde_json::to_value(&entry).expect("quarantine entry should serialize");
    assert_eq!(json["schemaVersion"], IMPORT_QUARANTINE_SCHEMA_VERSION);
    assert!(json.get("path").is_none());
    assert!(
        serde_json::from_value::<ImportQuarantineEntry>(serde_json::json!({
            "schemaVersion": IMPORT_QUARANTINE_SCHEMA_VERSION,
            "bundleId": "bundle-1",
            "planId": "plan-1",
            "sectionId": "extension-sample",
            "ownerId": "sample.extension",
            "sectionSchemaVersion": 1,
            "quarantinedAtMs": 42,
            "reason": "participant unavailable",
            "payload": [],
            "sourcePath": "/tmp/unsafe"
        }))
        .is_err()
    );
}

#[test]
fn protocol_handshake_negotiates_minor_features_and_schema_versions() {
    let response = negotiate_protocol(
        ProtocolHandshakeRequest {
            client_id: "desktop-test".into(),
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR + 1,
            features: vec![TRANSPORT_FEATURE_SESSION_V1.into(), "future-feature".into()],
            schema_versions: BTreeMap::from([
                ("profile".into(), PROFILE_SCHEMA_VERSION + 1),
                ("plugin-protocol".into(), PLUGIN_PROTOCOL_SCHEMA_VERSION),
                (
                    "job-execution-environment".into(),
                    JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION,
                ),
                ("operation".into(), OPERATION_SCHEMA_VERSION),
                ("future".into(), 1),
            ]),
        },
        "session-test".into(),
    )
    .expect("compatible protocol should negotiate");

    assert_eq!(response.protocol_major, PROTOCOL_MAJOR);
    assert_eq!(response.protocol_minor, PROTOCOL_MINOR);
    assert_eq!(
        response.selected_features,
        vec![TRANSPORT_FEATURE_SESSION_V1]
    );
    assert_eq!(response.schema_versions["profile"], PROFILE_SCHEMA_VERSION);
    assert_eq!(
        response.schema_versions["plugin-protocol"],
        PLUGIN_PROTOCOL_SCHEMA_VERSION
    );
    assert_eq!(
        response.schema_versions["job-execution-environment"],
        JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION
    );
    assert_eq!(
        response.schema_versions["operation"],
        OPERATION_SCHEMA_VERSION
    );
    assert!(!response.schema_versions.contains_key("future"));
    assert_eq!(response.session_id, "session-test");
}

#[test]
fn protocol_handshake_rejects_a_major_mismatch_before_any_command() {
    assert_eq!(
        negotiate_protocol(
            ProtocolHandshakeRequest {
                client_id: "desktop-test".into(),
                protocol_major: PROTOCOL_MAJOR + 1,
                protocol_minor: 0,
                features: Vec::new(),
                schema_versions: BTreeMap::new(),
            },
            "session-test".into(),
        ),
        Err(CoreError::ProtocolMajorMismatch {
            expected: PROTOCOL_MAJOR,
            received: PROTOCOL_MAJOR + 1,
        })
    );
}

#[test]
fn request_outcome_contract_marks_only_declared_commands_idempotent() {
    assert!(is_idempotent_request(&Request::Command(
        Command::CancelJob {
            job_id: JobId("job-1".into()),
        }
    )));
    assert!(!is_idempotent_request(&Request::Command(
        Command::RetryJob {
            job_id: JobId("job-1".into()),
        }
    )));
    assert!(is_idempotent_request(&Request::Command(
        Command::DiscardImportQuarantine {
            bundle_id: "bundle-1".into(),
            plan_id: "plan-1".into(),
            section_id: "section-1".into(),
        }
    )));
    assert!(!is_idempotent_request(&Request::Command(Command::Layout(
        LayoutCommand::ResetLayout,
    ))));
    assert!(is_idempotent_request(&Request::Command(
        Command::InvokePluginCommand {
            extension_id: ExtensionId("sample".into()),
            command_id: "sample.open".into(),
            args: Value::Null,
            context: Value::Null,
            context_revision: 1,
            deadline_ms: Some(1_000),
            idempotency_key: Some("sample.open:1".into()),
        }
    )));
}
