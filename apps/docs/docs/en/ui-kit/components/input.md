---
id: input
title: Input
sidebar_label: Input
---

import { InputPlayground } from '@site/src/components/playgrounds/input';

# VelvetInput

`VelvetInput` covers native text, search, and email entry. It keeps browser input semantics while adding UI Kit theme, busy, disabled, version, and qualification props.

## Demo

<InputPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` | `string \| number \| readonly string[]` | — | Controlled input value |
| `defaultValue` | `string \| number \| readonly string[]` | — | Uncontrolled initial value |
| `type` | `string` | `text` | Native input type |
| `placeholder` | `string` | — | Hint shown when empty |
| `onChange` | `ChangeEventHandler<HTMLInputElement>` | — | Input change callback |
| `disabled` | `boolean` | `false` | Prevents editing and focus |
| `busy` | `boolean` | `false` | Sets busy semantics and prevents editing |
| `ariaLabel` | `string` | — | Maps to native `aria-label` |
| `describedBy` | `string` | — | Maps to native `aria-describedby` |
| `ariaLive` | `off \| polite \| assertive` | — | Declared in the common type; current wrapper does not map it to native `aria-live` |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |
| `className` | `string` | — | Host-approved semantic style extension |
| `data-*` | `string \| boolean` | — | Debug or qualification metadata |

`name`, `id`, `autoComplete`, `required`, `inputMode`, and other native input attributes remain available through the typed React surface.

## States and actions

- `default`
- `focus`
- `disabled`
- `busy`
- `change`

Use `value` for controlled input and `defaultValue` for uncontrolled input. `busy` sets `aria-busy="true"` and prevents editing. Input events reach the trusted wrapper caller through `onChange`.

## Accessibility

The component renders a native `<input>` and keeps text editing, Tab focus, and browser keyboard behavior. Use a visible `<label>` when possible. Use `ariaLabel` when no visible label exists, and connect help or error text through `describedBy`.

The current implementation forwards `ariaLive` as a camelCase prop instead of mapping it to `aria-live`; even a direct native `aria-live` prop is overwritten by the common-prop projection. Use a separate live region for announcements until this implementation gap is fixed.

Behavior, stories, and browser evidence are listed in the [Input manual](/reference/ui-kit/components/primitives/input).

## Customization

Use semantic theme tokens, density, contrast, and host-provided state styles. Tailwind classes, private DOM structure, and internal CSS selectors are outside the plugin contract.

## Migration and qualification

Changing busy behavior, the native keyboard model, or ARIA mapping requires a component contract and fixture update. Run `npm run verify:ui-kit` after the change.
