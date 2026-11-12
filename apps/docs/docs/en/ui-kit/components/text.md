---
id: text
title: Text
sidebar_label: Text
---

import { TextPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetText

`VelvetText` provides semantic text and tone wrapping. It lets plugin surfaces use host typography, color, and HTML element semantics.

## Demo

<TextPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `as` | `p \| span \| strong \| small \| h1 \| h2 \| h3` | `span` | HTML element to render |
| `tone` | `primary \| secondary \| muted \| danger \| success \| warning` | `primary` | Semantic text tone |
| `children` | `ReactNode` | — | Text content |
| `id` | `string` | — | DOM id |
| `role` | `string` | — | Optional semantic role |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |

## States and accessibility

Text preserves selection, announcement, and native HTML semantics. It introduces no keyboard action. Choose `as` according to document hierarchy. Tone does not add semantic meaning.

Behavior is listed in the [Text manual](/reference/ui-kit/components/primitives/text).

## Customization and migration

Use `as`, tone, and theme tokens. Changing element semantics or tone meaning requires component contract and browser evidence updates.
