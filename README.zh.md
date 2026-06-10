# OctoMonitor

[English](README.md) · [文档索引](docs/README.md)

**面向 Claude Code、Codex、OpenClaw 与 Hermes（实验性）的本地优先统一监控面板。**

OctoMonitor 用一个统一仪表盘实时查看你的 AI 编码会话状态，包括 token 用量、配额、成本和会话状态，数据默认只保留在本机，不上传云端。

## 功能

- **统一本地监控器**：聚焦 `Monitor / Usage / Commits / Heatmap / Settings`
- **实时 WebSocket 更新**：事件驱动推送，无需轮询
- **Token 与成本跟踪**：支持按会话和聚合维度查看使用量
- **Commit 与热力图视图**：保留实用历史观察，不再扩张成独立分析产品
- **桌面通知**：当会话需要批准时给出提醒
- **键盘优先**：支持应用内快捷键，以及桌面端原生的设置、缩放和标准编辑操作
- **多主题**：支持深色、浅色、电子墨水风格，以及 VS Code 主题导入
- **本地优先**：服务端默认只绑定 `127.0.0.1`
- **零配置**：自动探测已安装工具，无需数据库
- **远程只读查看**：可选开启配对后的局域网 / 私网 companion，仅开放 `Monitor / Usage`
- **Hermes 适配器（实验性）**：继续接入监控与用量链路，但不再作为主线扩张
- **中英文 i18n**：编译期校验翻译完整性

## 集成支持状态

以下结论基于 2026-06-10 对官方文档和上游 GitHub 源码的复核。详细依据见 [docs/integration-support-audit-2026-06-10.md](docs/integration-support-audit-2026-06-10.md)。

| CLI | 级别 | OctoMonitor 当前能力 |
|-----|------|----------------------|
| Claude Code | 已监控 | 通过本地 transcript 扫描以及 statusline/hook ingest 统计和展示会话、token、成本、状态、工作区和详情；有 session id 时可复制 resume 命令，但不会启用变更型 operation bridge。 |
| Codex | 已监控 | 统计和展示本地会话、token、状态、Codex 事件时间线、resume 命令，以及有 thread id 时的桌面深链；Codex hooks 现在以 `[features].hooks` 作为官方配置键。 |
| OpenClaw | 已监控 | 通过现有 adapter 统计和展示 Gateway/session-store 状态、用量、会话和健康状态；操作保持只读。 |
| Hermes | 实验性 | 从本地状态和 profile-aware 扫描中统计、展示 Hermes CLI/Gateway 会话；有 profile/session 元数据时可复制 resume 命令，但 adapter 仍保持实验性。 |
| Gemini CLI / CodeBuddy / Pi Agent | 实验性，fixture-gated | 当对应本地存储存在且符合已锁定 fixture schema 时，可通过被动扫描统计会话与用量。Hook Manager 支持 Gemini 和 CodeBuddy 的显式 opt-in 安装。这些 adapter 不是 stable，也不得读取 OAuth token 或 provider secrets。 |
| opencode / GitHub Copilot / OpenHands / Continue cn / Qwen Code / Kimi Code / Goose | 实验性，fixture-gated | 对已知本地存储做被动扫描，展示 source health、usage semantics 和安全的复制/打开能力；不会启用 approve/deny/kill/send 等变更操作。 |
| Cursor Agent | 实验性 opt-in | 只有设置 `OCTOMONITOR_CURSOR_PRIVATE_STORE=1` 后才读取 private store；可展示会话元数据，但上游本地存储不包含 token/cost，因此用量为 `N/A`。 |
| Cline / Kiro | 实验性元数据 | 仅 fixture-gated metadata/custom-storage 解析；除非明确存在 usage 字段，否则用量保持 `N/A`，也不会启用 operation bridge。 |
| WorkBuddy / Amazon Q / Aider / Amp / Windsurf / Codebuff / Roo / Kilo | detection-only / watchlist | 仅用于 source control 和后续研究；不是 stable monitored source，也不会贡献 usage，除非未来 adapter 带 fixture 验证后升级。 |

## 安装

如果你是最终使用者，不想从源码启动，可以直接使用分发包：

- **macOS 桌面版**：从 [GitHub Releases](https://github.com/Octo-o-o-o/OctoMonitor/releases) 下载已公证的 `.dmg`
- **Homebrew 服务 / 本地服务端**：`brew install Octo-o-o-o/octomonitor/octomonitor`
- **npm 包**：安装命令为 `npm install -g octomonitor` 或 `npx octomonitor`

Homebrew 和 npm 分发会安装一个 `octomonitor` 命令，底层由本地服务端二进制驱动；npm 发行计划覆盖 macOS、Linux x64 和 Windows x64，目前仅 `octomonitor-darwin-arm64` 已发布到 npm，其余平台 binary 会在 v0.1.6 release workflow 跑通后补齐。桌面 `.dmg` 会单独通过 GitHub Releases 分发，当前仍仅提供 macOS 桌面包。如果你后续想使用短命令，可以先执行 `brew tap Octo-o-o-o/octomonitor`，再运行 `brew install octomonitor`。

## 快速开始

### 环境要求

- [Rust](https://rustup.rs/) 1.75+
- [Node.js](https://nodejs.org/) 20+，并安装 [pnpm](https://pnpm.io/) 10+
- 至少安装一个已监控或实验性来源：[Claude Code](https://docs.anthropic.com/en/docs/claude-code)、[Codex](https://github.com/openai/codex)、[OpenClaw](https://github.com/openclaw)、Hermes（实验性），或上表中的 fixture-gated 被动 adapter。

### 运行 Web 版

```bash
git clone https://github.com/Octo-o-o-o/OctoMonitor.git
cd OctoMonitor

pnpm install

# 启动本地服务（端口 46321）
cargo run -p octomonitor-server

# 另开一个终端启动 Web UI
pnpm --filter @octomonitor/web dev
```

打开 [http://127.0.0.1:4173](http://127.0.0.1:4173)。前端会通过 WebSocket 连接本地服务；如果服务未启动，界面会显示离线状态，而不是伪造 demo 数据。

### 运行桌面版

```bash
# 本地构建 unsigned 桌面包
pnpm build:desktop
```

如果当前机器已配置好 Developer ID 证书，可以构建只签名的 macOS 包：

```bash
pnpm build:desktop:signed
```

如果要构建用于正式发布的公证版 macOS 包：

```bash
pnpm build:desktop:notarized
```

`pnpm build:desktop:notarized` 需要在 shell 环境里提供 `APPLE_ID`、`APPLE_TEAM_ID`，以及 `APPLE_PASSWORD` 或 `APPLE_APP_SPECIFIC_PASSWORD`。命令会完成签名、`notarytool` 提交、公证通过后的 `staple`，并把最终产物放在 `target/release/bundle/` 下。

开发模式可以直接运行：

```bash
cargo tauri dev
```

## 远程只读 Viewer

OctoMonitor 的完整管理面始终留在 `127.0.0.1:46321`。远程访问默认关闭，只有在你显式开启后才会启动单独的只读 viewer。

1. 在本机应用或 localhost Web UI 中打开 `Settings -> Remote Access`。
2. 开启远程访问后，OctoMonitor 会在 `46322` 端口启动配对 viewer，并显示局域网 / 私网可访问地址。
3. 生成配对码，在另一台设备上打开上述地址并输入配对码。
4. 已配对 viewer 只开放 `Monitor / Usage`，并通过短期 cookie 会话访问。

## 架构

```text
OctoMonitor
├── crates/
│   ├── core/            # 领域模型、ts-rs 导出
│   ├── server/          # 本地 Axum HTTP / WS 服务 + 远程只读查看面
│   ├── adapters/
│   │   ├── claude/      # Claude Code 会话解析
│   │   ├── codex/       # Codex 会话解析
│   │   ├── openclaw/    # OpenClaw 会话解析
│   │   └── hermes/      # Hermes 会话解析（实验性）
│   ├── installer/       # 工具检测与诊断
│   └── companion/       # 配对码与远程查看会话
├── apps/
│   ├── web/             # React 19 + Zustand + Vite 7 + Tailwind CSS 4
│   └── desktop/         # Tauri 2 桌面壳
└── docs/
```

**关键设计决策：**

| 决策 | 原因 |
|------|------|
| Rust 服务端 + React 前端 | 浏览器无法直接读取本地工具文件，因此需要本地进程 |
| Tauri 2 桌面壳 | 更轻量，复用 Web UI，避免 Electron 体积和资源开销 |
| 不使用数据库 | 监控工具文件本身就是事实来源 |
| 仅 3 个运行时 JS 依赖 | `react`、`react-dom`、`zustand`，刻意保持精简 |
| 只用 WebSocket 做实时通道 | 事件驱动推送，避免轮询和竞态 |
| `tokio::sync::RwLock` | 适合异步场景下的多读少写状态 |
| 并行 adapter 探测 | 使用隔离并发 probe 任务并行本地扫描，不引入数据库 |
| 本地管理面与远程只读面分离 | 管理 API 仅 loopback 暴露；远程查看面只读且带 cookie 鉴权 |

## 开发与验证

```bash
# Rust 测试
cargo test --workspace

# Rust lint
cargo clippy --workspace -- -D warnings

# Web 单测
pnpm --filter @octomonitor/web test --run

# 可访问性审计
pnpm test:a11y

# 预发布检查
pnpm release:check

# 构建公证版 macOS 包
pnpm build:desktop:notarized
```

## 键盘快捷键

| 按键 | 功能 |
|------|------|
| `1` / `2` / `3` / `4` / `5` | 切换标签页（Monitor / Usage / Commits / Heatmap / Settings） |
| `j` / `k` | 在会话列表中移动 |
| `Enter` | 打开详情抽屉 |
| `Esc` | 关闭抽屉 |
| `?` | 显示快捷键面板 |
| `Cmd` / `Ctrl` + `,` | 在桌面客户端中打开设置 |
| `Cmd` / `Ctrl` + `+` / `-` / `0` | 在桌面客户端中放大 / 缩小 / 重置缩放 |

桌面版还通过系统菜单栏提供原生的撤销、重做、剪切、复制、粘贴和全选快捷键。

## 配置

本地管理面固定在 `127.0.0.1:46321`。如果开启远程访问，OctoMonitor 还会在 `0.0.0.0:46322` 启动一个单独的只读 viewer，并在 Settings 中展示可访问地址。远程访问相关配置会保存在 `~/.octomonitor/config.json`，重启后仍然有效。前端显示偏好（主题、密度、筛选条件、通知等）保存在 `localStorage`。

当前“环境与诊断”页面只提供检测与 doctor 能力，不会静默改写 Claude Code、Codex、OpenClaw、Hermes、Gemini、Cursor Agent、WorkBuddy/CodeBuddy 或 Pi 的配置文件。Hook Manager 只有在显式 preview/apply 后才写入，并带 backup、verify、uninstall 和 audit。Detection-only/watchlist 集成可以出现在 Settings 中，但只有 fixture-gated 或 monitored adapter 产出非 `N/A` usage semantics 的 run 时才会进入 usage。

## 许可证

[MIT](LICENSE)
