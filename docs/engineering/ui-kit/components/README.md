# Velvet UI Kit component manuals

Each migrated component has three linked artifacts:

1. the implementation module under `apps/desktop/src/app/ui/`;
2. the typed contract and executable stories;
3. this human-facing manual, which explains usage, ownership, lifecycle and
   evidence.

The first wave covers primitives:

| Component | Contract | Story | Manual |
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

The generated catalog at
`apps/desktop/src/app/ui/ui-kit-catalog.json` links each component id to its
contract and manual. Overlay and host-projection contracts, stories and
implementations are split into their own files. All primitives now carry a
browser qualification tier: the dedicated fixture checks native control
semantics, live regions, labels, layout wrappers and Tabs focus movement. Toast
uses the same fixture for live-region dismissal. Dialog and Context Menu require
the real-browser overlay qualification tier; AssetGrid, DataTable, CommandPalette
and Inspector use the host-projection keyboard/focus/edit qualification fixture.
Dialog and Context Menu use the overlay browser qualification fixture for
submit/select/cancel, portal and focus behavior. Run
`npm run qualify:ui-browser` from `apps/desktop` to execute all three fixtures
at the canonical 1280×800 viewport, or use the full `npm run verify:ui-kit`
acceptance gate. PreviewHost also owns the async
lifecycle story matrix for success, cancellation, timeout, failure and retry;
those deterministic scenarios run in the Node/Happy DOM story runner.
AssetGrid's layout and drag/drop surface is backed by the versioned Eagle
directive evidence matrix at `docs/engineering/ui-kit-eagle-directive-evidence.md`.
Inspector is the first host projection extracted from that matrix; its
selection and metadata contract lives in `host-projections/inspector.md`.
`VelvetZoomControl` is the host-owned slider projection backed by the Eagle
runtime step probe; its full range and persistence semantics remain explicitly
unknown.
The complete action-to-story/browser mapping is maintained in
`../action-coverage.md`.
