---
id: state
title: State
sidebar_label: State
---

import { StatePlayground } from '@site/src/components/playgrounds/state';

# VelvetState

`VelvetState` 用于组合 loading、empty 和 error 状态。这个基础组件呈现调用方拥有的状态语义、动态播报区域、错误重试和可选操作插槽，不拥有业务操作或重试的生命周期。

## Demo

<StatePlayground locale="zh-Hans" />

## API

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `kind` | `loading \| empty \| error` | — | 状态类型 |
| `title` | `string` | — | 状态标题 |
| `description` | `string` | — | 可选说明 |
| `retryLabel` | `string` | `Retry` | error 状态的重试按钮名称 |
| `onRetry` | `() => void` | — | error 状态的重试回调 |
| `action` | `ReactNode` | — | 可选内容 action slot |
| `apiVersion` | `string` | `1.0` | UI Kit contract 版本标记 |
| `testId` | `string` | — | qualification 使用的稳定标识 |

## 状态与无障碍

loading 使用 `aria-busy` 和 polite status，empty 使用 status，error 使用 alert。提供 `onRetry` 时，error 状态渲染可用键盘触发的 Button。

真实行为见 [State manual](/reference/ui-kit/components/primitives/state)。

## 定制与迁移

使用 kind、semantic token 和 retry contract。改变 live region、错误重试或 action slot 语义时，需要更新 component contract 和 recovery fixture。
