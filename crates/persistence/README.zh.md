# persistence crate

## 职责

Storage adapters for bounded JSON/file/in-memory snapshots, RecoverySet, durability, journals, owner stores, releases and quarantine.

## 非目标

It does not interpret domain semantics, own layout/Job/Profile/Plugin state or expose raw paths to extensions.

## 稳定性

StateSnapshotStore, RecoverySetStore, owner store traits and durability types are the adapter seam.

## 输入和输出

输入：Owner records, snapshots, store commands and durability settings.
输出：Snapshot, recovery, durability, journal, operation, release and quarantine stores/adapters.

## 状态所有权

Each domain owner remains authoritative; persistence owns storage mechanics and generations only.

## 依赖方向

job-system, layout-model and a compatibility dependency on workbench-protocol.

## 公共接口

Snapshot, recovery, durability, journal, operation, release and quarantine stores/adapters.

## 模块图

Existing owner-named modules remain; root implementation should continue moving toward journal, owner_records, recovery_set, release, quarantine and durability modules.

## 测试入口

Atomic stage/commit, checksum, corruption, RecoverySet and release qualification tests; cargo test -p velvet-persistence.

## 失败边界

Read-only, corruption, partial commit and checksum failures preserve prior generations and never silently overwrite authority.

## 迁移注意事项

The protocol dependency is a known compatibility edge. Remove it only through owner-specific record conversion and an ADR/fixture, not in a facade move.

本 README 描述当前实现边界。Proposed ADR 是设计方向，不代表未来能力已经完成。
