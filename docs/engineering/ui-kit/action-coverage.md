# UI Kit action coverage

Action coverage is part of the component contract. A Node/Happy DOM story
declares its semantic actions in `scenario.covers`; actions that require
portal, pointer geometry or native focus behavior are declared as
`browserActions` and executed by the browser qualification fixture. The
contract test rejects an action that appears in neither set.

| Component | Node story coverage | Browser coverage |
| --- | --- | --- |
| VelvetButton | `activate` — `button/default` | — |
| VelvetIconButton | `activate` — `icon-button/default` | — |
| VelvetInput | `change` — `input/change` | — |
| VelvetSelect | `change` — `select/default` | — |
| VelvetCheckbox | `toggle` — `checkbox/checked` | — |
| VelvetState | `retry` — `state/error-retry` | `action` is a content slot, not a command |
| VelvetTabs | `select`, `navigate` — `tabs/default` | — |
| VelvetToastHost | `dismiss` — `toast/default` | — |
| VelvetActivityBar | `select`, `navigate` — `activity-bar/default` | — |
| VelvetDialog | — | `open`, `close`, `submit` — overlay fixture |
| VelvetContextMenu | — | `open`, `select`, `dismiss` — overlay fixture |
| VelvetPreviewHost | `retry` — `preview-host/retry` | — |
| VelvetInspector | `pin`, `open`, `edit`, `save`, `retry` — Inspector stories | pin/edit/save in host fixture |
| VelvetAssetGrid | `select`, `navigate`, `open`, `drag`, `drop` — AssetGrid stories (including Shift range hint) | keyboard/open/Shift selection in host fixture |
| VelvetDataTable | `select`, `sort` — `data-table/default` | keyboard/sort in host fixture |
| VelvetCommandPalette | `select`, `navigate`, `close` — `command-palette/open` | `query` plus focus restoration in host fixture |
| VelvetZoomControl | `increment`, `decrement`, `change` — `zoom-control/default` | keyboard step, bounds and button actions in Eagle fixture |

Run the complete action matrix with:

```sh
npm run verify:ui-kit
```

The JSON agent catalog exposes the browser-only portion as `browserActions`;
the typed story registry remains the source for `scenario.covers`.
