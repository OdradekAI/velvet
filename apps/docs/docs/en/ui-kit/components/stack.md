---
id: stack
title: Stack
sidebar_label: Stack
---

import { StackPlayground } from '@site/src/components/playgrounds/stack';

# VelvetStack

`VelvetStack` arranges content with UI Kit spacing tokens along a vertical or horizontal axis. It owns layout only, not child actions or state.

## Demo

<StackPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | Child elements |
| `direction` | `column \| row` | `column` | Main-axis direction |
| `gap` | `none \| xs \| sm \| md \| lg` | `md` | Semantic spacing token |
| `className` | `string` | — | Host-approved semantic style extension |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |

## States and accessibility

Stack renders a layout container and does not change child reading order, focus order, or keyboard behavior. `direction` changes visual layout and does not replace list or form semantics.

Behavior is listed in the [Stack manual](/reference/ui-kit/components/primitives/stack).

## Customization and migration

Use spacing tokens and direction. Changing gap meaning or the default direction requires a visual fixture and contract update.
