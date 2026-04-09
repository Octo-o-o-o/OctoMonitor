# OctoMonitor 全量检查后的重构与补充方案

> 日期：2026-04-09
> 结论基于：仓库结构、核心 Rust crates、Tauri 壳、前端主要视图、workflow 子系统、adapter、测试文件与现有设计文档的完整阅读。
> 对应实施计划：`docs/project-audit-implementation-plan-2026-04-09.md`
> 已实际验证：
> - `apps/web` 的 `vitest --run` 通过，11 个测试文件里 10 个通过，1 个文件整体 `skip`
> - `apps/web` 的 `vite build` 通过
> - Rust 侧 `cargo test/clippy` 未能完成验证：当前环境同时遇到 crates.io 索引网络超时，且本地离线缓存不完整，无法给出“已通过”结论
> 二次复核修订：
> - `docs/architecture-upgrade.md` 中多项建议已经落地：`127.0.0.1` 默认绑定、事件驱动 WebSocket 首帧快照、`ErrorBoundary`、`SettingsView` 拆分、Tailwind、`ts-rs` 类型导出、桌面 crate 继承 workspace metadata、端口冲突提示；旧文档应降级为历史参考
> - remote redaction 当前不仅遗漏 `workspace_short / workflow_hint / project_name / commit summary`，还遗漏了 `recent_completions` 中的 `project_name / title / summary`
> - `patch_config()` 与 `patch_remote_access()` 在 `save_config()` 落盘失败时仍会返回成功，这个问题应纳入 API/持久化语义修复
> - 网络发现问题不只在 `network.rs`，`probe.rs::detect_local_ip()` 同样依赖连 `8.8.8.8` 推断地址
> - 文件 watcher 目前只注册启动时已存在的目录，运行中首次安装工具或首次创建目录后不会自动进入监听
> 三次复核修订（结合 Opus 审阅逐条回看代码后更新）：
> - P0-1 已补成明确 mini design：derive worker 内部统一采用 `snapshot-then-swap`，但交付顺序上直接进入“写 runtime + 标记 dirty + 唤醒 worker”模式；不在第一步就拆双锁
> - JSONL transcript 的增量基准改为 `offset/cursor`，`mtime + size` 只用于非 append-only 文件或截断检测
> - remote 不再建议维护一整套完全独立 DTO，改为“显式 allowlist 投影构造”，保留 compile-time exhaustiveness，同时避免无谓复制
> - workflow ID 不再笼统建议换成 `uuid v4`，而是换成“时间可排序且真正唯一”的 ID（如 UUIDv7/ULID/时间戳+计数器）
> - 新增两项此前遗漏但合理的补强：runtime overlay 重启恢复、adapter probe 的 timeout/panic 隔离
> 四次复核修订（结合最新审阅意见与当前实现再次校对）：
> - `refresh_bootstrap_once()` 的风险不只是“锁内太重”，还包括 `merge_runtime_state()` 基于旧快照锁外合并导致的 stale overwrite；冲突处理现在明确为“复用 raw scan、重放 merge/derive”，而不是简单丢弃 probe 结果
> - compile-time exhaustiveness 的实现路径已收窄为“显式 projection 函数 + 不带 `..` 的完整 struct literal”，并明确承认它只对显式投影到的层级生效
> - runtime overlay 从“保证重启连续性”改成“best-effort 的短 TTL 运行态恢复”，并明确作为额外 runtime source 走统一 merge 路径
> - adapter timeout 的方案已明确：保持 adapter `probe()` 同步签名，不先改 adapter API；超时与隔离由 orchestration 层的 `spawn_blocking + timeout` 负责
> - 全量 WebSocket 快照保留，但新增 payload 体积预算与 server-side 裁剪约束，避免增量化成功后反向把传输层拖慢

## 1. 总体判断

OctoMonitor 已经不是“原型”，而是一个方向正确、能力面完整、可继续演进的本地监控产品雏形。当前最值得肯定的地方有五个：

- 架构主线是对的：`core / server / adapters / installer / companion / desktop / web` 的边界已经形成，后续可以继续演进，不需要推倒重来。
- 产品能力面已经闭环：本地监控、历史、commit attribution、workflow、remote viewer、desktop packaging 都已有实现，不是空壳。
- 技术栈克制：Rust workspace + React/Vite + Tauri 2 + Zustand，运行时依赖少，维护成本总体可控。
- 类型同步方向正确：`crates/core/bindings/*` 已经把 Rust 类型导出到前端，避免了最危险的手写双份协议漂移。
- 测试不是空白：前端核心工具函数和部分复杂 UI 已有测试，Rust 侧对 `probe / commits / pricing / workflows / inspect / remote_access` 也已有一批单元测试。

但如果要把这个版本从“稳定可用”继续推进到“长期可维护、可扩展、可对外发布且抗风险”，当前有一批结构性问题必须处理。最核心的不是 UI，也不是小 bug，而是：

- 热路径上做了太多全量计算
- 一些模块已经明显过大，修改成本和误伤面开始上升
- workflow 与 remote access 的边界还不够硬
- 测试主要覆盖函数级，缺少跨模块、跨协议、跨运行形态的验证
- 现有文档里已有一部分计划过时，代码与文档开始分叉

这份方案的原则是：

- 保留现有整体架构，不推翻
- 先解决性能、稳定性、安全边界，再做体验层整理
- 尽量用“拆分现有模块 + 增加缓存/索引 + 增加测试”解决问题，而不是引入大而新的框架
- 明确哪些事情现在不该做，避免为了“完整”而过度设计

## 2. 保留项

以下设计建议明确保留，只做强化，不建议推翻：

- 保留 Rust 本地服务端，不改成纯前端或纯 Tauri event 模式
- 保留 Tauri 2 桌面壳，不改 Electron
- 保留无数据库、local-first、read-only by default 的产品原则
- 保留 `core` 作为共享领域模型来源
- 保留 adapter-per-tool 模式，不引入通用插件系统
- 保留 WebSocket 传全量快照的主思路，不在当前阶段做细粒度 diff/event sourcing
- 保留当前 `broadcast + snapshot.replace` 的 change-driven WebSocket 方案；`stream.rs` 已是“连接即首帧 + 有变化才推送”，不需要再回退到旧计划里的重做提案
- 保留 Zustand；前端先通过 selector、hook 拆分和组件拆分减压，不急着换状态库
- 保留 workflow 的线性 step 模型；当前阶段不扩成 DAG / 并行编排

## 2.1 已完成、不要重复规划的事项

以下事项已经在代码里落地，不应再被当作本轮重构目标重复提出：

- 默认绑定 `127.0.0.1`、明确端口冲突提示，已经完成
- WebSocket 已经是事件驱动模式，并在连接时主动发送首帧快照
- `ts-rs` 类型导出、`ErrorBoundary`、Tailwind、`SettingsView` 拆分、desktop crate 继承 workspace metadata，都已经完成
- adapter probe 已经通过 `std::thread::scope` 并行化；当前瓶颈是“每个 probe 自身全量扫描”，不是“probe 串行执行”

## 3. 当前主要问题与建议优先级

## P0：必须优先处理

### P0-1. 实时 ingest 热路径触发了重型全量重建

现状：

- `crates/server/src/handlers/ingest.rs` 中的 `upsert_runtime_run()` 每次收到 hook/statusline 都会直接调用 `rebuild_derived()`
- `crates/server/src/probe.rs` 的 `refresh_bootstrap_once()` 先读旧快照、锁外重建、最后整包写回；在重建期间进入的新 ingest 更新有被旧结果覆盖的风险
- `rebuild_derived()` 会继续触发：
  - run 历史裁剪
  - VCS hydration
  - usage bucket 全量重算
  - commit attribution 全量重算
- commit attribution 又会进入 `crates/server/src/commits.rs` 的 git 扫描与归因流程

问题：

- 高频事件和高成本计算耦合在一起
- 一次轻量状态更新，可能引发整个 repo commit 扫描
- 写锁持有时间被拉长，WebSocket 推送与其它 handler 都会受到影响
- 现有 probe 刷新还存在 stale snapshot overwrite 风险，不只是“慢”，而且可能把更新更晚的 runtime 状态写掉
- 实时体验越好，系统越容易被自己拖慢

建议：

- 把 `ingest` 写入与 `derived rebuild` 解耦
- 第一阶段先不拆 `AppState.bootstrap` 这把大锁，也不急着引入 `runtime/derived` 两套锁；先锁定更低风险的写回策略：derive worker 内部统一采用 `snapshot-then-swap`
  - 锁内只做最小状态更新与快照抓取
  - usage/history/commit 等派生计算全部移到锁外
  - 计算完成后短暂获取写锁，只替换派生字段
- 为 ingest 与 probe 都引入 revision/version 校验，禁止“旧快照算出的结果整包覆盖新 runtime”
- 把刷新链路明确拆成三层：
  - raw scan：adapter 扫到的原始运行态/身份/health 信息
  - authoritative runtime state：最新 ingest + overlay + raw scan 合并后的运行态真相
  - derived state：usage / commits / attentions 等派生字段
- `upsert_runtime_run()` 只做最小状态更新与轻量字段修正，写完后立即广播，让 run 原始状态先可见；随后标记 dirty、唤醒 derive worker，在 debounce 后做派生二次广播
- derive worker 不搭车现有 30s/120s probe 周期
  - reason: probe 周期太粗，不适合承载秒级 ingest 刷新
  - worker 通过 `Notify`/channel 接收 dirty 事件并做 debounce
- dirty 粒度明确为：
  - `usage_dirty: bool`
  - `history_dirty: bool`
  - `commit_dirty: all | repo/worktree set | unknown`
- 刷新上限明确为：
  - run 原始状态：立即写入、立即可见
  - usage/history/轻量派生：250-500ms 合并刷新，最晚 1s 内可见
  - commit attribution：1-2s debounce，最晚 5s 内收敛
- 对 `build_commit_records()` 改成 repo 维度增量/缓存刷新，而不是每次全量扫描
- revision 冲突时不应简单丢弃整个 probe 结果：
  - 已完成的 raw scan 结果应被复用
  - 只废弃基于旧 revision 的 merge/derived 结果
  - 再对最新 authoritative runtime state 重放 merge/derive
- 如果 Phase 0 指标证明锁竞争依然明显，再进入第三步：把 `bootstrap` 拆成 `runtime snapshot + derived snapshot` 两个锁，而不是一开始就拆

目标：

- 单次 ingest 请求不再执行 git 扫描
- 写锁持有时间显著缩短
- 连续高频 hook/statusline 事件只触发一次派生刷新
- 重型派生计算不再在 `RwLock<BootstrapPayload>` 的写锁内执行

### P0-2. adapter 全量扫描策略在数据量变大后会失控

现状：

- `crates/adapters/claude/src/lib.rs` 每次 probe 都遍历 `projects/*/*.jsonl`
- `crates/adapters/codex/src/lib.rs` 递归遍历 `sessions` 与 `archived_sessions`
- `crates/adapters/openclaw/src/lib.rs` 会读取 `sessions.json`，并对 session file 再做全文扫描
- `crates/server/src/watcher.rs` 只会注册启动时已经存在的目录，后续新出现的目录不会补注册

问题：

- 当前实现默认“全读全算”
- 运行时间将随 transcript/session 数量线性上升
- 历史积累足够大时，`probe.rs` 会成为系统瓶颈
- adapter probe 其实已经是并发的，真正的扩展瓶颈是“并发执行的全量扫描”

建议：

- 在真正写增量缓存前，先做一次 adapter 存储格式盘点：
  - Claude：确认是“每 session 一个 JSONL”还是“每 project 一个 JSONL”
  - Codex：确认 session 文件是否自包含、是否需要外部元信息
  - OpenClaw：确认 `sessions.json` 与 session file 的关系
- 为每个 adapter 建立文件索引缓存：
  - append-only JSONL transcript：以 `path + last_offset + truncation check` 为主
  - 非 append-only session 文件：以 `path + mtime + size` 为主
- 针对 JSONL transcript 明确改成“尾部补读”模型，而不是每次靠 `mtime` miss 触发整文件重读
  - 文件变短、inode/file id 变化、JSONL 尾部损坏时，自动回退到全量重建
- 让 watcher 支持缺失目录延迟注册与运行中补注册
- 对历史目录引入扫描上限与最近活跃优先策略
- 给每个 adapter probe 加 timeout 与 panic 隔离
  - panic 不能再通过“重新 inline 跑一次同一个 probe”兜底
  - 单个 adapter 超时或 panic 时，只降级该 adapter health，不拖住整轮 refresh
- 保持 adapter `pub fn probe() -> Snapshot` 的同步签名；不要先把 adapter API 改成 async
  - timeout 与并发隔离放在 orchestration 层，用单个 adapter 级别的 `spawn_blocking + timeout` 包装
- 区分：
  - 活跃数据源：高频、精确、低成本
  - 补全数据源：低频、重型、只用于历史修补

目标：

- 常规 probe 只处理变化过的文件
- 历史规模增长时，刷新耗时接近稳定
- 单个 adapter 异常不会拖垮整轮 probe

### P0-3. workflow prompt 文件注入存在目录逃逸风险

现状：

- `crates/server/src/workflows/prompt.rs` 的 `{{file:...}}` 直接 `Path::new(working_dir).join(file_path)`
- 没有限制 `..`、绝对路径、符号链接跳出工作目录

问题：

- 在 workflow 定义来自不可信输入时，可以读取任意本地文件
- 即使当前主要是本地使用，也不应把“模板读任意文件”作为默认能力

建议：

- 对 file inclusion 做强约束：
  - 仅允许相对路径
  - canonicalize 后必须仍在 `working_dir` 内
  - 默认拒绝隐藏目录与敏感文件模式
- 为 workflow schema 增加明确的 `allow_file_includes` 或 `include_roots` 配置
- 对被注入文件记录审计信息，便于调试与安全排查

目标：

- workflow prompt 模板不再具备任意文件读取能力

### P0-4. remote viewer 的脱敏边界还不够严格

现状：

- `crates/server/src/remote_access.rs` 已对 `workspace_path / transcript_path / account_alias / ids / questions / links` 做了大部分脱敏
- 但仍保留了若干可能泄漏上下文的信息：
  - `workspace_short`
  - `workflow_hint`
  - `project_name`
  - `recent_completions` 的 `project_name / title / summary`
  - commit `summary / author_name / repo_name`
- 当前 redaction 测试只覆盖了 `runs / commits / identities`，没有覆盖 `recent_completions`

问题：

- 当前 remote surface 是只读，但不等于低敏
- 对局域网或私网可见的内容，应该先有数据分级，再决定是否展示

建议：

- 不再 clone 后局部抹字段，而是改成显式 allowlist 投影构造
  - 第一阶段继续复用 `BootstrapPayload / RunRecord / CommitRecord` 等现有 core 类型
  - 但 remote payload 必须通过显式构造函数逐字段填写，而不是先 clone 再 null-out
  - 顶层 `BootstrapPayload` 与每个被投影的嵌套类型都使用不带 `..` 的完整 struct literal
  - 这样在这些显式 projection 函数覆盖到的层级里，新增字段会触发编译错误，迫使实现者决定“保留还是隐藏”
- 只有当 local/remote 的形状真正长期分叉时，再引入独立 remote types，而不是现在就复制整套 DTO
- 明确认可这个保证的边界：
  - 它不是魔法，也不会自动覆盖所有嵌套层级
  - 只有对 `RunRecord / CommitRecord / CompletionRecord` 等逐一写 projection 函数的层级，才有 compile-time exhaustiveness
- 做明确分级：
  - 必须保留：状态、计数、趋势、来源类型
  - 可选保留：项目名、workflow 名称这类低结构化标签
  - 默认去除：路径、账号、工作流 artifact/path 提示、原始 prompt 摘要、自由文本摘要、commit 作者与摘要
- 增加 remote redaction 测试矩阵，覆盖 workflow/commit/origin/recent-completions 等分支

目标：

- remote 暴露路径变成 allowlist + compile-time exhaustiveness
- 新字段不会因为 clone-and-null 漏掉而意外外泄
- 脱敏策略可审计、可回归测试

### P0-5. API 错误语义过于粗糙

现状：

- `crates/server/src/handlers/workflows.rs` 等模块大量把业务错误统一映射为 `500`
- `crates/server/src/handlers/config.rs` 与 `crates/server/src/handlers/remote.rs` 在配置落盘失败时仍返回成功

问题：

- 前端无法区分“用户输入非法”“资源不存在”“冲突”“系统错误”
- workflow 这种带状态机的能力，没有清晰错误码会很难继续扩展
- “看起来修改成功、实际没有落盘”的接口语义会制造最难排查的一类配置问题

建议：

- 引入统一的 API error 类型
- 让 `save_config()` 返回 `Result`，由 `patch_config()` / `patch_remote_access()` 决定返回错误，而不是只打 warning
- 至少规范以下映射：
  - `400`：非法输入/非法状态变更
  - `404`：workflow/run/step 不存在
  - `409`：工作区已有活动 run、资源冲突
  - `422`：业务规则不满足，例如缺 required artifact
  - `500`：真正的系统内部错误

目标：

- workflow/remote/config/installer API 都能给出稳定、可消费的错误语义
- 配置修改接口不能再把未落盘的修改报告为成功

## P1：高优先级结构整理

### P1-1. 过大的核心文件已经开始成为维护风险

当前明显超大的文件包括：

- `crates/server/src/probe.rs` 1820 行
- `crates/server/src/commits.rs` 1150 行
- `crates/core/src/lib.rs` 967 行
- `apps/web/src/lib/i18n.tsx` 897 行
- `apps/web/src/components/monitor/HeatmapView.tsx` 690 行
- `apps/web/src/components/monitor/MonitorView.tsx` 632 行
- `crates/server/src/handlers/inspect.rs` 648 行
- `crates/server/src/workflows/coordinator.rs` 674 行

建议：

- 以“按职责”而不是“按层次”拆分
- 每一轮只拆到 2-3 个子模块，不做一次性文件爆炸
- 后端第一轮优先拆：
  - `probe.rs` -> `probe/scan.rs`, `probe/derive.rs`，主文件先保留 orchestration / merge
  - `commits.rs` -> `commits/scan.rs`, `commits/attribution.rs`
  - `core/src/lib.rs` -> 先拆 `run.rs`, `commit.rs`, `remote.rs`
- 后端第二轮再看是否需要继续拆 `cache/context/history/runtime_merge`
- 前端第一轮优先拆：
  - `MonitorView` -> `monitor/view-model.ts` + 若干展示区块
  - `HeatmapView` -> `heatmap/view-model.ts` + surface/sidebar
  - `i18n.tsx` -> `i18n/provider.tsx`, `i18n/en.ts`, `i18n/zh.ts`

目标：

- 热点文件都降到可维护区间
- 功能修改时不必在一个大文件里横跳
- 拆分本身不制造巨量低价值 diff

### P1-2. workflow 子系统能力已有雏形，但状态机与存储层还不够硬

现状：

- `WorkflowStore` 以 JSON 文件落地
- `WorkflowCoordinator` 管理状态推进
- `handlers/workflows.rs` 负责 API 和 auto launch

问题：

- `WorkflowStore::atomic_write()` 当前是 `write + rename`，rename 本身足够原子；真正缺的是轻量持久化补强与损坏恢复策略
- def/run 索引文件解析失败时会静默回退为空索引，容易把“数据损坏”误判成“没有数据”
- 当前 ID 生成并不是真随机：后缀由时间戳本身推导，同一毫秒内创建多个 def/run 时存在确定性碰撞风险
- 切换 execution mode 时没有重新评估当前 step 状态
- artifact 校验是字符串包含，不够可靠
- link/unlink 缺少更严格的业务验证

建议：

- workflow run/def ID 改成“时间可排序且真正唯一”的方案
  - 首选 `UUIDv7 / ULID / 时间戳 + 计数器 + 随机后缀`
  - 不建议换成普通 `uuid v4`，避免失去排序与排查便利性
- store 层引入轻量但足够的持久化补强：
  - `tmp` 文件 `sync_all()`
  - 再 `rename`
  - 父目录 `fsync` 视平台能力做 best-effort，而不是追求数据库级保障
- 索引损坏时应重建而不是静默丢失；必要时记录损坏文件并给出 warning
- mode 切换时重算当前 pending/ready/approval 状态
- 对 artifact 引入结构化 identity，而不是依赖 path `contains`
- 让 coordinator 输出 typed event，供前端和日志系统消费

目标：

- workflow 可以稳定承载更复杂的自动推进逻辑，而不会变成隐式 if/else 丛林

### P1-3. remote access 与本地地址发现实现跨平台性不足

现状：

- `crates/server/src/network.rs` 解析 `ifconfig`
- fallback 通过连接 `8.8.8.8:80` 推断本机 IP
- `crates/server/src/probe.rs::detect_local_ip()` 也通过连接 `8.8.8.8:80` 推断地址

问题：

- 对 Linux/Windows 兼容性不稳
- 对无外网环境不友好
- 网络发现逻辑和产品策略耦合过重
- 同一类地址发现逻辑在 `network.rs` 与 `probe.rs` 分散实现，后续容易继续漂移

建议：

- 改为跨平台网卡枚举方案
- 合并成单一地址发现模块，让 `remote access` 与 `config.local_ip` 共用同一实现
- 把“地址发现”和“地址过滤/分类”拆开
- 对 LAN / private / tailscale 分类保留，但不要依赖外部地址探测

目标：

- remote access 地址发现与 bootstrap 本地地址发现都完全本地化、跨平台化

### P1-4. runtime overlay 缺少重启恢复，桌面重启后 live 状态会丢

现状：

- server 的 runtime 状态主要在内存里
- desktop 关闭时会直接终止 sidecar/server 进程
- ingest 写进来的 live run，如果短期内无法被 adapter probe 重新构造，重启后会直接丢失

问题：

- 用户关闭再打开 desktop 时，活动视图可能突然清空或回退
- 对“实时监控器”来说，这是一种用户可见的连续性缺口
- 这类状态其实不需要数据库，但也不应完全靠运气等下一轮 probe 恢复

建议：

- 把 overlay 视为一个额外 runtime source，而不是发明一套单独 merge 语义
- 只持久化“runtime overlay”，不持久化整个 bootstrap
  - active/idle/waiting runs
  - ingest-only recent completions
  - 必要的 last seen 时间与最小身份信息
- 持久化方式用本地 JSON snapshot/journal 即可，不引入数据库
- 写入策略：
  - debounce 周期性刷盘
  - graceful shutdown 时尽量刷一次
  - 启动时加载最近一次 overlay，并先并入 authoritative runtime state，再进入 derive/trim，而不是直接覆盖展示快照
- 当前 desktop 侧 SIGTERM 到 SIGKILL 只有约 200ms，若需要 shutdown flush，应一并调整这段 grace window
- 给 overlay 设置短 TTL 与 schema version，避免陈旧快照长期污染界面
  - 默认 TTL 建议 5 分钟级别，而不是小时级
- overlay 恢复是 best-effort，不保证一定恢复，只尽量减少桌面重启后的状态断层

目标：

- desktop 重启后，尽力减少“所有 live 状态瞬间消失”的体验断层

### P1-5. 全量 WebSocket 快照需要体积预算

现状：

- 项目明确保留 `snapshot.replace` 模式
- Phase 2 之后，adapter 增量化成功会提升系统可承载的数据规模
- 如果只提升扫描能力，不同步约束 bootstrap 体积，WS 每次全量推送的成本会被反向放大

问题：

- 传输层可能变成新的瓶颈
- 这不是要求改成 diff/event sourcing，而是要求对 payload 体积做预算与裁剪

建议：

- 继续保留全量快照协议，不改成 diff
- 给 `BootstrapPayload` 设定常态体积预算：
  - 常态目标：原始 JSON 小于约 500KB
  - 超预算时优先做 server-side 裁剪，而不是改协议
- 裁剪优先级：
  - history window
  - recent completions 数量
  - commits / runs cap
  - remote payload 单独更严格
- 在 Phase 0 把 payload 大小加入观测项

目标：

- 增量化提升扫描规模后，不会把 WS 全量推送拖成新的瓶颈

## P2：前端结构与交互质量

### P2-1. 前端数据获取模式过于分散

现状：

- 各视图自己 `useEffect + fetch`
- `App.tsx` 里还维护 WebSocket、remote auth check、desktop boot status、通知逻辑

问题：

- 请求、缓存、错误状态、重试逻辑分散
- Heatmap / Usage / Commits / Workflow detail 的模式不一致
- 后续要补测试时，需要每个组件都手动 mock 一遍

建议：

- 不新增重量级依赖，先在现有栈上建立 `hooks/`：
  - `useBootstrapStream`
  - `useHistoryRangeData`
  - `useWorkflowRunDetail`
  - `useRemoteAccess`
  - `useDesktopBootStatus`
- 把“拉取数据”和“展示数据”拆开
- 明确数据源边界：
  - `useBootstrapStream` 只负责 WS/Zustand 这条实时链路，不再额外维护第二套 fetch cache
  - `useHistoryRangeData` / `useWorkflowRunDetail` / `useRemoteAccess` 才是 HTTP/on-demand hooks
  - 同一类领域数据不允许同时存在“WS authoritative + HTTP authoritative”两套真相源
- 保持 Zustand 只存真正的全局状态，局部异步状态下沉到 hook

目标：

- 组件主要负责渲染，异步逻辑进入可复用 hook
- hooks 不会引入新的数据权威歧义

### P2-2. 部分大视图已经明显承担太多职责

现状：

- `MonitorView` 同时负责：
  - runs 分组
  - source column 汇总
  - quota 显示
  - cron 展开
  - mobile tab
  - workflow banner
- `HeatmapView` 同时负责：
  - scope cache
  - history 请求
  - summary
  - selected cell
  - sidebar
  - AI summary
- `InsightsSection` 同时负责：
  - installer detect
  - 报告生成队列
  - clipboard
  - UI 折叠

建议：

- 统一按“容器组件 + 纯展示组件 + 工具 hook”拆
- 复杂视图里禁止继续增加新职责
- 为每个大视图补一层 view-model 函数，减轻 JSX 中的条件分支

目标：

- 每个页面存在清晰的可测试边界

### P2-3. i18n 文件过大且耦合 Provider 与字典

现状：

- `apps/web/src/lib/i18n.tsx` 接近 900 行

问题：

- 每次改文案都要动 provider 逻辑文件
- 难以审查遗漏 key

建议：

- locale 字典拆到独立文件
- key 类型继续保留编译期校验
- 加一个 `missing key` 开发期告警

目标：

- i18n 可维护性提升，但不牺牲当前类型安全

## P3：测试、验证、CI 补齐

### P3-1. 前端测试覆盖不均衡

现状：

- 当前前端验证：
  - `vitest --run` 通过
  - 26 个测试通过，2 个测试被跳过
- `apps/web/src/components/monitor/CommitsView.test.tsx` 整个 suite 是 `describe.skip`

缺口：

- MonitorView 无测试
- WorkflowsView / StepDetail / WorkflowEditor 无测试
- RemoteAccessSection / SetupSection / SystemSection 几乎无测试
- App 的 WebSocket / remote auth / desktop boot 分支缺少覆盖

建议：

- 第一优先级恢复 `CommitsView` 测试，不允许长期整体 skip
- 为 workflows 补三类测试：
  - run detail refresh
  - step actions
  - mode change / cancel
- 为 `App.tsx` 的 runtime mode 分支补测试
- 为 remote viewer/auth gate 增加组件测试

目标：

- 前端复杂视图不再只靠人工回归

### P3-2. Rust 测试偏单元化，缺少 handler/integration 层

现状：

- Rust 已有较多单元测试，覆盖解析、归因、workflow 状态机、pricing、inspect 等
- 但对 HTTP handler、路由行为、错误码语义、跨模块流程覆盖不足

缺口：

- `/api/config` patch 与持久化语义
- `/api/history/*` 参数校验
- `/api/remote/*` 开关、pair、cookie、redaction
- `/api/workflows/*` 错误码与状态流
- ingest -> workflow auto-link -> derived update 的集成链路

建议：

- 增加 server integration tests：
  - 起 Axum app，不必起真实端口
  - 使用 temp home / temp workflow dir / temp git sandbox
- 给 remote access、workflow、config、history 建 handler 级回归测试
- 为 `probe` 和 `commits` 加 fixture 数据驱动测试，减少只靠手写临时数据

目标：

- 风险最大的 API 和状态迁移路径可自动回归

### P3-3. 缺少明确的质量门禁分层

建议引入分层验证：

- 快速层：
  - web unit tests
  - rust unit/integration tests
  - `cargo clippy --workspace -- -D warnings`
- 中等层：
  - `vite build`
  - `cargo build --workspace`
- 重型层：
  - a11y
  - desktop 打包 smoke
  - remote viewer smoke

目标：

- 本地开发与发布校验分层清晰，不再把所有命令塞进一个总入口里

## P4：补充能力与工程卫生

### P4-1. 文档需要与代码重新对齐

现状：

- `docs/architecture-upgrade.md` 里有一些已经完成的事项，继续保留会误导后续开发
- `main.md`、`docs/design.md`、`workflow` 相关文档和当前实现存在不同程度漂移

建议：

- 将旧计划文档标注为“已完成/已过期/仍待做”
- 把 `docs/architecture-upgrade.md` 中已完成事项明确打勾或迁移到 changelog，避免后续审计继续把已完成项当缺口
- 新增一份当前架构基线文档
- 对 commit attribution、workflow、remote access 各写一份“现状设计 + 当前限制”

目标：

- 文档成为真实决策记录，而不是历史堆积物

### P4-2. 发布资产与仓库卫生需要收敛

现状：

- 仓库内可见生成物与打包产物痕迹
- 发行相关脚本、package 元信息、homebrew 目录都在增长

建议：

- 明确哪些产物应该进仓库，哪些只应出现在 release pipeline
- 为 packaging 增加自检清单：
  - sidecar 是否存在
  - 版本是否对齐
  - npm optionalDependencies 是否一致

目标：

- 降低发布时的人肉检查成本

### P4-3. Tauri 壳层桥接仍有少量工程卫生欠账

现状：

- `apps/desktop/src-tauri/src/main.rs` 仍通过两处 `window.eval()` 注入 boot status

问题：

- 功能正确，但属于壳层技术债
- 它不构成当前最高优先级的安全问题，不应与热路径、脱敏、错误语义抢排期

建议：

- 降级为后置工程卫生项
- 保留 HTTP/WS 作为业务主通道
- 等 runtime continuity、remote 边界和关键测试补齐后，再把 desktop-specific 状态改成壳层 event/bridge

目标：

- Tauri 壳层逻辑更“壳化”，但不抢占前面更关键的重构预算

## 4. 分阶段实施路线

## Phase 0：建立基线与观测能力

目标：

- 在重构前先看清性能与行为，不盲改

任务：

- 给 `refresh_bootstrap_once()`、`rebuild_derived()`、`build_commit_records()`、各 adapter `probe()` 加 `tracing` 耗时日志
- 给 ingest 路径加采样日志，确认高频来源
- 给 `BootstrapPayload` 大小加观测，建立 WS 体积基线
- 先做 Claude/Codex/OpenClaw 存储格式盘点，确认增量读取前提成立
- 建立当前测试基线：
  - 不使用过时的“18 个 Rust 测试”口径
  - 以当前 `cargo test --workspace` 与测试清单为准
- 加一个开发期性能开关，输出本次 probe/derive 的分项耗时
- 补一份“当前大文件/测试覆盖/验证入口”的工程基线文档

完成标准：

- 能回答“慢在哪里、多久慢一次、由谁触发”

## Phase 1：热路径重构与直接 correctness 修复

目标：

- 先把最危险的锁内重计算和最直接的安全/语义错误修掉

任务：

- 同一步完成热路径重构：
  - ingest 改成“写 runtime + 标记 dirty + 唤醒 worker”
  - derive worker 内部统一使用 `snapshot-then-swap`
  - 不保留“锁外但同步执行 derive”的交付中间态
- 同期完成最直接的 correctness/security 修复：
  - workflow `{{file:...}}` canonicalize 越界校验
  - remote allowlist projection 与 `recent_completions` 脱敏补齐
  - `save_config()` 返回 `Result`，配置修改失败不再假成功
- 先把 commit attribution 从 ingest 即时重算中移出

完成标准：

- ingest handler 不再直接触发 git 扫描
- 连续 10 次 hook 更新不会做 10 次 full derive
- 重型派生计算已经全部不在写锁内执行

## Phase 2：adapter 增量化、刷新隔离与连续性补强

目标：

- 降低 `probe.rs` 的刷新成本，并建立真正可控的刷新隔离

任务：

- commit dirty set 精确到 repo/worktree
- 为 Claude/Codex/OpenClaw 引入 transcript/session 索引缓存
- JSONL transcript 以 offset/cursor 增量补读
- 统一 `remote access` 与 `config.local_ip` 的地址发现实现，移除 `ifconfig`/`8.8.8.8` 依赖
- 保持 adapter `probe()` 同步签名，在 orchestration 层建立目标契约：
  - timeout
  - panic isolation
  - cache handle/state
  - health downgrade
- watcher 改成：
  - 支持目录不存在时延迟注册
  - 支持新增目录后补注册
- 每个 adapter probe 增加 timeout / panic 隔离 / health 降级
- 区分“热数据刷新”和“历史补全扫描”
- 将最近活跃 session 与历史 archive 分层读取
- 引入 runtime overlay snapshot/journal，作为 best-effort 的短 TTL 连续性补强

完成标准：

- 常规 probe 只读取增量变化文件
- 大量历史 transcript 不再显著拖慢刷新
- 单个 adapter 异常不会卡死整轮 refresh
- desktop 重启后可 best-effort 恢复近期 live overlay

## Phase 3：workflow 与接口语义硬化

目标：

- 让 workflow 与关键持久化接口真正变成可依赖子系统，而不是“功能能跑但语义不硬”

任务：

- 把 workflow typed errors 全面收口到各 handler
- `change_mode()` 重算 step state
- `link_run()` / `unlink_run()` 增加显式业务校验
- artifact identity 结构化
- id 生成改为时间可排序的强唯一 ID
- store 写入与索引恢复改成更稳的原子策略
- launcher 增加：
  - stderr 读取与持久化
  - 执行日志持久化
  - 更明确的 timeout/exit 分类

完成标准：

- workflow API 对非法状态转换给出正确错误码
- launch step 与 approval/auto mode 行为一致可预测

## Phase 4：前端拆分与数据层整理

目标：

- 降低前端复杂页面的认知负担

任务：

- 把 `App.tsx` 的 runtime-specific 逻辑拆到 hooks
- 保持实时数据只通过 WS + Zustand 进入 UI
- 拆 `MonitorView`、`HeatmapView`、`CommitsView`
- `InsightsSection` 的生成队列抽到专用 hook/store slice
- `i18n.tsx` 拆字典文件
- 清理跳过测试，补 workflows/remote/app 分支

完成标准：

- 页面文件长度显著下降
- 每个页面主要剩“组装 + 渲染”

## Phase 5：桌面壳与工程边界收口

目标：

- 让 desktop 壳、发布物与工程边界更干净，但不抢占核心正确性工作的优先级

任务：

- Tauri boot status 从 `window.eval()` 改为明确 bridge
- remote cookie/session/pairing 生命周期策略整理
- 审查所有可执行入口：
  - daily summary
  - workflow launch
  - installer
- 发布资产与旧文档清理

完成标准：

- desktop 壳层逻辑不再混入脚本注入残留
- 发布物和历史文档边界清晰

## Phase 6：测试、CI 与发布清理

目标：

- 把“稳定”从个人感觉变成可重复验证

任务：

- 增加 Rust integration tests
- 补前端复杂交互测试
- 恢复 CommitsView 测试
- CI 分层
- 发布资产校验脚本化
- 文档与现状对齐

完成标准：

- 本地与 CI 都能稳定复现核心验证链

## 5. 各模块具体改造建议

## `crates/server`

- `probe.rs` 先拆出 `scan` 与 `derive`，主文件保留 orchestration；第二轮再决定是否继续拆
- `commits.rs` 先拆 `scan` 与 `attribution`
- `remote_access.rs` 从 clone-and-null 改成 allowlist projection
- `network.rs` 与 `probe.rs::detect_local_ip()` 合并成统一地址发现模块，替换掉 `ifconfig` 解析与 `8.8.8.8` 推断
- `handlers/workflows.rs` 引入 typed error 返回
- `handlers/config.rs` / `handlers/remote.rs` 需要把配置落盘失败显式暴露给调用方
- `handlers/ingest.rs` 从“立即全量 derive”改为“轻写入 + 锁外派生 + 后台 worker”
- `watcher.rs` 需要支持缺失目录延迟接管与后续补注册
- 新增 runtime overlay snapshot/journal 模块，处理重启恢复
- `scan_adapters()` 需要 timeout/panic 隔离，不能让单个 adapter 卡死整轮 refresh

## `crates/adapters/*`

- Claude/Codex/OpenClaw 都要引入增量解析缓存
- JSONL transcript 统一支持 offset/cursor 模式
- OpenClaw 的 session file 二次全文读取应改成按需/增量
- 把 workflow context 读取抽成共享工具，避免重复实现

## `crates/core`

- 按领域逐轮拆文件，不一次性展开成大量小文件
- 保持 `ts-rs` 导出，但把模块组织整理清楚
- 保留 shared core types；remote 先走显式 projection，而不是完整复制一套类型树
- `core` 的最小拆分放在 workflow/type-heavy 改动前做，避免在大文件里继续叠加语义变更

## `apps/web`

- 新增 `src/hooks/`
- 新增 `src/features/monitor`, `src/features/heatmap`, `src/features/workflows` 分层
- `useBootstrapStream` 只封装 WS/Zustand 实时链路，历史/按需数据由 HTTP hooks 负责
- 把页面型组件和子组件彻底分离
- i18n 字典拆文件
- Settings 下的各 section 继续解耦，避免新的复杂逻辑继续塞回单文件

## `apps/desktop`

- 保留当前 sidecar 启动策略
- 把 boot issue 传递方式从脚本注入改成明确桥接
- 为 sidecar 查找与启动失败路径补测试

## 6. 测试补充清单

建议新增的测试，不是“有空再说”，而是本轮重构应同时补上的：

- Rust
- `ingest -> derived refresh scheduler` 行为测试
- `snapshot-then-swap` 不在写锁内执行重型派生的测试
- `refresh_bootstrap_once()` 不会用旧快照覆盖新 ingest 更新的并发测试
- `workflow change_mode` 状态重算测试
- `workflow store` 索引损坏后的重建测试
- `workflow prompt include` 越界拒绝测试
- `remote redaction` 针对 workflow/commit/origin/recent-completions 的脱敏测试
- `patch_config / patch_remote_access` 落盘失败错误测试
- runtime overlay dump/load/TTL 测试
- `BootstrapPayload` 体积预算监测测试/断言
- `network` 地址发现与分类测试
- `watcher` 目录延迟出现/新增目录测试
- adapter timeout/panic 隔离测试
- `handlers/workflows` 错误码测试
- `proptest`：
  - usage bucket 聚合前后 token/cost 守恒
  - commit attribution 分配不超总量
  - history trim 不会丢 pinned runs

- Web
- `CommitsView` 恢复并稳定化
- `WorkflowsView` 交互测试
- `StepDetail` 的 preview/candidates/link/unlink 测试
- `App` 在 `local / remoteViewer / tauri` 三种模式下的关键分支测试
- `RemotePairingGate`、`RemoteAccessSection` 测试
- `InspectDrawer` 时间线与错误状态测试

## 7. 明确不做的事

为防止这份方案把项目带偏，以下事情当前不建议做：

- 不引入数据库
- 不把 snapshot WebSocket 改成细粒度事件系统
- 不在没有指标证明之前就把 `bootstrap` 粗暴拆成多把锁
- 不为了 remote 安全一上来复制一整套完整 DTO 树，allowlist projection 足够前先不做
- 不引入微服务拆分
- 不上 Redux / React Query / TanStack Query 作为第一反应
- 不把 workflow 直接升级成 DAG 编排平台
- 不做通用 adapter/plugin marketplace
- 不为 remote viewer 做复杂账户体系
- 不在当前阶段为 commit attribution 引入持久 ledger，先把读路径与缓存路径做稳
- 不因为旧计划文档而重复改造已经落地的部分，例如 `broadcast` 驱动 WS、Tailwind、`ErrorBoundary`、`SettingsView` 拆分、并发 adapter probe

## 8. 推荐执行顺序

如果按实际收益排序，我建议按这个顺序推进：

1. Phase 0：加耗时观测与基线
2. Phase 1：热路径重构与直接 correctness/security 修复
3. Phase 2：adapter 增量缓存 + 隔离 + 连续性补强
4. Phase 3：workflow 语义与接口硬化
5. Phase 4：前端大组件拆分与测试补齐
6. Phase 5：desktop/发布/工程边界清理
7. Phase 6：CI、文档、发布流程收尾

如果按“必须先做”与“可以第二批处理”再压一层，建议是：

- 下一轮 feature 开发前必须先做：
  - Phase 0 的耗时基线
  - P0-1：revision + dirty marker + derive worker 热路径改造
  - P0-3 workflow include 越界修复
  - P0-4 remote allowlist projection / redaction 收口
  - P0-5 typed error 与配置落盘失败显式化
- 可以放到第二批，但不应无限期拖延：
  - adapter timeout/isolation + 增量缓存
  - runtime overlay 重启恢复
  - 大文件拆分的长尾
  - `window.eval()` 到 bridge 的替换
  - i18n/页面层进一步拆分
  - 发布资产和历史文档清理

## 9. 完成标志

当以下条件满足时，可以认为这一轮重构达标：

- 实时 ingest 不再触发 commit 全量扫描
- 重型派生计算不再在 `RwLock<BootstrapPayload>` 写锁内执行
- probe/derived 耗时可观测，且在历史规模扩大时仍可控
- adapter 不再每轮全量重读所有 transcript
- JSONL transcript 增量读取基于 offset/cursor，而不是反复整文件重读
- workflow 文件注入不能越界读取工作目录外文件
- remote viewer 走显式 allowlist projection，脱敏策略有测试
- workflow API 错误码语义清晰
- `patch_config / patch_remote_access` 不再把未落盘修改报成成功
- desktop 重启后可 best-effort 恢复近期 runtime overlay
- `probe.rs / commits.rs / MonitorView / HeatmapView / i18n.tsx` 完成职责拆分
- CommitsView 测试恢复，workflow/remote/app 关键路径补上测试
- Rust 与前端验证链能稳定在 CI 中复现

## 10. 当前版本的简短结论

OctoMonitor 目前最不缺的是功能，最需要的是“把已经有的能力做成长期可承载的系统”。三次复核后，这个结论更明确了：这个项目不需要换栈，也不需要为显得完整而再造一层框架，真正要处理的是热点路径、脱敏模型、错误语义、增量扫描和状态连续性。重构不应该追求更多 feature，而应该聚焦三件事：

- 让实时链路变轻
- 让模块边界变清
- 让测试与安全边界跟上产品能力面

只要这三件事做对，当前仓库完全有机会从“稳定的个人/小范围产品”升级为“可长期维护、可持续发布、可放心扩展”的项目。

## 11. 对 Opus 审阅意见的逐条复核

以下结论是在重新对照当前代码后给出的，不是机械采纳：

1. `P0-1` 缺少 mini design：采纳。
   已补明确决策：derive worker 内部统一采用 `snapshot-then-swap`，但交付上直接进入“写 runtime + 标记 dirty + 唤醒 worker”模式；先不拆双锁；dirty 粒度与刷新上限也已明确。

2. `P0-2` 的 `mtime + size` 缓存设计不适合 append-only JSONL：采纳。
   文档已改为：JSONL 用 `offset/cursor` 增量补读，`mtime + size` 只用于非 append-only 文件或截断检测。

3. `P0-4` remote DTO 独立化过重：采纳，但不是退回现状。
   我没有回到“clone 后抹字段”，而是改成“显式 allowlist 投影构造”。这样既避免完整复制一套 DTO，又能保证新增字段必须显式决定是否暴露。

4. `P1-1` 拆分粒度过细：采纳。
   文档已改成“每轮只拆 2-3 个子模块”，先拆 `probe/scan + probe/derive`、`commits/scan + commits/attribution` 这类天然边界，避免一次性制造过大的 diff。

5. `P1-2` 直接改 UUID 的必要性不充分：部分采纳。
   我不再建议换成普通 `uuid v4`。但当前 `generate_id()` 的“随机后缀”实际上由时间戳推导，同一毫秒内存在确定性碰撞，因此仍需要升级成“时间可排序且真正唯一”的 ID。

6. `window.eval()` 优先级被高估：采纳。
   它已从高优先级语义中降为后置工程卫生项，移动到 `P4-3 / Phase 5` 处理。

7. 安全修复分期倒挂：采纳。
   `workflow` 路径逃逸、remote 脱敏补齐、配置落盘失败显式化，现在都前移到了 `Phase 1`，不再拖到后面。

8. 缺少对锁持有时间的具体分析：采纳。
   文档已明确“两步走”：先把重型派生搬到锁外，再做异步调度；不再只写“解耦”这种空话。

9. hooks 抽取与 WS 推送模式关系不清：采纳。
   文档已明确：实时数据只走 `WS + Zustand`，`useBootstrapStream` 只是这条链路的封装；历史/按需数据才走 HTTP hooks。

10. 缺少 graceful shutdown / state continuity：采纳。
    新增了 `runtime overlay` 持久化与恢复方案，但只覆盖内存态 live overlay，不扩成数据库或全量 state persistence。

11. 缺少 adapter probe 错误隔离：采纳。
    文档已补 `timeout / panic 隔离 / health 降级`，并明确指出当前“panic 后 inline 重跑 probe”不是合理兜底。

12. 测试策略缺少 property-based testing：部分采纳。
    不会把整个测试体系改成 property-based，但会对 `usage bucket` 守恒、`commit attribution` 分配上限、`history trim` 不丢 pinned runs 这类纯函数不变量引入 `proptest`。

这轮复核后的方案比上一版更收敛：更强调先做锁外计算、显式 allowlist、时间可排序强唯一 ID、runtime continuity 和 probe isolation；同时也明确避免了几类不必要复杂化的路径，例如过早拆多把锁、完整复制 remote DTO 树、一次性细碎拆文件。

## 12. 对最新审阅意见的逐条复核

1. `P0-1` 对 stale overwrite 的分析不完整：采纳。
   方案现已明确把刷新链路拆成 `raw scan / authoritative runtime state / derived state` 三层。revision 冲突时不再“整包丢弃 probe 结果”，而是复用已经完成的 raw scan，只重放 merge/derive。

2. `P0-2` 缺少 Claude/Codex/OpenClaw 实际文件格式分析：采纳。
   已把“存储格式盘点”前移到基线阶段，不再把 offset 策略当作默认真理。

3. allowlist projection 的 compile-time exhaustiveness 承诺过于笼统：采纳。
   现在已明确实现路径是“逐层 projection 函数 + 不带 `..` 的完整 struct literal”，并承认这个保证只覆盖到显式投影的层级。

4. workflow store 的 `atomic_write()` 问题描述不准确：采纳。
   现在不再把重点放在“rename 不原子”，而是收敛到轻量 durability：`tmp sync_all + rename + best-effort dir sync`，同时把重心放回索引损坏重建。

5. runtime overlay 设计与 no-DB 原则有张力：采纳，但做了边界收窄。
   文档现在把 overlay 定义成 best-effort 的短 TTL runtime source，不是 mini database，也不承诺恢复保证；默认 TTL 也收敛到分钟级。

6. 方案遗漏全量 WS 快照的带宽预算：采纳。
   已新增 `BootstrapPayload` 体积预算与观测要求，明确超预算时优先做 server-side 裁剪，而不是改协议。

7. 方案遗漏 adapter orchestration 的同步→异步迁移风险：采纳。
   现在已明确：保持 adapter `probe()` 同步签名，不先改 adapter API；timeout 与隔离通过 orchestration 层的 `spawn_blocking + timeout` 实现。

8. 实施计划中 Step 2/3 边界模糊：采纳，并体现在计划文档重排上。
   计划已改成在热路径重构时直接进入“写 runtime + 标记 dirty + 唤醒 worker”模式，不再保留“锁外但同步执行 derive”的中间态。

9. Step 4/5 缺少 adapter 目标签名前置设计：采纳。
   现在已把“adapter orchestration contract 设计”前移，先锁定 timeout/cache/health 的外层契约，再填缓存逻辑。

10. Step 1 粒度过大：采纳。
    实施计划已拆成 `Step 1a` 与 `Step 1b`，把最快可落地的 correctness 修复与 remote projection 重写分开。

11. Gate 缺少失败处理策略：采纳。
    实施计划现在会显式规定 Gate 不通过时的处理：优先在当前 Step 修复并重验；若根因来自更早步骤，再回退到对应 Step。

12. 缺少现有 Rust 测试基线保护：采纳。
    实施计划已补“测试基线记录”任务，并要求每一步退出条件都包含“当前已通过测试继续通过”。

13. Step 6 把 overlay 和地址发现捆绑：采纳。
    实施计划已拆成 `Step 5a / 5b`，允许并行或任意顺序推进。

14. 缺少 `core/src/lib.rs` 拆分时机：采纳。
    现在明确为 workflow/type-heavy 修改前的最小拆分前置任务，而不是随缘插入。

15. 两份文档 Phase/Step 映射不显式、路径逃逸时机等表述易混淆：采纳。
    已在实施计划中补 Phase↔Step 映射，方案与计划的时序表述也已经对齐。

16. launcher 的 `stderr capture` 表述不准确：采纳。
    方案已改成“stderr 读取与持久化”，与当前代码“已 `piped` 但未消费/落盘”的事实对齐。
