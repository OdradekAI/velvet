---
id: quick-start
title: 运行首个插件 fixture
---

# 运行首个插件 fixture

<span id="快速开始" className="velvet-doc-anchor" />

本教程使用确定性的内存宿主运行 Asset Inspector。需要本仓库的 checkout 和 **Node.js 22**（CI 基线）。它不会向桌面 Profile 安装插件，也不会读取你的素材。fixture 宿主不是可下载的 SDK。

<span id="当前可运行样例" className="velvet-doc-anchor" />

## 1. 在仓库根目录运行

```sh
node --version
node examples/plugins/asset-inspector/run-demo.mjs
```

首次运行不需要 npm install 或 Rust 构建。JSON 输出包含：

```json
{
  "extensionId": "com.acme.asset-inspector",
  "status": "completed",
  "registeredContributions": 7,
  "facets": ["com.acme.asset-inspector/quality:asset-1"]
}
```

输出还包含 `uiComponents` 和本机 `packageRoot`。成功表示 fixture 完成了贡献注册、假数据选区分析、自己的 facet 写入与注册清理，不代表生产运行时或安装程序已经通过验证。

## 2. 认识插件包

| 文件 | 阅读重点 |
| --- | --- |
| `examples/plugins/asset-inspector/manifest.json` | 身份、API 范围、贡献、能力和存储 schema |
| `examples/plugins/asset-inspector/src/asset-inspector.mjs` | `activate(host)`、`analyzeSelection`、取消和销毁 |
| `examples/plugins/asset-inspector/src/fixture-host.mjs` | 假数据与调用记录；这是测试设施 |
| `examples/plugins/asset-inspector/run-demo.mjs` | 有界选区和预期结果 |

[查看示例源码](https://github.com/OdradekAI/velvet/tree/main/examples/plugins/asset-inspector)，并对照 [schema 参考](/reference/plugin-manifest-v1)。复制示例时，同时修改导出的身份常量、校验逻辑和 manifest；只改 manifest ID 会按设计触发校验失败。

## 3. 验证失败路径

安装桌面测试依赖、编译测试输入，再运行针对性的契约测试：

```sh
npm --prefix apps/desktop ci
npm --prefix apps/desktop run test:compile
node apps/desktop/test/asset-inspector-demo.test.mjs
```

测试覆盖权限拒绝、过期指纹、取消、未知结果、runner 失败、迁移与清理。预期进程成功退出。[测试指南](./testing) 说明这些证据的适用范围。

## 排错与下一步

- `Cannot find module`：确认位于仓库根目录，示例文件完整。
- `manifest-invalid` 或 `manifest-incompatible`：对照示例常量及 schema 检查 ID 和版本范围，不绕过校验。
- 测试失败应保留证据排查，不应重置真实 Profile。

继续阅读[能力目录](./capabilities)、[Commands 与 Views](../guides/commands-and-views) 或 [Asset 分析](../guides/asset-analysis)。独立 JSONL 进程示例见[隔离 UI 与 sidecar](../guides/isolated-ui)。
