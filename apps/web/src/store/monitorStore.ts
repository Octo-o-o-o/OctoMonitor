import { create } from 'zustand'
import type { AppConfig, BootstrapPayload, RunRecord } from '../lib/types'
import {
  loadFrontendSettings,
  saveFrontendSettings,
  type FrontendSettings,
} from '../lib/preferences'

const DISMISSED_ATTENTIONS_KEY = 'octomonitor-dismissed-attentions-v2'

function loadDismissedAttentionKeys(): string[] {
  try {
    const raw = localStorage.getItem(DISMISSED_ATTENTIONS_KEY)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

export type ActiveTab = 'monitor' | 'usage' | 'commits' | 'settings'
export type {
  AgentDisplayFormat,
  ColumnLayout,
  FilterMode,
  FilterRules,
  FontSize,
  MonitorPeriod,
  PanelEntry,
  ToolFilter,
  UiDensity,
  UsageWindow,
} from '../lib/preferences'

export type ConnectionStatus = 'connecting' | 'live' | 'offline'

interface MonitorState {
  data: BootstrapPayload | null
  selectedRunId?: string
  focusedRunId?: string
  connectionStatus: ConnectionStatus
  activeTab: ActiveTab
  settingsOpen: boolean
  showShortcutHelp: boolean
  settings: FrontendSettings
  acknowledgedErrors: Set<string>
  dismissedAttentionKeys: Set<string>
  setData: (data: BootstrapPayload | null) => void
  setConfig: (config: AppConfig) => void
  selectRun: (id?: string) => void
  setFocusedRunId: (id?: string) => void
  setConnectionStatus: (status: ConnectionStatus) => void
  toggleShortcutHelp: () => void
  acknowledgeError: (id: string) => void
  dismissAttention: (id: string) => void
  setActiveTab: (tab: ActiveTab) => void
  setSettingsOpen: (open: boolean) => void
  updateSettings: (patch: Partial<FrontendSettings>) => void
}

export const useMonitorStore = create<MonitorState>((set, get) => ({
  data: null,
  connectionStatus: 'connecting',
  activeTab: 'monitor',
  settingsOpen: false,
  showShortcutHelp: false,
  settings: loadFrontendSettings(),
  acknowledgedErrors: new Set<string>(),
  dismissedAttentionKeys: new Set<string>(loadDismissedAttentionKeys()),
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
    localStorage.setItem(DISMISSED_ATTENTIONS_KEY, JSON.stringify([...next]))
    return { dismissedAttentionKeys: next }
  }),
  setActiveTab: (activeTab) => set({ activeTab }),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  updateSettings: (patch) => {
    const next = { ...get().settings, ...patch }
    saveFrontendSettings(next)
    set({ settings: next })
  },
}))

/** Selector: derive the selected run from data + selectedRunId */
export const selectSelectedRun = (s: MonitorState): RunRecord | undefined =>
  s.data?.runs.find((r) => r.id === s.selectedRunId)
