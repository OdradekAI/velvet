---
id: icon-button
title: IconButton 手册
---

# VelvetIconButton

`VelvetIconButton` 是 Velvet 的基础组件，用于以图标为主体、带有无障碍名称的紧凑操作按钮。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetIconButton } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `icon`、`label`、`disabled`、`busy`、`onClick`
- **状态：** `default`、`disabled`、`focus`
- **所有者：** `host`；参见 `apps/desktop/src/app/ui/contracts/primitives/icon-button.contract.ts`。
- **插槽：** `icon`。
- **动作：** `activate` 通过继承的 `onClick` 回调暴露。
- **键盘与无障碍：** Enter 和 Space 激活原生按钮。`disabled` 或 `busy` 会阻止激活；必须提供无障碍名称 `label`。

`icon: ReactNode` 和 `label: string` 为必填项。包装组件继承 `VelvetButtonProps`，但排除 `children`、`label` 和 `ariaLabel`；继承项包括 `variant`、`onClick`、`className`、`describedBy`、原生 button 属性和 `ref`。`variant` 可取 `'default' | 'quiet' | 'danger'`，默认为 `'default'`。`label` 提供按钮的 `aria-label`；图标内容对辅助技术隐藏。宿主负责命令是否可执行以及实际执行。

继承的通用属性包括 `apiVersion`（默认为 `'1.0'`）、`testId`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。`ariaLabel` 被排除，由 `label` 提供其值。

类型接受 `ariaLive`，但当前 `VelvetButton` 包装组件没有将其传给 `uiDataProps`，因此不会从 `ariaLive` 生成 `aria-live`。需要播报时，应使用独立的实时区域。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/icon-button.stories.tsx`：

- `icon-button/default`
- `icon-button/disabled`

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

交互演示与属性调试见 [VelvetIconButton API 与 Playground](/ui-kit/components/icon-button)。
