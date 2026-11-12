---
id: index
title: 维护者地图
---

# 维护者地图

从你要修改的状态或契约 owner 开始。本页是当前仓库的导航地图，不新增运行时分层。

| 关注点 | Owner / 实现 | 决策与证据 |
| --- | --- | --- |
| 领域词汇与 Asset 身份 | `asset-core`；`docs/GLOSSARY.md` | [词汇表](https://github.com/OdradekAI/velvet/blob/main/docs/GLOSSARY.md)、ADR-0002 |
| 应用组合与传输 | `application-core`、`workbench-protocol`、`tauri-adapter`；桌面 `platform` | ADR-0001 / ADR-0006；[契约矩阵](/reference/plugin-contract-matrix) |
| 扩展声明与生命周期 | `plugin-api`、`plugin-host` | ADR-0005；[Manifest](/reference/plugin-manifest-v1)、[Registry](/reference/plugin-registry-v1) |
| 布局、预览和后台工作 | `layout-model`、`preview-core`、`job-system` | ADR-0003 / ADR-0004 / ADR-0007；owner crate 测试 |
| 持久化与恢复 | `persistence` 和领域 owner | ADR-0013 / ADR-0014；原生发布验证 |
| 桌面 UI 与插件 surface | `app`、`platform`、`workbench`、`workspaces` | ADR-0008 / ADR-0009 / ADR-0011 / ADR-0012；[UI 覆盖](/reference/ui-kit/action-coverage) |
| 包信任与发布 | PluginHost 和发布工程 | ADR-0015；[生态门禁](/reference/plugin-ecosystem-qualification) |
| 文档应用 | `apps/docs` | [文档结构](./documentation-structure)；PR docs 检查 |

在 [ADR 目录](https://github.com/OdradekAI/velvet/tree/main/docs/adr)查找具体决策并读取其实际状态：**Accepted/Proposed 是设计状态，不证明全部验收场景已通过**。[基础计划](https://github.com/OdradekAI/velvet/blob/main/docs/engineering/workbench-foundation-plan.md)记录实现顺序和验收，[原生验证矩阵](https://github.com/OdradekAI/velvet/blob/main/docs/engineering/native-release-qualification-matrix.md)记录平台证据。

## 阅读层次

- Guide 教如何完成任务，并链接唯一的权威契约。
- Reference 定义声明、消息、所有权、错误和兼容性。
- Engineering 记录实现边界和可重复的验证流程。
- ADR 保存决策、备选方案和后果。
- Research 保存证据与假设，不自动成为 API 承诺。

`CoreTransport` 继续作为桌面与 Rust 的边界。演示消费现有的可信 UI 入口；文档不能成为第二个持久化、权限或运行时 owner。[API 索引](/reference/plugin-api-index)区分扩展协议、内部 Registry 方法和 fixture 便捷句柄。
