---
id: button
title: Button
sidebar_label: Button
---

import { ButtonPlayground } from '@site/src/components/UiKitComponentPlayground';

# VelvetButton

Use Button for a command or local action. The Plugin Platform still owns command visibility, capability checks, and enablement.

## Demo

<ButtonPlayground />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | Button content |
| `label` | `string` | — | Fallback content |
| `variant` | `default \| quiet \| danger` | `default` | Semantic visual variant |
| `disabled` | `boolean` | `false` | Prevents activation |
| `busy` | `boolean` | `false` | Shows busy state and prevents activation |
| `onClick` | `MouseEventHandler` | — | Local or host action callback |
| `ariaLabel` | `string` | — | Accessible name |
| `apiVersion` | `string` | `1.0` | UI Kit contract version |

## States and accessibility

Button covers default, busy, disabled, and activate states. Keyboard activation uses native Enter and Space behavior. Story and browser evidence are listed in the [Button manual](/reference/ui-kit/components/primitives/button).

## Customization and migration

Use semantic variants and theme tokens. Private DOM, Tailwind classes, and internal CSS selectors are not public API. Changes to keyboard behavior, state ownership, or variant meaning require a component contract decision.
