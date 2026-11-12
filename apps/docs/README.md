<p align="center">
  <img src="../../docs/logo/logo.png" alt="Velvet" width="120" />
</p>

# Velvet Docs

[English](README.md) | [简体中文](README.zh.md)

This directory contains Velvet's documentation website, built with Docusaurus.
It publishes English and Simplified Chinese guides for extension developers,
API references, UI Kit examples, and repository maintenance documentation.
This README explains how to contribute content and maintain the site.

[Read the documentation](https://odradekai.github.io/velvet/) ·
[About Velvet](../../README.md) ·
[Documentation architecture](docs/en/maintenance/documentation-structure.md)

## Develop locally

Use **Node.js 22** and npm, matching CI. Work from a checkout of the full
repository: the site consumes engineering documents and desktop UI components
outside this directory. Install both dependency sets from the repository root:

```sh
npm --prefix apps/desktop ci
npm --prefix apps/docs ci
cd apps/docs
npm run start
```

The development server serves English by default and reloads page edits.
All remaining commands run from `apps/docs` unless noted otherwise.

| Task | Command |
| --- | --- |
| Develop English content | `npm run start` |
| Develop Chinese content | `npm run start -- --locale zh-Hans` |
| Build both languages | `npm run build` |
| Preview the existing build | `npm run serve` |

A development server serves one language at a time. To test the language switch,
build both languages and use `npm run serve`; English is under `/`, and Chinese
under `/zh-Hans/`. Rebuild after editing content in a production preview.

## Find the right source

Paths below are relative to this directory.

| Path | What to maintain |
| --- | --- |
| `docs/en/`, `docs/zh-Hans/` | Paired guides, concepts, UI examples, and maintenance pages |
| `../../docs/engineering/` | Canonical English reference contracts; edit them here |
| `reference/zh-Hans/` | Chinese translations of published reference contracts |
| `reference-pages.json` | Explicit list of engineering pages published as Reference |
| `sidebars.ts`, `sidebars-reference.ts` | Guide and Reference navigation |
| `i18n/` | Translated navigation labels and other Docusaurus interface text |
| `src/components/`, `src/theme/`, `src/css/` | Interactive examples, theme customizations, and styles |
| `static/` | Public assets served by the site |
| `tools/`, `translation-status.json` | Documentation checks, index generation, and translation review records |
| `docusaurus.config.ts` | Site configuration, locales, content sources, and deployment URLs |

`reference/en` is a tracked symlink to the [canonical engineering directory](../../docs/engineering/).
Keep it for editor navigation; the build reads the canonical directory directly. Only entries in
`reference-pages.json` are published from that directory. The complete source
ownership rules are in the [documentation architecture guide](docs/en/maintenance/documentation-structure.md).

## Add or update documentation

1. **Choose the content type.** Put task instructions and concepts in `docs/en/`.
   Put API contracts in their canonical engineering source. Keep design rationale
   in the relevant [ADR](../../docs/adr/) and use the [glossary](../../docs/GLOSSARY.md)
   for domain terminology.
2. **Write for the reader's task.** State prerequisites, use a runnable example,
   and explain the expected result. References should cover inputs, outputs,
   permissions, errors, and lifecycle behavior. Distinguish implemented behavior
   from proposals and fixture-only behavior; link to the contract instead of
   duplicating it in each guide.
3. **Update both languages.** Use matching relative paths in the language trees.
   Preserve API identifiers, code examples, explicit anchors, and route identity.
   Point Chinese links to the corresponding Chinese page where one exists.
4. **Connect the page.** Update the relevant sidebar and translated labels in
   `i18n/`. Add new Reference pages to `reference-pages.json` using paths relative
   to `docs/engineering`. When moving or renaming a page, preserve its public URL
   with an explicit `slug` or a redirect, and update inbound links.
5. **Review and validate.** Record the translation review, run the checks below,
   and preview the affected pages in both languages. Run any example whose
   instructions changed.

Keep pull requests focused. Describe the reader-facing change, affected pages,
and validation performed; include screenshots for navigation or visual changes.

## Review translations

`translation-status.json` records the review status and content hashes for each
published English/Chinese pair. After comparing the complete texts and checking
API details against their owning source, record the affected page:

```sh
node tools/review-translation.mjs docs/platform/quick-start.md reviewed
```

Use a `docs/` key for a guide or a `reference/` key for a reference page, followed
by its relative Markdown path. Any later edit to either language invalidates the
recorded review. Update the record only after reviewing the pair again.

Local drafts may use `summary` or `unreviewed`; the site displays their status and
links to the English original. Publication requires every pair to be `reviewed`.
Hashes detect changes, while structural checks compare API literals, types,
defaults, and executable examples. Neither replaces review of the prose.

## Maintain the site and examples

UI examples consume the desktop's existing public UI entrypoint through
`@velvet-ui`. Keep example state in `src/components/playgrounds/`; use that
entrypoint instead of importing desktop internals. Site code must pass strict
TypeScript checks. Changes to desktop behavior belong in the desktop application
and require its own validation.

The metadata generator derives page notices and the route inventory from
published sources. `npm run start`, `npm run build`, and `npm run typecheck` run it
automatically. To refresh these files explicitly:

```sh
npm run generate:index
```

| Generated path | Version control |
| --- | --- |
| `static/llms.txt` | Commit changes when published routes change |
| `src/generated/` | Ignored page metadata; regenerate locally |
| `.docusaurus/`, `build/` | Ignored caches, search indexes and build output |

Edit source files rather than generated output. Keep dependency changes and their
`package-lock.json` updates together, then run the full release check.
Application-specific exclusions live in [`.gitignore`](.gitignore); repository-wide
exclusions remain in the root file.

Shared brand assets live in [`docs/logo`](../../docs/logo/). This README uses
`logo.png`; `static/img/favicon.svg` links to `logo_simple.svg`. Preserve the
relative symlink so the favicon has a single source.

## Search and reading tools

[`@easyops-cn/docusaurus-search-local`](https://github.com/easyops-cn/docusaurus-search-local)
provides the navbar search and `/search` results page. Readers can focus search
with **Cmd/Ctrl+K**, navigate suggestions with arrow keys, open a result with
Enter, and dismiss suggestions with Escape. Results include section paths and
matching snippets; following a result highlights the search terms on its page.

The plugin builds separate locale indexes from published HTML. Chinese pages use
Chinese segmentation alongside English API tokens. The existing Reference
allow-list still controls publication; no hosted search account or API key is
needed. Keep search configuration in `docusaurus.config.ts` and let the plugin own
the search routes, rather than adding a Markdown page at the same path.

Search indexes are produced by a **production build**, not the development
server. After changing content or search configuration, use `npm run build`
followed by `npm run serve` to test results in both languages. Page review notices
remain the responsibility of `generate:index`.

The classic theme supplies the section table of contents, code-copy controls,
previous/next links and back-to-top control. Sidebars can be collapsed for more
reading space, and opening a category collapses its siblings. Prefer these theme
features before adding custom reading components.

## Validate before opening a pull request

Run the complete site check before submitting a change:

```sh
npm run verify:release
```

This regenerates indexes, requires complete translation reviews, checks content
and API consistency, runs checker regression tests, typechecks site code, and
builds both languages, and verifies search coverage and section links in the
generated indexes. Broken links and anchors fail the production build.

Use narrower checks while editing:

| Command | Purpose |
| --- | --- |
| `npm run check:docs` | Page parity, source links, review freshness, contract checks, and index coverage |
| `npm run check:translations` | All documentation checks, plus a `reviewed` status for every published pair |
| `npm run check:links` | Internal Markdown links |
| `npm run check:source-link` | The tracked English reference symlink |
| `npm run test:docs` | Regression tests for the documentation checker |
| `npm run typecheck` | Strict TypeScript validation |
| `npm run check:search` | Published-page coverage and section targets in an existing production build |

For UI or navigation changes, also inspect the built pages, language switch,
search, and interactive examples. [`Documentation checks`](../../.github/workflows/docs-checks.yml)
runs the release check and the documented Asset Inspector fixture on pull
requests with read-only repository permissions.

## Build and deploy

[`GitHub Pages`](../../.github/workflows/github-pages.yml) validates and
publishes `main` to [GitHub Pages](https://odradekai.github.io/velvet/). It also
supports manual dispatch. The deploy job publishes `build/` through the
`github-pages` environment; pull request checks do not deploy.

To reproduce the Pages build locally, set the deployment environment for both
the build and preview (POSIX shell):

```sh
export DOCS_DEPLOY_TARGET=github-pages
export DOCS_BASE_URL=/velvet/
npm run verify:release
npm run serve
```

| Variable | Default and purpose |
| --- | --- |
| `DOCS_DEPLOY_TARGET` | `local`; `github-pages` selects the Pages URL and base path defaults |
| `DOCS_SITE_URL` | `https://velvet.local` locally; `https://odradekai.github.io` for Pages |
| `DOCS_BASE_URL` | `/` locally; `/velvet/` for Pages; retain the trailing slash |

Use a new shell or unset these variables to return to root-path development.
Keep full Git history in CI so page update dates reflect source commits.
See the [Docusaurus deployment guide](https://docusaurus.io/docs/deployment) when
changing the hosting configuration.

## Troubleshoot

| Symptom | Action |
| --- | --- |
| Missing desktop UI dependency or `@velvet-ui` import | Run `npm --prefix apps/desktop ci` from the repository root; use the full checkout |
| Search shows a build-required message or stale results | Run `npm run build`, then `npm run serve`; search is available in production previews |
| Translation notices are stale | Run `npm run generate:index` and restart the development server |
| Missing bundle chunks after a configuration or dependency change | Run `npm run start:clean`, then hard-refresh the browser |
| Language switch fails in development | Build both languages and preview with `npm run serve` |
| Translation review is stale | Compare the affected pair, correct it, then record the review again |
| Assets or links fail only on Pages | Reproduce with the `/velvet/` deployment environment above |
