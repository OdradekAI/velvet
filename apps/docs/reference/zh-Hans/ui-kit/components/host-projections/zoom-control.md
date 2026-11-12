---
id: zoom-control
title: Zoom Control
sidebar_label: Zoom Control
---

# VelvetZoomControl

`VelvetZoomControl` 是宿主拥有的受控缩放投影。它提供原生范围滑块、显式增减按钮和实时显示的数值。宿主拥有缩放值，并决定该值如何影响资产网格；组件本身不持久化设置，也不调整内容尺寸。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `value`、`min`、`max`、`step`、`onChange`、`disabled`
- **状态：** default、disabled、min、max、keyboard
- **动作：** increment、decrement、change
- **Story：** `zoom-control/default`、`zoom-control/keyboard`、`zoom-control/bounds`、`zoom-control/disabled`
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/zoom-control.contract.ts`
- **实现：** `apps/desktop/src/app/ui/host-projections/zoom-control.tsx`
- **无障碍：** 具名组、原生范围语义、polite 数值播报、`aria-valuetext` 和键盘步进行为

## 证据边界 {/* #evidence-boundary */}

本地对外部应用的研究探测暴露一个带有 increment/decrement 动作的 `AXSlider`，初始值为 `180`。一次受控探测将值移到 `505`，一次递减得到 `500`，再递减 64 次回到 `180`，由此确定该次捕获状态下观察到的步长为 `5`。这项观察记录在 `docs/research/eagle/ui-kit/visual-qualification.md` 中。这些证据仅证明滑块及步进行为；外部应用的最小值、最大值、持久化和网格重排语义仍未知。

因此，Velvet 封装将 `min=25`、`max=500` 和 `step=5` 作为宿主策略默认值，同时允许配置这三个值。这些默认值是 Velvet 的设计选择，不是对外部应用完整取值范围的声明。目录的视觉元数据目前标注 `zoom:5-800`；该标签不改变实现默认值，也不强制取值范围。

## 用法 {/* #usage */}

```tsx
<VelvetZoomControl
  ariaLabel="Asset zoom"
  value={zoom}
  onChange={setZoom}
  min={25}
  max={500}
  step={5}
/>
```

<span id="无障碍与验证" className="velvet-doc-anchor" />

组件使用原生范围语义。浏览器资格验证覆盖键盘焦点、方向键改变数值、实时数值和增减动作。边界和禁用 story 会在 Node 中渲染这些状态，但目前没有动作检查点。几何尺寸属于视觉夹具的一部分，不能证明外部应用的取值范围。
