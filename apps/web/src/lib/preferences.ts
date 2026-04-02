import type { ToolKind } from './types'

export type MonitorPeriod = '30m' | '1h' | '2h' | '4h' | '8h' | '24h'
export type UsageWindow = 'live' | 'day' | 'week' | 'month' | 'all'
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
  uiDensity: UiDensity
  columnLayout: ColumnLayout
  showFingerprints: boolean
  panelConfig: PanelEntry[]
  usageWindow: UsageWindow
  filterRules: FilterRules
  agentDisplayFormat: AgentDisplayFormat
  fontSize: FontSize
  notificationsEnabled: boolean
}

interface StoredFrontendSettings extends Partial<FrontendSettings> {
  version?: number
  companionEnabled?: boolean
}

const STORAGE_KEY = 'octomonitor-settings'
const SETTINGS_VERSION = 3
const allTools: ToolKind[] = ['claude', 'codex', 'openClaw']
const allUsageWindows: UsageWindow[] = ['live', 'day', 'week', 'month', 'all']

export const defaultPanelConfig: PanelEntry[] = [
  { tool: 'claude', enabled: true },
  { tool: 'codex', enabled: true },
  { tool: 'openClaw', enabled: true },
]

export const defaultFilterRules: FilterRules = {
  claude: { mode: 'off', patterns: [] },
  codex: { mode: 'off', patterns: [] },
  openClaw: { mode: 'off', patterns: [] },
}

export const defaultSettings: FrontendSettings = {
  monitorPeriod: '1h',
  uiDensity: 'comfortable',
  columnLayout: 'fixed',
  showFingerprints: true,
  panelConfig: defaultPanelConfig,
  usageWindow: 'all',
  filterRules: defaultFilterRules,
  agentDisplayFormat: 'id',
  fontSize: 'default',
  notificationsEnabled: false,
}

function cloneDefaultFilterRules(): FilterRules {
  return {
    claude: { ...defaultFilterRules.claude, patterns: [...defaultFilterRules.claude.patterns] },
    codex: { ...defaultFilterRules.codex, patterns: [...defaultFilterRules.codex.patterns] },
    openClaw: { ...defaultFilterRules.openClaw, patterns: [...defaultFilterRules.openClaw.patterns] },
  }
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

function migrateUsageWindow(
  usageWindow: UsageWindow | undefined,
  version: number | undefined,
): UsageWindow {
  if (version == null || version < 3) {
    return 'all'
  }

  if (usageWindow && allUsageWindows.includes(usageWindow)) {
    return usageWindow
  }

  return defaultSettings.usageWindow
}

export function loadFrontendSettings(): FrontendSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return { ...defaultSettings, panelConfig: migratePanelConfig(undefined), filterRules: cloneDefaultFilterRules() }

    const parsed = JSON.parse(raw) as StoredFrontendSettings
    return {
      monitorPeriod: parsed.monitorPeriod ?? defaultSettings.monitorPeriod,
      uiDensity: parsed.uiDensity ?? defaultSettings.uiDensity,
      columnLayout: parsed.columnLayout ?? defaultSettings.columnLayout,
      showFingerprints: parsed.showFingerprints ?? defaultSettings.showFingerprints,
      panelConfig: migratePanelConfig(parsed.panelConfig),
      usageWindow: migrateUsageWindow(parsed.usageWindow, parsed.version),
      filterRules: migrateFilterRules(parsed.filterRules),
      agentDisplayFormat: parsed.agentDisplayFormat ?? defaultSettings.agentDisplayFormat,
      fontSize: parsed.fontSize ?? defaultSettings.fontSize,
      notificationsEnabled: parsed.notificationsEnabled ?? defaultSettings.notificationsEnabled,
    }
  } catch {
    return { ...defaultSettings, panelConfig: migratePanelConfig(undefined), filterRules: cloneDefaultFilterRules() }
  }
}

export function saveFrontendSettings(settings: FrontendSettings) {
  const payload = {
    version: SETTINGS_VERSION,
    ...settings,
  }
  localStorage.setItem(STORAGE_KEY, JSON.stringify(payload))
}
