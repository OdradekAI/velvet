---
id: filter-facet-menu
title: Filter Facet Menu
sidebar_label: Filter Facet Menu
---

# VelvetFilterFacetMenu

`VelvetFilterFacetMenu` 是两类需要独立交互模型的筛选维度菜单的宿主投影：文件夹选择和颜色选择。文件夹层级、搜索结果、颜色匹配和持久化都留在 UI Kit 之外。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `kind`、`open`、`options`/`swatches`、`query`、`rule`、`selectedColor`、`graySelected`、`hexValue`、`accuracy`，以及对应宿主回调。
- **文件夹几何：** `360px` 表面、`40px` 页头、`26px` 选项行、OR/AND/EQUAL 标签页和 `40px` 页脚。
- **颜色几何：** `202px` 表面、`20px` 色块、`32px` 十六进制颜色输入框，以及 `5–40` 精度范围。
- **动作：** 文件夹 include/exclude、规则变更、查询变更、颜色预设、十六进制颜色、精度、清除和关闭。
- **所有权：** 宿主拥有已筛选选项、文件夹层级、颜色匹配、选中状态和持久化。
- **资格验证：** Node/Happy DOM story，以及宿主投影夹具中的真实浏览器指针、右键、焦点和 Escape 验证。

左键点击文件夹选项发出 `include`；上下文点击发出 `exclude`。组件不执行查询，也不推断文件夹锚点。颜色变更只向宿主报告选中值或精度。

<span id="故事与验证" className="velvet-doc-anchor" />

## Story {/* #stories */}

- `filter-facet-menu/folder-default`
- `filter-facet-menu/color-default`
- `filter-facet-menu/closed`

契约和 story 运行器覆盖观察到的几何尺寸、文件夹规则标签页、include/exclude 动作、颜色预设、十六进制颜色输入、精度范围和关闭状态。真实浏览器夹具还检查指针 include/exclude、菜单焦点进入、Escape 关闭和触发元素焦点恢复。

## 证据 {/* #evidence */}

此封装采用的模块级 SCSS、模板和行为证据，见被版本控制忽略的研究记录 `docs/research/eagle/eagle-analysis/component-style-evidence.md`。

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
```
