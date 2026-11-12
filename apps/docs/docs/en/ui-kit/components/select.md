---
id: select
title: Select
sidebar_label: Select
---

import { SelectPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetSelect

`VelvetSelect` covers native single and multiple option controls. It keeps browser selection and keyboard semantics while adding UI Kit theme, busy, disabled, version, and qualification props.

## Demo

<SelectPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | Native `option` or `optgroup` elements |
| `value` | `string \| number \| readonly string[]` | — | Controlled selected value |
| `defaultValue` | `string \| number \| readonly string[]` | — | Uncontrolled initial selection |
| `onChange` | `ChangeEventHandler<HTMLSelectElement>` | — | Selection change callback |
| `multiple` | `boolean` | `false` | Enables native multiple-selection semantics |
| `disabled` | `boolean` | `false` | Prevents selection and focus |
| `busy` | `boolean` | `false` | Sets busy semantics and prevents selection |
| `ariaLabel` | `string` | — | Maps to native `aria-label` |
| `describedBy` | `string` | — | Maps to native `aria-describedby` |
| `ariaLive` | `off \| polite \| assertive` | — | Live-region semantics |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |
| `className` | `string` | — | Host-approved semantic style extension |
| `data-*` | `string \| boolean` | — | Debug or qualification metadata |

`name`, `required`, `size`, `form`, and other native select attributes remain available through the typed React surface.

## States and actions

- `default`
- `focus`
- `disabled`
- `change`

Use `value` for controlled selection and `defaultValue` for uncontrolled selection. `multiple` uses native multi-select behavior. `busy` sets busy semantics and disables selection.

## Accessibility

The component renders a native `<select>` and keeps option announcements, arrow-key selection, and form submission semantics. Use a visible `<label>` where possible, or `ariaLabel` when no visible label exists. A multiple select should explain how to select and how many options are available.

Behavior, stories, and browser evidence are listed in the [Select manual](/reference/ui-kit/components/primitives/select).

## Customization

Use semantic theme tokens, density, contrast, and host-provided state styles. Plugins should not depend on private host classes, DOM structure, or platform-specific native select appearance.

## Migration and qualification

Changing option semantics, multiple behavior, busy state, or keyboard behavior requires a component contract and fixture update. Run `npm run verify:ui-kit` after the change.
