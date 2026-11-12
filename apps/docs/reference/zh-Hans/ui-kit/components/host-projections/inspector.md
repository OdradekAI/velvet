---
id: inspector
title: Inspector
sidebar_label: Inspector
---

# VelvetInspector

`VelvetInspector` 是当前选择的宿主补充投影。它统一承载零个、一个和多个选择项的状态，并把编辑、保存和重试交回 Workbench。

## 契约

- Props：`title`、`selectedCount`、`preview`、`fields`、`sections`、`palette`、`emptyMessage`、`onFieldChange`、`onSave`、`onRetry`、`saveState`、`saveError`、`pinned`、`onPin`、`onOpen`、`onSectionToggle`、`children`
- 状态：empty、single、multiple、pinned、editing、saving、saved、error、cancelled、grouped、palette、preview-landscape、preview-portrait、no-preview
- 动作：pin、open、edit、save、retry、toggle-section
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/inspector.contract.ts`

`selectedCount`、字段值、保存状态和预览内容都是宿主投影。组件不访问 Asset store、不持久化编辑，也不推导保存结果。预览区域保持给定的宽高比，无预览时仍保留元数据。

## 故事与验证

Stories 覆盖 landscape、portrait、no-preview、multiple selection、palette、分组和保存错误。浏览器 fixture 验证 pin、edit、save、retry、section collapse 和 selected-count announcement。

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
