---
id: state
title: State
sidebar_label: State
---

import { StatePlayground } from '@site/src/components/playgrounds/state';

# VelvetState

`VelvetState` composes loading, empty, and error states. This primitive renders caller-owned state semantics, live regions, error retry and optional action slots. It does not own the operation or retry lifecycle.

## Demo

<StatePlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `kind` | `loading \| empty \| error` | — | State kind |
| `title` | `string` | — | State title |
| `description` | `string` | — | Optional description |
| `retryLabel` | `string` | `Retry` | Error retry button label |
| `onRetry` | `() => void` | — | Error retry callback |
| `action` | `ReactNode` | — | Optional content action slot |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |

## States and accessibility

Loading uses `aria-busy` and a polite status, empty uses a status, and error uses an alert. When `onRetry` is provided, the error state renders a keyboard-activatable Button.

Behavior is listed in the [State manual](/reference/ui-kit/components/primitives/state).

## Customization and migration

Use kind, semantic tokens, and the retry contract. Changing live-region, retry, or action-slot semantics requires a component contract and recovery fixture update.
