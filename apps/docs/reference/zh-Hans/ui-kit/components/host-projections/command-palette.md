---
id: command-palette
title: Command Palette
sidebar_label: Command Palette
---

# VelvetCommandPalette

`VelvetCommandPalette` 是宿主拥有的命令搜索 Overlay。宿主负责查询、排序和执行命令，组件负责输入、键盘导航和结果投影。

## 契约

- Props：`open`、`commands`、`scope`、`scopeOptions`、`onScopeChange`、`query`、`onQueryChange`、`onSelect`、`onClose`、`restoreFocusRef`
- 状态：打开、空结果
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/command-palette.contract.ts`
- Stories：`apps/desktop/src/app/ui/stories/host-projections/command-palette.stories.tsx`

省略 `query` 时组件保留本地草稿。传入 `query` 后，宿主必须通过 `onQueryChange` 提交新值。`commands` 已经是当前 scope 的结果投影，组件不会访问索引或解析查询语法。

## 故事与无障碍

覆盖 `open`、`empty`、`controlled-query` 和 `host-results`。结果列表支持 Arrow、Home、End、Enter 和 Escape，输入使用 active descendant，关闭时可通过 `restoreFocusRef` 恢复触发元素焦点。

## 验证

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
