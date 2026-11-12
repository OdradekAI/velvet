---
id: inline
title: Inline
sidebar_label: Inline
---

import { InlinePlayground } from '@site/src/components/playgrounds/inline';

# VelvetInline

`VelvetInline` is a Stack variant with a fixed row direction for badges, text, and compact actions.

## Demo

<InlinePlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | Child elements |
| `gap` | `none \| xs \| sm \| md \| lg` | `md` | Semantic spacing token |
| `className` | `string` | — | Host-approved semantic style extension |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |

## States and accessibility

Inline renders a horizontal layout container and does not change child reading order, focus order, or keyboard behavior. Use Stack for a selectable row/column direction. Neither wrapper declares a wrapping prop; more complex layout belongs to the host composition.

Behavior is listed in the [Inline manual](/reference/ui-kit/components/primitives/inline).

## Customization and migration

Use spacing tokens. Inline always uses row direction, so changing that contract requires migrating to Stack.
