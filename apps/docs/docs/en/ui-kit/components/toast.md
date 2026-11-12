---
id: toast
title: Toast
sidebar_label: Toast
---

import { ToastPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetToastHost

`VelvetToastHost` projects live notifications. It renders info, success, warning, and error notifications with dismiss actions.

## Demo

<ToastPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `items` | `readonly VelvetToastItem[]` | — | Current notification items |
| `onDismiss` | `(id: string) => void` | — | Dismiss callback |
| `className` | `string` | — | Host-approved semantic style extension |

### VelvetToastItem

`id`, `message`, `title`, `severity`, and optional `action` form the notification contract. `useVelvetToast` exposes `push` and `dismiss` through the Provider.

## States and accessibility

Info, success, and warning use polite live regions. Error uses an assertive alert. Every notification provides an accessible dismiss IconButton.

Behavior is listed in the [Toast manual](/reference/ui-kit/components/overlays/toast).

## Customization and migration

Use severity, live-region semantics, and theme tokens. Changing notification lifecycle, dismiss behavior, or error announcement requires a qualification fixture update.
