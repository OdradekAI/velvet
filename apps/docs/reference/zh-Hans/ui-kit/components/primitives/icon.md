---
id: icon
title: Icon 手册
---

# VelvetIcon

`VelvetIcon` 是 Velvet 的基础组件，用于包装带无障碍名称的图标或装饰性图标。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetIcon } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `name`、`children`、`label`、`className`
- **状态：** `default`、`decorative`、`labelled`
- **所有者：** `component`；参见 `apps/desktop/src/app/ui/contracts/primitives/icon.contract.ts`。
- **插槽：** `name`、`children`。
- **动作：** 未定义语义动作。
- **键盘与无障碍：** 不添加键盘行为。非空 `label` 会产生 `role="img"` 和 `aria-label`；否则包装元素设置 `aria-hidden="true"`。

`name?: VelvetIconName` 选择宿主拥有的语义图标。`children?: ReactNode` 是用于自定义渲染内容的兼容插槽，其优先级高于 `name`。支持的名称字面量定义在 `apps/desktop/src/app/ui/primitives/icon.tsx` 中。`label?: string` 决定图标的无障碍语义；空标签按装饰性图标处理。显式的 `label` 处理会覆盖继承的 `ariaLabel` 属性。

包装组件还接受 `UiCommonProps`：`apiVersion`（默认为 `'1.0'`）、`testId`、`ariaLabel`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。参见 `apps/desktop/src/app/ui/internal/types.ts` 和 `apps/desktop/src/app/ui/internal/ui-data-props.ts`；这些属性不代表显示组件和交互组件具有统一的禁用行为。组件支持 `className`。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/icon.stories.tsx`：

- `icon/default`
- `icon/labelled`
- `icon/semantic-name`

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

交互演示与属性调试见 [VelvetIcon API 与 Playground](/ui-kit/components/icon)。
