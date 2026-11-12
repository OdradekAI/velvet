---
id: choose-contribution
title: Choose a contribution and UI surface
sidebar_label: Choose a contribution
---

# Choose a contribution and UI surface

Choose the host capability first, then decide whether custom UI is needed.

| Goal | Contribution | UI entry point |
| --- | --- | --- |
| Add navigation or a tool | `view-container`, `view`, `activity-bar-entry` | UI Kit or UI Bridge |
| Add a one-shot action | `command`, `menu`, `keybinding` | Host Command Palette, Context Menu, or Overlay |
| Enhance Asset preview | `preview-provider`, `preview-augmentation`, `inspector-section` | PreviewHost, Inspector, or UI Bridge |
| Run expensive work | `job` | OperationsPanel and Progress projection |

Trusted first-party extensions may use Velvet React wrappers. Third-party extensions use the framework-neutral UI Bridge. Plugins that only need host interaction should use Commands, Notifications, Progress, or Host projections without creating a custom surface.
