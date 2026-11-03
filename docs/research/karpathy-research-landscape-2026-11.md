# Karpathy 相关研究版图与 Velvet 插件启示

Date: 2026-11-03

## 结论先行

Karpathy 的公开工作不是一条单独的“LLM 研究线”，而是四条相互连接的线：

1. **多模态语义对齐**：把图像、区域和文字对齐，形成可检索、可生成的表示。
2. **从零实现与最小系统**：把自动微分、语言模型、训练和推理压缩成可读、可运行、可修改的最小闭环。
3. **端到端训练与 AI 原生教育**：让学习者从数据、训练、评测一路做到可交互的模型产品。
4. **Agent 化研究与知识维护**：把研究过程变成有边界、有指标、有记录、可回退的循环，把原始资料编译成可查询的知识结构，再生成适合人的表达。

对 Velvet 最有价值的不是照搬某个项目，而是把这些工作抽象成三个插件边界：

```text
数据模型 / 知识模型  →  任务型投影  →  人类可读的呈现
        ↑                    ↑                  ↑
      Agent 查询           Job 生成          文本/图/HTML/视频
```

其中，受控语言属于最后一层的质量约束；插件注册首先要解决的是第一层的语义契约。

## 研究与项目分组

### 1. 早期的视觉—语言对齐研究

Karpathy 博士阶段的代表工作包括 **Deep Visual-Semantic Alignments for Generating Image Descriptions**、**DenseCap**、**Deep Fragment Embeddings**、**Visualizing and Understanding Recurrent Networks** 等。个人主页将这组工作列为其主要论文；其中视觉—语义对齐论文学习图像区域与文字片段之间的对应关系，并用这些对齐结果生成图像描述。[个人主页](https://karpathy.ai/)、[视觉—语义对齐论文页面](https://cs.stanford.edu/people/karpathy/deepimagesent/)、[DenseCap 论文](https://arxiv.org/abs/1511.07571)

这条线对 Velvet 的启示是：**插件不应只提供一个模糊的“搜索文本”接口，而应声明对象、区域、关系和证据之间的可解释对应关系**。例如，图像插件可以把 `Asset`、`Region`、`Caption`、`Embedding` 和 `SourceReference` 作为不同实体或 Facet；Agent 才能进一步询问“这段描述对应哪个区域、来自哪个版本的模型”。

### 2. 最小、可解释、可复现的学习系统

- **micrograd**：约百行的标量自动微分和小型神经网络库，重点是让反向传播和计算图完全可读。[GitHub](https://github.com/karpathy/micrograd)
- **makemore**：从字符级语言模型开始，逐步加入嵌入、MLP、批归一化和 Transformer，形成教学式的语言模型递进路径。[仓库](https://github.com/karpathy/makemore)、[Zero to Hero 课程仓库](https://github.com/karpathy/nn-zero-to-hero)
- **minGPT / nanoGPT**：最小的 GPT 训练和微调实现。nanoGPT 的 README 明确把它定位成易读、易改的训练脚手架，并说明它后来由 nanochat 接替；它现在更多是一个历史上重要的参考实现。[minGPT](https://github.com/karpathy/minGPT)、[nanoGPT](https://github.com/karpathy/nanoGPT)
- **microgpt**：把数据集、Tokenizer、自动微分、Transformer、Adam、训练循环和推理循环放入一个无依赖的 Python 文件。Karpathy 把它描述为“算法本质”的压缩版本。[说明文章](https://karpathy.ai/microgpt.html)、[源文件说明](https://github.com/karpathy/karpathy.github.io/blob/master/_posts/2026-02-12-microgpt.markdown)
- **llm.c / llama2.c**：用纯 C/CUDA 训练或推理 LLM，强调参考实现、可移植性和可修改性。llama2.c 还明确把“训练”和“推理”拆成两个可观察的边界。[llm.c](https://github.com/karpathy/llm.c)、[llama2.c](https://github.com/karpathy/llama2.c)

这组工作的共同原则不是“代码越少越好”，而是：**核心语义要能被检查，系统要能被复现，扩展要有清晰的改动点**。对应 Velvet，插件 API 应先做小而硬的契约：稳定的 id、版本、输入输出 schema、能力和错误；不要用一个万能的 `payload: JSON` 或直接暴露插件数据库来换取短期灵活性。

### 3. nanochat：从训练到产品的完整闭环

**nanochat** 是比 nanoGPT 更完整的实验性训练框架，覆盖 Tokenizer、预训练、微调、评测和推理，并用一个模型深度参数驱动一系列规模的模型。[官方 README](https://github.com/karpathy/nanochat)

它对 Velvet 的启示是，插件能力不是孤立的函数调用，而是一个生命周期：

```text
输入数据 → 预处理/索引 → 计算或模型调用 → 评测 → 结果/缓存 → 交互投影
```

如果未来 Velvet 支持“某领域深入学习”插件，插件应至少区分：原始来源、处理后的实体或 Facet、可重算的 Job 结果、面向人的 Presentation Profile。模型、索引和页面不能共享一份无版本的隐式状态。

### 4. autoresearch：Agent 研究循环

**autoresearch** 是一套让 Agent 自动改进小型 LLM 训练代码的研究实验。官方 `program.md` 将边界写得很窄：只改 `train.py`，固定五分钟运行预算，用 `val_bpb` 作为主要指标，实验结果写入 TSV；改进则保留 commit，否则回退。[README](https://github.com/karpathy/autoresearch)、[program.md](https://github.com/karpathy/autoresearch/blob/master/program.md?plain=1)

这里真正值得借鉴的是实验协议：

- 固定输入、预算和评测方式，避免 Agent 通过扩大范围获得虚假的改进。
- 每次尝试都有 hypothesis、代码版本、运行结果和 keep/discard 状态。
- 失败、超时和资源不足都是一等结果，而不是被静默吞掉的异常。
- Git commit 既是实验身份，也是回滚边界。

映射到 Velvet，就是把 Agent 的自动化工作实现为宿主拥有的 `Job`：Job 声明 owner、预算、输入 fingerprint、模型/插件版本、指标、checkpoint 和结果 provenance；插件只能在宿主批准的文件、数据和能力范围内执行。`autoresearch` 不意味着 Velvet 要做一个无限循环的 Agent，而是说明**自主性必须建立在可观察、可取消、可回退的协议上**。

### 5. LLM Wiki：从原始资料到可维护知识

Karpathy 的 **LLM Wiki** idea file 把知识工作拆成三层：原始资料、由 LLM 维护的互联 Wiki、描述规则和工作流的 schema。它定义了 ingest、query、lint 等操作，并允许把高价值回答回写为 Wiki 条目。[原始 Gist](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)、[实现站点](https://llmwiki.app/)

这比通常的 RAG 更接近“知识编译”：

```text
raw sources → entities / relations / pages → schema checks → task projection
```

对 Velvet 来说，LLM Wiki 不应直接变成一个全局第二数据库。更稳妥的设计是让**数据型插件注册一个版本化 DataModel 或 KnowledgeModel**，并由宿主负责发现、索引和路由：

- `model_id`、schema version、entity types、fields、relations；
- stable id、来源、内容指纹、更新时间和 freshness；
- 可搜索字段、typed query operations、分页和有界结果；
- 允许的 Job、Preview、Storage 和网络能力；
- 迁移、失效、删除和禁用后的恢复行为。

知识页面、摘要、链接图和向量索引都是该模型的可重算投影；它们不能替代模型本身。Agent 先发现模型和操作，再调用 typed query，最后请求适合当前任务的投影。

### 6. arxiv-sanity：研究资料的发现与推荐

**arxiv-sanity** / **arxiv-sanity-lite** 通过论文搜索、相似度排序、标签、个人库和推荐来缓解 arXiv 信息过载；lite 版本的 README 明确采用论文摘要的 TF-IDF 特征和 SVM 推荐。[旧版说明](https://github.com/karpathy/arxiv-sanity-preserver)、[lite 仓库](https://github.com/karpathy/arxiv-sanity-lite)

这条线和 Velvet 的 Feed Workbench 关系很直接：一个 Feed 插件的价值不只是抓取条目，还应暴露稳定的 `FeedItem` 身份、来源、抓取时间、相似度依据、用户标签和推荐理由。推荐结果需要带 provenance，不能只返回一串分数。

### 7. AI 原生教育：CS231n、Zero to Hero 与 LLM101n

Karpathy 设计并主讲了 Stanford 的 CS231n，之后又以 **Neural Networks: Zero to Hero** 课程把“从基础到 GPT”的实现过程拆成连续讲座和可运行 notebook。[个人主页](https://karpathy.ai/)、[Zero to Hero](https://github.com/karpathy/nn-zero-to-hero)

**LLM101n** 则把目标推进到“学生自己训练一个 AI，并做成可交互的 Storyteller”。原仓库目前已归档，README 说明课程仍由 Eureka Labs 开发；Eureka Labs 对外描述的模型是“教师设计课程材料，AI 教学助理在共同平台上进行个性化辅导”。[归档仓库](https://github.com/karpathy/LLM101n)、[Eureka Labs](https://eurekalabs.ai/)

这条线支持一个面向 Velvet 的产品判断：学习插件应把**领域模型、学习路径、练习/评测和多种讲解投影**分开。HTML、动画和视频是学习界面，不是知识的唯一存储格式。受控语言可以约束每一步的解释，但不能代替练习结果、证据和模型版本。

## 对 Velvet 插件架构的综合判断

### A. “受控语言”应该是呈现契约，不是注册契约

ASD-STE100 这类受控语言解决的是“如何写得更清楚、更少歧义”。它适合成为 `PresentationProfile` 或 `ExplanationPolicy` 的一部分，例如：句长、术语、主动语态、定义顺序、警告等级和是否需要引用来源。

插件注册仍应以机器可读的语义模型为中心。否则 Workbench 可以全文检索，却无法知道某个词是实体、字段、关系、版本还是推断结果；Agent 也无法可靠地选择查询操作。

### B. 不是所有插件都要注册 DataModel

建议按能力分层：

| 插件类型 | 是否需要 DataModel | 需要声明什么 |
| --- | --- | --- |
| 纯 UI / View | 否 | View、Zone、主题与 bridge 版本 |
| Command / Action | 否，除非产生持久数据 | 参数、权限、可取消性、结果 schema |
| Preview / Renderer | 通常否 | 支持的 `ContentKind`、输入上限、输出 projection |
| 数据连接器 / Feed | 是 | Entity、identity、provenance、查询与 freshness |
| 知识库 / 学习插件 | 是 | DataModel、KnowledgeProjection、ingest/query/lint |
| 分析 / 训练插件 | 依赖输入模型 | Job、预算、指标、checkpoint、结果 provenance |

因此，“注册一个数据模型”应成为**数据型插件的准入标准**，而不是所有插件的统一负担。

### C. Agent 的发现顺序

Agent 接入时应遵循：

1. 发现插件能力和 `model_id`/版本。
2. 读取 entity、field、relation、query operation 和权限说明。
3. 以 typed query 获取有界数据，结果携带 source reference、fingerprint、freshness 和 plugin/model version。
4. 请求 `KnowledgeProjection` 或 `PresentationProfile`，选择文本、图、HTML 或视频。
5. 对模型生成的推断与来源事实分别标记，并把新的知识条目作为待审核/待 lint 的派生数据。

这样，插件不会通过“给 Agent 一个数据库连接”获得不受控的能力，Workbench 也不会把某个插件的页面结构误当成公共数据模型。

### D. 最值得做的第一个垂直切片

不建议立即做通用的全局 Knowledge Registry。Foundation Plan 已把 product-specific indexing 放在后续阶段，并要求具体 provider 或 Facet 自己声明缓存和状态 owner。[Foundation Plan](../engineering/workbench-foundation-plan.md#L16)

更小、可验证的切片是一个 `research.wiki` 或“领域学习” fixture：

1. 原始来源带 `SourceReference`、内容指纹和抓取时间。
2. 插件注册一个 versioned `KnowledgeModel`，只包含少量实体、字段和关系。
3. Workbench 通过 host-owned discovery/index 查询这些实体。
4. 一个 `Job` 生成摘要、概念页和链接图，并记录输入、模型、插件版本和结果 provenance。
5. `PreviewProvider` 用受控 Markdown-plus-components 渲染文本、表格和有限图形。
6. `PresentationProfile` 再选择 STE-inspired 文本、交互 HTML 或视频脚本。
7. 禁用或卸载插件后，核心 Asset、布局和通用预览仍可用；派生知识可以迁移、重算或丢弃。

这个切片同时验证了 LLM Wiki 的三层模型、autoresearch 的 Job 纪律、早期视觉—语言工作的 provenance/对齐思想，以及 Karpathy 输出格式讨论中的“语义与呈现分离”。

## 研究边界与可信度

本梳理把 Karpathy 的个人主页、论文页面、GitHub 仓库、LLM Wiki Gist 和 Eureka Labs 页面作为一手材料。仓库的 star/fork 数量、issue 数量和 README 中的成本/速度数字会随时间变化，只用于理解项目定位，不作为工程能力或研究结论的证明。`autoresearch` 的自动循环是实验方法示例，不等于已验证的通用科研替代方案；LLM Wiki 是公开的设计/实现原型，也不应被解释为成熟的知识治理标准。
