---
id: progress
title: Progress
sidebar_label: Progress
---

import { ProgressPlayground } from '@site/src/components/playgrounds/progress';

# VelvetProgress

`VelvetProgress` projects progress for Jobs and long-running operations. It supports determinate and indeterminate states.

## Demo

<ProgressPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` | `number` | — | Determinate value from 0 to 100. Omit for indeterminate |
| `label` | `string` | — | Accessible name and optional visible label |
| `tone` | `accent \| success \| danger` | `accent` | Semantic progress tone |
| `ariaLabel` | `string` | — | Accessible name for the host wrapper |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |
| `className` | `string` | — | Host-approved semantic style extension |

## States and accessibility

The component renders a native `<progress>`. `value` is clamped to 0–100. Omitting `value` preserves native indeterminate semantics. Provide `label` when the Job needs an accessible name.

Behavior is listed in the [Progress manual](/reference/ui-kit/components/primitives/progress).

## Customization and migration

Use semantic tones, theme tokens, and Job projection. Changing value bounds, label semantics, or indeterminate behavior requires a component contract update.
