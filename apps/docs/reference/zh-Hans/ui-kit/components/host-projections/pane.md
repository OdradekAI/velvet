---
id: pane
title: Pane
sidebar_label: Pane
---

# VelvetPane

`VelvetPane` 是工作台语义区域容器的 Velvet 宿主投影组件。实现位于 `apps/desktop/src/app/ui/host-projections/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `title`、`region`、`children`
- **状态：** navigation、context、operations、preview
- **无障碍：** 具名 `region`；提供 `ariaLabel` 时使用该名称，否则通过 `aria-labelledby` 关联标题
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/pane.contract.ts`
- **Story：** `apps/desktop/src/app/ui/stories/host-projections/pane.stories.tsx`
- **宿主边界：** 布局、浮层、焦点和投影状态仍由宿主拥有。

组件契约只规定语义。Radix 类型、私有 DOM 结构、Tailwind 类和 Workbench 实现细节不属于公开 API。

## Story 与证据 {/* #stories-and-evidence */}

- `pane/navigation`
- `pane/context`

导航 story 和浏览器夹具验证 region 角色、标题关联、区域标记和正文内容。

当前 story 运行器提供确定性夹具覆盖。对于浮层和宿主投影，修改焦点、portal、几何尺寸或宿主投影行为时，必须运行真实浏览器资格验证夹具。未经验证的外部行为仍标记为 `unknown`，直到运行时捕获提供证据。

<span id="验证" className="velvet-doc-anchor" />

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
