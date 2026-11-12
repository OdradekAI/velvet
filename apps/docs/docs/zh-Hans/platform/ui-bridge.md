---
id: ui-bridge
title: UI Bridge
---

# UI Bridge

Bridge 将隔离 surface 连接到宿主拥有的布局、焦点、overlay 和生命周期。它传递语义消息以及只读上下文、主题和 locale 投影，不暴露宿主 React 组件。

<span id="宿主--surface" className="velvet-doc-anchor" />
<span id="surface--宿主" className="velvet-doc-anchor" />

- [连接 surface](../guides/isolated-ui)：时序、示例与验证。
- [消息参考](/reference/ui-bridge-protocol-v1)：字段、版本、错误与销毁。
- [主题和组件契约](/reference/ui-kit-contract-v1)：语义 token 与协商。
- [安全边界](./security)：来源、会话和能力的执行责任。

当前契约是仓库内 Alpha。存在 adapter 不代表已经验证生产 SDK 或所有原生 WebView。
