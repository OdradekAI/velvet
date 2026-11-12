---
id: inspector
title: Inspector
sidebar_label: Inspector
---

# VelvetInspector

`VelvetInspector` 是宿主拥有的当前选择补充投影。它将零选中、单选和多选状态放在同一个语义界面中，选择、持久化和传输的所有权仍属于 Workbench 宿主。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `title`、`selectedCount`、`preview`、`fields`、`sections`、`palette`、`emptyMessage`、`onFieldChange`、`onSave`、`onRetry`、`saveState`、`saveError`、`pinned`、`onPin`、`onOpen`、`onSectionToggle`、`children`
- **状态：** empty、single、multiple、pinned、editing、saving、saved、error、cancelled、grouped、palette、preview-landscape、preview-portrait、no-preview
- **动作：** pin、open、edit、save、retry、toggle-section
- **插槽：** preview、metadata、sections、palette、actions、children
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/inspector.contract.ts`
- **Story：** `apps/desktop/src/app/ui/stories/host-projections/inspector.stories.tsx`
- **资格验证：** Node/Happy DOM 和真实浏览器两层

`selectedCount` 是宿主投影。组件不检查 Asset 存储，也不自行推导选择。多选时显示紧凑的选中数量。可编辑字段通过 `onFieldChange` 受控更新；保存状态和校验消息来自宿主。组件不会持久化编辑，也不会推断保存成功。`sections` 提供带标签、由宿主控制展开状态的字段分组；`palette` 将宿主预先计算的颜色比例渲染为紧凑的胶囊状色块，渲染器从不检查文件或计算颜色。预览区域最小高度为 200px，内容最大高度限制为 200px，并保留给定资产的宽高比。横向和纵向资产使用相同插槽；没有预览内容的资产省略该插槽，同时保留元数据。宿主可以渲染不含预览内容的多选摘要。

语义视觉契约预留紧凑的 250px Inspector 列、28px 可编辑控件以及 panel 表面上的分组区域。选择、字段值、持久化和保存生命周期仍由宿主拥有。

<span id="故事与验证" className="velvet-doc-anchor" />

## 证据 {/* #evidence */}

此结构参考已交付应用的 Inspector 证据：零选中、单选和多选显示不同字段集，名称和 URL 变更交由宿主处理器处理，单个条目的空名称会被拒绝。具体产品的编辑持久化和插件 Inspector 激活仍是独立宿主契约。已交付应用的证据记录位于 `docs/research/eagle/eagle-analysis/component-style-evidence.md`；其来源矩阵包括 `inspector.html`、`inspector-information.html` 和 `style/components/_inspector.scss`。已填充素材库的矩阵还在 `docs/research/eagle/ui-kit/populated-evidence.md` 中记录了单选、范围选择和 Inspector 选中数量分支。

资格验证夹具提供四个宿主拥有的场景，并使用稳定测试 ID：`inspector-scenario-landscape`、`inspector-scenario-portrait`、`inspector-scenario-no-preview` 和 `inspector-scenario-multiple`。Playwright 和 CUA 浏览器运行器会逐个断言选择模式、预览是否存在、宽高比、分组数量、调色板条目数和选中数量文本。

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
