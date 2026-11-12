---
id: icon
title: Icon
sidebar_label: Icon
---

import { IconPlayground } from '@site/src/components/playgrounds/icon';

# VelvetIcon

`VelvetIcon` 是可访问或装饰性图标 wrapper。它可以使用宿主提供的 semantic glyph，也可以通过 children 提供兼容性内容。

## Demo

<IconPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `name` | `VelvetIconName` | — | 宿主 glyph 名称 |
| `children` | `ReactNode` | — | 自定义 renderer 内容 |
| `label` | `string` | — | 可访问名称，存在时添加 `role="img"` |
| `className` | `string` | — | 宿主允许的语义样式扩展 |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |

`VelvetIconName` 包括 `add`、`check`、`search`、`settings`、`spark`、`warning` 等宿主 glyph。

## 状态与无障碍

没有 `label` 时图标标记为装饰性并设置 `aria-hidden`。有 `label` 时输出 `role="img"` 和 `aria-label`。Icon 本身不引入键盘 action。

真实行为见 [Icon manual](/reference/ui-kit/components/primitives/icon)。

## 定制与迁移

优先使用宿主 semantic glyph。改变 glyph 名称、label 语义或自定义 children contract 时，需要更新 component contract。
