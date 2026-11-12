---
id: dialog
title: Dialog
sidebar_label: Dialog
---

import { DialogPlayground } from '@site/src/components/UiKitPrimitivePlaygrounds';

# VelvetDialog

`VelvetDialog` 是宿主拥有的 modal overlay。它管理 portal、焦点捕获、Escape 关闭、焦点恢复和 action 区域。

## Demo

<DialogPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `open` | `boolean` | — | 是否打开 |
| `onOpenChange` | `(open: boolean) => void` | — | 打开状态变化回调 |
| `title` | `string` | — | Dialog 标题 |
| `description` | `string` | — | 可选描述 |
| `children` | `ReactNode` | — | Body 内容 |
| `actions` | `readonly VelvetDialogAction[]` | `[]` | Footer action |
| `severity` | `normal \| warning \| danger` | `normal` | 语义化风险级别 |
| `closeLabel` | `string` | `Close` | 关闭按钮 label |
| `restoreFocusRef` | `RefObject<HTMLElement>` | — | 关闭后恢复焦点的元素 |

### VelvetDialogAction

`id`、`label`、`variant`、`primary`、`disabled`、`busy` 和 `onSelect` 构成 action contract。

## 状态与无障碍

覆盖 closed、open、focus trap、Escape cancel、cancel button、submit 和 focus restoration。标题和描述由 Dialog 语义节点关联，action 使用原生 Button。

真实行为见 [Dialog manual](/reference/ui-kit/components/overlays/dialog)。

## 定制与迁移

使用 severity、semantic token 和 action variant。改变 portal、焦点恢复、Escape 行为或 action owner 时，需要运行 `npm run qualify:ui-browser`。
