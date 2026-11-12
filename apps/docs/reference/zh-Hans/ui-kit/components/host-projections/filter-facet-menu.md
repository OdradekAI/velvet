---
id: filter-facet-menu
title: Filter Facet Menu
sidebar_label: Filter Facet Menu
---

# VelvetFilterFacetMenu

`VelvetFilterFacetMenu` 是文件夹和颜色筛选所使用的宿主投影。它只渲染筛选控件，把层级、匹配和持久化交给宿主。

## 契约

- Props：`kind`、`open`、`options` 或 `swatches`、`query`、`rule`、`selectedColor`、`graySelected`、`hexValue`、`accuracy` 及对应回调
- 动作：文件夹 include/exclude、规则变化、查询变化、颜色预设、hex、accuracy、clear、close
- 所有权：宿主拥有选项、层级、匹配、选中状态和持久化

文件夹选项左键报告 `include`，右键报告 `exclude`。颜色变化只报告值或精度，不执行查询。

## 故事与验证

覆盖 `folder-default`、`color-default` 和 `closed`。浏览器 fixture 验证指针 include/exclude、焦点进入、Escape 关闭和触发元素焦点恢复。

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
