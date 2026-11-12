---
id: action-coverage
title: UI Kit action coverage
---

# UI Kit 动作覆盖

<span id="ui-kit-action-coverage" className="velvet-doc-anchor" />

动作覆盖是组件契约的一部分。Node/Happy DOM story 在 `scenario.covers` 中声明语义动作；需要 portal、指针几何或原生焦点行为的动作声明为 `browserActions`，由浏览器资格验证夹具执行。若某动作不在这两个集合中的任何一个，契约测试就会拒绝它。

| 组件 | Node story 覆盖 | 浏览器覆盖 |
| --- | --- | --- |
| VelvetButton | `activate` — `button/default` | 基础组件夹具中的原生激活和 busy 语义 |
| VelvetIconButton | `activate` — `icon-button/default` | 基础组件夹具中的无障碍名称和激活 |
| VelvetInput | `change` — `input/change` | 基础组件夹具中的原生 value/change 语义 |
| VelvetSelect | `change` — `select/default` | 基础组件夹具中的原生 value/change 语义 |
| VelvetCheckbox | `toggle` — `checkbox/checked` | 基础组件夹具中的标签关联和选中状态 |
| VelvetState | `retry` — `state/error-retry` | 基础组件夹具中的 alert 动态播报区域和重试 |
| VelvetTabs | `select`、`navigate` — `tabs/default` | 基础组件夹具中的 tablist 方向、ArrowRight 选择和焦点 |
| VelvetToastHost | `dismiss` — `toast/default` | 基础组件夹具中的 polite 动态播报区域和关闭 |
| VelvetActivityBar | `select`、`navigate` — `activity-bar/default` | 宿主夹具中的键盘选择、跳过禁用条目和焦点移动 |
| VelvetDialog | — | `open`、`close`、`submit` — 浮层夹具 |
| VelvetContextMenu | — | `open`、`select`、`dismiss`、`navigate` — 浮层夹具（包括子菜单） |
| VelvetInspector | `pin`、`open`、`edit`、`save`、`retry`、`toggle-section` — Inspector stories | 宿主夹具中的 pin/edit/save；Node/浏览器夹具中的分组字段、横向/纵向/无预览/多选分支、调色板、200px 预览区和分组折叠 |
| VelvetPreviewHost | `retry`、`change-mode`、`provider-action`、`open-external`、`close` — `preview-host/main-window-lifecycle` 和异步 story | Playwright 和 CUA 夹具中的错误重试、模式/提供者/外部打开动作、open/loading/ready/failure/retry/close 生命周期、横向与纵向宽高比以及无预览降级 |
| VelvetAssetGrid | `select`、`navigate`、`open`、`drag`、`drop` — AssetGrid stories（包括网格指标、Shift 范围提示和瀑布流几何） | 宿主夹具中的键盘操作、打开、Shift 选择和网格语义 |
| VelvetDataTable | `select`、`sort` — `data-table/default`、`data-table/sort-direction`；`data-table/empty` 中的空状态 | 宿主夹具中的键盘行选择、排序方向和 `aria-sort` |
| VelvetCommandPalette | `query`、`select`、`navigate`、`scope-change`、`close` — `command-palette/open`、`command-palette/scope`、`command-palette/controlled-query`、`command-palette/host-results` | 宿主夹具中的宿主结果替换、查询回调、选择/导航/范围/关闭，以及活动后代和焦点恢复 |
| VelvetQuickSearch | `query`、`select`、`navigate`、`tab-change`、`history-select`、`close` — `quick-search/open`、`quick-search/tabs-history`、`quick-search/keyboard-focus` | 宿主夹具中的结果/历史投影、标签页切换、选择/导航和焦点恢复 |
| VelvetFilterPanel | `change`、`apply`、`reset`、`save`、`close` — `filter-panel/default`、`filter-panel/reset-save`、`filter-panel/keyboard-close` | 宿主夹具中的文本规则、类型化运算符、结果数量、apply/reset/save 和关闭 |
| VelvetFilterFacetMenu | `query`、`folder-rule`、`folder-include`、`folder-exclude`、`color-select`、`color-hex`、`color-accuracy`、`clear`、`close` — `filter-facet-menu/folder-default`、`filter-facet-menu/color-default` | 宿主夹具中的指针 include/exclude、几何尺寸、焦点进入、Escape 关闭和触发元素焦点恢复 |
| VelvetZoomControl | `increment`、`decrement`、`change` — `zoom-control/default` | 视觉基线夹具中的键盘步进和按钮动作；边界与禁用状态在 Node story 中仅有渲染覆盖 |
| VelvetPane | 语义区域检查点 — `pane/navigation` | 宿主夹具中的角色、标题关联、区域标记和正文 |
| VelvetPanel | 语义区域检查点 — `panel/panel` | 宿主夹具中的角色、标题关联、表面和正文 |

在 `apps/desktop` 中运行以下命令执行完整动作矩阵：

```sh
npm run verify:ui-kit
```

JSON 智能体目录以 `browserActions` 暴露浏览器覆盖部分；`scenario.covers` 仍以类型化 story 注册表为准。
