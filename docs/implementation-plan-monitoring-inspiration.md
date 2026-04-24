# 借鉴参考监督台的实施计划

源文档：`docs/reference-monitoring-inspiration.md`

日期：2026-04-24

目标：把参考监督台中适合 OctoMonitor 的读侧能力拆成可独立交付的工程步骤，并明确每一步的输入、输出、安全边界和验收标准。

## 0. Review 结论

本计划以“只读观测增强”为主线。控制类能力、源工具写回、多机 bootstrap、WS delta、annotation layer 暂不进入近期路线。

相对源文档，需要做以下实施级订正：

| 议题 | 判断 | 实施决策 |
| --- | --- | --- |
| `token_count` | snapshot 级 token/quota 抽取已存在，但事件 timeline 仍缺 token_count 事件 | Phase 1 保留 `TokenCount` 事件，复用现有抽取逻辑，只暴露汇总数字 |
| local / remote router | 当前是 main router + 独立 remote router，不存在“local admin router”类型 | 新端点只挂 `main.rs::build_app`，绝不挂 `remote_access::build_remote_router` |
| parser 文件位置 | 事件 parser 会明显增加 adapter 体量 | 新建 `crates/adapters/codex/src/events.rs`，避免继续膨胀 `lib.rs` |
| event kind | task marker、web search、context 是后续 progress/timeline 的关键信号 | Phase 1 直接保留完整事件枚举，不做 6 个 variant 的过度裁剪 |
| progress 映射 | adapter crate 不能依赖 core 的 `RunState` | adapter 暴露 Codex 自身的 progress hint；server/probe 负责映射到 `RunState` |
| resume command | 实测 `codex resume --help`（codex-cli 0.124.0）：`Usage: codex resume [OPTIONS] [SESSION_ID] [PROMPT]`；SESSION_ID 是位置参数（UUID 或 thread name），`--all` 只是"显示全部 session 列表"的 flag，不接 id | Codex 建议命令使用 `codex resume <thread_id>`（`RunRecord.thread_id` 即 Codex session UUID，参考 `probe.rs:1181`）；未知工具返回 unavailable |
| Copy fallback | `document.execCommand('copy')` 过时且增加复杂度 | 仅用 `navigator.clipboard.writeText`；失败时 toast 提示 |
| 快捷筛选持久化 | quick filter 是视图状态，不是长期设置 | 先放 transient UI/store state，不写 `preferences.ts` |
| events fallback | 非 Codex run 仍应使用旧 inspect | `/events` 返回 `supported: false`；前端对非 Codex 或 unsupported 继续用 `/inspect` |
| 粘底滚动 | timeline 轮询会持续追加事件 | Phase 3 做简单 near-bottom lock，不延期 |
| Phase 4 元数据 | `thread_name` 已读，`deleted_sessions` 当前未扫描，SQLite ROI 低 | Phase 4 只保留为可选评估，不列当前必做项 |
| redact_run 一致性 | Phase 2 `last_tail` 填短 progress reason 时必须满足 remote 模式已清空 `last_tail` | `remote_access::redact_run` 当前就把 `last_tail` 置 `None`；Phase 2 不新增字段，避免联动扩脱敏表 |
| CodexSession 字段扩展 | Phase 2 需要 `progress_kind` / `progress_reason` / `recent_tools` / `turn_open` | 这些字段仅存在于 `CodexSession`（adapter 内部），不进 `RunRecord`；`probe` 在构造 `RunRecord` 时消费它们，产出 `state` 与 `last_tail` |

## 1. Phase 依赖

```text
Phase 0: Copy / Toast / resume-command / 标题降噪
    独立，可先做

Phase 1: Codex JSONL event parser + tail cursor + tests
    是 Phase 2 和 Phase 3 的强依赖

Phase 2: Codex progress hint + Monitor quick filter
    依赖 Phase 1 parser，不阻塞 Phase 3 UI endpoint

Phase 3: /api/runs/{id}/events + InspectDrawer timeline
    依赖 Phase 1 parser

Phase 4: 元数据补强评估
    只在 Phase 0-3 后仍有真实缺口时做
```

每个 Phase 动工前先跑一次基线：

```bash
cargo test --workspace
pnpm --filter @octomonitor/web test --run
```

涉及前端类型或构建路径时，再跑：

```bash
pnpm --filter @octomonitor/web build
```

涉及共享 Rust API 或发布前检查时，再跑：

```bash
cargo clippy --workspace -- -D warnings
```

## 2. Phase 0：只读 UX 小补丁

目标：低风险提升日常可用性，不改变数据模型和控制边界。

### 2.1 CopyButton + Toast

新增：

- `apps/web/src/components/common/CopyButton.tsx`
- `apps/web/src/components/common/Toast.tsx`（container + 单条 toast 渲染）
- `apps/web/src/store/toastStore.ts`：独立 Zustand store，沿用 `monitorStore` 风格，避免和业务数据耦合。暴露 `pushToast({ kind, message, durationMs? })` 和 `dismissToast(id)`。

行为：

- 使用 `navigator.clipboard.writeText`。
- 成功 toast：`已复制`。
- 失败 toast：`复制失败`。
- 不使用 `document.execCommand('copy')` fallback。
- Toast 容器挂载在 `App.tsx` 根级（local 和 remote runtime 共用）。

InspectDrawer 接入：

- run id。
- thread id / session id，存在才显示。
- workspace path。
- transcript path，仅 local 模式显示。
- resume command，存在才显示。

remote viewer：

- redacted 字段不渲染 CopyButton。
- 不显示 transcript path / workspace path / resume command 复制入口。

### 2.2 Resume Command API

新增：

```text
GET /api/runs/{run_id}/resume-command
```

响应：

```json
{
  "command": "codex resume <thread_id>",
  "tool": "codex",
  "note": null
}
```

规则：

- 只挂 `crates/server/src/main.rs::build_app`。
- 不挂 `crates/server/src/remote_access.rs::build_remote_router`。
- 只构造字符串，不执行、不写日志、不做权限校验。
- Codex：`run.thread_id` 存在时返回 `codex resume <thread_id>`（SESSION_ID 作为位置参数；`probe.rs:1181` 已把 Codex session UUID 写入 `RunRecord.thread_id`）。
- Codex 无 `thread_id`：`command: null`，说明缺少 thread id。
- Claude / OpenClaw / Hermes：MVP 返回 `command: null` 和明确 unavailable note。
- UI 标注为“复制建议命令”，不承诺一定可执行。

实现位置：

- 可新增 `crates/server/src/handlers/resume.rs`。
- `handlers/mod.rs` 暴露模块。
- `main.rs` 挂路由。

测试：

- Codex with thread id -> 返回 command。
- Codex without thread id -> `command: null`。
- non-Codex -> `command: null`。
- remote router unknown API -> 404。

### 2.3 标题降噪

新增纯函数：

```rust
fn looks_noisy_title(value: &str) -> bool
fn choose_codex_display_title(session: &CodexSession) -> String
```

建议放在 Codex adapter 内，或 adapter common 中；先不要为了跨工具复用重构。

噪声判定：

- JSON object/array 开头：`{`、`[`。
- markdown fence 开头。
- system/instruction 明显前缀。
- 超长结构化单行。
- 空白或只有符号。

显示标题优先级：

1. `last_question` 非 noisy。
2. `first_question` 非 noisy。
3. `thread_name` 非 noisy。
4. session id 短前缀。

注意：

- 不要改写源 JSONL。
- 不要改变 `CodexSession` 原始字段含义；降噪**只作用于 server/probe 映射到 `RunRecord` 时**：`probe.rs::build_run_from_codex_session` 在填 `first_question` / `last_question` 前先调用 `choose_codex_display_title`，若原值 noisy 则用降级候选替换；**不引入新的 `display_title` 字段**，避免扩 `RunRecord` schema 和联动 `redact_run`。
- `redact_run` 不需要改动：Codex RunRecord 的 `first_question` / `last_question` 在 remote 模式下本就被清空（`remote_access.rs`）。
- Codex adapter 输出的原始 `first_question` / `last_question` 保留未降噪值，方便后续测试/排查。

### 2.4 验收

- `cargo test --workspace` 通过。
- `pnpm --filter @octomonitor/web test --run` 通过。
- 前端改动涉及 TS 构建时，`pnpm --filter @octomonitor/web build` 通过。
- 本地 InspectDrawer 可复制可见字段。
- remote viewer 不显示被 redaction 的复制入口。
- `/api/runs/{id}/resume-command` 只在本地服务可用。
- noisy 标题在 Monitor 主列表降级。

## 3. Phase 1：Codex JSONL 事件纯函数

目标：实现可测的事件解析、尾部读取和去重，不挂 HTTP，不改 UI。

### 3.1 模块与类型

新增：

- `crates/adapters/codex/src/events.rs`
- `pub mod events;` 或按当前 crate 风格 re-export 必要类型/函数

事件类型：

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexEventKind {
    UserMessage,
    AssistantMessage,
    Reasoning,
    ToolCall,
    ToolOutput,
    WebSearch,
    TokenCount,
    TaskStarted,
    TaskComplete,
    TaskAborted,
    TurnAborted,
    Context,
}
```

事件结构：

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexEvent {
    pub kind: CodexEventKind,
    pub timestamp: String,
    pub turn_id: Option<String>,
    pub title: String,
    pub preview: String,
    pub text: Option<String>,
    pub tool_name: Option<String>,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub call_id: Option<String>,
}
```

限制：

- `preview` 单条最多 400 字符。
- `text` 单条最多 2000 字符。
- `command` 最多 400 字符。
- 去除 ANSI 控制序列。
- `timestamp` 使用 JSONL 原始字符串（Codex 侧已是 RFC 3339 / ISO 8601）；无 timestamp 时填空字符串，不伪造。
- `turn_id` 在 `event_msg.task_started` / `task_complete` / `turn_aborted` 载荷中存在；其他事件无则置 `None`。（实测 2026-04 session 分布确认。）

### 3.2 函数

```rust
pub fn parse_session_event_line(line: &str) -> Vec<CodexEvent>;

pub fn parse_exec_output(raw: &str) -> ExecOutput;

pub fn dedupe_adjacent(events: Vec<CodexEvent>) -> Vec<CodexEvent>;

pub fn read_tail_events(
    path: &Path,
    cursor: Option<u64>,
    byte_limit: usize,
    max_events: usize,
) -> std::io::Result<TailReadResult>;
```

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecOutput {
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub body: Option<String>,
}

pub struct TailReadResult {
    pub events: Vec<CodexEvent>,
    pub cursor: u64,
    pub reset: bool,
}
```

`parse_exec_output` 的策略：Codex 输出有两种格式：

1. **Text 格式**（来自 `function_call_output.output`）：
   ```
   Chunk ID: b6af7f
   Wall time: 0.0000 seconds
   Process exited with code 0
   Original token count: 12
   Output:
   <body>
   ```
   用行匹配抽 `exit_code`（"Process exited with code N"），`body` 取 `Output:` 之后的所有内容。

2. **JSON 格式**（来自 `custom_tool_call_output.output`）：
   ```json
   {"output":"<body>","metadata":{"exit_code":0,"duration_seconds":0.0}}
   ```
   先 `serde_json::from_str`；成功则取 `output` 字段为 body、`metadata.exit_code` 为 exit code。

`command` 不在 output 里，由 `parse_session_event_line` 从上一条 `function_call.arguments` 的 JSON 字符串抽 `cmd` 字段后组装（如果存在）。`parse_exec_output` 只返回 `(exit_code, body)`；如果两种格式都不匹配，把 `raw` 整段当 body。body 和 command 都做长度裁剪与 ANSI 控制序列去除。

Cursor 语义：

- 无 cursor：从 `max(0, file_len - byte_limit)` 起读，返回尾部最多 `max_events` 条事件，cursor 为文件末尾 offset。
- 有 cursor：从 offset 起增量读，返回新事件和新 cursor。
- cursor 大于文件大小：`reset = true`，执行一次尾部读。
- 文件末尾未换行的半行不解析。

默认建议：

- `byte_limit = 256 * 1024`。
- `max_events = 600`。

去重：

- 仅相邻去重。
- key 使用 `(kind, preview, text, tool_name, command, exit_code)`。
- 不跨请求窗口维护全局去重状态。

### 3.3 解析要求

- invalid JSON 返回空 vec。
- `response_item.message` -> user/assistant event。
- `event_msg.user_message` / `event_msg.agent_message` -> user/assistant event。
- `response_item.reasoning` -> reasoning preview。
- `function_call` / `custom_tool_call` -> tool call。
- `function_call_output` / `custom_tool_call_output` -> tool output。
- `web_search_call` -> web search。
- `event_msg.task_started` / `task_complete` / `task_aborted` / `turn_aborted` -> task marker。
- `event_msg.token_count` -> 汇总 usage/rate-limit，不暴露 raw token payload。
- `turn_context` -> context event。

### 3.4 测试

新增单测：

- `parse_user_message_line`
- `parse_assistant_message_line`
- `parse_reasoning_line`
- `parse_function_call_produces_tool_call`
- `parse_function_call_output_has_command_exit_body`
- `parse_web_search_call`
- `parse_task_markers`
- `parse_token_count_carries_summary_only`
- `parse_invalid_json_returns_empty`
- `parse_partial_last_line_ignored`
- `dedupe_adjacent_collapses_identical_pairs`
- `read_tail_events_respects_byte_limit`
- `read_tail_events_returns_reset_on_truncate`

验收：

- `cargo test -p octomonitor-codex-adapter` 通过。
- 该 Phase 的 diff 只落在 Codex adapter 及其测试。

## 4. Phase 2：Progress Summary + Monitor 筛选栏

目标：让主列表的状态更可信，并提供任务态快捷筛选。

### 4.1 Adapter 输出 Progress Hint

不要让 adapter 依赖 core 的 `RunState`。

在 adapter 内维护并输出轻量 Codex progress hint，例如：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexProgressKind {
    Running,
    Waiting,
    Completed,
    Aborted,
    Unknown,
}
```

可加入 `CodexSession` 的字段：

- `progress_kind: Option<CodexProgressKind>`
- `progress_reason: Option<String>`，最多 80 字符
- `recent_tools: Vec<String>`，最多 3 个
- `turn_open: bool`

这些字段来自 Phase 1 parser，不重复写一套 JSONL 解析。

### 4.2 Server 映射到 RunRecord

`crates/server/src/probe.rs::classify_codex_session_state` 负责映射（**严格按顺序命中即返回**）：

1. `has_pending_approval` 且最近活动 < 30min -> `WaitingApproval`。
2. `(progress_kind == Running || turn_open) && age < 5min` -> `Active`。  
   （加 5min 上限防止 stuck session 永久显示 Active；超时则继续下一条。）
3. `progress_kind == Completed` -> `Completed`。
4. `progress_kind == Aborted` -> `Error`，并把 `last_tail` 填 `Turn aborted`。
5. `progress_kind == Waiting`（非 approval，纯观察到 user_message 挂起）-> 沿用 age-based：< 2min `Active`，< 10min `Idle`，否则 `Completed`。
6. `progress_kind == Unknown` 或字段缺失 -> fallback 到现有 age-based 规则（`age < 2min` Active，`< 10min` Idle，否则 Completed）。

`turn_open` 语义：parser 扫描尾部事件窗口时，若观察到 `UserMessage` 后未见到对应的终结事件（`TokenCount` / `TaskComplete` / `TurnAborted` / `AssistantMessage` 配对），则置 true；否则 false。这是 "用户发完消息、等 assistant 回应" 的窗口。

`last_tail`：

- 填短文案，例如 `Running tool: shell_command`、`Task complete`、`Turn aborted`。
- 上限 80 字符。
- 禁止原始 output、命令参数全文、assistant 长文本、secret。

暂不新增 `RunRecord.progress` 字段。只有 UI 证明 `last_tail` 不够时，再扩 schema，并同步 `remote_access.rs::redact_run`。

### 4.3 MonitorFilterBar

新增 `apps/web/src/components/monitor/MonitorFilterBar.tsx`，挂载在 `MonitorView` 主列表顶部（标题/meta 行之下、分组之上）。

筛选：

- `all`
- `attention`：`waitingApproval` 或 error 类状态。
- `active`：`active`。
- 搜索：匹配 `projectName`、`workspaceShort`、`lastQuestion`、`firstQuestion`、run id 前缀。

状态存储：

- quick filter 和 search 先作为 transient UI state。
- 不写入 `preferences.ts`。
- 不新增按工具 chip。
- 不新增按时间 chip。

快捷键：

- 搜索框落地后，在现有 `useKeyboardShortcuts` 中增加 `/` 聚焦。
- 输入框聚焦时不触发全局快捷键。

remote viewer：

- 可复用 quick filter。
- 搜索只匹配 remote payload 中未 redaction 的字段。

### 4.4 测试与验收

测试：

- adapter progress hint 单测。
- server `classify_codex_session_state` 单测。
- `MonitorFilterBar` 组件测试。
- `buildVisibleRunsBySource` 不被 quick filter 破坏；quick filter 在组件层或单独 helper 中处理。

验收：

- `cargo test --workspace` 通过。
- `pnpm --filter @octomonitor/web test --run` 通过。
- Codex 工具调用中的 run 显示短 progress reason。
- 刚 `task_complete` 的 run 不再被时间窗口误判为 Active。
- quick filter 不与 Settings 中的 `FilterRules` / `MonitorPeriod` 重复。

## 5. Phase 3：Events Endpoint + InspectDrawer Timeline

目标：本地 drawer 展示 Codex 最近事件；remote viewer 不暴露。

### 5.1 API

新增：

```text
GET /api/runs/{run_id}/events?cursor=<u64>&limit=<usize>
```

响应：

```json
{
  "supported": true,
  "tool": "codex",
  "events": [],
  "cursor": 12345,
  "reset": false
}
```

规则：

- 只挂 `main.rs::build_app`。
- 不挂 `remote_access::build_remote_router`。
- 非 Codex run 返回 `supported: false`、空 events。
- Codex run 复用 transcript path resolution。
- `limit` clamp 到 `1..=300`，默认 120。
- 使用 Phase 1 的 `read_tail_events` 和 `dedupe_adjacent`。
- 不使用当前 inspect handler 的全文件扫描路径。

共享 helper：

- 将 `inspect.rs` 中的 transcript path resolution 提取为 `handlers/run_transcript.rs` 或 `inspect.rs` 内 `pub(super)` helper。
- 避免复制递归查找逻辑。

### 5.2 InspectDrawer

行为：

- 前端基于 `run.tool` **直接分支**，不靠打一次 events 看 `supported`（避免非 Codex run 多打一次请求）：
  - `run.tool === 'codex'` 走 `/api/runs/{id}/events`。
  - 其他 tool 走旧 `/api/runs/{id}/inspect`。
- remote：不请求 events endpoint（`runtimeMode === 'remoteViewer'` 时整段 timeline 都不加载）。
- drawer 打开期间每 2s poll 一次 cursor 增量；`Active` 状态下才 poll，其它状态只打一次（减少后端 IO）。
- drawer 关闭、切换 run、组件 unmount 时清理 interval。
- document hidden 时暂停 poll；恢复 visible 后拉一次增量。

UI：

- 事件卡片展示 kind、timestamp、tool name、exit code、preview。
- long output 默认折叠，点击展开。
- exit code 非 0 使用错误色边框。
- token_count 显示汇总数字。
- task marker 用轻量分隔/状态样式。
- 列表最多保留最近 300 条，防止 DOM 增长。

滚动：

- 实现简单 near-bottom lock。
- 用户在底部时新事件自动跟随。
- 用户向上滚动时不抢位置。

### 5.3 测试与验收

后端测试：

- Codex run returns supported events。
- non-Codex run returns supported false。
- missing run -> 404。
- limit clamp。
- remote router unknown events path -> 404。
- transcript missing -> supported true + empty events 或明确 error，二选一并测试。

前端测试：

- Codex local 模式走 events。
- non-Codex local 模式走 inspect。
- remote 模式不请求 events。
- interval cleanup。
- cursor 增量合并。

验收：

- `cargo test --workspace` 通过。
- `pnpm --filter @octomonitor/web test --run` 通过。
- 大 transcript 打开 drawer 不明显卡顿。
- 活跃 session 在 2-4s 内追加新事件。
- remote viewer 不显示 timeline。

## 6. Phase 4：只读元数据补强（可选）

Phase 0-3 后再决定是否执行。

先收集：

- Monitor 中 noisy/低价值标题比例。
- 是否还有 source/cwd 缺口。
- 是否有用户要求 parent/subagent 链路。

候选动作：

- 保持不扫描 `deleted_sessions`。
- 评估 `vscode_task_list.json` 是否能作为低优先级标题候选。
- 评估 parent/subagent 字段实际格式。

暂不做：

- `state_5.sqlite`，除非收益明确高于新增 SQLite 依赖成本。
- 写回任何 Codex metadata。

验收：

- 没有源工具写操作。
- 没有新增数据库。
- 标题质量有可观察提升。

## 7. 明确不做

- Supervisor / continue / stop。
- set-title / set-source / set-workdir。
- archive / delete / purge。
- VS Code extension patch。
- SSH + systemd bootstrap。
- WS delta protocol。
- annotation layer。
- parent/subagent 链路解析，除非后续有明确需求。

## 8. 安全边界

| # | 约束 | 适用 Phase |
| --- | --- | --- |
| 1 | 新前端请求必须走 `apiFetch` / `buildWsUrl` | 0, 3 |
| 2 | 新 API 只挂 main router，不挂 remote router | 0, 3 |
| 3 | 新 `RunRecord` 字段必须同步 `redact_run` | 2+ |
| 4 | `last_tail` 只放短状态文案，≤80 字符 | 2 |
| 5 | events tail 读取必须有 byte limit 和 max events | 1, 3 |
| 6 | event preview/text 必须有长度上限 | 1, 3 |
| 7 | token_count 只暴露汇总数字和 rate-limit | 1, 3 |
| 8 | remote 模式不请求 events/resume-command | 0, 3 |
| 9 | official/gateway API 如果提供等价事件，应优先于 JSONL fallback | 后续评估 |

## 9. 执行节奏

每个 Phase：

1. 跑 baseline：
   ```bash
   cargo test --workspace
   pnpm --filter @octomonitor/web test --run
   ```
2. 实施当前 Phase。
3. 对照本 Phase 验收点自查。
4. 对照第 8 节安全边界自查。
5. 跑对应测试。
6. 若涉及前端构建，跑：
   ```bash
   pnpm --filter @octomonitor/web build
   ```
7. 若涉及 Rust 共享 API 或准备合并，跑：
   ```bash
   cargo clippy --workspace -- -D warnings
   ```
8. 进入下一 Phase 前，重新确认后续计划是否仍成立。

## 10. 关键坐标

OctoMonitor：

- `crates/adapters/codex/src/lib.rs`：CodexSession、apply line、JsonlCursor 使用。
- `crates/adapters/common/src/lib.rs`：JsonlCursor、read_jsonl_delta。
- `crates/core/src/lib.rs`：RunRecord、RunState。
- `crates/server/src/main.rs`：main router。
- `crates/server/src/remote_access.rs`：remote router、redact_run。
- `crates/server/src/handlers/inspect.rs`：现有 inspect 和 transcript path resolution。
- `crates/server/src/handlers/stream.rs`：snapshot.replace 协议。
- `crates/server/src/probe.rs`：Codex session 到 RunRecord 映射。
- `apps/web/src/components/InspectDrawer.tsx`：drawer 改造。
- `apps/web/src/components/monitor/MonitorView.tsx`：主列表和筛选栏挂载点。
- `apps/web/src/App.tsx`：键盘快捷键 hook。
- `apps/web/src/lib/preferences.ts`：FilterRules、MonitorPeriod。
- `apps/web/src/lib/api.ts`：apiFetch、buildWsUrl。
- `apps/web/src/lib/runtimeMode.ts`：runtimeMode 判断。

参考文档：

- `docs/reference-monitoring-inspiration.md`：策略和边界。
- 本文件：实施计划。
