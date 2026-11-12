---
id: ui-bridge
title: UI Bridge
sidebar_label: UI Bridge
---

# UI Bridge

UI Bridge 是 third-party UI surface 与宿主之间的框架无关边界。它把 UI 的渲染实现留给插件，把布局、焦点、Overlay、权限和生命周期留给宿主。

## 宿主 → surface

- `host.ready`
- `host.context`
- `host.theme`
- `host.locale`
- `host.resize`
- `host.focus`
- `host.dispose`
- `host.error`

## Surface → 宿主

- `surface.ready`
- `surface.focus-request`
- `surface.command`
- `surface.overlay-request`
- `surface.notification`
- `surface.progress`
- `surface.close`
- `surface.error`

消息版本、entrypoint、origin、token、size 和 failure 规则见 [UI Kit contract v1](/reference/ui-kit-contract-v1)。
