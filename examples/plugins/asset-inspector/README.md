# Asset Inspector demo plugin

This directory is a runnable, deterministic Asset Manager plugin contract for
issue 009. It is an acceptance tool for plugin developers, not a production
Marketplace package. The current repository still has no accepted WASM/sidecar
runtime or published third-party SDK.

The package demonstrates:

- manifest, stable extension/contribution IDs, activation events and API ranges;
- a command, Asset context-menu item, Mod+Alt+A keybinding and navigation,
  contextual and operations Views;
- a bounded host context and path-free Asset projection read;
- structured diagnostics, host notifications and UI Kit v1 components
  (loading-state, progress, table, empty-state and error-state);
- a Rust-owned Job-shaped host call with progress, cooperative cancellation,
  unknown outcome and runner-crash results;
- namespaced private storage/configuration and schema migration;
- fingerprint-bound com.acme.asset-inspector/quality facet writes;
- typed manifest/version/permission/stale-result errors;
- cleanup on disable, re-enable and UI surface disposal.

src/asset-inspector.mjs is the plugin entrypoint. It only receives the
documented fixture HostFacade; it never reads a database, source path,
filesystem handle, Tauri state, React/Vue component or sibling View. The
surface entrypoint is a bridge-ready bootstrap that renders no arbitrary DOM;
the host owns UI Kit components, overlays, layout and focus.

src/fixture-host.mjs is a deterministic in-memory adapter used to exercise
the public seam. It is deliberately not a runtime implementation or a
security boundary. A real package must wait for an accepted isolated runtime
and host SDK.

Run the plugin directly:

~~~
node examples/plugins/asset-inspector/run-demo.mjs
~~~

Run the contract tests:

~~~
npm --prefix apps/desktop test
~~~

The focused test is:

~~~
node apps/desktop/test/asset-inspector-demo.test.mjs
~~~

The test covers manifest and version rejection, activation registration,
context reads, UI Kit negotiation and component projection, successful facet
writes, stale fingerprints, permission denial, cancellable Jobs, unknown
outcomes, runner failure, logging/notification sink failure, storage migration,
disable/re-enable cleanup and preservation of core Asset state.

## Developer walkthrough

1. Copy this package and keep the extension ID stable across upgrades.
2. Edit manifest.json first. Keep every contribution ID unique and
   extension-prefixed. Declare the target app, semantic zone, activation event,
   storage schema and only the capabilities the plugin needs.
3. Implement activate(host). Register declarations through host registries
   and retain every disposer. Never call an internal Workbench object.
4. Read only bounded projections. Include selection revision, config version,
   input fingerprints and schema version in Job provenance.
5. Submit work through the host Job handle. Report progress through the
   host/UI Kit bridge and make cancellation cooperative. Treat unknown as
   reconciliation-required.
6. Write only the plugin namespace after checking the fingerprint. A stale
   write must fail closed and leave the core Asset unchanged.
7. Use only the negotiated UI Kit component list. Request notifications,
   diagnostics and overlays through typed host calls; these calls can fail and
   must not hide the original domain error.
8. Dispose all registrations on disable/crash/uninstall. A later activation
   must be able to rebuild contributions from the manifest.

The host remains the owner of permissions, durable Jobs, layout, preview
selection, persistence and failure recovery. This fixture proves the shape of
that boundary; it does not promote the fake adapter or runtime: built-in to
a production third-party runtime.
