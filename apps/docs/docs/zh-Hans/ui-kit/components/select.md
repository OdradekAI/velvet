---
id: select
title: Select
sidebar_label: Select
---

import { SelectPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetSelect

`VelvetSelect` 用于单选或多选的原生 option 控件。它保留浏览器的选择和键盘语义，同时接入 UI Kit 的主题、busy、disabled、版本和 qualification 属性。

## Demo

<SelectPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | 原生 `option` 或 `optgroup` |
| `value` | `string \| number \| readonly string[]` | — | 受控选中值 |
| `defaultValue` | `string \| number \| readonly string[]` | — | 非受控初始选中值 |
| `onChange` | `ChangeEventHandler<HTMLSelectElement>` | — | 选择变化回调 |
| `multiple` | `boolean` | `false` | 启用原生多选语义 |
| `disabled` | `boolean` | `false` | 禁止选择和聚焦 |
| `busy` | `boolean` | `false` | 设置 busy 语义并禁止选择 |
| `ariaLabel` | `string` | — | 映射为原生 `aria-label` |
| `describedBy` | `string` | — | 映射为原生 `aria-describedby` |
| `ariaLive` | `off \| polite \| assertive` | — | 设置 live region 语义 |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |
| `className` | `string` | — | 宿主允许的语义样式扩展 |
| `data-*` | `string \| boolean` | — | 调试或 qualification 元数据 |

`name`、`required`、`size`、`form` 和其他原生 select 属性继续通过类型化 React surface 支持。

## 状态与动作

- `default`
- `focus`
- `disabled`
- `change`

`value` 和 `defaultValue` 分别对应受控与非受控用法。`multiple` 使用原生多选行为。`busy` 会设置 busy 语义并通过 disabled 阻止选择。

## 无障碍

组件输出原生 `<select>`，保留 option 朗读、方向键选择和表单提交语义。页面应使用可见 `<label>` 关联控件，无法提供可见标签时使用 `ariaLabel`。多选控件应同时说明选择方式和可用选项数量。

真实行为与 story、浏览器证据见 [Select manual](/reference/ui-kit/components/primitives/select)。

## 定制

使用 semantic theme token、density、contrast 和宿主提供的状态样式。插件不应依赖宿主的私有 class、DOM 结构或原生 select 的平台外观。

## 迁移与 qualification

改变 option 语义、multiple 行为、busy 状态或键盘模型，需要更新 component contract 和 fixture。修改后运行 `npm run verify:ui-kit`。
