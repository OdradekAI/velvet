---
id: documentation-structure
title: Documentation structure
sidebar_label: Documentation structure
---

# Documentation structure

The docs app separates public guidance, API reference material, and Docusaurus localization resources. The separation gives each kind of content one owner and keeps changes reviewable for a large repository.

## Directory map

```text
apps/docs/
├── docs/
│   ├── en/              public platform guides in the default locale
│   └── zh-Hans/         Chinese mirror of the public platform guides
├── reference/
│   ├── en -> docs/engineering/  tracked symlink to the canonical source
│   └── zh-Hans/         Chinese mirror of the API and contract reference
├── i18n/
│   ├── en/              Docusaurus metadata and theme resources
│   └── zh-Hans/         Docusaurus metadata and theme resources
├── src/                 interactive demos, theme overrides and showcase CSS
└── static/              assets and machine-readable indexes such as llms.txt

docs/engineering/        canonical English Reference and engineering records
```

The English and Chinese Markdown trees are intentionally side by side. A reviewer can compare the same relative path in `en` and `zh-Hans` without searching through generated locale directories.

## Content ownership

| Path | Responsibility | Maintainer action |
| --- | --- | --- |
| `apps/docs/docs/en` | English tutorials, platform concepts, contribution workflows and UI Kit usage guides | Update when a developer workflow or public navigation path changes |
| `apps/docs/docs/zh-Hans` | Chinese translation of the public platform guides | Keep the same relative paths and route links as the English tree |
| `apps/docs/reference/en` | Tracked symlink for repository navigation and editor comparison | Keep the link target at `docs/engineering` |
| `docs/engineering/ui-kit` | English API, component, overlay and host-projection reference built directly by Docusaurus | Change this source when a UI Kit contract changes |
| `apps/docs/reference/zh-Hans` | Chinese API and contract reference | Translate the English Reference and preserve props, states, actions and contract identifiers |
| `docs/engineering` | Engineering source for contracts, qualification evidence and design decisions | Change this source first when a contract or durable engineering rule changes |
| `apps/docs/i18n` | Docusaurus-generated sidebar, navbar, footer and code translation resources | Update through Docusaurus translation tooling. Do not use it for primary Markdown pages |
| `apps/docs/src` | Executable demos, theme overrides and showcase styling | Change when a demo or docs rendering behavior changes |
| `apps/docs/static` | Public assets and machine-readable indexes | Update `llms.txt` when a public route is added or removed |

## Choosing the correct directory

Use `docs/en` and `docs/zh-Hans` for material that teaches a plugin developer how to use Velvet. Examples include quick starts, contribution selection, security guidance and UI Kit customization.

Use `docs/engineering/ui-kit` and `reference/zh-Hans` for stable contracts. The English docs app Reference reads `docs/engineering` directly. The tracked `reference/en` symlink provides a side-by-side repository entry point. Examples include component props, states, keyboard behavior, accessibility semantics, host ownership, migration rules and qualification evidence.

Use `i18n` for interface strings generated or consumed by Docusaurus. A new page belongs in the parallel Markdown trees. A sidebar label belongs in `sidebars.ts` and its locale JSON resource.

Use `src` when the change is executable. A playground should expose a real component state and be imported by the matching page in both language trees. The textual API explanation stays in the Markdown page.

Engineering pages may link to the published `/reference/...` route when a reader needs the rendered docs site. The link is a navigation aid. The English build reads `docs/engineering` directly, while the tracked symlink supports local source discovery without duplicating content.

## Change workflow

1. Change the desktop contract, story or implementation when the behavior changes.
2. Update the matching engineering source under `docs/engineering`.
3. Verify that `apps/docs/reference/en` still points to `docs/engineering` and that the affected English Reference page is available through the direct canonical mount.
4. Update the Chinese Reference page under `apps/docs/reference/zh-Hans`.
5. Update the English and Chinese platform guides when the user workflow changes.
6. Update `sidebars.ts`, locale JSON resources or `static/llms.txt` when navigation or machine-readable coverage changes.
7. Run the docs checks, typecheck and production build.

```sh
cd apps/docs
npm run check:docs
npm run check:links
npm run check:source-link
npm run typecheck
npm run build
npm audit --audit-level=low
```

`check:docs` verifies relative-path parity, the English Reference source and symlink target, localized Chinese links, English-language residue, internal Markdown links and `llms.txt` routes. `verify:release` packages the same checks with the all-locale production build. CI publishes that build as a release artifact without changing the canonical source.

The `Documentation Pages` workflow builds the same release gate with the project-site base path and deploys `build/` through the `github-pages` environment. The public site is `https://odradekai.github.io/velvet/`.
