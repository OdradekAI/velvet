---
id: customization
title: 定制与主题
sidebar_label: 定制与主题
---

# 定制与主题

公共 UI 定制入口包括语义 token、密度、对比度、减少动态效果和组件变体。

## Token 快照

UI Bridge 发送只读的 `velvet.ui.theme/1` 快照。允许列表包含 `surface.*`、`border.*`、`text.*`、`accent.*`、`status.*`、`focus.*` 系列中的指定 token，以及 `control.height` 和 `control.radius`。这些系列名称用于描述分类，不是允许任意 token 名称的通配符。

组件实现 token、Tailwind class、字体文件、DOM 结构和 Radix data attribute 仍属宿主私有实现。

<span id="定制范围" className="velvet-doc-anchor" />

## 定制边界

| 表面 | 入口 |
| --- | --- |
| 可信 wrapper | 语义属性、变体和类型化 React 事件 |
| 隔离 surface | 主题快照、语义组件协商和 surface 自己的渲染器 |
| 全局工作台 | 宿主渲染器应用主题；插件不能把快照反写给宿主 |

Schema 和 token 过滤规则见 [UI Kit 契约](/reference/ui-kit-contract-v1)。兼容 token 的渲染不授予宿主组件实现的访问权。
