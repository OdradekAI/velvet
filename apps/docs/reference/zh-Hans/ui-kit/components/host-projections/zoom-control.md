---
id: zoom-control
title: Zoom Control
sidebar_label: Zoom Control
---

# VelvetZoomControl

`VelvetZoomControl` 是受控的缩放投影。宿主拥有缩放值并决定如何映射到 Asset Grid，组件提供原生 range、增减按钮和 live value。

## 契约

- Props：`value`、`min`、`max`、`step`、`onChange`、`disabled`
- 状态：默认、disabled、最小值、最大值、键盘操作
- 动作：increment、decrement、change
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/zoom-control.contract.ts`
- Stories：`zoom-control/default`、`keyboard`、`bounds`、`disabled`

默认策略为 `min=25`、`max=500`、`step=5`，三个值都可以由宿主覆盖。组件不保存设置，也不直接调整网格。

## 无障碍与验证

组件使用原生 range 语义、命名 group、polite live value 和 `aria-valuetext`。修改步进、边界或键盘行为时运行：

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
