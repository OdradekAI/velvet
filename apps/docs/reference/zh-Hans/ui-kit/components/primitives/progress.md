---
id: progress
title: Progress 手册
---

# VelvetProgress

`VelvetProgress` 是 Velvet 的基础组件，用于呈现 Job 进度投影。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetProgress } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `value`、`label`、`tone`
- **状态：** `determinate`、`indeterminate`、`success`、`danger`
- **所有者：** `host`；参见 `apps/desktop/src/app/ui/contracts/primitives/progress.contract.ts`。
- **插槽：** 无。
- **动作：** 未定义语义动作。
- **键盘与无障碍：** 不添加键盘行为。原生进度元素设置 `max={100}`；通用 ARIA 元数据作用于外层包装元素，`label` 为进度元素本身提供名称。

提供 `value?: number` 时呈现确定进度；有限数值会限制在 0–100 范围内，并显示为百分比。省略 `value` 时呈现不确定进度。`label?: string` 为原生进度元素提供名称，缺省名称为 `'Progress'`；`tone` 可取 `'accent' | 'success' | 'danger'`，默认为 `'accent'`。宿主提供进度投影；组件不会启动、监控或取消 Job。

包装组件还接受 `UiCommonProps`：`apiVersion`（默认为 `'1.0'`）、`testId`、`ariaLabel`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。参见 `apps/desktop/src/app/ui/internal/types.ts` 和 `apps/desktop/src/app/ui/internal/ui-data-props.ts`；这些属性不代表显示组件和交互组件具有统一的禁用行为。组件支持 `className`。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/progress.stories.tsx`：

- `progress/determinate`
- `progress/indeterminate`

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

交互演示与属性调试见 [VelvetProgress API 与 Playground](/ui-kit/components/progress)。
