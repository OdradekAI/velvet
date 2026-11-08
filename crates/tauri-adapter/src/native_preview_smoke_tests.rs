use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::TauriTransport;
use velvet_application_core::ApplicationCore;
use velvet_layout_model::WorkbenchZone;
use velvet_persistence::InMemoryLayoutStateStore;
use velvet_plugin_api::{
    ContributionDescriptor, ContributionKind, ContributionScope, ExtensionId, ExtensionManifest,
    Permission, PreviewActionContext, PreviewContributionDescriptor, PreviewIntent, PreviewSurface,
    RuntimeKind, VersionRange, ZonePermissionSet,
};
use velvet_plugin_host::{
    PluginInvocation, PluginInvocationResult, PluginRuntime, RuntimeError, RuntimeHealth,
    RuntimeProcess,
};
use velvet_preview_core::{PreviewRenderer, PreviewSurface as CorePreviewSurface};
use velvet_workbench_protocol::{
    supported_schema_versions, supported_transport_features, Command, CoreError,
    ProtocolHandshakeRequest, Query, Request, RequestId, Response, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

const EXTENSION_ID: &str = "native.smoke";
const ASSET_ID: &str = "native-smoke-asset";
const PROVIDER_SESSION: &str = "native.smoke:provider:native-smoke-asset";
const AUGMENTATION_SESSION: &str = "native.smoke:metadata:native-smoke-asset";

struct NativePreviewRuntime {
    provider_attempts: Arc<AtomicUsize>,
}

struct NativePreviewProcess {
    provider_attempts: Arc<AtomicUsize>,
}

impl PluginRuntime for NativePreviewRuntime {
    fn start(
        &mut self,
        _manifest: &ExtensionManifest,
        _permissions: &velvet_plugin_host::PermissionGrant,
    ) -> Result<Box<dyn RuntimeProcess>, RuntimeError> {
        Ok(Box::new(NativePreviewProcess {
            provider_attempts: self.provider_attempts.clone(),
        }))
    }
}

impl RuntimeProcess for NativePreviewProcess {
    fn poll(&mut self) -> RuntimeHealth {
        RuntimeHealth::Running
    }

    fn shutdown(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn invoke(
        &mut self,
        invocation: &PluginInvocation,
    ) -> Result<PluginInvocationResult, RuntimeError> {
        let result = match invocation.method.as_str() {
            "preview.open" => {
                let attempt = self.provider_attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt == 1 {
                    return Err(RuntimeError::InvocationRejected(
                        "native provider unavailable".into(),
                    ));
                } else {
                    serde_json::json!({
                    "sessionId": PROVIDER_SESSION,
                    "renderer": {
                        "kind": "ui-bridge",
                        "entrypoint": "wasm://native-smoke/provider",
                        "bridgeVersion": "1"
                    },
                    "commands": ["preview.inspect"],
                    "slots": []
                    })
                }
            }
            "preview.augment" => serde_json::json!({
                "sessionId": AUGMENTATION_SESSION,
                "renderer": {
                    "kind": "ui-bridge",
                    "entrypoint": "wasm://native-smoke/metadata",
                    "bridgeVersion": "1"
                },
                "commands": ["preview.inspect-metadata"],
                "slots": []
            }),
            "preview.action" => serde_json::json!({"kind": "accepted"}),
            other => {
                return Err(RuntimeError::InvocationRejected(format!(
                    "unexpected native preview method {other}"
                )));
            }
        };

        Ok(PluginInvocationResult {
            result: Some(result),
            error: None,
            events: Vec::new(),
        })
    }
}

fn native_preview_manifest() -> ExtensionManifest {
    let preview = |id: &str, kind: ContributionKind, priority: i32, slot| ContributionDescriptor {
        id: id.into(),
        kind,
        scope: ContributionScope::Capability,
        namespace: Some("capability.native.smoke".into()),
        preview: Some(PreviewContributionDescriptor {
            content_kinds: vec!["image".into()],
            intents: vec![PreviewIntent::Quick],
            surfaces: vec![PreviewSurface::Workspace],
            priority,
            slot,
        }),
        ..ContributionDescriptor::default()
    };
    let command = |id: &str, title: &str| ContributionDescriptor {
        id: id.into(),
        kind: ContributionKind::Command,
        scope: ContributionScope::Workbench,
        namespace: Some("workbench".into()),
        title: Some(title.into()),
        ..ContributionDescriptor::default()
    };

    ExtensionManifest {
        id: ExtensionId(EXTENSION_ID.into()),
        version: "0.1.0".into(),
        host_api: VersionRange("^1".into()),
        runtime: RuntimeKind::BuiltIn,
        contributions: vec![
            preview(
                "preview.image",
                ContributionKind::PreviewProvider,
                100,
                None,
            ),
            preview(
                "preview.metadata",
                ContributionKind::PreviewAugmentation,
                90,
                Some(velvet_plugin_api::PreviewAugmentationSlot::Metadata),
            ),
            command("preview.inspect", "Inspect preview"),
            command("preview.inspect-metadata", "Inspect preview metadata"),
        ],
        permission_sets: vec![ZonePermissionSet {
            zone: WorkbenchZone::Contextual,
            permissions: vec![Permission::PreviewRender],
        }],
        ..ExtensionManifest::default()
    }
}

fn preview_query() -> Request {
    Request::Query(Query::ResolvePreview {
        asset_id: velvet_asset_core::AssetId(ASSET_ID.into()),
        content_kind: "image".into(),
        intent: velvet_preview_core::PreviewIntent::Quick,
        surface: CorePreviewSurface::Workspace,
    })
}

fn action_request(session_id: &str, provider_id: &str, command_id: &str) -> Request {
    Request::Command(Command::InvokePreviewAction {
        extension_id: ExtensionId(EXTENSION_ID.into()),
        command_id: command_id.into(),
        args: serde_json::Value::Null,
        context: PreviewActionContext {
            session_id: session_id.into(),
            provider_id: provider_id.into(),
            asset_id: ASSET_ID.into(),
            content_kind: "image".into(),
            intent: velvet_plugin_api::PreviewIntent::Quick,
            surface: velvet_plugin_api::PreviewSurface::Workspace,
        },
    })
}

#[test]
fn native_preview_smoke_qualifies_real_tauri_transport_lifecycle() {
    let provider_attempts = Arc::new(AtomicUsize::new(0));
    let mut core = ApplicationCore::with_plugin_runtime(
        Arc::new(InMemoryLayoutStateStore::default()),
        Box::new(NativePreviewRuntime {
            provider_attempts: provider_attempts.clone(),
        }),
    );
    let manifest = native_preview_manifest();
    let extension_id = manifest.id.clone();
    core.register_plugin(manifest)
        .expect("native preview fixture should register");
    core.activate_plugin(&extension_id)
        .expect("native preview fixture should activate");

    let transport = TauriTransport::new(core);
    let handshake = transport
        .negotiate_handshake(ProtocolHandshakeRequest {
            client_id: "native-preview-smoke".into(),
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            features: supported_transport_features(),
            schema_versions: supported_schema_versions(),
        })
        .expect("the native CoreTransport handshake should succeed");
    let session_id = handshake.session_id;

    let first = transport
        .dispatch_session(&session_id, RequestId(1), preview_query())
        .expect("the failed provider attempt should still return fallback preview");
    let Response::Preview(first) = first else {
        panic!("expected fallback preview response, got {first:?}");
    };
    assert_eq!(first.candidate.provider_id, "builtin.generic-fallback");
    assert!(first.diagnostic.is_some());
    assert_eq!(first.augmentations.len(), 1);
    assert!(matches!(
        first.augmentations[0]
            .session
            .as_ref()
            .map(|session| &session.descriptor.renderer),
        Some(PreviewRenderer::UiBridge { .. })
    ));

    let second = transport
        .dispatch_session(&session_id, RequestId(2), preview_query())
        .expect("the retry should resolve the native provider");
    let Response::Preview(second) = second else {
        panic!("expected provider preview response, got {second:?}");
    };
    assert_eq!(provider_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(second.candidate.provider_id, "native.smoke.preview.image");
    assert_eq!(second.session.descriptor.session_id, PROVIDER_SESSION);
    assert!(matches!(
        second.session.descriptor.renderer,
        PreviewRenderer::UiBridge { .. }
    ));
    let augmentation = second.augmentations.first().expect("metadata augmentation");
    assert_eq!(
        augmentation.augmentation_id,
        "native.smoke.preview.metadata"
    );
    assert_eq!(augmentation.slot, "metadata");
    assert_eq!(
        augmentation
            .session
            .as_ref()
            .map(|session| session.descriptor.session_id.as_str()),
        Some(AUGMENTATION_SESSION)
    );
    assert!(matches!(
        augmentation
            .session
            .as_ref()
            .map(|session| &session.descriptor.renderer),
        Some(PreviewRenderer::UiBridge { .. })
    ));

    let provider_id = second.session.provider_id.clone();
    let provider_action = transport.dispatch_session(
        &session_id,
        RequestId(3),
        action_request(PROVIDER_SESSION, &provider_id, "preview.inspect"),
    );
    assert!(
        matches!(provider_action, Ok(Response::Accepted)),
        "provider action: {provider_action:?}"
    );
    assert!(matches!(
        transport.dispatch_session(
            &session_id,
            RequestId(4),
            action_request(
                AUGMENTATION_SESSION,
                "native.smoke.preview.metadata",
                "preview.inspect-metadata",
            ),
        ),
        Ok(Response::Accepted)
    ));

    for (request_id, session) in [(5, PROVIDER_SESSION), (6, AUGMENTATION_SESSION)] {
        assert!(matches!(
            transport.dispatch_session(
                &session_id,
                RequestId(request_id),
                Request::Command(Command::ReleasePreviewSession {
                    session_id: session.into(),
                }),
            ),
            Ok(Response::Accepted)
        ));
    }

    let provider_release = transport
        .dispatch_session(
            &session_id,
            RequestId(7),
            action_request(PROVIDER_SESSION, &provider_id, "preview.inspect"),
        )
        .expect_err("released provider session must reject actions");
    assert!(matches!(
        provider_release,
        CoreError::InvalidCommand(message) if message == "preview action session is no longer active"
    ));
    let augmentation_release = transport
        .dispatch_session(
            &session_id,
            RequestId(8),
            action_request(
                AUGMENTATION_SESSION,
                "native.smoke.preview.metadata",
                "preview.inspect-metadata",
            ),
        )
        .expect_err("released augmentation session must reject actions");
    assert!(matches!(
        augmentation_release,
        CoreError::InvalidCommand(message) if message == "preview action session is no longer active"
    ));
}
