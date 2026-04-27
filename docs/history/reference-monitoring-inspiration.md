# 参考监督台借鉴建议

> 2026-04-27 更新：本文档列出的 Phase 0-3 借鉴项已通过 `docs/history/implementation-plan-monitoring-inspiration.md` 落地。Phase 4（元数据补强）与 Phase 5（手机值守 / annotation 评估）尚未启动；本文件归档保留，作为后续相关讨论的策略与边界参考。

日期：2026-04-24

参考仓库：用户提供的本地参考监督台项目。

目标项目：OctoMonitor，Rust workspace + React/Vite + Tauri 2，本地优先、多工具统一监控面板。

## 核心判断

参考仓库是面向 Codex CLI 的监督和接管控制台；OctoMonitor 是多工具、read-only by default 的统一观测面板。两者定位不同，不能照搬控制面能力。

OctoMonitor 最值得借鉴的是读侧能力和交互细节：

- Codex JSONL 的事件级解析。
- 基于明确事件 marker 的进度和关注状态判断。
- 有界尾部读取和 byte cursor，避免大 transcript 全量加载。
- InspectDrawer 中的工具调用 timeline。
- 手机值守场景里的只读快捷筛选。
- 复制、Toast、标题降噪、可复制但不执行的建议命令。

不建议近期引入：

- 续跑、停止、持续推进、批量删除、批量归档。
- 改写 Codex session/title/source/workdir。
- SSH + systemd 远程 bootstrap 控制面。
- patch VS Code extension。
- 为标题补强过早引入 SQLite 依赖。

## 当前基线

OctoMonitor 已有：

- Codex adapter 已能扫描本地 sessions 和 archived sessions，增量读取 JSONL，统计 token、问题摘要、quota、pending approval。
- `crates/server/src/probe.rs` 把 Codex session 映射为 `RunRecord`，但状态主要按最后活动时间和 pending approval 推断。
- `crates/server/src/handlers/inspect.rs` 提供 `/api/runs/{run_id}/inspect`，当前只展示 input/output，并且从 transcript 开头顺序读到结尾。
- `apps/web/src/components/InspectDrawer.tsx` 已有本地 drawer；remote viewer 模式不加载 transcript。
- `crates/server/src/remote_access.rs` 已 redaction：remote viewer 隐藏 transcript path、session id、thread id、last action、last tail、first/last question 等字段。
- `crates/server/src/handlers/stream.rs` 和 remote stream 都发送 `snapshot.replace`，客户端只消费全量 snapshot。
- `apps/web/src/App.tsx` 已有键盘导航：`1-5` 切 tab、`j/k` 上下、`Enter` 打开 drawer、`?` 呼出帮助；`Esc` 已在 drawer/overlay 中处理。
- `apps/web/src/lib/preferences.ts` 已有 per-tool `FilterRules` 和 `MonitorPeriod`，不应再做一套按工具/按时间的快捷筛选。
- `crates/server/src/config.rs` 已有 `~/.octomonitor/config.json` 本地 JSON 配置先例。

仍缺失：

- Codex JSONL 事件级解析：reasoning、tool call、tool output、task marker、turn aborted、web search、token count、context。
- 有界尾部读取和 byte cursor events endpoint。
- 相邻事件去重。
- InspectDrawer 工具调用 timeline。
- marker-driven progress summary。
- `CopyButton` / `Toast` / clipboard 使用。
- 本地 resume-command API。
- 标题降噪。
- Monitor 顶部任务态快捷筛选和搜索。
- Codex parent/subagent 链路解析。
- 本地 annotation layer。

## 实施路线

### Phase 0：只读体验小修补

目标：低风险补齐直接可用的交互能力。

改动：

- 新增轻量 `CopyButton` 和 `Toast`。
- 新增本地 API：`GET /api/runs/{run_id}/resume-command`。
- 新增标题降噪函数，先接 Codex adapter。

`resume-command` 规则：

- 返回 `{ command: string | null, tool: ToolKind, note?: string }`。
- 只挂 local admin router，不挂 remote router。
- 后端只构造字符串，不 spawn、不校验权限、不写日志。
- 无法安全构造命令时返回 `command: null` 和明确说明。
- UI 标注为“复制建议命令”，不承诺一定可执行。

Copy 覆盖点：

- run id / thread id。
- workspace path。
- transcript path，仅 local 模式显示。
- resume command。
- 后续 timeline 中的 command 或 tool output preview。

标题降噪：

- 新增 `looks_noisy_title`。
- 过滤 JSON blob、markdown fence、系统提示、过长结构化单行。
- Codex adapter 中如果 `last_question` noisy，则降级到 `first_question`、`thread_name` 或 session id 短前缀。
- 不照搬多源 title 合并逻辑。

验收：

- InspectDrawer 可复制关键字段。
- remote viewer 不显示或不启用被 redaction 的复制按钮。
- noisy 标题不直接进入列表主标题。
- 未知工具的 resume command 明确 unavailable。

### Phase 1：Codex 事件 parser

目标：把 JSONL 事件理解能力做成 Rust 纯函数，先不改 UI。

新增：

- adapter 内新增 `events.rs`
- `parse_session_event_line`
- `parse_exec_output`
- `dedupe_adjacent`
- `read_tail_events`

事件类型：

```rust
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

事件字段：

```rust
pub struct CodexEvent {
    pub kind: CodexEventKind,
    pub timestamp: String,
    pub turn_id: Option<String>,
    pub title: String,
    pub preview: String,
    pub text: String,
    pub tool_name: Option<String>,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub call_id: Option<String>,
}
```

实现要求：

- tool output 提取 command、chunk id、exit code、body，并压缩 preview。
- `token_count` 只暴露汇总数字和 rate-limit 信息，不暴露任何 token payload。
- 尾部读取按 byte limit 和 max events 限制。
- cursor 超过文件大小时返回 reset。
- 最后一行未写完时不解析半行。
- 去重逻辑先留在 adapter 的 events 模块，不要提前上移到 core。

测试覆盖：

- user message。
- assistant message。
- reasoning preview。
- function call。
- function call output，包含 command、exit code、body。
- task_started / task_complete / task_aborted / turn_aborted。
- invalid JSON 忽略。
- cursor reset。
- 半行忽略。
- 相邻重复事件去重。

验收：

- 对应 adapter 包的单元测试通过。
- 未改 UI 和 handler。

### Phase 2：Progress Summary + 快捷筛选

目标：让 Monitor 主列表更准确回答“它是在跑、已完成，还是等我”。

Codex adapter 维护：

- 最近 marker 栈。
- recent tools。
- last assistant preview。
- open turn 状态。

状态映射：

- unmatched escalated function call -> `WaitingApproval`。
- open turn + 最近工具调用或工具输出 -> `Active`。
- 明确 `task_complete` 或 final answer -> `Completed`。
- 近期停住但无明确完成 -> `Idle`。
- `turn_aborted` 先映射为 `Error` + 明确文案；使用 `Cancelled` 前先确认 i18n 和样式覆盖。

字段策略：

- 先复用 `last_action` 和 `last_tail`。
- `last_tail` 只放短 progress reason，例如 `Running tool: shell_command`、`Task complete`、`Turn aborted`。
- 不把原始 tool output、命令参数全文、assistant 长文本或 secret 放进 `last_tail`。
- 只有 UI 证明需要更细粒度时，再给 `RunRecord` 加 optional `ProgressSummary`，并同步更新 remote redaction。

快捷筛选：

- Monitor 顶部新增 `<MonitorFilterBar />`。
- 只做“需关注 / 运行中 / 搜索”。
- 不做按工具 chip，已有 `FilterRules` 和 panel config。
- 不做 24h/7d chip，已有 `MonitorPeriod`。
- 搜索框落地后，在现有 `useKeyboardShortcuts` 中补 `/` 聚焦。

验收：

- 刚结束的 Codex session 不再因时间窗口误判为 Active。
- 工具调用中的 session 能显示具体工具名。
- 快捷筛选不与 Settings 的过滤和时间窗重复。
- remote viewer 可复用只读筛选，但不显示 transcript、命令、tool output。

### Phase 3：Events Endpoint + InspectDrawer Timeline

目标：让本地 drawer 展示最近事件窗口。

新增本地 API：

```text
GET /api/runs/{run_id}/events?cursor=<byte_offset>&limit=120
```

返回：

```json
{
  "events": [],
  "cursor": 12345,
  "reset": false
}
```

后端要求：

- route 只挂 local admin router。
- remote router 继续只暴露 `/api/bootstrap`、`/api/stream`、`/api/pair/claim`。
- 复用现有 transcript path resolution。
- 不复用当前 inspect 的全文件 `BufReader` 扫描。
- 使用 Phase 1 的 parser、tail read 和 dedupe。
- 保留现有 input/output entries 作为 fallback。

前端要求：

- `InspectDrawer` local 模式请求 events，必须走 `apiFetch`。
- remote 模式不请求 events。
- 每条事件卡片显示类型、时间、工具名、退出码和压缩预览。
- 长 output 默认折叠。
- 退出码非 0 使用明显但克制的错误状态。
- 事件列表硬上限，例如最近 120 条。
- 粘底滚动：用户在底部时新事件跟随；用户向上滚动时不抢滚动位置。

验收：

- 本地 drawer 能看到 reasoning、tool call、tool output、task marker timeline。
- remote viewer 不显示这些事件。
- 大 transcript 不会因打开 drawer 全量扫描卡住。
- 滚动体验稳定。

### Phase 4：只读元数据补强

目标：改善标题、source、cwd 和未来父子关系，但不改变源数据。

建议：

- 先完成标题降噪和候选优先级。
- `vscode_task_list.json` 只作为低优先级标题候选。
- `deleted_sessions` 不进默认 Monitor。
- `state_5.sqlite` 暂不接入；只有 Phase 3 后仍有明确标题/source 缺口，再评估是否引入 SQLite 依赖。
- Codex parent/subagent 链路放入 backlog，等 adapter 稳定解析 `parent_session_id` 或 subagent source 关系后再做。

验收：

- 标题不出现 JSON、markdown、system prompt 噪声。
- 没有任何源工具写操作。
- 没有新增数据库。

### Phase 5：手机值守和 Annotation 评估

目标：在现有 remote viewer 边界内提升手机查看效率。

建议：

- remote viewer 继续只显示 Monitor / Usage。
- 复用 Phase 2 的只读快捷筛选。
- Pinned / Watched / alias / hide 只有在真实用户诉求出现后再做。

如果做 annotation：

- 存 `~/.octomonitor/annotations.json`。
- 不回写 Codex / Claude / OpenClaw / Hermes 源数据。
- 先定义 remote 语义：pinned/hidden 是否对 remote 生效，annotation 字段是否进入 redaction 白名单。

## 明确暂缓或不做

### 暂缓：WS Delta

当前 `snapshot.replace` 简单可靠。事件 timeline 是 drawer 局部数据，先用 HTTP cursor endpoint。

只有在以下情况出现后再考虑 WS delta：

- snapshot payload 实际过大影响弱网 remote viewer。
- timeline 需要高频实时推送。
- HTTP cursor 轮询无法满足交互。

即使引入，也应保留 `snapshot.replace` 作为初始帧和兜底。

### 不做：Supervisor / Continue / Stop

这些能力会从观测面板变成控制面，近期不做。

如果未来产品边界变化，必须满足：

- local-only。
- 显式 opt-in。
- 审计日志。
- per-session lock。
- 只在明确 `task_complete` 后继续，不能靠 idle timeout 猜。
- 批量动作需要强确认。

### 不做：源工具写回

不做：

- set-title 写回 Codex。
- set-source / set-workdir。
- archive / delete / purge。
- patch VS Code extension。

## 安全边界

实施时必须逐项检查：

1. 新前端请求必须走 `apiFetch` / `buildWsUrl`，严禁硬编码 loopback URL。
2. `/api/runs/{id}/events` 和 `/api/runs/{id}/resume-command` 只挂 local router。
3. 新增 `RunRecord` 字段时必须同步更新 `redact_run`。
4. `last_tail` 只允许短状态说明，禁止原始 output、命令参数全文、assistant 长文本、secret。
5. `token_count` 只暴露汇总数字和 rate-limit 信息。
6. CopyButton 在 remote 模式下对 redacted 字段隐藏或禁用。
7. events endpoint 必须 tail/cursor 读取，禁止全文件扫描。
8. Gateway 或 official API 如果提供等价事件/状态，应优先使用；JSONL 解析是 fallback。

## 风险检查

| 项目 | 风险 | 处理 |
| --- | --- | --- |
| 原始 transcript 泄露 | tool output 可能含 secret | local-only；remote 不暴露；正文压缩 |
| 大文件读取卡顿 | JSONL 可能很大 | tail byte limit；cursor；不全量加载 |
| 状态误判 | 文本 heuristic 不稳定 | 显式 marker 优先；heuristic 只做辅助提示 |
| 过早抽象 | 多工具统一 event schema 复杂 | 先做 Codex adapter 内部 parser |
| SQLite 依赖 | 为标题补强增加维护成本 | Phase 4 后再评估 |
| WS 协议复杂化 | 影响 local 和 remote | 先 HTTP cursor endpoint |
| read-only 边界稀释 | 续跑/停止很诱人 | 控制类能力不进近期计划 |
| companion URL 回归 | 硬编码 loopback 会破坏 remote viewer | 强制使用 API helper |
| resume command 误导 | 工具命令语法可能变化 | 标注 advisory；未知工具返回 unavailable；不执行 |
| UI 重复过滤 | 与 `FilterRules` / `MonitorPeriod` 重叠 | 快捷栏只做任务态筛选和搜索 |

## 关键参考坐标

参考仓库：

- Web 主文件：`parse_exec_output`、`parse_session_event`、`dedupe_adjacent_session_events`、`read_session_events_since`、`read_tail_lines`。
- Web 主文件：`infer_progress_state`、`infer_attention_state`、`build_progress_summary`、`inspect_recent_turn_lifecycle`。
- Web 主文件：`/api/resume_cmd` 只返回命令字符串，不执行。
- CLI 主文件：`title_looks_noisy`、`choose_display_title`、`iter_session_files`、`load_thread_state_index`、`load_thread_name_index`、`load_vscode_task_title_index`。
- 测试文件：事件解析、状态推断、远端目标和持续推进相关测试。

OctoMonitor：

- `crates/adapters/common/src/lib.rs`: `JsonlCursor`、`read_jsonl_delta`。
- Codex adapter：session scan、JSONL delta、token/quota/pending approval、标题截取。
- `crates/core/src/lib.rs`: `RunState`、`RunRecord`。
- `crates/server/src/handlers/inspect.rs`: 当前 inspect entries。
- `crates/server/src/main.rs`: local inspect route。
- `crates/server/src/remote_access.rs`: `redact_run`。
- `crates/server/src/handlers/stream.rs`: 当前 WS snapshot 协议。
- `apps/web/src/components/InspectDrawer.tsx`: 本地 drawer 和 remote viewer 禁用 transcript 逻辑。
- `apps/web/src/components/monitor/MonitorView.tsx`: 主列表状态展示和分组。
- `apps/web/src/App.tsx`: 现有全局键盘导航。
- `apps/web/src/lib/preferences.ts`: `FilterRules` 和 `MonitorPeriod`。
- `apps/web/src/lib/api.ts`: `apiFetch` / `buildWsUrl`。

## 最终建议

优先级应保持保守：

1. 先做 Phase 0，低风险提升日常可用性。
2. 再做 Phase 1，把事件解析做成可测纯函数。
3. 再做 Phase 2，让主列表状态更可信。
4. 最后做 Phase 3，把事件 timeline 放进本地 drawer。

这条路线能吸收参考项目最有价值的观测能力，同时不改变 OctoMonitor 的 read-only、安全、remote redaction 和多工具边界。
