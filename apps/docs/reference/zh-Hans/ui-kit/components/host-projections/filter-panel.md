---
id: filter-panel
title: Filter Panel
sidebar_label: Filter Panel
---

# VelvetFilterPanel

`VelvetFilterPanel` 是宿主拥有的可组合查询筛选维度投影。宿主拥有 `rules` 数组；组件只渲染控件，并通过 `onRuleChange` 报告新规则集。

<span id="契约" className="velvet-doc-anchor" />

首个实现切片支持以下筛选维度类型：

- `text`：子串式输入（`contain` 运算符）。
- `single`：单个选项（`equal` 运算符）。
- `multi`：多个选项（`intersection` 运算符）。
- `range`：宿主定义的范围值（`between` 运算符）。

语义筛选维度可附加稳定 `semantic` id：`folder`、`tag`、`color`、`rating` 或 `date`。默认运算符映射为 `folder → equal`、`tag (multi) → intersection`、`color → equal`、`rating (range) → between` 和 `date (range) → within`。宿主需要 `before`、`after`、`not` 或 `or` 时，可在筛选维度中显式提供 `operator`；组件仅报告该运算符，从不自行执行查询。

规则采用类型化运算符集合：`contain`、`equal`、`intersection`、`between`、`before`、`after`、`within`、`not` 和 `or`。面板不解释这些运算符，只向宿主报告规则结构。`resultCount` 是会被播报的宿主投影，`onApply` 提交当前规则，`onSave` 通过宿主持久化命名筛选。

组件不读取数据库、执行搜索或推断结果数量。每个选项的数量由宿主提供。`onReset`、`onSave` 和 `onClose` 都是语义宿主动作。

```tsx
const facets = [
  { id: 'type', label: 'Type', kind: 'multi', options: [{ id: 'image', label: 'Image', count: 12 }] },
  { id: 'annotation', label: 'Annotation', kind: 'text', placeholder: 'Contains…' },
  { id: 'folder', label: 'Folder', semantic: 'folder', kind: 'single', options: [{ id: 'reference', label: 'Reference' }] },
  { id: 'tag', label: 'Tag', semantic: 'tag', kind: 'multi', options: [{ id: 'illustration', label: 'Illustration' }] },
  { id: 'rating', label: 'Rating', semantic: 'rating', kind: 'range', placeholder: '0–5' },
  { id: 'date', label: 'Date', semantic: 'date', kind: 'range', placeholder: 'YYYY-MM-DD..YYYY-MM-DD' },
] as const;

<VelvetFilterPanel
  facets={facets}
  resultCount={12}
  rules={rules}
  onApply={(nextRules) => applyRules(nextRules)}
  onRuleChange={setRules}
  onReset={() => setRules([])}
  onSave={(nextRules) => saveFilter(nextRules)}
/>
```

<span id="故事无障碍与验证" className="velvet-doc-anchor" />

契约位于 `apps/desktop/src/app/ui/contracts/host-projections/filter-panel.contract.ts`；可执行场景位于 `apps/desktop/src/app/ui/stories/host-projections/filter-panel.stories.tsx`。

语义名称和运算符类别以本地已交付应用研究矩阵为依据：`docs/research/eagle/eagle-analysis/report.md` 和 `docs/research/eagle/eagle-analysis/evidence-matrix.json`。研究记录仍是被版本控制忽略的产品证据；此组件只公开稳定的 Velvet 模式。
