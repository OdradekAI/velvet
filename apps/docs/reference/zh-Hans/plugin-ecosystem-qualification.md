---
id: plugin-ecosystem-qualification
title: 插件生态 qualification gate
---

# 插件生态 qualification gate

当前 Velvet 仍处于 Alpha。插件进入更广泛生态前，需要通过 contract、runtime、security、compatibility、failure isolation 和 documentation gate。

## Gate

- manifest schema 和 host API range 有 fixture。
- capability、zone、scope 和 origin 校验有拒绝证据。
- UI Bridge 有 theme、locale、focus、dispose 和 error evidence。
- 插件崩溃、超时、disable、rollback 和 uninstall 不影响核心状态。
- migration、retry、cancellation 和 diagnostics 可复现。
- 组件页面包含 Demo、API、无障碍、定制和兼容说明。

## 当前限制

仓库目前不发布 Marketplace ABI、外部 SDK、签名/撤销服务或跨平台 production runtime。Asset Inspector 和 Echo sidecar 是可重复的 host fixtures，不能被描述为完整生态能力。
