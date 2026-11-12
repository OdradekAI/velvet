---
id: asset-analysis
title: 使用 Jobs 与 facets 分析 Asset
---

# 使用 Jobs 与 facets 分析 Asset

[首个 fixture](../platform/quick-start)在不接触用户数据的情况下演示此流程。它的 `HostFacade` 是假 adapter，下列方法名只描述 fixture。

1. 读取有界选区和 `assets.readProjection(id)`。
2. 捕获选区 revision、内容指纹及配置版本。
3. 通过 `jobs.create(request)` 提交，用 `jobs.wait(id)` 观察。
4. 在 `facets.write(record)` 前拒绝过期结果。
5. 只写入扩展自己的、带 schema 版本的 facet 命名空间。
6. 通过已注册 View 或 Preview augmentation 展示结果。

真实 JobSystem 仍由 Rust 拥有。未来 SDK 必须通过授权句柄传递这些操作，不能将假宿主直接用于生产，也不能建立渲染器拥有的后台循环。

## 失败行为

| 条件 | 必须采取的行为 |
| --- | --- |
| 权限拒绝 | 停止操作；不能回退为打开原始 Asset 路径 |
| 指纹变化 | 丢弃过期结果，保留 Asset 身份 |
| 取消或禁用 | 停止新工作，清理自己的注册 |
| Job 结果未知 | 重试副作用前先核对权威状态 |
| 存储迁移失败 | 保留上一个已提交代际 |
| Provider 崩溃 | 保留通用预览和其他扩展 |

[Fixture API 目录](/reference/plugin-developer-guide#fixture-hostfacade-api-catalog)列出句柄与 owner 规则。可执行证据是 `apps/desktop/test/asset-inspector-demo.test.mjs`，准备步骤见[测试指南](../platform/testing)。[API 索引](/reference/plugin-api-index)单独标明宿主 Job 消息边界与假便捷方法。
