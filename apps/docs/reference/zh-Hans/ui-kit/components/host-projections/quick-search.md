---
id: quick-search
title: Quick Search
sidebar_label: Quick Search
---

# VelvetQuickSearch

`VelvetQuickSearch` 是跨文件夹、项目、标签和 smart folder 的快速导航投影。它与 Command Palette 分离，结果模型和 tab scope 属于导航而不是命令执行。

## 契约

- Props：`open`、`tabs`、`activeTab`、`results`、`query`、`history`、`onQueryChange`、`onTabChange`、`onHistorySelect`、`onSelect`、`onClose`、`restoreFocusRef`
- Tabs：`folders`、`items`、`tags`、`smart-folders`
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/quick-search.contract.ts`
- Stories：`apps/desktop/src/app/ui/stories/host-projections/quick-search.stories.tsx`

`results` 和 `history` 已由宿主过滤、排序、去重和限制数量。组件不读取索引、不解析查询、不执行转写、不写历史，也不调用 CoreTransport。

## 键盘与验证

ArrowUp、ArrowDown、Home、End、Enter 和 Escape 作用于结果列表。打开时进入输入框，关闭时通过 `restoreFocusRef` 恢复触发元素焦点。Stories 覆盖 open、tabs/history 和 keyboard/focus。

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
