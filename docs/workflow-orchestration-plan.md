# OctoMonitor Workflow Orchestration 方案

> 在 OctoMonitor 现有 monitor 能力上，为同时使用 Claude Code 和 Codex 的用户提供可追踪、可串联、可逐步升级到半自动执行的 workflow orchestration。

---

## 一、产品定位

OctoMonitor 做的是 **workflow tracking + explicit handoff + optional assisted execution**，不是通用 agent runtime。

### V1 交付范围

- 创建 workflow definition（线性 step 列表）
- 展示 workflow run 的历史、当前 step、pending step、linked runs、artifacts
- 把已有 Claude Code / Codex run 显式关联到 step
- 在开启执行能力后，启动 `launch` step 的非交互式 CLI 执行
- 基于显式 metadata 或手动确认串联整条链

### V1 不做

- 完整 DAG / 并行编排
- 完整 A2A 协议实现
- 远程 agent federation
- 靠 run inactivity 自动判定 step 完成
- 任意 prompt 自动识别 workflow 语义
- 在 remote viewer 模式下暴露可执行控制面

### 功能分层

| 层级 | 名称 | V1 |
|---|---|---|
| L1 | Workflow tracking：定义、run、step、历史、pending、关联 run | 是 |
| L2 | Assisted execution：显式审批后启动 `launch` step | 是 |
| L3 | Auto advance：有充分 metadata 且用户开启后自动推进 | 是，晚于 tracking |
| L4 | Full A2A / remote agent orchestration | 否 |

---

## 二、设计原则

1. **先简单工作流，再多 agent**：优先线性、可组合、可解释的 workflow
2. **区分 handoff 和 agents-as-tools**：workflow 必须显式区分"引用外部结果"和"把控制权 handoff 给下一工具"
3. **Artifact-first**：编码类任务天然依赖共享文件上下文，文件传递比 agent 对话更稳
4. **Orchestrator-led**：workflow 负责顺序，step 负责产出，artifact 负责交接，timeline 负责审计
5. **显式 metadata 为主**：自动串联依赖结构化 metadata，heuristic 只做候选

---

## 三、核心模型

Workflow 不替代 `RunRecord`，而是 run 的上层结构：

```text
WorkflowDef
  -> WorkflowRun
      -> StepRun
          -> LinkedRunRef -> RunRecord
          -> ArtifactRef
```

这样保持与现有 monitor 数据结构兼容，允许一个 step 绑定多个 run，且 workflow 历史独立于 run 扫描历史。

---

## 四、领域模型

### 4.1 核心类型

```rust
// crates/core/src/workflow.rs

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorkflowStepKind {
    Observe,
    Launch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorkflowExecutionMode {
    TrackingOnly,   // 只跟踪，不发起执行
    Assisted,       // 每次 launch 前都审批
    Auto,           // 满足条件时自动推进
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WorkflowRunState {
    Pending,
    Running,
    WaitingInput,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StepRunState {
    Pending,
    Ready,
    WaitingLink,
    WaitingApproval,
    Running,
    Completed,
    Failed,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CompletionSource {
    LauncherExit,
    HookEvent,
    LinkedRun,
    UserMarked,
    HeuristicSuggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CompletionMode {
    ManualLink,       // 用户手动 link run 并确认完成
    LauncherExit,     // CLI 子进程正常退出即完成
    HookEvent,        // 收到匹配的 hook/ingest 事件即完成
    ManualComplete,   // 纯手动标记完成
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LinkConfidence {
    Explicit,
    ContextFile,
    PromptMarker,
    WorkspaceAndArtifact,
    HeuristicCandidate,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkflowDef {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_working_dir: Option<String>,
    pub steps: Vec<StepDef>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StepDef {
    pub id: String,
    pub order: u32,
    pub label: String,
    pub tool: ToolKind,
    pub kind: WorkflowStepKind,
    pub prompt_template: Option<String>,
    pub inputs: Vec<ArtifactSpec>,
    pub outputs: Vec<ArtifactExpectation>,
    pub approval_required: bool,
    pub auto_advance_eligible: bool,
    pub completion: CompletionPolicy,
    pub launch: Option<LaunchSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtifactSpec {
    pub mode: String, // "file" | "linked-run-summary" | "linked-run-question"
    pub value: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtifactExpectation {
    pub kind: String,     // "file" | "diff" | "summary"
    pub value: String,    // path or logical name
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CompletionPolicy {
    pub mode: CompletionMode,
    pub required_artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LaunchSpec {
    pub model: Option<String>,
    pub timeout_secs: Option<u64>,
    pub allowed_tools: Vec<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkflowRun {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub execution_mode: WorkflowExecutionMode,
    pub working_dir: String,
    pub state: WorkflowRunState,
    pub definition_snapshot: WorkflowDef,
    pub steps: Vec<StepRun>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StepRun {
    pub step_id: String,
    pub order: u32,
    pub label: String,
    pub tool: ToolKind,
    pub kind: WorkflowStepKind,
    pub state: StepRunState,
    pub prompt_rendered: Option<String>,
    pub linked_runs: Vec<LinkedRunRef>,
    pub artifacts: Vec<ArtifactRef>,
    pub completion_source: Option<CompletionSource>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkedRunRef {
    pub run_id: String,
    pub confidence: LinkConfidence,
    pub matched_by: String,
    pub linked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtifactRef {
    pub id: String,
    pub kind: String,      // "file" | "diff" | "stdout-summary"
    pub path: Option<String>,
    pub preview: Option<String>,
    pub digest: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkflowRunSummary {
    pub id: String,
    pub workflow_name: String,
    pub state: WorkflowRunState,
    pub execution_mode: WorkflowExecutionMode,
    pub progress_label: String,     // "2/6"
    pub current_step: Option<String>,
    pub waiting_count: u32,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LaunchPreview {
    pub rendered_prompt: String,
    pub input_artifacts: Vec<ArtifactSpec>,
    pub model: Option<String>,
    pub allowed_tools: Vec<String>,
    pub estimated_prompt_chars: usize,
}
```

### 4.2 Observe vs Launch

**Observe step**：用户或外部 agent 在 Claude/Codex 自己执行，OctoMonitor 负责等待、关联、归档、推进。适合交互式操作（写方案、在已有会话中继续）。

**Launch step**：由 OctoMonitor 通过 CLI 非交互模式发起，属于 control plane 能力，受 execution mode 和审批控制。适合结构固定的 review、自动摘要、受控检查。

### 4.3 Run 关联为独立对象

`LinkedRunRef` 必须是独立对象（而非 `linked_run_id: Option<String>`），因为需要记录：哪个 run 属于哪个 step、可信度、谁建立的链接、完成依据。

---

## 五、状态机与推进规则

### 5.1 WorkflowRun 状态机

```text
Pending -> Running -> Completed
                   -> WaitingInput
                   -> WaitingApproval
                   -> Failed
                   -> Cancelled
```

| 状态 | 含义 |
|---|---|
| Pending | run 已创建，尚未进入首个 step |
| Running | 当前存在正在执行或已可执行的 step |
| WaitingInput | 当前 step 是 observe，等待用户交付结果 |
| WaitingApproval | 当前 step 是 launch 或下一个 handoff 需要审批 |
| Completed | 所有 step 已完成 |
| Failed | 某一步失败且未恢复 |
| Cancelled | 用户终止 |

### 5.2 StepRun 状态机

**Observe step**：`Pending -> WaitingLink -> Completed / Failed / Skipped / Cancelled`

**Launch step**：`Pending -> Ready -> WaitingApproval? -> Running -> Completed / Failed / Cancelled`

### 5.3 硬约束

1. **现有 monitor 的 `RunState::Completed` 不能直接完成 workflow step**
2. **`HeuristicSuggestion` 只能生成候选链接，不能自动完成 step**
3. **只有以下四类信号可以完成 step**：
   - Launcher 成功退出且满足 CompletionPolicy
   - Hook/ingest 上报了匹配的 workflowId + stepId
   - 用户显式把 run 链接到该 step 并点击完成
   - 用户手动标记完成

---

## 六、自动串联策略

### 6.1 匹配优先级

1. `workflowId + stepId` 明确匹配 → 强关联
2. `handoffFromRunId` 明确匹配 → 强关联
3. `workspace + artifactRef` 匹配 → 候选
4. 时间邻近 + 工具切换模式匹配 → 候选

只有前两类可以直接强关联；后两类只产生候选。

### 6.2 workflow-context.json

工作区维护上下文文件 `<repo>/.octomonitor/workflow-context.json`：

```json
{
  "workflowId": "wf_design_review_001",
  "stepId": "step_review_codex",
  "parentStepId": "step_write_claude",
  "artifactRefs": ["docs/plan.md"],
  "updatedAt": "2026-04-08T14:30:00Z"
}
```

用途：切换步骤前写入 → hook 上报时附带 → OctoMonitor 自动关联到正确 step。

### 6.3 Prompt marker 兜底

```text
[octomonitor wf=wf_design_review_001 step=step_review_codex parent=step_write_claude artifact=docs/plan.md]
```

可信度低于 context file，高于纯 heuristic。

### 6.4 Ingest 扩展字段

```rust
pub struct WorkflowIngestHint {
    pub workflow_id: Option<String>,
    pub step_id: Option<String>,
    pub parent_step_id: Option<String>,
    pub artifact_refs: Option<Vec<String>>,
}
```

不影响现有 run ingest 逻辑，只给 workflow coordinator 提供匹配依据。

### 6.5 Heuristic 边界

- **可以做**：产生候选关联、UI 显示推荐、一键确认
- **不能做**：自动完成 step、自动推进 workflow、改写历史链路

---

## 七、Artifact 传递策略

**主共享介质**：文件系统。Claude Code 和 Codex 的工作对象本来就是 repo 和文件。

**OctoMonitor 补的是"记录"**：哪些文件是 step 输入、哪些是输出、哪个 artifact 驱动了 handoff、当前 artifact 是否满足 completion policy。

| artifact 类型 | 来源 | 用途 |
|---|---|---|
| `file` | 工作区文件 | 主上下文传递 |
| `diff` | git diff / changed files | 实施与 review 的核心依据 |
| `stdout-summary` | CLI 退出后摘要 | 轻量运行回执 |
| `linked-run-summary` | 关联 run 的问题/摘要 | 给下一步做 prompt 注入 |

---

## 八、存储设计

遵循 `No database` 规则，全部本地 JSON 持久化。

```text
~/.octomonitor/
  workflows/
    defs/
      wf-*.json
    runs/
      wr-*.json
    index-defs.json
    index-runs.json
```

- Definition 和 run 分开：definition 是模板（改动频率低），run 是实例（写入频率高）
- Run 保存 definition snapshot，避免模板修改后历史 run 被污染
- `index-runs.json` 只保留摘要（id, workflowName, state, progress, currentStep, createdAt, updatedAt）
- 写入策略：临时文件写入 → `rename` 原子替换

---

## 九、服务端架构

### 9.1 新增服务组件

| 组件 | 职责 |
|---|---|
| `WorkflowStore` | definition/run JSON 读写、索引维护、snapshot 持久化 |
| `WorkflowCoordinator` | 启动 run、推进 step 状态机、审批/取消/跳过/重试、接收 launcher 结果和 ingest hint、生成 summary |
| `WorkflowLinkResolver` | 用 explicit metadata 做强匹配、用 heuristic 做候选匹配、维护 LinkedRunRef |
| `ToolLauncher` | 启动受控 CLI 子进程、解析输出、收集退出状态和 artifacts |

Adapter/probe 是观测面，launcher 是控制面，职责分开。

### 9.2 与 bootstrap / stream 集成

V1 不另起 WS 协议，继续复用现有机制：

- 服务端更新 workflow 状态
- Workflow summary 合并进 bootstrap
- 通过 `/api/stream` 广播 `snapshot.replace`
- 前端收到新 snapshot 后刷新 workflows 视图

### 9.3 BootstrapPayload 扩展

只加 summary，不加 full detail：

```rust
pub struct BootstrapPayload {
    // ... 现有字段 ...
    pub workflow_runs: Vec<WorkflowRunSummary>,
}
```

Detail 通过单独 REST 获取，因为 detail 可能包含渲染后的 prompt、artifact 预览、绝对路径等敏感内容，且会让 bootstrap 膨胀。

---

## 十、API 设计

### 10.1 Definition API

```text
GET    /api/workflows
POST   /api/workflows
GET    /api/workflows/{id}
PUT    /api/workflows/{id}
DELETE /api/workflows/{id}
```

### 10.2 Run API

```text
GET    /api/workflow-runs
POST   /api/workflows/{id}/runs
GET    /api/workflow-runs/{id}
POST   /api/workflow-runs/{id}/cancel
POST   /api/workflow-runs/{id}/mode
```

`/mode` 用来运行中切换 `trackingOnly` / `assisted` / `auto`。

### 10.3 Step Action API

```text
POST /api/workflow-runs/{id}/steps/{stepId}/link
POST /api/workflow-runs/{id}/steps/{stepId}/unlink
POST /api/workflow-runs/{id}/steps/{stepId}/approve
POST /api/workflow-runs/{id}/steps/{stepId}/complete
POST /api/workflow-runs/{id}/steps/{stepId}/fail
POST /api/workflow-runs/{id}/steps/{stepId}/skip
POST /api/workflow-runs/{id}/steps/{stepId}/retry
GET  /api/workflow-runs/{id}/steps/{stepId}/candidates
GET  /api/workflow-runs/{id}/steps/{stepId}/preview
```

- `link`: 把 monitor 中已有 run 挂到 step
- `complete`: 人工确认 step 已完成
- `candidates`: 返回 heuristic 候选 run
- `preview`: 返回 LaunchPreview（渲染后 prompt、input artifacts、model/tools）

### 10.4 Redaction 规则

Remote viewer 模式下：不返回绝对路径、prompt 全文、artifact 全内容，只返回安全摘要。

---

## 十一、执行层设计

### 11.1 ToolLauncher 接口

```rust
#[async_trait]
pub trait ToolLauncher: Send + Sync {
    async fn launch(&self, request: LaunchRequest) -> Result<LauncherHandle, LaunchError>;
    async fn poll_event(&self, handle: &LauncherHandle) -> Result<Option<LauncherEvent>, LaunchError>;
    async fn await_result(&self, handle: &LauncherHandle) -> Result<LauncherResult, LaunchError>;
    async fn cancel(&self, handle: &LauncherHandle) -> Result<(), LaunchError>;
}
```

### 11.2 ToolLauncher 职责边界

只负责：启动子进程、流式读取输出、收集退出状态、生成 stdout-summary / diff / changed files。
不负责：决定何时启动、决定 workflow 是否完成、解析 heuristic 关联。

### 11.3 CLI 能力检测

实现时把 CLI flag 视为运行时能力检测（检测本机安装版本 → 决定可用参数），而非写死的协议承诺，避免 CLI 升级后脆断。

---

## 十二、Prompt 与上下文注入

### 12.1 模板只对 Launch step 生效

Observe step 的真实 prompt 发生在 Claude/Codex 原生界面，OctoMonitor 无法保证掌握完整上下文。

### 12.2 支持的模板变量

```text
{{workflow.name}}
{{step.label}}
{{previous.summary}}
{{previous.artifacts}}
{{file:path/to/file.md}}
{{linked_run.last_question}}
{{linked_run.summary}}
```

### 12.3 上下文大小控制

- 单文件内容截断：max 8000 chars
- 总 prompt：max 32000 chars
- 过大文件优先注入摘要
- 不存在的文件 → `[file not found: {path}]`

---

## 十三、UI 方案

### 13.1 信息架构

新增第 6 个 tab：`WORKFLOWS`（快捷键 `6`）。

在 `MONITOR` tab 顶部增加辅助条带：`Pending Workflow Handoffs` / `Waiting Approval`。

### 13.2 Workflows 主视图（三栏）

```text
左栏：Workflow Runs
中栏：Pipeline / Timeline
右栏：Step Detail
```

**左栏：Runs 列表**
- 分组：Active / Waiting approval / Recent completed / Recent failed
- 每项：名称、当前模式 badge、进度 `2/6`、当前 step、更新时间

**中栏：Pipeline**
- 水平 pipeline，每个 step 一个节点 + edge 连线
- 节点展示：工具图标（Claude/Codex）、step 类型（Observe 虚线框 + 眼睛 / Launch 实线框 + 火箭）、状态色、耗时、link 数、artifact 数
- Edge 颜色：灰=未建立、蓝=运行中、绿=已完成、琥珀=等待确认、红=失败

**右栏：Step Detail**
- Completion policy 展示（模式 + 条件满足状态）
- Linked runs 列表（run_id、confidence badge、matched_by）
- 点击 linked run → 复用现有 InspectDrawer
- Candidate runs + 一键 Link
- Artifact 列表
- Prompt preview（仅 launch step）
- 操作按钮：Approve / Link / Complete / Retry / Skip

### 13.3 WorkflowEditor（轻量版）

V1 只做：线性 step 列表、顺序调整、tool 选择、step 类型选择、输入/输出 artifact 配置、审批开关。

不做：拖拽式 DAG、条件分支、图形化路由。

---

## 十四、安全与权限

### 14.1 三个权限等级

| 等级 | 名称 | 能力 |
|---|---|---|
| Level 0 | Tracking only（默认） | 创建/查看 workflow、手动 link/complete/fail/skip、不启动 CLI |
| Level 1 | Assisted execution | 允许启动 launch step、每步审批、显示完整 prompt preview |
| Level 2 | Auto advance | 满足条件时自动推进、`approval_required=true` 仍停下、失败绝不自动重试 |

### 14.2 额外约束

- Remote viewer 不能触发执行 API
- 不支持危险 flag 透传
- 同一 workspace 默认只允许一个 active launch workflow run
- Prompt template 渲染做长度上限
- 所有 step action 记入 run timeline

---

## 十五、实施路线

| Phase | 目标 |
|---|---|
| 1: Tracking 基础层 | definition/run/store/API/summary/UI，手动 link/complete/fail/skip |
| 2: 显式串联层 | workflow-context.json、ingest hint、LinkResolver 强匹配 + 候选 |
| 3: Assisted execution | ToolLauncher、prompt 渲染、审批执行、stdout summary、diff 收集 |
| 4: Auto advance | 自动推进、mode 切换、提醒通知、failure recovery |

详见 [workflow-implementation-plan.md](workflow-implementation-plan.md)。

---

## 十六、风险与缓解

| 风险 | 缓解 |
|---|---|
| 误把 monitor run 状态当 workflow 完成信号 | 只接受显式 completion source |
| CLI 参数或输出格式变动 | capability detection，不把 CLI flag 写成稳定协议 |
| Prompt 注入过大 | 文件截断、摘要优先、artifact 白名单 |
| 同仓库多 workflow 同时写 | 默认限制一个 active launch run / workspace |
| Remote viewer 泄露 prompt 或路径 | bootstrap 仅 summary，detail redaction |
| Workflow definition 改动导致历史 run 漂移 | run 内保存 definition snapshot |

---

## 十七、用户场景映射

用户原始 6 步示例在最终模型中的表达：

```text
用户原始流程                          最终模型表达
────────────────────────────────────────────────────────
1. Claude Code 写方案         →  Step 1: Observe, Claude, completion=manual-link
2. Codex Review 方案          →  Step 2: Observe, Codex,  completion=manual-link
3. Claude Code 根据建议修改   →  Step 3: Observe, Claude, completion=manual-link
4. Codex 再次 Review          →  Step 4: Launch,  Codex,  completion=launcher-exit
5. Claude Code 实施           →  Step 5: Observe, Claude, completion=manual-link
6. Codex 实施 Review          →  Step 6: Launch,  Codex,  completion=launcher-exit
```

- Steps 1-3 是 Observe：用户在 Claude Code / Codex 自己的界面交互式执行，OctoMonitor 等待用户链接对应 run
- Steps 4、6 是 Launch：后期 review prompt 结构稳定，可由 OctoMonitor 通过 `codex exec --json` 非交互发起
- Tracking 模式：6 个 step 全部手动 link + 手动 complete
- Assisted 模式：Launch step 弹出审批对话框，确认后自动启动 CLI
- Auto 模式：前一步 complete 后自动推进 Launch step

Steps 1-4 本质上是"写→审→改→审"的 review loop。V1 不将其作为一等公民建模（verdict 提取不够稳定），但 V2 可考虑在 StepDef 上新增 `review_loop_config` 支持自动重入。

---

## 十八、配套 Demo

- [workflow-orchestration-demo.cc.html](workflow-orchestration-demo.cc.html)
