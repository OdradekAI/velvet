---
id: asset-grid
title: Asset Grid
sidebar_label: Asset Grid
---

# VelvetAssetGrid

`VelvetAssetGrid` 是支持键盘导航的资产布局投影组件，属于 Velvet 宿主投影层。实现位于 `apps/desktop/src/app/ui/host-projections/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

<span id="契约" className="velvet-doc-anchor" />
<span id="语义与无障碍" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `items`、`columns`、`layout`、`onSelect`、`onOpen`、`onDragStart`、`onDrop`
- **状态：** default、selected、disabled、empty、layout、dragging、pinned
- **动作：** select、open、navigate、drag、drop
- **可观测项：** grid-role、grid-metrics、selection、selection-modifier、focus、open-callback、layout、drag-and-drop、pinned、metadata
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/asset-grid.contract.ts`
- **Story：** `apps/desktop/src/app/ui/stories/host-projections/asset-grid.stories.tsx`
- **资格验证：** Node/Happy DOM 和真实浏览器两层
- **宿主边界：** 布局、浮层、焦点和投影状态仍由宿主拥有。

组件契约只规定语义。Radix 类型、私有 DOM 结构、Tailwind 类和 Workbench 实现细节不属于公开 API。

## 布局与拖放 {/* #layout-and-dragdrop */}

`layout` 接受 `grid`（默认）、`square`、`list`、`justified` 或 `waterfall`。条目可以提供 `aspectRatio`、`metadata` 和 `pinned`；这些值按宿主拥有的投影状态渲染。`metadata` 是 React 插槽，宿主可用它投影标签、分辨率、评分或扩展详情，而不把这些字段纳入网格选择状态。置顶状态提供无障碍标记，但不会隐式触发持久化动作。拖放需要显式启用：`onDragStart` 启用原生可拖动单元格并报告源 id；`onDrop` 独立启用放置处理，并报告类型化的源与目标 id。组件不会持久化顺序或读取传输状态。

`onSelect(id, additive, range)` 报告引发选择的指针修饰键。在已填充素材库的探测中，Shift-click 选中了第二个条目，同时保留两个单元格的选中状态。宿主拥有选择锚点，并决定如何应用 `range` 提示；网格从不存储第二套选择模型。

布局和拖放接口来自本地研究矩阵中记录的有限交互证据。懒加载和分页仍是宿主运行时的职责，有意排除在此组件公开 API 之外。

<span id="故事与验证" className="velvet-doc-anchor" />

## Story 与证据 {/* #stories-and-evidence */}

- `asset-grid/default`
- `asset-grid/selected`
- `asset-grid/grid-semantics`
- `asset-grid/layout-modes`
- `asset-grid/selection-modifiers`
- `asset-grid/zoom-layout`
- `asset-grid/waterfall-layout`

`asset-grid/zoom-layout` 是集成 story：`VelvetZoomControl` 保持独立，由宿主将其值映射为 `columns`。该 story 记录观察到的几何检查点：值为 `180` 时显示八列，值为 `230` 时显示五列；这一映射并不是 AssetGrid 的公开算法。

由于选择修饰键会报告给宿主，网格提供 `aria-multiselectable="true"`。对于 `grid`、`square` 和 `list` 布局，它还根据宿主投影的列数提供 `aria-rowcount`、`aria-colcount`、`aria-rowindex` 和 `aria-colindex`（`list` 模式固定为一列）。`justified` 和 `waterfall` 不设置这些指标，因为它们的响应式几何不构成稳定的公开行列契约。瀑布流保留各条目的宽高比，列测量则交给宿主布局界面。

语义视觉契约采用紧凑画廊样式，默认卡片缩略图保持 `4/3` 比例，选择状态使用 active 表面。契约只命名这些不变量，不暴露 CSS 选择器。

当前 story 运行器提供确定性夹具覆盖。对于浮层和宿主投影，修改焦点、portal、几何尺寸或宿主投影行为时，必须运行真实浏览器资格验证夹具。未经验证的外部行为仍标记为 `unknown`，直到运行时捕获提供证据。

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
