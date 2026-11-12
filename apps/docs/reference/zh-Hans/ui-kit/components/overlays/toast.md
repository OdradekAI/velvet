---
id: toast
title: Toast 手册
---

# VelvetToastHost

`VelvetToastHost` 是 Velvet 的浮层组件，用于呈现实时通知投影。
实现位于 `apps/desktop/src/app/ui/overlays/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

## 契约 {/* #contract */}

- **属性：** `items`、`onDismiss`、`className`
- **状态：** `info`、`success`、`warning`、`error`
- **契约：** `apps/desktop/src/app/ui/contracts/overlays/toast.contract.ts`
- **示例：** `apps/desktop/src/app/ui/stories/overlays/toast.stories.tsx`
- **资格验证：** Node/Happy DOM，以及真实浏览器中的实时区域和关闭测试夹具
- **宿主边界：** 宿主提供通知条目并处理关闭。`VelvetToastHost` 呈现该投影，不会移除条目、持久化通知或安排自动过期。

`items: readonly VelvetToastItem[]` 和 `onDismiss: (id: string) => void` 为必填项；`className?: string` 为可选项。每个条目必须包含 `id: string` 和 `message: string`；可选字段为 `title?: string`、`severity?: 'info' | 'success' | 'warning' | 'error'`（默认为 `'info'`）和 `action?: ReactNode`。`action` 是内容插槽，不是命令回调。

组件呈现名称为 `'Notifications'` 的 region。错误条目使用 `role="alert"` 和 `aria-live="assertive"`；其余严重程度使用 `role="status"` 和 `aria-live="polite"`。关闭图标按钮的名称为 `'Dismiss notification'`，支持原生按钮键盘激活。`dismiss` 动作仅调用 `onDismiss(item.id)`；所有者必须在下一次 `items` 投影中移除条目。

在宿主的本地组合中，`VelvetToastProvider` 拥有内存中的列表并呈现 `VelvetToastHost`。`useVelvetToast()` 暴露 `push(item: Omit<VelvetToastItem, 'id'>): string` 和 `dismiss(id: string): void`。provider 为本次挂载中的条目分配递增的 `toast-N` id，并在关闭时移除条目；它不提供持久化或过期策略。在 provider 外调用 hook 会抛出 `useVelvetToast must be used inside VelvetToastProvider`。

组件契约约定的是语义。Radix 类型、私有 DOM 结构、Tailwind 类名和工作台实现细节均不属于公开 API。

## 示例与证据 {/* #stories-and-evidence */}

- `toast/default`
- `toast/error`

当前 story 运行器提供确定性的测试夹具覆盖。浮层和宿主投影在焦点、portal、几何布局或宿主投影行为发生变化时，必须运行真实浏览器资格验证夹具。未经验证的外部行为在运行时采集提供证据之前，仍标记为 `unknown`。

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-primitives
```

交互演示、API 与属性调试见 [VelvetToastHost API 与 Playground](/ui-kit/components/toast)。
