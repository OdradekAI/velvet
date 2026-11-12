---
id: text
title: Text
sidebar_label: Text
---

import { TextPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetText

`VelvetText` 用于语义化文本和 tone wrapper。它让插件 surface 使用宿主的字体、颜色和 HTML 元素语义。

## Demo

<TextPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `as` | `p \| span \| strong \| small \| h1 \| h2 \| h3` | `span` | 输出的 HTML 元素 |
| `tone` | `primary \| secondary \| muted \| danger \| success \| warning` | `primary` | 语义化文字色调 |
| `children` | `ReactNode` | — | 文本内容 |
| `id` | `string` | — | DOM id |
| `role` | `string` | — | 可选语义 role |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |

## 状态与无障碍

Text 保留选择、朗读和原生 HTML 元素语义，不引入键盘 action。标题和段落应按文档层级选择 `as`，tone 不应承担额外语义。

真实行为见 [Text manual](/reference/ui-kit/components/primitives/text)。

## 定制与迁移

使用 `as`、tone 和 theme token。改变元素语义或 tone 含义时，需要更新组件 contract 和浏览器证据。
