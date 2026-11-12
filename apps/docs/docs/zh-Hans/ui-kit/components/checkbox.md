---
id: checkbox
title: Checkbox
sidebar_label: Checkbox
---

import { CheckboxPlayground } from '@site/src/components/playgrounds/checkbox';

# VelvetCheckbox

`VelvetCheckbox` 用于受控的布尔设置。它提供可见 label、原生 checkbox、checked 状态和宿主回调，适合偏好设置、筛选条件和工作区开关。

## Demo

<CheckboxPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `checked` | `boolean` | — | 受控选中状态 |
| `label` | `string` | — | 与 checkbox 关联的可见名称 |
| `onCheckedChange` | `(checked: boolean) => void` | — | 状态变化回调 |
| `disabled` | `boolean` | `false` | 禁止切换和聚焦 |
| `ariaLabel` | `string` | — | wrapper 的可访问名称补充 |
| `describedBy` | `string` | — | wrapper 的说明节点引用 |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |
| `className` | `string` | — | 宿主允许的语义样式扩展 |

Checkbox 是受控组件。宿主接收 `onCheckedChange` 后，应更新下一次渲染的 `checked` 值。

## 状态与动作

- `checked`
- `unchecked`
- `disabled`
- `focus`
- `toggle`

点击 label 或 checkbox 会触发 `onCheckedChange`。disabled 状态不会触发 toggle。

## 无障碍

组件输出原生 `<input type="checkbox">`，通过生成的 id 将可见 `label` 与控件关联。键盘用户可以使用 Tab 聚焦并使用 Space 切换。优先提供清晰的可见 `label`。`ariaLabel` 和 `describedBy` 作用于 label wrapper，不直接作用于 input，不能代替控件的可见 `label`，也不能视为已经建立 input 的描述关联。

真实行为与 story、浏览器证据见 [Checkbox manual](/reference/ui-kit/components/primitives/checkbox)。

## 定制

使用 semantic theme token、checked/unchecked/disabled 状态和宿主的 focus token。插件不应依赖 wrapper 的私有 DOM、class 或生成的 id。

## 迁移与 qualification

改变受控状态模型、label 关联、Space 键行为或 disabled 规则，需要更新 component contract 和 fixture。修改后运行 `npm run verify:ui-kit`。
