---
id: concepts
title: Scopes, ownership and lifecycle
---

# Scopes, ownership and lifecycle

A product extension contributes capabilities to the Workbench. A Tauri host plugin supplies desktop/system integration and is a different trust boundary.

| Concept | Meaning |
| --- | --- |
| `workbench` scope | Global shell or cross-app contribution |
| `workbench-app` scope | Contribution to a specified WorkbenchApp |
| `capability` scope | Reusable Preview, processing or Job capability |
| Namespace | Host-derived visibility/identity boundary; an extension cannot claim another owner's namespace |
| Zone | `navigation`, `contextual` or `operations`; placement never grants data or system permissions |
| ViewContainer / View | Host-owned placement and a contribution's bounded content |
| WorkbenchApp / AppSurface | A top-level product app and its main surface; neither owns global layout |
| Projection | Read-only representation of host authority; it can be resynchronized |

The host validates a manifest before activation. Active contributions belong to one extension. Disable, crash, rollback and uninstall revoke their registrations and handles. A UI retry creates a fresh session; it does not replay old side effects.

Third-party UI consumes semantic projections and messages. Trusted host UI may use React wrappers. A fixture can model either boundary without proving production isolation. [Security](./security) identifies the enforcement responsibilities; [the registry record](/reference/plugin-registry-v1) describes the host implementation.

Canonical vocabulary lives in the repository [glossary](https://github.com/OdradekAI/velvet/blob/main/docs/GLOSSARY.md). Architecture decisions and implementation evidence are linked from the [maintainer map](/maintenance).
