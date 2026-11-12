---
id: ui-kit-contract-v1
title: Velvet UI Kit contract v1
---

# Velvet UI Kit contract v1

状态：Alpha host contract 已冻结在仓库内。本页是中文阅读层，完整实现证据仍来自桌面端源码、fixture 和 [UI Kit 组件指南](ui-kit-component-guide.md)。

## 版本面

| 面 | 当前版本 | 兼容规则 |
| --- | ---: | --- |
| UI Bridge message | `1` | 版本不匹配时在激活前拒绝 surface |
| Theme schema | `velvet.ui.theme/1` | 旧 snapshot 会先规范化，未知版本 fail closed |
| Component contract | `1` | 宿主公布版本 `1`，不兼容的 surface 会独立失败并 dispose |

宿主拥有布局、Activity Bar、Pane、PreviewHost、OperationsPanel、全局 Overlay、焦点恢复和全局快捷键。插件 surface 只接收 projection 和 typed action，不会拿到 React element、DOM、宿主 CSS class、文件路径或其他 View 的实现。

## Theme snapshot

宿主发送只读的 `velvet.ui.theme/1` snapshot，包含 scheme、density、contrast、reducedMotion 和 semantic token。插件应消费 token 语义，不应依赖宿主 CSS 变量名称或 Tailwind build。

## Surface lifecycle

surface 在 `host.ready` 后读取 context、theme、locale 和 size。宿主可以发送 `host.focus`、`host.resize`、`host.dispose` 或 `host.error`。插件通过 `surface.ready`、command、notification、progress、close 和 error message 响应。

## Failure boundary

origin、entrypoint、schema、version、token 和 payload size 在边界校验。插件崩溃、超时、disable、uninstall 或不兼容不会让 Asset、Layout、Preview、Job 或 UI fallback 失效。

## 继续阅读

- [UI Kit 组件指南](ui-kit-component-guide.md)
- [UI Bridge](/platform/ui-bridge)
- [UI Kit contract overview](/ui-kit)
