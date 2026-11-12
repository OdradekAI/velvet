---
id: button
title: Button
sidebar_label: Button
---

import { ButtonPlayground } from '@site/src/components/playgrounds/button';

# VelvetButton

Button 用于 command 或 local action。宿主 command、capability 和 enablement 仍然由 Plugin Platform 决定，组件负责呈现状态和发送 action。

## Demo

<ButtonPlayground />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | Button 内容 |
| `label` | `string` | — | `children` 缺失时的内容回退 |
| `variant` | `default \| quiet \| danger` | `default` | 语义化视觉变体 |
| `disabled` | `boolean` | `false` | 阻止激活 |
| `busy` | `boolean` | `false` | 显示 busy 状态并阻止激活 |
| `onClick` | `MouseEventHandler` | — | 本地或宿主 action 回调 |
| `ariaLabel` | `string` | — | 可访问名称 |
| `describedBy` | `string` | — | 可访问描述节点引用 |
| `ariaLive` | `off \| polite \| assertive` | — | 公共类型已声明；当前 Button 未映射到原生 `aria-live` |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | 宿主 qualification hook |

wrapper 通过类型化 React surface 支持原生 button props。外部 Plugin UI Bridge 使用语义化 action message，不直接暴露 React props。

## 状态与动作

- default
- busy
- disabled
- activate

键盘激活遵循原生 Enter 和 Space 行为。busy 和 disabled 状态会阻止激活。story 和浏览器证据见 [Button manual](/reference/ui-kit/components/primitives/button)。

## 定制

使用 `variant`、semantic theme token 和宿主拥有的 theme projection。私有 DOM、Tailwind class 和内部 CSS selector 不属于公共 API。

## 迁移与 qualification

改变键盘模型、状态 owner 或 variant 含义需要 component contract decision。修改交互或无障碍行为后运行 `npm run verify:ui-kit`。
