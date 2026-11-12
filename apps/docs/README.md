# Velvet Docs

This is the Docusaurus documentation app for Velvet's UI Kit and plugin platform.

## Commands

```sh
cd apps/docs
npm install
npm run start
npm run start:clean
npm run start:all
npm run check:docs
npm run check:links
npm run check:source-link
npm run verify:release
npm run typecheck
npm run build
npm run serve
npm run serve:all
```

The default development server uses English. Build output includes the
Chinese locale under `build/zh-Hans`. To run the Chinese development server,
use `npm run start -- --locale zh-Hans`.

The language dropdown preserves the current document path in a production
build or `npm run serve` preview. A single-locale development server only
serves the locale passed to `docusaurus start`, so use the matching
`--locale` flag when testing the other side of the language switch.

Use `npm run start:all` when you want one local server with both `/` and
`/zh-Hans/` routes available. `npm run serve:all` remains an equivalent
preview command.

`start:clean` clears Docusaurus's generated route and bundler cache before
starting the dev server. Use it after changing Docusaurus dependencies,
sidebars, plugin instances, or the external desktop UI alias. A browser tab
that still reports a missing chunk should also be hard-reloaded after the
server restarts.

The app keeps English and Chinese Markdown sources side by side. The public
navigation pages are in `apps/docs/docs/en` and `apps/docs/docs/zh-Hans`. The
Chinese Reference pages are in `apps/docs/reference/zh-Hans`. English Reference
pages are built directly from the canonical engineering contracts under
`docs/engineering`. The tracked symlink `apps/docs/reference/en` remains as a
repository navigation and editor comparison entry point.

The [documentation structure guide](docs/en/maintenance/documentation-structure.md)
defines the ownership boundary between public guides, Reference contracts,
Docusaurus locale resources, executable demos and engineering source records.

`npm run check:docs` checks English and Chinese page parity, the English
Reference source and symlink target, localized Chinese links, English
translation residue, internal Markdown links and every route listed in
`static/llms.txt`. `npm run check:links` runs only the internal link check.
`npm run check:source-link` verifies that `apps/docs/reference/en` is a tracked
symlink to `docs/engineering`.

`npm run verify:release` is the local release gate. It runs the documentation
checks, TypeScript validation and the all-locale production build. CI uploads
the resulting `build` directory as the documentation release artifact.

The `Documentation Pages` workflow publishes the main branch to
`https://odradekai.github.io/velvet/`. It builds with the project-site base path
`/velvet/`, uploads the Pages artifact and deploys it through the `github-pages`
environment. Local development keeps the root base path `/`.
