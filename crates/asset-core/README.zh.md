# asset-core crate

## 职责

Stable Asset identity and value objects: AssetId, ContentKindId, ContentFingerprint and AssetRef.

## 非目标

It does not own source paths, databases, indexing, previews, facets or persistence.

## 稳定性

High-stability leaf contract. New validation or fields require compatibility review because protocol and persistence consume these types.

## 输入和输出

输入：Validated or caller-provided identity strings.
输出：AssetId, ContentKindId, ContentFingerprint and AssetRef.

## 状态所有权

No durable state. Asset/source owners live above this crate.

## 依赖方向

No workspace dependencies.

## 公共接口

AssetId, ContentKindId, ContentFingerprint and AssetRef.

## 模块图

Single small facade is intentional; do not split until Asset domain has a second independent seam.

## 测试入口

cargo test -p velvet-asset-core; current crate has no behavior-heavy tests.

## 失败边界

This crate currently has no I/O or runtime failure boundary. Format validation remains a future domain decision.

## 迁移注意事项

Do not add source paths or extension data. A validated constructor must preserve existing wire and tuple-shape compatibility.

本 README 描述当前实现边界。Proposed ADR 是设计方向，不代表未来能力已经完成。
