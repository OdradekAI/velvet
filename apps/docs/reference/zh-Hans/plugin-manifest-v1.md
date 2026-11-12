---
id: plugin-manifest-v1
title: Velvet 插件 manifest 与 CoreTransport contract v1
---

# Velvet 插件 manifest 与 CoreTransport contract v1

manifest 是插件平台在激活前校验的声明。它定义 extension identity、host API range、contributions、permissions、runtime 和 UI surface contract。

## 必要字段

| 字段 | 作用 |
| --- | --- |
| `id` | 稳定的 extension identity |
| `version` | 插件版本和升级判断 |
| `hostApi` | 支持的宿主 API range |
| `contributions` | View、Command、Preview、Job 等声明 |
| `permissions` | capability grant 的最小集合 |
| `runtime` | in-process、WASM 或 sidecar 运行边界 |
| `ui` | surface、entrypoint、bridge 和 component contract |

## 校验规则

宿主在 activation 前校验 id、schema、API range、contribution id、zone、scope、permission 和 UI contract version。未知 required field、无效 capability、非法 zone 和版本不匹配都会 fail closed。

## CoreTransport

插件请求通过 typed request、query、event、session 和 resync 语义进入 `CoreTransport`。插件不直接访问数据库、文件系统、插件 registry、React component 或 Tauri command。

## 兼容性

新增 optional field 可以保持同一 major。改变 required field、message meaning、state owner、permission 或 keyboard model，需要新的 contract decision、fixture 和 migration evidence。
