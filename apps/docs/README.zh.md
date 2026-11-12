<p align="center">
  <img src="../../docs/logo/logo.png" alt="Velvet" width="120" />
</p>

# Velvet Docs

[English](README.md) | [简体中文](README.zh.md)

本目录是 Velvet 文档站点的 Docusaurus 工程，发布面向扩展开发者的指南、API 参考、UI Kit 示例和仓库维护文档，提供英文和简体中文版本。本手册说明如何贡献内容及维护站点。

[阅读文档](https://odradekai.github.io/velvet/) · [了解 Velvet](../../README.zh.md) · [文档架构](docs/zh-Hans/maintenance/documentation-structure.md)

## 本地开发

使用与 CI 一致的 **Node.js 22** 和 npm。请检出完整仓库：文档站点依赖本目录之外的工程文档和桌面端 UI 组件。在仓库根目录安装两组依赖：

```sh
npm --prefix apps/desktop ci
npm --prefix apps/docs ci
cd apps/docs
npm run start
```

开发服务器默认提供英文站点，修改页面后会自动重新加载。除非另有说明，下文命令均在 `apps/docs` 目录执行。

| 任务 | 命令 |
| --- | --- |
| 编辑英文内容 | `npm run start` |
| 编辑中文内容 | `npm run start -- --locale zh-Hans` |
| 构建中英文站点 | `npm run build` |
| 预览已有构建产物 | `npm run serve` |

开发服务器一次只提供一种语言。测试语言切换时，先构建两个语言版本，再运行 `npm run serve`；英文位于 `/`，中文位于 `/zh-Hans/`。使用生产构建预览时，修改内容后需要重新构建。

## 找到需要修改的源文件

下表路径均相对于本目录。

| 路径 | 维护内容 |
| --- | --- |
| `docs/en/`、`docs/zh-Hans/` | 成对维护的指南、概念说明、UI 示例和维护文档 |
| `../../docs/engineering/` | 英文参考契约的权威来源；在这里修改原文 |
| `reference/zh-Hans/` | 已发布参考契约的中文译文 |
| `reference-pages.json` | 作为参考文档发布的工程文件清单 |
| `sidebars.ts`、`sidebars-reference.ts` | 指南和参考文档的导航 |
| `i18n/` | 导航标签和其他 Docusaurus 界面文字的翻译 |
| `src/components/`、`src/theme/`、`src/css/` | 交互示例、主题定制和样式 |
| `static/` | 站点提供的公共静态资源 |
| `tools/`、`translation-status.json` | 文档检查、索引生成和翻译复核记录 |
| `docusaurus.config.ts` | 站点配置、语言、内容来源和部署 URL |

`reference/en` 是指向[权威工程目录](../../docs/engineering/)的受版本控制的符号链接，用于编辑器导航；构建直接读取权威源目录。该目录中只有列入 `reference-pages.json` 的页面会被发布。完整的内容归属规则见[文档架构指南](docs/zh-Hans/maintenance/documentation-structure.md)。

## 新增或更新文档

1. **选择内容类型。** 任务操作指南和概念说明放在 `docs/en/`，API 契约在对应的工程源文档中维护。设计理由记录在相关 [ADR](../../docs/adr/)，领域术语以[术语表](../../docs/GLOSSARY.md)为准。
2. **围绕读者任务写作。** 说明前置条件，提供可运行示例，并解释预期结果。参考文档应覆盖输入、输出、权限、错误和生命周期行为。区分已实现行为、提案和仅由 fixture 演示的行为；通过链接引用契约，避免在各篇指南中重复维护。
3. **同步两种语言。** 中英文目录使用相同的相对路径。保留 API 标识符、代码示例、显式锚点，并保持路由不变；存在对应中文页面时，中文链接应指向该页面。
4. **接入导航。** 更新对应侧栏和 `i18n/` 中的标签翻译。新增参考文档时，以相对于 `docs/engineering` 的路径加入 `reference-pages.json`。移动或重命名页面时，用显式 `slug` 或重定向保留公开 URL，并更新指向该页面的链接。
5. **复核并验证。** 记录翻译复核结果，运行下文检查，并预览两个语言版本的相关页面。示例操作说明发生变化时，实际运行该示例。

保持 PR 范围集中，说明面向读者的变化、受影响页面及已完成的验证。导航或视觉变化应附截图。

## 复核翻译

`translation-status.json` 保存每对已发布中英文页面的复核状态和内容指纹。完整对照中英文，并根据 API 所属的源文件核实细节后，记录相应页面：

```sh
node tools/review-translation.mjs docs/platform/quick-start.md reviewed
```

指南的记录键以 `docs/` 开头，参考文档以 `reference/` 开头，后接相对 Markdown 路径。任一语言后续发生修改，已有复核记录就会失效；重新对照后再更新记录。

本地草稿可以使用 `summary` 或 `unreviewed`，页面会展示状态并链接到英文原文。发布时，所有页面对都必须为 `reviewed`。内容指纹用于检测变更，结构检查用于比较 API 字面量、类型、默认值和可执行示例；两者都不能代替正文内容复核。

## 维护站点与示例

UI 示例通过 `@velvet-ui` 使用桌面端现有的公共 UI 入口。示例状态放在 `src/components/playgrounds/`，组件应从该入口导入，避免直接依赖桌面端内部模块。站点代码必须通过严格 TypeScript 检查。桌面端行为变更应在桌面应用中实现，并执行对应验证。

索引生成器从已发布源文件生成搜索数据、页面提示和路由清单。`npm run start`、`npm run build` 和 `npm run typecheck` 会自动运行生成器。也可以手动刷新：

```sh
npm run generate:index
```

| 生成路径 | 版本控制方式 |
| --- | --- |
| `static/llms.txt` | 已发布路由变化时，提交相应变更 |
| `static/search-index.json`、`src/generated/` | 已忽略；在本地重新生成 |
| `.docusaurus/`、`build/` | 已忽略的缓存和构建产物 |

修改源文件，不直接编辑生成产物。依赖变更应连同对应的 `package-lock.json` 一起提交，并执行完整发布检查。

共用品牌资源位于 [`docs/logo`](../../docs/logo/)。本手册使用 `logo.png`；`static/img/favicon.svg` 链接到 `logo_simple.svg`。保留相对符号链接，让站点图标只有一份权威来源。

## 提交 PR 前验证

提交变更前，运行完整站点检查：

```sh
npm run verify:release
```

该命令会重新生成索引、检查所有译文是否已完整复核、校验内容与 API 一致性、运行检查器回归测试、检查站点代码类型，并构建两个语言版本。生产构建会因失效链接或锚点而失败。

编辑过程中可以按需运行更小范围的检查：

| 命令 | 用途 |
| --- | --- |
| `npm run check:docs` | 页面对应关系、来源链接、复核记录是否过期、契约检查和索引覆盖 |
| `npm run check:translations` | 完整文档检查，并要求每对已发布页面均为 `reviewed` |
| `npm run check:links` | 内部 Markdown 链接 |
| `npm run check:source-link` | 英文参考文档的受版本控制符号链接 |
| `npm run test:docs` | 文档检查器回归测试 |
| `npm run typecheck` | 严格 TypeScript 验证 |

修改 UI 或导航后，还需检查构建页面、语言切换、搜索和交互示例。[`Documentation checks`](../../.github/workflows/docs-checks.yml) 工作流会在 PR 中执行发布检查和文档中的 Asset Inspector fixture，仅持有仓库只读权限。

## 构建与部署

[`Documentation Pages`](../../.github/workflows/docs-pages.yml) 工作流验证 `main` 分支并将其发布到 [GitHub Pages](https://odradekai.github.io/velvet/)，也支持手动触发。部署任务通过 `github-pages` 环境发布 `build/` 产物；PR 检查不执行部署。

在本地复现 Pages 构建时，为构建和预览同时设置部署环境（POSIX shell）：

```sh
export DOCS_DEPLOY_TARGET=github-pages
export DOCS_BASE_URL=/velvet/
npm run verify:release
npm run serve
```

| 变量 | 默认值及用途 |
| --- | --- |
| `DOCS_DEPLOY_TARGET` | 默认为 `local`；`github-pages` 会选择 Pages 的默认 URL 和基础路径 |
| `DOCS_SITE_URL` | 本地为 `https://velvet.local`；Pages 为 `https://odradekai.github.io` |
| `DOCS_BASE_URL` | 本地为 `/`；Pages 为 `/velvet/`；保留末尾斜杠 |

回到根路径开发时，使用新 shell 或取消这些环境变量。CI 应保留完整 Git 历史，使页面更新日期反映源文件提交时间。调整托管配置时，参阅 [Docusaurus 部署指南](https://docusaurus.io/docs/deployment)。

## 常见问题

| 现象 | 处理方法 |
| --- | --- |
| 缺少桌面端 UI 依赖或无法导入 `@velvet-ui` | 在仓库根目录运行 `npm --prefix apps/desktop ci`，确认使用完整仓库 |
| 搜索结果或翻译状态提示未更新 | 运行 `npm run generate:index`，然后重启开发服务器 |
| 修改配置或依赖后提示缺少打包分块 | 运行 `npm run start:clean`，然后强制刷新浏览器 |
| 开发时语言切换失败 | 构建两个语言版本，再用 `npm run serve` 预览 |
| 翻译复核记录过期 | 对照并修正相应页面对，然后重新记录复核结果 |
| 资源或链接仅在 Pages 上失效 | 使用上文的 `/velvet/` 部署环境复现 |
