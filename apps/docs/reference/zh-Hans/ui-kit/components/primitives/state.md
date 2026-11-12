---
id: state
title: State 手册
---

# VelvetState

State 组合 loading、empty 和 error 状态。Props 包括 `kind`、`title`、`description`、`retryLabel`、`onRetry`、`action`、`apiVersion` 和 `testId`。

loading 使用 busy/status，empty 使用 status，error 使用 alert。提供 `onRetry` 时会生成可键盘触发的 Button。完整 Demo 见 [State API 与 Playground](/ui-kit/components/state)。
