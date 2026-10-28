use std::sync::Arc;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewRequest {
    pub context: PreviewContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewCandidate {
    pub provider_id: String,
    pub content_kinds: Vec<ContentKindId>,
    pub intents: Vec<PreviewIntent>,
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

        content_matches && intent_matches
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewSession {
    pub provider_id: String,
    pub context: PreviewContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPreview {
    pub candidate: PreviewCandidate,
    pub session: PreviewSession,
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
            priority: i32::MIN,
        }
    }

    fn open(&self, context: PreviewContext) -> Result<PreviewSession, PreviewError> {
        Ok(PreviewSession {
            provider_id: "builtin.generic-fallback".into(),
            context,
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

        candidates.sort_by(|left, right| right.0.priority.cmp(&left.0.priority));
        let (candidate, provider) =
            candidates
                .into_iter()
                .next()
                .ok_or_else(|| PreviewError::NoProvider {
                    content_kind: request.context.content_kind.clone(),
                })?;
        let session = provider.open(request.context)?;

        Ok(ResolvedPreview { candidate, session })
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
                priority: 10,
            }
        }

        fn open(&self, context: PreviewContext) -> Result<PreviewSession, PreviewError> {
            Ok(PreviewSession {
                provider_id: "image.provider".into(),
                context,
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
}
