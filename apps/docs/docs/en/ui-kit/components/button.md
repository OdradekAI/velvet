---
id: button
title: Button
sidebar_label: Button
---

import { ButtonPlayground } from '@site/src/components/playgrounds/button';

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
| `describedBy` | `string` | — | Accessible description element ID |
| `ariaLive` | `off \| polite \| assertive` | — | Declared in the common type; current Button does not map it to native `aria-live` |
| `apiVersion` | `string` | `1.0` | UI Kit contract version |
| `testId` | `string` | — | Host qualification hook |

The typed React wrapper also supports native button props. Third-party UI Bridge messages do not expose React props.

## States and accessibility

Button covers default, busy, disabled, and activate states. Keyboard activation uses native Enter and Space behavior. Story and browser evidence are listed in the [Button manual](/reference/ui-kit/components/primitives/button).

## Customization and migration

Use semantic variants and theme tokens. Private DOM, Tailwind classes, and internal CSS selectors are not public API. Changes to keyboard behavior, state ownership, or variant meaning require a component contract decision.

Run `npm run verify:ui-kit` after interaction or accessibility changes.
