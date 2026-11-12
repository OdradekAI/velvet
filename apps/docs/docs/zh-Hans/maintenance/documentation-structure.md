---
id: documentation-structure
title: 文档工程结构
sidebar_label: 文档工程结构
---

# 文档工程结构

docs app 将公共使用指南、API 参考和 Docusaurus 本地化资源分开管理。每类内容都有明确 owner，维护者可以在大型仓库中独立审查和验证变更。

## 目录关系

```text
apps/docs/
├── docs/
│   ├── en/              默认语言的插件平台使用指南
│   └── zh-Hans/         公共插件平台指南的中文镜像
├── reference/
│   ├── en -> docs/engineering/  指向 canonical source 的受控软链接
│   └── zh-Hans/         API 和 contract 参考的中文镜像
├── i18n/
│   ├── en/              Docusaurus 元数据和主题资源
│   └── zh-Hans/         Docusaurus 元数据和主题资源
├── src/                 交互 Demo、主题覆盖和 showcase CSS
└── static/              静态资源和 llms.txt 等机器可读索引

docs/engineering/        英文 Reference 和工程记录的 canonical source
```

英文和中文 Markdown 目录保持同级。维护者可以通过相同的相对路径直接对照两个语言版本，不需要在生成的 locale 目录中查找。

## 内容职责

| 路径 | 职责 | 维护动作 |
| --- | --- | --- |
| `apps/docs/docs/en` | 英文教程、平台概念、contribution 流程和 UI Kit 使用指南 | 插件工作流或公共导航变化时更新 |
| `apps/docs/docs/zh-Hans` | 公共平台指南的中文翻译 | 保持与英文目录相同的相对路径和本地化链接 |
| `apps/docs/reference/en` | 用于仓库导航和编辑器对照的受控软链接 | 保持链接目标为 `docs/engineering` |
| `docs/engineering/ui-kit` | 由 Docusaurus 直接构建的英文 API、组件、Overlay 和 Host Projection 参考 | UI Kit contract 变化时直接修改这里 |
| `apps/docs/reference/zh-Hans` | 中文 API 和 contract 参考 | 翻译英文 Reference，保留 props、states、actions 和 contract 标识符 |
| `docs/engineering` | contract、qualification evidence 和设计决策的工程源 | contract 或持久工程规则变化时先修改这里 |
| `apps/docs/i18n` | Docusaurus 生成的 sidebar、navbar、footer 和代码翻译资源 | 使用 Docusaurus 翻译工具更新，不放置主要 Markdown 页面 |
| `apps/docs/src` | 可执行 Demo、Playground 状态、主题覆盖和样式 | Demo 或文档渲染行为变化时更新 |
| `apps/docs/static` | 公共静态资源和机器可读索引 | 新增或移除公共路由时更新 `llms.txt` |

## 如何选择目录

面向插件开发者的使用说明放在 `docs/en` 和 `docs/zh-Hans`。例如快速开始、contribution 选择、安全指南和 UI Kit 定制。

稳定的公共 contract 放在 `docs/engineering/ui-kit` 和 `reference/zh-Hans`。英文 docs app 直接读取 `docs/engineering`，受控的 `reference/en` 软链接用于仓库内对照，中文目录提供对应翻译。例如组件 Props、状态、键盘行为、无障碍语义、宿主所有权、迁移规则和 qualification evidence。

`i18n` 只存放 Docusaurus 生成或消费的界面字符串。新增文档应放入同级的 Markdown 目录。新增 sidebar 标签需要同时更新 `sidebars.ts` 和对应 locale JSON。

可执行内容放在 `src`。Playground 需要暴露真实组件状态，并由中英文对应页面共同引用。API 解释、边界和维护说明继续放在 Markdown 页面中。

工程页面可以在读者需要渲染站点时链接到发布后的 `/reference/...` 路由。这个链接用于导航，英文构建直接读取 `docs/engineering`，`reference/en` 软链接用于本地查找和对照，仓库在没有启动 docs server 时仍然可以查阅工程源。

## 变更流程

1. 行为变化时先修改 desktop contract、story 或实现。
2. 更新 `docs/engineering` 下对应的工程源文件。
3. 确认 `apps/docs/reference/en` 仍然指向 `docs/engineering`，并确认英文 Reference 页面通过 canonical mount 可见。
4. 更新 `apps/docs/reference/zh-Hans` 中文 Reference。
5. 用户工作流变化时同步更新中英文平台指南。
6. 导航或机器可读覆盖变化时更新 `sidebars.ts`、locale JSON 或 `static/llms.txt`。
7. 运行文档检查、类型检查和生产构建。

```sh
cd apps/docs
npm run check:docs
npm run check:links
npm run check:source-link
npm run typecheck
npm run build
npm audit --audit-level=low
```

`check:docs` 会检查相对路径一致性、英文 Reference source 和软链接目标、中文本地化链接、英文页面中文残留、内部 Markdown 链接和 `llms.txt` 路由。`verify:release` 会把相同检查和双语言生产构建组合起来。CI 会将构建结果作为发布 artifact 保存，不会改变 canonical source。

`Documentation Pages` workflow 会使用 project-site 的 `/velvet/` base path 构建同一个 release gate，并通过 `github-pages` environment 部署 `build/`。公开地址为 `https://odradekai.github.io/velvet/`。
