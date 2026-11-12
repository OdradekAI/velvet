---
id: separator
title: Separator 手册
---

# VelvetSeparator

`VelvetSeparator` 是 Velvet 的基础组件，用于视觉和语义分隔。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetSeparator } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `orientation`
- **状态：** `horizontal`、`vertical`
- **所有者：** `component`；参见 `apps/desktop/src/app/ui/contracts/primitives/separator.contract.ts`。
- **插槽：** 无。
- **动作：** 未定义语义动作。
- **键盘与无障碍：** 不可聚焦的分隔元素呈现 `role="separator"` 和 `aria-orientation`，不引入键盘行为。

唯一属性为 `orientation?: 'horizontal' | 'vertical'`，默认为 `'horizontal'`。此包装组件不接受 `UiCommonProps`、`children` 或 `className`。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/separator.stories.tsx`：

- `separator/horizontal`
- `separator/vertical`

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

交互演示与属性调试见 [VelvetSeparator API 与 Playground](/ui-kit/components/separator)。
