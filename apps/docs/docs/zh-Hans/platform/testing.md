---
id: testing
title: 测试与 qualification
sidebar_label: 测试与 qualification
---

# 测试与 qualification

平台合同需要同时验证 schema、行为、浏览器交互和失败恢复。UI Kit 的 story 是可执行证据，不能用静态截图代替。

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-browser
npm run verify:ui-kit
```

每个跨边界能力至少需要覆盖：

- valid round-trip 或 projection
- unknown schema、version、capability 或 contribution rejection
- cancellation、timeout 或 stale-result
- extension failure 后核心状态仍然可用
- disable、rollback 或 uninstall cleanup
- migration 或 recovery

完整矩阵见 [Contract matrix](/reference/plugin-contract-matrix)。
