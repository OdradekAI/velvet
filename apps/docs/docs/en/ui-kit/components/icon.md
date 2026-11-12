---
id: icon
title: Icon
sidebar_label: Icon
---

import { IconPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetIcon

`VelvetIcon` is an accessible or decorative icon wrapper. It can use a host-owned semantic glyph or compatible custom children.

## Demo

<IconPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `name` | `VelvetIconName` | — | Host glyph name |
| `children` | `ReactNode` | — | Custom renderer content |
| `label` | `string` | — | Accessible name. Adds `role="img"` |
| `className` | `string` | — | Host-approved semantic style extension |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |

`VelvetIconName` includes host glyphs such as `add`, `check`, `search`, `settings`, `spark`, and `warning`.

## States and accessibility

Without `label`, the icon is decorative and uses `aria-hidden`. With `label`, it exposes `role="img"` and `aria-label`. Icon introduces no keyboard action.

Behavior is listed in the [Icon manual](/reference/ui-kit/components/primitives/icon).

## Customization and migration

Prefer host semantic glyphs. Changing glyph names, label semantics, or custom children contract requires a component contract update.
