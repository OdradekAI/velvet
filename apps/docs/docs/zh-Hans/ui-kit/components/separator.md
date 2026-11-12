---
id: separator
title: Separator
sidebar_label: Separator
---

import { SeparatorPlayground } from '@site/src/components/playgrounds/separator';

# VelvetSeparator

`VelvetSeparator` 用于视觉和语义分隔。它输出 `role="separator"`，可选择水平或垂直方向。

## Demo

<SeparatorPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `orientation` | `horizontal \| vertical` | `horizontal` | 分隔方向 |

## 状态与无障碍

组件输出 `role="separator"` 和 `aria-orientation`。Separator 不引入键盘 action，也不应承载可交互内容。

真实行为见 [Separator manual](/reference/ui-kit/components/primitives/separator)。

## 定制与迁移

使用 border、density 和 semantic spacing token。改变 orientation 语义或 role 时，需要更新 component contract。
