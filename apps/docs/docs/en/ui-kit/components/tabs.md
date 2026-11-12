---
id: tabs
title: Tabs
sidebar_label: Tabs
---

import { TabsPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetTabs

`VelvetTabs` switches content within one semantic region. It provides a tablist, roving tabindex, selection state, and keyboard navigation. The host or plugin surface composes the content panel.

## Demo

<TabsPlayground locale="en" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `tabs` | `readonly VelvetTab[]` | — | Tab item definitions |
| `selectedId` | `string` | — | Selected tab id |
| `onSelect` | `(id: string) => void` | — | Selection and keyboard navigation callback |
| `ariaLabel` | `string` | `Tabs` | Accessible tablist name |
| `describedBy` | `string` | — | Tablist description reference |
| `ariaLive` | `off \| polite \| assertive` | — | Tablist live-region semantics |
| `apiVersion` | `string` | `1.0` | UI Kit contract version marker |
| `testId` | `string` | — | Stable qualification hook |
| `className` | `string` | — | Host-approved semantic style extension |

### VelvetTab

| Property | Type | Default | Description |
| --- | --- | --- | --- |
| `id` | `string` | — | Stable unique tab id |
| `label` | `string` | — | Visible tab name |
| `disabled` | `boolean` | `false` | Skips selection and keyboard navigation |
| `badge` | `string \| number` | — | Short status count or marker after the label |

## States and actions

- `selected`
- `disabled`
- `focus`
- `select`
- `navigate`

Click, Enter, and Space activate `onSelect`. Horizontal tabs use ArrowLeft and ArrowRight for navigation. ArrowUp and ArrowDown are also accepted. Home and End select the first and last enabled tab.

## Accessibility

The component renders `role="tablist"` and `role="tab"`, exposes selection through `aria-selected`, and keeps one keyboard entry point through roving `tabIndex`. It does not create a `tabpanel`. The host must pair the selected tab with the panel, `aria-controls`, and `aria-labelledby`.

Behavior, stories, and browser evidence are listed in the [Tabs manual](/reference/ui-kit/components/primitives/tabs).

## Customization

Use semantic theme tokens, selected and disabled states, and the host focus token. Do not use private button order, class names, or CSS selectors as a plugin contract.

## Migration and qualification

Changing roving tabindex, arrow-key behavior, Home/End selection, disabled-tab rules, or panel association requires a component contract and browser fixture update. Run `npm run qualify:ui-primitives` after the change.
