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
| VelvetContextMenu | — | `open`, `select`, `dismiss` — overlay fixture |
| VelvetPreviewHost | `retry` — `preview-host/retry` | error announcement, retry and ready transition in host fixture |
| VelvetInspector | `pin`, `open`, `edit`, `save`, `retry` — Inspector stories | pin/edit/save in host fixture |
| VelvetAssetGrid | `select`, `navigate`, `open`, `drag`, `drop` — AssetGrid stories (including grid metrics and Shift range hint) | keyboard/open/Shift selection and grid semantics in host fixture |
| VelvetDataTable | `select`, `sort` — `data-table/default` | keyboard/sort in host fixture |
| VelvetCommandPalette | `select`, `navigate`, `close` — `command-palette/open` | query/select/navigation/close plus focus restoration in host fixture |
| VelvetFilterPanel | `change`, `reset`, `save`, `close` — `filter-panel/default`, `filter-panel/reset-save`, `filter-panel/keyboard-close` | text rule, reset/save and close in host fixture |
| VelvetZoomControl | `increment`, `decrement`, `change` — `zoom-control/default` | keyboard step, bounds and button actions in the visual-baseline fixture |
| VelvetPane | semantic region checkpoint — `pane/navigation` | role, heading link, zone marker and body in host fixture |
| VelvetPanel | semantic region checkpoint — `panel/panel` | role, heading link, surface and body in host fixture |

Run the complete action matrix with:

```sh
npm run verify:ui-kit
```

The JSON agent catalog exposes the browser-only portion as `browserActions`;
the typed story registry remains the source for `scenario.covers`.
