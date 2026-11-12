---
id: testing
title: 测试与验证
---

# 测试与验证

<span id="测试与-qualification" className="velvet-doc-anchor" />

首个 fixture 只需 Node。运行桌面和传输契约测试前，在仓库准备依赖与测试输入：

```sh
npm --prefix apps/desktop ci
npm --prefix apps/desktop run test:compile
node apps/desktop/test/asset-inspector-demo.test.mjs
```

选择能力指南链接的针对性测试。成功退出只证明所测 fixture 边界。覆盖成功、权限或版本拒绝、取消或超时、过期结果、owner 失败和清理，并保持其他扩展及核心状态可用。

完整仓库检查、浏览器验证和原生发布矩阵见[维护者地图](/maintenance)与[契约矩阵](/reference/plugin-contract-matrix)。本地 fixture 通过，不代表生产包信任、安全沙箱或全部 OS/WebView 已经验证。
