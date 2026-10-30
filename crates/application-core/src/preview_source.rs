use std::collections::BTreeMap;
use std::sync::RwLock;

use velvet_asset_core::AssetId;
use velvet_preview_core::{
    PreviewAssetContent, PreviewAssetSource, PreviewContext, PreviewReadRequest, PreviewSourceError,
};

/// Read-only source adapter used by the current skeleton and contract tests.
///
/// `ApplicationCore` owns the source instance. The adapter deliberately stores
/// only typed content keyed by opaque `AssetId`; no path, database handle or
/// credential crosses the preview seam. A future Asset source-location/store
/// adapter can replace this implementation without changing `preview-core`.
#[derive(Default)]
pub struct InMemoryPreviewAssetSource {
    assets: RwLock<BTreeMap<AssetId, PreviewAssetContent>>,
}

impl InMemoryPreviewAssetSource {
    pub fn insert_text(&self, asset_id: AssetId, text: impl Into<String>) {
        self.insert(
            asset_id,
            PreviewAssetContent::Text {
                text: text.into(),
                truncated: false,
            },
        );
    }

    pub fn insert_bytes(&self, asset_id: AssetId, bytes: impl Into<Vec<u8>>) {
        self.insert(
            asset_id,
            PreviewAssetContent::Bytes {
                bytes: bytes.into(),
                truncated: false,
            },
        );
    }

    pub fn insert(&self, asset_id: AssetId, content: PreviewAssetContent) {
        if let Ok(mut assets) = self.assets.write() {
            assets.insert(asset_id, content);
        }
    }

    pub fn remove(&self, asset_id: &AssetId) -> Option<PreviewAssetContent> {
        self.assets
            .write()
            .ok()
            .and_then(|mut assets| assets.remove(asset_id))
    }
}

impl PreviewAssetSource for InMemoryPreviewAssetSource {
    fn read(
        &self,
        context: &PreviewContext,
        request: PreviewReadRequest,
    ) -> Result<PreviewAssetContent, PreviewSourceError> {
        let assets = self
            .assets
            .read()
            .map_err(|_| PreviewSourceError::Failed("asset source lock poisoned".into()))?;
        let Some(content) = assets.get(&context.asset_id) else {
            return Err(PreviewSourceError::NotFound);
        };
        Ok(bound_content(content, request.max_bytes()))
    }
}

fn bound_content(content: &PreviewAssetContent, max_bytes: usize) -> PreviewAssetContent {
    match content {
        PreviewAssetContent::Text { text, truncated } => {
            if text.len() <= max_bytes {
                return PreviewAssetContent::Text {
                    text: text.clone(),
                    truncated: *truncated,
                };
            }

            let mut end = max_bytes;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            PreviewAssetContent::Text {
                text: text[..end].to_string(),
                truncated: true,
            }
        }
        PreviewAssetContent::Bytes { bytes, truncated } => PreviewAssetContent::Bytes {
            bytes: bytes[..bytes.len().min(max_bytes)].to_vec(),
            truncated: *truncated || bytes.len() > max_bytes,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(asset_id: &str) -> PreviewContext {
        PreviewContext {
            asset_id: AssetId(asset_id.into()),
            content_kind: velvet_asset_core::ContentKindId("text/plain".into()),
            intent: velvet_preview_core::PreviewIntent::Quick,
            surface: velvet_preview_core::PreviewSurface::Workspace,
        }
    }

    #[test]
    fn reads_typed_content_with_a_byte_bound_and_utf8_boundary() {
        let source = InMemoryPreviewAssetSource::default();
        source.insert_text(AssetId("asset-1".into()), "你好 preview");

        let content = source
            .read(&context("asset-1"), PreviewReadRequest::new(6))
            .expect("the source should find the asset");
        assert_eq!(
            content,
            PreviewAssetContent::Text {
                text: "你好".into(),
                truncated: true,
            }
        );
    }

    #[test]
    fn reports_missing_assets_without_exposing_storage_details() {
        let source = InMemoryPreviewAssetSource::default();
        assert_eq!(
            source.read(&context("missing"), PreviewReadRequest::default()),
            Err(PreviewSourceError::NotFound)
        );
    }
}
