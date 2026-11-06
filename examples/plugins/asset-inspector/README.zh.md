# Asset Inspector Demo 插件

这个目录是可直接运行的、确定性的 Asset Manager 插件合同示例，也是
issue 009 的开发者验收工具。它不是 Marketplace 包；当前仓库仍没有被接受的
WASM/sidecar 生产 runtime，也没有独立发布的第三方 SDK。

Demo 覆盖：

- manifest、稳定的插件/贡献 ID、激活条件和 API range；
- 一个命令、Asset 右键菜单、Mod+Alt+A 快捷键，以及 navigation、
  contextual、operations 三个语义区域的 View；
- 有界的宿主上下文和不带路径的 Asset projection 读取；
- 结构化日志、宿主通知和 UI Kit v1 组件（loading-state、progress、
  table、empty-state、error-state）；
- 由宿主拥有的 Job 调用、进度、协作取消、unknown 结果和 runner 崩溃；
- 插件私有 storage/configuration 和 schema migration；
- 带 fingerprint 的 com.acme.asset-inspector/quality facet；
- manifest、版本、权限和 stale-result typed error；
- 禁用、重新启用和 UI surface 卸载时的清理。

src/asset-inspector.mjs 是插件入口。它只接收文档化的 fake HostFacade，
不读取数据库、源路径、文件句柄、Tauri state、React/Vue 组件或其他 View。
surface 入口只是发送 bridge ready 的 bootstrap，不渲染任意 DOM；UI Kit 组件、
overlay、布局和焦点都由宿主拥有。

src/fixture-host.mjs 是用于验证公开接缝的确定性内存 adapter，不是 runtime
实现，也不是安全沙箱。真实第三方包仍需等待已接受的隔离 runtime 和 host SDK。

直接运行 Demo：

~~~
node examples/plugins/asset-inspector/run-demo.mjs
~~~

运行完整合同测试：

~~~
npm --prefix apps/desktop test
~~~

只运行 Demo 测试：

~~~
node apps/desktop/test/asset-inspector-demo.test.mjs
~~~

测试覆盖 manifest 和版本拒绝、激活注册、宿主状态读取、UI Kit 协商和组件投影、
成功 facet 写入、过期 fingerprint、权限拒绝、可取消 Job、unknown 结果、runner
失败、日志/通知 sink 失败、存储迁移、禁用/重新启用清理，以及核心 Asset 状态保留。

## 开发步骤

1. 复制包并在升级过程中保持插件 ID 不变。
2. 先编辑 manifest.json。贡献 ID 必须唯一且以插件 ID 为前缀；显式声明目标应用、
   语义 zone、激活事件、storage schema 和实际需要的能力。
3. 实现 activate(host)。只通过宿主 registry 注册声明，并保存所有 disposer。
   不调用 Workbench 内部对象。
4. 只读取有界 projection。Job provenance 中携带 selection revision、config version、
   input fingerprint 和 schema version。
5. 通过宿主 Job handle 提交工作；通过 Host/UI Kit 报告进度并协作取消。
   unknown 必须先对账，不能盲目重复非幂等副作用。
6. 通过 fingerprint 检查后，只写插件命名空间；过期写入必须失败关闭。
7. 只使用协商后的 UI Kit 组件列表；通知、日志和 overlay 通过 typed host call 请求，
   这些调用失败时不能掩盖原始领域错误。
8. 在 disable、crash、uninstall 时释放所有注册；重新激活必须能从 manifest 重建贡献。

宿主仍拥有权限、持久 Job、布局、预览选择、持久化和失败恢复。这个 fixture
证明边界形状，不把 fake adapter 或 runtime: built-in 解释成生产第三方 runtime。
