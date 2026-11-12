<p align="center">
  <img src="../../docs/logo/logo.png" alt="Velvet" width="120" />
</p>

# Velvet Desktop

[English](README.md) | [简体中文](README.zh.md)

Velvet's desktop application uses React, TypeScript and Vite for its interface,
and Tauri for the native host. This directory owns application composition,
rendering and native integration. Rust's `ApplicationCore` and domain crates own
durable product state, permissions, extension lifecycle and background work.

[Project overview](../../README.md) · [Frontend architecture](../../docs/adr/0008-desktop-frontend-organization.md) · [Native host](src-tauri/README.md) · [Documentation site](../docs/README.md)

## Prepare your environment

Use a full repository checkout and **Node.js 22** with npm, matching Desktop CI.
Native development also requires Rust stable and the platform dependencies in
the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). The launcher
preparation script uses Bash; on Windows, use the Bash environment available in
Git Bash or the equivalent build environment used by CI.

From the repository root:

```sh
cd apps/desktop
npm ci
```

All remaining commands run from `apps/desktop` unless noted otherwise.

## Run the application

### Browser development

```sh
npm run dev
```

Open the Vite URL printed in the terminal, normally `http://localhost:5173`.
Outside Tauri, the renderer starts `DemoApp`. Use this mode for layout, components
and fixture interactions; native transport and operating-system integration
require the Tauri application.

### Native development

Prepare the launcher binary for your current Rust target, then start Tauri:

```sh
bash ../../tools/build-release-launcher.sh
npm run tauri -- dev
```

Tauri starts Vite through `beforeDevCommand` and opens `NativeWorkbenchApp`.
Keep port `5173` available: `src-tauri/tauri.conf.json` points to that address.
Rebuild the launcher after changing its source or target architecture.

## Source ownership

| Path | Responsibility |
| --- | --- |
| `src/app/` | Startup and dependency composition |
| `src/app/renderer/` | React workbench shell, native/demo entrypoints and UI fixtures |
| `src/app/ui/` | Trusted Velvet UI components, tokens, contracts and stories |
| `src/platform/` | Transport-facing types and Tauri adapters |
| `src/workbench/` | Framework-independent layout, views, commands, previews and extension infrastructure |
| `src/workspaces/` | Built-in WorkbenchApp descriptors and adapters |
| `src-tauri/` | Native startup, command/event mapping, recovery and packaging |
| `test/`, `tools/` | Frontend regression tests, fixture runners and acceptance tooling |
| `../../crates/` | Rust application, domain and protocol owners |

Keep the import direction `app → platform/workbench/workspaces`,
`workspaces → public workbench/platform contracts`, `workbench → platform`.
`platform` must not depend on React or feature modules. Renderer components send
typed requests through the transport boundary; they do not read persistence or
plugin runtimes directly. See [ADR-0008](../../docs/adr/0008-desktop-frontend-organization.md)
for the full boundary rules and [the glossary](../../docs/GLOSSARY.md) for terminology.

The documentation site consumes `src/app/ui/index.ts` for trusted component
examples. These React exports are not a third-party extension SDK; extension
authors use the documented contribution contracts and isolated UI bridge.

## Validate a change

| Command | Purpose |
| --- | --- |
| `npm run typecheck` | Strict TypeScript validation |
| `npm test` | Compile fixtures and run frontend regression tests |
| `npm run build` | Typecheck and generate the Vite production bundle |
| `npm run generate:ui-kit-catalog` | Regenerate the tracked component catalog |
| `npm run verify:ui-kit` | Typecheck, catalog generation, tests, build and browser UI qualification |

For reproducible browser qualification with the CI runner:

```sh
npx playwright install chromium
UI_BROWSER_ENGINE=playwright npm run verify:ui-kit
```

The environment-assignment syntax above is for a POSIX shell. On Linux, CI uses
`npx playwright install --with-deps chromium` to install browser system dependencies
as well. Keep changes to the generated `src/app/ui/ui-kit-catalog.json` alongside
the contracts that produced them.

Rust checks run separately **from the repository root**:

```sh
cargo fmt --all -- --check
cargo test --workspace
bash tools/build-release-launcher.sh
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --features launcher-bin
```

`src-tauri` is a separate Cargo workspace, so root workspace tests do not cover it.
Choose checks for the boundary you change; transport, permissions and recovery
changes need the relevant Rust and frontend coverage. See [Desktop CI](../../.github/workflows/desktop.yml)
for the platform matrix and release acceptance steps.

## Build a native package

```sh
npm run tauri -- build
```

Tauri invokes `build:tauri`, which builds the frontend and prepares the launcher
sidecar. Bundles are produced beneath `src-tauri/target/` for the selected target.
`npm run build` alone produces only the web assets in `dist/`.

Signing, updater configuration and release qualification are handled by the
repository's release workflow. Follow the [release qualification guide](../../tools/release-qualification/README.md)
and [native host README](src-tauri/README.md) when changing those paths.

## Generated files and troubleshooting

[`.gitignore`](.gitignore) owns this application's dependency and build exclusions:
`node_modules/`, `dist/`, native `target/`, launcher `binaries/`, generated `gen/`
and `src-tauri/tauri.release.generated.json`. Repository-wide ignores remain in
the root file. Frontend test compilation currently writes to `/tmp/velvet-plugin-tests`;
test commands recreate its dependency link.

| Symptom | Check |
| --- | --- |
| Browser mode has no native integration | Run `npm run tauri -- dev`; browser development intentionally uses `DemoApp` |
| Tauri cannot load the frontend | Check that Vite is running on the configured port `5173` |
| Launcher binary is missing or has the wrong target | Rerun `bash ../../tools/build-release-launcher.sh` for the selected Rust target |
| Native compile fails on system libraries | Check the platform's Tauri prerequisites and CI setup |
| Browser qualification cannot find Chromium | Install it with Playwright and select `UI_BROWSER_ENGINE=playwright` |
