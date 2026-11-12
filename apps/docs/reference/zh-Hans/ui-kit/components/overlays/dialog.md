---
id: dialog
title: Dialog 手册
---

# VelvetDialog

`VelvetDialog` 是 Velvet 的浮层组件，用于呈现由宿主控制的模态浮层。
实现位于 `apps/desktop/src/app/ui/overlays/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

## 契约 {/* #contract */}

- **属性：** `open`、`onOpenChange`、`title`、`description`、`children`、`actions`、`severity`、`closeLabel`、`restoreFocusRef`
- **状态：** `closed`、`open`、`warning`、`danger`、`busy`
- **契约：** `apps/desktop/src/app/ui/contracts/overlays/dialog.contract.ts`
- **示例：** `apps/desktop/src/app/ui/stories/overlays/dialog.stories.tsx`
- **资格验证：** browser（Radix portal、焦点约束和恢复）
- **宿主边界：** 宿主拥有受控的 `open` 值和动作生命周期。包装组件将 portal 和模态焦点处理交给 Radix，并在宿主将 `open` 从 true 改为 false 时使用 `restoreFocusRef`。

必填属性为 `open: boolean`、`onOpenChange: (open: boolean) => void`、`title: string` 和 `children: ReactNode`。可选属性为 `description?: string`、`actions?: readonly VelvetDialogAction[]`（默认为 `[]`）、`severity?: 'normal' | 'warning' | 'danger'`（默认为 `'normal'`）、`closeLabel?: string`（默认为 `'Close'`）和 `restoreFocusRef?: RefObject<HTMLElement | null>`。

每个动作必须包含 `id: string`、`label: string` 和 `onSelect: () => void`；还可包含 `variant?: 'default' | 'quiet' | 'danger'`、`primary?: boolean`、`disabled?: boolean` 和 `busy?: boolean`。显式的 `variant` 优先；否则主要动作使用 `'default'`，其余动作使用 `'quiet'`。`disabled` 或 `busy` 只阻止对应动作按钮。组件没有对话框级别的 `busy` 属性，忙碌动作也不会阻止关闭。

插槽为 `children` 和 `actions`；语义动作为 `open`、`close` 和 `submit`。调用动作的 `onSelect` 不会自动关闭对话框。宿主必须响应 `onOpenChange` 或动作来更新 `open`。关闭按钮和 Escape 会请求关闭；Radix 提供模态焦点行为及标题、描述语义。关闭后若要将焦点恢复到宿主触发元素，应传入其 `restoreFocusRef`；只有引用仍指向元素时才会聚焦。

组件契约约定的是语义。Radix 类型、私有 DOM 结构、Tailwind 类名和工作台实现细节均不属于公开 API。

## 示例与证据 {/* #stories-and-evidence */}

- `dialog/closed`
- `dialog/open`

真实浏览器测试夹具还覆盖 `dialog/submit`、`dialog/cancel-button` 和 `dialog/cancel-escape`，包括 portal 关闭后的焦点恢复。

当前 story 运行器提供确定性的测试夹具覆盖。浮层和宿主投影在焦点、portal、几何布局或宿主投影行为发生变化时，必须运行真实浏览器资格验证夹具。未经验证的外部行为在运行时采集提供证据之前，仍标记为 `unknown`。

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-browser
```

交互演示、API 与属性调试见 [VelvetDialog API 与 Playground](/ui-kit/components/dialog)。
