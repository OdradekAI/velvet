---
id: checkbox
title: Checkbox 手册
---

# VelvetCheckbox

`VelvetCheckbox` 是 Velvet 的基础组件，用于受控的布尔设置。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetCheckbox } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `checked`、`label`、`disabled`、`onCheckedChange`
- **状态：** `checked`、`unchecked`、`disabled`、`focus`
- **所有者：** `host`；参见 `apps/desktop/src/app/ui/contracts/primitives/checkbox.contract.ts`。
- **插槽：** 无。
- **动作：** `toggle` 调用 `onCheckedChange`，传入 `event.currentTarget.checked`；回调参数是布尔值，不是事件。
- **键盘与无障碍：** Tab 将焦点移到启用的原生复选框；Space 或点击复选框及其标签会请求切换。禁用的复选框不能通过正常用户交互获得焦点或切换状态。

`checked: boolean`、`label: string` 和 `onCheckedChange: (checked: boolean) => void` 为必填项。回调之后，宿主必须传入更新后的 `checked` 值。`ariaLabel`、`describedBy` 和其余通用元数据作用于外层 label，不会直接传给 input。可见的 `label` 通过生成的 id 为 input 提供名称。`disabled` 会禁用 input；仅设置 `busy` 不会禁用它。

包装组件还接受 `UiCommonProps`：`apiVersion`（默认为 `'1.0'`）、`testId`、`ariaLabel`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。参见 `apps/desktop/src/app/ui/internal/types.ts` 和 `apps/desktop/src/app/ui/internal/ui-data-props.ts`；这些属性不代表显示组件和交互组件具有统一的禁用行为。组件支持 `className`。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/checkbox.stories.tsx`：

- `checkbox/checked`
- `checkbox/disabled`

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
```

交互演示与属性调试见 [VelvetCheckbox API 与 Playground](/ui-kit/components/checkbox)。
