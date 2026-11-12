---
id: preview-host
title: Preview Host
sidebar_label: Preview Host
---

# VelvetPreviewHost

`VelvetPreviewHost` 是预览生命周期和 fallback 的宿主投影。宿主拥有 provider、transport、job 和 session，组件只呈现当前状态并报告动作。

## 契约

- Props：`state`、`mode`、`message`、`toolbar`、`providerActions`、`children`、`onRetry`、`onModeChange`、`onProviderAction`、`onOpenExternal`、`onClose`
- 状态：loading、ready、empty、error、disabled
- 异步生命周期：success、cancelled、timeout、failure、retry
- 合同：`apps/desktop/src/app/ui/contracts/host-projections/preview-host.contract.ts`
- Stories：`apps/desktop/src/app/ui/stories/host-projections/preview-host.stories.tsx`

`mode`、provider actions 和 close 都由宿主控制。组件不解析资产、不调用 CoreTransport，也不推断 provider 能力。重试必须由宿主创建新 attempt，并保留失败原因供诊断。

## 无障碍与验证

预览区域有明确名称，loading 使用 busy 语义，error 和 empty fallback 会被 live region 宣布。Stories 覆盖 ready、error、retry、toolbar、provider action、主窗口生命周期以及 success、cancelled、timeout、failure、retry。

```sh
cd apps/desktop
npm run typecheck
npm run generate:ui-kit-catalog
npm run qualify:ui-host-projections
npm run qualify:native-preview
```
