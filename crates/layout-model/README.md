# layout-model crate

## Responsibility

Own Workbench Parts, semantic Zones, View and ViewContainer state, constraints, commands and layout migrations.

## Non-goals

It does not persist files, render React/Vue components or own product Asset data.

## Stability

LayoutState, LayoutCommand, LayoutService and LayoutError are the stable seam. InMemoryLayoutService is an in-memory adapter.

## Inputs and outputs

Inputs: Typed LayoutCommand and prior LayoutState.
Outputs: WorkbenchAppId, WorkbenchZone, WorkbenchPart, LayoutState, LayoutCommand, LayoutService and InMemoryLayoutService.

## State ownership

LayoutService owns layout semantics and state; persistence only stores snapshots.

## Dependency direction

serde only.

## Public interface

WorkbenchAppId, WorkbenchZone, WorkbenchPart, LayoutState, LayoutCommand, LayoutService and InMemoryLayoutService.

## Module map

Target private modules are parts, zones, layout_state, commands, migrations and service; keep the facade re-exports.

## Tests

Inline layout constraint, ordering, scope and migration tests; cargo test -p velvet-layout-model.

## Failure boundary

Invalid commands and future schemas return typed errors without writing persistence.

## Migration notes

Preserve serde names, schema versions and public re-exports while moving implementation into owner-named modules.

This README describes the current implementation boundary. Proposed ADRs are design direction, not evidence that a future capability is complete.
