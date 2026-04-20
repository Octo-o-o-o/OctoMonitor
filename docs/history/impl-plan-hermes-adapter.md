# Hermes Agent Adapter 实施计划

> 2026-04-16 更新：本文件中的 workflow 相关步骤已失效。`Workflows` 产品线已在简化方案中删除，当前 Hermes 只需要接入 adapter / probe / monitor / usage / installer 这条主线。
> 状态：历史实施记录。Hermes 适配器已经落地，当前仓库不再按这份分阶段计划推进。

## Phase 1: Core 层 ToolKind 扩展

**修改文件:**
- `crates/core/src/lib.rs` — ToolKind enum 添加 `Hermes` variant

**验证:**
- `cargo build --workspace` 将产生编译错误，列出所有需要处理的 match 分支
- 这些错误即为 Phase 3 的修改清单

**提交:** Phase 1 完成后提交

---

## Phase 2: Hermes Adapter Crate

**新建文件:**
- `crates/adapters/hermes/Cargo.toml`
- `crates/adapters/hermes/src/lib.rs`

**修改文件:**
- `Cargo.toml` (workspace) — members 添加 `crates/adapters/hermes`

**实现内容:**
1. `descriptor()` → AdapterDescriptor
2. `HermesSession` struct (字段映射 sessions.json)
3. `HermesCronJob` struct  
4. `HermesInstance` struct (profile 元数据)
5. `HermesSnapshot` struct
6. `HermesProbeCache` struct + Default impl
7. `discover_hermes_instances()` — 扫描默认目录 + profiles/
8. `parse_hermes_sessions()` — 解析 sessions/sessions.json
9. `check_gateway_status()` — 读取 gateway.pid + gateway_state.json
10. `probe()` 和 `probe_with_cache()` — 主探测入口
11. 单元测试

**验证:**
- `cargo test -p octomonitor-hermes-adapter`
- `cargo build -p octomonitor-hermes-adapter`

**提交:** Phase 2 完成后提交

---

## Phase 3: Server 集成

**修改文件:**

### 3.1 state.rs
- 添加 `hermes_probe_cache: Arc<StdMutex<HermesProbeCache>>`
- `AppState::new()` 初始化 hermes cache

### 3.2 probe.rs
- import `octomonitor_hermes_adapter as hermes_adapter`
- `failed_hermes_snapshot()` fallback 函数
- `scan_adapters_isolated()` — 4 路并行 (加入 hermes)，返回值改为 4-tuple
- `scan_adapters_blocking()` — 同上
- `collect_probe_scan_from_snapshots()` — 参数增加 hermes snapshot
  - 添加 hermes sessions → runs 转换
  - 添加 hermes identity
  - 添加 hermes adapter_health
  - 将 hermes cron_jobs 合入 pending_crons
- `build_run_from_hermes_session()` — session → RunRecord 映射
- `build_probe_run_from_hermes()` — placeholder run
- `normalized_total_tokens()` — match 添加 Hermes (同 OpenClaw 逻辑)
- `tool_key()` 辅助函数 — 添加 Hermes → "hermes" 映射
- `collect_probe_scan()` 和 `collect_probe_scan_isolated()` — 参数适配

### 3.3 handlers/inspect.rs
- `load_run_entries()` match — 添加 `ToolKind::Hermes`
  - Hermes 不使用 JSONL transcript，返回空 Vec 即可

### 3.4 pricing.rs
- `normalized_total_tokens()` 和相关 match — 添加 Hermes 分支

### 3.5 server Cargo.toml
- 添加 `octomonitor-hermes-adapter` 依赖

**验证:**
- `cargo build --workspace` 无错误
- `cargo test --workspace` 全部通过

**提交:** Phase 3 完成后提交

---

## Phase 4: Installer 检测

**修改文件:**
- `crates/installer/src/lib.rs`
  - `detect_tools()` — 添加 hermes CLI 检测
  - `doctor_report()` — 添加 hermes 诊断信息
  - 2026-04-16 起不再包含安装/回滚写入能力，installer 只保留 detect/doctor

**验证:**
- `cargo test -p octomonitor-installer`

**提交:** Phase 4 完成后提交

---

## Phase 5: 前端适配

**修改文件:**

### 5.1 TS 类型更新
- 运行 `cargo test` 触发 ts-rs 生成 `crates/core/bindings/ToolKind.ts`
- 确认类型文件已包含 `"hermes"`

### 5.2 constants.ts
- `allTools` — 添加 `'hermes'`
- `sourceLabels` — 添加 `hermes: 'Hermes'`
- `sourceLabelsUpper` — 添加 `hermes: 'HERMES'`

### 5.3 monitor.ts
- `groupRunsBySource()` — sessionsBySource 初始化添加 `hermes: []`

### 5.4 MonitorView.tsx
- `sourceAccents` — 添加 `hermes: 'accent-hermes'`
- Hermes 的 origin/platform 显示逻辑 (类似 OpenClaw)
- sessionsBySource 初始化处添加 hermes

### 5.5 UsageView.tsx
- `sourceOrder` — 添加 `'hermes'`
- `sourceTagLabels` — 添加 `hermes: 'Hermes'`
- `barColors` — 添加 hermes 颜色
- `accentClass()` — 添加 hermes case
- 分组初始化添加 hermes

### 5.6 CSS
- 添加 `accent-hermes` 相关样式 (参考 accent-openclaw)

### 5.7 preferences.ts / FilterSection.tsx
- FilterRules 初始化添加 hermes

**验证:**
- `pnpm --filter @octomonitor/web build` 无错误
- `pnpm --filter @octomonitor/web test --run` 通过

**提交:** Phase 5 完成后提交

---

## Phase 6: 端到端验证

1. `cargo test --workspace` — 全部通过
2. `pnpm --filter @octomonitor/web build` — 构建成功
3. `cargo run -p octomonitor-server` — 服务启动，`/api/bootstrap` 返回 hermes adapter_health
4. 检查 `~/.hermes/` 目录存在时能正确扫描 sessions

---

## 注意事项

- ToolKind 是 serde+ts-rs 导出的枚举，添加新 variant 后 Rust 编译器会强制检查所有 match 分支
- 不需要新的 API 路由 (hermes 数据通过 bootstrap 返回)
- 不需要 ingest 端点 (hermes 没有 hook 机制)
- sessions.json 格式与 OpenClaw 非常相似，可大量复用模式
- Hermes 的 cron 系统在当前本地部署可能没有 cron 数据文件，先实现扫描逻辑，无文件时返回空
