---
id: quick-search
title: Quick Search
sidebar_label: Quick Search
---

# VelvetQuickSearch

`VelvetQuickSearch` 是用于在文件夹、条目、标签和智能文件夹之间快速导航的宿主投影。它独立于 `VelvetCommandPalette`，因为它的结果模型和标签页范围用于导航投影，而非命令执行。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `open`、`tabs`、`activeTab`、`results`、`query`、`history`、`onQueryChange`、`onTabChange`、`onHistorySelect`、`onSelect`、`onClose`、`restoreFocusRef`
- **标签页：** `folders`、`items`、`tags`、`smart-folders`
- **结果：** `results` 已由宿主适配器确定范围、筛选并排序。组件渲染列表并报告选择。
- **历史：** `history` 已由宿主排序、去重并限制数量。组件渲染条目并报告历史选择。
- **键盘：** ArrowUp/ArrowDown、Home、End、Enter 和 Escape 操作结果列表；原生标签页按钮仍可通过键盘访问。
- **焦点：** 打开时进入输入框；关闭时恢复到提供的 `restoreFocusRef`。
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/quick-search.contract.ts`
- **Story：** `apps/desktop/src/app/ui/stories/host-projections/quick-search.stories.tsx`

组件从不读取素材库索引、解析查询语法、执行转写、对结果评分、去重历史、写入历史或调用 CoreTransport。Workbench 适配器准备结果和历史投影，并拥有查询、标签页、选择和持久化状态。省略 `query` 时，组件保留本地输入草稿，并继续通过 `onQueryChange` 报告变更；提供 `query` 后，该值由宿主控制。

## 证据边界 {/* #evidence-boundary */}

本地对已交付应用的研究记录了四种快速搜索模式、拼音与简繁匹配、评分、有界历史，以及方向键、Enter 和 Escape 行为，也记录了焦点进入与恢复。本 UI 契约保留观察到的视觉和键盘界面，将搜索语义、排序、索引和历史持久化留在 Workbench 边界。

## Story 与资格验证 {/* #stories-and-qualification */}

- `quick-search/open` 验证宿主提供的结果、活动后代和键盘选择。
- `quick-search/tabs-history` 验证四标签页投影以及宿主提供的历史选择。
- `quick-search/keyboard-focus` 验证输入框焦点进入、Escape 关闭和触发元素焦点恢复。

真实浏览器夹具还验证宿主结果替换、标签页切换、历史渲染与选择、方向键与 Enter 选择，以及触发元素焦点恢复。

<span id="键盘与验证" className="velvet-doc-anchor" />

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
