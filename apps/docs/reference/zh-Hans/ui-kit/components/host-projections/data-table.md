---
id: data-table
title: Data Table
sidebar_label: Data Table
---

# VelvetDataTable

`VelvetDataTable` 是可排序的数据表投影。行数据、选中项和排序状态由宿主控制，组件只负责渲染和报告动作。

## 契约

- Props：`rows`、`columns`、`selectedId`、`sort`、`onSelect`、`onSort`、`emptyMessage`
- 状态：默认、选中、可排序、空列表
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/data-table.contract.ts`
- Stories：`default`、`selected`、`sort-direction`、`empty`

`onSort` 报告列 id 和下一方向，受控 `sort` 会映射到 `aria-sort`。空状态使用带标签的 status cell，不创建第二个选中状态 owner。

## 无障碍与验证

键盘选择、排序方向和 `aria-sort` 由 host-projection fixture 验证。修改行语义或排序 contract 时运行：

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
