---
id: choose-contribution
title: 选择 contribution 和 UI surface
sidebar_label: 选择 contribution
---

# 选择 contribution 和 UI surface

先确定插件要扩展的宿主能力，再决定是否需要 UI。

| 目标 | Contribution | UI 入口 |
| --- | --- | --- |
| 增加导航或工具入口 | `view-container`、`view`、`activity-bar-entry` | UI Kit 或 UI Bridge |
| 增加一次性操作 | `command`、`menu`、`keybinding` | 宿主 Command Palette、Context Menu 或 Overlay |
| 增强 Asset 预览 | `preview-provider`、`preview-augmentation`、`inspector-section` | PreviewHost、Inspector、UI Bridge |
| 运行耗时处理 | `job` | OperationsPanel、Progress projection |
| 提供可复用数据能力 | `source-connector`、`content-kind-provider`、`processor` | 由其他 contribution 消费 |

## UI surface 的服务对象

- **受信任的 first-party extension** 可以复用 Velvet React wrapper，直接共享宿主交互实现。
- **第三方 extension** 使用 framework-neutral UI Bridge，可以在自己的 React、Vue、Svelte 或原生 HTML surface 中渲染。
- **只需要宿主交互** 的插件不应创建自定义 surface，直接使用 Command、Menu、Notification、Progress 或 Host projection。

宿主始终拥有布局、焦点、Overlay、全局快捷键、权限检查和失败恢复。
