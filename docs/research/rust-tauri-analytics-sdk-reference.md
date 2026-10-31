# Rust + Tauri 桌面应用埋点 SDK 参考

取证日期：2026-10-31（Asia/Shanghai）

这份笔记只使用 SDK/服务商的官方文档、官方源码或官方仓库，关注四个能力：离线缓存、批量上报、失败重试和隐私控制。桌面应用的“离线缓存”特指进程退出后仍能恢复的本地持久化队列；仅保存在内存中的 buffer 不算 durable offline cache。

## 结论

首选顺序取决于目标：

1. **PostHog `posthog-rs`**：最适合作为产品分析主 SDK。它是官方 Rust SDK，后台线程/worker 负责批量、重试和 flush，支持 EU endpoint、自托管和 `before_send` 隐私过滤。当前 Rust SDK 的队列是进程内 buffer，没有文档化的磁盘队列，因此要满足“重启后不丢事件”，仍需在 Rust 侧加 outbox。
2. **Aptabase `tauri-plugin-aptabase`**：最适合隐私优先、事件模型简单的 Tauri 应用。它是直接的 Tauri Rust 插件，MIT 开源，默认不自动采集事件，SDK 不收集设备标识符/持久用户 ID，内置批量和网络失败重试；但队列只在内存中，4xx 会丢弃事件，不能单独满足 durable offline。
3. **Matomo Tracking API**：最适合需要自托管、数据留在自己的基础设施、并愿意维护一层 Rust adapter/outbox 的团队。官方 HTTP API 支持 bulk requests，Matomo 侧有 IP 匿名化、无 Cookie、User ID 伪名、保留期和 opt-out 等隐私控制；客户端离线队列和重试要自行实现。
4. **OpenTelemetry Rust + Collector**：适合作为稳定的可观测性底座，而不是产品分析 SDK。Rust SDK 有内存批处理、队列上限、`force_flush`/`shutdown`，可把数据交给 OTLP Collector；默认没有跨重启磁盘队列，也没有产品分析漏斗/留存 UI。
5. **Sentry Rust + Tauri 插件**：用于异常、panic、崩溃和性能信号，与产品埋点互补。Rust SDK 有后台 transport、flush、`before_send` 和 PII 开关，但默认 transport channel 是内存队列，不是持久化离线队列；Tauri 插件是社区维护的 `timfish/sentry-tauri`，应把它视为错误监控集成，而不是产品分析 SDK。

如果“离线缓存”是硬要求，建议把 SDK 当作上游或后端适配器，先在 Rust 中定义 `TelemetryClient` 和本地 outbox，再接入 PostHog、Aptabase 或 Matomo。这样可统一同意状态、队列上限、重试语义、删除/过期和关闭时 flush 行为，也避免让 Vue/React 直接拥有遥测状态。

## 能力对照

| 方案 | Rust/Tauri 集成 | 批量上报 | 失败重试 | 进程退出后离线缓存 | 隐私控制 | 自托管/数据控制 | 适合定位 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| PostHog `posthog-rs` | 官方 Rust SDK；Tauri 中可由 Rust core 调用 | 自动 flush；默认最多 100 条/批 | 有限次数、指数退避；支持 `Retry-After` | **未提供磁盘队列**；仅进程内 buffer | `before_send` 可改写/丢弃；`disabled`；`disable_geoip`；可选 EU host | Cloud EU、Hobby/self-host | 产品分析、漏斗、留存、feature flags |
| Aptabase Tauri plugin | 官方 Aptabase Tauri 插件；Rust 和 JS guest bindings | 每个请求最多 25 条；定时或显式 flush | 网络/超时/5xx 保留到下一次 flush；4xx 丢弃 | **明确不落盘** | 默认无自动采集；不收集设备 ID、Cookie、持久 ID 或存储 IP；短期匿名 session | Cloud EU/US；官方称支持自托管 | 隐私优先的轻量产品分析 |
| Matomo Tracking API | Rust 直接 HTTPS 调用官方 HTTP API；没有一等 Rust SDK | 官方 bulk `requests` JSON | 客户端策略自定；服务端有 queued tracking 但不是桌面客户端离线队列 | **需自建** | IP masking/anonymization、禁 Cookie、User ID 伪名、数据保留、opt-out | On-Premise，自有 MySQL/服务器 | 自托管 web/desktop analytics |
| OpenTelemetry Rust | 官方 Rust SDK；OTLP exporter | `BatchSpanProcessor` | 由 exporter/Collector 负责；SDK本身不提供 durable retry queue | **默认无**；内存 queue | 由应用决定属性、采样和 exporter；无 PII 自动保证 | Collector/backend 可自建 | traces、metrics、logs、性能观测 |
| Sentry Rust + Tauri | 官方 Rust SDK；Tauri bridge 为社区插件 | Envelope/background transport | transport 层处理失败、限流和 flush；默认队列内存 | **Rust SDK 默认无持久离线队列** | `before_send`、`before_breadcrumb`、`send_default_pii=false` | Sentry Cloud；可接近期自托管版本 | crash/error/performance telemetry |
| RudderStack Rust | 官方开源 Rust SDK，但集成成熟度较低 | 支持手工 `Batch` message | `send()` 对 429/5xx/连接/超时重试 | **无本地持久队列**；调用是阻塞 HTTP | 事件字段由调用方控制；需自己做过滤/同意门控 | Data Plane/warehouse 路由 | 需要 CDP 多目的地路由时使用 |

## 逐项证据

### PostHog

- 官方仓库称 `posthog-rs` 是 PostHog 官方 Rust SDK，并采用 MIT 许可证：[PostHog/posthog-rs](https://github.com/PostHog/posthog-rs)。
- Rust 文档说明 `Client::capture`/`capture_batch` 将事件交给后台 worker，worker 负责批量、发送和重试；`flush()` 与 `shutdown()` 可用于退出前排空队列：[crate docs](https://docs.rs/posthog-rs/latest/posthog_rs/)。
- 官方源码公开的默认参数包括：达到 100 条自动 flush、单批最多 100 条、5 秒定时 flush、内存队列上限 10,000、capture 最多 3 次尝试、退避从 200ms 到 30s：[ClientOptions source](https://docs.rs/posthog-rs/latest/src/posthog_rs/client/mod.rs.html)。源码没有磁盘 outbox/persistence 选项，因此不要把它当作跨重启离线保证。
- 隐私方面，Rust SDK 有 `before_send` hook，可在序列化前改写或丢弃事件；还有 `disabled`、`disable_geoip` 等选项。PostHog 的隐私文档明确说明数据收集合规责任仍由应用方承担：[privacy compliance](https://posthog.com/docs/privacy)。
- 可将 host 指向 EU ingestion endpoint 或自托管实例。官方自托管文档说明 Hobby/Docker Compose 版本可自行部署，但部署、升级、扩展和数据恢复由使用方负责，且官方不为自托管提供产品支持：[self-host](https://posthog.com/docs/self-host)。

**判断**：如果团队需要完整的产品分析能力，并愿意在 Rust 中补一个小型持久 outbox，PostHog 是默认推荐。不要直接从前端调用 SDK；由 Rust 统一接收事件、执行同意门控和脱敏。

### Aptabase

- 官方 Tauri 插件提供 Rust `EventTracker` 和 JS guest bindings，安装方式是 `tauri-plugin-aptabase` + `@aptabase/tauri`；仓库为 MIT 许可证：[tauri-plugin-aptabase README](https://github.com/aptabase/tauri-plugin-aptabase)。
- 官方 README 明确说明：事件在内存中排队，release 默认每 60 秒 flush、debug 默认每 2 秒 flush；每次请求最多 25 个事件；网络/超时或 5xx 会留在队列中等待下一次 flush，其他失败状态（例如 4xx）会丢弃；队列不会保存到磁盘：[Delivery](https://github.com/aptabase/tauri-plugin-aptabase#delivery)。
- 插件不会自动采集事件，属性只允许字符串和数字；这对桌面应用的显式埋点和事件白名单很友好：[README usage](https://github.com/aptabase/tauri-plugin-aptabase#usage)。
- Aptabase 官方文档说明 SDK 不发送设备标识符，服务端仅用 IP/User-Agent 与每日轮换 salt 生成短期匿名 hash，IP 本身不保存；也不收集 Cookie、指纹、持久用户标识、姓名或邮箱：[Aptabase docs](https://aptabase.com/docs)。
- 官方自建 SDK 指南明确 API 是 `{host}/api/v0/events`，最大 batch size 为 25，并建议用内存队列处理暂时离线：[build your own SDK](https://github.com/aptabase/aptabase/wiki/How-to-build-your-own-SDK)。

**判断**：如果重点是“少采集、简单、Tauri 原生”，Aptabase 是最省集成成本的选择；如果需要重启恢复、强一致删除或更复杂身份分析，必须补 Rust outbox，且要评估 Aptabase 的匿名模型对留存/MAU 的限制。

### Matomo

- 官方 Tracking API 允许从 C++、Python、C# 等客户端直接调用，也可直接使用 HTTPS；bulk tracking 通过 POST 一个包含 `requests` 数组的 JSON 对象完成：[Tracking API reference](https://developer.matomo.org/api-reference/tracking-api)。
- 官方隐私指南支持 IP masking/anonymization、完全移除 IP、禁用 Cookie、User ID 伪名、删除旧 raw data 和 opt-out；Matomo On-Premise 的数据保存在自己的 MySQL，不发送到第三方：[privacy settings](https://matomo.org/faq/general/configure-privacy-settings-in-matomo/)、[privacy overview](https://matomo.org/privacy/)。
- Matomo 的 queued tracking 是服务端在 Redis/MySQL 中排队以应对流量峰值，不等于桌面端本地离线缓存。桌面端应在 Rust 侧保存事件，再按 bulk API 上报。

**判断**：Matomo 是数据主权优先的选择。它最适合已经有服务器运维能力的团队；Rust adapter 应明确设置匿名/无 Cookie参数，避免把文件路径、账号邮箱、搜索词等直接作为 URL、title 或 custom dimension。

### OpenTelemetry Rust

- `opentelemetry_sdk` 的 `BatchSpanProcessor` 会把完成的 spans 放入内存 buffer，按 queue size、batch size 和 scheduled delay 导出，并支持 `force_flush` 与 shutdown：[BatchSpanProcessor docs](https://docs.rs/opentelemetry_sdk/latest/opentelemetry_sdk/trace/struct.BatchSpanProcessor.html)。
- 当前默认值是 queue 2048、batch 512、5 秒 scheduled delay；这类 queue 是进程内的吞吐优化，不是跨重启 durable storage：[SDK defaults](https://docs.rs/opentelemetry_sdk/latest/opentelemetry_sdk/)。
- 它是标准化遥测管道，适合把 Rust backend 和 Tauri renderer 的 traces/metrics/logs 汇聚到 Collector，再由 Collector 输出到后端。产品事件仍需在事件命名、同意、脱敏和留存上定义自己的契约。

**判断**：若目标是“埋点 + 性能/错误关联 + 可替换后端”，可将 OTel 作为第二条信号通道；不要把它单独当作产品分析平台。

### Sentry

- `sentry-rust` 是官方 Rust SDK；官方仓库说明它支持事件、错误、panic 以及 tracing 集成：[getsentry/sentry-rust](https://github.com/getsentry/sentry-rust)。
- `ClientOptions` 默认提供后台 transport channel，可设置 channel capacity、`shutdown_timeout`、`before_send`、`before_breadcrumb` 和 `send_default_pii`；文档说明 channel 满时会丢弃新 envelope，默认容量当前为 30：[ClientOptions](https://docs.rs/sentry/latest/sentry/struct.ClientOptions.html)。这说明它是内存缓冲和失败隔离模型，不是可靠的本地事件日志。
- `timfish/sentry-tauri` 是社区 Tauri v2 插件，它把 webview 的 browser events/breadcrumbs 通过 Tauri IPC 转给 Rust SDK，并与 Rust/OS/device context 合并：[sentry-tauri](https://github.com/timfish/sentry-tauri)。

**判断**：Sentry 应与产品分析并用，用于 crash/error 证据和 release health；不要用它替代产品事件分析，也不要把它的内存 transport 当作离线事件保证。

### RudderStack Rust

- 官方 Rust SDK 支持 Track/Identify/Page/Screen/Group/Alias 和 `Batch` 消息，并将事件发送到 RudderStack Data Plane：[rudder-sdk-rust](https://github.com/rudderlabs/rudder-sdk-rust)。
- 官方 README 说明 `send()` 对 HTTP 429、5xx、连接错误和超时做有界指数退避重试，并尊重 `Retry-After`；测试覆盖这些重试场景：[retry behavior](https://github.com/rudderlabs/rudder-sdk-rust#retry-behavior)。
- 源码显示 `send()` 使用 `reqwest::blocking::Client` 直接发请求，单事件调用为阻塞路径；虽然协议有 `Batch` 类型，但没有客户端本地持久队列或后台 worker：[client source](https://raw.githubusercontent.com/rudderlabs/rudder-sdk-rust/master/src/client.rs)。

**判断**：仅当需要把一套事件路由到多个下游目的地时考虑它。对 Tauri 桌面应用的离线和 UI 解耦要求，它需要和 Matomo 类似的 Rust outbox 包装层；当前仓库活跃度和生态规模也明显小于 PostHog/Matomo。

## 推荐的 Rust/Tauri 接入形状

建议在 `ApplicationCore` 所在的 Rust 层持有一个小的 `TelemetryClient` seam，前端只发已命名的 typed command，不直接拥有 SDK client。实现可以分成：

1. `Event`：`event_id`、`occurred_at`、稳定事件名、有限的 JSON scalar properties、应用版本和 schema version。
2. `ConsentGate`：初始化前默认关闭或按产品策略开启；用户 opt-out 时停止入队并清理尚未上报的队列；不能把账号邮箱、完整文件路径、原始搜索词或令牌当作属性。
3. `Outbox`：使用现有 persistence owner（例如 SQLite/JSONL），有容量上限、事件 TTL、schema migration、崩溃恢复和显式 clear；记录 `attempts`、`next_attempt_at`、最后一次 HTTP 分类和 provider。
4. `DeliveryWorker`：按 FIFO/大小组 batch；网络错误、408、429、5xx 重试并带 jitter；4xx/鉴权/协议错误标为 terminal，避免无限重试；成功按 `event_id` 确认，避免并发 flush 时误删新事件。
5. `ProviderAdapter`：PostHog/Aptabase/Matomo/OTLP/Sentry 各自只负责协议映射；adapter 失败不能影响核心资产、布局或工作区状态。
6. 生命周期：定时 flush、网络恢复触发 flush、退出时在有界时间内 flush；flush 失败只记录诊断状态，不阻塞用户动作。

这样可以把“是否采集”和“是否成功送达”分开建模：用户同意控制入队，outbox 负责可靠性，provider 只负责远端协议。这个边界也方便后续替换 SaaS、切换自托管或在本地部署 Collector。

## 适合直接落地的选择

- **默认产品分析**：`posthog-rs` + Rust durable outbox；使用 EU endpoint 或自托管，默认 `before_send` 脱敏和 `disable_geoip`。
- **隐私优先 MVP**：`tauri-plugin-aptabase`；如果暂时接受进程级 best-effort，可不加 outbox；一旦“重启不丢”成为验收条件，再把 outbox 放在插件前面。
- **自托管/合规优先**：Matomo Tracking API + Rust bulk adapter + SQLite/JSONL outbox。
- **运行质量观测**：Sentry Rust/Tauri 负责 crash/error，OpenTelemetry 负责 traces/metrics/logs；两者都不应承担产品事件 outbox 的所有权。
