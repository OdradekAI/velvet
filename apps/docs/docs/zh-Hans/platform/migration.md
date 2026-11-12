---
id: migration
title: 迁移与兼容
sidebar_label: 迁移与兼容
---

# 迁移与兼容

Velvet 把 manifest schema、host API、bridge API、runtime protocol、theme schema 和 component contract 独立版本化。

## 兼容规则

- optional additive fields 可以在同一 major 中加入。
- required field、message meaning、state owner 或 keyboard model 的变化需要 major decision。
- 版本变化需要 contract fixture 或 migration evidence。
- reconnect 会创建新 session，旧事件不会盲目 replay。
- extension failure、disable 和 uninstall 不得改变核心 Asset、Layout、Preview 或 Job authority。

当前版本面见 [manifest reference](/reference/plugin-manifest-v1)，manifest 和 UI contract 见 [reference](/reference/ui-kit-contract-v1)。
