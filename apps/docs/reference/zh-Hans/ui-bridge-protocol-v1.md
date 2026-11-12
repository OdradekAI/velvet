---
id: ui-bridge-protocol-v1
title: UI Bridge 消息协议 v1
---

# UI Bridge 消息协议 v1

读者：隔离 UI 作者与宿主 adapter 维护者。状态：**Alpha bridge 版本 `1`，组件契约 `1`**。Owner：`apps/desktop/src/workbench/plugins/ui-bridge.ts`；主题与 overlay DTO 位于相邻的 `ui-kit.ts`。这些源码类型解释边界，不是 SDK 导入路径。

## 消息 {/* #messages */}

每条消息都包含 `type`，问号表示可选字段。宿主 adapter 拥有传输封装、origin/source 校验和会话销毁。

{/* bridge-messages:start */}
| type | Fields besides type | Meaning |
| --- | --- | --- |
| `host.ready` | `bridgeVersion, componentContractVersion, surfaceId, theme, size, context; locale?` | 宿主初始投影与支持版本。 |
| `host.context` | `context` | 只读 WorkspaceContext 快照。 |
| `host.theme` | `theme` | 规范化后的语义主题快照。 |
| `host.locale` | `locale` | 有界 locale 投影。 |
| `host.resize` | `size` | width / height；devicePixelRatio 可选。 |
| `host.focus` | `focused` | 宿主焦点状态。 |
| `host.dispose` | `reason` | unmount、crash 或 incompatible；释放本地资源。 |
| `host.error` | `code, message` | 有界错误；不返回通用命令结果。 |
| `surface.ready` | `bridgeVersion; componentContractVersion?` | 报告兼容版本。 |
| `surface.focus-request` | `—` | 向宿主请求焦点。 |
| `surface.command` | `requestId, commandId, args` | 通过绑定的宿主处理器分发已注册命令。 |
| `surface.overlay-request` | `request: UiOverlayRequest` | 宿主拥有 overlay 生命周期和焦点。 |
| `surface.notification` | `request: UiOverlayRequest` | 请求宿主通知。 |
| `surface.progress` | `request: UiProgressRequest` | 投影进度；不创建或拥有 Job。 |
| `surface.close` | `—` | 以 unmount 原因销毁 surface。 |
| `surface.error` | `message` | 向宿主报告 surface 失败。 |

{/* bridge-messages:end */}

## 请求与错误 {/* #requests-and-errors */}

`UiOverlayRequest` 必须包含 `requestId` 和 `kind`，可选字段包括 `title`、`message`、`severity`、`anchor` 和 `actions`。完整允许字段以及 kind/anchor/action 规则由 Bridge 源码中的 `validateUiOverlayRequest` 校验，不能传入 React element 或任意 DOM。

`UiProgressRequest` 必须包含 `requestId`；可选 `value` 必须为 0 到 1 之间的有限数值，`message` 有长度限制，`state` 为 `running`、`completed`、`failed` 或 `cancelled`。进度消息不绕过 Rust Job 所有权。

Surface 消息拒绝未知字段和超过 64 KiB 的序列化 payload。ID 最长 256 字符，消息文字最长 4096，overlay action 最多 32 个。错误数据在宿主处理器执行前被拒绝；不能假设每条无效消息都会收到错误响应。

| 错误码 | 含义与恢复 |
| --- | --- |
| `bridge-version-mismatch` | Bridge 不兼容；宿主标记失败并销毁 surface |
| `component-contract-version-mismatch` | 组件版本不支持；宿主标记失败并销毁 surface |
| `command-handler-unavailable` | Surface 未绑定命令处理器 |
| `command-failed` | 宿主处理器拒绝；不盲目重放副作用 |
| `overlay-handler-unavailable` / `notification-handler-unavailable` | 请求的宿主 adapter 不存在 |
| `overlay-request-failed` / `notification-request-failed` / `progress-request-failed` | 宿主请求失败；保留有界诊断 |

版本 1 没有为 `surface.command` 定义通用成功 ack、结果 payload 或取消消息；其 `requestId` 不是幂等键。Job 取消通过授权的 Job 边界执行，不能靠 `surface.progress` 取消。

## 生命周期与安全 {/* #lifecycle-and-security */}

宿主发送 `host.ready`，surface 报告 `surface.ready`。之后的 context、theme、locale 和 size 消息都是投影。销毁时取消订阅并释放资源；宿主重试创建新会话，不重放旧消息。

浏览器 adapter 校验 source window，在存在网络 origin 时精确匹配来源；opaque origin 不获得隐式权限。Tauri child WebView 使用明确的 capability 和清理路径。Adapter 边界与运行时沙箱验证应分开理解。[连接 surface](/guides/isolated-ui) 介绍流程，[UI 契约](ui-kit-contract-v1.md)定义语义主题规则。

## 证据 {/* #evidence */}

`apps/desktop/test/ui-kit-contract.test.mjs` 与 `apps/desktop/test/plugin-surface-adapter.test.mjs` 验证消息、协商、来源和销毁。先完成[测试准备](/platform/testing)。原生 OS/WebView 与无障碍仍是单独的[发布门禁](plugin-ecosystem-qualification.md)。
