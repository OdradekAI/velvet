---
id: customization
title: 定制与主题
sidebar_label: 定制与主题
---

# 定制与主题

Velvet 的公共定制入口是 semantic tokens、density、contrast、reduced motion 和组件 semantic variants。

## Token 快照

UI Bridge 使用 `velvet.ui.theme/1` 发送只读 snapshot。插件可以消费 `surface.*`、`border.*`、`text.*`、`accent.*`、`status.*`、`focus.*` 和 control geometry tokens。

组件实现 token、Tailwind class、font file、DOM structure 和 Radix data attributes 属于宿主内部实现。

## 定制范围

| 范围 | 入口 |
| --- | --- |
| 单个 trusted wrapper | semantic props、variant、standard React event |
| isolated surface | theme snapshot、CSS variables、component contract |
| 全局 Workbench | host-owned theme service 和 Profile projection |

主题 schema 和 token 过滤见 [UI Kit contract v1](/reference/ui-kit-contract-v1)。
