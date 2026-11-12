---
id: badge
title: Badge
sidebar_label: Badge
---

import { BadgePlayground } from '@site/src/components/playgrounds/badge';

# VelvetBadge

`VelvetBadge` displays a compact status, count, or qualification marker. It preserves child semantics and introduces no keyboard action.

## Demo

<BadgePlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | Badge content |
| `tone` | `neutral \| info \| success \| warning \| danger` | `neutral` | Semantic status tone |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |
| `className` | `string` | — | Host-approved semantic style extension |

## States and accessibility

The component covers default and tone variants. It does not render a button or link, so it should not carry an action that requires keyboard activation. Use it as content within a Button or link when interaction is needed.

Behavior is listed in the [Badge manual](/reference/ui-kit/components/primitives/badge).

## Customization and migration

Use semantic tones and theme tokens. Changing tone meaning or child semantics requires a component contract and story update.
