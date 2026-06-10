import { create } from 'zustand'
import type { AppConfig, BootstrapPayload, RunRecord, ToolKind } from '../lib/types'
import {
  loadFrontendSettings,
  saveFrontendSettings,
  type FrontendSettings,
} from '../lib/preferences'
import { STORAGE_KEYS } from '../lib/storageKeys'
import { applyDesktopDisplaySettings } from '../lib/desktopDisplay'

function loadDismissedAttentionKeys(): string[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.dismissedAttentions)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

const VISITED_RUN_LIMIT = 1000

function parseStringArray(raw: string | null): string[] {
  if (!raw) return []
  try {
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter((value): value is string => typeof value === 'string' && value !== '')
  } catch {
    return []
  }
}

function loadVisitedRunIds(): string[] {
  try {
    return parseStringArray(localStorage.getItem(STORAGE_KEYS.visitedRuns)).slice(-VISITED_RUN_LIMIT)
  } catch {
    return []
  }
}

function nextVisitedRunIds(current: Iterable<string>, id: string): string[] {
  const ordered = [...current].filter((value) => value !== id)
  ordered.push(id)
  return ordered.slice(-VISITED_RUN_LIMIT)
}

export type ActiveTab = 'monitor' | 'usage' | 'commits' | 'heatmap' | 'settings'
export type {
  AgentDisplayFormat,
  ColumnLayout,
  DesktopDisplayMode,
  FilterMode,
  FilterRules,
  FontSize,
  IslandPosition,
  MonitorPeriod,
  PanelEntry,
  SnapshotWindow,
  ToolFilter,
  UiDensity,
} from '../lib/preferences'

export type ConnectionStatus = 'connecting' | 'live' | 'offline'

export type MonitorQuickFilter = 'all' | 'attention' | 'active'
export type MonitorToolFilter = 'all' | ToolKind

interface MonitorState {
  data: BootstrapPayload | null
  selectedRunId?: string
  focusedRunId?: string
  connectionStatus: ConnectionStatus
  activeTab: ActiveTab
  showShortcutHelp: boolean
  settings: FrontendSettings
  acknowledgedErrors: Set<string>
  dismissedAttentionKeys: Set<string>
  visitedRunIds: Set<string>
  monitorQuickFilter: MonitorQuickFilter
  monitorToolFilter: MonitorToolFilter
  monitorSearch: string
  setData: (data: BootstrapPayload | null) => void
  setConfig: (config: AppConfig) => void
  selectRun: (id?: string) => void
  setFocusedRunId: (id?: string) => void
  setConnectionStatus: (status: ConnectionStatus) => void
  toggleShortcutHelp: () => void
  acknowledgeError: (id: string) => void
  dismissAttention: (id: string) => void
  markRunVisited: (id: string) => void
  syncVisitedRunsFromStorage: () => void
  setActiveTab: (tab: ActiveTab) => void
  updateSettings: (patch: Partial<FrontendSettings>) => void
  setMonitorQuickFilter: (filter: MonitorQuickFilter) => void
  setMonitorToolFilter: (filter: MonitorToolFilter) => void
  setMonitorSearch: (value: string) => void
}

export const useMonitorStore = create<MonitorState>((set, get) => ({
  data: null,
  connectionStatus: 'connecting',
  activeTab: 'monitor',
  showShortcutHelp: false,
  settings: loadFrontendSettings(),
  acknowledgedErrors: new Set<string>(),
  dismissedAttentionKeys: new Set<string>(loadDismissedAttentionKeys()),
  visitedRunIds: new Set<string>(loadVisitedRunIds()),
  monitorQuickFilter: 'all',
  monitorToolFilter: 'all',
  monitorSearch: '',
  setData: (data) => set({ data }),
  setConfig: (config) => set((s) => (s.data ? { data: { ...s.data, config } } : {})),
  selectRun: (selectedRunId) => set({ selectedRunId }),
  setFocusedRunId: (focusedRunId) => set({ focusedRunId }),
  setConnectionStatus: (connectionStatus) => set({ connectionStatus }),
  toggleShortcutHelp: () => set((s) => ({ showShortcutHelp: !s.showShortcutHelp })),
  acknowledgeError: (id) => set((s) => {
    const next = new Set(s.acknowledgedErrors)
    next.add(id)
    return { acknowledgedErrors: next }
  }),
  dismissAttention: (id) => set((s) => {
    const next = new Set(s.dismissedAttentionKeys)
    next.add(id)
    try {
      localStorage.setItem(STORAGE_KEYS.dismissedAttentions, JSON.stringify([...next]))
    } catch (err) {
      console.warn('[OctoMonitor] storage.write.dismissedAttentions', err)
    }
    return { dismissedAttentionKeys: next }
  }),
  markRunVisited: (id) => set((s) => {
    const stored = loadVisitedRunIds()
    const source = stored.length > 0 ? stored : s.visitedRunIds
    const ordered = nextVisitedRunIds(source, id)
    try {
      localStorage.setItem(STORAGE_KEYS.visitedRuns, JSON.stringify(ordered))
    } catch (err) {
      console.warn('[OctoMonitor] storage.write.visitedRuns', err)
    }
    return { visitedRunIds: new Set(ordered) }
  }),
  syncVisitedRunsFromStorage: () => set({
    visitedRunIds: new Set<string>(loadVisitedRunIds()),
  }),
  setActiveTab: (activeTab) => set({ activeTab }),
  updateSettings: (patch) => {
    const next = { ...get().settings, ...patch }
    saveFrontendSettings(next)
    set({ settings: next })
    if ('desktopDisplayMode' in patch || 'islandPosition' in patch) {
      void applyDesktopDisplaySettings({
        mode: next.desktopDisplayMode,
        position: next.islandPosition,
      }).catch((err) => {
        console.warn('[OctoMonitor] desktop.displayMode', err)
      })
    }
  },
  setMonitorQuickFilter: (monitorQuickFilter) => set({ monitorQuickFilter }),
  setMonitorToolFilter: (monitorToolFilter) => set({ monitorToolFilter }),
  setMonitorSearch: (monitorSearch) => set({ monitorSearch }),
}))

/** Selector: derive the selected run from data + selectedRunId */
export const selectSelectedRun = (s: MonitorState): RunRecord | undefined =>
  s.selectedRunId == null ? undefined : s.data?.runs.find((r) => r.id === s.selectedRunId)

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (event) => {
    if (event.key === STORAGE_KEYS.visitedRuns) {
      useMonitorStore.getState().syncVisitedRunsFromStorage()
    }
  })
}
