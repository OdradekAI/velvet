# job-system crate

## 职责

由 Rust 持有 Job 生命周期、runner 注册、取消、重试、deadline、budget、checkpoint、provenance 和 state-store seam。

## 非目标

不拥有插件运行时进程、UI progress state、文件路径或格式专用后台循环。

## 稳定性

JobSystem、JobSpec、JobSnapshot、JobRunner、JobExecutionContext 和 JobStateStore 是公共 seam。内部调度和取消实现可以在 facade 后移动。

## 输入和输出

输入是 JobSpec、runner、执行策略、budget request 和 execution environment。输出是 JobSnapshot、JobResult、JobError、progress event 和 state-store 写入。

## 状态所有权

JobSystem 拥有 Job 生命周期和内存状态；JobStateStore 持久化 snapshot；调用者和插件只获得 handle 或 projection。

## 依赖方向

本 crate 只依赖 serde 和 serde_json。persistence 与 ApplicationCore 通过 store seam 提供适配器。

## 公共接口

JobId、JobStatus、JobSpec、JobSnapshot、JobRunner、JobExecutionContext、JobSystem、JobStateStore 和 budget policy 类型。

## 模块图

lifecycle.rs 是私有实现模块；budget.rs 拥有资源核算。未来的 model、policy、execution、scheduler、cancellation 和 store 仍保持在本 crate。

## 测试入口

运行 cargo test -p velvet-job-system。测试覆盖取消、重试、deadline、budget、checkpoint、provenance 和重启 snapshot。

## 失败边界

runner panic、timeout、取消、budget 拒绝和 store error 都转成 typed Job outcome。失败 Job 不能终止 ApplicationCore。

## 迁移注意事项

保持 Job schema、checkpoint、environment 和 cancellation 语义稳定。不新增第二个 scheduler，也不把 Job 状态迁入 UI 或 PluginHost。
