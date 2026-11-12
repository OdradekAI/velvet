---
id: inline
title: Inline
sidebar_label: Inline
---

import { InlinePlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetInline

`VelvetInline` 是固定使用 row direction 的 Stack 变体，用于徽章、文本和紧凑操作的水平排列。

## Demo

<InlinePlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | 子元素 |
| `gap` | `none \| xs \| sm \| md \| lg` | `md` | semantic spacing token |
| `className` | `string` | — | 宿主允许的语义样式扩展 |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |

## 状态与无障碍

Inline 输出水平布局容器，不改变子元素的阅读顺序、焦点顺序或键盘行为。需要换行或复杂布局时使用 Stack 或宿主 projection。

真实行为见 [Inline manual](/reference/ui-kit/components/primitives/inline)。

## 定制与迁移

使用 spacing token。Inline 始终保持 row direction，改变这一约定需要迁移到 Stack contract。
