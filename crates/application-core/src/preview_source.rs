use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use velvet_asset_core::{AssetId, ContentFingerprint};
use velvet_preview_core::{
    PreviewAssetContent, PreviewAssetSource, PreviewContext, PreviewReadRequest, PreviewSourceError,
};

/// Resolves an opaque Asset identity to an application-owned source location.
/// The returned path is consumed only by the application source adapter and
/// never crosses the preview provider seam.
pub trait AssetSourceLocationResolver: Send + Sync {
    fn resolve(&self, asset_id: &AssetId) -> Result<PathBuf, PreviewSourceError>;

    /// Returns the source-owned identity of the complete content at this
    /// location. Resolvers that cannot provide one leave derived work
    /// uncacheable rather than guessing from an opaque path.
    fn fingerprint(
        &self,
        _asset_id: &AssetId,
    ) -> Result<Option<ContentFingerprint>, PreviewSourceError> {
        Ok(None)
    }
}

/// Read-only source adapter used by the current skeleton and contract tests.
///
/// `ApplicationCore` owns the source instance. The adapter deliberately stores
/// only typed content keyed by opaque `AssetId`; no path, database handle or
/// credential crosses the preview seam. A future Asset source-location/store
/// adapter can replace this implementation without changing `preview-core`.
#[derive(Default)]
pub struct InMemoryPreviewAssetSource {
    assets: RwLock<BTreeMap<AssetId, PreviewAssetRecord>>,
}

struct PreviewAssetRecord {
    content: PreviewAssetContent,
    fingerprint: Option<ContentFingerprint>,
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
            assets.insert(
                asset_id,
                PreviewAssetRecord {
                    content,
                    fingerprint: None,
                },
            );
        }
    }

    pub fn insert_with_fingerprint(
        &self,
        asset_id: AssetId,
        content: PreviewAssetContent,
        fingerprint: ContentFingerprint,
    ) {
        if let Ok(mut assets) = self.assets.write() {
            assets.insert(
                asset_id,
                PreviewAssetRecord {
                    content,
                    fingerprint: Some(fingerprint),
                },
            );
        }
    }

    pub fn remove(&self, asset_id: &AssetId) -> Option<PreviewAssetContent> {
        self.assets
            .write()
            .ok()
            .and_then(|mut assets| assets.remove(asset_id).map(|record| record.content))
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
        Ok(bound_content(&content.content, request.max_bytes()))
    }

    fn fingerprint(
        &self,
        asset_id: &AssetId,
    ) -> Result<Option<ContentFingerprint>, PreviewSourceError> {
        let assets = self
            .assets
            .read()
            .map_err(|_| PreviewSourceError::Failed("asset source lock poisoned".into()))?;
        let Some(record) = assets.get(asset_id) else {
            return Err(PreviewSourceError::NotFound);
        };
        Ok(record.fingerprint.clone())
    }
}

/// Bounded filesystem adapter for trusted application composition.
///
/// The resolver owns source-location policy (roots, permissions and identity
/// mapping); this adapter owns bounded reads and typed content conversion.
/// Providers still receive only `PreviewAssetContent`.
pub struct FilePreviewAssetSource {
    locations: Arc<dyn AssetSourceLocationResolver>,
}

impl FilePreviewAssetSource {
    pub fn new(locations: Arc<dyn AssetSourceLocationResolver>) -> Self {
        Self { locations }
    }
}

impl PreviewAssetSource for FilePreviewAssetSource {
    fn read(
        &self,
        context: &PreviewContext,
        request: PreviewReadRequest,
    ) -> Result<PreviewAssetContent, PreviewSourceError> {
        let path = self.locations.resolve(&context.asset_id)?;
        let file = File::open(path).map_err(map_io_error)?;
        let max_bytes = request.max_bytes();
        let mut bytes = Vec::with_capacity(max_bytes.saturating_add(1));
        file.take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(map_io_error)?;
        let truncated = bytes.len() > max_bytes;
        if truncated {
            bytes.truncate(max_bytes);
        }

        if is_text_content_kind(&context.content_kind.0) {
            if let Some(text) = decode_bounded_utf8(&bytes) {
                return Ok(PreviewAssetContent::Text { text, truncated });
            }
        }

        Ok(PreviewAssetContent::Bytes { bytes, truncated })
    }

    fn fingerprint(
        &self,
        asset_id: &AssetId,
    ) -> Result<Option<ContentFingerprint>, PreviewSourceError> {
        self.locations.fingerprint(asset_id)
    }
}

fn map_io_error(error: io::Error) -> PreviewSourceError {
    match error.kind() {
        io::ErrorKind::NotFound => PreviewSourceError::NotFound,
        io::ErrorKind::PermissionDenied => PreviewSourceError::PermissionDenied,
        _ => PreviewSourceError::Failed(error.to_string()),
    }
}

fn is_text_content_kind(content_kind: &str) -> bool {
    content_kind.starts_with("text/")
        || matches!(
            content_kind,
            "application/json"
                | "application/javascript"
                | "application/xml"
                | "application/yaml"
                | "application/x-yaml"
                | "application/toml"
        )
}

fn decode_bounded_utf8(bytes: &[u8]) -> Option<String> {
    let mut end = bytes.len();
    while end > 0 {
        if let Ok(text) = std::str::from_utf8(&bytes[..end]) {
            return Some(text.to_owned());
        }
        end -= 1;
    }
    if bytes.is_empty() {
        Some(String::new())
    } else {
        None
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
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        assert_eq!(
            source.fingerprint(&AssetId("missing".into())),
            Err(PreviewSourceError::NotFound)
        );
    }

    #[test]
    fn returns_source_owned_fingerprints_and_clears_them_on_unknown_updates() {
        let source = InMemoryPreviewAssetSource::default();
        source.insert_with_fingerprint(
            AssetId("asset-1".into()),
            PreviewAssetContent::Text {
                text: "v1".into(),
                truncated: false,
            },
            ContentFingerprint("sha256:v1".into()),
        );
        assert_eq!(
            source.fingerprint(&AssetId("asset-1".into())),
            Ok(Some(ContentFingerprint("sha256:v1".into())))
        );

        source.insert_text(AssetId("asset-1".into()), "v2");
        assert_eq!(source.fingerprint(&AssetId("asset-1".into())), Ok(None));
    }

    struct MapAssetLocations(BTreeMap<AssetId, PathBuf>);

    impl AssetSourceLocationResolver for MapAssetLocations {
        fn resolve(&self, asset_id: &AssetId) -> Result<PathBuf, PreviewSourceError> {
            self.0
                .get(asset_id)
                .cloned()
                .ok_or(PreviewSourceError::NotFound)
        }
    }

    #[test]
    fn file_source_reads_bounded_text_without_exposing_the_path() {
        let path = std::env::temp_dir().join(format!(
            "velvet-preview-source-{}-{}.txt",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        fs::write(&path, "你好 preview").expect("test source should be writable");

        let mut locations = BTreeMap::new();
        locations.insert(AssetId("asset-1".into()), path.clone());
        let source = FilePreviewAssetSource::new(Arc::new(MapAssetLocations(locations)));
        let content = source
            .read(&context("asset-1"), PreviewReadRequest::new(6))
            .expect("the file source should read the mapped asset");
        assert_eq!(
            content,
            PreviewAssetContent::Text {
                text: "你好".into(),
                truncated: true,
            }
        );

        fs::remove_file(path).expect("test source should be cleaned up");
    }
}
