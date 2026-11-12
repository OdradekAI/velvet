---
id: index
title: Maintainer map
---

# Maintainer map

Start with the owner of the state or contract you are changing. This page is a map of the current repository, not a new runtime layer.

| Concern | Owner / implementation | Decision and evidence |
| --- | --- | --- |
| Domain vocabulary and Asset identity | `asset-core`; `docs/GLOSSARY.md` | [Glossary](https://github.com/OdradekAI/velvet/blob/main/docs/GLOSSARY.md), ADR-0002 |
| Application composition and transport | `application-core`, `workbench-protocol`, `tauri-adapter`; desktop `platform` | ADR-0001 / ADR-0006; [contract matrix](/reference/plugin-contract-matrix) |
| Extension declarations and lifecycle | `plugin-api`, `plugin-host` | ADR-0005; [Manifest](/reference/plugin-manifest-v1), [registry](/reference/plugin-registry-v1) |
| Layout, previews and background work | `layout-model`, `preview-core`, `job-system` | ADR-0003 / ADR-0004 / ADR-0007; owning crate tests |
| Durable storage and recovery | `persistence` and the domain owner | ADR-0013 / ADR-0014; native release qualification |
| Desktop UI and plugin surfaces | `app`, `platform`, `workbench`, `workspaces` | ADR-0008 / ADR-0009 / ADR-0011 / ADR-0012; [UI coverage](/reference/ui-kit/action-coverage) |
| Package trust and release | PluginHost and release engineering | ADR-0015; [ecosystem gate](/reference/plugin-ecosystem-qualification) |
| Documentation app | `apps/docs` | [Documentation structure](./documentation-structure); PR docs checks |

Find each decision in the [ADR directory](https://github.com/OdradekAI/velvet/tree/main/docs/adr). Read its actual status: **Accepted/Proposed is a design status, not proof that every acceptance scenario has passed**. The [foundation plan](https://github.com/OdradekAI/velvet/blob/main/docs/engineering/workbench-foundation-plan.md) records implementation sequence and acceptance; [native qualification](https://github.com/OdradekAI/velvet/blob/main/docs/engineering/native-release-qualification-matrix.md) records platform evidence.

## Reading layers

- Guides teach a task and link to one canonical contract.
- Reference specifies declarations, messages, ownership, errors and compatibility.
- Engineering records explain implementation seams and repeatable qualification.
- ADRs preserve decisions, alternatives and consequences.
- Research records preserve evidence and hypotheses, without becoming API promises.

`CoreTransport` remains the desktop/Rust seam. Demos consume the existing trusted UI entrypoint; docs cannot become a second persistence, permission or runtime owner. Use the [API index](/reference/plugin-api-index) to distinguish extension protocols from internal registry methods and fixture convenience handles.
