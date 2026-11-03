# DeepSeek Super-agent 与 Velvet 多插件生态适配研究

日期：2026-11-02（补充核对 DeepSeek Harness 当前公开架构）

## 2026-11-02 的更新：不能只看 superagent 原型

这次核对把 DeepSeek 的公开材料分成两层：`dsh-superagent-prototype` 是面向 GitHub 单仓库的主动工作研究原型；当前的 DeepSeek Harness 是更底层、可组合的 Agent Harness。两者不能互相替代。原型 README 明确说它不是官方产品，公开实现也不等同于文章或内部版本；Harness 官方仓库则把自己定位为 DeepSeek AI 的开源 agent harness，当前仍是 developer preview，允许不兼容变化。

Harness 的官方架构文档给出了比原型更稳定的系统边界：

- **Everything is a Plugin**：模型适配器、工具注册、Session log 和 Agent loop 都是插件；插件向共享上下文提供 service、typed event 和可回滚 effect。卸载插件时，其注册和 effect 要能撤销。
- **Profile/Bundle/patch composition**：运行时由有序 bundle 和 patch 层组成，profile 是持久化组合；配置层可以替换或插入组件，而不需要改一个特权核心。
- **Durable SessionEvent log + live Agent events**：模型可见事实进入 append-only SessionEvent；`agent/*`、`tools/*` 等事件用于拦截和观察运行中的工作。Session log 是模型上下文的来源，UI 增量来自独立的 live stream。
- **Capability seams**：Service Definition、Provider、Consumer 三者组成可替换能力 seam；工具、文件系统、子进程、sandbox、subagent、webhook、UI 都通过 seam 接入。
- **Agent loop 的恢复语义**：一个 step 是一次模型请求及其工具调用，一个 turn 由多个 step 组成；取消、工具结果缺失、失败 attempt、fork/resume 和持久化都围绕已提交事件处理，而不是把运行中的进程状态当作权威。
- **Experimental coordination**：官方文档列出 `ctx.agentTeams` 的持久 roster/task state 与可继续 subagent；官方站点和 release note 也把 Agent teams、subagent delegation、scheduled tasks、developer tools 标成 experimental，不能把它们当成稳定协议。

官方 Cordis 论文把 Harness 的插件基础抽象成两种可组合性：每个 context transformation 都带可逆 effect（卸载时撤销），组件声明 coeffect 并对上下文变化做 reactive activation/deactivation；运行时通过统一 context 处理中间交互。这说明 DeepSeek 的“插件化”重点不只是把功能拆成包，还包括**注册副作用可回滚**和**依赖变化可重算**。

一手资料：

- [DeepSeek Harness README](https://github.com/deepseek-ai/deepseek-harness)（官方仓库，developer preview、Everything is a Plugin、Cordis 依赖）
- [DeepSeek Harness Architecture](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/architecture.md)（官方架构文档，profile/bundle、事件、Agent loop、session log、capability seam）
- [DeepSeek Harness 官方站点](https://www.deepseek.com/en/harness/)（public preview、Creator mode、插件/自动化/Agent teams 等产品面）
- [A Programming Paradigm for Spatiotemporal Composability](https://arxiv.org/abs/2608.25512)（DeepSeek-AI 参与的一手 Cordis 论文，revertible effects 与 reactive coeffects）
- [dsh-superagent-prototype README](https://github.com/deepseek-ai/dsh-superagent-prototype)（研究原型边界、Router/Worker 方向的公开实现）

### 对 Velvet 研究结论的增量影响

原有建议“在 `ApplicationCore` 上增加宿主拥有的 Agent 编排平面”仍然成立，但应把它拆成两条明确的 seam：

1. **Agent orchestration seam**：负责 Trigger/Eval/Mandate/Ticket/Assignment、事件去重、授权重验证、Watchdog 与注意力投影。它对应 superagent 原型的 Router/Worker 领域，不应被某个插件替换。
2. **Reversible plugin capability seam**：负责插件注册、typed events、service provider、worker/source/channel 能力和卸载回滚。它吸收 Harness/Cordis 的可回滚 effect 与 reactive activation 思想，但仍服从 Velvet 的 `PluginHost`、`CapabilityGrant`、`StateSnapshot`、`RecoverySet` 和宿主失败隔离。

因此，Velvet 不需要改成“没有核心的全插件系统”。当前 ADR 已要求 Rust `ApplicationCore`、`PluginHost`、`CoreTransport` 和宿主拥有的状态/恢复边界；这些边界是可靠性和安全 owner。可吸收的是**插件注册与 Agent 能力的可逆生命周期**，而不是放弃 owner。只有在未来出现第二客户端或远程运行时，才把 orchestration/capability seam 映射到独立 transport/adapter；不应为了追随 Harness 的 profile/bundle 形态而提前拆 daemon 或把持久化状态交给插件。

## 结论

Velvet 不需要推翻当前的本地优先、Rust `ApplicationCore`、`PluginHost`、共享 `JobSystem`、宿主拥有的 `CompanionView`/`OperationsPanel` 和 `CoreTransport` 方向。它们已经覆盖了 Super-agent 所需的运行时边界、失败隔离、任务生命周期和 UI 投影的主要骨架。

需要调整的是编排层的定义：如果 Velvet 要支持持续运行、事件触发、并行 Worker 和低打扰的人机协作，应在现有骨架上增加一个由 Rust 核心拥有的 Agent orchestration plane，而不是把“Agent”做成普通插件、把每个插件变成 Worker，或把 `Job` 直接改造成 Agent 会话。

建议把这次变化记录为新的 ADR（暂定名：Agent orchestration and plugin execution），再按垂直切片实现。研究阶段不改变现有 ADR 的状态。

## 一手资料与证据范围

- [DeepSeek Harness 官方仓库](https://github.com/deepseek-ai/deepseek-harness) 将 Harness 定义为 DeepSeek AI 的开源 agent harness，采用 “Everything is a Plugin” 架构；仓库明确标注为 developer preview，可能有兼容性破坏。
- [官方 dsh-superagent-prototype 仓库](https://github.com/deepseek-ai/dsh-superagent-prototype) 明确说明它是“仅供参考的研究原型”，不是官方产品，且公开实现与设计文章及内部版本并不完全相同。它监听 GitHub/可选的飞书事件，为每个工作项创建独立的 Agent Session 和目录，并通过面板呈现需要人工处理的事项。
- [原型的 Philosophy](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/main/docs/000-philosophy.md) 把目标概括为尽量少占用人的注意力，只在需要人行动或能使用结果时发声；主动工作先从 `observe` 开始，再逐步扩大范围。
- [原型的 Domain model](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/main/docs/001-domain.md) 把工作建模为 `Source → Trigger → Thread → Eval → Mandate → Ticket → Assignment → Executor → Channel`，并以 `Subscription` 做上下文扇出。
- [原型的 Authorization](https://raw.githubusercontent.com/deepseek-ai/dsh-superagent-prototype/main/docs/002-authorization.md) 明确规定：消息、评论和引用文档只能作为决策输入，不能改变权限；`off / observe / auto` 是渐进的主动工作准入模式；每个授权都带有 policy version 和 mandate。
- [原型的 Execution](https://raw.githubusercontent.com/deepseek-ai/dsh-superagent-prototype/main/docs/003-execution.md) 将一个 Ticket 与一个独立 Worker Session/目录绑定，支持并行 worker slot、后台 shell/subagent、重启恢复、输入投递重试和有限资源。
- [原型的 Panel](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/main/docs/005-panel.md) 只优先显示需要人处理的事项，再显示系统健康和历史；健康状态不占用额外注意力。
- [原型的 Architecture](https://raw.githubusercontent.com/deepseek-ai/dsh-superagent-prototype/main/docs/007-architecture.md) 将 `Engine` 作为 Trigger、Thread、Decision、Ticket、Assignment 的唯一写入者，用事务和提交后的 Effects/Outbox 隔离持久化与外部副作用。
- [DeepSeek Tool Calls 文档](https://api-docs.deepseek.com/guides/tool_calls/) 明确说明模型只返回工具调用，实际函数由调用方提供和执行；这支持把权限、能力和副作用留在宿主，而不是留给模型。
- [DeepSeek Responses API 文档](https://api-docs.deepseek.com/guides/responses_api/) 当前将 API 描述为无状态：`store`、`conversation`、`previous_response_id` 等服务端会话能力不提供，状态需要由调用方维护。

以下判断以这些一手资料为准。公开原型的代码质量和实验数据不能直接当作 Velvet 的生产设计或性能保证。

## Super-agent 的架构要点

### 1. Router / Worker 双层

Router 是常驻的全局协调者，负责接收人和外部事件、维护全局上下文、做准入判断、分派和汇总。Worker 拥有独立上下文和执行目录，负责一个具体工作项；多个 Worker 可以并行。Worker 的结果回到 Router，由 Router 决定是否、何时以及以什么渠道呈现给人。

这不是一个“更强的单 Session”。它把人面对的一个助手与内部多个相互隔离的执行会话分开。

### 2. 事件驱动和订阅

GitHub 评论、CI 结果、聊天消息等先成为不可变 Trigger。程序规则先做快速过滤，再由模型做需要语义判断的 Assessment。已经归属某个 Ticket 的 Thread 后续事件通过 Subscription 直接扇出给 Worker，避免每条事件都重新经过 Router；Subscription 只提供上下文，不自动扩大授权范围。

### 3. 授权与工作状态分离

Eval 决定是否创建新的工作；Mandate 固化这次工作允许的范围、来源和 policy version；Ticket 持有长期工作状态；Assignment 表示某一次 Executor 绑定。策略变化不会把提示词中的文字直接变成权限，恢复前需要重新验证仍然有效。

### 4. 异步执行与低打扰交互

慢工具调用可以后台启动并在完成后作为新事件回到会话，前台仍可接收新的指令。面板按“需要人处理”排序，普通进度留在工作项里，避免通知泛滥。

### 5. 计算、状态、环境分离

设计方向是 Agent Loop/Session state 与执行环境分离：执行循环可以在云端，代码仓库和工具链由本地轻量 daemon 提供。这个方向为断线后继续工作和 BYOM 提供基础，但不是当前公开原型的完整生产保证。

### 6. Watchdog 与系统化恢复

独立的 Watchdog 观察 Worker 是否存活、是否持续产出、是否陷入循环或方向漂移。它与普通 Job 超时不同：目标是判断“是否有实质进展”，并把恢复、重试或人工介入变成显式状态。

## 与 Velvet 当前规划的对照

| Super-agent 要素 | Velvet 已有对应物 | 判断 |
|---|---|---|
| Router | `ApplicationCore`、`CoreTransport`、宿主命令/事件和 `CompanionView` | 拓扑兼容，但缺少独立的 Agent 编排领域和持续事件循环 |
| Worker / Executor | `PluginHost` 的隔离运行时、`JobRunner`、`JobSystem` | 执行边界兼容；不能把 Worker 直接等同为插件或普通 Job |
| Trigger / Source | `FeedMonitor`、`SourceConnector`、文件/Asset 变化和插件事件 | 需要统一的 Agent Trigger 输入和去重/游标语义；`FeedItem` 不能直接充当控制事件 |
| Eval / Mandate | `CapabilityGrant`、Profile/Workspace policy、权限检查 | 需要增加“这次 Agent 工作允许做什么”的不可变、版本化授权；CapabilityGrant 不能被模型或提示词扩大 |
| Ticket / Assignment | `Job`、attempt、checkpoint、retry | 两者粒度不同；建议增加 `AgentTask`/Ticket 层，Job 只承载一次执行和结果 |
| Subscription / Thread | `WorkspaceContext`、Core events、插件事件 | 需要 host-owned subscription、上下文扇出和“上下文不等于授权”规则 |
| Channel / Outbox | CoreTransport events、Overlay/Notification、插件适配器 | 需要提交后副作用队列和按来源回复的 Channel seam |
| Watchdog | `PluginHost` health、`JobSystem` snapshots、`PluginDiagnosticSink`、RecoverySupervisor | 可复用诊断和健康投影，但应新增 Agent progress watchdog；不能混同启动恢复监督 |
| Supervisor panel | `CompanionView` + `OperationsPanel` | 现有空间正合适：CompanionView 放 attention/控制，OperationsPanel 放所有 Job/Assignment 进度 |
| Context/memory | `StateSnapshot`、ExtensionStorage、Projection | 需要独立的 Agent context/session owner；不能把全局 Agent 记忆塞入某个插件的 ExtensionStorage |
| Remote/local split | `CoreTransport` 的未来 adapter、sidecar runtime | 可以作为未来 adapter；不应改变 v1 不拆本地 Core 的决定 |

## 方向上需要调整的地方

### A. 新增宿主拥有的 Agent 编排平面

在 `application-core` 先增加小而明确的 `AgentCoordinator`/`AgentRouter` seam，不要一开始新增通用 daemon 或独立大 crate。它应拥有：

- Agent Trigger 的接收、去重、顺序和来源游标；
- Thread/Subscription 的上下文关系；
- Eval、AgentPolicy 和 Mandate；
- 长期的 `AgentTask`/Ticket 状态；
- Assignment 与 Worker/Job 的绑定；
- 通过 CoreTransport 投影给 CompanionView/OperationsPanel 的 attention、健康和历史；
- 提交后的 Channel/Outbox 副作用。

当第二个独立客户端或远程部署真的出现，再把这条 seam 抽成独立 crate/adapter。当前阶段不应把 Router 实现成一个可被第三方插件替换的全局插件。

### B. 把 AgentTask 与 Job 分开

现有 `JobSystem` 的职责应保持不变：Job 是可观察、可取消、可重试的异步执行，拥有 attempt、progress、result、provenance 和资源预算。

Super-agent 需要的 Ticket 是更高层的长期授权工作：它可以等待 Trigger、订阅多个 Thread、经历多次 Assignment、等待人工回答、在失败后恢复。建议关系为：

```text
AgentTask/Ticket  ── owns ──> Assignment ── binds ──> Job attempt / Worker session
```

这样既保留 `JobSystem` 的单一生命周期 owner，也避免用一个 Job 快照伪装成长期 Agent 会话。Job 的 `owner` 仍是 core 或具体扩展；AgentTask 不能绕过 PluginHost 去调用运行时。

### C. 把“准入”提升为独立安全边界

现有 `CapabilityGrant` 解决“扩展在宿主中能做什么”；Agent Mandate 解决“这一次工作在什么范围内被允许”。两者需要同时满足：

```text
有效动作 = Plugin/Host CapabilityGrant ∩ Agent Mandate ∩ 当前资源/生命周期策略
```

模型上下文、评论、附件、网页或插件返回文本一律是数据，不是授权。建议首期实现：

- `off`：不接受推断的主动工作；
- `observe`：评估并记录“本来会接受什么”，但不执行；
- `auto`：只在明确规则命中且 Mandate 有效时自动接单；
- 停止、恢复、策略变更和扩展禁用都触发 Mandate revalidation；
- 每次副作用 Command/Job invocation 都由 Rust 重新检查作用域、owner、capability、revision 和 cancellation。

这比单纯给 AI Companion 一个 `Command` 能力安全得多，也与现有 deny-by-default、typed command 和 failure isolation 方向一致。

### D. 为多插件增加 Source/Channel/Worker 贡献，但禁止隐式互信

插件可以分别贡献：

- `Source`/Trigger adapter：把 Feed、Git、文件变化、外部监控转换成带来源、游标和去重键的 Trigger；
- `Worker`/Job capability：声明能处理的 `AgentTask` kind，接受受限 input，返回结构化 progress/result/attention；
- `Channel` adapter：在宿主允许的来源上发送回复或交付结果。

这些贡献都应经过 `PluginHost` 注册和 `CapabilityGrant`，而不是让插件直接调用另一个插件。Worker 之间不直接共享上下文或凭据；跨插件协作只通过 Router 的 typed projection、Subscription 和受限 handoff。

### E. 复用现有 UI 分区，不新增“超级 Agent 壳”

- `CompanionView`：显示需要人回答、批准、复核或停止的 Ticket；默认安静，健康状态不占空间。
- `OperationsPanel`：显示所有 Job、Assignment、progress、retry、logs 和失败诊断。
- `WorkspaceContext`：只提供经授权的当前 Asset/Collection/Feed 引用，不能让 Companion 直接读取数据库或插件路径。
- `CommandPalette`/Overlay：执行 stop、retry、approve、handoff 等宿主命令；命令必须有 typed context 和 revision。

这样 Agent 是跨工作台的能力投影，而不是新的 WorkbenchApp，也不会破坏 Activity Bar、Zone 和全局 OperationsPanel 的语义。

### F. Watchdog 要和 RecoverySupervisor 分开

`RecoverySupervisor` 负责启动失败、崩溃证据和 Safe Mode；Agent Watchdog 负责运行中 Worker 的进展、循环、停滞、超时和策略漂移。两者都可以复用诊断存储和 typed recovery action，但不能共享“谁拥有业务状态”的职责。

首期 Watchdog 保持进程内、低频、可取消；只有测量证明需要跨 Core 崩溃继续监督时，才增加外部 adapter。这样符合 ADR-0013 对 out-of-process supervisor 的门槛。

### G. 把云端/本地分离留作 transport adapter

DeepSeek 的 compute/state/environment 分离很有价值，但它不是 Velvet 当前 v1 的前置条件。保留：

- Rust Core 仍是本地唯一权威 owner；
- 插件运行时用 WASM/sidecar 做隔离；
- 未来的本地 daemon、远程 Agent Loop 或第二客户端实现 `CoreTransport`/Worker adapter；
- 不在 v1 为“Super-agent”再拆一个本地 daemon。

### H. 将上下文记忆作为独立 owner，延后实现

Inline/Spill、Chain of Recall 或长期记忆属于 Agent context 的压缩、分层和检索策略，不应混入 AssetFacet、Preview cache 或某个插件的 ExtensionStorage。先把 Session/Task event log、边界、大小上限、脱敏、失效和恢复语义定义清楚，再选具体记忆实现。

## 建议的实施顺序

1. 新 ADR：固定 AgentCoordinator、AgentTask/Ticket 与 JobSystem、PluginHost、CompanionView 的所有权关系。
2. P0 最小闭环：一个显式用户请求 → 一个 AgentTask → 一个受限 Worker Job → 一个结构化结果/attention；支持 stop、retry、late-result rejection 和插件失败隔离。
3. P0 事件输入：先接一个受控 Source（例如 Feed/文件变化），验证 Trigger 去重、游标、Subscription 和“上下文不产生授权”。
4. P0 准入：实现 `off/observe/auto`、Mandate、policy version、revalidation 和审计投影；默认 `observe`。
5. P0 监督 UI：CompanionView 只显示 attention 和需要人的动作；OperationsPanel 复用现有 Job 投影。
6. P1 Watchdog：对停滞、重复失败、超时和 runtime crash 给出 typed 状态和恢复动作。
7. P1 资源与恢复：把 AgentTask 的并发、上下文、子进程和网络压力接入现有 device → owner → Job 预算；为任务事件日志加 StateSnapshot/RecoverySet 规则。
8. P2 远程/本地执行分离、长期记忆、推测执行和多 Agent 实例协作；每一项都需要独立的安全、成本和故障证据。

## 必须新增的验收场景

- 恶意评论、附件或插件输出不能扩大 CapabilityGrant 或 Mandate；模型只能提出 typed action，宿主决定是否执行。
- 同一个 Trigger 重放不会重复创建 AgentTask；断线后的未知副作用结果先查询权威状态，不能盲目重放。
- 一个扩展的 Worker crash、timeout、malformed result 或 disable 只影响该扩展的 Assignment/Job 和投影；其他扩展、Asset、Workspace 和 Workbench 继续可用。
- Subscription 只扇出上下文，不赋予新权限；跨插件 handoff 重新进行 capability、scope、revision 和 policy 检查。
- Ticket 可以等待人工回答后恢复；策略变更、扩展禁用和超出 Mandate 的请求会被拒绝或重新评估。
- AgentTask 重启后可以从最后一个已提交状态继续，未提交的外部副作用不会伪装成已完成；旧 Assignment 的 late progress/result 被丢弃。
- Watchdog 能区分“慢但有进展”和“停滞/循环”，并提供 stop、retry、disable 或人工介入动作。
- CompanionView 默认只显示需要人处理的事项；健康 Worker 不产生额外通知。
- Worker 和 AgentTask 都受 device → owner → Job 预算、deadline、cancellation grace 和 backpressure 约束。
- 禁用或卸载扩展后，扩展的 AgentTask/Job 数据按其 retention/migration policy 保留或隔离，不能使核心 Asset、Workspace、布局或其他扩展失效。

## 不建议的方向

- 不要把所有插件都注册成 Agent，也不要让插件之间形成任意 Agent-to-Agent 直连网。
- 不要把 `WorkbenchApp` 或 `CompanionView` 当作 Router 的权威 owner；它们只能消费 projection 和发宿主命令。
- 不要把 `Job` 改成“万能 Agent Session”，也不要让 `JobSystem` 直接拥有权限、Source policy 或模型上下文。
- 不要用提示词、manifest 字段或 UI 所在 Zone 推断网络、文件系统、凭据、模型或进程能力。
- 不要为了仿照 DeepSeek 的云端模型而提前拆本地 Core/daemon；等第二客户端和实际远程需求出现后再做 adapter/ADR。
- 不要在没有安全边界和可观测性前开启自动推测执行；先 `observe`，再基于审计结果逐步放开。

## 对 Velvet 规划的最终判断

方向无需重写，需在“插件/任务/工作台”之上补出一层“Agent 编排与授权”。当前规划对多插件生态的关键优势——单一 Rust 权威状态、显式 CoreTransport、能力授权、每扩展失败域、共享 JobSystem、全局 Companion/Operations 分区和可恢复状态——正好是 Super-agent 落地所需的底座。

真正需要改变的是优先级和边界：在继续扩充插件种类之前，先固定 AgentTask 与 Job 的关系、Trigger/Subscription 的语义、Mandate 的准入模型、Watchdog 的 owner 以及 Agent context 的持久化边界。这样可以吸收 DeepSeek 的 Router–Worker 和事件驱动思想，同时保持 Velvet 的插件隔离和数据所有权不被一个“万能 Agent”吞掉。

## 结合自动化实践文章的调度补充

参考文章：[《折腾了整整1年，毫无保留地分享一下我这套Agent自动化系统》](https://mp.weixin.qq.com/s/2R9xtPdgYr3kF6L-LGr8aA)，作者为“数字生命卡兹克”，发表于 2026-11-02。文章不是 DeepSeek 官方设计文档，而是一套已经运行近一年的个人自动化实践；它补充了公开 superagent 原型里没有展开的运维和调度细节。

文章的核心形态是“自动化中台 + 本地执行节点 + 独立 CI 中台 + 单一人工决策出口”：各种定时任务、Webhook、消息、告警和代码事件先进入统一中台；中台把来源事件归一化，再用规则选择 Agent、模型、推理强度、Skill、工作目录、并行度和回传方式；本地机器上的接单程序通过长轮询领取任务；重复事件需要去重；真正需要人拍板的事项汇聚为决策卡片；代码变更交给独立的审查、检查、合并、发布和上线核验流水线。

这说明“自动调度”比 Router/Worker 多一层控制面，至少要明确以下语义：

1. **统一事件入口**：定时、Webhook、文件变化、Feed、Job 完成、CI 状态、人工决定和插件事件都应进入同一种带来源、对象、事件类型、时间、幂等键、关联任务和可信级别的 `AgentTrigger`。当前 Velvet 的 `ActivationRequest` 只解决启动激活，不是通用事件总线。
2. **规则路由与模型调用分离**：规则先决定是否接纳、交给哪类 Worker、使用哪个 `ExecutionProfile`、限制多少并发和成本，再由模型做语义评估。`rules.json` 可以作为原型，但生产形态必须有 schema、版本、优先级、冲突诊断、dry-run、审计和回滚；规则不能授予 manifest 没有的能力。
3. **执行节点调度**：本地 sidecar、WASM、浏览器节点、CI runner 和未来远程 worker 需要声明能力、容量、健康、区域/数据边界和租约。调度器要处理 claim、heartbeat、lease expiry、取消、重试、回收和 backpressure，而不是只把事件转成一次模型调用。
4. **模型与成本路由**：应把模型、推理深度、Token/费用预算、延迟和数据驻留作为可审计的策略输入。当前 `JobSystem` 有设备/owner/Job 资源预算，但还没有 provider、模型、订阅额度和 Token 成本账本；这会成为多模型自动化的缺口。
5. **幂等与至少一次投递**：文章指出重复事件是常态。Velvet 需要区分事件去重、任务去重、Assignment 去重和外部副作用幂等；不能承诺跨系统 exactly-once，应采用持久 inbox/outbox、source cursor、幂等 key、未知结果查询和过期策略。
6. **注意力与决策中台**：`CompanionView`/`OperationsPanel` 可以承载界面，但还需要宿主拥有的 `AttentionBroker`/`DecisionInbox`，统一聚合、去重、静默、升级、超时和唯一出口。决策卡片必须带提案、证据、影响范围、风险、过期时间、策略版本、授权范围和 revision；点击后的处理必须是幂等 typed command。
7. **决定记录与反复询问抑制**：文章把已决定和明确拒绝的事项沉淀下来。Velvet 应将其建模为带范围、来源、时间、policy revision、有效期和撤销语义的 `DecisionRecord`，不能只保存一份自然语言文件；策略变化或作用域变化时要重新验证。
8. **Skill/Runbook 作为版本化输入**：文章中的 Skill 是任务 SOP。Velvet 可以把它作为 `ExecutionProfile` 或插件贡献提供的版本化 playbook，但 Skill 只能约束步骤和输出，不能扩大 CapabilityGrant、访问路径或副作用权限。
9. **Workflow/CI DAG 高于 Job**：审查、测试、合并、发布、上线核验是有依赖和门禁的流程图。`JobSystem` 继续拥有每一步的执行生命周期；上层需要 `Workflow`/`AgentTask` 持有步骤依赖、审批门、artifact provenance、失败恢复和回滚，不应把整条流水线压成一个 Job。
10. **安全执行环境**：文章为审查和检查任务使用只读、无密钥、隔离的临时环境。Velvet 的 ADR-0005 已说明 sidecar 只是进程故障边界，不是安全沙箱；自动调度真正落地前，还要定义 runner capability profile、网络/Secret/浏览器权限、临时工作区、产物扫描和销毁证明。

因此，结合这篇文章后，原方案需要把“Agent orchestration plane”进一步明确为两个宿主服务：

```text
EventIngress + RuleRouter + AttentionBroker
                    │
          AgentTask / Workflow coordinator
                    │
       WorkerScheduler → JobSystem / PluginHost
```

第一阶段仍可把这些服务放在 `ApplicationCore` 内，保持 ADR-0001 的单部署单元；但接口必须从一开始支持多个 `ExecutionNode` 和多个模型 Provider。出现第二台执行机、独立 CI、长时间后台运行或第二客户端后，再把 EventIngress/WorkerScheduler 提取为远程 adapter 或独立 automation hub。这里需要新增的不是一个“超级 Agent WorkbenchApp”，而是一个 Core-owned control plane 及其投影。

这篇文章还改变了实施优先级：在扩充插件种类之前，先完成 `AgentTrigger`、规则版本化、执行节点租约、模型/成本预算、Attention/DecisionInbox、Workflow DAG 和端到端审计。否则多插件只会增加入口和模型数量，却没有统一的调度、回收和人工决策边界。
