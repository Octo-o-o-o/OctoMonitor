import { allTools } from './constants'
import { STORAGE_KEYS } from './storageKeys'
import type { ToolKind } from './types'

export type MonitorPeriod = '30m' | '1h' | '2h' | '4h' | '8h' | '24h'
export type SnapshotWindow = 'day' | 'week' | 'month' | 'all'
export type UiDensity = 'compact' | 'comfortable' | 'spacious'
export type ColumnLayout = 'fixed' | 'adaptive'
export type AgentDisplayFormat = 'id' | 'name' | 'id:name'
export type FontSize = 'xsmall' | 'small' | 'default' | 'large' | 'xlarge'
export type FilterMode = 'off' | 'include' | 'exclude'
export type DesktopDisplayMode = 'dashboard' | 'island' | 'both'
export type IslandPosition = 'auto' | 'topCenter'

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
  desktopDisplayMode: DesktopDisplayMode
  islandPosition: IslandPosition
}

interface StoredFrontendSettings extends Partial<FrontendSettings> {
  version?: number
  companionEnabled?: boolean
  usageWindow?: 'live' | 'day' | 'week' | 'month' | 'all'
}

const SETTINGS_VERSION = 4
const allSnapshotWindows: SnapshotWindow[] = ['day', 'week', 'month', 'all']
const allDesktopDisplayModes: DesktopDisplayMode[] = ['dashboard', 'island', 'both']
const allIslandPositions: IslandPosition[] = ['auto', 'topCenter']

export const defaultPanelConfig: PanelEntry[] = [
  ...allTools.map((tool) => ({ tool, enabled: true })),
]

export const defaultFilterRules: FilterRules = Object.fromEntries(
  allTools.map((tool) => [tool, { mode: 'off', patterns: [] }]),
) as unknown as FilterRules

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
  desktopDisplayMode: 'both',
  islandPosition: 'auto',
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

  const seen = new Set<ToolKind>()
  const existing: PanelEntry[] = []
  for (const entry of panels) {
    if (!allTools.includes(entry.tool)) continue
    if (seen.has(entry.tool)) continue
    seen.add(entry.tool)
    existing.push({ ...entry })
  }
  const missing = allTools.filter((tool) => !seen.has(tool))
  return [
    ...existing,
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
  return {
    ...defaultSettings,
    panelConfig: defaultPanelConfig.map((entry) => ({ ...entry })),
    filterRules: cloneDefaultFilterRules(),
  }
}

function migrateDesktopDisplayMode(value: DesktopDisplayMode | undefined): DesktopDisplayMode {
  return value && allDesktopDisplayModes.includes(value)
    ? value
    : defaultSettings.desktopDisplayMode
}

function migrateIslandPosition(value: IslandPosition | undefined): IslandPosition {
  return value && allIslandPositions.includes(value)
    ? value
    : defaultSettings.islandPosition
}

export function loadFrontendSettings(): FrontendSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.settings)
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
      desktopDisplayMode: migrateDesktopDisplayMode(parsed.desktopDisplayMode),
      islandPosition: migrateIslandPosition(parsed.islandPosition),
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
  try {
    localStorage.setItem(STORAGE_KEYS.settings, JSON.stringify(payload))
  } catch (err) {
    console.warn('[OctoMonitor] storage.write.settings', err)
  }
}
