# LLM output formats and Velvet plugin implications

Date: 2026-11-03

## Source limitations

The canonical post is [Andrej Karpathy's X post](https://x.com/karpathy/status/2105819303471976479), but X returned HTTP 403 during this review. I therefore do not treat the post text as directly verified. The public [answer-me-with-html repository](https://github.com/QingYunA/answer-me-with-html) identifies the post as its motivation and summarizes the claim; that is a secondary account by the project author, not an authenticated X export. The repository itself is primary evidence for the implementation and measured claims about that project. ASD-STE100 evidence below is from the official STEMG site; the full standard is copyright-controlled and was not reproduced.

## Observed post claims (with attribution limits)

The repository's background section says Karpathy observed that, as LLMs do more work, keeping up with their output becomes difficult, and that a diagram or web page is easier to consume than a long text block ([README lines 339–344](https://github.com/QingYunA/answer-me-with-html/blob/main/README.md#L339-L344)). This should be read as the repository's reported interpretation of the post, not a verbatim quotation. The project then separates content generation from presentation: the model writes a short Markdown draft while a CLI supplies layout, styling and diagram geometry (README lines 343–368). Its benchmark reports 4,893 versus 612 model-written tokens, 31 versus 12 seconds, and approximately 8× fewer output tokens / 2.6× faster for the tested corpus (lines 232–248); these are project measurements, not general LLM guarantees.

The repository also reports a practical output contract: one offline HTML file, generated from Markdown, with diagrams selected by information shape; real source excerpts can be embedded; and a writing check warns about long sentences, wordy phrases and passive voice (README lines 366–401). The README states that its writing check is adapted from ASD-STE100 and is warning-only unless strict mode is selected (lines 398–401).

## Source-backed evidence

ASD-STE100's official [About STE](https://www.asd-ste100.org/about_STE.html) describes STE as a controlled natural language developed for maintenance documentation, intended to reduce misunderstanding for readers who may not be native English speakers. The official [FAQ](https://www.asd-ste100.org/STE_faq.html) states that the standard has writing rules plus a controlled dictionary, generally assigns one meaning and one part of speech to approved words, and selects one synonym where alternatives exist. The official downloads page also warns that AI output can appear compliant while violating the standard and says human oversight and the standard itself remain necessary ([downloads](https://asd-ste100.org/STE_downloads.html)). Therefore, STE is a quality aid and review target, not proof that generated text is safe or compliant.

## Velvet implications

Velvet already has the ownership seams needed for this pattern:

- `plugin-api` models versioned runtime kinds, strict manifest decoding, contribution kinds including `PreviewProvider`, `PreviewAugmentation`, `Inspector` and `Job`, and capability-scoped permissions ([`crates/plugin-api/src/lib.rs:7-20`](../../crates/plugin-api/src/lib.rs#L7-L20), [`:124-173`](../../crates/plugin-api/src/lib.rs#L124-L173)). An output-format extension should be a declared capability/contribution, not an unrestricted renderer or shell command.
- `preview-core` bounds reads to 512 KiB by default and passes typed content rather than paths, database handles or credentials ([`crates/preview-core/src/lib.rs:28-50`](../../crates/preview-core/src/lib.rs#L28-L50), [`:68-78`](../../crates/preview-core/src/lib.rs#L68-L78)). Generated pages should consume bounded, host-approved projections and preserve truncation/error state.
- The preview model already admits a `UiBridge` renderer with an entrypoint and bridge version ([`crates/preview-core/src/lib.rs:103-128`](../../crates/preview-core/src/lib.rs#L103-L128)). This is a natural host-owned path for an offline explainer page, but the page must remain an isolated preview surface with explicit commands and slots.
- The UI bridge has versioned host/surface messages for context, theme, locale, resize, focus, errors and disposal ([`apps/desktop/src/workbench/plugins/ui-bridge.ts:7-68`](../../apps/desktop/src/workbench/plugins/ui-bridge.ts#L7-L68)). A generated page should use these projections rather than direct filesystem/database access. Its session lifecycle and failure isolation are already explicit ([`:111-123`](../../apps/desktop/src/workbench/plugins/ui-bridge.ts#L111-L123)).
- ADR-0005 requires lazy activation, capability grants, isolated WASM/sidecar execution, bounded RPC and failure isolation. ADR-0009 requires isolated UI surfaces and a typed bridge instead of framework internals. ADR-0012 keeps app namespaces and global operations/companions host-owned. These rules imply that an LLM-output plugin may render an app-scoped preview or inspector, but cannot create a second layout owner, bypass transport, or claim the global OperationsPanel.

## Prioritized recommendations

1. **Define a declarative “explanation document” projection first.** Add a versioned, bounded Markdown-plus-components DTO owned by Rust/plugin protocol. Permit `text`, tables, code excerpts, and a small allow-list of diagrams; reject arbitrary script, remote resources and unbounded embedded data. Keep source references as host-validated file/line projections.
2. **Implement the renderer as an isolated `UiBridge` preview surface.** Reuse `PreviewRenderer::UiBridge`, bridge version negotiation, theme/locale/context messages and retry/dispose behavior. Treat generated HTML as an untrusted artifact; use a sandboxed iframe/child WebView policy already required by ADR-0009.
3. **Keep generation in a Job contribution.** Route model calls and rendering through `JobSystem`/`Job` contributions with cancellation, progress, timeout and diagnostic state. Do not create format-specific loops in the desktop UI. Persist only extension-owned, versioned drafts or caches.
4. **Use STE as an opt-in lint/check, never an authority.** Expose warning diagnostics and an optional strict mode, but label it “STE-inspired” unless Velvet obtains and implements the official rules/dictionary. Preserve human review and show the source/rule version in diagnostics.
5. **Measure the real workload.** Reproduce token/time and readability comparisons on Velvet scenarios (asset explanation, feed comparison, plugin review) with fixed prompts and model/version metadata. Treat the answer-me-with-html figures as indicative project benchmarks.
6. **Add contract fixtures and failure tests.** Cover schema/version negotiation, size limits, malformed component drafts, unavailable providers, renderer crash, stale job results, locale/theme updates and cleanup after disable/uninstall.

## Open questions

- Should the first format be Markdown-plus-components rendered by Velvet, or a restricted HTML artifact produced by an external tool? The former gives stronger host control; the latter may ease adoption but expands sanitization and provenance work.
- Which diagram grammars are worth admitting (sequence, flow, tree, chart), and what are their maximum nodes, labels and render time?
- Should model/network access be a separate capability from `jobs.create` and `network`, with explicit user policy and redaction before prompts?
- How should generated pages cite asset provenance, plugin versions and source line ranges so users can distinguish evidence from model inference?
- Can Velvet ship a compatible STE-inspired checker without implying ASD certification or redistributing protected dictionary content?
- Should pages be transient preview sessions only, or can an extension persist drafts in its bounded storage declaration and rehydrate them after restart?

## Follow-up: LLM Wiki and a plugin data contract

Karpathy's [LLM Wiki idea file](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) makes the relationship clearer. It separates immutable raw sources, an LLM-maintained interlinked wiki, and a schema that defines the wiki's conventions and workflows (gist lines 62-87). Its operations are ingest, query, lint and filing useful answers back into the wiki (lines 88-102). This is a knowledge-maintenance workflow, not a replacement for a domain schema.

The controlled-language idea and plugin registration should therefore stay separate:

1. **Semantic contract** — a machine-readable, versioned data model: entity types, fields, relations, stable ids, provenance, freshness, capabilities and query operations. This is what Workbench indexing and Agents should consume.
2. **Knowledge projections** — derived summaries, concept pages, cross-links, indexes and optional embeddings. These can be regenerated or linted when sources change.
3. **Presentation contract** — STE-style text, diagrams, HTML and videos. These are human-facing projections selected by task and preference; they must not become the machine's only interface.

For Velvet, a future capability could declare a `DataModel`/`KnowledgeModel` resource and typed read/query operations. The manifest should advertise the model id, version, supported `ContentKind`/entity types, provenance policy, searchable fields and required capabilities. The full schema should be a versioned package resource rather than an opaque manifest blob. The host should own discovery, indexing, permission checks and cross-extension query routing; the extension should own its provider logic and namespaced derived state. `ExtensionStorage` is the extension's durable state, not the Workbench's global index.

This maps to the current vocabulary: `ContentKind` identifies what an Asset can be previewed as; `AssetFacet` is derived extension data; `PreviewProvider` and `Job` produce task-specific projections. A new model registry would cross the current plugin boundary, so it should be introduced only after a concrete consumer and recorded in the glossary/ADR. The foundation plan explicitly keeps product-specific indexing later and currently has no generic artifact cache ([plan](../../docs/engineering/workbench-foundation-plan.md#L16), [plan](../../docs/engineering/workbench-foundation-plan.md#L1020-L1022)).

The useful end-to-end shape is:

```text
raw sources / Assets / FeedItems
        ↓ ingest with provenance and fingerprints
versioned plugin data model + typed query capability
        ↓ host index and Agent discovery
knowledge projections: pages, summaries, links, facets
        ↓ task-selected presentation
STE-style text | diagram | interactive HTML | explainer video
```

The Agent should first discover the model and its operations, then ask for a projection. A response should carry source references, model/plugin version, input fingerprint and freshness status. This preserves the LLM Wiki benefit—knowledge compounds—without making model-generated prose authoritative or allowing a plugin to become a second persistence owner.
