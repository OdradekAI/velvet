---
id: plugin-contract-matrix
title: 插件 contract 与集成验证矩阵
---

# 插件 contract 与集成验证矩阵

每个跨边界 contract 至少需要 schema、行为、浏览器和恢复证据。Alpha 阶段的 fixture 证明宿主 seam，不能替代生产 runtime 的 qualification。

| 层 | 需要验证的内容 | 失败时的恢复 |
| --- | --- | --- |
| Manifest | identity、schema、API range、required fields | activation 前拒绝 |
| Contribution | kind、scope、zone、container、permission | 只移除当前 projection |
| UI Bridge | origin、entrypoint、message、theme、locale、dispose | dispose 当前 surface |
| Preview | primary provider、augmentation、fallback | 保留 generic preview |
| Job | ownership、progress、cancel、retry、diagnostics | 保留可诊断 snapshot |
| Persistence | namespace、schema、fingerprint、migration | 独立迁移或丢弃 extension data |
| Lifecycle | suspend、crash、disable、uninstall | 核心 Workbench 继续可用 |

## 证据类型

- contract fixture 验证 schema 和 typed boundary。
- story 验证组件状态和 action。
- real-browser fixture 验证 focus、keyboard、portal 和 recovery。
- integration test 验证 host/plugin 组合。

新的公开字段、权限、消息、持久化状态或 compatibility promise 都需要更新矩阵。
