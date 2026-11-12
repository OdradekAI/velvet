---
id: asset-analysis
title: Analyze Assets with Jobs and facets
---

# Analyze Assets with Jobs and facets

The [first fixture](../platform/quick-start) demonstrates this workflow without accessing user data. Its `HostFacade` is a fake adapter, so the method names here describe the fixture only.

1. Read the bounded selection and `assets.readProjection(id)`.
2. Capture selection revision, content fingerprint and configuration version.
3. Submit through `jobs.create(request)` and observe with `jobs.wait(id)`.
4. Reject stale results before `facets.write(record)`.
5. Write only the extension's schema-versioned facet namespace.
6. Show the result through its registered View or Preview augmentation.

The actual JobSystem remains Rust-owned. A future SDK must carry these operations through authorized handles; it cannot copy the fake host into production or start a renderer-owned background loop.

## Failure behavior

| Condition | Required behavior |
| --- | --- |
| Permission denied | Stop the operation; never open a raw Asset path as a fallback |
| Changed fingerprint | Discard the stale result; preserve Asset identity |
| Cancelled or disabled | Stop new work and clean up owned registrations |
| Unknown Job outcome | Reconcile authoritative state before retrying a side effect |
| Storage migration fails | Keep the previous committed generation |
| Provider crashes | Keep the generic preview and unrelated extensions usable |

The [fixture API catalog](/reference/plugin-developer-guide#fixture-hostfacade-api-catalog) lists handles and owner rules. The executable evidence is `apps/desktop/test/asset-inspector-demo.test.mjs`; [testing](../platform/testing) provides its setup. The [API index](/reference/plugin-api-index) identifies the host Job message seam separately from fake convenience methods.
