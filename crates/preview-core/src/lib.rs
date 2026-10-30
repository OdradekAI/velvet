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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PreviewRenderer {
    Generic,
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
    fn open(&self, context: PreviewContext) -> Result<PreviewSession, PreviewError>;
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

    fn open(&self, context: PreviewContext) -> Result<PreviewSession, PreviewError> {
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

#[derive(Default)]
pub struct PreviewResolver {
    providers: Vec<Arc<dyn PreviewProvider>>,
}

impl PreviewResolver {
    pub fn register(&mut self, provider: Arc<dyn PreviewProvider>) {
        self.providers.push(provider);
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
            match provider.open(request.context.clone()) {
                Ok(session) => {
                    return Ok(ResolvedPreview {
                        candidate,
                        session,
                        diagnostic: None,
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

        fn open(&self, context: PreviewContext) -> Result<PreviewSession, PreviewError> {
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

        fn open(&self, _context: PreviewContext) -> Result<PreviewSession, PreviewError> {
            Err(PreviewError::ProviderFailed {
                provider_id: "broken.image".into(),
                message: "decoder failed".into(),
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
}
