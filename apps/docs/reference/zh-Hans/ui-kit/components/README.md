---
id: components
title: UI Kit 组件手册
---

# Velvet UI Kit 组件手册

<span id="ui-kit-组件手册" className="velvet-doc-anchor" />

每个已迁移组件都有三份相互关联的产物：

1. `apps/desktop/src/app/ui/` 下的实现模块；
2. 类型化契约和可执行 story；
3. 本目录中面向读者的手册，说明用法、所有权、生命周期和证据。

<span id="primitives" className="velvet-doc-anchor" />

第一轮覆盖基础组件：

| 组件 | 契约 | Story | 手册 |
| --- | --- | --- | --- |
| VelvetText | `src/app/ui/contracts/primitives/text.contract.ts` | `src/app/ui/stories/primitives/text.stories.tsx` | [text](primitives/text.md) |
| VelvetIcon | `src/app/ui/contracts/primitives/icon.contract.ts` | `src/app/ui/stories/primitives/icon.stories.tsx` | [icon](primitives/icon.md) |
| VelvetBadge | `src/app/ui/contracts/primitives/badge.contract.ts` | `src/app/ui/stories/primitives/badge.stories.tsx` | [badge](primitives/badge.md) |
| VelvetStack | `src/app/ui/contracts/primitives/stack.contract.ts` | `src/app/ui/stories/primitives/stack.stories.tsx` | [stack](primitives/stack.md) |
| VelvetInline | `src/app/ui/contracts/primitives/inline.contract.ts` | `src/app/ui/stories/primitives/inline.stories.tsx` | [inline](primitives/inline.md) |
| VelvetButton | `src/app/ui/contracts/primitives/button.contract.ts` | `src/app/ui/stories/primitives/button.stories.tsx` | [button](primitives/button.md) |
| VelvetIconButton | `src/app/ui/contracts/primitives/icon-button.contract.ts` | `src/app/ui/stories/primitives/icon-button.stories.tsx` | [icon-button](primitives/icon-button.md) |
| VelvetInput | `src/app/ui/contracts/primitives/input.contract.ts` | `src/app/ui/stories/primitives/input.stories.tsx` | [input](primitives/input.md) |
| VelvetSelect | `src/app/ui/contracts/primitives/select.contract.ts` | `src/app/ui/stories/primitives/select.stories.tsx` | [select](primitives/select.md) |
| VelvetCheckbox | `src/app/ui/contracts/primitives/checkbox.contract.ts` | `src/app/ui/stories/primitives/checkbox.stories.tsx` | [checkbox](primitives/checkbox.md) |
| VelvetSeparator | `src/app/ui/contracts/primitives/separator.contract.ts` | `src/app/ui/stories/primitives/separator.stories.tsx` | [separator](primitives/separator.md) |
| VelvetTabs | `src/app/ui/contracts/primitives/tabs.contract.ts` | `src/app/ui/stories/primitives/tabs.stories.tsx` | [tabs](primitives/tabs.md) |
| VelvetProgress | `src/app/ui/contracts/primitives/progress.contract.ts` | `src/app/ui/stories/primitives/progress.stories.tsx` | [progress](primitives/progress.md) |
| VelvetState | `src/app/ui/contracts/primitives/state.contract.ts` | `src/app/ui/stories/primitives/state.stories.tsx` | [state](primitives/state.md) |

生成的目录 `apps/desktop/src/app/ui/ui-kit-catalog.json` 将每个组件 id 关联到其契约与手册。浮层和宿主投影的契约、story 及实现分别放在独立文件中。所有基础组件目前都具有浏览器资格验证层级：专用夹具检查原生控件语义、动态播报区域、标签、布局封装和 Tabs 焦点移动。Toast 使用同一夹具验证动态播报区域的关闭行为。Dialog 和 Context Menu 要求真实浏览器浮层资格验证；AssetGrid、DataTable、Inspector、CommandPalette 和 QuickSearch 使用宿主投影键盘与焦点资格验证夹具。

<span id="overlays" className="velvet-doc-anchor" />

Dialog 和 Context Menu 通过浮层浏览器资格验证夹具检查提交、选择、取消、portal 和焦点行为。CommandPalette 和 QuickSearch 是只呈现投影的搜索界面：宿主提供结果和历史列表，夹具验证查询回调、标签页、键盘导航和焦点。在 `apps/desktop` 中运行 `npm run qualify:ui-browser`，可在标准 1280×800 视口下执行三类夹具；也可以运行完整的 `npm run verify:ui-kit` 验收入口。PreviewHost 还拥有成功、取消、超时、失败和重试的异步生命周期 story 矩阵；这些确定性场景通过 Node/Happy DOM story 运行器执行。

<span id="host-projections" className="velvet-doc-anchor" />

AssetGrid、Inspector 和 ZoomControl 将交互证据保留在各自的契约和浏览器资格验证夹具中。它们在外部运行时中的取值范围与持久化语义仍明确标记为未知。完整的动作到 story/浏览器映射维护在 `../action-coverage.md` 中。

这一轮首个根据证据新增的宿主投影是 `VelvetFilterPanel`（[手册](host-projections/filter-panel.md)）。筛选规则仍由宿主持有，组件支持 text、single、multi 和 range 筛选维度，并通过 Node story 运行器和真实浏览器宿主夹具进行资格验证。

`VelvetFilterFacetMenu`（[手册](host-projections/filter-facet-menu.md)）是独立的文件夹与颜色投影。它通过宿主回调封装观察到的 360px 文件夹菜单和 202px 颜色菜单几何、include/exclude 与 OR/AND/EQUAL 动作、调色板、十六进制颜色和精度控件。其浏览器资格验证覆盖指针 include/exclude、焦点进入、Escape 关闭和触发元素焦点恢复。

当前语义与视觉映射维护在本地研究记录中；产品契约仅保留由研究得出的宿主所有权和动作语义。
