---
id: separator
title: Separator
sidebar_label: Separator
---

import { SeparatorPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetSeparator

`VelvetSeparator` is a visual and semantic divider. It renders `role="separator"` with horizontal or vertical orientation.

## Demo

<SeparatorPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `orientation` | `horizontal \| vertical` | `horizontal` | Divider orientation |

## States and accessibility

The component renders `role="separator"` and `aria-orientation`. It introduces no keyboard action and should not contain interactive content.

Behavior is listed in the [Separator manual](/reference/ui-kit/components/primitives/separator).

## Customization and migration

Use border, density, and semantic spacing tokens. Changing orientation meaning or role requires a component contract update.
