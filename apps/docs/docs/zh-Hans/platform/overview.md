---
id: overview
title: Velvet Plugin Platform
sidebar_label: 总览
---

# Velvet Plugin Platform

Velvet 的插件平台是文档主线。插件通过 manifest、contribution、capability 和 UI surface 扩展 Workbench。UI Kit 是插件平台提供的 UI 能力之一，服务于受信任的 first-party UI 和隔离的 third-party UI surface。

## 平台分层

```text
Plugin Platform
├── Contributions     View、Command、Menu、Keybinding、Preview、Job
├── Capability grants  权限、scope、zone、namespace
├── UI surfaces        UI Kit、UI Bridge、Overlay、Host projection
├── Lifecycle          activation、suspend、failure、retry、dispose
├── Transport          typed request、query、event、session、resync
└── Qualification      contract、story、browser、release gate
```

## 选择阅读路径

- 第一次开发插件，从 [快速开始](./quick-start) 开始。
- 想贡献 View、Command 或 Job，阅读 [插件贡献](./contributions)。
- 想渲染 UI，先阅读 [选择 contribution](./choose-contribution)，再进入 [UI Kit](../ui-kit) 或 [UI Bridge](./ui-bridge)。
- 想了解安全边界，阅读 [安全模型](./security)。
- 想查组件 Props、状态和交互，进入 [UI Kit 组件](../ui-kit/components/button)。
