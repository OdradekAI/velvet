---
id: testing
title: Testing and qualification
sidebar_label: Testing
---

# Testing and qualification

Platform contracts require schema, behavior, browser, and recovery evidence. UI Kit stories are executable evidence rather than static screenshots.

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-browser
npm run verify:ui-kit
```

Every cross-boundary capability should cover valid projection, rejection, cancellation or timeout, owner failure, cleanup, and migration or recovery.
