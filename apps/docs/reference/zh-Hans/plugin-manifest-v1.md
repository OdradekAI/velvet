---
id: plugin-manifest-v1
title: 插件 Manifest 与传输契约（schema 1）
---

# 插件 Manifest 与传输契约（schema 1）

<span id="velvet-插件-manifest-与-coretransport-contract-v1" className="velvet-doc-anchor" />

状态：**仓库内维护的 Alpha 契约**，不是已发布的外部 SDK。

Owner：`crates/plugin-api/src/lib.rs` 的 `ExtensionManifest` / `RawExtensionManifest`。消费者是 PluginHost 准入和 transport adapter。宿主独占权限授权、激活和包状态的所有权。

<span id="必要字段" className="velvet-doc-anchor" />

## Manifest schema 1 {/* #manifest-schema-1 */}

JSON 字段名使用 camelCase。`required` 表示反序列化必填；`optional` 表示可以省略或为 null。运行时和贡献校验可能施加额外要求。必填数组即使为空也必须提供。

{/* manifest-fields:start */}
| Field | Presence | Meaning |
| --- | --- | --- |
| `manifestSchema` | `required` | 数字 schema，当前为 `1`。 |
| `id` | `required` | 稳定的扩展身份。 |
| `version` | `required` | 数字包版本；升级时保留身份。 |
| `hostApi` | `required` | 宿主 API 范围。 |
| `bridgeApi` | `required` | UI Bridge API 范围。 |
| `runtimeProtocol` | `required` | 外部运行时协议范围。 |
| `runtime` | `required` | `built-in`、`wasm` 或 `sidecar`；声明不代表运行时已通过验证。 |
| `extensionKind` | `required` | 由 `ui`、`workspace`、`capability` 组成的数组。 |
| `launch` | `optional` | 启动描述；运行时准入策略可能要求提供。 |
| `activationEvents` | `required` | 激活选择器；引用的本地 command/View 必须存在。 |
| `extensionDependencies` | `required` | 稳定扩展 ID 数组。 |
| `targetApp` | `optional` | WorkbenchApp 目标及 API 范围。 |
| `support` | `required` | 信任、虚拟 Workspace 和 Profile 支持声明。 |
| `requiredCapabilities` | `required` | 必要能力名称；授权仍由宿主策略决定。 |
| `optionalCapabilities` | `required` | 可选能力名称；数组可以为空。 |
| `localization` | `optional` | 相对包路径的本地化声明。 |
| `storage` | `optional` | 扩展存储 schema 与配额。 |
| `workbenchApp` | `optional` | 顶层产品应用描述。 |
| `contributions` | `required` | 具有唯一局部 ID 的贡献描述。 |
| `permissionSets` | `required` | 按 zone 划分的权限集合；`permissions` 位于此对象内部。 |

{/* manifest-fields:end */}

`ui` 位于贡献描述中，不是 manifest 顶层字段。`permissionSets[].permissions` 与能力名称数组不同。应复制完整的 [Echo manifest](https://github.com/OdradekAI/velvet/blob/main/examples/plugins/echo-sidecar/manifest.json)，不能将此表视为完整插件包。

除顶层 `x-*` 元数据外，未知字段均被拒绝。宿主不把元数据解释为能力。`launch` 在 JSON 中可选，但运行时准入仍可能要求提供。`built-in` 是可信组合路径，不是第三方绕过隔离的入口。

<span id="校验规则" className="velvet-doc-anchor" />

## 标识符与升级规则 {/* #identifier-and-upgrade-rules */}

- 扩展和贡献 ID 符合 `[A-Za-z0-9][A-Za-z0-9._-]*`，最长 128 字节。
- 贡献 ID 在扩展内唯一。命名空间权威由宿主派生，包不能占用其他 owner 的命名空间。
- 升级保留扩展 ID 并提高数字版本；新 ID 代表不同安装。
- 重复依赖、不存在的激活引用、错误范围、未知能力和重复能力声明均被拒绝。
- Alpha API 范围接受 `*`、精确数字版本和 `^1` 等 caret major。
- Scope、zone、target app 和有效授权仍由 PluginHost 决定。

<span id="coretransport" className="velvet-doc-anchor" />

## Plugin CoreTransport envelope {/* #plugin-coretransport-envelope */}

宿主侧 `PluginRequestEnvelope` 包含 `protocolVersion`、`sessionId`、`callerExtensionId`、`runtimeNonce`、`requestId` 和 `message`。Runtime adapter 绑定 caller 身份与宿主签发的 lease nonce。这与快速开始中 sidecar 的 JSONL payload 不同。

```json
{
  "protocolVersion": 1,
  "sessionId": "opaque-host-session",
  "callerExtensionId": "publisher.reader",
  "runtimeNonce": "host-issued-runtime-lease-nonce",
  "requestId": 7,
  "message": {
    "kind": "query-projection",
    "extensionId": "publisher.reader"
  }
}
```

这些占位值不能授权请求。Adapter 校验活动 lease 与 session；目标扩展必须等于绑定的 caller，`requestId` 必须非零。所有当前宿主请求种类及响应变体见 [API 索引](plugin-api-index.md)。

命令调用不能靠 request ID 重放，投影查询可重复。传输失败可能让修改结果处于未知状态，重试前应核对权威状态。Envelope 不授予数据库、原始路径、Tauri state 对象或任意进程访问权。

<span id="兼容性" className="velvet-doc-anchor" />

## 兼容性策略 {/* #compatibility-policy */}

`manifestSchema`、`hostApi`、`bridgeApi` 和 `runtimeProtocol` 分别协商。当前宿主接受 schema/API major `1`，不支持的范围按 fail-closed 拒绝。新增可选字段可以保持 major，改变必填字段、消息含义或所有权需要兼容性决策和 fixture。

重新连接创建新会话，不重放旧事件和副作用。ADR 状态描述设计决策，实现与平台验证在[契约矩阵](plugin-contract-matrix.md)中单独记录。

## 契约 fixture {/* #contract-fixtures */}

- Manifest 解析与校验：`crates/plugin-api/src/lib.rs`。
- Envelope 与请求转换：`crates/workbench-protocol/src/protocol_tests.rs`。
- Lease/session 校验：`crates/tauri-adapter/src/transport_tests.rs`。
- Tauri 映射：`apps/desktop/src-tauri/src/lib.rs` 测试。
- 桌面镜像：`apps/desktop/src/platform/protocol.ts`。

这些检查只证明对应边界，不证明生产 WASM 运行时、市场或 SDK 包。继续阅读[开发流程](plugin-developer-guide.md)和[生态门禁](plugin-ecosystem-qualification.md)。
