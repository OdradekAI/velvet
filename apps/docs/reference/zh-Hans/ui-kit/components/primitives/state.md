---
id: state
title: State 手册
---

# VelvetState

`VelvetState` 是 Velvet 的基础组件，用于组合加载中、空内容和错误状态。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetState } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `kind`、`title`、`description`、`retryLabel`、`onRetry`、`action`
- **状态：** `loading`、`empty`、`error`
- **所有者：** `host`；参见 `apps/desktop/src/app/ui/contracts/primitives/state.contract.ts`。
- **插槽：** `action`。
- **动作：** `retry` 调用 `onRetry`。`action` 是各类状态均可使用的内容插槽，不代表宿主命令。通用属性 `disabled` 或 `busy` 不会禁用重试按钮。
- **键盘与无障碍：** 加载状态设置 `aria-busy="true"`；加载和空内容状态使用 `role="status"` 和 `aria-live="polite"`。错误状态使用 `role="alert"` 和 `aria-live="assertive"`。只有 `kind="error"` 且提供 `onRetry` 时才呈现支持 Enter 和 Space 的原生重试按钮。

`kind: 'loading' | 'empty' | 'error'` 和 `title: string` 为必填项。可选属性包括 `description?: string`、`retryLabel?: string`（默认为 `'Retry'`）、`onRetry?: () => void`、`action?: ReactNode` 和 `className`。宿主负责加载、失败和重试工作；组件不会自动修改 `kind` 或执行重试策略。`kind` 决定 `aria-busy` 和 `aria-live`，覆盖继承的 `busy` 和 `ariaLive` 值。

包装组件还接受 `UiCommonProps`：`apiVersion`（默认为 `'1.0'`）、`testId`、`ariaLabel`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。参见 `apps/desktop/src/app/ui/internal/types.ts` 和 `apps/desktop/src/app/ui/internal/ui-data-props.ts`；这些属性不代表显示组件和交互组件具有统一的禁用行为。组件支持 `className`。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/state.stories.tsx`：

- `state/loading`
- `state/empty`
- `state/error-retry`

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

交互演示与属性调试见 [VelvetState API 与 Playground](/ui-kit/components/state)。
