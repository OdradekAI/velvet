# Velvet

Your assets and information should be understandable at a glance.

[中文说明](README.zh.md) · [Architecture](docs/adr/) · [Changelog](CHANGELOG.md) · [License](LICENSE)

---

> [!NOTE]
> Velvet `0.0.1` is an architecture foundation release. The workbench and extension seams are in place; the full Tauri window, asset indexer and production extensions are still being built.

## One workbench, two starting points

Velvet is a Rust and Tauri desktop workbench for organizing assets and information in one adaptable workspace. Its Chinese name is **白翎**.

The first built-in workspaces are:

- **Asset Manager** for images, RAW files, video, audio, documents and 3D models.
- **Feed Workbench** for subscriptions, messages and reading material.

## A layout that explains itself

The interface follows the spatial ideas of VS Code and JetBrains IDEs. Each region has one clear job, so users can understand where a feature belongs without learning a plugin-specific layout.

```text
Activity Bar       switch between built-in workspaces
Primary Sidebar    navigation, libraries, sources and collections
Workspace Area     grids, previews, comparisons and editors
Context Sidebar    inspectors, metadata and third-party tools
Operations Panel   jobs, logs, sync and terminal-like operations
Status Bar         current state and progress
```

The initial extension zones are fixed by purpose:

1. `navigation` for workspace and library navigation;
2. `contextual` for metadata, inspection and preview tools;
3. `operations` for jobs, logs and operational tools.

Extensions can customize content inside their allowed zone. A contribution cannot move across zones in the initial architecture.

## Preview at the depth you need

Velvet treats preview as a host-managed capability rather than a separate full-screen viewer for every file format.

- **Quick** shows a clean, fast preview.
- **Inspect** shows useful file and metadata information.
- **Analyze** exposes specialist tools such as RAW histograms, RGB curves, color information or 3D material controls.

The preview system separates primary renderers from augmentations:

```text
ContentKindProvider   recognizes a file kind
PreviewProvider       renders the main preview
InspectorContribution adds metadata and analysis sections
PreviewTool           adds interactive commands
AnalysisJob           performs expensive background work
```

A 3D or RAW extension can provide a renderer, thumbnails, metadata and analysis. A metadata-only extension can add information to the Context Sidebar without replacing the main preview.

## Extend by capability

An extension manifest declares its contributions and permissions. The host validates the zone before registration.

- Navigation extensions use library, source and workspace APIs.
- Contextual extensions use asset, metadata and preview APIs.
- Operations extensions use jobs, logs and explicitly granted process or network APIs.

Third-party extensions are designed for isolated WASM or sidecar runtimes. Built-in adapters may run in-process, but they use the same public contribution model. Extensions communicate through stable protocol types and do not depend on internal Vue or React component types.

## Architecture

The first release uses a local-first, single-process topology:

```text
Tauri webview
      │ CoreTransport
      ▼
Rust ApplicationCore
      ├── layout state and migrations
      ├── preview resolver
      ├── plugin host
      ├── persistence
      └── jobs and indexing
```

The frontend owns rendering and interaction. Rust owns durable state, permissions, plugin lifecycle, preview selection and background work. The Tauri adapter stays thin so a future local daemon or remote transport can implement the same protocol.

The desktop frontend follows an application-first organization inspired by vben's separation of runnable apps, reusable capabilities and tooling, while keeping the first client self-contained:

```text
apps/desktop/src/
  app/          startup and dependency composition
  platform/     CoreTransport and Tauri-facing adapters
  workbench/    layout, parts, views, previews and zones
  workspaces/   Asset Manager and Feed Workbench product modules
```

The workbench stays independent of Vue and React. A renderer consumes its public descriptors, while a future frontend package is extracted only when a second client or independently versioned SDK needs it.

## Build from source

### Rust workspace

Install Rust through [rustup](https://rustup.rs/), then run:

```bash
cargo test --workspace
cargo fmt --all -- --check
```

### Desktop TypeScript skeleton

```bash
cd apps/desktop
npm install
npm run typecheck
```

The current desktop package contains the app composition root, platform transport types, framework-independent Workbench, two built-in workspace descriptors, preview host, view registry, zone policy and plugin client interfaces. The complete Tauri window and production UI are planned next.

## Repository layout

```text
crates/
  application-core/       Rust state owner and request dispatcher
  workbench-protocol/     commands, queries, events and transport seam
  layout-model/           Parts, zones and versioned layout state
  preview-core/           intents, providers, resolver and fallback
  plugin-api/             manifests, contributions and permissions
  plugin-host/            registration, activation and zone validation
  persistence/            layout storage seam
  job-system/             background job model
  tauri-adapter/          Tauri-facing transport adapter

apps/desktop/
  src/app/                 desktop composition root
  src/platform/            framework-independent core transport types
  src/workbench/           layout and extension host infrastructure
  src/workspaces/          Asset Manager and Feed Workbench modules
  src-tauri/               future Tauri command integration boundary
```

## Project documents

- [Glossary](docs/GLOSSARY.md)
- [Architecture decisions](docs/adr/)
- [Workbench foundation plan](docs/engineering/workbench-foundation-plan.md)
- [Change history](CHANGELOG.md)
- [License](LICENSE)

## License

Velvet is distributed under the [Apache License 2.0](LICENSE).
