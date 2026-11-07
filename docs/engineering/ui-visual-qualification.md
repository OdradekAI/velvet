# UI visual qualification

Velvet uses a real-browser comparison loop to keep its host UI Kit visually
coherent with the observed Eagle grammar. The loop is a qualification aid, not
an attempt to reproduce Eagle's private assets or pixel output.

## Reference order

Use evidence in this order:

1. The installed Eagle application at the same viewport, captured through REA
   native UI observation or a declared native UI scenario.
2. Eagle's official interface documentation when a runtime state is not
   available. Documentation screenshots are secondary visual references and
   their text is treated as product guidance, not as Velvet behavior.
3. Velvet's semantic contract and accessibility requirements when the reference
   leaves behavior unspecified.

Every comparison note labels each claim as `observed`, `inferred` or
`unknown`. Unknown Eagle behavior stays an explicit gap; it is not filled by
guessing.

## Capture contract

The canonical viewport is 1280×800 CSS pixels, dark theme, en-US, UTC and
reduced motion. A browser fixture must expose deterministic story states and a
representative Workbench shell at a stable local URL. Each capture records:

- a screenshot;
- the accessibility tree and visible DOM landmarks;
- computed styles for host surfaces, separators, text, controls, selection and
  focus rings;
- bounding boxes for the Activity Bar, navigation, workspace, Inspector and
  Operations zones; and
- the story/component id and fixture state.

Eagle captures record the same viewport dimensions when the native window allows
it, plus the AX tree and selected-window screenshot returned by REA. Official
documentation captures keep the source URL and screenshot path alongside the
comparison note.

## Comparison rubric

Compare semantic regions before individual controls:

| Priority | Mismatch class | Acceptance signal |
| --- | --- | --- |
| P0 | topology and ownership | three-pane roles, persistent Inspector, toolbar/grid/empty-state placement are present and stable |
| P1 | shared visual tokens | surface hierarchy, separator weight, typography scale, density, accent and selection treatment agree across at least two components |
| P1 | component geometry | control heights, pane padding and selected-row radii stay within ±1 CSS px of the fixture contract; text wrapping may vary by ±2 px |
| P2 | interaction states | hover, focus, selected, disabled, busy, keyboard navigation and overlay dismissal satisfy the component metadata and contract tests |
| P2 | content-specific detail | icon glyphs, image treatment and wording are only changed when the Velvet contract or an observed host requirement calls for them |

Do not make a token change for a one-off mismatch. A shared token change needs
two or more affected components or corroborating Eagle/runtime and official-doc
evidence. Otherwise change the owning wrapper or fixture state.

## Correction loop

For each mismatch class:

1. Capture Velvet and the relevant Eagle/docs reference at the canonical
   viewport.
2. Record the mismatch with evidence ids, screenshot paths, affected component
   ids and priority.
3. Use REA static/runtime evidence to test the cause (for example, a host token
   versus a browser default, an overlay primitive boundary or a shell layout
   rule).
4. Patch one semantic token or wrapper boundary at a time.
5. Run the narrow component/story/adoption tests, then recapture the same state.
6. Keep the before/after screenshots and mark the mismatch resolved only when
   geometry, computed styles and required interaction states all pass.

The route stops when the P0 topology gaps are closed, P1 token and geometry
differences are either within the contract or documented as intentional, and
the P2 interaction matrix is green in the real browser for Radix-backed
overlays.

## Fixture command

Start the Vite renderer from `apps/desktop`, then open the deterministic fixture
at `http://127.0.0.1:5173/?fixture=eagle`. The fixture can be captured with
`npm run capture:ui-visual`; it sets a 1280×800 browser viewport, writes the
screenshot to `.scratch/velvet-ui-kit-and-plugin-boundaries/research/velvet-eagle-fixture.png`,
and prints the probe rectangles and computed styles used by the comparison
record. The command uses the installed `ego-browser` CLI; if it is unavailable,
the URL remains directly inspectable in any Chromium browser at the same
viewport.

Radix-backed overlays use `npm run qualify:ui-overlays`. It exercises Dialog
submit, cancel-button, Escape/focus restoration and Context Menu selection,
Escape cancellation and outside dismissal, then saves overlay screenshots
under the same research directory. The combined browser gate is
`npm run qualify:ui-browser`; the full UI Kit acceptance gate is
`npm run verify:ui-kit`.

## First observed baseline

The current Velvet browser baseline already has a dense dark shell with thin
separators and a persistent auxiliary pane. The fixture now exposes an
Eagle-like asset toolbar/grid state, selected asset and Inspector so geometry and
selection evidence can be captured deterministically. The current Eagle runtime
and official sidebar documentation both show a left navigation surface, a
compact toolbar/search row, a central image grid and a right Inspector; those
observations define the first P0 comparison target.

The first P1 correction pass is recorded in
`.scratch/velvet-ui-kit-and-plugin-boundaries/research/ui-visual-diff-2026-11-06.md`.
It aligns compact typography, landscape asset tiles, selection emphasis,
toolbar/facet surfaces and the CSS fallback values with the canonical semantic
theme snapshot.

The second comparison pass is recorded in
`.scratch/velvet-ui-evidence-alignment/research/eagle-visual-diff-2026-11-07.md`.
It replaces the fixture's status-like facet badges with compact icon/label
actions and equalizes the navigation and Inspector side zones. The native Eagle
probe then established a 5-point zoom step from the observed `180 → 505 → 500`
sequence; `VelvetZoomControl` now owns that bounded, controlled slider contract.
Eagle's full range, persistence and grid reflow semantics remain unknown.
