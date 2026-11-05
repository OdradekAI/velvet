# Asset Inspector fixture

这是 issue 009 的 Asset Inspector 合同 fixture，用来演示第三方开发者旅程。
它不是生产插件包，也没有新增插件 runtime。

`manifest.json` 展示稳定的插件/贡献 ID、Asset Manager scope、导航区/上下文区/
操作区 View、上下文命令、快捷键、Preview augmentation 和质量分析 Job。
`src/asset-inspector.mjs` 只依赖测试中的 fake HostFacade，由
`apps/desktop/test/asset-inspector-demo.test.mjs` 驱动。

fixture 只接收不含路径的 Asset projection 和 fingerprint，不访问数据库、文件路径、
Tauri state、React 组件或其他 View。facet 写入命名空间
`com.acme.asset-inspector/quality`，携带 schema 和 input fingerprint；fingerprint
过期时写入会失败并产生诊断。

manifest 中的 `runtime: built-in` 只是为了复用当前 trusted adapter 测试接缝，不能理解为
允许第三方代码生产环境内置运行。WASM/sidecar 的最终选择仍属于后续 runtime 决策。

从仓库根目录运行完整合同测试：

```sh
npm --prefix apps/desktop test
```
