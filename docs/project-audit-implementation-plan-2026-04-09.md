# OctoMonitor 重构实施计划

> 日期：2026-04-09
> 对应方案：`docs/project-audit-refactor-plan-2026-04-09.md`
> 目标：把方案文档转成可执行、可验证、顺序明确、尽量不返工的实施路径。

## 1. 固定决策

这些决策先锁定，后续所有步骤都以它们为前提：

- 不引入数据库。
- 不改 WebSocket `snapshot.replace` 主协议。
- 实时数据唯一权威来源仍是 `WS + Zustand`。
- remote 先走显式 allowlist projection，不复制一整套 DTO 树。
- adapter `probe()` 保持同步签名；timeout/隔离由 orchestration 层负责。
- JSONL transcript 的增量基准是 `offset/cursor`，不是 `mtime`。
- runtime continuity 只做 best-effort 的短 TTL `runtime overlay`，不是完整 state persistence。
- `bootstrap` 不在第一步就拆成多把锁；先做 revision、防 stale overwrite、锁外计算。
- 文件拆分按轮次推进，每轮只拆当前改动真正需要的 2-3 个子模块。

## 2. Phase 与 Step 映射

为避免方案文档的 `Phase` 与本计划的 `Step` 混淆，映射关系固定如下：

- 方案 `Phase 0` = 本计划 `Step 0`
- 方案 `Phase 1` = 本计划 `Step 1a + Step 1b + Step 2`
- 方案 `Phase 2` = 本计划 `Step 3 + Step 4 + Step 5a + Step 5b`
- 方案 `Phase 3` = 本计划 `Step 6`
- 方案 `Phase 4` = 本计划 `Step 7 + Step 8`
- 方案 `Phase 5` = 本计划 `Step 9`
- 方案 `Phase 6` = 本计划 `Step 10`

## 3. Gate 规则

每个 Gate 都按同一规则处理：

- 如果 Gate 通过：进入下一 Step。
- 如果 Gate 不通过，但根因在当前 Step：在当前 Step 内修复并重新验证，不回退。
- 如果 Gate 不通过，且根因来自更早 Step：标记阻塞项，回到对应 Step 修复，再重新跑当前 Gate。
- 不允许带着未解释的 Gate 失败进入下一 Step。

## 4. 实施总顺序

1. Step 0：基线观测、格式盘点、测试基线
2. Step 1a：最快速 correctness 修复
3. Step 1b：remote allowlist projection 与错误传播
4. Step 2：热路径重构，直接进入 worker 模式
5. Gate A：确认热路径已脱离锁内重算且无 stale overwrite
6. Step 3：adapter orchestration 契约与隔离
7. Step 4：adapter 增量缓存与 watcher 修复
8. Step 5a：runtime overlay best-effort 连续性
9. Step 5b：地址发现统一
10. Gate B：确认刷新隔离、增量读取、状态恢复、地址发现都成立
11. Step 6：最小 core 拆分 + workflow / 接口语义硬化
12. Step 7：前端数据边界与 hooks 收口
13. Step 8：前端大视图拆分与交互测试
14. Gate C：确认前后端权威边界清晰且视图拆分没有反向增复杂度
15. Step 9：桌面壳、remote 生命周期、发布与文档收尾
16. Step 10：最终验证、CI 与回归清单

## 5. 各步骤实施细则

## Step 0：基线观测、格式盘点、测试基线

目标：

- 在改代码之前先看清热点、锁竞争、payload 体积、adapter 数据格式。
- 固定后续每一步的回归基线。

触点：

- `crates/server/src/probe.rs`
- `crates/server/src/commits.rs`
- `crates/server/src/handlers/ingest.rs`
- `crates/server/src/state.rs`
- `crates/adapters/*`
- Rust / web test helpers

动作：

- 给 `refresh_bootstrap_once()`、`merge_runtime_state()`、`rebuild_derived()`、`build_commit_records()`、`scan_adapters()` 加耗时 `tracing`。
- 给 ingest 路径加采样日志，区分 `statusline`、`hook`、`probe_wake` 来源。
- 给 `BootstrapPayload` 大小加观测，建立 WS 体积基线。
- 盘点 Claude/Codex/OpenClaw 的真实存储结构：
  - 文件粒度
  - 是否 append-only
  - JSONL 每行是否自包含
  - rotate/truncate 风险
  - 是否需要外部上下文
- 抽一套 server integration helper：
  - temp home
  - temp workflow dir
  - temp git fixture / worktree fixture
  - app/router builder
  - 可控的“慢 derive / 慢 probe / 并发 ingest”测试注入点
- 抽一套 adapter fixture helper。
- 建立测试基线：
  - 记录当前通过的 web tests / build
  - 在环境允许时记录 `cargo test --workspace` 清单
  - 不再使用过时的“18 个 Rust 测试”口径

验证：

- 新增日志默认不污染普通输出。
- 至少一条 server integration 测试使用了新 helper。
- 能得到当前 payload 大小、热点函数耗时、adapter 文件格式结论。

退出条件：

- 能回答“谁触发了重算、重算慢在哪里、一次多长、当前 payload 多大、每个 adapter 的文件形态是什么”。

## Step 1a：最快速 correctness 修复

目标：

- 先解决最小、最直接、最高价值的 correctness/security 问题。

触点：

- `crates/server/src/workflows/prompt.rs`
- `crates/server/src/config.rs`
- `crates/server/src/handlers/config.rs`
- `crates/server/src/handlers/remote.rs`

动作：

- 修复 `{{file:...}}`：
  - 仅允许相对路径
  - canonicalize 后必须仍在 `working_dir` 内
  - 拒绝绝对路径、`..`、符号链接逃逸
- 让 `save_config()` 返回 `Result`。
- `patch_config()` / `patch_remote_access()` 在落盘失败时返回错误，不再“内存成功、接口也成功”。

验证：

- workflow include 越界测试通过。
- config/remote 落盘失败测试通过。
- 当前既有测试继续通过。

退出条件：

- 路径逃逸与配置假成功问题已单独关闭。

## Step 1b：remote allowlist projection 与错误传播

目标：

- 在不复制整套 DTO 树的前提下，把 remote 暴露边界做扎实。

触点：

- `crates/server/src/remote_access.rs`
- `crates/core/src/lib.rs`
- remote 相关 tests

动作：

- 把 remote 从 clone-and-null 改成显式 projection。
- 顶层与关键嵌套层都用不带 `..` 的完整 struct literal：
  - `BootstrapPayload`
  - `RunRecord`
  - `CommitRecord`
  - `CompletionRecord`
- 显式决定这些字段的暴露策略：
  - `recent_completions`
  - `workflow_hint`
  - commit 摘要/作者/仓库名
  - project/workspace label
- 承认 compile-time exhaustiveness 的边界：
  - 只对显式 projection 到的层级有效
  - 不把它宣传成“自动覆盖所有嵌套层级”的魔法

验证：

- remote redaction 覆盖 `runs / commits / identities / recent_completions`。
- 当前既有测试继续通过。

退出条件：

- remote 暴露边界已经从“后清洗”改成“先投影”。

## Step 2：热路径重构，直接进入 worker 模式

目标：

- 一次完成热路径的正确性与性能重构。
- 不保留“锁外但同步执行 derive”的中间态，避免二次返工。

触点：

- `crates/server/src/handlers/ingest.rs`
- `crates/server/src/probe.rs`
- `crates/server/src/state.rs`

动作：

- 在 `AppState` 引入 revision/version。
- 明确把状态分成三层概念：
  - raw scan
  - authoritative runtime state
  - derived state
- ingest 写入路径改成：
  - 最小锁内 runtime 更新
  - revision 增加
  - 立即 `signal_change()`，让 run 原始状态先可见
  - 标记 dirty
  - 唤醒 derive worker
- derive worker 内部使用 `snapshot-then-swap`：
  - 锁外算
  - 写回前校验 revision
  - 仅派生字段更新成功后，再做第二次派生广播
- `refresh_bootstrap_once()` 不再是“读旧快照 -> 慢构建 -> 整包覆盖”：
  - raw scan 可以先完成
  - revision 冲突时不丢 raw scan
  - 只废弃旧 revision 上的 merge/derive 结果
  - 再对最新 authoritative runtime state 重放 merge/derive
- commit attribution 从 ingest 即时重算中彻底移出。

验证：

- `upsert_runtime_run()` 不再在写锁内执行 `rebuild_derived()`。
- `refresh_bootstrap_once()` 不会用旧快照覆盖新 ingest 更新。
- 并发测试：
  - 慢 probe 期间到来的 ingest 更新不会丢
  - 慢 derive 期间到来的 ingest 更新不会被旧结果覆盖
- 当前既有测试继续通过。

退出条件：

- “runtime 立即可见 + 后台派生 + revision 防 stale overwrite” 三件事同时成立。

## Gate A：热路径第一轮验收

必须同时满足：

- ingest handler 不再同步执行重型 derive
- probe 刷新不再有 stale overwrite
- 重型派生计算不在写锁内执行
- 现有 WS 行为没有回退
- 当前既有测试继续通过

## Step 3：adapter orchestration 契约与隔离

目标：

- 在不改 adapter `probe()` 同步签名的前提下，把 timeout、panic 隔离和 cache 接口一次设计到位。

触点：

- `crates/server/src/probe.rs`
- `crates/adapters/*`
- `crates/server/src/watcher.rs`

动作：

- 先设计 orchestration 目标契约，而不是先改 adapter trait：
  - timeout
  - panic isolation
  - health downgrade
  - cache handle/state
- adapter 继续保持 `pub fn probe() -> Snapshot`。
- 在 orchestration 层对单个 adapter 使用：
  - `tokio::spawn_blocking`
  - `tokio::time::timeout`
  - panic 捕获/降级
- 明确禁止“panic 后 inline 重跑一次同一个 probe”的兜底方式。
- watcher 修复为：
  - 启动时目录不存在
  - 运行中目录后出现
  - 首次安装工具后自动补注册

验证：

- 某个 adapter 故意 sleep/panic 时，其它 adapter 仍可完成刷新。
- 缺失目录在后续出现时进入 watcher 覆盖范围。
- 当前既有测试继续通过。

退出条件：

- adapter 契约已经稳定，不会在下一 Step 再改一次接口形状。

## Step 4：adapter 增量缓存与 watcher 修复

目标：

- 按每种文件格式的真实特征做增量化，而不是把所有 adapter 套进同一缓存模板。

触点：

- `crates/adapters/claude/src/lib.rs`
- `crates/adapters/codex/src/lib.rs`
- `crates/adapters/openclaw/src/lib.rs`
- 共享 adapter cache/helper 模块

动作：

- 根据 Step 0 的格式盘点结果落地缓存：
  - Claude JSONL：`offset/cursor`
  - Codex session 文件：`mtime + size` 或等价 fingerprint
  - OpenClaw：`sessions.json` 与 session file 分开缓存
- append-only 文件尾部补读。
- truncate/rotate/tail corruption 时回退整文件重建。
- 先做进程内缓存。
- 只有当 Step 0 指标证明冷启动仍不可接受时，再扩展到磁盘 manifest，不提前做。

验证：

- 二次 probe 的读取量明显下降。
- JSONL append 不会重新读完整文件。
- 当前既有测试继续通过。

退出条件：

- adapter 已经按“append-only / mutable-file”两种模式分别增量化。

## Step 5a：runtime overlay best-effort 连续性

目标：

- 在不引入数据库的前提下，尽力减少 desktop 重启后的 live 状态断层。

触点：

- `crates/server/src/probe.rs`
- `apps/desktop/src-tauri/src/main.rs`
- 新增 overlay 模块

动作：

- 新增短 TTL `runtime overlay`：
  - active/idle/waiting runs
  - ingest-only recent completions
  - 最小身份信息 + last seen
  - schema version
- overlay 作为额外 runtime source 进入统一 merge 路径，不发明单独 overlay merge 语义。
- 周期性 debounce 刷盘。
- graceful shutdown 时尽量再刷一次。
- 如需要，调整 desktop 当前约 200ms 的 SIGTERM grace window。
- 默认 TTL 控制在分钟级，建议约 5 分钟。

验证：

- 重启 desktop 后，近期 live runs 可 best-effort 恢复。
- 过期 overlay 不会长期污染界面。
- 当前既有测试继续通过。

退出条件：

- overlay 已经是受控的短期恢复机制，而不是隐式 mini database。

## Step 5b：地址发现统一

目标：

- 去掉 `ifconfig`/`8.8.8.8` 依赖，让地址发现完全本地化。

触点：

- `crates/server/src/network.rs`
- `crates/server/src/probe.rs`

动作：

- 把 `remote access` 和 `config.local_ip` 收敛到同一套地址发现实现。
- 地址发现与地址分类分开。
- 继续保留 LAN / private / tailscale 分类。

验证：

- 关闭外网时地址发现仍可工作。
- remote 与 bootstrap 的地址来源一致。
- 当前既有测试继续通过。

退出条件：

- 地址发现完全本地化，且实现不再分叉。

## Gate B：基础设施第二轮验收

必须同时满足：

- derive worker 合并刷新稳定
- adapter timeout/panic 隔离稳定
- JSONL 增量补读有效
- overlay best-effort 恢复正常
- 地址发现完全本地化
- 当前既有测试继续通过

## Step 6：最小 core 拆分 + workflow / 接口语义硬化

目标：

- 在 workflow 与类型密集修改前，先做最小必要的 core 拆分。
- 同时把 workflow 从“可用功能”提升为“可靠子系统”。

触点：

- `crates/core/src/lib.rs`
- `crates/core/src/workflow.rs`
- `crates/server/src/workflows/*`
- `crates/server/src/handlers/workflows.rs`

动作：

- 先做最小 core 拆分：
  - `run.rs`
  - `commit.rs`
  - `remote.rs`
  - 如确有必要，再补 workflow 相关模块
- workflow/api error enum + handler 映射。
- `change_mode()` 重算 step state。
- `link_run()` / `unlink_run()` 显式业务校验。
- artifact 从路径包含判断升级到结构化 identity。
- ID 改成时间可排序的强唯一方案。
- store 持久化补强：
  - `tmp sync_all`
  - `rename`
  - 父目录 best-effort sync
  - 索引损坏重建 + 可见 warning
- launcher：
  - stderr 读取与持久化
  - 执行日志持久化
  - timeout/exit 分类

验证：

- workflow handler 错误码测试通过。
- `change_mode()`、artifact 校验、link/unlink 行为测试通过。
- core 最小拆分后没有制造额外状态体系。
- 当前既有测试继续通过。

退出条件：

- workflow 不再依赖隐式 if/else 语义。

## Step 7：前端数据边界与 hooks 收口

目标：

- 先把数据权威边界讲清楚，再拆页面。

触点：

- `apps/web/src/App.tsx`
- `apps/web/src/store/monitorStore.ts`
- `apps/web/src/lib/api.ts`
- 新增 `apps/web/src/hooks/*`

动作：

- `useBootstrapStream` 只封装 `WS + Zustand`。
- 抽 `useRemoteViewerAuth`、`useDesktopBootStatus`、`useWaitingNotifications`。
- 历史/按需数据才走：
  - `useHistoryRangeData`
  - `useWorkflowRunDetail`
  - `useRemoteAccess`
- 明确禁止同一份业务数据同时存在 WS 权威和 HTTP 权威两条链路。

验证：

- `App` 在 `local / remoteViewer / tauri` 三种模式下测试通过。
- store 没变成第二套异步缓存系统。
- 当前既有测试继续通过。

退出条件：

- 数据边界已收口，不会在 Step 8 拆页面时再发明第二套真相源。

## Step 8：前端大视图拆分与交互测试

目标：

- 降低复杂页面认知负担，同时把关键交互纳入自动回归。

触点：

- `apps/web/src/components/monitor/MonitorView.tsx`
- `apps/web/src/components/monitor/HeatmapView.tsx`
- `apps/web/src/components/monitor/CommitsView.tsx`
- `apps/web/src/components/workflows/*`
- `apps/web/src/lib/i18n.tsx`

动作：

- `MonitorView`：先拆 view-model + 展示区块。
- `HeatmapView`：先拆 view-model + surface/sidebar。
- `CommitsView`：先恢复测试，再拆。
- workflows：补 detail / preview / link / unlink / mode change 覆盖。
- `i18n.tsx`：拆 provider 与字典。

验证：

- `CommitsView.test.tsx` 不再 `skip`。
- workflows 与 App 关键分支可回归。
- 当前既有测试继续通过。

退出条件：

- 页面拆分后，测试覆盖上升而不是下降。

## Gate C：前后端边界验收

必须同时满足：

- 实时数据链路仍然单一
- `CommitsView` 测试恢复
- workflows 关键交互可回归
- 页面拆分没有引入第二套状态体系
- 当前既有测试继续通过

## Step 9：桌面壳、remote 生命周期、发布与文档收尾

目标：

- 处理真正后置的工程卫生问题，不抢占核心正确性工作的优先级。

触点：

- `apps/desktop/src-tauri/src/main.rs`
- `crates/server/src/remote_access.rs`
- 发布脚本
- 文档目录

动作：

- 把 boot status 从 `window.eval()` 改成明确 bridge/event。
- remote cookie/session/pairing 生命周期规则文档化并收口实现。
- 检查 `daily summary / workflow launch / installer` 的边界和日志。
- 清理旧文档与发布产物边界。

验证：

- 桌面壳不再依赖脚本注入。
- 发布检查清单可自动执行。
- 当前既有测试继续通过。

退出条件：

- 工程卫生问题不再反向影响主功能设计。

## Step 10：最终验证、CI 与回归清单

目标：

- 把“感觉稳定”变成“可重复验证”。

动作：

- 补全 Rust integration tests。
- 增加 property-based tests：
  - usage bucket 守恒
  - commit attribution 不超总量
  - history trim 不丢 pinned runs
- 固化 CI 分层：
  - 快速层
  - 中等层
  - 重型层
- 补齐 desktop/remote smoke。

退出条件：

- 核心链路可以在 CI 中稳定重现。

## 6. 持续约束

- 任何一步只要引入第二套数据权威来源，就回退重做。
- 任何一步只要为了“更规范”而复制一整套类型树或状态树，就先停下来复核是否过度设计。
- 任何一步如果需要拆出超过 3 个新模块，必须先解释为什么当前边界不够。
- 任何一步在没有测试保护前，不允许同时做“行为修改 + 大规模文件移动”。
- `window.eval()`、文档清理、发布整理这类事项，不能与热路径、安全修复抢顺序。

## 7. 对最新审阅意见的逐条复核

1. 方案文档对 stale overwrite 的处理不完整：采纳。
   现在计划里把刷新链路拆成 `raw scan / authoritative runtime / derived`，并明确 revision 冲突时复用 raw scan，不简单丢弃 probe 结果。

2. 缺少 Claude/Codex/OpenClaw 实际文件格式分析：采纳。
   现在存储格式盘点已前移到 `Step 0`，Step 4 只在盘点结论成立后再落缓存。

3. compile-time exhaustiveness 承诺需要更具体实现路径：采纳。
   这在方案文档里已经收窄为“逐层 projection 函数 + 不带 `..` 的完整 struct literal”，计划文档据此执行。

4. `atomic_write()` 问题描述不准确：采纳。
   计划里不再追求 DB 级 durability，而是采用轻量持久化补强 + 索引损坏重建。

5. runtime overlay 与 no-DB 原则有张力：采纳，但边界已收窄。
   计划把它改成 best-effort 的短 TTL runtime source，并通过统一 merge 路径进入系统，不再发明 overlay 专属语义。

6. 方案遗漏 WS 体积预算：采纳。
   Step 0 已加入 payload 大小基线，后续通过 server-side 裁剪守预算，不改协议。

7. adapter probe 从同步调用图到 async 的迁移风险：采纳。
   计划明确保持 adapter `probe()` 同步签名，timeout 只放在 orchestration 层做。

8. Step 2/3 边界不清、会返工：采纳。
   计划已合并为一个完整的热路径重构步骤，不再保留“锁外但同步 derive”的过渡态。

9. Step 4/5 缺少 adapter 目标签名前置设计：采纳。
   计划已新增 `Step 3` 专门锁定 orchestration 契约，再进入缓存实现。

10. Step 1 粒度过大：采纳。
    计划已拆成 `Step 1a / Step 1b`。

11. Gate 缺少失败处理策略：采纳。
    文档现在有统一 Gate 规则。

12. 缺少现有测试基线保护：采纳。
    `Step 0` 已加入基线记录；所有 Step 的验证都包含“当前既有测试继续通过”。

13. Step 6 把 overlay 和地址发现绑在一起：采纳。
    现在已拆为 `Step 5a / Step 5b`，可并行或任意顺序推进。

14. 缺少 `core/src/lib.rs` 拆分时机：采纳。
    现在明确放在 `Step 6` 开头，作为 workflow/type-heavy 修改前的最小前置任务。

15. Phase/Step 映射不显式：采纳。
    文档已新增固定映射表。

16. `stderr capture` 表述不准：采纳。
    计划已改成“stderr 读取与持久化”，与当前代码事实一致。
