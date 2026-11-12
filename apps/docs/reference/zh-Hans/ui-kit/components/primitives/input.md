---
id: input
title: Input 手册
---

# VelvetInput

`VelvetInput` 是 Velvet 的基础组件，用于带有宿主密度和焦点样式的文本输入。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetInput } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `value`、`defaultValue`、`placeholder`、`disabled`、`busy`、`onChange`、`onKeyDown`
- **状态：** `default`、`focus`、`disabled`、`busy`
- **所有者：** `component`；参见 `apps/desktop/src/app/ui/contracts/primitives/input.contract.ts`。
- **插槽：** 无。
- **动作：** `change` 通过 `onChange` 暴露；`onKeyDown` 保留类型化的原生键盘回调。
- **键盘与无障碍：** 保留原生文本编辑和焦点语义。`disabled` 或 `busy` 会禁用编辑和正常聚焦；`busy` 还设置 `aria-busy`。应通过标签或 `ariaLabel` 提供无障碍名称。

包装组件接受类型化的原生 input 属性，包括 `type`、`value`、`defaultValue`、`placeholder`、`readOnly`、`onChange` 和 `onKeyDown`，以及 `className`。`onChange` 接收 `React.ChangeEvent<HTMLInputElement>`。使用 `value` 时，调用方控制当前值，并须在变化后更新；使用 `defaultValue` 时，后续编辑由原生输入框保存。组件不添加校验或持久化策略。

包装组件还接受 `UiCommonProps`：`apiVersion`（默认为 `'1.0'`）、`testId`、`ariaLabel`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。参见 `apps/desktop/src/app/ui/internal/types.ts` 和 `apps/desktop/src/app/ui/internal/ui-data-props.ts`；这些属性不代表显示组件和交互组件具有统一的禁用行为。组件支持 `className`。

类型接受 `ariaLive`，但当前 `VelvetInput` 包装组件没有将其传给 `uiDataProps`，因此不会从 `ariaLive` 生成 `aria-live`。需要播报时，应使用独立的实时区域。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/input.stories.tsx`：

- `input/default`
- `input/change`
- `input/busy`

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

交互演示与属性调试见 [VelvetInput API 与 Playground](/ui-kit/components/input)。
