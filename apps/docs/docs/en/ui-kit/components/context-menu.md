---
id: context-menu
title: ContextMenu
sidebar_label: ContextMenu
---

import { ContextMenuPlayground } from '@site/src/components/playgrounds/context-menu';

# VelvetContextMenu

`VelvetContextMenu` is a trigger-driven contextual command menu. The host owns action callbacks while Radix owns the portal, roving focus, and submenu transitions.

## Demo

<ContextMenuPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | Trigger area |
| `actions` | `readonly VelvetContextMenuAction[]` | — | Menu item definitions |

### VelvetContextMenuAction

| Property | Type | Description |
| --- | --- | --- |
| `id` | `string` | Stable action id |
| `label` | `string` | Visible name |
| `disabled` | `boolean` | Prevents selection |
| `checked` | `boolean` | `menuitemcheckbox` state |
| `shortcutHint` | `string` | Keyboard hint |
| `separatorBefore` | `boolean` | Adds a separator |
| `group` | `string` | Optional metadata; the current renderer does not group items by it |
| `submenu` | `readonly VelvetContextMenuAction[]` | Recursive submenu |
| `onSelect` | `() => void` | Optional selection callback |

Only `id` and `label` are required on an action. `checked` is caller-owned: selection invokes `onSelect` but does not toggle it automatically. `shortcutHint` displays text and does not register a shortcut. A nonempty submenu renders a submenu trigger rather than invoking the parent action.

## States and accessibility

Coverage includes default, checked, disabled, submenu, pointer dismissal, Escape cancel, and keyboard selection. Menu items use Radix menu semantics. Host callbacks should remain inside the registry boundary.

Behavior is listed in the [ContextMenu manual](/reference/ui-kit/components/overlays/context-menu).

## Customization and migration

Use action schema, shortcut hints, and semantic tokens. Changing submenu, focus navigation, or checked semantics requires `npm run qualify:ui-browser`.
