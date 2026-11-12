---
id: ui-kit-component-guide
title: Velvet UI Kit 组件指南
---

# Velvet UI Kit 组件指南

状态：UI Kit v1 host implementation。组件文档服务于插件开发者和宿主维护者，公开语义、状态、动作、焦点和无障碍 contract，React 实现和 CSS class 保持宿主内部可替换。

## 组件分层

- primitives 提供 Button、Input、Select、Tabs、Checkbox 等基础交互。
- overlays 提供 Dialog、Context Menu 和 Toast。
- host projections 提供 Activity Bar、PreviewHost、Inspector、Panel 等宿主拥有的 surface。

## 公开边界

插件可以依赖 semantic props、typed callback、theme snapshot、component version 和 qualification hook。插件不能依赖 React component identity、私有 DOM、Tailwind class、Radix data attribute 或源文件路径。

## 组件页面阅读顺序

1. 先查看 Demo 和属性 Playground。
2. 再查看 Props、状态、动作和 keyboard model。
3. 接着查看 accessibility、定制和主题 token。
4. 最后查看 migration、story、browser qualification 和 compatibility。

## 组件入口

- [Button](/ui-kit/components/button)
- [Input](/ui-kit/components/input)
- [Select](/ui-kit/components/select)
- [Tabs](/ui-kit/components/tabs)
- [Checkbox](/ui-kit/components/checkbox)

完整的组件 evidence 来自 `apps/desktop/src/app/ui/contracts`、story runner、browser qualification 和 [UI Kit action coverage](ui-kit/action-coverage.md)。
