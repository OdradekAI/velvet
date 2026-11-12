---
id: activity-bar
title: Activity Bar
sidebar_label: Activity Bar
---

# VelvetActivityBar

`VelvetActivityBar` 是工作台导航栏的 Velvet 宿主投影组件。实现位于 `apps/desktop/src/app/ui/host-projections/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `items`、`onSelect`
- **状态：** selected、disabled、focus、badge（story：default、badges）
- **无障碍：** navigation 地标、移动式焦点；方向键、Home 和 End 改变选择时焦点随之移动，并跳过禁用条目
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/activity-bar.contract.ts`
- **Story：** `apps/desktop/src/app/ui/stories/host-projections/activity-bar.stories.tsx`
- **宿主边界：** 布局、浮层、焦点和投影状态仍由宿主拥有。

组件契约只规定语义。Radix 类型、私有 DOM 结构、Tailwind 类和 Workbench 实现细节不属于公开 API。

<span id="故事与无障碍" className="velvet-doc-anchor" />

## Story 与证据 {/* #stories-and-evidence */}

- `activity-bar/default`
- `activity-bar/badges`

默认场景覆盖 ArrowRight 导航、点击选择和焦点移动。真实浏览器宿主夹具验证禁用条目会被跳过，且新选中的条目获得焦点。

当前 story 运行器提供确定性夹具覆盖。对于浮层和宿主投影，修改焦点、portal、几何尺寸或宿主投影行为时，必须运行真实浏览器资格验证夹具。未经验证的外部行为仍标记为 `unknown`，直到运行时捕获提供证据。

<span id="所有权与验证" className="velvet-doc-anchor" />

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
