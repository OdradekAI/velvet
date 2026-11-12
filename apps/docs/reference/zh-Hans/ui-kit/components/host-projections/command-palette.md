---
id: command-palette
title: Command Palette
sidebar_label: Command Palette
---

# VelvetCommandPalette

`VelvetCommandPalette` 是宿主命令搜索浮层的 Velvet 宿主投影组件。实现位于 `apps/desktop/src/app/ui/host-projections/`；公开导入入口仍为 `apps/desktop/src/app/ui/index.ts`。

<span id="契约" className="velvet-doc-anchor" />

## 契约 {/* #contract */}

- **属性：** `open`、`commands`、`scope`、`scopeOptions`、`onScopeChange`、`query`、`onQueryChange`、`onSelect`、`onClose`、`restoreFocusRef`
- **状态：** open、empty、selected、disabled
- **契约：** `apps/desktop/src/app/ui/contracts/host-projections/command-palette.contract.ts`
- **Story：** `apps/desktop/src/app/ui/stories/host-projections/command-palette.stories.tsx`
- **资格验证：** Node/Happy DOM 和真实浏览器两层
- **宿主边界：** 布局、浮层、焦点和投影状态仍由宿主拥有。

组件契约只规定语义。Radix 类型、私有 DOM 结构、Tailwind 类和 Workbench 实现细节不属于公开 API。

<span id="故事与无障碍" className="velvet-doc-anchor" />

## Story 与证据 {/* #stories-and-evidence */}

- `command-palette/open`
- `command-palette/empty`
- `command-palette/scope`
- `command-palette/controlled-query`
- `command-palette/host-results`

Story 运行器覆盖键盘导航、选择、Escape 关闭、受控查询回调，以及宿主提供的结果替换。省略 `query` 时，组件为输入框保留本地草稿；提供 `query` 时，该值由宿主拥有，宿主必须提交通过 `onQueryChange` 收到的变更，组件不维护第二个事实来源。

`commands` 已经是当前范围的结果投影。组件不解析查询语法、筛选命令、对结果评分或访问索引。这些职责属于 Workbench 搜索或命令适配器。浏览器资格验证会确认：宿主替换结果列表时，浮层仍保持键盘、ARIA 和焦点行为。

当前 story 运行器提供确定性夹具覆盖。对于浮层和宿主投影，修改焦点、portal、几何尺寸或宿主投影行为时，必须运行真实浏览器资格验证夹具。

`command-palette/open` 场景覆盖键盘导航、选择和 Escape 关闭。`scopeOptions` 和 `onScopeChange` 让宿主投影 current/all 结果范围，不会把查询所有权移入组件。输入框提供指向当前选项的活动后代。Home 和 End 仍属于对宿主提供列表的键盘契约。视觉契约采用 overlay 表面、紧凑行以及触发元素焦点恢复；命令执行仍是宿主生命周期的一部分，而不是渲染器副作用。未经验证的外部行为仍标记为 `unknown`，直到运行时捕获提供证据。

<span id="验证" className="velvet-doc-anchor" />

## 验证 {/* #validation */}

在 `apps/desktop` 中运行：

```sh
npm run typecheck
npm run generate:ui-kit-catalog
npm test
npm run qualify:ui-host-projections
```
