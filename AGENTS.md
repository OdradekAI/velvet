# Velvet agent guide

This file is the repository's single source of truth for agent instructions. `CLAUDE.md` is a symlink to it, and `.claude/skills` is a symlink to `.agent/skills`. Keep those links intact and keep generated skill state in ignored directories.

## Operating contract

- Treat a user request to change, build or fix the repository as authorization for the in-scope local work. Inspect the relevant files, implement the complete change, and run the checks that match its risk.
- For questions, reviews, diagnosis or planning, inspect the relevant material and report the result. Implement only when the request also asks for a change.
- Ask before external writes, destructive actions, purchases, or a material expansion of scope. Safe local reads, edits, builds, tests and reviews do not need a confirmation step.
- State assumptions when they affect a design choice. If two interpretations would produce materially different systems, surface the choice before coding. Otherwise choose a reversible path and continue.
- Define what “done” means before a multi-step change and keep working until that criterion is verified. Do not stop after the first plausible implementation.
- Keep diffs surgical. Change the requested surface, remove only orphans created by the change, and leave unrelated cleanup for a separate task.

## Repository map and context routing

Velvet is a Rust and Tauri desktop workbench. The first product workspaces are Asset Manager and Feed Workbench. Rust owns durable state, domain rules, permissions, plugin lifecycle and background work; the desktop frontend renders projections and sends typed requests through the transport seam. The repository is still an architecture skeleton: ADRs 0001 and 0008 are accepted, while ADRs 0002–0007 describe proposed target behavior. Treat proposed rules as acceptance direction, not proof that the capability already exists.

Read only the documents that the task needs:

- Use `docs/GLOSSARY.md` before changing domain terminology or introducing a new concept. Use its terms in code, issues, tests and plans.
- Use the relevant files under `docs/adr/` for topology, ownership, layout, preview, plugin runtime, protocol, jobs or frontend organization. If a change conflicts with an ADR, call out the conflict and update or supersede the ADR with the change.
- Use `docs/engineering/workbench-foundation-plan.md` for the current implementation sequence, acceptance scenarios and structure checkpoints.
- For domain or architecture work, read `docs/agents/domain.md` first; it directs you to the glossary and relevant ADRs before exploration or design. Read `docs/agents/issue-tracker.md` and `docs/agents/triage-labels.md` when creating or updating `.scratch/` work.
- Use `README.md`, `Cargo.toml`, package manifests and compiler help as the source of truth for commands. Do not duplicate volatile command lists here.

## Architecture invariants

Preserve these seams unless an ADR explicitly changes them:

- `CoreTransport` is the intended boundary between the desktop and Rust `ApplicationCore`. The Rust trait lives in `crates/workbench-protocol`; the current desktop `apps/desktop/src/platform/protocol.ts` is a parallel skeleton contract with a different request/response shape. Keep the mapping explicit and update both sides until a generated or shared protocol replaces it. The Tauri adapter is an implementation of that boundary, not a second domain owner.
- The frontend owns rendering, transient interaction state and user input. It does not read the database, filesystem or plugin runtime directly.
- `apps/desktop/src/app` composes dependencies; `platform` owns transport-facing types and adapters; `workbench` owns spatial infrastructure; `workspaces` owns product-level workspaces. Keep the import direction `app → platform/workbench/workspaces`, `workspaces → public workbench and platform contracts`, `workbench → platform contracts`, `platform → no Vue/React or feature modules`.
- Rust crates own one clear responsibility: domain entities (`asset-core`), transport contracts (`workbench-protocol`), layout (`layout-model`), preview resolution (`preview-core`), extension contracts (`plugin-api`), extension lifecycle (`plugin-host`), jobs (`job-system`), persistence (`persistence`), application composition (`application-core`) and Tauri integration (`tauri-adapter`). Avoid dependency cycles and pass-through crates.
- The Rust layout service owns layout semantics and migrations; `persistence` is its storage adapter. The desktop `LayoutService` may adapt and migrate transport snapshots in the current skeleton, but it must not persist independently; remove duplicate migration logic when the protocol becomes canonical. Views dispatch typed commands and consume projections; they do not persist geometry themselves.
- Product workspaces remain stable Activity Bar entries. Extension views register through host registries and stay inside their declared semantic zone: `navigation`, `contextual` or `operations`.
- The planned preview host owns preview selection, loading, error, disabled-extension and fallback states. A provider owns a primary preview; augmentations, inspectors, tools and analysis jobs compose around it. Keep the current skeleton's generic fallback usable while the proposed preview contracts are incomplete.
- When implemented, long-running work goes through the Rust `JobSystem` with ownership, cancellation, progress, retry and diagnosable result state. The current skeleton only models submission, cancellation and snapshots, so wire it into `ApplicationCore` before production jobs. UI code does not create format-specific background loops.
- Asset identity, source locations, user state and derived extension data stay separate. Extension facets are planned to be namespaced and versioned; disabling or removing an extension must not make the asset inaccessible.
- `apps/desktop/src-tauri` is a future thin command boundary: decode a typed request, call `TauriTransport`, and return a typed response or event. Keep persistence, preview providers and plugin runtimes out of command handlers.

## Architecture admission and change control

Use the lightest process that protects a real boundary. A local change inside one module does not need an ADR. Apply the following gates when a change crosses ownership or makes a promise to other code:

1. **Name the boundary.** Before adding a crate, directory, service, registry or cross-boundary public type, identify its owner, consumers, state it may mutate, and the seam used to test it. A directory earns its place when it owns a meaningful interface, state model or adapter, or when a second consumer needs independent versioning.
2. **Check the decision record.** Search the glossary and relevant ADRs first. For a new durable architectural choice, add or update an ADR with status, context, decision, consequences, alternatives and open questions. Do not silently turn a `Proposed` decision into an `Accepted` one.
3. **Keep the early path small.** For the current v1 skeleton, do not add root `packages/`, extra clients, a daemon split, generic `common/`/`shared/`/`utils/` buckets or speculative configurability to mirror a future shape. Extract a package only when a second consumer or independently versioned SDK creates a real ownership boundary; reopen ADR-0001 or ADR-0008 when that trigger appears.
4. **Make cross-boundary contracts explicit.** New transport messages, externally consumed manifest fields, contribution kinds, permissions, facet schemas, persisted state or cache keys need stable identifiers, versioning or migration behavior, typed errors, cancellation semantics where applicable, and compatibility tests or fixtures. Local in-crate shape sketches may remain provisional and need only milestone-appropriate coverage.
5. **Give state one owner.** Every new durable field or cache must name its owner, provenance, migration path, invalidation rule and recovery behavior. Do not create a second persistence owner for state already managed by an existing service.
6. **Protect failure boundaries.** A plugin, provider, job or adapter may fail, time out, be disabled or be upgraded without taking down the workbench or corrupting core state. Prefer capability handles and typed protocols over raw paths, unrestricted commands or framework internals.
7. **Record durable impact.** If a change establishes or changes a durable architecture decision, protocol compatibility promise, extension installation contract, data-retention rule or recovery guarantee, update the relevant ADR, glossary, plan or acceptance scenario in the same change.

When a proposal passes these gates, implement a focused slice that proves the boundary. Defer marketplace, remote deployment, multi-client, arbitrary workspace placement and other ecosystem scale features until a concrete consumer requires them.

## Plugin and extension rules

The WASM/sidecar runtime, lazy activation, uninstall/rollback, schema validation and facet lifecycle below are target constraints from Proposed ADRs. Until those ADRs are accepted, keep the skeleton narrow and make any missing enforcement explicit in the relevant issue or plan.

- Keep Tauri host plugins (desktop/system capabilities) separate from product extensions (workbench, preview, asset and feed capabilities).
- Third-party extensions use the isolated WASM or sidecar model planned by the plugin ADR. Built-in adapters may be in-process only when they use the same public contribution contracts.
- Register contributions through `PluginHost`, `ViewRegistry`, `PreviewResolver` and other host-owned registries. Do not call another view's implementation, inject arbitrary DOM, bypass the transport, or access internal Vue/React component types.
- Validate extension id, contribution id, host API range, declared zone, permissions and schema versions before activation. Activation should be lazy and failure-isolated.
- Treat permissions as capabilities. Grant the narrowest zone and host operation set needed by the contribution; never infer network, process, filesystem or metadata-write access from a UI location.
- A disabled, incompatible, crashed or uninstalled extension leaves the core asset model, layout and generic fallback behavior usable. Persist extension-owned data so it can be migrated, recomputed or discarded independently.

## Work tracking and documentation

- Issues and feature specs live in `.scratch/<feature-slug>/`. Use `spec.md` for the feature, numbered files under `issues/` for child tickets, and the canonical triage labels from `docs/agents/triage-labels.md`.
- Create or update a local issue for a feature, architectural refactor, compatibility change or bug that needs more than a small local fix. Skip tracker ceremony for a typo, formatting-only change or narrow documentation correction.
- Keep design rationale and acceptance criteria in the relevant ADR or engineering document. Keep code comments for non-obvious rationale and recovery behavior, not for task tracking or restating code.
- Update `docs/GLOSSARY.md` when a concept becomes part of the product vocabulary. Update `CHANGELOG.md` when the repository's release history requires a user-visible entry.

## Validation

Run the narrowest meaningful checks, then broaden only when a failure or cross-boundary change justifies it:

- Rust changes: `cargo fmt --all -- --check` and the affected crate tests; run `cargo test --workspace` for shared contracts, persistence, plugin host, protocol, layout or cross-crate changes. Use `cargo clippy --workspace --all-targets` when lint behavior or broad Rust code changes are involved.
- Desktop TypeScript changes: from `apps/desktop`, run `npm run typecheck` after dependencies are installed. Keep `strict` and `isolatedModules` clean; do not use `any` or duplicate backend contract types to bypass errors.
- Protocol, manifest, layout, persistence, preview or plugin changes: add or update meaningful contract, migration, fallback, permission, cancellation or failure-isolation coverage. Prefer fake adapters and fixtures over UI-driven tests.
- Documentation-only changes: validate links and paths that were touched. Do not add tests that merely mirror prose.
- If a required tool is unavailable, report the exact command and environment limitation. Do not claim a check passed when it did not run.

## Code and writing conventions

- Rust: use idiomatic `Result`-based errors, avoid blocking synchronous work in async paths, use `tracing` for logs, and explain why when a comment is needed. Keep public interfaces small and framework-independent.
- TypeScript: keep protocol and domain types explicit, preserve strict compiler settings, and separate transport calls from view rendering.
- Prefer the smallest implementation that satisfies the request. Avoid speculative abstractions, one-use indirection and broad refactors.
- Keep comments short and causal. Track future work in issues or ADRs, not `TODO` comments that can silently become permanent.
- Write documentation in clear, direct language. Lead with the result, use canonical domain terms, and keep examples tied to a real contract or acceptance scenario.
