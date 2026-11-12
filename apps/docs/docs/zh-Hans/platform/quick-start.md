---
id: quick-start
title: 快速开始
sidebar_label: 快速开始
---

# 快速开始

当前平台是 Alpha contract。最小开发路径是：

1. 定义稳定的 extension id 和 manifest version。
2. 声明 contribution、target app、zone 和 required capabilities。
3. 选择 host projection、trusted UI 或 isolated UI surface。
4. 通过宿主注册的 Command、View、Preview 或 Job 进入平台能力。
5. 用 contract fixture 和浏览器 qualification 验证成功、失败、取消和重试。

## 当前可运行样例

- Asset Inspector 展示 View、context command、Preview augmentation、Job 和 namespaced facet。
- Echo sidecar 展示外部 runtime、UI Bridge 和失败恢复。

这两个样例位于 `examples/plugins/`，详细说明见[插件开发者指南](/reference/plugin-developer-guide)。当前仓库没有发布独立的外部 SDK package，样例使用的是 in-repository Alpha contract。
