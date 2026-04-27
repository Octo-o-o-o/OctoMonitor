import { create } from 'zustand'
import type { AppConfig, BootstrapPayload, RunRecord } from '../lib/types'
import {
  loadFrontendSettings,
  saveFrontendSettings,
  type FrontendSettings,
} from '../lib/preferences'
import { STORAGE_KEYS } from '../lib/storageKeys'

function loadDismissedAttentionKeys(): string[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.dismissedAttentions)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

export type ActiveTab = 'monitor' | 'usage' | 'commits' | 'heatmap' | 'settings'
export type {
  AgentDisplayFormat,
  ColumnLayout,
  FilterMode,
  FilterRules,
  FontSize,
  MonitorPeriod,
  PanelEntry,
  SnapshotWindow,
  ToolFilter,
  UiDensity,
} from '../lib/preferences'

export type ConnectionStatus = 'connecting' | 'live' | 'offline'

export type MonitorQuickFilter = 'all' | 'attention' | 'active'

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
  monitorQuickFilter: MonitorQuickFilter
  monitorSearch: string
  setData: (data: BootstrapPayload | null) => void
  setConfig: (config: AppConfig) => void
  selectRun: (id?: string) => void
  setFocusedRunId: (id?: string) => void
  setConnectionStatus: (status: ConnectionStatus) => void
  toggleShortcutHelp: () => void
  acknowledgeError: (id: string) => void
  dismissAttention: (id: string) => void
  setActiveTab: (tab: ActiveTab) => void
  updateSettings: (patch: Partial<FrontendSettings>) => void
  setMonitorQuickFilter: (filter: MonitorQuickFilter) => void
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
  monitorQuickFilter: 'all',
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
  setActiveTab: (activeTab) => set({ activeTab }),
  updateSettings: (patch) => {
    const next = { ...get().settings, ...patch }
    saveFrontendSettings(next)
    set({ settings: next })
  },
  setMonitorQuickFilter: (monitorQuickFilter) => set({ monitorQuickFilter }),
  setMonitorSearch: (monitorSearch) => set({ monitorSearch }),
}))

/** Selector: derive the selected run from data + selectedRunId */
export const selectSelectedRun = (s: MonitorState): RunRecord | undefined =>
  s.selectedRunId == null ? undefined : s.data?.runs.find((r) => r.id === s.selectedRunId)
