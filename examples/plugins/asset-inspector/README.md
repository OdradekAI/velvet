# Asset Inspector fixture

This directory is the fixture-only Asset Inspector demo for issue 009. It is
an executable contract example for a third-party developer, not a production
plugin package and not a new runtime implementation.

The manifest demonstrates stable IDs, Asset Manager scope, navigation/context/
operations views, a context command, keybinding, Preview augmentation and an
extension-owned quality-analysis Job. `src/asset-inspector.mjs` runs against a
small fake HostFacade used by `apps/desktop/test/asset-inspector-demo.test.mjs`.

The fixture receives opaque Asset projections and fingerprints. It never reads
paths, databases, Tauri state, React components or sibling View instances. A
facet write is namespaced as `com.acme.asset-inspector/quality`, carries schema
and input fingerprint metadata, and fails closed when the fingerprint is stale.

The manifest uses `runtime: built-in` only to make the in-memory fixture
composable with the current trusted adapter seam. It must not be interpreted as
permission to ship third-party code in-process. WASM/sidecar runtime selection
remains a later runtime decision.

Run the contract fixture from the repository root with:

```sh
npm --prefix apps/desktop test
```

The test covers manifest projection, activation/cleanup, context analysis,
provenance propagation, cancellation, unknown outcome, stale fingerprints,
facet migration and failure diagnostics. Progress, retry, crash, upgrade,
downgrade and uninstall transitions remain owned by the generic host contracts;
the fixture verifies that its namespaced facet is retained for migration,
recomputation or quarantine and that core Asset state is untouched.
