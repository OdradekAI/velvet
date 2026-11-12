---
id: filter-panel
title: Filter Panel
sidebar_label: Filter Panel
---

# VelvetFilterPanel

`VelvetFilterPanel` 是可组合的查询 facet 投影。宿主拥有 `rules`，组件只渲染控件并通过回调报告下一组规则。

## 契约

- Props：`facets`、`rules`、`resultCount`、`onRuleChange`、`onApply`、`onReset`、`onSave`、`onClose`
- facet 类型：`text`、`single`、`multi`、`range`
- 操作符：`contain`、`equal`、`intersection`、`between`、`before`、`after`、`within`、`not`、`or`
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/filter-panel.contract.ts`

组件不会读取数据库、执行搜索或推导结果数量。`resultCount`、选项数量和规则都来自宿主。`onApply` 提交当前规则，`onSave` 请求宿主保存命名筛选。

## 故事、无障碍与验证

Node story 覆盖默认 facet、reset/save 和 keyboard close。浏览器 fixture 覆盖文本规则、typed operator、结果数量、apply/reset/save 和关闭动作。修改规则 schema 时运行：

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
