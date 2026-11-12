---
id: tabs
title: Tabs 手册
---

# VelvetTabs

`VelvetTabs` 是 Velvet 的基础组件，用于带有游走焦点的标签页选择控件。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetTabs } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `tabs`、`selectedId`、`onSelect`
- **状态：** `selected`、`disabled`、`focus`
- **所有者：** `host`；参见 `apps/desktop/src/app/ui/contracts/primitives/tabs.contract.ts`。
- **插槽：** `tab labels`。
- **动作：** `select` 和 `navigate` 调用 `onSelect` 并传入标签页 id。点击会请求选择；键盘导航还会移动焦点。
- **键盘与无障碍：** tablist 的方向为 horizontal，名称来自 `ariaLabel`，缺省为 `Tabs`。只有选中的标签页设置 `tabIndex={0}`。ArrowRight/ArrowDown 选择并聚焦下一个启用的标签页；ArrowLeft/ArrowUp 选择并聚焦上一个，到达两端时循环。Home/End 选择并聚焦第一个或最后一个启用的标签页。导航会跳过禁用的标签页。
- **资格验证：** Node/Happy DOM story，以及真实浏览器中的 tablist 方向、选择和焦点测试夹具。

`tabs: readonly VelvetTab[]`、`selectedId: string` 和 `onSelect: (id: string) => void` 为必填项。每个标签页包含 `id: string`、`label: string`，以及可选的 `disabled?: boolean` 和 `badge?: string | number`。宿主必须在 `onSelect` 后更新 `selectedId`，并使其对应启用的标签页，以保留 Tab 键停靠点。组件不生成 `tabpanel` 或 `aria-controls`；内容及其无障碍关联由宿主负责。通用属性 `disabled` 或 `busy` 不会禁用标签页按钮；应使用各标签页的 `disabled` 字段。

包装组件还接受 `UiCommonProps`：`apiVersion`（默认为 `'1.0'`）、`testId`、`ariaLabel`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。参见 `apps/desktop/src/app/ui/internal/types.ts` 和 `apps/desktop/src/app/ui/internal/ui-data-props.ts`；这些属性不代表显示组件和交互组件具有统一的禁用行为。组件支持 `className`。

`describedBy` 和 `ariaLive` 作用于外层包装元素，不会传给内部 tablist。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/tabs.stories.tsx`：

- `tabs/default`
- `tabs/disabled`

默认场景覆盖 ArrowRight 导航、直接选择标签页，以及焦点随选中标签页移动的行为。

## 证据 {/* #evidence */}

- `host-guarantee`：`docs/engineering/ui-kit-component-guide.md`。
- `observed`：`apps/desktop/test/ui-kit-components.test.mjs` 和 story 运行器。
- `unknown`：契约未列出的状态，在测试夹具或浏览器资格验证提供证据之前，均不作保证。

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-primitives
```

交互演示与属性调试见 [VelvetTabs API 与 Playground](/ui-kit/components/tabs)。
