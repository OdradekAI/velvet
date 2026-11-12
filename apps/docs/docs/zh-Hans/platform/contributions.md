---
id: contributions
title: 插件贡献
sidebar_label: 插件贡献
---

# 插件贡献

Contribution 是插件向 Workbench、WorkbenchApp 或 capability namespace 注册的能力。每个 contribution 都声明 scope、namespace、target app、container、zone、activation 和所需权限。

## 范围

- `workbench` 负责全局 shell、Overlay、Command Palette、CompanionView 和 OperationsView。
- `workbench-app` 负责指定 WorkbenchApp 的 AppSurface、ViewContainer、View、toolbar 和 inspector。
- `capability` 负责 Source、Preview、Processor、Job、AssetFacet 等可复用能力。

## 宿主关系

插件声明目标位置，宿主拥有容器和布局。ContributionRegistry 在激活前验证 scope、kind、namespace、zone、container 和 permission，失败的 extension 只会失去自己的 projection。

详细字段见 [Manifest reference](/reference/plugin-manifest-v1)，跨层证据见 [Contract matrix](/reference/plugin-contract-matrix)。
