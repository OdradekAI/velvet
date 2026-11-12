---
id: text
title: Text 手册
---

# VelvetText

`VelvetText` 是 Velvet 的基础组件，用于包装具有语义和色调的文本。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetText } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `as`、`tone`、`children`、`className`、`ariaLabel`
- **状态：** `default`、`tone`
- **所有者：** `component`；参见 `apps/desktop/src/app/ui/contracts/primitives/text.contract.ts`。
- **插槽：** `children`。
- **动作：** 未定义语义动作。
- **键盘与无障碍：** 不引入键盘行为，保留所选 HTML 元素和子内容的语义。

`children: ReactNode` 为必填项。`as` 可取 `'p' | 'span' | 'strong' | 'small' | 'h1' | 'h2' | 'h3'`（默认为 `'span'`）；`tone` 可取 `'primary' | 'secondary' | 'muted' | 'danger' | 'success' | 'warning'`（默认为 `'primary'`）。包装组件还接受 `className?: string`、`id?: string` 和 `role?: string`。使用 `as` 选择合适的原生文本或标题语义。

包装组件还接受 `UiCommonProps`：`apiVersion`（默认为 `'1.0'`）、`testId`、`ariaLabel`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。参见 `apps/desktop/src/app/ui/internal/types.ts` 和 `apps/desktop/src/app/ui/internal/ui-data-props.ts`；这些属性不代表显示组件和交互组件具有统一的禁用行为。组件支持 `className`。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/text.stories.tsx`：

- `text/default`
- `text/tones`

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

交互演示与属性调试见 [VelvetText API 与 Playground](/ui-kit/components/text)。
