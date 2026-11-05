# plugin-api crate

## 职责

External extension contract: manifest, contribution descriptors, permissions, Preview/UI descriptors and install projections.

## 非目标

It does not discover, install, activate, persist or execute extensions.

## 稳定性

Potential public SDK seam. MANIFEST_SCHEMA_VERSION and public serde shapes require fixture and compatibility review.

## 输入和输出

输入：Manifest JSON and host/version declarations.
输出：ExtensionManifest、ManifestValidationError、ExtensionId、VersionRange、ActivationEvent、Permission、贡献描述和安装/恢复 DTO。

## 状态所有权

No durable state. PluginHost owns manifest provenance, lifecycle and extension data.

## 依赖方向

layout-model plus serde/serde_json.

## 公共接口

ExtensionManifest、ManifestValidationError、ExtensionId、VersionRange、ActivationEvent、Permission、贡献描述和安装/恢复 DTO。

## 模块图

Target modules are version, manifest, contribution, permissions, preview, ui and install; the facade remains the stable export point.

## 测试入口

Manifest schema、未知字段、标识符/引用校验、版本和描述往返测试；cargo test -p velvet-plugin-api。

## 失败边界

Decode and local validation errors are typed. Runtime, trust and storage failures belong to PluginHost.

## 迁移注意事项

不要把 RuntimeKind 或 Proposed plugin ADR 当作生产运行时已存在的证明。未知必填字段必须拒绝。详见 [plugin manifest schema 1](../../docs/engineering/plugin-manifest-v1.md)。

本 README 描述当前实现边界。Proposed ADR 是设计方向，不代表未来能力已经完成。
