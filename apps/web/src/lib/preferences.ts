import { allTools } from './constants'
import type { ToolKind } from './types'

export type MonitorPeriod = '30m' | '1h' | '2h' | '4h' | '8h' | '24h'
export type SnapshotWindow = 'day' | 'week' | 'month' | 'all'
export type UiDensity = 'compact' | 'comfortable' | 'spacious'
export type ColumnLayout = 'fixed' | 'adaptive'
export type AgentDisplayFormat = 'id' | 'name' | 'id:name'
export type FontSize = 'xsmall' | 'small' | 'default' | 'large' | 'xlarge'
export type FilterMode = 'off' | 'include' | 'exclude'

export interface PanelEntry {
  tool: ToolKind
  enabled: boolean
}

export interface ToolFilter {
  mode: FilterMode
  patterns: string[]
}

export type FilterRules = Record<ToolKind, ToolFilter>

export interface FrontendSettings {
  monitorPeriod: MonitorPeriod
  snapshotWindow: SnapshotWindow
  uiDensity: UiDensity
  columnLayout: ColumnLayout
  showFingerprints: boolean
  panelConfig: PanelEntry[]
  filterRules: FilterRules
  agentDisplayFormat: AgentDisplayFormat
  fontSize: FontSize
  notificationsEnabled: boolean
}

interface StoredFrontendSettings extends Partial<FrontendSettings> {
  version?: number
  companionEnabled?: boolean
  usageWindow?: 'live' | 'day' | 'week' | 'month' | 'all'
}

const STORAGE_KEY = 'octomonitor-settings'
const SETTINGS_VERSION = 3
const allSnapshotWindows: SnapshotWindow[] = ['day', 'week', 'month', 'all']

export const defaultPanelConfig: PanelEntry[] = [
  { tool: 'claude', enabled: true },
  { tool: 'codex', enabled: true },
  { tool: 'openClaw', enabled: true },
  { tool: 'hermes', enabled: true },
]

export const defaultFilterRules: FilterRules = {
  claude: { mode: 'off', patterns: [] },
  codex: { mode: 'off', patterns: [] },
  openClaw: { mode: 'off', patterns: [] },
  hermes: { mode: 'off', patterns: [] },
}

export const defaultSettings: FrontendSettings = {
  monitorPeriod: '1h',
  snapshotWindow: 'week',
  uiDensity: 'comfortable',
  columnLayout: 'fixed',
  showFingerprints: true,
  panelConfig: defaultPanelConfig,
  filterRules: defaultFilterRules,
  agentDisplayFormat: 'id',
  fontSize: 'default',
  notificationsEnabled: false,
}

function cloneDefaultFilterRules(): FilterRules {
  const clone: Partial<FilterRules> = {}
  for (const tool of allTools) {
    const src = defaultFilterRules[tool]
    clone[tool] = { mode: src.mode, patterns: [...src.patterns] }
  }
  return clone as FilterRules
}

export function migratePanelConfig(panels: PanelEntry[] | undefined): PanelEntry[] {
  if (!Array.isArray(panels) || panels.length === 0) {
    return defaultPanelConfig.map((entry) => ({ ...entry }))
  }

  const existing = panels.filter((entry) => allTools.includes(entry.tool))
  const missing = allTools.filter((tool) => !existing.some((entry) => entry.tool === tool))
  return [
    ...existing.map((entry) => ({ ...entry })),
    ...missing.map((tool) => ({ tool, enabled: true })),
  ]
}

function migrateFilterRules(filterRules: FilterRules | undefined): FilterRules {
  const merged = cloneDefaultFilterRules()
  if (!filterRules) return merged

  for (const tool of allTools) {
    const incoming = filterRules[tool]
    if (!incoming) continue
    merged[tool] = {
      mode: incoming.mode ?? 'off',
      patterns: Array.isArray(incoming.patterns) ? [...incoming.patterns] : [],
    }
  }

  return merged
}

function migrateSnapshotWindow(
  snapshotWindow: SnapshotWindow | undefined,
  legacyUsageWindow: StoredFrontendSettings['usageWindow'],
): SnapshotWindow {
  if (snapshotWindow && allSnapshotWindows.includes(snapshotWindow)) {
    return snapshotWindow
  }

  switch (legacyUsageWindow) {
    case 'day':
    case 'week':
    case 'month':
    case 'all':
      return legacyUsageWindow
    case 'live':
      return 'day'
    default:
      return defaultSettings.snapshotWindow
  }
}

function freshDefaults(): FrontendSettings {
  return { ...defaultSettings, panelConfig: migratePanelConfig(undefined), filterRules: cloneDefaultFilterRules() }
}

export function loadFrontendSettings(): FrontendSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return freshDefaults()

    const parsed = JSON.parse(raw) as StoredFrontendSettings
    return {
      monitorPeriod: parsed.monitorPeriod ?? defaultSettings.monitorPeriod,
      snapshotWindow: migrateSnapshotWindow(parsed.snapshotWindow, parsed.usageWindow),
      uiDensity: parsed.uiDensity ?? defaultSettings.uiDensity,
      columnLayout: parsed.columnLayout ?? defaultSettings.columnLayout,
      showFingerprints: parsed.showFingerprints ?? defaultSettings.showFingerprints,
      panelConfig: migratePanelConfig(parsed.panelConfig),
      filterRules: migrateFilterRules(parsed.filterRules),
      agentDisplayFormat: parsed.agentDisplayFormat ?? defaultSettings.agentDisplayFormat,
      fontSize: parsed.fontSize ?? defaultSettings.fontSize,
      notificationsEnabled: parsed.notificationsEnabled ?? defaultSettings.notificationsEnabled,
    }
  } catch {
    return freshDefaults()
  }
}

export function saveFrontendSettings(settings: FrontendSettings) {
  const payload = {
    version: SETTINGS_VERSION,
    ...settings,
  }
  localStorage.setItem(STORAGE_KEY, JSON.stringify(payload))
}
