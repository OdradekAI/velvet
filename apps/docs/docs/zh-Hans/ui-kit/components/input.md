---
id: input
title: Input
sidebar_label: Input
---

import { InputPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetInput

`VelvetInput` 用于文本、搜索和邮件等原生输入场景。它保留浏览器的输入语义，同时接入 UI Kit 的主题、busy、disabled、版本和 qualification 属性。

## Demo

<InputPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` | `string \| number \| readonly string[]` | — | 受控输入值 |
| `defaultValue` | `string \| number \| readonly string[]` | — | 非受控初始值 |
| `type` | `string` | `text` | 原生输入类型 |
| `placeholder` | `string` | — | 空值时显示的提示 |
| `onChange` | `ChangeEventHandler<HTMLInputElement>` | — | 输入变化回调 |
| `disabled` | `boolean` | `false` | 禁止编辑和聚焦 |
| `busy` | `boolean` | `false` | 设置 busy 语义并禁止编辑 |
| `ariaLabel` | `string` | — | 映射为原生 `aria-label` |
| `describedBy` | `string` | — | 映射为原生 `aria-describedby` |
| `ariaLive` | `off \| polite \| assertive` | — | 设置 live region 语义 |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |
| `className` | `string` | — | 宿主允许的语义样式扩展 |
| `data-*` | `string \| boolean` | — | 调试或 qualification 元数据 |

`name`、`id`、`autoComplete`、`required`、`inputMode` 和其他原生 input 属性继续通过类型化 React surface 支持。

## 状态与动作

- `default`
- `focus`
- `disabled`
- `busy`
- `change`

`value` 和 `defaultValue` 分别对应受控与非受控用法。`busy` 会设置 `aria-busy="true"` 并阻止编辑。输入事件通过 `onChange` 返回给宿主或插件 surface。

## 无障碍

组件输出原生 `<input>`，保留文本编辑、Tab 聚焦和浏览器键盘行为。页面应使用可见 `<label>` 关联输入，无法提供可见标签时再使用 `ariaLabel`。错误、帮助和格式说明通过 `describedBy` 关联到说明节点。

真实行为与 story、浏览器证据见 [Input manual](/reference/ui-kit/components/primitives/input)。

## 定制

使用 semantic theme token、density、contrast 和宿主提供的状态样式。Tailwind class、私有 DOM 结构和内部 CSS selector 不属于插件 contract。

## 迁移与 qualification

改变 busy 对编辑行为的影响、原生键盘模型或 ARIA 映射，需要更新 component contract 和 fixture。修改后运行 `npm run verify:ui-kit`。
