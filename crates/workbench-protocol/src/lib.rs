use serde_json::Value;
use velvet_asset_core::AssetId;
use velvet_layout_model::{LayoutCommand, LayoutState};
use velvet_plugin_api::{ExtensionId, ExtensionManifest, ExtensionProjection, ExtensionSummary};
use velvet_preview_core::{PreviewIntent, PreviewSurface, ResolvedPreview};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Layout(LayoutCommand),
    LayoutAt {
        expected_revision: Option<u64>,
        command: LayoutCommand,
    },
    RegisterPlugin {
        manifest: ExtensionManifest,
    },
    ActivatePlugin {
        extension_id: ExtensionId,
    },
    DisablePlugin {
        extension_id: ExtensionId,
        reason: PluginDisableReason,
    },
    CheckPluginHealth {
        extension_id: ExtensionId,
    },
    RecoverPlugin {
        extension_id: ExtensionId,
    },
    InvokePluginCommand {
        extension_id: ExtensionId,
        command_id: String,
        args: Value,
        context: Value,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginDisableReason {
    User,
    Crash,
    Incompatible,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Query {
    Layout,
    Extensions,
    PluginProjection {
        extension_id: ExtensionId,
    },
    ResolvePreview {
        asset_id: AssetId,
        content_kind: String,
        intent: PreviewIntent,
        surface: PreviewSurface,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    Command(Command),
    Query(Query),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response {
    Layout(LayoutState),
    Extensions(Vec<ExtensionSummary>),
    Extension(ExtensionSummary),
    PluginProjection(ExtensionProjection),
    Preview(ResolvedPreview),
    Accepted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    LayoutChanged {
        revision: u64,
        state: LayoutState,
    },
    PluginDisabled {
        extension_id: String,
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    ProtocolVersionMismatch {
        expected: u16,
        received: u16,
    },
    InvalidCommand(String),
    Layout(String),
    Plugin(String),
    PluginRuntimeFailed {
        extension_id: String,
        message: String,
    },
    StaleRevision {
        expected: u64,
        actual: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestEnvelope {
    pub protocol_version: u16,
    pub request_id: RequestId,
    pub request: Request,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseEnvelope {
    pub protocol_version: u16,
    pub request_id: RequestId,
    pub response: Response,
}

pub trait CoreTransport: Send + Sync {
    fn dispatch(&self, request_id: RequestId, request: Request) -> Result<Response, CoreError>;

    fn dispatch_envelope(&self, envelope: RequestEnvelope) -> Result<ResponseEnvelope, CoreError> {
        if envelope.protocol_version != PROTOCOL_VERSION {
            return Err(CoreError::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                received: envelope.protocol_version,
            });
        }

        let request_id = envelope.request_id.clone();
        let response = self.dispatch(envelope.request_id, envelope.request)?;
        Ok(ResponseEnvelope {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            response,
        })
    }
}
