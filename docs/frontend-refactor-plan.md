# 前端规范化改进方案（apps/web）

> 2026-04-16 起草。对象：`apps/web/src`（React 19 + Zustand + Vite + Tailwind v4，**纯 CSR，无 SSR**）。
> 目标：修正真实存在的规范/可维护性问题，给出**最小必要**的改动，不追求理想态。

> 本文档经过一次内部 review（2026-04-16），已删除虚假问题（原 P1-4）与过度设计（原 `reportError` 独立文件、原 `I18nKeyOf` 误用、`safeStorage` 的 SSR 分支），修正了夸大描述（原 P0-3 对 InspectDrawer/WS 的指控），并补全遗漏项（i18n.tsx、MonitorView 拆分判断、PR 顺序）。

---

## 一、审阅结论概要

代码总体健康度良好：
- 纯工具函数比例高（`format.ts`、`heatmap.ts`、`usage.ts`、`monitor.ts`）；
- 类型边界清晰（`types.ts` 来自 Rust bindings）；
- i18n 基础类型安全（`I18nKey = keyof typeof en`）；
- Zustand store 规模小、语义清晰；
- `useMediaQuery` 使用 `useSyncExternalStore`。

需要处理的真实问题集中在 3 类：
1. **类型一致性**：运行时可能产生 `undefined`，但类型声明为非可选；动态 i18n key 失去类型检查。
2. **可观测性**：几处 `catch` 完全空吞错误，排障困难。
3. **常量管理**：localStorage key 与自定义事件名在 5–6 处硬编码。

---

## 二、问题清单（按优先级）

### P0（会导致缺陷，必须改）

#### P0-1　`buildSummary` reduce 初值可能为 `undefined`，与类型声明不一致
- 位置：`src/lib/heatmap.ts:195-208`
- 现象：`visibleCells.reduce((best, cell) => {...}, visibleCells[0])` — 空数组时 reducer 不执行，直接返回初值 `undefined`。但 `HeatmapSummary.peakCell` / `topDay`（第 28–29 行）声明为非可选，消费方（`HeatmapView.tsx`）按非空访问。
- 影响：极端数据（无 run / 无 commit）下渲染空白或潜在 `TypeError`。
- 严重度：高。

#### P0-2　6 处 i18n 动态 key 使用 `as any` / `as never`
- 位置：`InspectDrawer.tsx:30`、`MonitorView.tsx:105`、`settings/MonitorSection.tsx:141`、`settings/FilterSection.tsx:60`、`settings/AppearanceSection.tsx:92`（`as never`）+ `settings/AppearanceSection.tsx:104`。
- 现象：`` t(`state.${state}` as any) ``、`` t(`settings.fontSize.${size}` as any) ``。
- 影响：`RunState` / `FontSize` / `UiDensity` / `FilterMode` / `AgentDisplayFormat` 任一扩容，翻译缺失只在运行时 fallback 为 key 字符串，无编译期提示。
- 严重度：中。

#### P0-3　真正空吞错误的 2 处
- 位置：
  - `src/components/monitor/settings/SetupSection.tsx:21-23`（`.catch(() => {})` — 能力探测失败无任何反馈）
  - 4 处 localStorage 写入无 try/catch：`preferences.ts:165`、`theme.tsx:190,224`、`i18n.tsx:664`、`monitorStore.ts:83`（Safari 隐私模式 / 配额超限会抛 `QuotaExceededError`，当前整个写路径中断）
  - 另有 1 处 `desktopZoom.ts:39-41` 已有 try/catch 但 catch 为空（静默），按同一标准补 `console.warn`
- **不包含**：
  - `InspectDrawer.tsx:76-80` — 已有降级行为（`setEntries([])`），并非"完全静默"；
  - `App.tsx:85-90`（WS 消息解析）— 注释 `// ignore malformed frames`，单帧失败不应中断连接，是刻意设计。
- 严重度：中。

---

### P1（维护性风险，应改）

#### P1-1　6 个 localStorage key 分散硬编码
- `octomonitor-theme`：`main.tsx:3`、`lib/theme.tsx:194,224`
- `octomonitor-custom-themes`：`lib/theme.tsx:182,190`
- `octomonitor-settings`：`lib/preferences.ts:43`（已是 const，但外部测试写死字符串）
- `octomonitor-locale`：`lib/i18n.tsx:654,664`
- `octomonitor-desktop-zoom`：`lib/desktopZoom.ts:3`（已是 const，未共享）
- `octomonitor-dismissed-attentions-v2`：`store/monitorStore.ts:9`（已是 const，未共享）
- 严重度：中。

#### P1-2　2 个自定义 DOM 事件名硬编码
- `octomonitor:desktop-boot-status`、`octomonitor:desktop-menu-action`（`App.tsx:47-48`）
- 仅 `App.tsx` 内部消费，后续若在 desktop shell 或其他组件联动，字符串易错。
- 严重度：低。

#### P1-3　跨视图重复的"历史数据加载 + 缓存"逻辑
- 位置：`CommitsView.tsx`、`HeatmapView.tsx:225-261`、`UsageView.tsx`
- 注意：三处**缓存语义不同**：
  - `HeatmapView` 按 `scope` 的多键缓存（切换 scope 时保留旧数据）；
  - `CommitsView` 按 preset 键缓存；
  - `UsageView` 单键缓存。
- 影响：局部维护成本增加，但因差异实在，**不适合**抽象为统一 hook（见 §三 决策）。
- 严重度：低。

---

### P2（记录项，本轮不改）

- `ReturnType<typeof getRuntimeMode>` 在 `App.tsx` 出现 3 次 → 低成本顺手做（见 §3.5）。
- `CommitsView.tsx`（494 行）/ `HeatmapView.tsx`（597 行）/ `MonitorView.tsx`（640 行）三个大组件：均有测试覆盖；`MonitorView` 内部子组件（SourceColumn 等）虽职责可拆，但 prop 接口膨胀代价较高。**不在本轮范围**。
- `i18n.tsx` 675 行：95% 是 `en`/`zh` 翻译表。可拆为独立 JSON/`.ts` 数据文件，但纯机械迁移、无行为变化，留给未来翻译工作流改造一起做。**不在本轮范围**。
- Heatmap cell 方向键导航 / ShortcutOverlay 焦点管理：无障碍增强，不是"规范性"问题。
- `showShortcutHelp` / `selectedRunId` 下沉到局部 state / Context：当前访问链路短，下沉反而引入 Provider 层级，收益小于成本。
- `styles.css` 5025 行：一次性 Tailwind 化风险>收益。约定新组件优先 Tailwind，老样式保持不动，自然演进。

---

## 三、方案设计

### 3.1　修复 `buildSummary` 空集合处理（P0-1）

`src/lib/heatmap.ts`：

```ts
export interface HeatmapSummary {
  total: number
  peakCell?: HeatmapCell     // 改为可选
  topDay?: HeatmapDayTotal   // 改为可选
  activeDays: number
  longestStreak: number
}

function buildSummary(cells: HeatmapCell[], dayTotals: HeatmapDayTotal[]): HeatmapSummary {
  const visibleCells = cells.filter((c) => !c.hidden)

  const peakCell = visibleCells.length === 0
    ? undefined
    : visibleCells.reduce((best, cell) => {
        if (cell.value > best.value) return cell
        if (cell.value === best.value && cell.startMs > best.startMs) return cell
        return best
      })   // 去掉显式初值，reduce 自动用第一个元素

  const topDay = dayTotals.length === 0
    ? undefined
    : dayTotals.reduce((best, day) => {
        if (day.total > best.total) return day
        if (day.total === best.total && day.date.getTime() > best.date.getTime()) return day
        return best
      })

  // total / activeDays / longestStreak 保持不变
}
```

消费处（`HeatmapView.tsx`）：用 `summary.peakCell?.…` / `summary.topDay &&` 条件渲染，空时显示占位文案。

**理由**：让类型反映事实；去掉可疑的初值，避免空分支潜在崩溃。

---

### 3.2　用映射表替代模板字符串 + `as any`（P0-2）

> **Review 指出**：原稿提议的 `I18nKeyOf<P, V>` + `as` 断言**无法产生编译期保护**（`as` 是断言不是结构检查）。正确做法是显式映射表，让 `tsc` 在枚举扩容时报缺 key。

新建 `src/lib/i18nMaps.ts`（或就近放在各 settings 组件旁）：

```ts
import type { I18nKey } from './i18n'
import type { RunState, FontSize, UiDensity, FilterMode, AgentDisplayFormat } from './types'  // 按实际导出调整

export const stateLabelKeys: Record<RunState, I18nKey> = {
  active: 'state.active',
  waitingApproval: 'state.waitingApproval',
  idle: 'state.idle',
  error: 'state.error',
  completed: 'state.completed',
  // 新增 RunState 成员 → tsc 报 "missing property" → 必须补翻译
}

export const fontSizeKeys: Record<FontSize, I18nKey> = { /* … */ }
export const uiDensityKeys: Record<UiDensity, I18nKey> = { /* … */ }
export const filterModeKeys: Record<FilterMode, I18nKey> = { /* … */ }
export const agentDisplayFormatKeys: Record<AgentDisplayFormat, I18nKey> = { /* … */ }
```

调用处：
```ts
// 改前：t(`state.${state}` as any)
// 改后：t(stateLabelKeys[state])
```

**理由**：`Record<Enum, I18nKey>` 的缺 key 是 tsc 硬错误，而 `as any` 不是。零运行时成本。`I18nKey` 本身是 `keyof typeof en` 已有，无需扩展。

---

### 3.3　集中 localStorage key 与事件名（P1-1、P1-2）

新建 `src/lib/storageKeys.ts`：

```ts
export const STORAGE_KEYS = {
  theme: 'octomonitor-theme',
  customThemes: 'octomonitor-custom-themes',
  settings: 'octomonitor-settings',
  locale: 'octomonitor-locale',
  desktopZoom: 'octomonitor-desktop-zoom',
  dismissedAttentions: 'octomonitor-dismissed-attentions-v2',
} as const
```

新建 `src/lib/desktopEvents.ts`：

```ts
export const DESKTOP_BOOT_EVENT = 'octomonitor:desktop-boot-status'
export const DESKTOP_MENU_ACTION_EVENT = 'octomonitor:desktop-menu-action'
```

替换业务代码中所有硬编码字符串。**测试文件一并替换**（import STORAGE_KEYS）——否则 key 重命名后测试断裂，常量集中失去意义。

**理由**：零运行时成本；单点改名；未来迁移版本（如把 `-v2` 升到 `-v3`）只改一处。

---

### 3.4　写 `catch`（P0-3）

> **Review 指出**：原稿设计的 `reportError(scope, err, meta)` 独立文件只有一层 `console.warn`，当前没有 Sentry/Tauri log 的路线图。一个纯间接层属于过度设计。

直接在 5 处写 `console.warn` 即可，不引入新模块：

1. `settings/SetupSection.tsx:21-23`：
   ```ts
   .catch((err) => { console.warn('[OctoMonitor] setup capabilities', err) })
   ```
2. `preferences.ts` `saveFrontendSettings`、`theme.tsx` `saveCustomThemes` / `setTheme`、`i18n.tsx` `setLocale`、`monitorStore.ts` `dismissAttention`：
   ```ts
   try { localStorage.setItem(KEY, value) }
   catch (err) { console.warn('[OctoMonitor] storage.write', { key: KEY, err }) }
   ```

保留一个**共识**：若未来接入远程日志，再统一提取 `reportError`。现在不做抽象。

**不新增** `safeStorage` 包装。原稿中的 `typeof localStorage === 'undefined'` 分支在浏览器/Tauri webview 永远不会命中（CLAUDE.md 明确无 SSR），是死代码。需要的只是 try/catch。

---

### 3.5　微小项

- `runtimeMode.ts`：导出 `export type RuntimeMode = 'local' | 'remoteViewer'`，`getRuntimeMode(): RuntimeMode`，`App.tsx` 删 5 处 `ReturnType<typeof getRuntimeMode>`。
- `preferences.ts` `migratePanelConfig` 补一次 `tool` 去重（`seen = new Set<ToolKind>()`）——防止损坏 storage 数据让同一 tool 出现两次。

---

### 3.6　P1-3 不做抽象（显式决策）

原稿提出 `useAsyncResource` 统一三处历史数据加载。Review 指出：
- `HeatmapView` 的缓存是 `Record<HeatmapScope, ScopeCache>` 多键结构（切 scope 保留旧缓存）；
- `CommitsView` / `UsageView` 缓存粒度不同；
- 一个 `(signal) => Promise<T>` 签名的 hook 无法承载多键缓存而不破坏 `HeatmapView` 现有行为。

**决策**：不抽象。保持三份实现。如果将来某个视图也要这个缓存语义，再考虑提取。

---

## 四、不纳入本方案的事项（明确排除）

| 事项 | 原因 |
|------|------|
| `styles.css` 拆分 / Tailwind 全量迁移 | 5025 行，一次迁移风险过高；新组件优先 Tailwind 自然演进。 |
| `CommitsView` / `HeatmapView` / `MonitorView` 组件拆分 | 三者均有测试覆盖；拆分后 prop 接口膨胀、中间组件难单测，收益<成本。 |
| `i18n.tsx` 675 行拆翻译表 | 纯机械迁移，无行为变化；留给未来翻译工作流一起改。 |
| `selectedRunId`/`showShortcutHelp` 下沉到 Context | 当前访问链路短；Context 嵌套反而复杂。 |
| 引入 toast 错误 UI | 需要新依赖 + 样式系统，超出"规范化"范围。 |
| 引入 `react-query` / `swr` | `P1-3` 决定不抽象历史数据 hook，更无引入框架必要。 |
| Heatmap cell 方向键导航、`main.tsx` 顶层 localStorage "加固" | 前者是无障碍增强；后者在无 SSR 环境下是虚假问题（浏览器必有 localStorage）。 |
| 独立的 `reportError` / `safeStorage` 工具 | 只有一层 `console.warn`，属于纯间接层。 |

---

## 五、执行分组

拆 3 个 PR。**PR-1 与 PR-2 都会改 `preferences.ts`，合并顺序 PR-1 → PR-2**（PR-2 rebase PR-1 后解决 trivial 冲突）。

### PR-1：常量集中 + catch 加日志（P0-3、P1-1、P1-2）
**范围**：
- 新增 `storageKeys.ts`、`desktopEvents.ts`。
- 业务代码与测试代码一并替换字符串常量。
- 5 处 `catch` 加 `console.warn`（1 处 SetupSection + 4 处 localStorage 写入）。

**验收**：
- `pnpm --filter @octomonitor/web test --run` 全过。
- Grep 确认 `'octomonitor-'` 字符串只出现在 `storageKeys.ts` 和 vitest.setup.ts 相关 mock 里。
- 手测：在 DevTools Application 面板把 Storage 设为 "No storage"（或在代码临时 `localStorage.setItem = () => { throw new Error('quota') }`）后切换主题，UI 不再未捕获报错，控制台有 `[OctoMonitor] storage.write` 记录。

### PR-2：类型一致性（P0-1、P0-2、§3.5）
**范围**：
- `HeatmapSummary.peakCell`/`topDay` → optional；消费处加守卫。
- 新增 `i18nMaps.ts`（或就近映射表），替换 6 处 `as any` / `as never`。
- `RuntimeMode` 类型别名。
- `migratePanelConfig` 加去重。

**验收**：
- `pnpm --filter @octomonitor/web build`（含 `tsc -b`）无新增错误。
- `heatmap.test.ts` 补一个空集合用例（`peakCell === undefined`）。
- 手测：故意删除 `en.ts` 里一个 `state.*` 条目 → 构建应报 `Record<RunState, I18nKey>` 缺成员；验证后还原。

### PR-3（可选）：无
原稿的 PR-3/PR-4 被 §3.6 取消。

---

## 六、回滚策略

每个 PR 独立可 revert；不跨 PR 共享运行时状态。如 PR-2 的 optional 化导致某些旧测试意外失败，可临时在消费方用 `summary.peakCell!`（非空断言）暂时保留行为、补测试后再去掉。

---

## 七、非目标

- 不重构架构；不新增业务功能；不引入新的运行时依赖。
- 不追求"理想状态"——只修会直接造成 bug / 显著增加排障成本的条目。
