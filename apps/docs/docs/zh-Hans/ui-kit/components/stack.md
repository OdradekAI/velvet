---
id: stack
title: Stack
sidebar_label: Stack
---

import { StackPlayground } from '@site/src/components/playgrounds/stack';

# VelvetStack

`VelvetStack` 用于按 UI Kit spacing token 组织垂直或水平内容。它只负责布局，不拥有子元素的 action 或状态。

## Demo

<StackPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | 子元素 |
| `direction` | `column \| row` | `column` | 主轴方向 |
| `gap` | `none \| xs \| sm \| md \| lg` | `md` | semantic spacing token |
| `className` | `string` | — | 宿主允许的语义样式扩展 |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |

## 状态与无障碍

Stack 输出布局容器，不改变子元素的阅读顺序、焦点顺序或键盘行为。`direction` 只改变视觉布局，不能替代语义列表或表单结构。

真实行为见 [Stack manual](/reference/ui-kit/components/primitives/stack)。

## 定制与迁移

使用 spacing token 和方向。改变 gap 含义或默认方向时，需要更新 component contract 和视觉 fixture。
