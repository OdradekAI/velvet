---
id: panel
title: Panel
sidebar_label: Panel
---

# VelvetPanel

`VelvetPanel` 是宿主拥有的 panel surface。它渲染标题、surface 状态和 body，不负责 Workbench 布局或持久化尺寸。

## 契约

- Props：`title`、`surface`、`children`
- 状态：panel、raised
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/panel.contract.ts`
- Stories：`panel/panel`、`panel/raised`

Panel 提供带名称的 `region` 和关联标题，并通过显式 surface 状态表达层级。插件依赖语义属性，不依赖私有 CSS。

## 验证

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
