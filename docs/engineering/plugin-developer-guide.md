# Velvet plugin developer guide (alpha contract)

This guide takes a third-party developer from a local package to the current
runnable fixture host. It documents the public contract exercised by the Asset
Inspector demo and marks the parts that are still fixture-only or release-gated.
It is not a promise that a Marketplace, a production SDK package, or an
unrestricted in-process runtime exists.

## Choose a starting fixture

- `examples/plugins/asset-inspector/` is the Resource Manager (Asset Manager)
  path. It registers
  Views, a context command, a keybinding, a Preview augmentation and a Job,
  then writes a namespaced facet after checking the Asset fingerprint.
- `examples/plugins/echo-sidecar/` is the external runtime path. It uses a
  sidecar JSONL protocol, exposes a UI Bridge View and Preview actions, and
  intentionally crashes so the host can remove and recover only that
  extension.

Run the complete desktop fixture suite from the repository root:

```sh
npm --prefix apps/desktop test
```

Run only the Asset Inspector walkthrough while iterating on the fixture:

```sh
npm --prefix apps/desktop run test:compile
node apps/desktop/test/asset-inspector-demo.test.mjs
```

The sidecar can also be exercised directly:

```sh
printf '%s\n' '{"protocolVersion":1,"kind":"invoke-command","extensionId":"velvet.example.echo","commandId":"echo.open","args":{"source":"manual"},"context":{"activeAppId":null,"keys":{}}}' \
  | node examples/plugins/echo-sidecar/sidecar.mjs
```

## Package layout

Keep package contents relocatable and addressable through the manifest:

```text
my-extension/
  manifest.json
  README.md
  src/                 # trusted fixture or future isolated entrypoint
  ui/                  # optional UI Bridge/static surface resources
  l10n/                # optional declared localization bundles
  test/                # host contract fixtures; never a runtime capability
  assets/              # package resources, if a host contract permits them
  dist/                # optional build output submitted as a directory or ZIP
```

The host discovers committed packages under its application data plugin
directory. Relative launch paths are resolved against the committed package
root and are rejected if canonicalization escapes that root. A package must
not require an absolute executable path, an undeclared host command, a raw
filesystem path, or a Tauri capability.

`ui/`, `l10n/`, `test/` and `dist/` are package conventions, not implicit
permissions. A surface or locale becomes visible only when a manifest
contribution declares it and the host validates the entrypoint/resource
allow-list. The current examples are intentionally small and omit optional
directories.

## Manifest, IDs and compatibility

Start with [plugin-manifest-v1.md](plugin-manifest-v1.md). The current manifest
uses independent compatibility surfaces:

| Field | Rule |
| --- | --- |
| `id` | Stable extension identity. Keep it across upgrades. |
| Contribution `id` | Stable within the manifest and unique for that extension. |
| `manifestSchema` | Current canonical schema is `1`. Unknown required fields fail closed; only top-level `x-*` metadata is reserved. |
| `hostApi` | Host contract range. A major mismatch prevents activation. |
| `bridgeApi` | UI Bridge contract range. An incompatible surface is failed and disposed. |
| `runtimeProtocol` | Runtime envelope range for an external runner. |
| `targetApp` and `scope` | Keep WorkbenchApp and semantic zone explicit. The host owns placement. |
| `requiredCapabilities` / `optionalCapabilities` | Declarations only. Effective grants are host policy. |
| `storage` | Namespaced extension schema and quota declaration. It does not expose a raw path. |

Use the extension ID as the root of contribution IDs and facet namespaces. Do
not use a user path, Asset path or React component name as an identity.

## Activation and lifecycle

Activation events are selectors, not permission grants. The host validates the
manifest, target app, zone, capabilities and dependency order before starting
an extension. Activation registers declarative contributions through host
registries. Deactivation, disable, runtime crash, rollback and uninstall must
dispose every contribution owned by that extension.

The current host lifecycle is:

```text
discovered → validated → registered → active
                                      ├─ disabled
                                      ├─ failed
                                      └─ quarantined
active → recovered → active
active → uninstalled → absent
```

The renderer receives projections after Rust accepts the registration. It may
cache descriptors, but it cannot make an inactive extension active by mutating
the local registry.

## Install, run, upgrade and uninstall

The current Extensions dialog exposes the same typed sequence used by the
transport. A developer can walk it without calling a private Tauri API:

1. Choose **Browse directory…** or **Browse .zip…** and select a package.
2. The host sends `inspect-plugin` or `inspect-plugin-archive`. It parses the
   manifest before copying bytes and returns the extension ID, version, target
   app, host API compatibility, runtime and requested permissions. An
   incompatible or malformed package stops here.
3. Review the preview. The host then stages the package with `install-plugin`
   or `install-plugin-archive`. Staging leaves the old committed package and
   extension data available.
4. The host activates the staged extension lazily, projects its contributions,
   and commits with `commit-plugin-install` only after activation and UI
   projection succeed. A projection, activation or runtime failure invokes
   `rollback-plugin-install`, removes a new package or restores the previous
   package, and leaves unrelated extensions active.
5. An upgrade keeps the same extension ID and supplies a newer numeric
   version. The host reports `operation: upgrade`; the old package/data remain
   until the new health/activation commit. A downgrade is rejected by the
   package policy unless an explicit recovery path permits it.
6. Disable/recover actions use the host-owned `disable-plugin` and
   `recover-plugin` commands. Disable removes the extension's contributions
   and cancels its owned Jobs; recovery starts a fresh runtime and reprojects
   the validated manifest.
7. After confirmation, **Uninstall** sends `uninstall-plugin`, stops the
   runtime, removes the installed package and unregisters its projections.
   Extension data is retained by default for migration, reinstall or explicit
   user cleanup; external Asset sources are never deleted implicitly.

The concrete Rust and TypeScript seams are
`crates/workbench-protocol/src/protocol.rs`,
`crates/application-core/src/composition.rs` and
`apps/desktop/src/workbench/plugins/plugin-client.ts`. The repeatable upgrade,
rollback and uninstall fixtures are listed in the
[contract matrix](plugin-contract-matrix.md).

## Contribution rules

Register a command, menu item, keybinding, View, Preview provider/augmentation
or Job in the manifest and let the host create its registration. The host owns
Activity Bar entries, pane layout, PreviewHost primary selection, Operations
Panel, global overlays and global shortcuts. A contribution declares its
semantic zone (`navigation`, `contextual` or `operations`) and stays inside it.

The plugin may provide content for a registered View through the UI Bridge and
may use host semantic tokens. It cannot inject arbitrary DOM or CSS, call
another View's implementation, install a global key handler, or replace the
layout owner.

## Fixture HostFacade API catalog

There is no production `HostFacade` Rust or TypeScript type yet. The following
table names the structural fake used by
`examples/plugins/asset-inspector/src/asset-inspector.mjs` so a developer can
understand the intended ownership boundary without mistaking it for a stable
SDK.

The fake methods only record calls and return bounded fixture values. The last
column describes host policy that the real PluginHost/registry/transport must
enforce; it is not evidence that the fake replaces the host. The fixture's
concrete assertions are in `apps/desktop/test/asset-inspector-demo.test.mjs`,
and `node examples/plugins/asset-inspector/run-demo.mjs` runs the same package
without the desktop renderer.

| Fixture handle | Operation | Host owner | Plugin may read/write | Capability and failure rule |
| --- | --- | --- | --- | --- |
| `commands.register(id, handler)` | Register a command callback | Command registry / PluginHost | Register and dispose its own command | Manifest command plus declared zone; duplicate or undeclared IDs fail locally. |
| `menus.register(id)` | Register a context-menu contribution | Menu registry / PluginHost | Register and dispose its own menu item | Menu location and `when` expression are host-validated. |
| `keybindings.register(id)` | Register a keybinding | Keybinding registry / host | Register and dispose its own binding | The host resolves conflicts and keeps global shortcut ownership. |
| `views.register(id)` | Register a View descriptor | View registry / Workbench | Register a declarative View | Zone, container and namespace are host-owned; no sibling View handle is exposed. |
| `preview.registerAugmentation(id, handler)` | Register a Preview augmentation | PreviewResolver / PreviewHost | Read the bounded Preview context and return an augmentation | Provider failure becomes a typed fallback/diagnostic; it cannot replace core Asset state. |
| `assets.readProjection(id)` | Read path-free Asset projection | Asset source owner / ApplicationCore | Read selected identity, content kind and fingerprint | Requires `asset.read`; no database, source path or raw file handle crosses the seam. |
| `jobs.create(request)` / `jobs.wait(id)` / `jobs.cancel(id)` | Submit, observe and cancel a Job | `job-system` | Submit bounded input, read progress/result and cancel an owned Job | Requires `jobs.create`/`jobs.read`; cancellation, deadline, stale provenance and retry are host policy. Cancellation is owner-scoped and Rust `JobSystem` remains authoritative. |
| `facets.read({namespace, assetId})` | Read extension-derived data | PluginHost extension storage/facet owner | Read its namespaced facet | Namespace is host-derived; another extension's facet is inaccessible. |
| `facets.write(record)` | Write derived facet data | PluginHost plus Asset provenance owner | Write bounded value with schema and expected fingerprint | Requires `asset.facet.write`; stale fingerprint fails closed and leaves core Asset identity unchanged. |
| `configuration.version()` | Read configuration revision | Profile/configuration owner | Read a version token used for Job provenance | This fixture does not expose arbitrary config writes. A future write needs a versioned command and capability. |
| `storage.read(key)` / `storage.write(key, value)` / `storage.migrate(from, to)` | Read/write and migrate extension-owned storage | PluginHost storage owner | Use only the plugin namespace and declared quota | `storage.read`/`storage.write` are capability checked; migration is bounded and transactional, and failure leaves the previous generation authoritative. |
| `ui.register(surface)` / `ui.render(request)` | Negotiate and project UI Kit components | UI Bridge / Workbench | Use only declared semantic components on the plugin View | UI Kit/API and component-contract versions are negotiated before activation; the host owns DOM, focus, layout and overlay state. |
| `diagnostics.log(record)` | Emit structured diagnostic | PluginDiagnosticSink / ApplicationCore | Send bounded redacted records | Host bounds and redacts; logs cannot include source paths, secrets or Asset payloads. |
| `notifications.show(record)` | Request a user notification | Host notification surface | Request a typed severity/message | Host owns presentation, localization and rate limits; a plugin cannot write global UI state. |

These handles are intentionally path-free. A production extension will receive
capability-scoped handles over the accepted runtime/bridge protocol, not raw
Rust objects or framework components.

## Asset, Preview and Job data

The Asset Inspector pattern is the safe default for analysis extensions:

1. read a path-free Asset projection;
2. capture the current selection revision, content fingerprint and config
   version in Job provenance;
3. submit work to the Rust-owned JobSystem;
4. reject or discard results whose fingerprint is stale;
5. write only a namespaced, schema-versioned facet;
6. expose the facet through a Preview augmentation or contextual View.

The extension must not open the source path itself, persist a second Asset
record, or start a renderer-owned background loop. The generic Preview fallback
and core Asset identity remain usable when the extension is disabled or fails.

## UI Bridge and UI Kit

Use [ui-kit-contract-v1.md](ui-kit-contract-v1.md) for the current bridge
schema, semantic token allow-list, origin checks and component negotiation.
The host supplies layout containers, focus restoration, overlays, global menus
and shortcuts. A plugin surface may render token-compatible content and use
registered commands, focus and resize requests. The Asset Inspector example
uses the public `progress`, `table`, `loading-state`, `empty-state` and
`error-state` component names through the UI Kit adapter; it does not import
React or inject DOM.

The bridge has independent `bridgeApi`, component-contract and theme-schema
versions. Theme changes are delivered as host-owned snapshots; the current
fixture covers token filtering and a `host.theme` update. Keyboard focus and
ARIA ownership remain in the host Workbench, with the DOM regression fixture
covering labels and focus restoration.

Do not rely on React types, host CSS classes, DOM selectors, arbitrary URLs or
another View's implementation. The current data/blob/builtin surface examples
are fixture conveniences; a production surface still needs the host's origin,
CSP and capability policy.

## Permissions, errors and recovery

Manifest permissions are requests. The effective grant is computed by the
host and is narrowed by zone, target app, Profile trust and platform policy.
The host reports typed errors for incompatible versions, denied capabilities,
invalid contributions, cancellation, timeout, stale input, malformed runtime
output and unavailable storage.

On failure, the extension should:

- stop scheduling new work after cancellation or disable;
- surface a bounded diagnostic and a user-facing retry/disable action;
- leave core Asset, layout, Preview fallback and unrelated extensions usable;
- retain or migrate its own namespaced data without implicitly deleting user
  sources;
- treat `unknown` Job outcomes as reconciliation-required instead of silently
  retrying a non-idempotent operation.

The host-owned diagnostic workflow is concrete and local:

1. Open the Operations panel and query the plugin install/recovery log. Filter
   by extension, status or action; the same bounded records are available in
   the global diagnostic projection.
2. Use the panel's **Export** action, which calls the typed
   `export-plugin-install-recovery-log` command through CoreTransport and a
   native save dialog. A broader user-initiated `ExportRecoveryDiagnostics`
   bundle may include bounded plugin diagnostic history, but never raw Asset
   content, credentials or unrestricted paths.
3. For a failed runtime, choose host-owned **Recover**, **Disable** or retry.
   For a staged install/upgrade, choose resume, keep new, restore previous or
   discard; the host uses the corresponding typed recovery command and keeps
   the old committed package until a successful commit.
4. If a failure remains, retain the bounded diagnostic record and attach the
   fixture/test result when reporting it. Do not delete package directories or
   extension data manually to force recovery.

## Local verification checklist

Before sharing a fixture, run the commands in the
[plugin contract matrix](plugin-contract-matrix.md). During iteration, the
fixture-specific commands at the start of this guide are sufficient for a
quick loop. Map each new contribution to the matrix and add a positive and
negative fixture at the owning seam. A fixture is not a production
qualification for signing, Marketplace publication, notarization, or every
target OS.

## Current limits

- The public manifest and protocol are versioned, but no separate third-party
  SDK package is published.
- Facet schemas are owner-versioned and fingerprint-bound; the current host has
  no independent global facet negotiation registry.
- `RuntimeKind::Wasm` and `RuntimeKind::Sidecar` are contract declarations;
  the sidecar fixture is an isolation test, not a security sandbox.
- Publisher trust, revocation, Marketplace policy and release signing remain
  release gates.
- A second client or independently versioned SDK would justify an ADR and a
  package extraction. Until then, keep the current in-repo protocol mirrors
  explicit.
