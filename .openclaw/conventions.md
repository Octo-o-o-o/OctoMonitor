# Project Conventions — OctoMonitor

> 最近更新: 2026-03-30 — intake-20260330-1722-aac2 — 初始化 greenfield monorepo 约定

## 1. 项目概况

- **类型**: monorepo
- **技术栈**: Rust 1.89（Tokio / Axum / Notify / Serde / Tracing），React 19 + TypeScript + Vite，Zustand，Tauri 2
- **关键目录**:
  - `apps/web` → Web UI / Companion UI
  - `apps/desktop/src-tauri` → Tauri desktop shell
  - `crates/core` → 领域模型、聚合器、状态机
  - `crates/server` → HTTP / WebSocket API
  - `crates/adapters/*` → Claude / Codex / OpenClaw 适配器
  - `crates/installer` → 安装、探测、回滚
  - `crates/companion` → pairing、只读 companion 会话
- **已有规范文档**: `main.md`, `CLAUDE.md`, `docs/design.md`, `.openclaw/frontend-conventions.md`

## 2. 代码规范

### 2.1 命名约定

| 对象 | 风格 | 示例 |
|------|------|------|
| Rust modules/files | snake_case | `run_record.rs` |
| Rust types/traits | PascalCase | `RunRecord`, `AdapterHealth` |
| TS variables/functions | camelCase | `buildBootstrapPayload` |
| React components | PascalCase | `RunningLaneCard` |
| React hooks / Zustand stores | camelCase with prefix | `useLayoutMode`, `useMonitorStore` |
| Routes | kebab-case path | `/companion/eink` |
| CSS variables | `--category-name` | `--surface-raised` |
| API paths | REST + kebab-case | `/api/bootstrap` |
| Config keys | camelCase JSON | `historyDays` |

### 2.2 架构模式

**服务端 / 后端**:
- 分层: adapter → normalize → aggregate → query / stream
- 模块组织: 按领域拆 crate，避免单个 server crate 吞掉所有逻辑
- 数据库: 不使用数据库；仅允许配置 / pairing / alias / pricing 等文件持久化
- API 风格: REST for snapshot/config/setup, WebSocket for live patches

**客户端 / 前端**:
- 路由: React Router 显式路由
- 组件分层: route shell → feature panel → shared primitives
- 状态管理: Zustand 持有全局快照、选择态和 WS 状态
- 样式: Tailwind CSS + 少量 CSS variables / container queries
- 设计系统: 自定义 dark-monitor tokens，图标优先 Lucide

**CLI / 脚本**:
- 命令组织: 子命令式（doctor / install / rollback / serve）
- 参数解析: Rust `clap`
- 输出格式: 人类可读 + `--json` 机器可读

### 2.3 错误处理

- Rust 统一返回显式错误类型，边界层转换为 API 错误响应
- 使用 `tracing` 输出结构化日志
- 前端异步状态必须覆盖 loading / empty / error
- 禁止吞错、禁止仅 `console.log` 不回传 UI

### 2.4 i18n

- 首版默认英文 UI 文案，内部文本集中在 `apps/web/src/i18n/messages.ts`

## 3. 测试约定

- **框架**:
  - Rust: `cargo test`
  - Web: `vitest`, `@testing-library/react`
  - E2E/acceptance: Playwright
- **目录**:
  - Rust tests colocated / `tests/`
  - Web unit tests colocated as `*.test.ts(x)`
  - E2E under `apps/web/e2e`
- **策略**: 单元覆盖领域模型 / 聚合逻辑，集成覆盖 API 和 installer，E2E 覆盖 wallboard / history / setup / companion 关键路径
- **Mock 策略**: UI 层可 mock API；聚合器和 adapter normalization 使用真实 fixture 文件，不 mock 领域计算

## 4. 部署约定

- **平台**: 本地桌面 + 本地 Web；桌面采用 Tauri bundle
- **CI/CD**: 首版以本地可构建、可运行为准，后续再接正式 CI
- **环境**: dev = 本地开发；release = 本地 bundle / tarball
- **关键注意事项**: Companion 默认关闭；任何 LAN 能力必须显式开启并只读

## 5. 审批门控 ⚠️ 待确认

### 5.1 需要 Yixiao 确认的操作

- 引入数据库或后台云服务
- 修改监控源工具的敏感配置语义
- 任何会暴露入站网络端口到非局域网的能力
- 破坏性配置迁移

### 5.2 Athena 可自主执行

- monorepo 初始化与功能编码
- 本地配置文件、installer 脚本、测试与文档补充
- 小版本依赖选择与调整

### 5.3 质量门控建议

| 条件 | gate_mode | gate_sequence |
|------|-----------|---------------|
| 首版完整交付 | manual | self-test → reviewer → qa |

## 6. Agent 协作约定 ⚠️ 待确认

### 6.1 Athena 自主范围（本项目补充）

- 可修改: 仓库内所有业务代码、文档、测试、脚本
- 禁止修改: 用户本机上的外部敏感配置文件，除非 setup/install 流程明确且由用户触发

### 6.2 跨 Agent 信息传递

- implementation / review / qa 产物必须明确记录 fixture 来源、已验证页面、未覆盖风险
- reviewer 需重点关注本地只读边界、敏感字段泄露、布局退化
- qa 需覆盖 wallboard/history/setup/companion/eink 五类路由

### 6.3 与 Yixiao 的沟通约定

- 直接给结论和可验证命令
- 未完成项必须明确列为风险或后续 intake

## 7. 反模式

- 把所有逻辑塞进单一 `server` crate
- 把 cost / quota 估算伪装成官方数据
- 在前端硬编码颜色 / spacing 而不走 token
- 把实时高频事件走 Tauri event 而不是 localhost WS
- 为了展示方便读取或回显敏感 token / auth 内容
