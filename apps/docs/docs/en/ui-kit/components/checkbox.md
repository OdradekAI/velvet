---
id: checkbox
title: Checkbox
sidebar_label: Checkbox
---

import { CheckboxPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetCheckbox

`VelvetCheckbox` is a controlled boolean setting. It provides a visible label, a native checkbox, checked state, and a host callback for preferences, filters, and workbench toggles.

## Demo

<CheckboxPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `checked` | `boolean` | — | Controlled checked state |
| `label` | `string` | — | Visible name associated with the checkbox |
| `onCheckedChange` | `(checked: boolean) => void` | — | State change callback |
| `disabled` | `boolean` | `false` | Prevents toggling and focus |
| `ariaLabel` | `string` | — | Accessible name supplement for the wrapper |
| `describedBy` | `string` | — | Wrapper description reference |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |
| `className` | `string` | — | Host-approved semantic style extension |

Checkbox is controlled. After `onCheckedChange`, the host should update the `checked` value rendered on the next pass.

## States and actions

- `checked`
- `unchecked`
- `disabled`
- `focus`
- `toggle`

Clicking the label or checkbox calls `onCheckedChange`. Disabled checkboxes do not toggle.

## Accessibility

The component renders a native `<input type="checkbox">` and links the visible `label` to the control through a generated id. Keyboard users can focus with Tab and toggle with Space. Prefer a clear visible label. Use `ariaLabel` only to add context that cannot be inferred from the label.

Behavior, stories, and browser evidence are listed in the [Checkbox manual](/reference/ui-kit/components/primitives/checkbox).

## Customization

Use semantic theme tokens, checked, unchecked, and disabled states, and the host focus token. Plugins should not depend on private wrapper DOM, class names, or generated ids.

## Migration and qualification

Changing the controlled-state model, label association, Space-key behavior, or disabled rules requires a component contract and fixture update. Run `npm run verify:ui-kit` after the change.
