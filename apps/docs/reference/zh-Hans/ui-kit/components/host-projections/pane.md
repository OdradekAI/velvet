---
id: pane
title: Pane
sidebar_label: Pane
---

# VelvetPane

`VelvetPane` 是 Workbench zone 的语义容器。它提供标题、region 标记和 body slot，布局尺寸由宿主控制。

## 契约

- Props：`title`、`region`、`children`
- 状态：navigation、context
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/pane.contract.ts`
- Stories：`pane/navigation`、`pane/context`

组件使用带名称的 `region`，标题通过 `aria-labelledby` 关联。插件不能依赖内部 DOM 或 CSS class 来改变 zone 布局。

## 验证

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
