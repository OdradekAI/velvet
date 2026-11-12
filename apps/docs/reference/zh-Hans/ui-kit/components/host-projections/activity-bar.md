---
id: activity-bar
title: Activity Bar
sidebar_label: Activity Bar
---

# VelvetActivityBar

`VelvetActivityBar` 是 Workbench 导航栏的宿主投影。宿主提供条目和选择回调，插件不能直接修改导航布局。

## 契约

- Props：`items`、`onSelect`
- 状态：默认、带徽标
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/activity-bar.contract.ts`
- Stories：`apps/desktop/src/app/ui/stories/host-projections/activity-bar.stories.tsx`

组件只公开语义属性和回调。Radix 类型、私有 DOM、Tailwind class 和 Workbench 实现细节不属于插件 API。

## 故事与无障碍

- `activity-bar/default` 覆盖点击选择和方向键导航。
- `activity-bar/badges` 覆盖条目徽标。
- 使用 navigation landmark 和 roving focus。Arrow、Home、End 会移动焦点和选择，disabled 条目会被跳过。

## 所有权与验证

布局、焦点和 projection state 由宿主拥有。修改键盘行为或条目语义时运行：

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
```
