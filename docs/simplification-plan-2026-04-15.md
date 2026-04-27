# OctoMonitor 精简方案

> 日期：2026-04-15
> 依据：完整阅读当前仓库，并逐点评估此前的 `opus` 临时方案与独立审阅结论后形成的最终版
> 目标：去掉偏离主线或重复建设的部分，同时避免把真正有产品价值、能体现差异化的能力砍得过头
> 状态：方案中所列 Phase 0-4 均已落地；当前作为范围收敛的决策记录保留。实际行为说明请以 `README.md`、`README.zh.md`、`CONTRIBUTING.md` 和 `docs/README.md` 为准。

## 0. 先说结论

最终判断不是“尽可能删”，而是：

- **坚决删除**：已经长成第二产品的 workflow 编排链、设计导出残留、历史 demo 工程、旧配对 API
- **保留但收窄**：remote companion、installer、commit/heatmap 分析、Hermes
- **明确不删**：核心监控链路、pricing、usage、inspect、主题系统、历史可视化本身

也就是说，OctoMonitor 应该收敛成：

**一个以本地只读监控为主、带适度历史分析和可选 companion 查看的统一监控器。**

而不是继续往“工作流编排器 / LLM 报告器 / 设计交付仓库 / 多产品实验场”方向发散。

## 1. 精简原则

这次最终方案遵循 4 条判断规则：

1. 是否直接服务于“查看 AI 工具现在在做什么”
2. 是否明显违反了 `read-only by default`
3. 是否引入了一整套额外状态机、协议或构建链路
4. 删除后会不会伤到产品的核心价值或差异化能力

如果一个模块：

- 偏离监控主线
- 写操作重
- 状态复杂
- 用户路径又不清晰

就应该优先删除。

如果一个模块：

- 仍属于监控或查看能力
- 是产品差异化的一部分
- 代码边界相对清楚
- 可以通过收窄边界而不是整块拿掉来降复杂度

就不应该为了“看起来更小”而硬砍。

## 2. 已经落地的低风险收口

下面这些已经实际完成，属于第一批高回报、低风险改动：

### 2.1 移除旧配对 API

已移除：

- `/api/pair/request`
- `/api/pair/approve/{token}`
- `/api/pair/revoke/{token}`
- `crates/server/src/handlers/pairing.rs`

判断：

- 这套 API 当前前端已经不用
- 只是在保留两套 pairing 体系
- 删除后不会影响现有 remote pairing 主路径

### 2.2 Installer 收缩为纯诊断面

已收缩为仅保留：

- `/api/installer/detect`
- `/api/installer/doctor`
- crate 内部的 `install_plan / apply_install / rollback_install / verify_install` 也已删除

已经去掉：

- `install-plan`
- `install`
- `rollback`
- 内部 sandbox manifest 写入/校验/回滚逻辑
- Settings 里的写入型按钮

判断：

- 当前产品并不真正完成工具接入安装
- 与其给用户一种“能自动安装/回滚”的错觉，不如明确把它定义成环境诊断

### 2.3 Figma Make 退出正式构建链路

已完成：

- `apps/web/vite.config.ts` 不再构建 `figma-make.html`
- `apps/desktop/src-tauri/build.rs` 不再为它生成占位壳

判断：

- 设计导出页不应继续参与正式 Web/桌面构建

## 3. 最终逐项裁决

## 3.1 Workflow 编排系统：删除

删除范围：

- `crates/server/src/workflows/*`
- `crates/server/src/handlers/workflows.rs`
- `crates/core/src/workflow.rs`
- `crates/cli`
- `apps/web/src/components/workflows/*`
- `BootstrapPayload.workflow_runs`
- `RunRecord.workflow_hint`
- 相关 i18n、CSS、App tab、快捷键、README/文档

最终判断：

- 这是本仓库最明确的“第二产品”
- 它不是监控视图，而是执行/编排系统
- 它天然把产品从 read-only 推向 write/control plane
- 它引入了过多状态和操作：create/run/approve/link/retry/launch/complete
- CLI crate 基本也完全围绕 workflow 存在

结论：

- **整条移除**
- 不建议继续以“以后可能有用”为理由保留
- 如果未来真要做，应该独立成单独产品或至少独立子系统，不再挂在 OctoMonitor 主仓库里

## 3.2 Remote Companion：保留，但收窄

最终判断：

- `companion` 仍属于“查看能力”，不是偏航能力
- 它是 OctoMonitor 与普通本地 wallboard 的差异化之一
- 但它现在暴露的远程 surface 太宽，放大了双模维护成本

保留内容：

- pairing / session
- read-only remote bootstrap + stream
- 本地设置里对 remote 的开关与设备管理

建议收窄为：

- remote viewer 只保留 `Monitor` 和 `Usage`
- 不再给 remote 暴露 `Commits / Heatmap / Workflows / Settings`
- redaction 继续维持显式 allowlist，不继续扩表

结论：

- **不删**
- **但不再把 remote 当成“完整镜像 UI”**

## 3.3 Installer：保留，但重新定义为 Environment/Doctor

最终判断：

- `detect + doctor` 是合理的
- 安装/回滚则已经证明是伪能力，应退出主线

保留：

- crate 本身
- detect/doctor
- 环境状态展示

不再做：

- 自动写 sandbox manifest
- 任何“好像要帮用户接入工具配置”的按钮和表述

产品文案建议：

- `Setup & Sandbox` 改为 `Environment`
- 明确这是“检测与诊断”，不是安装器

结论：

- **保留诊断**
- **不再保留写入型 onboarding 幻觉**

## 3.4 Commit / Commit Attribution：保留 Commit 视图，精简归因算法

最终判断：

- 最近 commit 与 session 的关系，仍然属于“结果观察”
- 但当前 `crates/server/src/commits.rs` 的启发式归因过重
- 现在的问题不是“commit 视图不该存在”，而是“算法和分配模型太重”

建议保留：

- 最近 commit 列表
- repo/worktree 发现
- 基本时间窗口 + 仓库匹配
- 简单的工具关联

建议删除或显著弱化：

- 复杂消息相似度匹配
- 高颗粒度置信度/分配算法
- 过细的 token/cost 分摊叙事
- 对“归因精确度”的过度承诺

结论：

- **保留 Commits tab**
- **但 commits.rs 要从“高级归因引擎”收缩成“实用型 commit 观察层”**

## 3.5 Heatmap：保留视图，去掉额外控制面和 LLM 化延展

最终判断：

- Heatmap 是一个有辨识度的历史可视化
- 把它整个删掉，会把产品压得过于“只有列表”
- 但围绕 heatmap 继续叠 LLM 总结、历史报告队列、模式选择，会再次让产品偏到分析平台

建议保留：

- `HeatmapView`
- `week / month / total` 三层时间范围
- 本地计算出的基础 summary
- `commit` 维度如果继续保留，只保留原始 commit 数量，不再叠加复杂 attribution 权重和 LLM 解释

建议删除：

- `InsightsSection`
- `historicalReports` 队列
- `/api/daily-summary/generate`
- 通过 Claude/Codex CLI 生成日报的路径
- `settings.dailySummaryMode.*` 这类控制项

换句话说：

- **保留 Heatmap**
- **删除“LLM 报告器”**

这能留下可视化价值，同时明显降低设置面和后端控制面的复杂度。

## 3.6 Daily Summary LLM：删除

最终判断：

- 后端实现虽然不长，但它把产品拉向“让一个模型总结另一个模型”
- 这不是主监控链路
- 前端围绕它形成了更大规模的复杂度：模式、历史生成、复制整批报告、日期范围、可用性检测

保留：

- 基础本地 summary 计算函数，如果 Heatmap 仍然要用

删除：

- `POST /api/daily-summary/generate`
- `InsightsSection`
- `dailySummaryMode`
- 历史报告生成工作流

结论：

- **删除 LLM 报告功能**
- **保留轻量本地摘要**

## 3.7 Hermes：不作为当前优先删除项，但降级为 Experimental

最终判断：

- Hermes 确实增加了分支复杂度
- 但它已经接入了 probe / monitor / usage / watcher / styles，立刻整删的回归面不小
- 与 workflow 不同，它仍然属于“监控适配器”，不是完全偏航的第二产品

所以最终结论不是“立即删除 Hermes”，而是：

- 先把它从产品主叙事里降级
- README/定位仍以 Claude / Codex / OpenClaw 为主
- Hermes 视作 experimental adapter
- 在主线稳定前，不再继续为 Hermes 扩张专用逻辑

只有在后续确认：

- 几乎无人使用
- 维护成本持续拖累主线

再考虑整块抽离或 feature gate。

结论：

- **现在不删**
- **但不再把它当第一优先级覆盖面继续扩展**

## 3.8 Pricing：保留

最终判断：

- Usage 的价值直接依赖 pricing
- 现在 pricing 不是粗暴硬编码，而是相对合理的动态来源
- 为了省几百行去砍它，不值

结论：

- **完整保留**

## 3.9 主题 / i18n：只做联动清理，不单独动刀

最终判断：

- 主题系统本身是产品完成度的一部分
- i18n 也已经深度进入产品结构
- 它们的问题不是“存在”，而是跟着 workflow/insights/设计残留积了额外键值和样式

结论：

- **只随删除模块联动清理**
- **不单独简化主题系统本体**

## 3.10 设计导出物、历史 demo、仓库噪音：清理

删除或移出主仓库：

- `apps/web/src/figma-make/*`
- `apps/web/figma-make.html`
- `apps/web/FIGMA_MAKE_HANDOFF.md`
- `CommitUIDemo/`
- `exports/`
- 根目录验证截图与一次性产物

处理原则：

- 设计交付资产不再放在产品代码主树里
- 历史 demo 不再继续参与仓库认知和构建

## 4. 最终目标产品形态

经过这轮收敛后，OctoMonitor 应该保留这 5 个本地 tab：

1. `Monitor`
2. `Usage`
3. `Commits`
4. `Heatmap`
5. `Settings`

其中：

- `Monitor` 是主入口
- `Usage` 是核心二级分析
- `Commits` 是结果观察
- `Heatmap` 是历史模式观察
- `Settings` 只保留真正属于产品自身的配置

明确移除：

- `Workflows`

### Settings 最终建议结构

保留：

- `Appearance`
- `Remote Access`
- `Monitor`
- `Filter`
- `System`
- `Environment`（由当前 Setup 收缩而来）

移除：

- `Insights`

## 5. 后端 API 最终建议形态

保留：

- `GET /api/bootstrap`
- `GET /api/health`
- `GET/PATCH /api/config`
- `GET /api/history/usage`
- `GET /api/history/commits`
- `GET /api/runs/{id}/inspect`
- `GET/PATCH /api/remote/access`
- `GET /api/remote/devices`
- `POST /api/remote/pairings`
- `DELETE /api/remote/devices/{device_id}`
- `POST /api/pair/claim`
- `POST /api/ingest/claude/statusline`
- `POST /api/ingest/claude/hook`
- `POST /api/ingest/codex/hook`
- `GET /api/stream`
- `GET /api/installer/detect`
- `GET /api/installer/doctor`

删除：

- 全部 workflow 路由
- 全部旧 pairing 路由
- `/api/daily-summary/generate`
- installer 的写入型路由（已完成）

## 6. 文档最终建议形态

主入口保留：

- `README.md`
- `README.zh.md`
- `CONTRIBUTING.md`
- `docs/README.md`
- `docs/simplification-plan-2026-04-15.md`

历史设计、实施计划和视觉探索统一归档到 `docs/history/`。
- 文档里“现行事实”和“历史方案”必须分开

## 7. 实施顺序

### Phase 0：已完成

- 移除旧 pairing API
- installer 收缩为 detect/doctor
- installer 内部写入能力删除，彻底变成纯诊断模块
- `figma-make` 退出正式构建链路

### Phase 1：无核心功能风险的仓库清理

- 删除 `figma-make` 源文件与 handoff 文档
- 删除 `CommitUIDemo/`
- 删除 `exports/`
- 清掉 workflow 相关旧文档与截图类 artifact
- 顺手清掉 `docs/.DS_Store` 这类仓库噪音文件

### Phase 2：删除第二产品

- 删除 workflow 后端
- 删除 workflow 前端
- 删除 CLI crate
- 清理 `core` 中 workflow 类型和 `workflow_hint`
- 清理 App tab、快捷键、i18n、CSS

### Phase 3：收缩分析面

- 删除 `InsightsSection`
- 删除 LLM 日报 API 和状态
- 把 Heatmap 留在“本地历史可视化”边界内
- 精简 `commits.rs`

### Phase 4：收窄 remote 和产品叙事

- remote viewer 只保留 `Monitor / Usage`
- README 收敛为“本地只读监控器 + 可选 companion”
- Hermes 降级为 experimental

## 8. 这份方案如何避免过度精简

这是这次最终版与更激进版本最大的区别。

明确没有继续砍掉的，是下面这些：

- Remote companion
- Heatmap 视图本身
- Commits 视图本身
- Pricing
- Theme
- 多工具适配器体系

原因很简单：

- 如果把这些也一起删掉，OctoMonitor 会过度收缩成“只有一个本地 session 列表”
- 那样虽然更小，但产品价值、覆盖面和辨识度都会明显下降
- 精简的正确方向不是把产品砍成最小，而是把**偏航能力**砍掉，把**主线能力**做厚

## 9. 最终判断

最终版的核心判断可以压缩成一句话：

**删掉 workflow 和设计残留，收缩报告器和伪安装器，保留 companion、history、usage、commits、heatmap 这些仍然属于监控价值的能力。**

这比“全砍成 3 个 tab”的方案更稳，也更符合产品价值。

OctoMonitor 最终应该是：

- 一个本地优先
- 只读为主
- 能看实时、也能看历史
- 能在本机和 companion 设备上查看
- 但不再承担工作流编排和 LLM 报告生成

的统一 AI 工具监控器。
