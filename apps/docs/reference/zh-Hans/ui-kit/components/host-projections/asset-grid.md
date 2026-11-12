---
id: asset-grid
title: Asset Grid
sidebar_label: Asset Grid
---

# VelvetAssetGrid

`VelvetAssetGrid` 是可键盘操作的 Asset projection。它渲染宿主准备好的资产列表，并把选择、打开和拖放动作回传给宿主。

## 契约

- Props：`items`、`columns`、`layout`、`onSelect`、`onOpen`、`onDragStart`、`onDrop`
- 状态：默认、选中、disabled、空列表、布局、拖拽、置顶
- 动作：select、open、navigate、drag、drop
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/asset-grid.contract.ts`
- Stories：`apps/desktop/src/app/ui/stories/host-projections/asset-grid.stories.tsx`

`layout` 支持 `grid`、`square`、`list`、`justified` 和 `waterfall`。`onSelect(id, additive, range)` 报告指针修饰键，选择锚点和持久化由宿主处理。组件不读取索引、不分页，也不保存排序。

## 语义与无障碍

`grid`、`square` 和 `list` 会提供稳定的 row/column 语义和多选标记。`justified` 与 `waterfall` 不承诺固定行列指标。置顶状态提供可访问标记，拖放只有在同时提供对应回调时才启用。

## 故事与验证

覆盖 `asset-grid/default`、`selected`、`grid-semantics`、`layout-modes`、`selection-modifiers`、`zoom-layout` 和 `waterfall-layout`。修改网格几何、键盘选择或拖放时运行：

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
