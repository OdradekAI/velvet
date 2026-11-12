---
id: commands-and-views
title: 增加 Commands 与 Views
---

# 增加 Commands 与 Views

从可运行的 [Echo 插件包](https://github.com/OdradekAI/velvet/tree/main/examples/plugins/echo-sidecar)开始。本指南解释它的 Alpha 声明路径，不提供独立 SDK 生成器。

## 声明贡献

示例 `manifest.json` 将 `echo.container` 声明为 `view-container`，将 `echo.view` 声明为 `view`。两者都面向 `asset-manager`，使用 `workbench-app` scope 和 `navigation` zone。`viewContainerId` 连接 View 与容器；View 的 `ui` 字段声明 surface 入口和 `bridgeVersion`。

`echo.open` 是 `command`。`context-menu` 贡献 `echo.menu` 通过 `commandId` 引用它，并使用 `asset` 上下文锚点。菜单只暴露操作入口，宿主仍检查命令能否执行。修改字段前阅读 [Manifest 参考](/reference/plugin-manifest-v1)。

## 运行处理器

在仓库根目录运行：

```sh
printf '%s\n' '{"protocolVersion":1,"kind":"invoke-command","extensionId":"velvet.example.echo","commandId":"echo.open","args":{"source":"manual"},"context":{"activeAppId":null,"keys":{}}}' | node examples/plugins/echo-sidecar/sidecar.mjs
```

进程输出 `echo.open` 的协议 JSONL，并在 stdin 关闭后退出。这只验证 sidecar 处理器，未经过宿主准入、权限检查和 UI 挂载。stdout 留给协议记录，stderr 用于有界诊断。

## 挂载与清理

宿主注册和激活决定 View 是否出现。View 在声明的 zone 中提供内容，不能修改布局或挂载其他 View 的实现。禁用、崩溃和卸载后，宿主移除贡献；恢复时创建新会话。

完成[测试准备](../platform/testing)后运行：

```sh
node apps/desktop/test/external-plugin-e2e.test.mjs
node apps/desktop/test/plugin-contribution-lifecycle.test.mjs
```

这些测试覆盖本地宿主与 adapter 边界。包信任和原生平台发布验证仍单独进行。继续阅读[隔离 UI](./isolated-ui)或[宿主 Registry 记录](/reference/plugin-registry-v1)。
