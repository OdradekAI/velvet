---
id: isolated-ui
title: 连接隔离 UI surface
---

# 连接隔离 UI surface

当前仓库实现了 iframe 和 Tauri child-WebView adapter，消息契约处于 Alpha；尚未发布框架 SDK，也未验证所有原生 surface。先阅读 Echo 包的 `contributions[].ui` 声明与[协议参考](/reference/ui-bridge-protocol-v1)。

## Surface 时序

1. 声明 View 或 AppSurface、scope/zone、入口及 bridge 版本。
2. 宿主校验声明并挂载 adapter，发送带上下文、尺寸、主题和组件版本的 `host.ready`。
3. Surface 发送 `surface.ready`，报告兼容的 bridge 版本。
4. 将 `host.context`、`host.theme`、`host.locale`、`host.resize` 作为投影应用；它们不授予数据访问权。
5. 使用 `surface.command` 请求已注册操作，由宿主处理器授权和分发。
6. 收到 `host.dispose` 后移除监听器和本地资源；重试使用新的宿主会话。

语义命令消息形状如下：

```json
{"type":"surface.command","requestId":"inspect-1","commandId":"echo.open","args":{}}
```

这只是消息 payload，不是完整 `postMessage` bootstrap 或生产安装流程。来源和 source 校验、传输封装、会话所有权由宿主 adapter 管理。版本 1 没有通用的命令结果或 ack 消息；`requestId` 不构成重放或去重保证。

Echo 的 `data:` UI 是 fixture 便利设施。生产入口必须满足实际来源、资源、CSP 和能力策略。Sidecar 进程提供崩溃边界，不自动构成操作系统安全沙箱。详见[安全指南](../platform/security)。

## 验证

完成[测试准备](../platform/testing)后运行：

```sh
node apps/desktop/test/ui-kit-contract.test.mjs
node apps/desktop/test/plugin-surface-adapter.test.mjs
```

这些 fixture 覆盖消息校验、主题与组件协商、来源处理和销毁。原生 WebView 与无障碍验证仍在[维护者证据地图](/reference/plugin-contract-matrix)中追踪。
