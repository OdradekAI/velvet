---
id: security
title: Security boundaries
---

# Security boundaries

<span id="security" className="velvet-doc-anchor" />

A product extension and a Tauri host plugin cross different boundaries. A contribution's UI zone never grants filesystem, network, process or metadata-write access. Manifest capabilities are requests; effective grants are host policy.

| Boundary | Enforcement owner | Extension responsibility |
| --- | --- | --- |
| Package and identity | PluginHost validates schema, compatibility, package containment and trust policy | Keep IDs stable; do not treat developer mode as extra permission |
| Runtime to host | Runtime adapter binds caller identity and nonce; TauriTransport checks session; ApplicationCore dispatches authorized operations | Never invent another caller, reuse a revoked session or retry unknown mutations blindly |
| UI surface to host | Surface adapter checks source/origin; Bridge validates message shape and size; handlers check commands/capabilities | Send declared messages, dispose listeners and accept bounded projections |
| Jobs and extension data | JobSystem and PluginHost enforce ownership, quotas, provenance and migration | Cancel owned work, reject stale results and keep writes namespaced |
| Diagnostics | Host bounds and redacts records and controls export | Avoid secrets, raw paths and Asset payloads |

Process crash isolation is distinct from a security sandbox. The sidecar fixture and a `wasm` manifest value are not evidence of production containment. Tauri capabilities control its WebView-to-core exposure, not arbitrary Rust code or an external process's operating-system access.

The host owns overlays, focus restoration, layout and fallback. Error, disable, upgrade and uninstall must leave core Asset state and unrelated extensions usable. Follow the [recovery walkthrough](/reference/plugin-developer-guide#permissions-errors-and-recovery) and [qualification evidence](/reference/plugin-ecosystem-qualification); the latter records remaining release gates rather than promising completed protection.
