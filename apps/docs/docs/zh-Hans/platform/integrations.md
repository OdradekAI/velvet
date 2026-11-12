---
id: integrations
title: 集成
sidebar_label: 集成
---

# 集成

平台按 surface 提供不同集成方式。

| 集成方式 | 适用对象 | 当前边界 |
| --- | --- | --- |
| Velvet React wrapper | 受信任的 first-party extension | 只允许使用 Velvet-owned wrapper |
| iframe surface | third-party browser UI | origin、CSP、Bridge message validation |
| Tauri child WebView | 经过宿主批准的本地 surface | 需要显式 capability 和 lifecycle cleanup |
| Plain HTML / Vue / Svelte | isolated surface | 通过 framework-neutral Bridge 通信 |

插件不要直接依赖宿主 Tailwind build、Radix type 或 React component。UI Kit 组件页提供 semantic props 和交互 contract，UI Bridge 页提供跨框架消息 contract。
