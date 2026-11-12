---
id: security
title: 安全边界
---

# 安全边界

<span id="安全" className="velvet-doc-anchor" />

产品扩展与 Tauri host plugin 跨越不同边界。贡献的 UI zone 不授予文件系统、网络、进程或元数据写入权限。Manifest 能力只是请求，有效授权由宿主策略决定。

<span id="保护边界" className="velvet-doc-anchor" />

| 边界 | 执行 owner | 扩展责任 |
| --- | --- | --- |
| 包与身份 | PluginHost 校验 schema、兼容性、包路径约束和信任策略 | 保持 ID 稳定；开发模式不代表额外权限 |
| 运行时到宿主 | Runtime adapter 绑定 caller 身份和 nonce；TauriTransport 校验会话；ApplicationCore 分发授权操作 | 不伪造 caller、不复用已撤销会话、不盲目重试结果未知的修改 |
| UI surface 到宿主 | Surface adapter 校验 source/origin；Bridge 校验消息形状和大小；处理器校验命令及能力 | 发送声明过的消息，销毁监听器，接受有界投影 |
| Jobs 与扩展数据 | JobSystem 和 PluginHost 执行所有权、配额、provenance 和迁移规则 | 取消自己的工作，拒绝过期结果，保持写入命名空间隔离 |
| 诊断 | 宿主限制大小、脱敏并控制导出 | 不发送秘密、原始路径和 Asset payload |

进程崩溃隔离与安全沙箱不同。Sidecar fixture 和 manifest 中的 `wasm` 值不能证明生产隔离。Tauri capabilities 控制它的 WebView 到 core 暴露面，不约束任意 Rust 代码或外部进程的操作系统访问。

宿主拥有 overlay、焦点恢复、布局和 fallback。错误、禁用、升级和卸载必须保持核心 Asset 状态及其他扩展可用。详见[恢复流程](/reference/plugin-developer-guide#permissions-errors-and-recovery)和[验证证据](/reference/plugin-ecosystem-qualification)；后者记录尚未完成的发布门禁，不承诺保护已经全部完成。
