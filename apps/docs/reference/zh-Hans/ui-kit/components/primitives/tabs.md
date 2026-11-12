---
id: tabs
title: Tabs 手册
---

# VelvetTabs

Tabs 提供 tablist、roving tabindex、选中状态和键盘导航。`tabs` 项包含 `id`、`label`、`disabled` 和可选 `badge`。组件 Props 还包括 `selectedId`、`onSelect`、`ariaLabel`、`describedBy`、`apiVersion`、`testId` 和 `className`。

ArrowLeft、ArrowRight、ArrowUp、ArrowDown、Home 和 End 的行为由宿主组件负责。组件不会生成 `tabpanel`，宿主需要关联选中 Tab 与内容区域。完整 Demo 见 [Tabs API 与 Playground](/ui-kit/components/tabs)。
