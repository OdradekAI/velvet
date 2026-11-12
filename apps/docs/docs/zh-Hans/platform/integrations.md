---
id: integrations
title: UI 集成选择
---

# UI 集成选择

<span id="集成" className="velvet-doc-anchor" />

若宿主 Commands、菜单、通知和进度已满足任务，就使用这些能力。自定义 surface 是可选项。

可信第一方 UI 使用[宿主 React wrapper](../ui-kit)。第三方 surface 渲染自己的内容，通过框架无关的 [Bridge](./ui-bridge) 接入。React、Vue、Svelte 和纯 HTML 是渲染选择，不是分别发布的 Velvet SDK。

仓库有 iframe 和 Tauri child-WebView adapter，其 Alpha 和原生验证限制见[隔离 UI 指南](../guides/isolated-ui)。宿主 Tailwind class、Radix 类型、私有 DOM 和内部 React 组件不属于第三方契约。
