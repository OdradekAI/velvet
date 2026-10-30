use std::sync::Arc;

use serde::{Deserialize, Serialize};
use velvet_asset_core::{AssetId, ContentKindId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreviewIntent {
    Quick,
    Inspect,
    Analyze,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreviewSurface {
    Workspace,
    Thumbnail,
    Inspector,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewContext {
    pub asset_id: AssetId,
    pub content_kind: ContentKindId,
    pub intent: PreviewIntent,
    pub surface: PreviewSurface,
}

pub const DEFAULT_PREVIEW_READ_LIMIT: usize = 512 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewReadRequest {
    max_bytes: usize,
}

impl Default for PreviewReadRequest {
    fn default() -> Self {
        Self::new(DEFAULT_PREVIEW_READ_LIMIT)
    }
}

impl PreviewReadRequest {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes: max_bytes.clamp(1, DEFAULT_PREVIEW_READ_LIMIT),
        }
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreviewAssetContent {
    Text { text: String, truncated: bool },
    Bytes { bytes: Vec<u8>, truncated: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreviewSourceError {
    Unavailable,
    NotFound,
    PermissionDenied,
    InvalidRequest(String),
    Failed(String),
}

/// Host-owned, bounded read seam for built-in preview providers.
///
/// Implementations may read from persistence, a connected source or a cache,
/// but providers receive content only through this typed result. Paths,
/// database handles and source credentials never cross the preview boundary.
pub trait PreviewAssetSource: Send + Sync {
    fn read(
        &self,
        context: &PreviewContext,
        request: PreviewReadRequest,
    ) -> Result<PreviewAssetContent, PreviewSourceError>;
}

#[derive(Default)]
struct UnavailablePreviewAssetSource;

impl PreviewAssetSource for UnavailablePreviewAssetSource {
    fn read(
        &self,
        _context: &PreviewContext,
        _request: PreviewReadRequest,
    ) -> Result<PreviewAssetContent, PreviewSourceError> {
        Err(PreviewSourceError::Unavailable)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PreviewRenderer {
    Generic,
    Text {
        text: String,
        truncated: bool,
    },
    Image {
        mime_type: String,
        bytes: Vec<u8>,
        truncated: bool,
    },
    UiBridge {
        entrypoint: String,
        bridge_version: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSessionDescriptor {
    pub session_id: String,
    pub renderer: PreviewRenderer,
    pub commands: Vec<String>,
    pub slots: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewRequest {
    pub context: PreviewContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewCandidate {
    pub provider_id: String,
    pub content_kinds: Vec<ContentKindId>,
    pub intents: Vec<PreviewIntent>,
    pub surfaces: Vec<PreviewSurface>,
    pub priority: i32,
}

impl PreviewCandidate {
    pub fn matches(&self, context: &PreviewContext) -> bool {
        let content_matches = self.content_kinds.is_empty()
            || self
                .content_kinds
                .iter()
                .any(|kind| kind == &context.content_kind);
        let intent_matches =
            self.intents.is_empty() || self.intents.iter().any(|intent| intent == &context.intent);
        let surface_matches = self.surfaces.is_empty()
            || self
                .surfaces
                .iter()
                .any(|surface| surface == &context.surface);

        content_matches && intent_matches && surface_matches
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewSession {
    pub provider_id: String,
    pub context: PreviewContext,
    pub descriptor: PreviewSessionDescriptor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPreview {
    pub candidate: PreviewCandidate,
    pub session: PreviewSession,
    /// Non-fatal provider diagnostics preserved when the host falls back.
    pub diagnostic: Option<String>,
    pub augmentations: Vec<ResolvedPreviewAugmentation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPreviewAugmentation {
    pub augmentation_id: String,
    pub slot: String,
    pub session: Option<PreviewSession>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreviewError {
    NoProvider {
        content_kind: ContentKindId,
    },
    ProviderFailed {
        provider_id: String,
        message: String,
    },
}

pub trait PreviewProvider: Send + Sync {
    fn candidate(&self) -> PreviewCandidate;
    fn open(
        &self,
        context: PreviewContext,
        source: &dyn PreviewAssetSource,
    ) -> Result<PreviewSession, PreviewError>;
}

#[derive(Default)]
pub struct BuiltinTextPreviewProvider;

impl PreviewProvider for BuiltinTextPreviewProvider {
    fn candidate(&self) -> PreviewCandidate {
        PreviewCandidate {
            provider_id: "builtin.text".into(),
            content_kinds: vec![
                ContentKindId("text/plain".into()),
                ContentKindId("text/markdown".into()),
                ContentKindId("application/json".into()),
            ],
            intents: vec![PreviewIntent::Quick, PreviewIntent::Inspect],
            surfaces: vec![PreviewSurface::Workspace, PreviewSurface::Inspector],
            priority: 100,
        }
    }

    fn open(
        &self,
        context: PreviewContext,
        source: &dyn PreviewAssetSource,
    ) -> Result<PreviewSession, PreviewError> {
        let content = source
            .read(
                &context,
                PreviewReadRequest::new(DEFAULT_PREVIEW_READ_LIMIT),
            )
            .map_err(|error| provider_source_error("builtin.text", error))?;
        let PreviewAssetContent::Text { text, truncated } = content else {
            return Err(PreviewError::ProviderFailed {
                provider_id: "builtin.text".into(),
                message: "text provider received binary content".into(),
            });
        };
        Ok(PreviewSession {
            provider_id: "builtin.text".into(),
            context: context.clone(),
            descriptor: PreviewSessionDescriptor {
                session_id: format!("builtin.text:{}", context.asset_id),
                renderer: PreviewRenderer::Text { text, truncated },
                commands: Vec::new(),
                slots: Vec::new(),
            },
        })
    }
}

#[derive(Default)]
pub struct BuiltinImagePreviewProvider;

impl PreviewProvider for BuiltinImagePreviewProvider {
    fn candidate(&self) -> PreviewCandidate {
        PreviewCandidate {
            provider_id: "builtin.image".into(),
            content_kinds: vec![
                ContentKindId("image/png".into()),
                ContentKindId("image/jpeg".into()),
                ContentKindId("image/webp".into()),
                ContentKindId("image/gif".into()),
                ContentKindId("image/svg+xml".into()),
            ],
            intents: vec![PreviewIntent::Quick, PreviewIntent::Inspect],
            surfaces: vec![PreviewSurface::Workspace, PreviewSurface::Inspector],
            priority: 100,
        }
    }

    fn open(
        &self,
        context: PreviewContext,
        source: &dyn PreviewAssetSource,
    ) -> Result<PreviewSession, PreviewError> {
        let content = source
            .read(
                &context,
                PreviewReadRequest::new(DEFAULT_PREVIEW_READ_LIMIT),
            )
            .map_err(|error| provider_source_error("builtin.image", error))?;
        let (bytes, truncated) = match content {
            PreviewAssetContent::Bytes { bytes, truncated } => (bytes, truncated),
            PreviewAssetContent::Text { text, truncated } => (text.into_bytes(), truncated),
        };
        Ok(PreviewSession {
            provider_id: "builtin.image".into(),
            context: context.clone(),
            descriptor: PreviewSessionDescriptor {
                session_id: format!("builtin.image:{}", context.asset_id),
                renderer: PreviewRenderer::Image {
                    mime_type: context.content_kind.0.clone(),
                    bytes,
                    truncated,
                },
                commands: Vec::new(),
                slots: Vec::new(),
            },
        })
    }
}

fn provider_source_error(provider_id: &str, error: PreviewSourceError) -> PreviewError {
    PreviewError::ProviderFailed {
        provider_id: provider_id.into(),
        message: format!("source read failed: {error:?}"),
    }
}

#[derive(Default)]
pub struct GenericFallbackProvider;

impl PreviewProvider for GenericFallbackProvider {
    fn candidate(&self) -> PreviewCandidate {
        PreviewCandidate {
            provider_id: "builtin.generic-fallback".into(),
            content_kinds: Vec::new(),
            intents: Vec::new(),
            surfaces: Vec::new(),
            priority: i32::MIN,
        }
    }

    fn open(
        &self,
        context: PreviewContext,
        _source: &dyn PreviewAssetSource,
    ) -> Result<PreviewSession, PreviewError> {
        Ok(PreviewSession {
            provider_id: "builtin.generic-fallback".into(),
            context,
            descriptor: PreviewSessionDescriptor {
                session_id: "builtin.generic-fallback".into(),
                renderer: PreviewRenderer::Generic,
                commands: Vec::new(),
                slots: Vec::new(),
            },
        })
    }
}

pub struct PreviewResolver {
    providers: Vec<Arc<dyn PreviewProvider>>,
    source: Arc<dyn PreviewAssetSource>,
}

impl Default for PreviewResolver {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            source: Arc::new(UnavailablePreviewAssetSource),
        }
    }
}

impl PreviewResolver {
    pub fn register(&mut self, provider: Arc<dyn PreviewProvider>) {
        self.providers.push(provider);
    }

    pub fn set_asset_source(&mut self, source: Arc<dyn PreviewAssetSource>) {
        self.source = source;
    }

    pub fn resolve(&self, request: PreviewRequest) -> Result<ResolvedPreview, PreviewError> {
        let mut candidates = self
            .providers
            .iter()
            .filter_map(|provider| {
                let candidate = provider.candidate();
                candidate
                    .matches(&request.context)
                    .then_some((candidate, provider))
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| {
            right
                .0
                .priority
                .cmp(&left.0.priority)
                .then_with(|| left.0.provider_id.cmp(&right.0.provider_id))
        });
        if candidates.is_empty() {
            return Err(PreviewError::NoProvider {
                content_kind: request.context.content_kind.clone(),
            });
        }

        let mut first_error = None;
        for (candidate, provider) in candidates {
            match provider.open(request.context.clone(), self.source.as_ref()) {
                Ok(session) => {
                    return Ok(ResolvedPreview {
                        candidate,
                        session,
                        diagnostic: first_error.as_ref().map(|error| format!("{error:?}")),
                        augmentations: Vec::new(),
                    })
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }

        Err(first_error.unwrap_or(PreviewError::NoProvider {
            content_kind: request.context.content_kind,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ImageProvider;

    impl PreviewProvider for ImageProvider {
        fn candidate(&self) -> PreviewCandidate {
            PreviewCandidate {
                provider_id: "image.provider".into(),
                content_kinds: vec![ContentKindId("image".into())],
                intents: vec![PreviewIntent::Quick, PreviewIntent::Inspect],
                surfaces: vec![PreviewSurface::Workspace],
                priority: 10,
            }
        }

        fn open(
            &self,
            context: PreviewContext,
            _source: &dyn PreviewAssetSource,
        ) -> Result<PreviewSession, PreviewError> {
            Ok(PreviewSession {
                provider_id: "image.provider".into(),
                context,
                descriptor: PreviewSessionDescriptor {
                    session_id: "image.provider:asset-1".into(),
                    renderer: PreviewRenderer::Generic,
                    commands: Vec::new(),
                    slots: Vec::new(),
                },
            })
        }
    }

    struct FailingProvider;

    impl PreviewProvider for FailingProvider {
        fn candidate(&self) -> PreviewCandidate {
            PreviewCandidate {
                provider_id: "broken.image".into(),
                content_kinds: vec![ContentKindId("image".into())],
                intents: vec![PreviewIntent::Quick],
                surfaces: vec![PreviewSurface::Workspace],
                priority: 100,
            }
        }

        fn open(
            &self,
            _context: PreviewContext,
            _source: &dyn PreviewAssetSource,
        ) -> Result<PreviewSession, PreviewError> {
            Err(PreviewError::ProviderFailed {
                provider_id: "broken.image".into(),
                message: "decoder failed".into(),
            })
        }
    }

    struct MemoryTextSource;

    impl PreviewAssetSource for MemoryTextSource {
        fn read(
            &self,
            _context: &PreviewContext,
            request: PreviewReadRequest,
        ) -> Result<PreviewAssetContent, PreviewSourceError> {
            assert_eq!(request.max_bytes(), 64);
            Ok(PreviewAssetContent::Text {
                text: "hello preview".into(),
                truncated: false,
            })
        }
    }

    struct SourceBackedProvider;

    impl PreviewProvider for SourceBackedProvider {
        fn candidate(&self) -> PreviewCandidate {
            PreviewCandidate {
                provider_id: "builtin.text".into(),
                content_kinds: vec![ContentKindId("text/plain".into())],
                intents: vec![PreviewIntent::Quick],
                surfaces: vec![PreviewSurface::Workspace],
                priority: 10,
            }
        }

        fn open(
            &self,
            context: PreviewContext,
            source: &dyn PreviewAssetSource,
        ) -> Result<PreviewSession, PreviewError> {
            let content = source
                .read(&context, PreviewReadRequest::new(64))
                .map_err(|error| PreviewError::ProviderFailed {
                    provider_id: "builtin.text".into(),
                    message: format!("source read failed: {error:?}"),
                })?;
            let PreviewAssetContent::Text { text, truncated } = content else {
                return Err(PreviewError::ProviderFailed {
                    provider_id: "builtin.text".into(),
                    message: "text provider received non-text content".into(),
                });
            };
            assert_eq!(text, "hello preview");
            assert!(!truncated);
            Ok(PreviewSession {
                provider_id: "builtin.text".into(),
                context,
                descriptor: PreviewSessionDescriptor {
                    session_id: "builtin.text:asset-1".into(),
                    renderer: PreviewRenderer::Generic,
                    commands: Vec::new(),
                    slots: Vec::new(),
                },
            })
        }
    }

    fn request(intent: PreviewIntent) -> PreviewRequest {
        PreviewRequest {
            context: PreviewContext {
                asset_id: AssetId("asset-1".into()),
                content_kind: ContentKindId("image".into()),
                intent,
                surface: PreviewSurface::Workspace,
            },
        }
    }

    #[test]
    fn resolver_passes_bounded_content_through_the_host_source_seam() {
        let mut resolver = PreviewResolver::default();
        resolver.set_asset_source(Arc::new(MemoryTextSource));
        resolver.register(Arc::new(SourceBackedProvider));

        let resolved = resolver
            .resolve(PreviewRequest {
                context: PreviewContext {
                    asset_id: AssetId("asset-1".into()),
                    content_kind: ContentKindId("text/plain".into()),
                    intent: PreviewIntent::Quick,
                    surface: PreviewSurface::Workspace,
                },
            })
            .expect("source-backed provider should resolve");
        assert_eq!(resolved.candidate.provider_id, "builtin.text");
    }

    #[test]
    fn resolver_prefers_a_matching_provider_over_fallback() {
        let mut resolver = PreviewResolver::default();
        resolver.register(Arc::new(GenericFallbackProvider));
        resolver.register(Arc::new(ImageProvider));

        let resolved = resolver.resolve(request(PreviewIntent::Quick)).unwrap();
        assert_eq!(resolved.candidate.provider_id, "image.provider");
    }

    #[test]
    fn resolver_uses_fallback_for_an_unhandled_intent() {
        let mut resolver = PreviewResolver::default();
        resolver.register(Arc::new(GenericFallbackProvider));
        resolver.register(Arc::new(ImageProvider));

        let resolved = resolver.resolve(request(PreviewIntent::Analyze)).unwrap();
        assert_eq!(resolved.candidate.provider_id, "builtin.generic-fallback");
    }

    #[test]
    fn resolver_falls_back_when_the_selected_provider_fails() {
        let mut resolver = PreviewResolver::default();
        resolver.register(Arc::new(GenericFallbackProvider));
        resolver.register(Arc::new(FailingProvider));

        let resolved = resolver.resolve(request(PreviewIntent::Quick)).unwrap();
        assert_eq!(resolved.candidate.provider_id, "builtin.generic-fallback");
    }

    #[test]
    fn resolver_filters_candidates_by_surface() {
        let mut resolver = PreviewResolver::default();
        resolver.register(Arc::new(ImageProvider));

        let mut request = request(PreviewIntent::Quick);
        request.context.surface = PreviewSurface::Thumbnail;
        assert!(matches!(
            resolver.resolve(request),
            Err(PreviewError::NoProvider { .. })
        ));
    }

    struct BuiltinProviderSource;

    impl PreviewAssetSource for BuiltinProviderSource {
        fn read(
            &self,
            context: &PreviewContext,
            _request: PreviewReadRequest,
        ) -> Result<PreviewAssetContent, PreviewSourceError> {
            match context.asset_id.0.as_str() {
                "text-1" => Ok(PreviewAssetContent::Text {
                    text: "hello".into(),
                    truncated: false,
                }),
                "image-1" => Ok(PreviewAssetContent::Bytes {
                    bytes: vec![1, 2, 3],
                    truncated: false,
                }),
                _ => Err(PreviewSourceError::NotFound),
            }
        }
    }

    #[test]
    fn built_in_text_and_image_providers_return_host_renderers() {
        let source = BuiltinProviderSource;

        let text_context = PreviewContext {
            asset_id: AssetId("text-1".into()),
            content_kind: ContentKindId("text/plain".into()),
            intent: PreviewIntent::Quick,
            surface: PreviewSurface::Workspace,
        };
        let text = BuiltinTextPreviewProvider
            .open(text_context, &source)
            .expect("the built-in text provider should open");
        assert!(matches!(
            text.descriptor.renderer,
            PreviewRenderer::Text { ref text, truncated: false } if text == "hello"
        ));

        let image_context = PreviewContext {
            asset_id: AssetId("image-1".into()),
            content_kind: ContentKindId("image/png".into()),
            intent: PreviewIntent::Quick,
            surface: PreviewSurface::Workspace,
        };
        let image = BuiltinImagePreviewProvider
            .open(image_context, &source)
            .expect("the built-in image provider should open");
        assert!(matches!(
            image.descriptor.renderer,
            PreviewRenderer::Image { ref mime_type, ref bytes, truncated: false }
                if mime_type == "image/png" && bytes == &[1, 2, 3]
        ));
    }
}
