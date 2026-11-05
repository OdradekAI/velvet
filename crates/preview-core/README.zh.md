# preview-core crate

## 职责

Preview intent/context, bounded source seam, provider selection, PreviewResolver, sessions and generic fallback.

## 非目标

It does not own Asset source locations, plugin runtime processes, UI framework components or durable preview caches.

## 稳定性

PreviewProvider, PreviewAssetSource, PreviewCandidate, PreviewSession and PreviewResolver are the public seam.

## 输入和输出

输入：PreviewRequest, PreviewContext and a bounded PreviewAssetSource.
输出：PreviewIntent, PreviewSurface, PreviewContext, PreviewProvider, PreviewResolver, built-in providers and fallback types.

## 状态所有权

PreviewResolver owns provider registry and resolution state; source ownership remains in ApplicationCore/source adapters.

## 依赖方向

asset-core and serde.

## 公共接口

PreviewIntent, PreviewSurface, PreviewContext, PreviewProvider, PreviewResolver, built-in providers and fallback types.

## 模块图

Target modules are intent/context, source, provider, resolver and builtin/fallback.

## 测试入口

Provider matching, bounded reads, provider failure fallback and surface tests; cargo test -p velvet-preview-core.

## 失败边界

Provider failure becomes a typed error and may fall back to generic preview; paths and credentials do not cross the seam.

## 迁移注意事项

Keep generic fallback usable while plugin preview contracts remain provisional.

本 README 描述当前实现边界。Proposed ADR 是设计方向，不代表未来能力已经完成。
