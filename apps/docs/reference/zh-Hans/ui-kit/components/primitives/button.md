---
id: button
title: Button 手册
---

# VelvetButton

`VelvetButton` 是 Velvet 的基础组件，用于执行命令或本地操作。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetButton } from './app/ui/index.js';
```

<span id="props" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `children`、`label`、`variant`、`disabled`、`busy`、`onClick`
- **状态：** `default`、`quiet`、`danger`、`disabled`、`busy`、`focus`
- **所有者：** `host`；参见 `apps/desktop/src/app/ui/contracts/primitives/button.contract.ts`。
- **插槽：** `children`。
- **动作：** `activate` 通过 `onClick` 暴露，不会自行派发宿主命令。
- **键盘与无障碍：** Enter 和 Space 激活原生按钮；`disabled` 或 `busy` 会禁用按钮并阻止激活。`busy` 还设置 `aria-busy` 并显示装饰性的加载指示器。

`children?: ReactNode` 的优先级高于 `label?: string`。`variant` 可取 `'default' | 'quiet' | 'danger'`，默认为 `'default'`。包装组件接受类型化的原生 button 属性、`className` 和 `ref?: Ref<HTMLButtonElement>`；`type` 默认为 `'button'`。`onClick` 接收 `React.MouseEvent<HTMLButtonElement>`。宿主负责命令可见性、能力检查、启用条件和命令执行；组件只呈现状态并调用回调。

包装组件还接受 `UiCommonProps`：`apiVersion`（默认为 `'1.0'`）、`testId`、`ariaLabel`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。参见 `apps/desktop/src/app/ui/internal/types.ts` 和 `apps/desktop/src/app/ui/internal/ui-data-props.ts`；这些属性不代表显示组件和交互组件具有统一的禁用行为。组件支持 `className`。

类型接受 `ariaLive`，但当前 `VelvetButton` 包装组件没有将其传给 `uiDataProps`，因此不会从 `ariaLive` 生成 `aria-live`。需要播报时，应使用独立的实时区域。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/button.stories.tsx`：

- `button/default`
- `button/busy`
- `button/disabled`

<span id="状态与证据" className="velvet-doc-anchor" />

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

交互演示与属性调试见 [VelvetButton API 与 Playground](/ui-kit/components/button)。
动作覆盖见 [UI Kit 动作覆盖](/reference/ui-kit/action-coverage)。
