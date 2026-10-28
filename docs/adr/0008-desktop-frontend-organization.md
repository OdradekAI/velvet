# ADR-0008: Desktop frontend organization and extraction boundary

Status: Accepted
Date: 2026-10-28

## Context

Velvet's desktop client must present a VS Code-like workbench while keeping the Rust core and future backend independent of the chosen Vue or React renderer. The first client has two built-in `WorkbenchApp` surfaces: Resource Manager and Feed Workbench. The central WorkspaceArea is an app surface and does not assume VS Code editor tabs. It also has host infrastructure for layout, views, previews and product extensions.

The vben project demonstrates a useful application boundary: `apps/*` contains runnable applications, `packages/*` contains reusable product capabilities, and `internal/*` contains build and tooling packages. Its application entrypoint composes shared capabilities before mounting the app. That organization is valuable, but Velvet currently has one desktop consumer and a Rust workspace that owns the domain.

DeepSeek Harness provides a second repository-level lesson: agent instructions have one source of truth. Its root `CLAUDE.md` is a symbolic link to `AGENTS.md`, and `.claude/skills` is a symbolic link to the local agent skill directory.

## Decision

Keep `apps/desktop` as the only frontend composition root until a second frontend or an independently versioned SDK consumes the same TypeScript capability. Do not create a root-level frontend `packages/` workspace merely to mirror vben.

Organize the desktop source by ownership and direction:

```text
apps/desktop/src/
  app/          startup and dependency composition
  platform/     CoreTransport and Tauri-facing adapters
  workbench/    framework-independent host: parts, layout, views, previews and zones
  workspaces/   built-in WorkbenchApp descriptors and app adapters
    asset-manager/
    feed-workbench/
```

The import direction is:

```text
app → platform, workbench, workspaces
workspaces → public workbench contracts and platform queries
workbench → platform contracts
platform → no Vue, React or feature module
```

The current desktop renderer is React with TypeScript and Vite. React is an
adapter choice for `apps/desktop`; it is not part of the Workbench protocol or
the product extension contract. The desktop UI uses an owned Velvet UI layer
implemented from shadcn-style source components and Tailwind CSS tokens. The
host exposes stable `Velvet*` wrappers to first-party extensions, while
third-party extensions use framework-independent descriptors and isolated UI
surfaces.

Shared UI primitives and cross-workspace feature modules will be added inside `apps/desktop/src` only when a real second consumer exists. At that point, extract the stable module to a named root package such as `packages/workbench-sdk` or `packages/ui-kit`; extraction is triggered by an ownership boundary, not by directory symmetry.

`workbench` remains framework-independent. React components are adapters owned
by the app layer and may consume `Workbench`, `ViewRegistry`, `PreviewHost` and
`LayoutService`. Product extensions receive protocol descriptors and capability
handles rather than internal React, shadcn or Tailwind implementation types.

The desktop `platform` module owns the TypeScript side of the versioned
CoreTransport contract. It mirrors the Rust `workbench-protocol` request,
response, event, revision and error concepts while the Tauri adapter performs
the explicit serialization mapping. The renderer may optimistically project a
layout command, but the Rust `LayoutService` remains authoritative and sends
back the revisioned snapshot; a stale expected revision is surfaced as a typed
transport error.

The UI ownership layers are:

```text
apps/desktop/src/app/ui/          Velvet UI implementation and React adapters
apps/desktop/src/app/renderer/    React Workbench shell and product mounting
apps/desktop/src/workbench/       framework-independent host contracts
apps/desktop/src/platform/        transport-facing types and adapters
```

The first-party UI layer may later be extracted as `@velvet/ui-react` after a
second consumer justifies an independently versioned package. Until then it
stays inside the single desktop composition root.

At the repository level, `CLAUDE.md` is a symlink to `AGENTS.md`. `.claude/skills` is a symlink to the repository-local `.agent/skills` directory, which may contain intentional skill definitions. Generated skill state remains ignored; the links provide one discoverable entrypoint without duplicating instructions.

## Alternatives considered

1. Copy vben's complete `packages/`, `internal/` and multi-app layout now. Rejected because there is one desktop client, and premature packages would obscure the Rust/desktop boundary.
2. Put WorkbenchApps inside `workbench/`. Rejected because an app is a product surface and feature owner, while the workbench is spatial infrastructure shared by every app.
3. Let React or shadcn components become the plugin API. Rejected because changing the renderer or UI implementation would become an ecosystem migration.
4. Keep separate `CLAUDE.md` instructions. Rejected because duplicated agent instructions drift; one `AGENTS.md` is authoritative.

## Consequences

Positive:

- The desktop app has a visible composition root without weakening the Rust core seam.
- Resource Manager, Feed Workbench and later contributed WorkbenchApps can grow independently while using one layout, shared context and plugin host.
- A future backend can reuse Rust protocol and domain crates without importing desktop modules.
- Frontend packages are extracted only when ownership and versioning justify them.
- Claude-compatible instruction lookup has one source of truth, with intentional skill definitions retained and generated state ignored.

Costs:

- The initial app keeps some reusable code local until a second consumer appears.
- The platform protocol needs deliberate versioning before it is published as an SDK.
- First-party extensions can share the Velvet UI visual system without making
  framework components part of the third-party extension ABI.

## References

- [Vue Vben Admin repository](https://github.com/vbenjs/vue-vben-admin)
- [DeepSeek Harness `AGENTS.md`](https://github.com/deepseek-ai/deepseek-harness/blob/master/AGENTS.md)
- [DeepSeek Harness `CLAUDE.md`](https://github.com/deepseek-ai/deepseek-harness/blob/master/CLAUDE.md)
- [VS Code workbench architecture](https://github.com/microsoft/vscode/tree/main/src/vs/workbench)
