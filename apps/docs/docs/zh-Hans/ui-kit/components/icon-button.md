---
id: icon-button
title: IconButton
sidebar_label: IconButton
---

import { IconButtonPlayground } from '@site/src/components/playgrounds/icon-button';

# VelvetIconButton

`VelvetIconButton` 用于紧凑的 icon-first action。它继承 Button 的状态和 action contract，并强制提供可访问 label。

## Demo

<IconButtonPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `icon` | `ReactNode` | — | 视觉图标内容 |
| `label` | `string` | — | 必填的可访问名称 |
| `disabled` | `boolean` | `false` | 阻止激活 |
| `busy` | `boolean` | `false` | 显示 busy 并阻止激活 |
| `onClick` | `MouseEventHandler` | — | action 回调 |
| `variant` | `default \| quiet \| danger` | `default` | Button 视觉变体 |
| `ariaLive` | `off \| polite \| assertive` | — | 公共类型已声明；当前 Button 未映射到原生 `aria-live` |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |

## 状态与无障碍

组件使用原生 button，Enter 和 Space 可以激活。`label` 会映射为 `aria-label`。busy 和 disabled 状态都会阻止 action。

真实行为见 [IconButton manual](/reference/ui-kit/components/primitives/icon-button)。

## 定制与迁移

使用 icon、semantic variant 和 theme token。改变 label、键盘模型或 busy 行为时，需要同步更新 Button contract 和 fixture。
