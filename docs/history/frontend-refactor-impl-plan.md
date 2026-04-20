# 前端规范化改进 —— 分步实施计划

> 配套方案：`docs/history/frontend-refactor-plan.md`（2026-04-16）
> 执行方式：两个 Phase，每个 Phase 独立可回滚。Phase 间有文件重叠（`preferences.ts` 等），**顺序不可调**。
> 状态：历史实施记录。这里的改动已经在仓库中落地，保留它仅用于回看当时的拆分方式。

## 执行总则

1. 每一步结束后**立刻**跑：
   - `pnpm --filter @octomonitor/web test --run`
   - `pnpm --filter @octomonitor/web build`（包含 `tsc -b`）
2. 发现任何新增失败 / 类型错误 → 先定位根因，不许 `@ts-ignore` / `as any` 绕过。
3. 每个 Phase 结束才 commit；Phase 内部**不**中途提交。
4. Phase 2 在 rebase Phase 1 之上工作，若涉及 `preferences.ts` 冲突（§Phase2.6 migratePanelConfig），手动合并。

---

## Phase 1：常量集中 + catch 日志

对应方案 P0-3、P1-1、P1-2。目标：所有 localStorage key / 自定义事件名收敛到两个常量文件；5 处真正空吞的 catch 补上 `console.warn`。

### 1.1　新建 `src/lib/storageKeys.ts`

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

**验证点**：`pnpm --filter @octomonitor/web build` 通过（新文件不被引用也无错）。

### 1.2　新建 `src/lib/desktopEvents.ts`

```ts
export const DESKTOP_BOOT_EVENT = 'octomonitor:desktop-boot-status'
export const DESKTOP_MENU_ACTION_EVENT = 'octomonitor:desktop-menu-action'
```

### 1.3　替换业务代码中的 storage key

| 文件:行号 | 改动 |
|---|---|
| `src/main.tsx:3` | `localStorage.getItem('octomonitor-theme')` → `localStorage.getItem(STORAGE_KEYS.theme)`，添加 import（相对路径 `./lib/storageKeys`） |
| `src/lib/theme.tsx:182,190,194,224` | 4 处 `'octomonitor-theme'`/`'octomonitor-custom-themes'` 全改常量 |
| `src/lib/preferences.ts:43` | 删掉本文件内 `const STORAGE_KEY = 'octomonitor-settings'`，改为 `import { STORAGE_KEYS }`；使用 `STORAGE_KEYS.settings` |
| `src/lib/i18n.tsx:654,664` | 改常量 |
| `src/lib/desktopZoom.ts:3` | 删 `const STORAGE_KEY`，改用 `STORAGE_KEYS.desktopZoom` |
| `src/store/monitorStore.ts:9,13,83` | 删 `DISMISSED_ATTENTIONS_KEY`，改用 `STORAGE_KEYS.dismissedAttentions` |

### 1.4　替换业务代码中的事件名

| 文件:行号 | 改动 |
|---|---|
| `src/App.tsx:47-48` | 删 2 个 `const *_EVENT`，改 `import { DESKTOP_BOOT_EVENT, DESKTOP_MENU_ACTION_EVENT }`，行 233、395 的 `addEventListener` 及对应 `removeEventListener` 会自动复用常量，不需额外改 |

### 1.5　替换测试代码中的 storage key

- `src/lib/preferences.test.ts:17,48`、`src/components/monitor/MonitorView.test.tsx:56`、`src/components/monitor/SettingsView.test.tsx:22`
- 全部 `'octomonitor-settings'` / `'octomonitor-locale'` 改用 `STORAGE_KEYS.settings` / `STORAGE_KEYS.locale`
- 目的：避免常量改名后测试悄悄断掉

**验证点**：
```
pnpm --filter @octomonitor/web test --run   # 应全过（内容完全等价）
grep -R "octomonitor-" apps/web/src --include='*.ts*' | grep -v storageKeys.ts | grep -v vitest.setup.ts
# 预期只剩 console.warn 里的字符串前缀（如有）
```

### 1.6　补 `SetupSection.tsx:30,38` 的空 catch

```ts
.catch((err) => { console.warn('[OctoMonitor] setup.capabilities', err) })
.catch((err) => { console.warn('[OctoMonitor] setup.doctor', err) })
```

### 1.7　为 5 处 localStorage 写入补齐 try/catch + 日志

| 文件:行号 | 写操作 | 当前状态 |
|---|---|---|
| `src/lib/preferences.ts:165` (`saveFrontendSettings`) | 新增 try/catch，失败时 `console.warn('[OctoMonitor] storage.write.settings', err)` | 无 try/catch |
| `src/lib/theme.tsx:190` (`saveCustomThemes`) | 同上，scope 用 `storage.write.customThemes` | 无 try/catch |
| `src/lib/theme.tsx:224` (`setTheme` 内) | 同上，scope 用 `storage.write.theme` | 无 try/catch |
| `src/lib/i18n.tsx:664` (`setLocale` 内) | 同上，scope 用 `storage.write.locale` | 无 try/catch |
| `src/store/monitorStore.ts:83` (`dismissAttention` 内) | 同上，scope 用 `storage.write.dismissedAttentions` | 无 try/catch |
| `src/lib/desktopZoom.ts:39-41` (`saveDesktopZoom` 内) | 已有 try/catch 但 catch 为空（`// ignore storage failures`）。改为 `console.warn('[OctoMonitor] storage.write.desktopZoom', err)` | 空 catch，与其他 5 处标准不一致 |

### 1.8　全量自测

```bash
pnpm --filter @octomonitor/web test --run
pnpm --filter @octomonitor/web build
cargo test --workspace   # 确认没触及 rust
```

手测：浏览器 DevTools Application → Storage → 临时覆盖 `Storage.prototype.setItem` 抛错，切换主题 / 修改字号，确认 console 有 `[OctoMonitor] storage.write.*` 且 UI 不崩。

### 1.9　Review → Commit

自审清单：
- [ ] `grep -R "octomonitor-" apps/web/src` 仅余 `storageKeys.ts`、vitest 相关
- [ ] `grep -R "octomonitor:" apps/web/src` 仅余 `desktopEvents.ts`、`App.tsx`（import）
- [ ] 5 处 `catch` 均有 `console.warn` 且带 scope 标签
- [ ] 无新增 `as any` / `@ts-ignore`
- [ ] 测试、构建全绿

```
git add -A && git commit -m "refactor: centralize storage keys/events and log storage errors"
```

（与项目 git log 风格一致：`refactor:` / `feat:` 不带 scope。）

---

## Phase 2：类型一致性

对应方案 P0-1、P0-2、§3.5。目标：消除运行期 undefined / 类型声明不一致；消除 6 处 i18n 类型规避；RuntimeMode 别名；migratePanelConfig 去重。

### 2.1　`HeatmapSummary` 可选化 + 空集合分支

文件 `src/lib/heatmap.ts`：

1. `HeatmapSummary.peakCell` / `topDay` → `peakCell?`, `topDay?`（行 28-29）
2. `buildSummary`（行 195-208）改：
   ```ts
   const visibleCells = cells.filter((c) => !c.hidden)
   const peakCell = visibleCells.length === 0
     ? undefined
     : visibleCells.reduce((best, cell) => {
         if (cell.value > best.value) return cell
         if (cell.value === best.value && cell.startMs > best.startMs) return cell
         return best
       })
   const topDay = dayTotals.length === 0
     ? undefined
     : dayTotals.reduce((best, day) => {
         if (day.total > best.total) return day
         if (day.total === best.total && day.date.getTime() > best.date.getTime()) return day
         return best
       })
   ```

### 2.2　`HeatmapView` 消费处加守卫

读 `HeatmapView.tsx` 中所有 `summary.peakCell` / `summary.topDay` 的使用点，改 `?.` 或条件渲染。若模板中直接展示（如 `formatX(summary.peakCell.value)`），加条件渲染或 `—` 占位。

### 2.3　补 `heatmap.test.ts` 空用例

新增一个断言：输入空 `cells` / `dayTotals` 时 `buildSummary` 返回 `peakCell === undefined` 且 `topDay === undefined`，且 `total === 0`、`activeDays === 0`。若 `buildSummary` 未导出，则通过 `buildHeatmapViewModel` 的公开入口间接覆盖（空 runs / 空 buckets）。

### 2.4　导出 `RuntimeMode` 类型别名

文件 `src/lib/runtimeMode.ts`：
```ts
export type RuntimeMode = 'local' | 'remoteViewer'
export function getRuntimeMode(): RuntimeMode { /* 现有实现 */ }
```

`App.tsx` 中 5 处 `ReturnType<typeof getRuntimeMode>` 全改 `RuntimeMode`。

### 2.5　新建 `src/lib/i18nMaps.ts`

```ts
import type { I18nKey } from './i18n'
import type { RunState } from './types'
import type {
  FontSize, UiDensity, FilterMode, AgentDisplayFormat,
} from './preferences'

export const stateLabelKeys: Record<RunState, I18nKey> = {
  active: 'state.active',
  waitingApproval: 'state.waitingApproval',
  idle: 'state.idle',
  completed: 'state.completed',
  error: 'state.error',
  stale: 'state.stale',
  gatewayOffline: 'state.gatewayOffline',
  limitExceeded: 'state.limitExceeded',
  contextExceeded: 'state.contextExceeded',
  cancelled: 'state.cancelled',
}

export const fontSizeLabelKeys: Record<FontSize, I18nKey> = {
  xsmall: 'settings.fontSize.xsmall',
  small: 'settings.fontSize.small',
  default: 'settings.fontSize.default',
  large: 'settings.fontSize.large',
  xlarge: 'settings.fontSize.xlarge',
}

export const uiDensityLabelKeys: Record<UiDensity, I18nKey> = {
  compact: 'settings.uiDensity.compact',
  comfortable: 'settings.uiDensity.comfortable',
  spacious: 'settings.uiDensity.spacious',
}

export const filterModeLabelKeys: Record<FilterMode, I18nKey> = {
  off: 'settings.filterMode.off',
  include: 'settings.filterMode.include',
  exclude: 'settings.filterMode.exclude',
}

export const agentDisplayLabelKeys: Record<AgentDisplayFormat, I18nKey> = {
  id: 'settings.agentDisplay.id',
  name: 'settings.agentDisplay.name',
  'id:name': 'settings.agentDisplay.id:name',
}
```

> 确认：上面 5 个映射在 `en.ts` / `zh.ts` 中对应 key 均已存在（已 grep 核对）。

### 2.6　替换 6 处类型规避

| 文件:行号 | 现状 | 改法 |
|---|---|---|
| `src/components/InspectDrawer.tsx:29-31` | `stateLabel(state, t: (key: any) => string)` | 直接删 `stateLabel` 函数，调用处（第 105 行附近）改 `t(stateLabelKeys[selectedRun.state])`。**不要**加 `?? toUpperCase()` 回退——`t` 实现本身已有 `?? key` 回退（`i18n.tsx:668`），外层再加一层 `??` 永远触发不到，是死代码 |
| `src/components/monitor/MonitorView.tsx:105-106` | ``const stateKey = `state.${run.state}` as any`` | `const stateLabel = t(stateLabelKeys[run.state])`；删掉 `stateKey` 中间变量 |
| `src/components/monitor/settings/AppearanceSection.tsx:92` | `t(\`settings.uiDensity.${density}\` as never)` | `t(uiDensityLabelKeys[density])` |
| `src/components/monitor/settings/AppearanceSection.tsx:104` | `as any` | `t(fontSizeLabelKeys[size])` |
| `src/components/monitor/settings/FilterSection.tsx:60` | `as any` | `t(filterModeLabelKeys[mode])` |
| `src/components/monitor/settings/MonitorSection.tsx:141` | `as any` | `t(agentDisplayLabelKeys[fmt])` |

### 2.7　`migratePanelConfig` 去重

文件 `src/lib/preferences.ts:83-94`：
```ts
export function migratePanelConfig(panels: PanelEntry[] | undefined): PanelEntry[] {
  if (!Array.isArray(panels) || panels.length === 0) {
    return defaultPanelConfig.map((entry) => ({ ...entry }))
  }
  const seen = new Set<ToolKind>()
  const existing: PanelEntry[] = []
  for (const entry of panels) {
    if (!allTools.includes(entry.tool)) continue
    if (seen.has(entry.tool)) continue
    seen.add(entry.tool)
    existing.push({ ...entry })
  }
  const missing = allTools.filter((tool) => !seen.has(tool))
  return [...existing, ...missing.map((tool) => ({ tool, enabled: true }))]
}
```

`preferences.test.ts` 若已覆盖 migratePanelConfig 则确保通过；否则补一个"重复 tool 入口应去重"的测试。

### 2.8　全量自测

```bash
pnpm --filter @octomonitor/web test --run
pnpm --filter @octomonitor/web build   # 含 tsc -b
pnpm --filter @octomonitor/web test:a11y   # 可选；若耗时过长则跳过
```

预期：`grep -R "as any" apps/web/src --include='*.ts*' | grep -v vitest.setup.ts` 仅返回 0 行（或**不**包括 `state.` / `settings.` 前缀）。

### 2.9　Review → Commit

自审清单：
- [ ] `HeatmapView` 编译通过且空数据无运行期错误
- [ ] 6 处 `as any` / `as never` 全部删除
- [ ] `i18nMaps.ts` 所有成员都能在 `en` / `zh` 里找到对应 key（若缺失，tsc 本身不会报——这是运行期 fallback，需人工核对）
- [ ] `App.tsx` 不再出现 `ReturnType<typeof getRuntimeMode>`
- [ ] 新增 heatmap 空集合测试通过
- [ ] migratePanelConfig 去重行为验证（手动 localStorage 注入重复条目或单测）

```
git add -A && git commit -m "refactor: tighten types around heatmap summary and i18n dynamic keys"
```

---

## 风险与回滚

| Phase | 主要风险 | 检测手段 | 回滚策略 |
|---|---|---|---|
| 1 | `console.warn` 误入被某 e2e 断言捕获 | `pnpm test:a11y` | 在 catch 处加条件 `if (import.meta.env.DEV)` |
| 1 | 常量替换遗漏（仍有硬编码字符串） | `grep 'octomonitor-'`/`grep 'octomonitor:'` | 补漏，或 `git revert` 单个 Phase commit |
| 2 | `HeatmapSummary` optional 化导致其他未搜到的消费点编译失败 | `tsc -b` | 在该消费点加 `?.` 或 `!` 断言 |
| 2 | `stateLabelKeys` 等映射漏成员（若未来枚举扩容未同步） | `tsc -b` 会在 `Record<RunState, I18nKey>` 处报错 | 补成员 |
| 2 | `migratePanelConfig` 去重后某用户的旧配置丢项 | 新加测试 + 手测 | 保守策略：仅去重，不删保留不在 `allTools` 的未来扩展项（本方案已这么做） |

---

## 明确不做

- 不引入 toast / 远程日志。
- 不抽象 `useAsyncResource`（方案 §3.6 已决定）。
- 不拆分 `CommitsView` / `HeatmapView` / `MonitorView` / `i18n.tsx`。
- 不触及 `styles.css`。
- 不新增 `safeStorage` 独立模块。try/catch 就近写。
