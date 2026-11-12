---
id: quick-start
title: Run your first plugin fixture
---

# Run your first plugin fixture

<span id="quick-start" className="velvet-doc-anchor" />

This walkthrough runs Asset Inspector against a deterministic in-memory host. It needs a checkout of this repository and **Node.js 22** (the CI baseline). It does not install a plugin into your desktop profile or read your assets. The fixture host is not a downloadable SDK.

## 1. Run from the repository root

```sh
node --version
node examples/plugins/asset-inspector/run-demo.mjs
```

No npm install or Rust build is needed for this first command. The JSON output includes:

```json
{
  "extensionId": "com.acme.asset-inspector",
  "status": "completed",
  "registeredContributions": 7,
  "facets": ["com.acme.asset-inspector/quality:asset-1"]
}
```

The output also lists `uiComponents` and your local `packageRoot`. Success means the fixture registered contributions, analyzed its fake selection, wrote its own facet and disposed its registrations. It does not mean a production runtime or installer has been qualified.

## 2. Follow the package

| File | What to inspect |
| --- | --- |
| `examples/plugins/asset-inspector/manifest.json` | Identity, API ranges, contributions, capabilities and storage schema |
| `examples/plugins/asset-inspector/src/asset-inspector.mjs` | `activate(host)`, `analyzeSelection`, cancellation and disposal |
| `examples/plugins/asset-inspector/src/fixture-host.mjs` | Fake values and recorded calls; this is test infrastructure |
| `examples/plugins/asset-inspector/run-demo.mjs` | The bounded selection and expected result |

[Browse the example](https://github.com/OdradekAI/velvet/tree/main/examples/plugins/asset-inspector). Match manifest fields to the [schema reference](/reference/plugin-manifest-v1). When copying the example, update its exported identity constants and validation along with the manifest; changing only the manifest ID intentionally fails the fixture.

## 3. Exercise failure paths

Install the desktop test dependencies, compile its test inputs, then run the focused contract test:

```sh
npm --prefix apps/desktop ci
npm --prefix apps/desktop run test:compile
node apps/desktop/test/asset-inspector-demo.test.mjs
```

The focused test checks permission denial, stale fingerprints, cancellation, unknown outcomes, runner failure, migration and cleanup. The expected result is a successful exit. Read [testing](./testing) for what this proves.

## Troubleshooting and next steps

- `Cannot find module`: run from the repository root and keep the example files together.
- `manifest-invalid` or `manifest-incompatible`: compare IDs and version ranges with the example constants and the schema; do not bypass validation.
- A failed test is evidence to investigate, not permission to reset a real profile.

Continue with the [capability catalog](./capabilities), [Commands and Views](../guides/commands-and-views), or [Asset analysis](../guides/asset-analysis). For the separate external JSONL process example, see [isolated UI and sidecar](../guides/isolated-ui).
