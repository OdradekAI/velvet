# layout-model crate

## 职责

Own Workbench Parts, semantic Zones, View and ViewContainer state, constraints, commands and layout migrations.

## 非目标

It does not persist files, render React/Vue components or own product Asset data.

## 稳定性

LayoutState, LayoutCommand, LayoutService and LayoutError are the stable seam. InMemoryLayoutService is an in-memory adapter.

## 输入和输出

输入：Typed LayoutCommand and prior LayoutState.
输出：WorkbenchAppId, WorkbenchZone, WorkbenchPart, LayoutState, LayoutCommand, LayoutService and InMemoryLayoutService.

## 状态所有权

LayoutService owns layout semantics and state; persistence only stores snapshots.

## 依赖方向

serde only.

## 公共接口

WorkbenchAppId, WorkbenchZone, WorkbenchPart, LayoutState, LayoutCommand, LayoutService and InMemoryLayoutService.

## 模块图

Target private modules are parts, zones, layout_state, commands, migrations and service; keep the facade re-exports.

## 测试入口

Inline layout constraint, ordering, scope and migration tests; cargo test -p velvet-layout-model.

## 失败边界

Invalid commands and future schemas return typed errors without writing persistence.

## 迁移注意事项

Preserve serde names, schema versions and public re-exports while moving implementation into owner-named modules.

本 README 描述当前实现边界。Proposed ADR 是设计方向，不代表未来能力已经完成。
