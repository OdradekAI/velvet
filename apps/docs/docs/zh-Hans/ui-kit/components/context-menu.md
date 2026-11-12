---
id: context-menu
title: ContextMenu
sidebar_label: ContextMenu
---

import { ContextMenuPlayground } from '@site/src/components/playgrounds/context-menu';

# VelvetContextMenu

`VelvetContextMenu` 用于 trigger-driven contextual command menu。宿主拥有 action callback，Radix 负责 portal、roving focus 和 submenu transition。

## Demo

<ContextMenuPlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `ReactNode` | — | 触发区域 |
| `actions` | `readonly VelvetContextMenuAction[]` | — | 菜单项定义 |

### VelvetContextMenuAction

| Property | Type | Description |
| --- | --- | --- |
| `id` | `string` | 稳定 action id |
| `label` | `string` | 可见名称 |
| `disabled` | `boolean` | 禁止选择 |
| `checked` | `boolean` | `menuitemcheckbox` 状态 |
| `shortcutHint` | `string` | 键盘提示 |
| `separatorBefore` | `boolean` | 是否显示分隔线 |
| `group` | `string` | 可选元数据；当前渲染器不按它分组 |
| `submenu` | `readonly VelvetContextMenuAction[]` | 递归子菜单 |
| `onSelect` | `() => void` | 可选选择回调 |

Action 只有 `id` 和 `label` 必填。`checked` 由调用方拥有，选择时调用 `onSelect`，不会自动切换该值。`shortcutHint` 只显示文本，不注册快捷键。非空 submenu 渲染为子菜单触发器，不调用父项操作。

## 状态与无障碍

覆盖 default、checked、disabled、submenu、pointer dismissal、Escape cancel 和 keyboard selection。菜单项使用 Radix menu semantics，宿主 action callback 不应直接绕过 registry。

真实行为见 [ContextMenu manual](/reference/ui-kit/components/overlays/context-menu)。

## 定制与迁移

使用 action schema、shortcutHint 和 semantic token。改变 submenu、焦点导航或 checked semantics 时，需要运行 `npm run qualify:ui-browser`。
