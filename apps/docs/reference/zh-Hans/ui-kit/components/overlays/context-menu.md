---
id: context-menu
title: ContextMenu 手册
---

# VelvetContextMenu

`VelvetContextMenu` 是 Velvet 的浮层组件，用于通过触发区域打开上下文命令菜单。
实现位于 `apps/desktop/src/app/ui/overlays/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

## 契约 {/* #contract */}

- **属性：** `actions`、`children`；每个动作均可包含递归的 `submenu`
- **状态：** `closed`、`open`、`disabled`、`checked`、`submenu`
- **契约：** `apps/desktop/src/app/ui/contracts/overlays/context-menu.contract.ts`
- **示例：** `apps/desktop/src/app/ui/stories/overlays/context-menu.stories.tsx`
- **资格验证：** browser（Radix portal、指针关闭和键盘选择）
- **宿主边界：** 宿主拥有动作数据、回调和命令生命周期。包装组件将临时打开状态、portal、游走焦点和子菜单切换交给 Radix；它不暴露受控的 `open` 属性。

`children: ReactNode` 和 `actions: readonly VelvetContextMenuAction[]` 为必填项。每个动作必须包含 `id: string` 和 `label: string`；可选字段为 `disabled?: boolean`、`checked?: boolean`、`shortcutHint?: string`、`separatorBefore?: boolean`、`group?: string`、`submenu?: readonly VelvetContextMenuAction[]` 和 `onSelect?: () => void`。非空的 `submenu` 会呈现子菜单触发器，而非叶级动作。`group` 已声明，但当前渲染器不使用它；`shortcutHint` 只显示提示，不注册快捷键。

对于叶级动作，只要定义了 `checked`（包括 `false`），就会产生 `role="menuitemcheckbox"` 和 `aria-checked`。选择动作会调用已提供的 `onSelect`；包装组件不会自行切换 `checked`，宿主必须更新动作数据。禁用的动作不能被选择。键盘导航和子菜单方向由 Radix 处理；浏览器测试夹具覆盖 Escape 和外部指针操作关闭。插槽为 `trigger`、`actions` 和 `submenu.actions`；语义动作为 `open`、`select`、`dismiss` 和 `navigate`。

组件契约约定的是语义。Radix 类型、私有 DOM 结构、Tailwind 类名和工作台实现细节均不属于公开 API。

## 示例与证据 {/* #stories-and-evidence */}

- `context-menu/default`
- `context-menu/checked`
- `context-menu/submenu`

真实浏览器测试夹具还覆盖 `context-menu/select`、`context-menu/cancel-escape` 和 `context-menu/cancel-outside`，包括选中菜单项的语义。

嵌套动作通过 Velvet 包装组件的子菜单插槽呈现。Radix 负责 portal、游走焦点和按方向切换子菜单；动作回调及命令生命周期仍由宿主负责。

当前 story 运行器提供确定性的测试夹具覆盖。浮层和宿主投影在焦点、portal、几何布局或宿主投影行为发生变化时，必须运行真实浏览器资格验证夹具。未经验证的外部行为在运行时采集提供证据之前，仍标记为 `unknown`。

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-browser
```

交互演示、API 与属性调试见 [VelvetContextMenu API 与 Playground](/ui-kit/components/context-menu)。
