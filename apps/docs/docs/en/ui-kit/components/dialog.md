---
id: dialog
title: Dialog
sidebar_label: Dialog
---

import { DialogPlayground } from '@site/src/components/playgrounds/dialog';

# VelvetDialog

`VelvetDialog` is a host-owned modal overlay. It owns the portal, focus trap, Escape dismissal, focus restoration, and action area.

## Demo

<DialogPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `open` | `boolean` | — | Whether the dialog is open |
| `onOpenChange` | `(open: boolean) => void` | — | Open-state callback |
| `title` | `string` | — | Dialog title |
| `description` | `string` | — | Optional description |
| `children` | `ReactNode` | — | Body content |
| `actions` | `readonly VelvetDialogAction[]` | `[]` | Footer actions |
| `severity` | `normal \| warning \| danger` | `normal` | Semantic risk level |
| `closeLabel` | `string` | `Close` | Close button label |
| `restoreFocusRef` | `RefObject<HTMLElement \| null>` | — | Element to focus after close |

### VelvetDialogAction

`id`, `label`, `variant`, `primary`, `disabled`, `busy`, and `onSelect` form the action contract. `id`, `label` and `onSelect` are required; the other fields are optional. An action callback does not automatically close the controlled dialog: the caller updates `open`.

## States and accessibility

Coverage includes closed, open, focus trap, Escape cancel, cancel button, submit, and focus restoration. Dialog title and description use semantic nodes. Actions use native Button behavior.

Behavior is listed in the [Dialog manual](/reference/ui-kit/components/overlays/dialog).

## Customization and migration

Use severity, semantic tokens, and action variants. Changing portal, focus restoration, Escape behavior, or action ownership requires `npm run qualify:ui-browser`.
