---
id: panel
title: Panel
sidebar_label: Panel
---

# VelvetPanel

`VelvetPanel` 是 Velvet 宿主投影层的界面容器。实现位于 `apps/desktop/src/app/ui/host-projections/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `title`、`surface`、`children`
- **状态：** panel、raised、overlay
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/panel.contract.ts`
- **Story：** `apps/desktop/src/app/ui/stories/host-projections/panel.stories.tsx`
- **宿主边界：** 布局、浮层、焦点和投影状态仍由宿主拥有。
- **无障碍：** 具名 `region`；名称来自 `ariaLabel`、提供 title 时关联的标题，或默认名称 `Panel`；表面状态显式指定

组件契约只规定语义。Radix 类型、私有 DOM 结构、Tailwind 类和 Workbench 实现细节不属于公开 API。

## Story 与证据 {/* #stories-and-evidence */}

- `panel/panel`
- `panel/raised`

Story 和浏览器夹具验证具名区域、标题关联、子内容和 raised 表面投影。

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
