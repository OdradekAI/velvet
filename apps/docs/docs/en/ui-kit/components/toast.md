---
id: toast
title: Toast
sidebar_label: Toast
---

import { ToastPlayground } from '@site/src/components/playgrounds/toast';

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

`id` and `message` are required; `title`, `severity` and `action` are optional. `severity` defaults to `info`. `useVelvetToast` exposes `push(item: Omit<VelvetToastItem, 'id'>): string` and `dismiss(id: string): void` inside `VelvetToastProvider`; using the hook outside that provider throws. `push` assigns an ID. The host does not auto-dismiss notifications; callers control removal. An isolated extension requests notifications through the host protocol, not this React hook.

## States and accessibility

Info, success, and warning use polite live regions. Error uses an assertive alert. Every notification provides an accessible dismiss IconButton.

Behavior is listed in the [Toast manual](/reference/ui-kit/components/overlays/toast).

## Customization and migration

Use severity, live-region semantics, and theme tokens. Changing notification lifecycle, dismiss behavior, or error announcement requires a qualification fixture update.
