---
id: documentation-structure
title: Documentation structure
---

# Documentation structure

The docs app owns navigation, translation review metadata, local search projections and demo interaction state. It consumes existing host UI and engineering contracts. It does not own a plugin SDK, runtime or persistent product state.

<span id="directory-map" className="velvet-doc-anchor" />
<span id="content-ownership" className="velvet-doc-anchor" />
<span id="choosing-the-correct-directory" className="velvet-doc-anchor" />

## Source ownership

| Source | Responsibility |
| --- | --- |
| `apps/docs/docs/en` and `apps/docs/docs/zh-Hans` | Task guides, concepts, UI examples and maintainer maps; matching relative paths |
| `docs/engineering` | Canonical English contracts and separate engineering records; only the publication allow-list is mounted |
| `apps/docs/reference/en` | Tracked symlink to `docs/engineering`; editor navigation only |
| `apps/docs/reference/zh-Hans` | Translations or explicitly labeled summaries of published reference pages |
| `docs/adr`, `docs/GLOSSARY.md`, `docs/research` | Decisions, canonical vocabulary and research evidence; link from the maintainer map |
| `apps/docs/i18n` | Docusaurus interface labels; primary Markdown stays in the paired language trees |
| `apps/docs/src/components/playgrounds` | Per-component demo state; imports use only the existing `@velvet-ui` entrypoint |
| `apps/docs/reference-pages.json` | Explicit public reference allow-list shared by build, checks and indexes |
| `apps/docs/translation-status.json` | Review status and hashes for English source and Chinese content |

`src/generated/page-metadata.json` and `static/search-index.json` are ignored build projections. `static/llms.txt` is generated and tracked. Never index all of `docs/engineering` or the repository: unpublished engineering records and scratch material must not enter the site by directory discovery.

## Reading paths and routes

The existing two docs plugin instances serve separate sidebars for plugin development, API/protocols, trusted UI and maintainers. Existing page URLs remain valid. The canonical English source and symlink remain unchanged; a physical migration to `docs/reference` is deferred until a real maintenance need justifies it.

A future relocation must preserve the old route with an explicit absolute `slug`, or provide a redirect, and update sidebar IDs, translations and indexes. New public reference pages must be admitted to `reference-pages.json`. Do not duplicate an English contract to obtain a different navigation entry.

## Contract and translation review

Every contract page should identify audience, owner/source, status, version, inputs/outputs, permissions, errors, cancellation/disposal and evidence. Separate design status (ADR), implementation evidence (fixture/host/platform) and compatibility promise (Alpha/stable). A source-visible internal type is not automatically a public API.

Translation states are:

- `reviewed`: both complete texts were compared. Any change to either hash fails the review gate.
- `summary`: intentionally condensed. Readers see a notice and a link to the English original.
- `unreviewed`: existing translation awaiting consistency review; the page shows a notice.

A changed summary or unreviewed page shows an additional stale notice. `npm run check:translations` requires every published page to be `reviewed`, and `verify:release` runs this stricter gate. Summary and unreviewed states remain available for local drafts. Hashes detect changes, not semantic equivalence. Do not mark an abbreviated reference as reviewed just to pass CI. API-critical Manifest fields/presence, Bridge message names and component prop tables have additional structural checks. All reviewed pairs must preserve contract literals, executable code blocks and API table names, types and defaults. These checks supplement paragraph-by-paragraph review; they do not prove prose equivalence.

After comparing the actual pair, record one page from `apps/docs`:

```sh
node tools/review-translation.mjs docs/platform/quick-start.md reviewed
node tools/review-translation.mjs reference/plugin-developer-guide.md reviewed
```

New pages require a status record; page parity alone is not translation review. A synopsis must never replace a schema with different field names. Preserve identifiers, error codes and explicit anchors when translating.

<span id="change-workflow" className="velvet-doc-anchor" />

## Change and validation workflow

1. Update the owning implementation/contract and its relevant fixture when behavior changes.
2. Update the guide and Chinese content; record the actual review state.
3. Update sidebars, interface translations and the reference allow-list as needed.
4. Run the docs release gate below. It generates indexes from public sources, checks content and import boundaries, runs checker regression tests, typechecks and builds both locales.
5. For workflow changes, run the documented example. For UI/navigation changes, inspect the built site, including Chinese routes, notices and search.

From the repository root:

```sh
npm --prefix apps/desktop ci
npm --prefix apps/docs ci
npm --prefix apps/docs run verify:release
```

The first command installs the real UI dependencies consumed by docs demos. No SDK package is extracted for that dependency. The docs tsconfig keeps `strict` and `isolatedModules` enabled.

`Documentation checks` runs on PRs with read-only repository permissions; it does not deploy. `Documentation Pages` builds main and deploys with permissions scoped to its deploy job. Both check out Git history, so last-updated dates describe source commits rather than file-copy time. New uncommitted pages have no reliable historical timestamp.

See `apps/docs/README.md` for development commands and supported Node version. Search and notices are regenerated at start/build; restart the development server after changing indexed content or translation records.
