---
id: icon-button
title: IconButton
sidebar_label: IconButton
---

import { IconButtonPlayground } from '@site/src/components/playgrounds/icon-button';

# VelvetIconButton

`VelvetIconButton` is a compact icon-first action. It inherits the Button state and action contract and requires an accessible label.

## Demo

<IconButtonPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `icon` | `ReactNode` | — | Visual icon content |
| `label` | `string` | — | Required accessible name |
| `disabled` | `boolean` | `false` | Prevents activation |
| `busy` | `boolean` | `false` | Shows busy state and prevents activation |
| `onClick` | `MouseEventHandler` | — | Action callback |
| `variant` | `default \| quiet \| danger` | `default` | Button visual variant |
| `ariaLive` | `off \| polite \| assertive` | — | Declared in the common type; current Button does not map it to native `aria-live` |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |

## States and accessibility

The component uses a native button, so Enter and Space activate it. `label` maps to `aria-label`. Busy and disabled states prevent the action.

Behavior is listed in the [IconButton manual](/reference/ui-kit/components/primitives/icon-button).

## Customization and migration

Use icon, semantic variant, and theme tokens. Changing label, keyboard behavior, or busy behavior requires a Button contract and fixture update.
