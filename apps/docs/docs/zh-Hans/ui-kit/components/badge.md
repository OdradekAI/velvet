---
id: badge
title: Badge
sidebar_label: Badge
---

import { BadgePlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetBadge

`VelvetBadge` 用于显示紧凑的状态、数量或 qualification 标记。它保留 children 语义，不引入键盘 action。

## Demo

<BadgePlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | Badge 内容 |
| `tone` | `neutral \| info \| success \| warning \| danger` | `neutral` | 语义化状态色调 |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |
| `className` | `string` | — | 宿主允许的语义样式扩展 |

## 状态与无障碍

组件覆盖 default 和 tone variants。Badge 本身不生成按钮或链接，不应承载需要键盘触发的 action。需要操作时，应包裹在 Button 或 Link contract 中。

真实行为见 [Badge manual](/reference/ui-kit/components/primitives/badge)。

## 定制与迁移

使用 semantic tone 和 theme token。改变 tone 含义或 child semantics 时，需要更新 component contract 和 story。
