---
id: data-table
title: Data Table
sidebar_label: Data Table
---

# VelvetDataTable

`VelvetDataTable` 是可排序表格投影的 Velvet 宿主组件。实现位于 `apps/desktop/src/app/ui/host-projections/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `rows`、`columns`、`rowId`、`selectedId`、`sort`、`onSelect`、`onSort`、`emptyMessage`
- **状态：** default、selected、sortable、empty
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/data-table.contract.ts`
- **Story：** `default`、`selected`、`sort-direction`、`empty`
- **资格验证：** Node/Happy DOM 和真实浏览器两层
- **宿主边界：** 布局、浮层、焦点和投影状态仍由宿主拥有。

组件契约只规定语义。排序状态属于宿主：`onSort` 接收列 id 和下一个 `ascending`/`descending` 方向，受控 `sort` 属性则通过 `aria-sort` 投影已提交状态。没有行数据时渲染带标签的状态单元格，不创建第二个选择状态所有者。Radix 类型、私有 DOM 结构、Tailwind 类和 Workbench 实现细节不属于公开 API。

## Story 与证据 {/* #stories-and-evidence */}

- `data-table/default`
- `data-table/selected`
- `data-table/sort-direction`
- `data-table/empty`

当前 story 运行器提供确定性夹具覆盖。对于浮层和宿主投影，修改焦点、portal、几何尺寸或宿主投影行为时，必须运行真实浏览器资格验证夹具。未经验证的外部行为仍标记为 `unknown`，直到运行时捕获提供证据。

<span id="无障碍与验证" className="velvet-doc-anchor" />

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
