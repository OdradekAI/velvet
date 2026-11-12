---
id: select
title: Select 手册
---

# VelvetSelect

`VelvetSelect` 是 Velvet 的基础组件，用于带有宿主样式的原生选择控件。它位于 `primitives` 层，通过 `apps/desktop/src/app/ui/index.ts` 导入。

```tsx
import { VelvetSelect } from './app/ui/index.js';
```

## 契约 {/* #contract */}

- **属性：** `value`、`defaultValue`、`disabled`、`busy`、`onChange`、`children`
- **状态：** `default`、`focus`、`disabled`
- **所有者：** `component`；参见 `apps/desktop/src/app/ui/contracts/primitives/select.contract.ts`。
- **插槽：** `options`。
- **动作：** `change` 通过原生 `onChange` 回调暴露。
- **键盘与无障碍：** 保留原生选项选择和方向键行为。`disabled` 或 `busy` 会禁用控件；`busy` 还设置 `aria-busy`。应提供标签或 `ariaLabel`。

包装组件接受类型化的原生 select 属性，包括 `value`、`defaultValue`、`multiple`、`name`、`onChange` 和 `children`，以及 `className`。通过 `children` 传入原生选项内容，即契约中的 `options` 插槽。`onChange` 接收 `React.ChangeEvent<HTMLSelectElement>`。`value` 由调用方控制；`defaultValue` 初始化原生状态。组件保留原生选项、多选和表单提交语义。

包装组件还接受 `UiCommonProps`：`apiVersion`（默认为 `'1.0'`）、`testId`、`ariaLabel`、`describedBy`、`disabled`、`busy`、`ariaLive?: 'off' | 'polite' | 'assertive'` 和 `data-*` 元数据。参见 `apps/desktop/src/app/ui/internal/types.ts` 和 `apps/desktop/src/app/ui/internal/ui-data-props.ts`；这些属性不代表显示组件和交互组件具有统一的禁用行为。组件支持 `className`。

类型接受 `ariaLive`，但当前 `VelvetSelect` 包装组件没有将其传给 `uiDataProps`，因此不会从 `ariaLive` 生成 `aria-live`。需要播报时，应使用独立的实时区域。

此契约约定的是语义。React 实现细节、Tailwind 类名、Radix 属性和私有 DOM 结构均不属于公开 API。

## 示例 {/* #stories */}

可执行 story 位于 `apps/desktop/src/app/ui/stories/primitives/select.stories.tsx`：

- `select/default`
- `select/disabled`

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

交互演示与属性调试见 [VelvetSelect API 与 Playground](/ui-kit/components/select)。
