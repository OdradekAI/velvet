---
id: concepts
title: 作用域、所有权与生命周期
---

# 作用域、所有权与生命周期

产品扩展向 Workbench 贡献能力。Tauri host plugin 提供桌面或系统集成，属于不同的信任边界。

| 概念 | 含义 |
| --- | --- |
| `workbench` scope | 全局 shell 或跨应用贡献 |
| `workbench-app` scope | 面向指定 WorkbenchApp 的贡献 |
| `capability` scope | 可复用的 Preview、处理或 Job 能力 |
| Namespace | 宿主派生的可见性和身份边界；扩展不能占用其他 owner 的命名空间 |
| Zone | `navigation`、`contextual` 或 `operations`；位置不授予数据或系统权限 |
| ViewContainer / View | 宿主拥有的位置和贡献提供的有界内容 |
| WorkbenchApp / AppSurface | 顶层产品应用及其主界面；两者都不拥有全局布局 |
| Projection | 宿主权威状态的只读表示，可以重新同步 |

宿主先校验 manifest，再激活扩展。活动贡献归属于一个扩展；禁用、崩溃、回滚和卸载会撤销注册及能力句柄。UI 重试创建新会话，不重放旧副作用。

第三方 UI 消费语义投影和消息，可信宿主 UI 可以使用 React wrapper。fixture 能模拟这些边界，但不能证明生产隔离。[安全指南](./security) 说明执行责任，[Registry 记录](/reference/plugin-registry-v1) 说明宿主实现。

统一术语见仓库[词汇表](https://github.com/OdradekAI/velvet/blob/main/docs/GLOSSARY.md)。架构决策和实现证据从[维护者地图](/maintenance)进入。
