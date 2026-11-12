---
id: contributions
title: 插件贡献
sidebar_label: 插件贡献
---

# 插件贡献

贡献（Contribution）声明扩展向工作台、WorkbenchApp 或可复用能力命名空间提供的能力。每个描述必须包含 `id`、`kind` 和 `scope`。`namespace`、`container` 和 `zone` 等字段在序列化形状中可选，宿主再根据 kind 和 scope 校验哪些字段必须提供、哪些允许使用。权威命名空间由宿主派生。

## 范围

- `workbench` 覆盖全局 shell 和跨应用贡献。
- `workbench-app` 面向指定 WorkbenchApp 及其兼容表面。
- `capability` 覆盖 Preview provider、Job 等可复用能力。

Scope 描述贡献的适用范围，不把布局、核心数据或权限的所有权转交给扩展。规划中的 Source、Processor 抽象不是 schema 1 中额外的序列化贡献种类。

## 宿主关系

扩展请求放置位置，宿主拥有容器和布局。Manifest 层的 `activationEvents`、`requiredCapabilities`、`optionalCapabilities` 和 `permissionSets` 描述激活与权限请求。宿主在激活前校验 kind、scope、namespace、zone、container 和有效授权。扩展失败时只移除它自己的投影，其他扩展仍然可用。

字段见 [Manifest 参考](/reference/plugin-manifest-v1)，跨层证据见[契约矩阵](/reference/plugin-contract-matrix)。
