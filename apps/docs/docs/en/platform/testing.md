---
id: testing
title: Testing and qualification
---

# Testing and qualification

The first fixture runs with Node alone. For the desktop/transport contract tests, prepare the repository once:

```sh
npm --prefix apps/desktop ci
npm --prefix apps/desktop run test:compile
node apps/desktop/test/asset-inspector-demo.test.mjs
```

Choose the focused test linked by the capability guide. A successful exit proves the named fixture seam. Cover success, permission/version rejection, cancellation or timeout, stale results, owner failure and cleanup. Keep unrelated extensions and core state usable.

Full repository gates, browser qualification and native release matrices belong to the [maintainer map](/maintenance) and [contract matrix](/reference/plugin-contract-matrix). Passing a local fixture does not qualify production package trust, a security sandbox or every OS/WebView.
