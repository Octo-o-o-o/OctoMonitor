# 本地 AI Coding Agents 监控台（OctoMonitor）开发说明书

> 2026-04-16 更新：本文件保留为历史设计说明。当前实现已经按简化方案收敛，真实产品边界请以 `README.md` / `README.zh.md` 和 `docs/simplification-plan-2026-04-15.md` 为准。下文如与当前代码不一致，应以后者为准。

## 1. 项目名称与定位

项目名称暂定：**Agent Mission Control**。
一句话定位：**一个 local-first、只读优先、黑底低亮度、面向 Claude Code / Codex / OpenClaw 重度用户的统一监控台**，用于实时查看当前运行任务、当天与近 7 天的 token / cost / quota / account 状态，并支持桌面全屏、副屏、竖屏、超宽条屏，以及手机 / 平板 / Kindle 作为局域网副屏。

当前三家工具其实都已经暴露出足够多的监控面：Claude Code 提供 status line、hooks 与 OpenTelemetry；Codex 提供 hooks、本地状态、`codex exec --json` 事件流和 app-server；OpenClaw 提供 Gateway、session store、health、`status --usage` 与本地 Control UI。问题不是“拿不到数据”，而是缺少一个跨工具、跨账号、跨项目、跨 7 天的统一观察层。与此同时，sub2api / new-api 这类成熟仪表盘产品都不是一张总表，而是“总览 + 分析 + 单对象明细 + 健康面板”的组合，这正是本项目应该借鉴的结构。([Claude API Docs][1])

> 注：你提到的 `newapi3`，本说明书按公开可检索结果对应为 **QuantumNous/new-api / newapi.ai** 这条开源线处理。([New API][2])

---

## 2. 交付目标

本说明书定义的范围，就是首版完整交付范围，不再拆分 V1 / V2。

交付物包含三种运行形态：

1. **桌面 App**：Tauri 2 壳，默认全屏监控。
2. **本地 Web**：浏览器访问本机地址，体验与桌面版一致。
3. **Companion Mode**：局域网只读副屏，供 iPhone / iPad / Android / Kindle 访问。

本项目必须满足以下核心目标：

* 本地运行，不依赖云端服务。
* 不建立数据库；只读取三家原始数据，并在内存中聚合。
* 默认只读，不拦截原工具工作流。
* 一套前端渲染层，适配各种宽高比。
* 黑底、低亮度、专业监控风格。
* 3 秒内让用户看清：**谁在跑、谁在等我、今天花了多少、还剩多少**。

---

## 3. 非目标

以下内容不属于本项目首版范围：

* 不替代 Claude Code / Codex / OpenClaw 自身的运行界面。
* 不做新的 agent 平台，不承担任务调度。
* 不做云同步、不做远程账户系统、不做 SaaS。
* 不做原生 iOS / Android App。
* 不做多用户管理系统。
* 不把第三方凭证上传到任何外部服务。

---

## 4. 核心设计原则

### 4.1 Local-first

所有核心数据都来自本机文件、CLI、Gateway、hooks 或本地 HTTP/WS；默认不向外发送任何监控数据。

### 4.2 Read-only by default

默认只读；当前允许的持久化仅限于应用自身配置与 companion 配对/会话管理。环境页只提供 detect/doctor，不再承担 hooks、statusline 或任何工具配置写入。

### 4.3 One renderer, multiple shells

不做两套 UI 渲染器。不做真正的 TUI renderer。
**只保留一套 Web renderer**，桌面壳用 Tauri 2，浏览器直接访问 localhost，手机 / 平板 / Kindle 走同一套前端的 companion layout。

### 4.4 No database

不引入 SQLite / PostgreSQL / Redis 等数据库。
允许存在的本地持久化仅包括：

* 应用自身配置文件
* 局域网副屏配对信息
* 可选的用户 alias / pricing 配置

除此之外，运行状态、聚合桶、趋势数据全部在内存中维护，重启后从原始数据重建。

### 4.5 Source confidence is a first-class UI element

任何 token / cost / quota / auth 信息，都必须附带来源标签：

* `official`
* `live`
* `derived`
* `estimated`
* `heuristic`

同时显示新鲜度：

* `hot`
* `warm`
* `stale`
* `cold`

### 4.6 Dark, low-glare, high-density

黑底、低亮度、高信息密度；像专业监控，不像 BI 平台。

---

## 5. 技术选型

## 5.1 桌面层

**Tauri 2**

选择原因：

* Tauri 2 用系统原生 WebView，体积更小，支持任意前端框架，并可通过配置管理前端资源和托盘等桌面能力。([Tauri][3])
* Tauri 官方明确说明：其 event system 适合小量数据流，不适合低延迟、高吞吐的流式数据；因此本项目不应把高频 telemetry 主通道建立在 Tauri event 上，而应使用本地 HTTP + WebSocket。([Tauri][4])

结论：
**桌面壳只承担窗口、全屏、托盘、开机自启、系统权限桥接，不承担监控数据总线。**

---

## 5.2 后端内核

**Rust + Tokio + Axum + Notify + Serde + Tracing**

选择原因：

* `axum` 适合做本地 HTTP/JSON/WebSocket 服务，支持清晰的路由、extractors、responses、error handling 与 middleware 生态。([Docs.rs][5])
* `notify` 是跨平台文件系统通知库，支持 `recommended_watcher()` 自动选择平台最佳实现，也支持 `PollWatcher` 作为网络盘、WSL、某些 Docker / macOS 场景的回退方案。([Docs.rs][6])

结论：
**Rust 内核负责：适配器、文件监听、CLI/Gateway 轮询、聚合、状态机、本地 HTTP/WS 服务。**

---

## 5.3 前端

**React + TypeScript + Vite + Zustand + CSS Grid + Container Queries + 自绘 SVG 微图表**

选择原因：

* 需要同一套渲染层覆盖桌面全屏、竖屏、超宽条屏、手机、平板、Kindle。
* Container Queries 可以让组件根据“自己的容器尺寸”而不是“整个 viewport”变化，非常适合奇怪宽高比与嵌套面板布局。MDN 明确将 container queries 定义为“根据容器属性（尤其是尺寸）而不是 viewport 来应用样式”的机制。([MDN Web Docs][7])

结论：
**布局的第一原则不是 breakpoint，而是 root layout mode + panel container mode 双层适配。**

---

## 5.4 为什么不做 Electron

不选 Electron。
理由：本项目不需要独立 Chromium；Tauri 已经可以把 Web 渲染层直接装进系统 WebView，并且更轻。这个项目追求的是“最轻量、最优雅”，不是“最通用打包器”。([Tauri][3])

---

## 5.5 为什么不做第二套 TUI renderer

不做第二套真正的 terminal UI renderer。
理由：

* 同一套 Web renderer 已经可以做成 TUI-like 风格；
* Tauri + 浏览器 + 局域网 companion 都需要 Web；
* 真正双 renderer 会显著增加维护成本；
* 极宽、竖屏、手机、Kindle 场景下，Web renderer 的适配能力明显更强。

因此：
**视觉上像 TUI，技术上只有一套 Web UI。**

---

## 6. 系统架构总览

```text
agent-mission-control/
  apps/
    desktop/                 # Tauri 2 桌面壳
    web/                     # React + TS + Vite 前端
  crates/
    core/                    # 领域模型、状态机、聚合器、source confidence
    server/                  # Axum HTTP/WS API
    adapters/
      claude/
      codex/
      openclaw/
      hermes/                # 实验性适配器
    installer/               # 工具检测与诊断
    companion/               # pairing / LAN read-only access
```

### 6.1 运行流程

1. 启动服务。
2. 自动探测本机是否安装 Claude Code / Codex / OpenClaw / Hermes（实验性）。
3. 扫描最近 7 天原始数据，重建内存快照。
4. 挂上实时输入源：

   * Claude statusline / hooks / transcript tail / OTel（可选）
   * Codex hooks / app-server（增强模式）/ `exec --json`（可选）/ local state scan
   * OpenClaw Gateway / `status --usage` / `health --json` / sessions files / process poll
5. 前端先请求 `/api/bootstrap` 拿完整快照。
6. 后续只通过 `/api/stream` WebSocket 收增量 patch。
7. Companion Mode 复用同一服务和同一前端，只切 layout 与权限。

### 6.2 数据持久化策略

不建数据库。
应用自身只允许落地以下文件：

* `~/.agent-monitor/config.json`
* `~/.agent-monitor/aliases.json`
* `~/.agent-monitor/pricing.json`
* `~/.agent-monitor/pairing.json`

其余所有运行态数据不落地，只在内存中维护。

---

## 7. 数据源与适配器设计

## 7.1 Claude 适配器

### 官方可用数据面

Claude Code status line 本地运行、不消耗 API token，通过 stdin 发送结构化 JSON，字段包括 `session_id`、`transcript_path`、`workspace.project_dir`、`cost.total_cost_usd`、`cost.total_duration_ms`、累计 token、rate limit 窗口等；并且在每次新 assistant 消息、permission mode 变化、vim mode 切换后触发更新。`rate_limits` 只对 Claude.ai Pro/Max 订阅者在首个 API 响应后出现，字段可能缺失。([Claude API Docs][1])

Claude hooks 支持在 SessionStart、Notification 等阶段运行本地脚本。SessionStart 输入里包含 `session_id`、`transcript_path`、`cwd`、`source`、`model` 等字段；Notification 可区分 `permission_prompt` 与 `idle_prompt`。([Claude API Docs][8])

Claude OTel 会导出 `claude_code.cost.usage`、`claude_code.token.usage`、`claude_code.active_time.total` 等指标，但官方明确标注 `cost.usage` 只是近似值，正式计费应以 Claude Console / Bedrock / Vertex 等 provider 账单为准。([Claude API Docs][9])

Claude Code 账户来源可以是 Claude 订阅、Claude Console 或 Bedrock / Vertex / Foundry，并支持通过 `/login` 切换账户。([Claude API Docs][10])

### 适配器实现

Claude 适配器采用以下优先级：

1. **hooks**：实时生命周期事件主来源。
2. **statusline**：session 级 token / cost / duration / quota 主来源。
3. **transcript tail**：历史补全、完成态重建、失败补全。
4. **OTel**：可选增强，仅用于趋势与高级分析，不用于“官方账单”。

### Claude 接入方式

Installer 执行两件事：

1. 在本机生成一个 statusline 脚本，例如：

   * `~/.agent-monitor/bin/claude-statusline.sh`
   * `~/.agent-monitor/bin/claude-statusline.ps1`

2. 将 Claude statusLine 配置写入其 settings，官方文档示例使用 `~/.claude/settings.json`。([Claude API Docs][1])

同时安装 hooks 配置，至少覆盖：

* `SessionStart`
* `Notification(permission_prompt)`
* `Notification(idle_prompt)`
* 可选：`PostCompact`、`SubagentStop`

### Claude 状态映射规则

* `active`：最近 15 秒内有 statusline/hook 更新，且未进入 waiting 状态。
* `waitingApproval`：最近 Notification 为 `permission_prompt`。
* `idle`：收到 `idle_prompt`，且最近无后续 activity。
* `completed`：transcript 收到正常结束信号，且会话长时间无后续更新。
* `stale`：超过 freshness 阈值未收到任何更新。

### Claude 成本与配额显示规则

* `session cost`：直接显示 statusline 的 `cost.total_cost_usd`，标记为 `live-native`。
* `5h / 7d quota`：仅当 `rate_limits` 存在时显示，标记为 `official-window`。
* `OTel cost`：只用于趋势与组织级分析，标记为 `estimated`。

---

## 7.2 Codex 适配器

### 官方可用数据面

Codex 本地状态存放在 `CODEX_HOME`，默认 `~/.codex`，常见文件包括 `config.toml`、`auth.json`、`history.jsonl` 等；hooks 文件可放在 `~/.codex/hooks.json` 或 `<repo>/.codex/hooks.json`，并通过 `[features] codex_hooks = true` 启用。([OpenAI Developers][11])

Codex hooks 仍属 experimental，官方明确写明 **Windows support temporarily disabled**。hooks 事件包括 `SessionStart`、`PreToolUse`、`PostToolUse`、`UserPromptSubmit`、`Stop`；通用输入字段包括 `session_id`、`transcript_path`、`cwd`、`hook_event_name`、`model`。目前 `PreToolUse` / `PostToolUse` 在当前 runtime 下只会发出 `Bash`。([OpenAI Developers][12])

`codex exec --json` 会把 stdout 变成 JSONL 事件流，事件类型包括 `thread.started`、`turn.started`、`turn.completed`、`turn.failed`、`item.*`、`error`；样例中 `turn.completed` 会携带 `input_tokens`、`cached_input_tokens`、`output_tokens`。([OpenAI Developers][13])

Codex app-server 支持 `thread/list`、`thread/loaded/list`、`thread/read`、`thread/status/changed` 等接口；runtime 状态可区分 `notLoaded`、`idle`、`systemError`、`active`，而 `activeFlags` 可以包含 `waitingOnApproval`。app-server 的错误事件还能结构化给出 `UsageLimitExceeded`、`ContextWindowExceeded` 等错误。([GitHub][14])

Codex 登录支持 ChatGPT OAuth 和 API key；`codex login status` 可以输出当前 active authentication mode。Codex 认证会缓存到 `~/.codex/auth.json` 或操作系统 credential store；官方明确提示把 `auth.json` 当作密码看待。([OpenAI Developers][15])

### 适配器实现

Codex 适配器采用以下优先级：

1. **app-server**：增强实时模式，最高优先级。
2. **hooks**：默认实时模式。
3. **`codex exec --json`**：非交互运行或 wrapper 启动时可用。
4. **local state scan**：兜底重建与历史聚合。

### Codex 接入方式

Installer 执行以下动作：

1. 探测 `CODEX_HOME`。
2. 若系统支持 hooks：

   * 启用 `[features] codex_hooks = true`
   * 安装 `~/.codex/hooks.json`
3. 若系统为 Windows：

   * 不依赖 hooks
   * 自动回退为 local state scan + optional launcher mode
4. 提供可选 helper：

   * `agent-monitor launch codex ...`
   * 当用户愿意通过监控器启动 Codex 时，自动接 app-server 或 `exec --json`

### Codex 状态映射规则

* `active`：thread status = active
* `waitingApproval`：`activeFlags` 含 `waitingOnApproval`
* `idle`：thread status = idle
* `error`：thread status = systemError 或收到 `error` 事件
* `completed`：turn completed，且线程恢复 idle
* `limitExceeded`：错误码为 `UsageLimitExceeded`
* `contextExceeded`：错误码为 `ContextWindowExceeded`

### Codex 成本与配额显示规则

* `token usage`：以 `turn.completed.usage` 或 hooks / local state 为主。
* `cost`：

  * API key 模式：默认按用户配置的 pricing table 计算，标记 `estimated`
  * ChatGPT 模式：没有可靠美元成本时显示 `N/A`，仅显示 token / limit
* `auth mode`：通过 `codex login status` 获取，标记 `verified`

---

## 7.3 OpenClaw 适配器

### 官方可用数据面

OpenClaw 官方明确把 **Gateway** 定义为 session state 的 source of truth；UI 应向 Gateway 查询 session list 与 token counts。它同时有两层持久化：`sessions.json` 和 `<sessionId>.jsonl` transcript，默认路径位于 `~/.openclaw/agents/<agentId>/sessions/`。`sessions.json` 中会跟踪 `inputTokens`、`outputTokens`、`totalTokens`、`contextTokens` 等计数。([OpenClaw][16])

OpenClaw 的 usage tracking 直接从 provider usage / quota endpoint 拉取，官方明确写明“no estimates”；`openclaw status --usage` 会输出完整 per-provider breakdown，providers 包括 Anthropic、GitHub Copilot、OpenAI Codex OAuth、Gemini CLI、Antigravity 等。([OpenClaw][17])

`openclaw health --json` 会报告 linked creds / auth age、probe summaries、session-store summary。([OpenClaw][18])

OpenClaw Control UI 文档表明：本地 `127.0.0.1` 连接 auto-approved，而 LAN / Tailnet 等远程连接需要 explicit approval；Control UI 本身也能展示 live tool output、session list 与 per-session overrides。([OpenClaw][19])

OpenClaw 的 background process / exec 状态只存在于内存；官方文档明确说明 background sessions 在进程重启后会消失，因此不能把 process tool 当作 7 天历史的主来源。([OpenClaw][20])

### 适配器实现

OpenClaw 适配器采用以下优先级：

1. **Gateway / CLI JSON**：主来源。
2. **health / status --usage**：身份、配额、健康状态主来源。
3. **sessions.json + transcript jsonl**：历史与离线回补。
4. **process poll/log**：仅用于实时短 tail，不用于历史聚合。

### OpenClaw 接入方式

Installer 执行以下动作：

1. 探测 Gateway 是否可达。
2. 自动识别 Gateway URL、profile、bind 模式。
3. 不强制修改 OpenClaw 配置。
4. 若 Gateway 不可达，则退回 file-scan 模式。
5. 若用户启用 LAN companion，可借鉴 OpenClaw 的 pairing 逻辑做本项目自己的只读配对。

### OpenClaw 状态映射规则

* `active`：Gateway session active / recent tool output
* `idle`：会话存在但无活动
* `waitingApproval`：从 task / agent event 推断待确认
* `error`：Gateway health / session status / process fail
* `completed`：session run 或 task 完结
* `gatewayOffline`：无法连接 Gateway
* `quotaOfficial`：provider usage endpoint 可用

### OpenClaw 成本与配额显示规则

* `quota`：来自 provider endpoint，标记 `official`
* `session token counters`：来自 Gateway / sessions.json，标记 `live` 或 `derived`
* `local cost summary`：若来自 `/usage cost` 或 session logs，标记 `derived`

---

## 8. 统一领域模型

## 8.1 核心对象

### `RunRecord`

```ts
type ToolKind = 'claude' | 'codex' | 'openclaw'
type RunState =
  | 'active'
  | 'waitingApproval'
  | 'idle'
  | 'completed'
  | 'error'
  | 'stale'
  | 'gatewayOffline'
  | 'limitExceeded'
  | 'contextExceeded'

type SourceConfidence =
  | 'official'
  | 'live'
  | 'derived'
  | 'estimated'
  | 'heuristic'

type Freshness = 'hot' | 'warm' | 'stale' | 'cold'

interface RunRecord {
  id: string
  tool: ToolKind
  sourceMode:
    | 'claude_hook'
    | 'claude_statusline'
    | 'claude_transcript'
    | 'claude_otel'
    | 'codex_app_server'
    | 'codex_hook'
    | 'codex_exec_json'
    | 'codex_local_state'
    | 'openclaw_gateway'
    | 'openclaw_status'
    | 'openclaw_sessions'
    | 'openclaw_process'
  projectName: string
  workspacePath: string
  workspaceShort: string
  model?: string
  provider?: string
  agentName?: string
  accountAlias?: string
  authMode?: string
  authVerified: boolean
  sessionId?: string
  threadId?: string
  sessionKey?: string
  transcriptPath?: string
  startedAt: string
  lastActivityAt: string
  elapsedMs: number
  state: RunState
  lastAction?: string
  lastTail?: string
  pendingApproval?: boolean
  tokens: {
    input?: number
    output?: number
    cacheRead?: number
    cacheWrite?: number
    total?: number
    context?: number
  }
  cost: {
    usd?: number
    confidence: SourceConfidence
  }
  quota: {
    fiveHourUsedPct?: number
    sevenDayUsedPct?: number
    resetAt?: string[]
    confidence: SourceConfidence
  }
  source: {
    confidence: SourceConfidence
    freshness: Freshness
    lastUpdatedAt: string
  }
}
```

### `AttentionItem`

```ts
type AttentionKind =
  | 'permission'
  | 'idle'
  | 'stuck'
  | 'limit'
  | 'gateway'
  | 'auth'
  | 'source'
  | 'error'

interface AttentionItem {
  id: string
  tool: ToolKind
  runId?: string
  severity: 'info' | 'warn' | 'critical'
  kind: AttentionKind
  title: string
  detail?: string
  since: string
}
```

### `UsageBucket`

```ts
interface UsageBucket {
  scope: {
    tool?: string
    provider?: string
    account?: string
    project?: string
    model?: string
    agent?: string
  }
  window: 'today' | 'rolling24h' | 'fiveHour' | 'sevenDay' | 'week'
  start: string
  end: string
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheWriteTokens: number
  totalTokens: number
  costUsd?: number
  confidence: SourceConfidence
}
```

### `IdentityState`

```ts
interface IdentityState {
  tool: ToolKind
  authMode?: string
  provider?: string
  accountAlias?: string
  fingerprint?: string   // only when user explicitly enables
  authAge?: string
  verified: boolean
  configured: boolean
  source: SourceConfidence
}
```

### `AdapterHealth`

```ts
interface AdapterHealth {
  tool: ToolKind
  mode: string
  online: boolean
  lastSuccessAt?: string
  lastErrorAt?: string
  lastError?: string
  freshness: Freshness
}
```

### `CompletionRecord`

```ts
interface CompletionRecord {
  id: string
  tool: ToolKind
  projectName: string
  title: string
  finishedAt: string
  durationMs: number
  totalTokens?: number
  costUsd?: number
  state: 'completed' | 'error' | 'cancelled'
  summary?: string
}
```

---

## 8.2 Scope 维度

本项目不做 key 统计，但必须做 **scope 统计**。
scope switcher 必须支持：

* `tool`
* `account`
* `provider`
* `project`
* `model`
* `agent`

这来自 sub2api/new-api 的经验：真正好用的仪表盘不只是总览，还要能从“全局 → 分组 → 单对象”快速下钻。sub2api 明确拆成 `DashboardView`、`UsageView`、`KeyUsageView`；new-api README 也明确把 `Data Dashboard`、`Permission Management`、`Key quota query usage` 作为独立能力，同时前端单独存在 `ApiInfoPanel` 与 `UptimePanel` 这种健康面板。([GitHub][21])

---

## 9. 数据采集与聚合逻辑

## 9.1 启动重建

启动后扫描最近 7 天数据，构建内存索引：

* Claude：扫描 transcript_path 最近文件、已有 statusline/hook 记录
* Codex：扫描 `CODEX_HOME` 下 history / thread / auth / config 相关数据
* OpenClaw：扫描 Gateway 可达数据；不可达时扫描 `sessions.json` 与 transcript jsonl

### 内存结构

* `HashMap<RunId, RunRecord>`
* `VecDeque<AttentionItem>`
* `VecDeque<CompletionRecord>`
* today 维度 minute buckets：`1440`
* seven-day 维度 hour buckets：`168`
* recent event ring buffer：每个 run 保留最近 `200` 条

## 9.2 实时更新

### 输入通道

* 文件监听：`notify::recommended_watcher()`，必要时切 `PollWatcher`
* 本地 HTTP ingest：

  * `POST /api/ingest/claude/statusline`
  * `POST /api/ingest/claude/hook`
  * `POST /api/ingest/codex/hook`
* WebSocket / CLI poll：

  * OpenClaw Gateway
  * Codex app-server
* 定时 poll：

  * `codex login status`
  * `openclaw status --usage`
  * `openclaw health --json`

## 9.3 Freshness 规则

* `hot`：0–3 秒
* `warm`：3–15 秒
* `stale`：15–60 秒
* `cold`：60 秒以上

## 9.4 Stuck 规则

满足以下任一条件即产生 `AttentionItem(kind='stuck')`：

* state = active 且超过 `N` 秒无 lastActivity 更新
* Bash / command item 一直 in-progress 且无新输出
* Gateway 在线但该 run 长时间无事件
* transcript 无新增且工具仍自称 active

默认阈值：

* 常规 run：60 秒
* shell / process：30 秒
* companion/e-ink 不实时告警，只显示状态

## 9.5 成本规则

* **不内置固定价格表。**
* 价格表由 `pricing.json` 提供，可为空。
* 显示优先级：

  1. provider / product 原生美元字段
  2. 原生 session cost
  3. 用户价格表估算
  4. 无法计算则显示 `N/A`

### 来源标签规则

* OpenClaw `status --usage` / provider quota：`official`。([OpenClaw][22])
* Claude statusline `total_cost_usd`：`live`
* Claude OTel `cost.usage`：`estimated`。([Claude API Docs][9])
* Codex API-key 本地价格表换算：`estimated`
* 从 transcript / logs 重建：`derived`
* “下一步可能动作”之类推断：`heuristic`

---

## 10. 本地 API / IPC 设计

## 10.1 HTTP

```text
GET   /api/bootstrap
GET   /api/health
GET   /api/config
PATCH /api/config

POST  /api/ingest/claude/statusline
POST  /api/ingest/claude/hook
POST  /api/ingest/codex/hook

GET   /api/history/usage
GET   /api/history/commits
GET   /api/runs/{id}/inspect

GET   /api/remote/access
PATCH /api/remote/access
GET   /api/remote/devices
POST  /api/remote/pairings
DELETE /api/remote/devices/{device_id}
POST  /api/pair/claim
```

## 10.2 WebSocket

```text
GET /api/stream
```

事件类型：

* `snapshot.replace`
* `run.upsert`
* `run.remove`
* `attention.upsert`
* `attention.remove`
* `usage.update`
* `completion.push`
* `identity.update`
* `adapter.health`
* `config.changed`

前端只接受 typed JSON patch，不直接调用 Rust 内部状态。

## 10.3 为什么不用 Tauri event 做主通道

Tauri 官方说明其 event system 不是为低延迟、高吞吐 streaming data 设计的，因此主监控通道必须走 localhost HTTP + WS；Tauri 只用来做桌面壳和有限命令桥接。([Tauri][4])

---

## 11. 前端信息架构

## 11.1 信息架构

本地主界面保留 5 个核心 tab：

* `Monitor`
* `Usage`
* `Commits`
* `Heatmap`
* `Settings`

远程只读 viewer 只保留：

* `Monitor`
* `Usage`

详情不跳大页面，使用右侧 drawer：

* `Inspect Drawer`：`/wallboard` 内部抽屉

---

## 11.2 `/wallboard` 结构

### A. Global Rail

固定顶栏，展示：

* 当前 active runs
* waiting approvals
* blocked / error 数
* today total tokens
* today total USD
* 当前 quota summary
* adapter health summary

### B. Identity Strip

顶栏右侧，按工具展示：

* 工具名
* auth mode
* provider
* alias
* verified/configured 标记
* auth age（若可得）

### C. Running Lanes

主区域，默认按工具分三条 lane：

* Claude
* Codex
* OpenClaw

每条 lane 中每个 row 必须展示：

* 项目名
* workspace 短路径
* session / thread / sessionKey 短标识
* agent / subagent（若有）
* model
* 开始时间
* 已运行时长
* 最近活跃时间
* token
* USD
* 当前状态
* 最后一条动作
* 可选一行 tail

### D. Attention Queue

独立窄列，展示：

* 等待授权
* idle 太久
* stuck
* quota 接近上限
* gateway offline
* adapter source error
* auth 状态异常

### E. Usage Analyzer

底部面板，支持切换：

* Today
* Rolling 24h
* 5h
* 7d
* Week

展示内容：

* 总量
* 趋势线
* Top projects
* Top burners
* 维度切片：tool / account / provider / project / model / agent

### F. Recent Completions

底部或抽屉式区域，展示：

* 完成时间
* 项目
* 工具
* 任务标题 / 摘要
* duration
* token
* USD
* state

### G. Data Source Health

单独小 panel，展示：

* Claude adapter 当前模式：hook / statusline / transcript / otel
* Codex adapter 当前模式：app-server / hook / local state
* OpenClaw adapter 当前模式：gateway / sessions / process
* 最后更新时间
* last error

这部分的灵感直接来自 new-api 把 API 信息和 Uptime 单独面板化的做法。([GitHub][23])

---

## 11.3 Inspect Drawer

点击任意 run 打开右侧 drawer，字段包括：

* 基础元数据

  * tool
  * project
  * workspace full path
  * model
  * provider
  * auth mode
  * alias
  * source mode
  * transcript path
* 运行态

  * state
  * startedAt
  * lastActivityAt
  * elapsed
  * pendingApproval
* token breakdown

  * input / output / cache / total / context
* cost / quota

  * 数值
  * confidence
  * resetAt
* 近期事件时间线
* 最近工具调用
* 最近日志 tail
* source health
* copy actions

  * copy run id
  * copy transcript path
  * open workspace path

---

## 11.4 `/history` 结构

### 顶部过滤器

* 时间：today / 7d / custom last N hours
* 维度：tool / account / provider / project / model / agent

### 内容区

* 累计 token / USD 总览
* 趋势线
* 分布图
* Top 项目
* Top 模型
* Top 账号 / provider

这里要借鉴 sub2api 的思路：Dashboard 负责总览，Usage 负责分布和趋势，单对象明细负责下钻。([GitHub][21])

---

## 12. 布局与适配策略

## 12.1 Root Layout Modes

### `portrait`

触发条件：

* `aspectRatio < 0.92` 或 `width < 900`

布局：

* 顶部：Global Rail
* 中部：Running Now
* 底部：Attention / Usage 切换页签
* Recent Completions 作为抽屉

### `standard`

触发条件：

* `0.92 <= aspectRatio < 1.9`

布局：

* 12 列 grid
* 顶部：Global Rail
* 左 8 列：Running Lanes
* 右 4 列：Attention + Source Health
* 底部：Usage + Recent Completions

### `wide`

触发条件：

* `1.9 <= aspectRatio < 3.2`

布局：

* 16 列 grid
* 中央直接三条 lane：Claude / Codex / OpenClaw
* 右侧：Attention
* 下方：Usage band

### `strip`

触发条件：

* `aspectRatio >= 3.2` 或 `height < 430`

布局：

* 横向单行监控带
* 三个工具 lane 横排
* 全局摘要压成窄 rail
* 趋势图仅保留 sparkline
* 不展示大 drawer，点击用 modal

## 12.2 Panel Container Modes

每个 panel 内部再根据自身容器尺寸切：

* `full`
* `compact`
* `micro`

实现方式：

* 所有主要组件都声明 `container-type: inline-size`
* 使用 `@container` 做内部布局切换
* 使用 `cqi/cqw` 等容器单位微调字体和间距

Container Queries 的设计目的正是让组件根据容器而非 viewport 变化，适合“同一组件被放入不同布局容器”的场景。([MDN Web Docs][7])

---

## 13. Companion Mode

## 13.1 目标

让用户不额外买副屏，直接用：

* iPhone
* iPad
* Android 手机 / 平板
* Kindle / 墨水屏

查看电脑上的监控数据。

## 13.2 设计原则

* 不做原生移动 App
* 不做第二套前端
* 不增加服务器
* 仍由本地 daemon 提供页面
* Companion 默认只读

## 13.3 访问方式

默认服务仅监听 `127.0.0.1`。
用户手动开启 **Enable Companion Access** 后：

* daemon 额外绑定局域网地址
* 生成一次性 pairing token
* 在桌面 UI 显示 QR code
* 手机 / 平板扫码后进入只读 companion 页面

设计灵感来自两条已被验证的路径：

* Claude Remote Control 会在本地会话启动后显示 session URL 和 QR code，供另一台设备连接；它通过 outbound HTTPS 工作，不开入站端口。([Claude API Docs][24])
* OpenClaw Control UI 明确区分本地 auto-approved 与 LAN/Tailnet explicit approval。([OpenClaw][19])

## 13.4 安全模型

* localhost 模式：无 pairing
* LAN 模式：

  * 必须手动开启
  * pairing token 10 分钟过期
  * 首次扫码后换成只读 session cookie
  * 每个设备单独记录
  * 可在设置页 revoke

## 13.5 Companion 布局

### 手机 `/companion`

显示：

* Global Rail（精简版）
* 前 3–5 个 active runs
* Attention Queue
* Today total
* 当前 quota summary

### 平板 `/companion`

接近 `standard` 布局，但：

* 卡片密度更高
* 默认隐藏 Recent Completions 详情

### Kindle `/companion/eink`

仅保留：

* active runs
* waiting approvals
* today tokens / USD
* quota remaining
* last updated at

特性：

* 无动画
* 无彩色趋势线
* 15–60 秒轮询
* 黑白 / 高对比度模式

---

## 14. 视觉规范

## 14.1 主题

* 背景：非纯黑，接近 `#0b0f14`
* 面板底：略高一层
* 不使用玻璃态、渐变、发光、阴影秀
* 辅助文字：低亮度灰蓝
* 重要数字：高对比度单色
* 状态色只表达语义

## 14.2 字体

* 文本：`system-ui`
* 数字与路径：`ui-monospace`
* 所有数字必须开启 `font-variant-numeric: tabular-nums`

## 14.3 图表

* 不引入重图表库
* 只用自绘 SVG：

  * sparkline
  * stacked mini bars
  * progress rails
  * tiny histograms

## 14.4 动画

* 默认关闭连续动画
* 允许：

  * 1Hz elapsed 更新
  * 新告警轻微闪烁
  * skeleton / loading shimmer 极低频

---

## 15. 账号与授权信息显示规范

## 15.1 总体规则

账号显示不是主角，但必须存在。
实现方式：**Identity Strip + lane badge + inspect drawer** 三层。

显示内容仅限：

* auth mode
* provider
* user-defined alias
* verified / configured
* 可选 fingerprint（需用户显式开启）

绝不显示：

* 明文 API key
* OAuth token
* 完整邮箱（除非未来有稳定官方字段且用户开启）

## 15.2 Claude

Claude Code 支持的账户类型包括 Claude 订阅、Claude Console，以及 Bedrock / Vertex / Foundry；用户可通过 `/login` 切换。([Claude API Docs][10])

Claude 身份展示规则：

* `authMode` 值域：

  * `claude.ai`
  * `console`
  * `bedrock`
  * `vertex`
  * `foundry`
* `alias`：用户可配置，如 `work` / `personal`
* `verified`：

  * 能从登录类型 / provider 标记确认时为 true
  * 否则为 false，仅显示 `configured`

附加规则：

* 若 `rate_limits` 存在，则在 Claude badge 上显示 `subscriber limits`
* 若当前是 third-party provider，则不显示 Claude subscriber quota

## 15.3 Codex

Codex 支持 ChatGPT 和 API key 两种登录方式，`codex login status` 可打印 active authentication mode。登录缓存位于 `~/.codex/auth.json` 或 OS credential store。([OpenAI Developers][15])

Codex 身份展示规则：

* `authMode`：

  * `chatgpt`
  * `api`
* `alias`：用户自定义
* `verified`：来自 `codex login status`
* 若检测到 `forced_login_method` 或 `forced_chatgpt_workspace_id` 约束，则在 drawer 中显示 `managed`

安全要求：

* `auth.json` 只用于本地判断，不在 UI 中回显路径内敏感内容
* 不读取或显示 token 明文
* 仅在用户开启时显示指纹尾号

## 15.4 OpenClaw

OpenClaw 可以通过 `status --usage` 和 `health --json` 暴露 provider usage/quota、linked creds / auth age。([OpenClaw][22])

OpenClaw 身份展示规则：

* `provider`：anthropic / openai-codex / gemini / antigravity / copilot ...
* `authMode`：oauth / api-key / token / profile-based
* `authAge`：若 `health --json` 可得则显示
* `verified`：来自 Gateway / health
* 可选附加：

  * endpoint/api mode
  * profile 名称

## 15.5 身份状态标签

UI 文案固定三档：

* `verified`
* `configured`
* `alias-only`

---

## 16. Environment / Doctor 设计

## 16.1 功能

`Settings -> Environment / Doctor` 区域必须支持：

* 自动探测四类工具（Hermes 为 experimental）
* 显示当前可接入能力矩阵
* 运行 doctor

## 16.2 诊断流程

### Claude

* 探测 `claude` 命令
* 诊断 statusline / hook 路径是否可用
* 报告当前环境是否满足监控接入条件

### Codex

* 探测 `codex` 命令
* 探测 `CODEX_HOME`
* 报告 hooks / app-server 路径是否可用
* Windows 上将 hooks 视为可选增强项，而不是必需能力

### OpenClaw

* 探测 `openclaw` 命令
* 探测 Gateway
* 探测 profile / bind / auth 状态

### Hermes

* 探测 `hermes` 命令
* 探测 gateway / sessions 路径
* 标记为 experimental adapter
* 不默认改写其配置，只读取

## 16.3 回滚

Installer 必须支持：

* 恢复 Claude settings 备份
* 恢复 Codex hooks/config 改动
* 删除本项目注入脚本
* 清理 pairings

---

## 17. 键盘交互

因为 UI 要保持 TUI-like 体验，必须支持键盘优先：

* `j/k`：上下选择 run
* `h/l`：lane 间移动
* `Enter`：打开 inspect drawer
* `Esc`：关闭 drawer
* `/`：搜索项目 / 路径 / session
* `1..6`：切 scope 维度
* `t`：切 today / 7d
* `a`：聚焦 Attention Queue
* `u`：聚焦 Usage Analyzer
* `r`：手动 refresh
* `m`：切密度模式
* `f`：全屏
* `?`：快捷键帮助

---

## 18. 功能清单

以下功能全部属于首版交付范围：

### 18.1 运行态监控

* 检测并展示当前活跃 Claude / Codex / OpenClaw 任务
* 展示项目、开始时间、运行时长、当前状态、token、USD
* 展示 waiting approval / idle / stuck / error
* 展示最近完成任务

### 18.2 使用量与成本

* today 总 token
* today 总 USD
* 近 7 天趋势
* 5h / 7d / rolling24h / week 窗口
* scope 切换分析

### 18.3 配额与限制

* Claude subscriber quota（可得时）
* OpenClaw official provider quota
* Codex 当前 auth mode / limits 信息（能确认时）

### 18.4 身份与账号

* 当前 auth mode
* 当前 provider
* alias
* verified/configured
* auth age（若可得）

### 18.5 数据源健康

* adapter 在线状态
* 当前 source mode
* 最后更新时间
* 最近错误

### 18.6 明细与诊断

* Inspect Drawer
* 近期事件
* 工具调用记录
* 日志 tail
* transcript path / workspace path copy

### 18.7 环境与维护

* environment / doctor
* tool detection
* re-pair / revoke companion device

### 18.8 Companion Mode

* LAN pairing
* 手机 / 平板视图
* Kindle/e-ink 低刷新视图

---

## 19. 关键实现注意点

Claude statusline 的 `rate_limits` 字段只在 Claude.ai Pro/Max 订阅且“首个 API 响应后”才出现，因此 UI 必须把 quota 视为 nullable，不得把“没有字段”误显示为“0%”。Claude 的 `context_window.current_usage` 也可能在 session 早期为 `null`。([Claude API Docs][1])

Claude statusline 会在 permission prompt 时临时隐藏，因此“等待授权”不应只靠 statusline 判断，而必须依赖 Notification hook。([Claude API Docs][1])

Codex hooks 仍属 experimental，并且官方明确写了 Windows 暂时禁用；因此 Windows 上不能把 hooks 当唯一实时来源。([OpenAI Developers][12])

Codex 的 `auth.json` 含有访问 token，官方明确提示要把它当密码看待；本项目只能读取最小必要信息，禁止日志中回显其内容。([OpenAI Developers][13])

OpenClaw 的 Gateway 才是 source of truth；本地 `sessions.json` 和 transcript 只能作为 fallback。remote mode 下，读取本地文件可能完全不反映真实运行态。([OpenClaw][16])

OpenClaw process/background session 只在内存中存在，进程重启即失效，因此 process tool 只能做“当前 tail”，不能做 7 天历史。([OpenClaw][20])

文件监听层必须支持 poll fallback。`notify` 文档明确指出在 NFS、WSL、某些 Docker/macOS 场景下，原生事件可能不可用或不可靠，需用 `PollWatcher` 兜底。([Docs.rs][6])

局域网 companion 必须默认关闭，开启时必须 read-only，并使用 pairing token；不要直接把本地监控页面裸露在 LAN。OpenClaw 的 Control UI 明确区分本地 auto-approved 与远程 explicit approval，这个经验应直接沿用。([OpenClaw][19])

---

## 20. 配置文件建议

## 20.1 `config.json`

```json
{
  "listen": {
    "host": "127.0.0.1",
    "port": 46321
  },
  "historyDays": 7,
  "freshness": {
    "hotSec": 3,
    "warmSec": 15,
    "staleSec": 60
  },
  "stuckThresholds": {
    "defaultSec": 60,
    "processSec": 30
  },
  "companion": {
    "enabled": false,
    "host": "0.0.0.0",
    "port": 46321,
    "readOnly": true
  },
  "ui": {
    "density": "auto",
    "theme": "dark-monitor",
    "showFingerprints": false
  }
}
```

## 20.2 `aliases.json`

```json
{
  "claude": {
    "claude.ai": "work-claude",
    "console": "billing-console"
  },
  "codex": {
    "chatgpt": "openai-main",
    "api": "openai-api"
  },
  "openclaw": {
    "anthropic:default": "anthropic-oauth",
    "openai-codex:main": "codex-oauth"
  }
}
```

## 20.3 `pricing.json`

```json
{
  "codex": {
    "api": {
      "defaultCurrency": "USD",
      "models": {}
    }
  },
  "claude": {
    "thirdPartyFallback": {
      "models": {}
    }
  }
}
```

---

## 21. 建议仓库结构

```text
agent-mission-control/
  apps/
    web/
      src/
        app/
        routes/
          wallboard/
          history/
          settings/
          companion/
          companion-eink/
        components/
          global-rail/
          identity-strip/
          running-lanes/
          attention-queue/
          usage-analyzer/
          recent-completions/
          inspect-drawer/
          source-health/
        styles/
          tokens.css
          layouts.css
          components.css
    desktop/
      src-tauri/
        src/
          main.rs
          commands.rs
          windows.rs
          tray.rs
          autostart.rs
  crates/
    core/
      src/
        models.rs
        state.rs
        aggregator.rs
        freshness.rs
        confidence.rs
    server/
      src/
        api.rs
        ws.rs
        auth.rs
    adapters/
      claude/
        src/
          statusline.rs
          hooks.rs
          transcript.rs
          otel.rs
      codex/
        src/
          hooks.rs
          app_server.rs
          exec_json.rs
          state_scan.rs
          auth.rs
      openclaw/
        src/
          gateway.rs
          status.rs
          health.rs
          sessions.rs
          process.rs
    installer/
      src/
        lib.rs
    companion/
      src/
        pairing.rs
        qr.rs
        sessions.rs
  scripts/
    claude-statusline.sh
    claude-hook.sh
    codex-hook.sh
```

---

## 22. 定义完成标准（Definition of Done）

以下条件全部满足，才算首版完成：

1. 在装有 Claude Code / Codex / OpenClaw 的本机上，应用能自动探测三者并显示接入状态。
2. Claude：

   * 能显示当前 session 的项目、开始时间、时长、token、USD
   * 能显示 permission / idle 状态
   * quota 缺失时不误报
3. Codex：

   * 能显示当前 run / thread 的项目、开始时间、时长、token
   * 有 hooks 时可实时刷新
   * 有 app-server 时可展示 waitingOnApproval
   * Windows 可在无 hooks 情况下正常回退
4. OpenClaw：

   * Gateway 在线时优先使用 Gateway 数据
   * 能展示 provider quota / usage
   * 能展示 auth age / health
5. `/wallboard` 在以下模式均可用：

   * 竖屏
   * 普通横屏
   * 宽屏
   * 4:1 条屏
6. `/companion` 可通过二维码在手机或平板访问。
7. `/companion/eink` 可在 Kindle / 墨水屏上稳定显示。
8. 所有 cost / quota / identity 字段都附带 confidence / freshness。
9. 应用重启后可从原始数据重建最近 7 天历史，不依赖数据库。
10. 默认不泄露任何敏感凭证，不显示明文 token / key。

---

## 23. 调研来源摘要

### Claude Code 官方文档

* statusline：本地运行、不耗 API token、stdin JSON、`session_id` / `transcript_path` / `workspace.project_dir` / `total_cost_usd` / token / `rate_limits` / 更新时机。([Claude API Docs][1])
* hooks：`SessionStart`、`Notification(permission_prompt / idle_prompt)` 等生命周期事件。([Claude API Docs][8])
* monitoring / OTel：`claude_code.cost.usage`、`claude_code.token.usage`、近似成本说明。([Claude API Docs][9])
* quickstart / login：Claude 订阅、Console、Bedrock / Vertex / Foundry、`/login` 切换。([Claude API Docs][10])
* remote control：session URL、QR code、手机访问、outbound HTTPS only。([Claude API Docs][24])

### Codex 官方文档

* advanced config：`CODEX_HOME`、`~/.codex`、`config.toml`、`auth.json`、`history.jsonl`、`hooks.json`。([OpenAI Developers][11])
* hooks：experimental、Windows disabled、`SessionStart` / `PreToolUse` / `PostToolUse`、`session_id` / `transcript_path` / `cwd`。([OpenAI Developers][12])
* non-interactive：`codex exec --json` JSONL 事件流。([OpenAI Developers][13])
* app-server：`thread/list`、`thread/loaded/list`、`thread/status/changed`、`waitingOnApproval`、`UsageLimitExceeded`。([OpenAI Developers][25])
* auth / CLI reference：ChatGPT vs API key、`codex login status`、`auth.json` 敏感性。([OpenAI Developers][15])

### OpenClaw 官方文档

* session management deep dive：Gateway 是 source of truth；`sessions.json` 与 transcript jsonl；token counters；默认路径。([OpenClaw][16])
* usage tracking / CLI：provider usage / quota 直接来自 provider endpoint，`status --usage`，no estimates。([OpenClaw][17])
* health：linked creds / auth age。([OpenClaw][18])
* control UI：本地 auto-approved，远程 explicit approval，session / live tool output。([OpenClaw][19])
* process / exec：background session 仅内存存在。([OpenClaw][20])
* ACP agents：未来可扩展接入 Claude Code / Codex / Cursor 等。([OpenClaw][26])

### 仪表盘参考项目

* sub2api：README 中明确有 multi-account、API key distribution、precise billing、admin dashboard，并存在 `sub2api-mobile`（iOS / Android / Web）移动管理台；前端结构拆分为 `DashboardView`、`UsageView`、`KeyUsageView`。([GitHub][27])
* new-api：README 明确包含 `Data Dashboard`、`Permission Management`、`Key quota query usage`；前端组件存在 `ApiInfoPanel` 和 `UptimePanel`。([GitHub][28])

### 技术栈参考

* Tauri 2：系统 WebView、支持任意前端框架、配置前端资源、event 不适合高吞吐 streaming。([Tauri][3])
* axum：HTTP/JSON/WebSocket、extractors、responses、middleware。([Docs.rs][5])
* notify：跨平台文件通知、recommended_watcher、PollWatcher fallback。([Docs.rs][6])
* CSS Container Queries：按容器而非 viewport 自适应。([MDN Web Docs][7])

以上即为完整开发说明书。

[1]: https://docs.anthropic.com/en/docs/claude-code/statusline "https://docs.anthropic.com/en/docs/claude-code/statusline"
[2]: https://www.newapi.ai/en "https://www.newapi.ai/en"
[3]: https://v2.tauri.app/start/ "https://v2.tauri.app/start/"
[4]: https://v2.tauri.app/develop/calling-frontend/ "https://v2.tauri.app/develop/calling-frontend/"
[5]: https://docs.rs/axum/latest/axum/ "https://docs.rs/axum/latest/axum/"
[6]: https://docs.rs/notify "https://docs.rs/notify"
[7]: https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Containment/Container_queries "https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Containment/Container_queries"
[8]: https://docs.anthropic.com/en/docs/claude-code/hooks "https://docs.anthropic.com/en/docs/claude-code/hooks"
[9]: https://docs.anthropic.com/en/docs/claude-code/monitoring-usage "https://docs.anthropic.com/en/docs/claude-code/monitoring-usage"
[10]: https://docs.anthropic.com/en/docs/claude-code/quickstart "https://docs.anthropic.com/en/docs/claude-code/quickstart"
[11]: https://developers.openai.com/codex/config-advanced/ "https://developers.openai.com/codex/config-advanced/"
[12]: https://developers.openai.com/codex/hooks/ "https://developers.openai.com/codex/hooks/"
[13]: https://developers.openai.com/codex/noninteractive/ "https://developers.openai.com/codex/noninteractive/"
[14]: https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md "https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md"
[15]: https://developers.openai.com/codex/cli/reference/ "https://developers.openai.com/codex/cli/reference/"
[16]: https://docs.openclaw.ai/reference/session-management-compaction "https://docs.openclaw.ai/reference/session-management-compaction"
[17]: https://docs.openclaw.ai/concepts/usage-tracking "https://docs.openclaw.ai/concepts/usage-tracking"
[18]: https://docs.openclaw.ai/gateway/health "https://docs.openclaw.ai/gateway/health"
[19]: https://docs.openclaw.ai/web/control-ui "https://docs.openclaw.ai/web/control-ui"
[20]: https://docs.openclaw.ai/gateway/background-process "https://docs.openclaw.ai/gateway/background-process"
[21]: https://github.com/Wei-Shaw/sub2api/blob/main/frontend/src/views/admin/DashboardView.vue "https://github.com/Wei-Shaw/sub2api/blob/main/frontend/src/views/admin/DashboardView.vue"
[22]: https://docs.openclaw.ai/cli "https://docs.openclaw.ai/cli"
[23]: https://raw.githubusercontent.com/QuantumNous/new-api/main/web/src/components/dashboard/ApiInfoPanel.jsx "https://raw.githubusercontent.com/QuantumNous/new-api/main/web/src/components/dashboard/ApiInfoPanel.jsx"
[24]: https://docs.anthropic.com/en/docs/claude-code/remote-control "https://docs.anthropic.com/en/docs/claude-code/remote-control"
[25]: https://developers.openai.com/codex/app-server/ "https://developers.openai.com/codex/app-server/"
[26]: https://docs.openclaw.ai/tools/acp-agents "https://docs.openclaw.ai/tools/acp-agents"
[27]: https://github.com/Wei-Shaw/sub2api/blob/main/README.md "https://github.com/Wei-Shaw/sub2api/blob/main/README.md"
[28]: https://raw.githubusercontent.com/QuantumNous/new-api/main/README.md "https://raw.githubusercontent.com/QuantumNous/new-api/main/README.md"
