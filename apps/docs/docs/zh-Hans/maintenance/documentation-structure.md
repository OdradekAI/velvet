---
id: documentation-structure
title: 文档结构与维护
---

# 文档结构与维护

<span id="文档工程结构" className="velvet-doc-anchor" />

文档应用拥有导航、翻译审阅元数据、本地搜索投影和演示交互状态，消费已有宿主 UI 与工程契约。它不拥有插件 SDK、运行时或持久产品状态。

<span id="目录关系" className="velvet-doc-anchor" />
<span id="内容职责" className="velvet-doc-anchor" />
<span id="如何选择目录" className="velvet-doc-anchor" />

## 内容来源与所有权

| 来源 | 职责 |
| --- | --- |
| `apps/docs/docs/en` 与 `apps/docs/docs/zh-Hans` | 任务指南、概念、UI 示例和维护者地图；相对路径对应 |
| `docs/engineering` | 权威英文契约与独立工程记录；仅挂载发布清单中的页面 |
| `apps/docs/reference/en` | 指向 `docs/engineering` 的受版本控制 symlink，仅用于编辑导航 |
| `apps/docs/reference/zh-Hans` | 已发布参考的译文或明确标注的摘要 |
| `docs/adr`、`docs/GLOSSARY.md`、`docs/research` | 决策、统一词汇和研究证据；从维护者地图链接 |
| `apps/docs/i18n` | Docusaurus 界面标签；正文保留在并排的语言目录中 |
| `apps/docs/src/components/playgrounds` | 按组件组织演示状态；只从已有 `@velvet-ui` 入口导入 |
| `apps/docs/reference-pages.json` | 构建、检查和索引共用的明确参考发布清单 |
| `apps/docs/translation-status.json` | 英文源文与中文内容的审阅状态及 hash |

`src/generated/page-metadata.json` 与 `static/search-index.json` 是忽略提交的构建投影；`static/llms.txt` 自动生成并提交。不能扫描整个 `docs/engineering` 或仓库来建立公开索引，未发布的工程记录和 scratch 资料不能因目录发现而进入站点。

## 阅读路径与 URL

保留两个 docs plugin instance，为插件开发、API/协议、可信 UI 和维护者提供独立侧栏。既有 URL 继续有效；英文权威源和 symlink 不变。迁往 `docs/reference` 的物理重组留待真实维护需求出现后进行。

以后搬动文件时，用显式绝对 `slug` 保留旧地址，或提供重定向，并更新 sidebar ID、译文与索引。新增公共参考必须加入 `reference-pages.json`。不能为了不同导航入口复制英文契约。

## 契约与翻译审阅

每份契约应说明读者、owner/来源、状态、版本、输入输出、权限、错误、取消/销毁和证据。设计状态（ADR）、实现证据（fixture/宿主/平台）和兼容性承诺（Alpha/stable）分别表达。源码可见的内部类型不自动成为公共 API。

翻译状态包括：

- `reviewed`：已对照完整双语内容；任一侧 hash 变化都会让审阅检查失败。
- `summary`：有意压缩的摘要；读者看到提示和英文原文链接。
- `unreviewed`：尚待一致性复核的已有译文；页面显示提示。

摘要或未复核页面发生变化时，额外显示过期提示。`npm run check:translations` 要求全部已发布页面为 `reviewed`，`verify:release` 会运行这一严格检查；summary 和 unreviewed 仍可用于本地草稿。Hash 只能检测变化，不能证明语义等价；不能为了通过 CI 将缩略参考标记为 reviewed。关键 Manifest 字段与必填性、Bridge 消息名和组件属性表还会进行结构检查。所有已复核页面还必须保留契约字面量、可执行代码块，以及 API 表中的名称、类型和默认值。这些检查辅助逐段复核，不能证明正文语义等价。

实际对照一组文件后，在 `apps/docs` 中记录该页：

```sh
node tools/review-translation.mjs docs/platform/quick-start.md reviewed
node tools/review-translation.mjs reference/plugin-developer-guide.md reviewed
```

新增页面必须登记状态；路径一致不代表翻译已经审阅。摘要也不能用不同字段名替代 schema。翻译时保留标识符、错误码和显式锚点。

<span id="变更流程" className="velvet-doc-anchor" />

## 修改与验证流程

1. 行为变化时，更新 owner 实现、契约及相关 fixture。
2. 更新指南与中文内容，记录真实审阅状态。
3. 按需更新 sidebar、界面翻译和参考发布清单。
4. 运行下方文档发布检查：从公开来源生成索引，检查内容与导入边界，运行校验器回归测试，进行类型检查和双语构建。
5. 流程变化运行文档示例；UI/导航变化检查构建站点，包括中文路由、提示和搜索。

在仓库根目录运行：

```sh
npm --prefix apps/desktop ci
npm --prefix apps/docs ci
npm --prefix apps/docs run verify:release
```

第一条命令安装文档演示依赖的真实 UI 依赖，不因此抽出 SDK package。文档 tsconfig 保持 `strict` 和 `isolatedModules` 开启。

`Documentation checks` 在 PR 上以仓库只读权限运行，不部署。`Documentation Pages` 构建 main，部署权限仅授予 deploy job。两者 checkout Git 历史，使最后更新时间来自源文件提交，而非复制时间。未提交的新页面没有可靠历史时间。

开发命令和支持的 Node 版本见 `apps/docs/README.md`。搜索和提示在 start/build 时生成；修改索引内容或翻译记录后重启开发服务器。
