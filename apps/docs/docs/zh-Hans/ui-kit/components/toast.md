---
id: toast
title: Toast
sidebar_label: Toast
---

import { ToastPlayground } from '@site/src/components/playgrounds/toast';

# VelvetToastHost

`VelvetToastHost` 用于 live notification projection。它显示宿主或插件提交的 info、success、warning 和 error 通知，并提供 dismiss action。

## Demo

<ToastPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `items` | `readonly VelvetToastItem[]` | — | 当前通知集合 |
| `onDismiss` | `(id: string) => void` | — | dismiss 回调 |
| `className` | `string` | — | 宿主允许的语义样式扩展 |

### VelvetToastItem

`id` 和 `message` 必填，`title`、`severity`、`action` 可选，`severity` 默认为 `info`。`useVelvetToast` 在 `VelvetToastProvider` 内提供 `push(item: Omit<VelvetToastItem, 'id'>): string` 和 `dismiss(id: string): void`；在该 provider 外使用会抛错。`push` 分配 ID，宿主不会自动移除通知，由调用方控制移除。隔离扩展通过宿主协议请求通知，不调用此 React hook。

## 状态与无障碍

info、success、warning 使用 polite live region，error 使用 assertive alert。每条通知提供可访问的 dismiss IconButton。

真实行为见 [Toast manual](/reference/ui-kit/components/overlays/toast)。

## 定制与迁移

使用 severity、live region 和 semantic token。改变通知生命周期、dismiss action 或 error announcement 时，需要更新 qualification fixture。
