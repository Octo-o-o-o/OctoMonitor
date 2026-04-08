# Workflow Orchestration 实施计划

> 基于 workflow-orchestration-plan.md 最终方案的逐步实施清单。
> 每个任务标注：文件路径、依赖、验收标准。
> 新会话可直接按此文档顺序执行。

---

## Phase 1：Tracking 基础层

目标：跑通 workflow 定义 → 创建 run → 手动 link / complete → UI 可看到 pipeline 和历史。

### 1.1 领域模型

**创建** `crates/core/src/workflow.rs`

内容：方案第六节所有领域类型 + 第十一节 `WorkflowRunSummary`，包括：
- `WorkflowStepKind` (Observe / Launch)
- `WorkflowExecutionMode` (TrackingOnly / Assisted / Auto)
- `WorkflowRunState`, `StepRunState`
- `CompletionSource`, `CompletionMode`, `LinkConfidence`
- `WorkflowDef`, `StepDef`, `ArtifactSpec`, `ArtifactExpectation`, `CompletionPolicy`, `LaunchSpec`
- `WorkflowRun`, `StepRun`, `LinkedRunRef`, `ArtifactRef`
- `WorkflowRunSummary`（方案第 11.4 节定义，含 id/workflow_name/state/execution_mode/progress_label/current_step/waiting_count/updated_at）

注意：
- 所有 struct 加 `#[derive(Debug, Clone, Serialize, Deserialize, TS)]`，`#[serde(rename_all = "camelCase")]`，`#[ts(export)]`
- `ToolKind` 复用 `crates/core/src/lib.rs` 现有定义
- 时间字段统一用 `String`（ISO 8601），和现有 RunRecord 一致
- ID 生成用 `nanoid` crate（项目已有依赖则复用，否则加到 core 的 Cargo.toml）

**修改** `crates/core/src/lib.rs`
- 添加 `pub mod workflow;`
- 在 `BootstrapPayload` 中加字段 `pub workflow_runs: Vec<workflow::WorkflowRunSummary>`

依赖：无
验收：`cargo test --workspace` 通过，类型可 serialize/deserialize

---

### 1.2 存储层

**创建** `crates/server/src/workflows/mod.rs`
- 导出 `store`, `coordinator`

**创建** `crates/server/src/workflows/store.rs`

功能：
- `WorkflowStore::new(base_dir)` — 初始化 `~/.octomonitor/workflows/` 目录结构
- `list_defs() -> Vec<WorkflowDef>` — 读 `index-defs.json`
- `load_def(id) -> WorkflowDef` — 读 `defs/wf-{id}.json`
- `save_def(def)` — 写 def + 更新 index
- `delete_def(id)` — 删 def + 更新 index
- `list_runs(limit, state_filter) -> Vec<WorkflowRunSummary>` — 读 `index-runs.json`
- `load_run(id) -> WorkflowRun` — 读 `runs/wr-{id}.json`
- `save_run(run)` — 原子写 run + 更新 index
- `update_run_index(run)` — 从 WorkflowRun 提取 summary 更新到 index

写入策略：write temp file → rename（和方案第十节一致）

依赖：1.1
验收：单元测试——创建/读取/更新 def 和 run，index 正确

---

### 1.3 Coordinator（Phase 1 子集）

**创建** `crates/server/src/workflows/coordinator.rs`

Phase 1 只实现 tracking 相关操作：

- `create_run(workflow_id, working_dir, mode) -> WorkflowRun`
  - 加载 def，复制为 definition_snapshot
  - 初始化所有 StepRun（state=Pending）
  - 把第一个 step 推进到对应初始状态（Observe → WaitingLink, Launch → Ready/Pending）
  - 保存 run
- `link_run(run_id, step_id, monitor_run_id, confidence, matched_by)`
  - 向 step 的 linked_runs 追加 LinkedRunRef
  - 保存 run
- `unlink_run(run_id, step_id, monitor_run_id)`
- `complete_step(run_id, step_id, source: CompletionSource)`
  - 检查 CompletionPolicy 是否满足
  - 标记 step completed
  - 推进下一个 step 到初始状态
  - 如果所有 step 完成 → run 状态 Completed
  - 保存 run
- `fail_step(run_id, step_id, error)`
- `skip_step(run_id, step_id)`
- `cancel_run(run_id)`
- `get_summary_list() -> Vec<WorkflowRunSummary>` — 给 bootstrap 用

约束：`create_run` 需检查同一 working_dir 下是否已有 active launch run（方案第 16.2 节），如有则拒绝创建。

状态推进规则严格遵循方案第七节。

依赖：1.1, 1.2
验收：单元测试——创建 run → link → complete → 自动推进 → 最终完成

---

### 1.4 HTTP API

**创建** `crates/server/src/handlers/workflows.rs`

路由（方案第十二节）：

```
GET    /api/workflows                              → list_defs
POST   /api/workflows                              → create_def
GET    /api/workflows/{id}                         → get_def
PUT    /api/workflows/{id}                         → update_def
DELETE /api/workflows/{id}                         → delete_def

POST   /api/workflows/{id}/runs                    → create_run
GET    /api/workflow-runs                           → list_runs
GET    /api/workflow-runs/{id}                      → get_run (full detail)
POST   /api/workflow-runs/{id}/cancel               → cancel_run
POST   /api/workflow-runs/{id}/mode                 → change_mode

POST   /api/workflow-runs/{id}/steps/{stepId}/link      → link_run
POST   /api/workflow-runs/{id}/steps/{stepId}/unlink    → unlink_run
POST   /api/workflow-runs/{id}/steps/{stepId}/complete  → complete_step
POST   /api/workflow-runs/{id}/steps/{stepId}/fail      → fail_step
POST   /api/workflow-runs/{id}/steps/{stepId}/skip      → skip_step
```

注意：
- detail API 在 remote viewer 模式下做 redaction（不返回绝对路径、prompt 全文）
- link body: `{ "runId": "ingest-claude-xxx", "confidence": "explicit", "matchedBy": "user-manual" }`

**修改** `crates/server/src/main.rs`
- 在 router 中注册上述所有路由
- 在 `AppState` 中持有 `WorkflowStore` 和 `WorkflowCoordinator`（包在 `Arc<Mutex<>>`）
- 在 `build_bootstrap()` 中调用 coordinator 获取 summary list

依赖：1.1, 1.2, 1.3
验收：`cargo build` 通过；用 curl 调用各 API 能正常返回

---

### 1.5 Web UI — Workflows Tab

**创建** `apps/web/src/components/workflows/WorkflowsView.tsx`

三栏布局（方案第十五节）：
- 左栏：WorkflowRunList
- 中栏：PipelineView（pipeline 可视化 + artifacts 概览）
- 右栏：StepDetail

数据来源：
- bootstrap 的 `workflow_runs` summary 用于列表
- 选中 run 后 fetch `GET /api/workflow-runs/{id}` 获取完整数据

**创建** `apps/web/src/components/workflows/WorkflowRunList.tsx`
- 展示 Active / Waiting / Recent 分组
- 每项显示：名称、模式 badge、进度、当前 step、更新时间

**创建** `apps/web/src/components/workflows/PipelineView.tsx`
- 水平 pipeline：每个 step 一个节点 + edge 连线
- 节点展示：tool icon、step label、Observe/Launch badge、状态色、耗时、link 数、artifact 数
- Observe step 虚线框，Launch step 实线框
- Edge 颜色按方案：灰=未建立、蓝=运行中、绿=已完成、琥珀=等待确认、红=失败

**创建** `apps/web/src/components/workflows/StepDetail.tsx`
- Completion Policy 展示（模式 + 条件满足状态）
- Linked Runs 列表（run_id、confidence badge、matched_by）
- 点击 linked run → 复用 InspectDrawer
- Artifact 列表
- 操作按钮：Link Run / Complete / Skip（根据 step kind 和 state 条件显示）

**修改** `apps/web/src/App.tsx`
- 新增 `workflows` tab（第 6 个，快捷键 `6`）

**修改** `apps/web/src/store/monitorStore.ts`
- 在 store 中加入 `workflowRuns: WorkflowRunSummary[]` 字段，从 bootstrap 的 `workflow_runs` 读取

**修改** `apps/web/src/components/monitor/MonitorView.tsx`
- 顶部增加 workflow 横幅提示（当有 WaitingLink/WaitingApproval 的 run 时显示）

依赖：1.4
验收：
- `pnpm --filter @octomonitor/web build` 通过
- 浏览器中能看到 Workflows tab、pipeline 可视化、step detail
- 能手动 link run 并 complete step

---

### 1.6 WorkflowEditor（轻量版）

**创建** `apps/web/src/components/workflows/WorkflowEditor.tsx`

Phase 1 只做最简编辑器：
- 名称、描述输入
- Step 列表（表格或卡片）：
  - label 输入
  - tool 选择（Claude / Codex / OpenClaw 下拉）
  - kind 选择（Observe / Launch）
  - completion mode 选择
  - 输入/输出 artifact 配置（简单文件路径列表）
  - approval_required 开关
- 排序按钮（上移/下移）
- 保存 → POST /api/workflows
- 保存并运行 → POST /api/workflows + POST /api/workflows/{id}/runs

不做：拖拽 DAG、条件分支、图形路由。

依赖：1.5
验收：能创建 workflow definition 并启动 run

---

## Phase 2：显式串联层

目标：让 workflow 能基于 metadata 自动关联 run，减少手动操作。

### 2.1 workflow-context.json 支持

**修改** `crates/adapters/claude/src/lib.rs`
- 在 `probe()` 中，扫描各 project 的 `.octomonitor/workflow-context.json`
- 如果存在，把 `workflowId` / `stepId` 附加到对应 `ClaudeSession` 的新可选字段中

**修改** `crates/adapters/codex/src/lib.rs`
- 同上逻辑

**定义** workflow-context.json schema（在方案中已定义）：
```json
{
  "workflowId": "wf-xxx",
  "stepId": "step-xxx",
  "parentStepId": "step-yyy",
  "artifactRefs": ["docs/plan.md"],
  "updatedAt": "2026-04-08T14:30:00Z"
}
```

依赖：Phase 1
验收：adapter probe 能读到 context file 并附加到 session

---

### 2.2 Ingest 扩展

**修改** `crates/server/src/handlers/ingest.rs`
- `ClaudeHookIngest` 和 `CodexHookIngest` 增加可选字段：
  ```rust
  pub workflow_id: Option<String>,
  pub step_id: Option<String>,
  pub parent_step_id: Option<String>,
  pub artifact_refs: Option<Vec<String>>,
  ```
- 收到带 workflow hint 的 ingest 时，通知 WorkflowCoordinator

依赖：2.1
验收：hook ingest 带 workflow 字段时，coordinator 能收到通知

---

### 2.3 LinkResolver

**创建** `crates/server/src/workflows/link_resolver.rs`

功能：
- `resolve_strong(hint: WorkflowIngestHint, runs: &[RunRecord]) -> Option<LinkedRunRef>`
  - 匹配 workflowId + stepId → confidence=Explicit
  - 匹配 workspace + artifactRef → confidence=ContextFile
- `resolve_candidates(step: &StepRun, runs: &[RunRecord]) -> Vec<LinkedRunRef>`
  - 匹配 workspace + tool + 时间窗口 → confidence=HeuristicCandidate
- Heuristic candidates 绝不自动完成 step，只存为候选

**新增 API** `GET /api/workflow-runs/{id}/steps/{stepId}/candidates`
- 调用 LinkResolver，返回候选 run 列表
- UI 显示候选 + 一键 Link 按钮

**修改** `crates/server/src/workflows/coordinator.rs`
- 收到 ingest hint 时调用 LinkResolver
- 强匹配 → 自动 link（但不自动 complete，除非 completion mode = HookEvent）
- 候选 → 存入 step 的 candidates 字段（或 API 实时计算）

依赖：2.2
验收：
- 用户在 repo 里写了 workflow-context.json → 新 run 自动关联到正确 step
- candidates API 返回合理候选

---

### 2.4 Prompt Marker 支持

**修改** `crates/server/src/workflows/link_resolver.rs`
- 在 `resolve_strong` 中增加 prompt marker 解析：
  ```
  [octomonitor wf=xxx step=yyy parent=zzz artifact=path]
  ```
- 从 RunRecord 的 `first_question` 或 `last_question` 中提取
- confidence=PromptMarker

依赖：2.3
验收：含 prompt marker 的 run 能被解析和关联

---

## Phase 3：Assisted Execution

目标：Launch step 能由 OctoMonitor 通过 CLI 发起执行。

### 3.1 ToolLauncher Trait + 实现

**创建** `crates/server/src/workflows/launcher.rs`

Trait（方案第十三节）：
```rust
#[async_trait]
pub trait ToolLauncher: Send + Sync {
    async fn launch(&self, request: LaunchRequest) -> Result<LauncherHandle, LaunchError>;
    async fn poll_event(&self, handle: &LauncherHandle) -> Result<Option<LauncherEvent>, LaunchError>;
    async fn await_result(&self, handle: &LauncherHandle) -> Result<LauncherResult, LaunchError>;
    async fn cancel(&self, handle: &LauncherHandle) -> Result<(), LaunchError>;
}
```

`LaunchRequest` 包含：prompt（已渲染）、working_dir、tool（Claude/Codex）、LaunchSpec。

实现 `ClaudeLauncher`:
- 先检测 `claude --version` 确认可用
- spawn `claude -p "{prompt}" --output-format stream-json --allowedTools "{tools}"`
- stdout 逐行解析 JSONL → LauncherEvent
- 退出后收集 git diff、changed files → LauncherResult

实现 `CodexLauncher`:
- 检测 `codex --version`
- spawn `codex exec "{prompt}" --json --cd {working_dir}`
- 同上

实现 `LauncherDispatcher`：
- 根据 ToolKind 分发到对应实现
- 做 capability detection（检测 CLI 版本，决定可用参数）

依赖：Phase 1
验收：单元测试——能 spawn 并解析 `echo '{"type":"test"}' | ...` 的模拟输出

---

### 3.2 Prompt 渲染

**创建** `crates/server/src/workflows/prompt.rs`

功能：`render_prompt(template, step, run, working_dir) -> String`

支持变量（方案第十四节）：
- `{{workflow.name}}`
- `{{step.label}}`
- `{{previous.summary}}` — 上一步 linked run 的 stdout summary
- `{{previous.artifacts}}` — 上一步 artifact 文件列表
- `{{file:path/to/file.md}}` — 文件内容注入（截断保护：max 8000 chars）
- `{{linked_run.last_question}}` — 上一步关联 run 的 last_question
- `{{linked_run.summary}}` — 上一步关联 run 的摘要

安全：
- 变量值不做 shell 转义（prompt 通过 stdin 传递给 CLI，不经过 shell）
- 文件内容截断：单文件 max 8000 chars，总 prompt max 32000 chars
- 不存在的文件 → 替换为 `[file not found: {path}]`

依赖：3.1
验收：渲染测试——各变量正确替换，截断生效

---

### 3.3 Coordinator 集成 Launcher

**修改** `crates/server/src/workflows/coordinator.rs`

新增方法：
- `approve_step(run_id, step_id)` — 仅 Launch step + WaitingApproval 状态可调用
  - 渲染 prompt
  - 调用 launcher.launch()
  - 更新 step state → Running
  - spawn tokio task 监听 launcher result
  - 完成后：收集 artifacts、更新 step state、推进下一步
- `get_launch_preview(run_id, step_id) -> LaunchPreview`
  - `LaunchPreview` 类型定义（放在 `crates/core/src/workflow.rs`）：
    ```rust
    pub struct LaunchPreview {
        pub rendered_prompt: String,
        pub input_artifacts: Vec<ArtifactSpec>,
        pub model: Option<String>,
        pub allowed_tools: Vec<String>,
        pub estimated_prompt_chars: usize,
    }
    ```
  - 用于 UI 审批前展示

推进逻辑扩展：
- Launch step 前序完成后 → state=Ready
- mode=Assisted → Ready → WaitingApproval（等用户 approve）
- mode=TrackingOnly → Launch step 跳过自动启动，等手动操作

**新增 API**
```
POST /api/workflow-runs/{id}/steps/{stepId}/approve    → approve_step
GET  /api/workflow-runs/{id}/steps/{stepId}/preview     → get_launch_preview
```

**修改** `crates/server/src/handlers/workflows.rs`
- 注册上述路由
- approve 需检查 execution mode，TrackingOnly 模式拒绝

依赖：3.1, 3.2
验收：
- Assisted 模式：Launch step 等待 → 用户 approve → CLI 启动 → 完成 → 自动推进
- TrackingOnly 模式：approve API 返回 403

---

### 3.4 UI 执行支持

**修改** `apps/web/src/components/workflows/StepDetail.tsx`
- Launch step 在 WaitingApproval 状态显示：
  - Prompt 预览（从 preview API 获取）
  - Artifact 读取列表
  - Model / Tools 信息
  - Approve 按钮 + Cancel 按钮
- Running 状态显示进度指示器
- Completed 后显示 stdout summary、changed files、cost

**修改** `apps/web/src/components/workflows/PipelineView.tsx`
- Launch step Running 状态：呼吸动画
- Launch step WaitingApproval 状态：琥珀色脉动

依赖：3.3
验收：浏览器中能审批 → 执行 → 看到结果

---

## Phase 4：Auto Advance

目标：满足条件时自动推进，减少人工介入。

### 4.1 自动推进逻辑

**修改** `crates/server/src/workflows/coordinator.rs`

在 `complete_step` 的推进逻辑中：
- 如果 run.execution_mode == Auto：
  - 下一个 step 是 Launch 且 `approval_required=false` 且 `auto_advance_eligible=true` → 直接启动
  - 下一个 step 是 Launch 但 `auto_advance_eligible=false` 或 `approval_required=true` → WaitingApproval
  - 下一个 step 是 Observe → WaitingLink（Auto 不能跳过 observe，用户仍需操作）
- 如果 run.execution_mode == Assisted：
  - 所有 Launch step → WaitingApproval
- 失败绝不自动重试

**新增** `POST /api/workflow-runs/{id}/steps/{stepId}/retry`
- 只对 Failed step 有效
- 重置 state → Ready / WaitingLink
- 需要用户显式触发

依赖：3.3
验收：
- Auto 模式：Observe step complete → 下一个 Launch step 自动启动（如果 approval_required=false）
- 失败后不自动重试

---

### 4.2 模式切换 API + UI

**已有 API**: `POST /api/workflow-runs/{id}/mode`
- body: `{ "mode": "assisted" | "auto" | "trackingOnly" }`
- 运行中可切换（不需要重建 run）

**修改** `apps/web/src/components/workflows/WorkflowsView.tsx`
- Header 中的 mode selector 连接到 mode API
- 切换后刷新 run 数据

依赖：4.1
验收：切到 Auto 后，pending 的 Launch step 在条件满足时自动推进

---

### 4.3 桌面通知

**修改** `apps/desktop/src-tauri/src/main.rs`（或相关通知逻辑）
- workflow 横幅变化时触发桌面通知
- 通知场景：
  - WaitingApproval：有 Launch step 等待审批
  - WaitingLink：有 Observe step 等待关联
  - Failed：某 step 失败
  - Completed：全部完成

复用现有的通知机制（项目已有 approval required 的桌面通知）。

依赖：4.1
验收：step 等待审批时弹出桌面通知

---

## 验收总表

| Phase | 验收标准 |
|-------|---------|
| 1 | `cargo test --workspace` 通过；`pnpm build` 通过；能创建 workflow → 手动 link → complete → pipeline 可视化正确 |
| 2 | workflow-context.json 放入 repo 后，新 run 自动关联到正确 step；candidates API 返回合理候选 |
| 3 | Assisted 模式下 Launch step 能审批执行；CLI 正确启动；结果正确收集 |
| 4 | Auto 模式下 Launch step 自动推进；模式运行中可切换；桌面通知正常 |

## 文件清单

### 新增文件

| 文件 | Phase | 职责 |
|------|-------|------|
| `crates/core/src/workflow.rs` | 1.1 | 所有领域类型 |
| `crates/server/src/workflows/mod.rs` | 1.2 | 模块导出 |
| `crates/server/src/workflows/store.rs` | 1.2 | JSON 文件存储 |
| `crates/server/src/workflows/coordinator.rs` | 1.3 | 状态机 + 推进逻辑 |
| `crates/server/src/handlers/workflows.rs` | 1.4 | HTTP API handlers |
| `apps/web/src/components/workflows/WorkflowsView.tsx` | 1.5 | Workflows tab 主视图 |
| `apps/web/src/components/workflows/WorkflowRunList.tsx` | 1.5 | 左栏 run 列表 |
| `apps/web/src/components/workflows/PipelineView.tsx` | 1.5 | 中栏 pipeline 可视化 |
| `apps/web/src/components/workflows/StepDetail.tsx` | 1.5 | 右栏 step 详情 |
| `apps/web/src/components/workflows/WorkflowEditor.tsx` | 1.6 | 轻量编辑器 |
| `crates/server/src/workflows/link_resolver.rs` | 2.3 | 自动关联 + 候选匹配 |
| `crates/server/src/workflows/launcher.rs` | 3.1 | CLI 子进程管理 |
| `crates/server/src/workflows/prompt.rs` | 3.2 | Prompt 模板渲染 |

### 修改文件

| 文件 | Phase | 改动 |
|------|-------|------|
| `crates/core/src/lib.rs` | 1.1 | 加 `pub mod workflow;`，BootstrapPayload 加字段 |
| `crates/server/src/main.rs` | 1.4 | 注册路由，AppState 加 WorkflowStore/Coordinator |
| `crates/server/src/probe.rs` | 1.4 | build_bootstrap 加 workflow_runs summary |
| `apps/web/src/App.tsx` | 1.5 | 加第 6 个 tab |
| `apps/web/src/store/monitorStore.ts` | 1.5 | 加 workflowRuns 字段 |
| `apps/web/src/components/monitor/MonitorView.tsx` | 1.5 | 加 workflow 横幅提示 |
| `crates/adapters/claude/src/lib.rs` | 2.1 | 读 workflow-context.json |
| `crates/adapters/codex/src/lib.rs` | 2.1 | 读 workflow-context.json |
| `crates/server/src/handlers/ingest.rs` | 2.2 | ingest 加可选 workflow hint 字段 |
| `crates/server/Cargo.toml` | 1.2 | 如需 nanoid 依赖 |
| `apps/desktop/src-tauri/src/main.rs` | 4.3 | workflow 桌面通知 |
