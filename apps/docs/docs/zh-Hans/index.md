---
id: index
title: Velvet Plugin Platform
slug: /
sidebar_position: 1
---

# Velvet Plugin Platform

Velvet 是一个由宿主 Workbench 管理布局、权限、插件生命周期和核心状态的桌面工作台。插件可以贡献 View、Command、Preview、Job 和 UI surface，UI Kit 是插件平台提供的 UI 能力层。

文档从插件平台开始，再进入 contribution、UI surface、UI Kit、Bridge、安全和 qualification。这样可以先说明插件能向宿主贡献什么，再说明组件如何渲染。

## 从这里开始

- [插件平台总览](./platform/overview)
- [快速开始](./platform/quick-start)
- [选择 contribution](./platform/choose-contribution)
- [UI Kit 总览](./ui-kit)
- [UI Kit Showcase](./ui-kit/showcase)
- [Button API 与 Playground](./ui-kit/components/button)

## 参考层

- [插件 manifest](/reference/plugin-manifest-v1)
- [UI Kit contract](/reference/ui-kit-contract-v1)
- [组件手册](/reference/ui-kit/components)
- [插件开发者指南](/reference/plugin-developer-guide)
- [Contract matrix](/reference/plugin-contract-matrix)

## 状态

当前平台和 UI Kit 仍然是 Alpha contract。组件 Demo 复用桌面应用中的真实 primitives，Host projection、Overlay、Bridge 和 runtime qualification 继续由各自的 reference 与 fixture 提供证据。
