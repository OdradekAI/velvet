---
id: progress
title: Progress
sidebar_label: Progress
---

import { ProgressPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetProgress

`VelvetProgress` 用于 Job 或长时间操作的进度 projection。它支持 determinate 和 indeterminate 两种状态。

## Demo

<ProgressPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` | `number` | — | 0 到 100 的 determinate 值，省略时为 indeterminate |
| `label` | `string` | — | progress 的可访问名称和可选可见标签 |
| `tone` | `accent \| success \| danger` | `accent` | 语义化进度色调 |
| `ariaLabel` | `string` | — | 宿主 wrapper 的可访问名称 |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |
| `className` | `string` | — | 宿主允许的语义样式扩展 |

## 状态与无障碍

组件输出原生 `<progress>`。`value` 会被限制在 0 到 100，省略 `value` 时保留浏览器的 indeterminate 语义。需要说明当前 Job 时应提供 `label`。

真实行为见 [Progress manual](/reference/ui-kit/components/primitives/progress)。

## 定制与迁移

使用 semantic tone、theme token 和 Job projection。改变 value 边界、label 语义或 indeterminate 行为时，需要更新 component contract。
