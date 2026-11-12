# UI Kit action coverage

Action coverage is part of the component contract. A Node/Happy DOM story
declares its semantic actions in `scenario.covers`; actions that require
portal, pointer geometry or native focus behavior are declared as
`browserActions` and executed by the browser qualification fixture. The
contract test rejects an action that appears in neither set.

| Component | Node story coverage | Browser coverage |
| --- | --- | --- |
| VelvetButton | `activate` — `button/default` | native activation and busy semantics in primitives fixture |
| VelvetIconButton | `activate` — `icon-button/default` | accessible label and activation in primitives fixture |
| VelvetInput | `change` — `input/change` | native value/change semantics in primitives fixture |
| VelvetSelect | `change` — `select/default` | native value/change semantics in primitives fixture |
| VelvetCheckbox | `toggle` — `checkbox/checked` | associated label and checked state in primitives fixture |
| VelvetState | `retry` — `state/error-retry` | alert live region and retry in primitives fixture |
| VelvetTabs | `select`, `navigate` — `tabs/default` | tablist orientation, ArrowRight selection and focus in primitives fixture |
| VelvetToastHost | `dismiss` — `toast/default` | polite live region and dismissal in primitives fixture |
| VelvetActivityBar | `select`, `navigate` — `activity-bar/default` | keyboard selection, disabled-item skip and focus movement in host fixture |
| VelvetDialog | — | `open`, `close`, `submit` — overlay fixture |
| VelvetContextMenu | — | `open`, `select`, `dismiss`, `navigate` — overlay fixture (including submenu) |
| VelvetInspector | `pin`, `open`, `edit`, `save`, `retry`, `toggle-section` — Inspector stories | pin/edit/save in host fixture; grouped fields, landscape/portrait/no-preview/multiple selection branches, palette, 200px preview region and section collapse in Node/browser fixtures |
| VelvetPreviewHost | `retry`, `change-mode`, `provider-action`, `open-external`, `close` — `preview-host/main-window-lifecycle` and async stories | error retry, mode/provider/external actions, open/loading/ready/failure/retry/close lifecycle, landscape/portrait aspect ratios and no-preview fallback in Playwright and CUA fixtures |
| VelvetAssetGrid | `select`, `navigate`, `open`, `drag`, `drop` — AssetGrid stories (including grid metrics, Shift range hint and waterfall geometry) | keyboard/open/Shift selection and grid semantics in host fixture |
| VelvetDataTable | `select`, `sort` — `data-table/default`, `data-table/sort-direction`; empty status in `data-table/empty` | keyboard row selection, sort direction and `aria-sort` in host fixture |
| VelvetCommandPalette | `query`, `select`, `navigate`, `scope-change`, `close` — `command-palette/open`, `command-palette/scope`, `command-palette/controlled-query`, `command-palette/host-results` | host result replacement, query callback, selection/navigation/scope/close plus active descendant and focus restoration in host fixture |
| VelvetQuickSearch | `query`, `select`, `navigate`, `tab-change`, `history-select`, `close` — `quick-search/open`, `quick-search/tabs-history`, `quick-search/keyboard-focus` | host result/history projection, tab switching, selection/navigation and focus restoration in host fixture |
| VelvetFilterPanel | `change`, `apply`, `reset`, `save`, `close` — `filter-panel/default`, `filter-panel/reset-save`, `filter-panel/keyboard-close` | text rule, typed operators, result count, apply/reset/save and close in host fixture |
| VelvetFilterFacetMenu | `query`, `folder-rule`, `folder-include`, `folder-exclude`, `color-select`, `color-hex`, `color-accuracy`, `clear`, `close` — `filter-facet-menu/folder-default`, `filter-facet-menu/color-default` | pointer include/exclude, geometry, focus entry, Escape dismissal and trigger restoration in host fixture |
| VelvetZoomControl | `increment`, `decrement`, `change` — `zoom-control/default` | keyboard step and button actions in the visual-baseline fixture; bounds/disabled states have render-only Node stories |
| VelvetPane | semantic region checkpoint — `pane/navigation` | role, heading link, zone marker and body in host fixture |
| VelvetPanel | semantic region checkpoint — `panel/panel` | role, heading link, surface and body in host fixture |

Run the complete action matrix from `apps/desktop`:

```sh
npm run verify:ui-kit
```

The JSON agent catalog exposes the browser-qualified portion as `browserActions`;
the typed story registry remains the source for `scenario.covers`.
