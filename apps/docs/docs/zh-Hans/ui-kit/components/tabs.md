---
id: tabs
title: Tabs
sidebar_label: Tabs
---

import { TabsPlayground } from '@site/src/components/playgrounds/tabs';

# VelvetTabs

`VelvetTabs` 用于在同一语义区域内切换内容。它提供 tablist、roving tabindex、选中状态和键盘导航，内容面板由可信 wrapper 的调用方组合。

## Demo

<TabsPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `tabs` | `readonly VelvetTab[]` | — | Tab 项定义 |
| `selectedId` | `string` | — | 当前选中 Tab 的 id |
| `onSelect` | `(id: string) => void` | — | 选中或键盘导航回调 |
| `ariaLabel` | `string` | `Tabs` | tablist 的可访问名称 |
| `describedBy` | `string` | — | 外层 wrapper 的说明节点引用 |
| `ariaLive` | `off \| polite \| assertive` | — | 外层 wrapper 的动态播报语义 |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |
| `className` | `string` | — | 宿主允许的语义样式扩展 |

### VelvetTab

| Property | Type | Default | Description |
| --- | --- | --- | --- |
| `id` | `string` | — | 稳定且唯一的 Tab 标识 |
| `label` | `string` | — | Tab 的可见名称 |
| `disabled` | `boolean` | `false` | 跳过选择和键盘导航 |
| `badge` | `string \| number` | — | 显示在 label 后的状态数量或短标记 |

## 状态与动作

- `selected`
- `disabled`
- `focus`
- `select`
- `navigate`

点击、Enter 或 Space 触发 `onSelect`。水平 Tab 使用 ArrowLeft 和 ArrowRight 导航，ArrowUp 和 ArrowDown 也会被接受。Home 和 End 选择第一个或最后一个可用 Tab。

## 无障碍

组件输出 `role="tablist"` 和 `role="tab"`，使用 `aria-selected` 表达当前状态，并通过 roving `tabIndex` 保持单一键盘入口。组件本身不生成 `tabpanel`，宿主可以用 `aria-labelledby` 引用 `velvet-tab-${id}`，为对应面板命名。当前 `VelvetTab` 没有 panel ID 或 `aria-controls` 字段，wrapper 不会建立该关系。必须提供有效且可用的 `selectedId` 才能保留键盘入口；所有 Tab ID 在已渲染的多个 tablist 之间也必须唯一。

真实行为与 story、浏览器证据见 [Tabs manual](/reference/ui-kit/components/primitives/tabs)。

## 定制

使用 semantic theme token、selected/disabled 状态和宿主的 focus token。不要依赖内部 button 顺序、私有 class 或 CSS selector 作为插件 contract。

## 迁移与 qualification

改变 roving tabindex、方向键、Home/End 选择、Tab disabled 规则或 panel 关联方式，需要更新 component contract 和浏览器 fixture。修改后运行 `npm run qualify:ui-primitives`。
