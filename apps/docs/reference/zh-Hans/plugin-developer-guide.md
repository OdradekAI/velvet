---
id: plugin-developer-guide
title: Velvet 插件开发者指南
---

# Velvet 插件开发者指南

本页描述当前 Alpha contract 的可用开发路径。插件通过 manifest、contribution、capability 和 UI surface 扩展 Workbench。生产 Marketplace、外部 SDK、签名服务和 WASM/sidecar runtime 仍属于后续 qualification gate。

## 开发路径

1. 为插件定义稳定的 extension id 和 manifest version。
2. 声明 contribution、target app、container、zone、activation 和 capability。
3. 选择 View、Command、Preview、Job 或 UI surface。
4. 通过宿主 registry 和 typed transport 访问能力。
5. 使用 schema、story、browser 和 failure-recovery evidence 验证插件。

## 当前 fixture

- `examples/plugins/asset-inspector/` 覆盖 View、context command、Preview augmentation、Job 和 namespaced facet。
- `examples/plugins/echo-sidecar/` 覆盖 sidecar JSONL、UI Bridge、Preview action 和独立崩溃恢复。

## UI 选择

- first-party extension 可以使用 Velvet React wrapper。
- third-party extension 使用 framework-neutral UI Bridge，在自己的 React、Vue、Svelte 或 HTML surface 中渲染。
- 只需要宿主交互时，直接贡献 Command、Menu、Notification、Progress 或 Host projection。

## 失败隔离

无效 manifest、权限不足、版本不兼容、超时、崩溃、disable 和 uninstall 都应只影响当前 extension。核心 Asset、Layout、Preview、Job 和 generic fallback 必须保持可用。

详细字段见 [Plugin manifest](/reference/plugin-manifest-v1)，跨层检查见 [Contract matrix](/reference/plugin-contract-matrix)，生态门禁见 [Qualification gate](/reference/plugin-ecosystem-qualification)。
