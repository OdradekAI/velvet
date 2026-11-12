---
id: button
title: Button 手册
---

# VelvetButton

Button 用于 command 或 local action。宿主拥有 command visibility、capability check 和 enablement，组件负责呈现状态并发送 action。

## Props

`children`、`label`、`variant`、`disabled`、`busy`、`onClick`、`ariaLabel`、`describedBy`、`apiVersion` 和 `testId` 构成公开 surface。原生 button props 继续通过类型化 React wrapper 支持。

## 状态与证据

组件覆盖 default、busy、disabled 和 activate。Enter、Space、busy 和 disabled 行为见 [Button API 与 Playground](/ui-kit/components/button) 和 [UI Kit action coverage](/reference/ui-kit/action-coverage)。
