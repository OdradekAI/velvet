use velvet_asset_core::AssetId;
use velvet_layout_model::{LayoutCommand, LayoutState};
use velvet_plugin_api::ExtensionSummary;
use velvet_preview_core::{PreviewIntent, PreviewSurface, ResolvedPreview};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Layout(LayoutCommand),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Query {
    Layout,
    Extensions,
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
    Preview(ResolvedPreview),
    Accepted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    InvalidCommand(String),
    Layout(String),
    Plugin(String),
}

pub trait CoreTransport: Send + Sync {
    fn dispatch(&self, request_id: RequestId, request: Request) -> Result<Response, CoreError>;
}
