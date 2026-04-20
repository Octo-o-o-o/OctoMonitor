# Hermes Agent Adapter 设计方案

> 2026-04-16 更新：本设计文档早于 2026-04-15 的项目收敛方案。文中如果出现 workflow 集成点，应以当前实现为准；现有产品已经移除 workflow 子系统，Hermes 仅保留监控适配器角色。
> 状态：历史设计记录。Hermes 现已接入当前实现，这份文档保留作设计背景，不再作为实施说明。

## 1. 背景

Hermes Agent (Nous Research) 是一个类似 OpenClaw 的自托管 AI Agent 框架，支持：
- CLI 交互 (TUI)
- 消息网关 (Telegram/Discord/Slack/WeChat 等多平台)
- FastAPI Web UI (默认端口 9119)
- Profile 系统 (多实例隔离)

### 1.1 Hermes 数据存储结构

```
~/.hermes/                          # HERMES_HOME (可通过环境变量覆盖)
├── config.yaml                     # 主配置
├── .env                            # API 密钥 (0600)
├── SOUL.md                         # Agent 人设
├── gateway.pid                     # Gateway 进程 PID
├── gateway_state.json              # Gateway 运行状态
├── sessions/
│   └── sessions.json               # Session 索引 (key → session_id + metadata)
├── state.db                        # SQLite (FTS5 全文搜索)
├── cron/                           # 定时任务
├── logs/
│   ├── agent.log
│   └── errors.log
├── memories/
├── skills/
└── profiles/                       # 多实例 Profile
    ├── <profile-name>/             # 每个 profile 是完整的 HERMES_HOME
    │   ├── config.yaml
    │   ├── .env
    │   ├── sessions/sessions.json
    │   ├── gateway.pid
    │   ├── gateway_state.json
    │   ├── state.db
    │   ├── cron/
    │   └── ...
    └── ...
```

### 1.2 Session 数据格式

**sessions/sessions.json** (类似 OpenClaw，JSON dict):
```json
{
  "local:cli:user": {
    "session_key": "local:cli:user",
    "session_id": "uuid-xxx",
    "created_at": "2026-04-12T10:00:00",
    "updated_at": "2026-04-12T10:30:00",
    "display_name": "CLI User",
    "platform": "local",
    "chat_type": "dm",
    "input_tokens": 5000,
    "output_tokens": 3000,
    "cache_read_tokens": 1000,
    "cache_write_tokens": 500,
    "total_tokens": 9500,
    "estimated_cost_usd": 0.15,
    "origin": {
      "platform": "telegram",
      "chat_id": "12345",
      "chat_name": "Yixiao",
      "user_name": "yixiao"
    }
  }
}
```

**state.db** (SQLite):
- `sessions` 表: id, source, model, title, started_at, ended_at, parent_session_id 等
- `messages` 表: session_id, role, content, timestamp, tool_name 等
- FTS5 索引用于全文搜索

### 1.3 多实例支持

**Hermes Profile 系统**:
- 默认实例: `~/.hermes/` 本身
- 命名 Profile: `~/.hermes/profiles/<name>/`
- 每个 Profile 有独立的 `gateway.pid`、`sessions/`、`config.yaml`
- 通过 `hermes -p <name>` 或 `HERMES_HOME` 环境变量切换

**OpenClaw 多实例**:
- OpenClaw 使用 agents 目录: `~/.openclaw/agents/<agent-name>/`
- 每个 agent 有独立的 sessions 目录
- 当前 adapter 已经扫描所有 agents 子目录，天然支持多 agent

### 1.4 Gateway 检测

- PID 文件: `{HERMES_HOME}/gateway.pid`
- 状态文件: `{HERMES_HOME}/gateway_state.json`
  ```json
  {
    "gateway_state": "running",
    "platforms": {"telegram": {"status": "connected"}, ...},
    "updated_at": "2026-04-12T10:00:00Z"
  }
  ```
- Web API: `http://127.0.0.1:9119/api/status` (可选，不依赖)

---

## 2. 设计目标

1. **一致性**: 与现有 Claude/Codex/OpenClaw adapter 保持相同的架构模式
2. **多实例自动发现**: 自动扫描默认目录和所有 Profile 子目录
3. **增量探测**: 使用 `JsonlCursor` + 文件签名缓存，避免重复解析
4. **安全**: 不暴露 secrets，遵循 read-only 原则
5. **最小侵入**: 只读文件系统数据，不依赖 Hermes API

---

## 3. 技术方案

### 3.1 新增 Adapter Crate

```
crates/adapters/hermes/
├── Cargo.toml
└── src/
    └── lib.rs
```

**核心类型**:

```rust
pub struct HermesSession {
    pub session_id: String,
    pub session_key: String,
    pub profile_name: String,           // "default" 或 profile 名称
    pub display_name: Option<String>,
    pub platform: Option<String>,       // "local", "telegram", "discord" 等
    pub chat_type: String,              // "dm", "group", "channel"
    pub model: Option<String>,
    pub started_at: Option<String>,     // ISO 8601
    pub updated_at: Option<String>,     // ISO 8601
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: Option<f64>,
    pub first_question: Option<String>,
    pub last_question: Option<String>,
    pub message_count: u64,
    pub error_message: Option<String>,
    pub origin_label: Option<String>,   // "Telegram: Yixiao"
    pub origin_provider: Option<String>,// "telegram", "local", "cron"
}

pub struct HermesCronJob {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub profile_name: String,
    pub schedule_expr: String,
    pub schedule_tz: String,
    pub schedule_human: String,
}

pub struct HermesSnapshot {
    pub probed_at: String,
    pub cli_available: bool,
    pub gateway_running: bool,
    pub cli_version: Option<String>,
    pub instances: Vec<HermesInstance>,
    pub sessions: Vec<HermesSession>,
    pub cron_jobs: Vec<HermesCronJob>,
    pub command_probes: Vec<CommandProbeResult>,
    pub file_probes: Vec<FileProbeResult>,
}

pub struct HermesInstance {
    pub profile_name: String,
    pub home_dir: String,
    pub gateway_running: bool,
    pub gateway_state: Option<String>,  // "running", "stopped" 等
    pub gateway_platforms: Vec<String>, // 已连接平台列表
    pub config_exists: bool,
    pub session_count: usize,
}
```

### 3.2 多实例自动发现策略

扫描逻辑:

1. **默认实例**: 扫描 `~/.hermes/` (作为 profile_name = "default")
2. **Profile 实例**: 扫描 `~/.hermes/profiles/*/` 目录下所有子目录
3. **每个实例独立处理**: 读取 `sessions/sessions.json`、`gateway.pid`、`config.yaml`

```rust
fn discover_hermes_instances() -> Vec<(String, PathBuf)> {
    let root = resolve_home_dir(".hermes");
    let mut instances = vec![];
    
    // 默认实例
    if root.join("config.yaml").exists() || root.join("sessions").is_dir() {
        instances.push(("default".to_string(), root.clone()));
    }
    
    // Profile 实例
    let profiles_dir = root.join("profiles");
    if profiles_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&profiles_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    instances.push((name, entry.path()));
                }
            }
        }
    }
    
    instances
}
```

### 3.3 Session 解析

从 `sessions/sessions.json` 解析 (与 OpenClaw 模式一致):

```rust
fn parse_hermes_sessions(
    sessions_json: &Path,
    profile_name: &str,
) -> Option<Vec<HermesSession>> {
    let contents = fs::read_to_string(sessions_json).ok()?;
    let val: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let obj = val.as_object()?;
    
    let mut sessions = Vec::new();
    for (session_key, entry) in obj {
        let session_id = entry.get("session_id")?.as_str()?.to_string();
        // ... 解析字段
        sessions.push(HermesSession { ... });
    }
    Some(sessions)
}
```

### 3.4 Gateway 状态检测

```rust
fn check_gateway_status(home: &Path) -> (bool, Option<String>, Vec<String>) {
    // 1. 检查 PID 文件
    let pid_file = home.join("gateway.pid");
    let pid_exists = pid_file.exists();
    
    // 2. 读取 gateway_state.json
    let state_file = home.join("gateway_state.json");
    let (state, platforms) = if let Ok(contents) = fs::read_to_string(&state_file) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&contents) {
            let state = val.get("gateway_state").and_then(|v| v.as_str()).map(String::from);
            let platforms = val.get("platforms")
                .and_then(|v| v.as_object())
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            (state, platforms)
        } else {
            (None, vec![])
        }
    } else {
        (None, vec![])
    };
    
    // gateway 运行 = PID 文件存在 + state 不是 "stopped"
    let running = pid_exists && state.as_deref() != Some("stopped");
    (running, state, platforms)
}
```

### 3.5 Cron 任务扫描

Hermes 的 cron 结构与 OpenClaw 类似，存储在 `{HERMES_HOME}/cron/` 目录。
需要进一步确认 Hermes cron 文件格式后实现。如果 Hermes 目前没有 cron 数据或格式不同，先置空。

### 3.6 增量缓存

```rust
pub struct HermesProbeCache {
    // 按 profile_name 缓存 sessions.json 文件签名
    session_lists: HashMap<String, CachedSessionsFile>,
}
```

注意: Hermes 使用 SQLite (`state.db`) 存储 session 详情和消息，但:
- 读取 SQLite 需要额外依赖且可能锁冲突
- `sessions/sessions.json` 包含足够的元数据用于监控
- 与 OpenClaw 保持一致，优先使用 JSON 文件

### 3.7 ToolKind 扩展

在 `crates/core/src/lib.rs` 中添加:

```rust
pub enum ToolKind {
    Claude,
    Codex,
    OpenClaw,
    Hermes,  // 新增
}
```

### 3.8 Server 集成

1. **state.rs**: 添加 `hermes_probe_cache` 字段
2. **probe.rs**:
   - `scan_adapters_isolated()`: 添加 hermes probe (4路并行)
   - `collect_probe_scan_from_snapshots()`: 处理 hermes sessions
   - 添加 `build_run_from_hermes_session()` 和 `build_probe_run_from_hermes()`
   - 添加 hermes identity 和 adapter_health
   - 将 hermes cron_jobs 合入 pending_crons
3. **installer**: `detect_tools()` 添加 hermes 检测

### 3.9 RunRecord 映射

```rust
fn build_run_from_hermes_session(session, probe) -> RunRecord {
    RunRecord {
        id: format!("hermes-{}-{}", session.profile_name, session.session_id),
        tool: ToolKind::Hermes,
        source_mode: "hermes_sessions",
        project_name: // session display_name 或 origin_label 或 profile_name
        workspace_path: // hermes home dir
        agent_name: Some(session.profile_name),  // profile 名作为 agent 名
        origin_label: // 从 origin 构建 "Telegram: xxx"
        origin_provider: // "telegram", "local" 等
        // tokens, cost 等直接映射
    }
}
```

---

## 4. 不实现的部分 (避免过度设计)

1. **不读取 SQLite state.db**: 避免引入 sqlite 依赖和锁冲突，sessions.json 足够
2. **不调用 Hermes Web API**: 保持 read-only 文件扫描，不依赖 Hermes 服务运行
3. **不添加 ingest 端点**: Hermes 没有 hook 机制向外推送事件，后续按需添加
4. **不解析 JSONL transcript**: Hermes 使用 SQLite 存消息而非 JSONL 文件，sessions.json 中的 token 计数已足够。无需像 OpenClaw 那样解析 transcript
5. **不支持 Docker/SSH 远程实例**: 只扫描本机文件系统

---

## 5. OpenClaw 多实例增强

当前 OpenClaw adapter 已扫描 `~/.openclaw/agents/*/sessions/sessions.json`。
如果用户部署多个独立 OpenClaw 实例 (不同 HERMES_HOME)，需确认 OpenClaw 是否也有类似 profile 机制。

基于研究: OpenClaw 的多实例是通过多个 agent 实现的，都在 `~/.openclaw/agents/` 下，当前 adapter 已支持。
如果用户通过不同环境变量运行多个完全独立的 OpenClaw 实例，则超出当前范围 (需要用户配置额外扫描路径)。

---

## 6. 变更影响范围

| 文件/Crate | 变更类型 | 说明 |
|---|---|---|
| `Cargo.toml` (workspace) | 修改 | 添加 hermes adapter member |
| `crates/adapters/hermes/` | 新建 | Adapter 实现 |
| `crates/core/src/lib.rs` | 修改 | ToolKind 添加 Hermes |
| `crates/server/Cargo.toml` | 修改 | 依赖 hermes adapter |
| `crates/server/src/state.rs` | 修改 | 添加 hermes_probe_cache |
| `crates/server/src/probe.rs` | 修改 | 集成 hermes probe |
| `crates/server/src/main.rs` | 不变 | 无需新路由 |
| `crates/installer/src/lib.rs` | 修改 | 添加 hermes 检测 |
| workflow 子系统 | 无需变更 | 2026-04-15 收敛后已删除，不再为 Hermes 添加 workflow 分支 |
| `apps/web/src/lib/constants.ts` | 修改 | allTools, sourceLabels 添加 hermes |
| `apps/web/src/lib/monitor.ts` | 修改 | sessionsBySource 初始化添加 hermes |
| `apps/web/src/components/monitor/MonitorView.tsx` | 修改 | sourceAccents 添加 hermes，显示逻辑适配 |
| `apps/web/src/components/monitor/UsageView.tsx` | 修改 | sourceOrder, barColors 添加 hermes |
| `apps/web/src/app.css` (或类似) | 修改 | 添加 accent-hermes CSS 类 |
| `crates/core/bindings/ToolKind.ts` | 自动生成 | ts-rs 自动更新 |
