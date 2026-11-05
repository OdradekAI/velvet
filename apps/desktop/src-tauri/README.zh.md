# velvet-desktop

## 职责

独立 Tauri workspace 的桌面组合根，负责 native startup、wire mapping、capability、recovery、activation 和 update 装配。

## 非目标

不拥有领域持久化、Preview 选择、插件运行时内部实现或 renderer 业务状态。

## 稳定性

run 入口和 typed wire/command seam 是稳定的 native 集成点。该 workspace 有意与根 Cargo workspace 分离。

## 输入和输出

输入是 Tauri commands/events、native 平台证据以及发往 TauriTransport 的 typed request。输出是 typed wire response/event 和 native 生命周期证据。

## 状态所有权

native startup/recovery/update 状态由桌面宿主持有；产品 durable state 仍由 ApplicationCore 及其领域 owner 持有。

## 依赖方向

它依赖根 workspace 的核心 crate 和 Tauri 插件。command handler 只调用 TauriTransport，不直接调用 persistence、Preview 或 PluginHost 内部实现。

## 公共接口

run、velvet-desktop binary、velvet-launcher 和薄的 typed command handler。

## 模块图

crash_capture、instance、capability_policy、launcher_failure、native_recovery_window、platform_probe、release_policy 和 update_coordinator 是按 owner 命名的模块。根 lib 保持组合根；未来 wire、commands、startup、recovery、activation、update 拆分仍应是内部模块。

## 测试入口

使用 cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml 单独验证，不包含在根 workspace 测试中。native lifecycle 和 capability fixture 位于此 workspace。

## 失败边界

native startup、capability 和 update failure 必须有界且可诊断。command failure 不能绕过 CoreTransport 或破坏 ApplicationCore 状态。

## 迁移注意事项

保留 command name、wire JSON、capability policy 和 launcher/recovery 文件格式。不新增通用 common、shared 或 utils 目录。
