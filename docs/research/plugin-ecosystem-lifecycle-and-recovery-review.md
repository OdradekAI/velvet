# Plugin ecosystem lifecycle and recovery review

Date: 2026-10-31

This review evaluates [ADR-0005](../adr/0005-plugin-host-runtime-and-permissions.md)
and the plugin-related parts of the [native lifecycle/recovery review](native-lifecycle-recovery-review.md)
from the point of view of a developer who must ship, update, debug and recover a
Velvet extension. The goal is to keep the SDK convenient without turning a
third-party package into a second application core.

The conclusion is that the existing direction is sound, but three boundaries
must be explicit before the ecosystem grows:

1. runtime isolation is a failure boundary and may or may not be a security
   boundary;
2. extension backup/migration is an extension-owned data operation orchestrated
   by the host, not a plugin-controlled restore of core state; and
3. package signature verification proves artifact integrity and publisher
   identity, while paid access is a separate entitlement service.

## Evidence and counterexamples

### VS Code

VS Code runs extensions in local, web or remote Extension Hosts and chooses the
host from the extension's declared capabilities and location. It activates
extensions lazily and deliberately prevents them from modifying the workbench
DOM. This is the right lesson for Velvet: the public API should be expressive,
but the shell, layout, focus, accessibility and recovery surface must remain
host-owned. ([Extension Host](https://code.visualstudio.com/api/advanced-topics/extension-host),
[Extension API overview](https://code.visualstudio.com/api/extension-capabilities/overview))

VS Code gives extensions separate workspace/global storage, encrypted secret
storage, logs and a public extension context. This is a useful developer
experience, but it is not a backup contract: extension storage is still the
extension's state and a secret must not be copied into a portable backup.
([Data storage](https://code.visualstudio.com/api/extension-capabilities/common-capabilities),
[VS Code API: ExtensionContext](https://code.visualstudio.com/api/references/vscode-api))

The local VS Code source also makes the failure boundary concrete. The
extension host has a graceful shutdown window and a force-kill path when the
process does not exit; extension-host crash tracking can restart an unhealthy
host and eventually ask the user to reload. This supports per-extension
runtime health and bounded restart, but it does not justify copying VS Code's
thresholds or assuming that a renderer can recover Velvet's durable Asset
state. ([localProcessExtensionHost.ts](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/services/extensions/electron-browser/localProcessExtensionHost.ts),
[extensionHostStarter.ts](https://github.com/microsoft/vscode/blob/main/src/vs/platform/extensions/electron-main/extensionHostStarter.ts))

The VS Code Marketplace signs published extensions and the client verifies the
signature, while publisher identity, deprecation and marketplace policy remain
separate concerns. Pricing labels and sponsorship are not a license-validation
API. Velvet should therefore not overload a package signature with a paid
entitlement. ([Extension Marketplace](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace),
[Publishing extensions](https://code.visualstudio.com/api/working-with-extensions/publishing-extension))

VS Code's runtime-security guidance is also a counterexample for Velvet: a
desktop Node extension host has the same broad user permissions as the editor.
Marketplace scanning, verified publisher status and an allow/block list improve
trust signals, but they do not turn an extension into a least-privilege
sandbox. ([Extension runtime security](https://code.visualstudio.com/docs/configure/extensions/extension-runtime-security),
[Enterprise extension policy](https://code.visualstudio.com/docs/enterprise/extensions))

The local VS Code checkout makes the operational details concrete: its
extension downloader verifies a separate signature archive and reports typed
integrity errors such as invalid signatures, tampered entries and revoked
certificates; the native extension service tracks repeated host crashes and
offers restart/disable choices. Velvet should adopt the error taxonomy and
user action shape while keeping its own package envelope and Rust ownership.
Relevant local files are
`/Users/luiz/Odradek/vscode/src/vs/platform/extensionManagement/node/extensionDownloader.ts`,
`extensionManagementService.ts`,
`extensionSignatureVerificationService.ts` and
`/Users/luiz/Odradek/vscode/src/vs/workbench/services/extensions/electron-browser/nativeExtensionService.ts`.

### Eagle

Eagle exposes distinct Window, Background Service, Format Extension and other
plugin forms. Its marketplace contains format readers, preview/thumbnail
providers, inspector tools, batch transformations, background services and
network integrations, with both free and paid entries. This validates Velvet's
composition model: a format provider, an inspector and an operation should be
separate contributions even when one package implements several of them. It
also shows why a background plugin must be governed as a Job owner rather than
being treated as a harmless UI add-on. ([Eagle Plugin API introduction](https://developer.eagle.cool/plugin-api/get-started),
[Eagle Plugin Center](https://community-cn.eagle.cool/plugins),
[Eagle format extensions](https://community-cn.eagle.cool/plugins/format))

The direct Velvet mapping is Window → isolated UI/AppSurface, Background
Service → host-owned Job/activation policy, Format Extension → PreviewProvider
plus thumbnail/analysis Jobs, and Inspector → InspectorContribution. The
mapping is semantic rather than an Eagle ABI: Velvet does not let a format
extension bypass the resolver or let a background service own an unbounded
daemon.

Eagle also distinguishes its in-app JavaScript Plugin API from an external Web
API. Velvet should make the same distinction: an in-process trusted adapter and
an external/remote connector are different trust and lifecycle paths even when
they expose similar product capabilities. ([Eagle Web API](https://developer.eagle.cool/web-api))

Eagle's review criteria explicitly cover unconsented commands/subprocesses,
file mutation, network disclosure, cancellation, data loss, hidden background
processes and external accounts/API tokens. Its public material describes
automatic scanning and human review rather than a cryptographic publisher
signature or capability sandbox. Velvet should keep review and reputation as
marketplace signals, but make signature, capability grant and process
isolation enforceable client-side. ([Security and privacy review](https://developer.eagle.cool/plugin-api/plugin-review/criteria/security-and-privacy),
[Functionality and policy review](https://developer.eagle.cool/plugin-api/plugin-review/criteria/functionality-and-policy),
[Package and review process](https://developer.eagle.cool/plugin-api/publishing/package))

### DeepSeek Harness

DeepSeek Harness explicitly uses an “everything is a plugin” architecture and
labels itself a developer preview with compatibility-breaking changes. Its
Tauri desktop distribution shows the practical ecosystem surface: built-in and
community plugins, profiles, backup/restore, plugin management, runtime
updates, schedulers, remote connections and optional network services. The
lesson is not to make every Velvet feature a plugin. The lesson is to keep the
host protocol, profile/data ownership and failure UX stable while allowing
rapidly changing contributions behind versioned contracts. ([DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness),
[DeepSeek Harness desktop distribution](https://github.com/dsh-tauri/deepseek-harness-desktop))

The upstream architecture uses reversible registrations/effects, ordered
profile layers and append-only session evidence, while still being explicit
that the project is a developer preview with compatibility-breaking changes.
Those ideas support reversible contribution cleanup and monotonic generations;
Velvet should retain a non-plugin ApplicationCore for Asset, Layout, Job,
permissions and recovery so that “everything is a plugin” does not erase the
durable ownership anchor.

### Tauri and industry standards

Tauri capabilities and runtime authority constrain which windows or WebViews
may invoke commands, but Tauri plugins and Rust application code are trusted
code with access to the process's available system resources. Tauri's ACL is
therefore a necessary command boundary, not a sandbox for arbitrary native
plugin code. Scopes, command-specific permissions and repeated validation in
Rust remain required. ([Tauri security](https://v2.tauri.app/security/),
[Capabilities](https://tauri.app/security/capabilities/),
[Runtime authority](https://v2.tauri.app/security/runtime-authority/),
[Plugin development](https://v2.tauri.app/develop/plugins/))

WASI and the WebAssembly Component Model provide the strongest portable
capability story for untrusted extension code: a component starts without
ambient authority and receives only explicitly granted imports. The model also
gives Velvet a language-neutral interface definition path if the WASM runtime
becomes a first-class SDK target. A sidecar process does not provide this
guarantee because it inherits the operating system user's permissions unless a
platform sandbox is added. ([WASI security](https://wasi.dev/security),
[Component Model concepts](https://component-model.bytecodealliance.org/design/component-model-concepts.html),
[Wasmtime security](https://docs.wasmtime.dev/security.html))

Package signing should be compatible with established supply-chain concepts:
the package digest and publisher identity are signed, build provenance can be
attested separately, and an SBOM can describe included components. SLSA
provenance, in-toto/Sigstore verification and SPDX are useful vocabulary and
interchange points; they are not a reason to make an online marketplace a
startup dependency. ([SLSA provenance](https://slsa.dev/spec/v1.2/provenance),
[Sigstore verification](https://docs.sigstore.dev/cosign/verifying/verify/),
[SPDX](https://spdx.dev/))

## Plugin forms and their failure/security meaning

The runtime form, extension kind and contribution kind are independent. A
package can be a `PreviewProvider` and a `Job`, but that does not decide whether
it is trusted or where it runs.

| Form | Good fit | Failure boundary | Security meaning | Default policy |
| --- | --- | --- | --- | --- |
| Declarative package | Views, menus, labels, file selectors and static metadata | Manifest/projection validation | No executable authority; malformed data is rejected | Allow when schema and signature policy pass |
| Built-in trusted adapter | First-party providers that need low latency or native APIs | The application process; a panic/abort can take down Core | Trusted application code, not third-party sandboxing | First-party, signed with the application, same public interfaces |
| WASM/component | Decoders, metadata readers, pure transforms and bounded analysis | Instance or WASM runtime; trap affects that extension | Capability sandbox when the host grants no ambient WASI imports | Preferred for untrusted computation |
| Isolated sidecar | Node/Python/Rust tools, large native libraries and legacy integrations | One extension process group | Crash isolation only; a sidecar is not a security sandbox | Explicit user trust and narrow mediated capabilities |
| Isolated UI surface | AppSurface, View, Dialog, Inspector and preview UI | UI session/WebView surface | UI Bridge is the only host path; CSP/origin rules still apply | Host frame, bridge handshake and teardown owned by Velvet |
| Tauri host plugin | First-party OS integration such as notifications or native shortcuts | Application process | Privileged host code; never a marketplace product-extension boundary | Compile-time/bundled only |
| Remote connector | Feed, sync, model or service integration | Network request/connector Job | Network capability plus endpoint and data policy; remote service is not trusted code | Treat as a capability profile over WASM/sidecar/host Job |

The alpha implementation may keep `builtin`, `wasm` and `sidecar` as the
runtime enum, but the manifest and install policy must also expose the trust
level and whether the process boundary is only for availability or is backed by
an OS/WASI sandbox. Calling every sidecar “isolated” without this distinction
would create a false security promise.

Manifest-only contributions should be a first-class no-runtime package shape in
the target SDK. They do not need a dummy in-process runtime merely to register a
View, menu, label or file selector. This can be introduced as a `declarative`
runtime/profile in the next manifest major while the alpha compatibility layer
maps it to a no-op host adapter.

The default process topology is one runtime process or WASM instance per
extension. Jobs belonging to one extension may share that extension's runtime,
but two extensions do not share a process by default. A shared runtime can be
introduced only as an explicit performance optimization with a larger failure
domain and a test proving that a trap, timeout or protocol violation cannot
cross instance boundaries.

## Host interfaces versus extension-owned interfaces

The host SDK should expose small interfaces that are useful in all runtime
forms. The same semantics are available to a trusted adapter, WASM component,
sidecar and isolated UI surface, but the transport adapter differs.

| Host-owned interface | Extension receives | Extension owns |
| --- | --- | --- |
| Manifest/handshake | Host API, bridge and runtime protocol ranges; effective capability grant; session id | Contribution declarations, activation interests, support metadata and public API surface |
| Context handles | Opaque Asset/Selection/WorkspaceContext/URI projections; locale/theme/focus snapshots | UI and algorithm decisions based on those projections |
| Contribution registries | Typed View, AppSurface, Preview, Inspector, Command, Menu and Job registration | Contribution implementation, namespace-local state and disposal |
| `JobSystem` | Job id, attempt, bounded input, provenance, deadline, cancellation and checkpoint handle | Work algorithm, checkpoint payload, progress messages, retry-safe behavior and result schema |
| `ExtensionStorage` | Namespaced transactional key/blob handles with size and quota limits | Extension schema, keys, serialization and data retention within its namespace |
| `SecretStore` | Opaque get/set/delete handle; no raw database or backup path | Which secrets it needs and how it reacts to missing/expired secrets |
| `BackupParticipant` / `MigrationParticipant` | Staged read/write handles for the extension's own store and schema metadata | Export/import shape, forward migration code and schema invariants |
| `DiagnosticSink` | Structured logger, correlation ids, bounded artifact attachment and redaction classes | Useful messages, user-facing recovery hints and safe diagnostic context |
| `EntitlementHandle` | Read-only feature status for this extension/product | Feature gating and graceful behavior when entitlement is absent |
| External effects | Narrow URI, network, process or file capability with host policy and user consent | Request intent and handling of denial; never the global command line or unrestricted path |

The host keeps ownership of lifecycle, package trust, permissions, resource
budgets, process groups, protocol compatibility, Job state, core snapshots,
RecoverySet publication, diagnostics retention and user recovery actions.
The extension owns its code, contribution behavior, extension namespace data,
forward migrations, Job checkpoint format and redaction hints. A plugin may
request a host operation, but it cannot redefine the host's interpretation of
an Asset, Workspace, Profile, Job, layout or RecoverySet.

Use these boundary tests when deciding where a new API belongs:

1. If failure could make the Workbench unable to start, the host owns it.
2. If two extensions or a future client must agree on the meaning, the
   versioned host protocol owns it.
3. If the state is user-authored or authoritative domain data, the domain owner
   owns it; the extension may propose a typed mutation.
4. If the state can be deleted and recomputed from provenance, the producer may
   own it as a derived result, but it must use `JobSystem` invalidation rules.
5. If the operation crosses the machine, starts a process or writes outside the
   extension namespace, the host mediates it through a capability.
6. If a runtime can be replaced or quarantined without losing core meaning, it
   belongs to the extension boundary.

This keeps developer ergonomics high: an extension developer writes against
one SDK and one test fake, while the host decides whether the same call travels
through a direct adapter, WIT/WASM imports, JSONL sidecar RPC or UI Bridge.

## Backup, restore and migration

An extension can participate in backup and migration, but it cannot control
global recovery. The contract is:

1. `PluginHost` creates an extension-owned storage namespace and records its
   schema id/version in the extension state owner.
2. A `BackupParticipant` may export a bounded, typed logical snapshot of that
   namespace for a user-selected portable backup. The host excludes secrets,
   entitlement tokens, transient logs, caches and runtime binaries by default.
3. The host also captures opaque owner bytes in its internal rolling
   `StateSnapshot`/`RecoverySet` flow. The extension does not choose the commit
   barrier or publish pointer.
4. A `MigrationParticipant` receives a read-only source generation and a
   staged destination writer. It declares `fromSchema`, `toSchema`, required
   host API and resource needs, then produces a new generation without network,
   external-process or cross-owner writes.
5. The host validates size, checksum, references and schema invariants before
   publishing. Migration runs under the extension's runtime budget and has a
   deadline. A failure quarantines only the extension store and disables the
   extension; the previous generation remains restorable.
6. Rollback uses the previous validated generation and package/data pairing.
   Reverse migrations are optional and never required for safety. An older
   package cannot open a newer extension schema unless its manifest explicitly
   declares that read range and the host has validated it.
7. Uninstall retains extension state by default. Explicit deletion is a host
   action with a clear scope; an extension cannot silently delete its own
   recoverable data during deactivation or upgrade.

Jobs use a separate `JobCheckpoint` contract. A checkpoint is a restart hint,
not an implicit backup of user data. A non-checkpointable Job restarts from a
clean attempt; a checkpoint failure before suspend marks the Job interrupted and
retryable instead of claiming partial durability.

## Signing, trust and paid features

Velvet needs three separate decisions:

### Package integrity and publisher trust

Every stable and preview package is installed through a signed package
envelope. The envelope binds at least the package digest, `ExtensionId`,
publisher key id, package version, target platform/runtime, manifest schema,
host API range and declared capability set. A detached signature or a signed
DSSE/COSE-style envelope is acceptable; the concrete encoding is a release
implementation decision. The verifier also checks archive safety, hash,
anti-downgrade policy and the trusted publisher/key set.

The application release signer is not an extension publisher. Built-in
extensions are covered by the application package signature; installed product
extensions need their own publisher trust decision. Stable/preview builds
reject unsigned external packages. A developer mode can install an unsigned
local package, but the mode is explicit, visible, profile-scoped and never
silently inherited by a production profile.

Key rotation, revocation and offline verification use signed trust metadata or
an equivalent pinned-root policy. A valid old signature alone does not override
revocation, release floor or a newer installed extension generation. SLSA/in-
toto provenance and SPDX SBOM data are optional metadata in the first local
installer and become release-gate requirements when a public marketplace is
introduced.

### Paid access and entitlement

A package signature is not a payment proof. Velvet therefore does not expose a
generic “paid signature verification” API. When a marketplace or account
service exists, the host may expose an `EntitlementHandle` that verifies a
server-issued, signed entitlement bound to `ExtensionId`, product/feature id,
account or installation scope, issuer, sequence, not-before and expiry. The
extension receives only `active`, `trial`, `grace`, `expired` or `unknown` plus
bounded display metadata; it never receives payment credentials or an opaque
token that it can replay to another service.

Entitlements are read-only and must work offline for a bounded grace period.
Clock skew, revoked purchases, refunds and account sign-out are diagnosable
states, not reasons to corrupt extension data. Payment, tax, refunds and
merchant-of-record behavior stay outside Velvet Core. Until a marketplace
contract is selected, the entitlement API remains reserved and disabled; a
plugin may integrate its own licensing service only through an explicit
network capability and must not present that self-assertion as Velvet-verified.

## Jobs, crashes and diagnostics

The host records one structured event stream for each extension runtime and
each Job attempt. Minimum fields are:

```text
extensionId, packageVersion, runtimeKind, runtimeSessionId,
jobId, jobAttempt, requestId, phase, eventKind,
monotonicElapsed, exitCodeOrSignalOrWasmTrap,
deadline, budgetDecision, retryable, redactionClass
```

The extension logger accepts structured fields and bounded messages. The host
adds stderr tail, runtime health transitions, protocol violations, timeout and
process-group termination evidence. It truncates and redacts untrusted fields;
an extension cannot force the diagnostic bundle to include an asset payload,
secret, environment dump or arbitrary path. A plugin may attach a small
diagnostic artifact only when it declares its content type and redaction class.

When an isolated runtime crashes, `PluginHost` emits a typed
`plugin-runtime-crashed` event, removes the extension's projections and marks
its running Jobs as `Interrupted` or typed failure with a retryable decision.
Late progress and results are rejected by extension session, Job id, attempt
and provenance. A timeout, malformed response, resource kill and WASM trap use
distinct event kinds so a developer can tell a bug from cancellation or
budget pressure.

One plugin's sidecar/WASM crash does not affect another plugin because the
default failure domain is per extension. A shared runtime, if introduced
later, must advertise the larger failure domain and prove instance-level
containment before it is enabled. A built-in adapter is different: a panic can
be caught at a Rust boundary, but an abort, memory corruption or process crash
still affects the Core. Third-party code is therefore never accepted as a
built-in adapter.

The main process crash, forced power loss or OS kill terminates all child
plugins that are correctly attached to its process group; no plugin can promise
to continue executing. On the next launch, the host reloads the last committed
extension store and Job snapshots, marks in-flight attempts interrupted and
retries only when the Job contract permits it. Sidecars must be launched with
parent/process-group cleanup or an orphan-reconciliation record; a sidecar
left behind after a core crash is never allowed to keep writing extension or
core state unsupervised.

The current alpha `ProcessPluginRuntime` proves the right seam but does not yet
prove this full policy: it launches the manifest-declared program, passes a
permission summary in the environment and discards stderr. Before production,
the runtime adapter must resolve only package-relative/allow-listed launch
targets, capture bounded stderr, attach children to a process group or
parent-death policy, report exit code/signal, and reconcile orphaned sidecars.
The host contract should remain stable while that adapter hardening lands.

## Recovery scenarios that must be explicit

The following cases are the minimum counterexample set for an ergonomic but
safe SDK:

- A declarative manifest contains a valid contribution but an unknown required
  field. The host rejects only that package and shows the exact schema issue.
- A WASM decoder traps while another plugin's Job is running. The decoder's
  preview falls back and its Job is retryable; the other Job continues.
- A sidecar launches a child process and then hangs. The host terminates the
  process group after the grace period and records the child-budget decision.
- A background plugin writes a large cache until disk is full. Derived work is
  evicted or rejected, while core Asset/Workspace/Profile/Job state remains
  readable and the user gets a typed action.
- An extension migration writes half a new store and loses power. The previous
  generation remains current; the staged bytes are quarantined and the
  extension is disabled on the next launch.
- A plugin is upgraded while one of its Jobs is running. The old runtime is
  drained or interrupted, the package/data transaction is staged, and the new
  runtime receives a fresh session. No old result is committed under the new
  provenance unless the host explicitly proves compatibility.
- A paid extension is offline after a valid purchase. The bounded entitlement
  grace status keeps the feature usable; after grace expires the extension
  degrades its paid feature without losing its data or blocking startup.
- The user installs a syntactically valid but revoked package from an offline
  archive. Trust metadata and release floor reject it before activation and
  leave the current package usable.
- Plugin A emits a malformed log event containing a secret-looking field.
  The host stores a redacted bounded record and never forwards the raw field to
  a diagnostic upload.
- The core is killed while two plugins are running. Both runtimes are marked
  interrupted on the next launch, but their independently committed extension
  stores and unrelated plugin state remain recoverable.

## Verdict on the native lifecycle/recovery review

The review's separation of `RecoverySupervisor`, `CrashCaptureAdapter`,
`RecoverySet`, host-native `RecoverySurface`, and per-owner Job/plugin recovery
is correct and should remain. The plugin perspective adds these required
qualifications:

- `sidecar` means process failure isolation, not sandboxing;
- the default failure domain is one extension runtime, not one process for the
  whole ecosystem;
- extension backup/migration is a host-orchestrated participant API and never a
  plugin-controlled global restore;
- package signature, publisher trust, provenance and paid entitlement are
  separate records and policies; and
- crash/Job diagnostics are host-owned structured events with plugin-provided
  redaction hints, not arbitrary plugin log files.

These changes preserve developer convenience because the SDK still presents
one typed capability model and one fakeable protocol. They also avoid making
the Tauri command surface, marketplace service or a plugin's own server the
de facto owner of Velvet's recovery semantics.

## Recommended contract additions

The following names should be added to the target SDK/ADR vocabulary before a
public extension SDK is released:

- `RuntimeTrust` / `SandboxPolicy`: distinguish trusted built-in code,
  capability-sandboxed WASM, process-isolated sidecars and declarative packages.
- `ExtensionStorage`: transactional, namespaced extension state with an
  explicit schema version and bounded quota.
- `BackupParticipant` and `MigrationParticipant`: staged extension-only
  snapshot/export/migration hooks controlled by `PluginHost`.
- `PluginDiagnosticSink`: structured, bounded, redaction-aware logs and crash
  events correlated to runtime session and Job attempt.
- `EntitlementHandle`: read-only, host-verified paid-feature status, introduced
  only with a concrete marketplace/account service.
- `PluginTrustRecord`: package digest, publisher key id, signature status,
  revocation/freshness state, provenance/SBOM references and developer-mode
  origin.

These interfaces are intentionally host-owned names. They describe the seam a
plugin can use; they do not give a plugin ownership of the underlying store,
trust root, diagnostic retention or recovery policy.
