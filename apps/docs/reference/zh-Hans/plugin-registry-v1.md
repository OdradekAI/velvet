---
id: plugin-registry-v1
title: 插件 Registry v1
---

# 插件 Registry v1

Plugin registry 是宿主拥有的 contribution、View、Preview、Command 和 Job 注册边界。插件通过 registry 声明能力，宿主负责校验、激活、禁用、dispose 和恢复。

## 注册顺序

1. 读取 manifest 并验证 schema。
2. 检查 host API range、permission、scope、zone 和 namespace。
3. 写入 registry 的稳定 contribution id。
4. 在首次使用时 lazy activation。
5. 记录 failure、retry、disable 或 uninstall 状态。

## 所有权

registry 不把宿主布局、Asset identity、Layout、Preview fallback 或 Job state 转交给插件。插件只拥有自己的 namespaced facet、运行状态和 contribution projection。

## 失败处理

注册冲突、超时、崩溃、不兼容和撤销只会使当前 extension 失效。宿主仍保留核心状态和 generic fallback，并提供诊断信息。
