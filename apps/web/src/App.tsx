import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { BootstrapPayload } from './lib/types'
import { useMonitorStore } from './store/monitorStore'
import { StatusBar } from './components/monitor/StatusBar'
import { MonitorView } from './components/monitor/MonitorView'
import { UsageView } from './components/monitor/UsageView'
import { CommitsView } from './components/monitor/CommitsView'
import { HeatmapView } from './components/monitor/HeatmapView'
import { SettingsView } from './components/monitor/SettingsView'
import { InspectDrawer } from './components/InspectDrawer'
import { ShortcutOverlay } from './components/ShortcutOverlay'
import { ToastContainer } from './components/common/Toast'
import { RemotePairingGate } from './components/RemotePairingGate'
import { LoadingScreen } from './components/LoadingScreen'
import { apiFetch, normalizeBootstrapPayload } from './lib/api'
import { DESKTOP_BOOT_EVENT, DESKTOP_MENU_ACTION_EVENT } from './lib/desktopEvents'
import {
  applyDesktopZoom,
  loadDesktopZoom,
  nextDesktopZoom,
  saveDesktopZoom,
  type DesktopZoomAction,
} from './lib/desktopZoom'
import { applyMonitorFilters } from './lib/monitorFilters'
import { buildVisiblePanels, buildVisibleRunsBySource, flattenVisibleRunsBySource } from './lib/monitor'
import { isTauriEnvironment } from './lib/runtimeEnvironment'
import { getRuntimeMode, type RuntimeMode } from './lib/runtimeMode'
import { useI18n, type I18nKey } from './lib/i18n'
import { useLiveSnapshot } from './lib/useLiveSnapshot'
import { applyDesktopDisplaySettings } from './lib/desktopDisplay'

type DesktopBootIssue = {
  title: string
  message: string
}

type DesktopBootWindow = Window & {
  __OCTOMONITOR_DESKTOP_BOOT__?: DesktopBootIssue | null
}

type DesktopMenuAction =
  | 'open-settings'
  | 'toggle-shortcuts'
  | 'zoom-in'
  | 'zoom-out'
  | 'zoom-reset'

type DesktopMenuActionDetail = {
  action?: DesktopMenuAction
}

function readDesktopBootIssue(): DesktopBootIssue | null {
  const bootIssue = (window as DesktopBootWindow).__OCTOMONITOR_DESKTOP_BOOT__
  if (!bootIssue || typeof bootIssue.message !== 'string') return null
  return bootIssue
}

// Mirror a setting to <html data-*> while the default value uses no attribute,
// so themed CSS can target only non-default rows without an extra reset.
function useDocumentDataAttr(attr: string, value: string, defaultValue: string) {
  useEffect(() => {
    if (value === defaultValue) {
      document.documentElement.removeAttribute(attr)
    } else {
      document.documentElement.setAttribute(attr, value)
    }
  }, [attr, value, defaultValue])
}

function TabContent({ runtimeMode, tab }: { runtimeMode: RuntimeMode; tab: string }) {
  switch (tab) {
    case 'monitor': return <MonitorView />
    case 'usage': return <UsageView />
    case 'commits': return <CommitsView />
    case 'heatmap': return <HeatmapView />
    case 'settings': return runtimeMode === 'local' ? <SettingsView /> : <MonitorView />
    default: return <MonitorView />
  }
}

function useKeyboardShortcuts(runtimeMode: RuntimeMode, activeTab: string) {
  const setActiveTab = useMonitorStore((s) => s.setActiveTab)
  const data = useMonitorStore((s) => s.data)
  const monitorPeriod = useMonitorStore((s) => s.settings.monitorPeriod)
  const panelConfig = useMonitorStore((s) => s.settings.panelConfig)
  const filterRules = useMonitorStore((s) => s.settings.filterRules)
  const quickFilter = useMonitorStore((s) => s.monitorQuickFilter)
  const toolFilter = useMonitorStore((s) => s.monitorToolFilter)
  const searchQuery = useMonitorStore((s) => s.monitorSearch)
  const focusedRunId = useMonitorStore((s) => s.focusedRunId)
  const setFocusedRunId = useMonitorStore((s) => s.setFocusedRunId)
  const selectRun = useMonitorStore((s) => s.selectRun)
  const toggleShortcutHelp = useMonitorStore((s) => s.toggleShortcutHelp)

  const runIds = useMemo(() => {
    if (!data || activeTab !== 'monitor') return []
    const hidden = new Set(data.config.hiddenSources)
    const visiblePanels = buildVisiblePanels(panelConfig).filter((tool) => !hidden.has(tool))
    const effectiveToolFilter = toolFilter === 'all' || visiblePanels.includes(toolFilter)
      ? toolFilter
      : 'all'
    const sessionsBySource = buildVisibleRunsBySource(data.runs, filterRules, monitorPeriod)
    return applyMonitorFilters(
      flattenVisibleRunsBySource(sessionsBySource, visiblePanels),
      quickFilter,
      effectiveToolFilter,
      searchQuery,
    ).map((run) => run.id)
  }, [activeTab, data, filterRules, monitorPeriod, panelConfig, quickFilter, searchQuery, toolFilter])

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement).tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return

      if (!e.ctrlKey && !e.metaKey && !e.altKey) {
        switch (e.key) {
          case '1': setActiveTab('monitor'); break
          case '2': setActiveTab('usage'); break
          case '3':
            if (runtimeMode === 'local') setActiveTab('commits')
            break
          case '4':
            if (runtimeMode === 'local') setActiveTab('heatmap')
            break
          case '5':
            if (runtimeMode === 'local') setActiveTab('settings')
            break
          case 'j':
          case 'k': {
            if (runIds.length === 0) break
            const currentIdx = focusedRunId ? runIds.indexOf(focusedRunId) : -1
            let nextIdx: number
            if (e.key === 'j') {
              nextIdx = currentIdx < runIds.length - 1 ? currentIdx + 1 : 0
            } else {
              nextIdx = currentIdx > 0 ? currentIdx - 1 : runIds.length - 1
            }
            setFocusedRunId(runIds[nextIdx])
            break
          }
          case 'Enter': {
            if (focusedRunId) selectRun(focusedRunId)
            break
          }
          case '?': {
            toggleShortcutHelp()
            break
          }
          case '/': {
            if (activeTab !== 'monitor') break
            const searchInput = document.querySelector<HTMLInputElement>(
              '.monitor-filter-search',
            )
            if (searchInput) {
              e.preventDefault()
              searchInput.focus()
              searchInput.select()
            }
            break
          }
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [runtimeMode, activeTab, setActiveTab, runIds, focusedRunId, setFocusedRunId, selectRun, toggleShortcutHelp])
}

function toDesktopZoomAction(action: DesktopMenuAction): DesktopZoomAction | null {
  switch (action) {
    case 'zoom-in': return 'in'
    case 'zoom-out': return 'out'
    case 'zoom-reset': return 'reset'
    default: return null
  }
}

function useDesktopMenuActions(runtimeMode: RuntimeMode) {
  const setActiveTab = useMonitorStore((s) => s.setActiveTab)
  const toggleShortcutHelp = useMonitorStore((s) => s.toggleShortcutHelp)
  const zoomRef = useRef(1)

  useEffect(() => {
    if (!isTauriEnvironment() || runtimeMode !== 'local') return

    zoomRef.current = loadDesktopZoom()

    function handleDesktopMenuAction(event: Event) {
      const action = (event as CustomEvent<DesktopMenuActionDetail>).detail?.action
      if (!action) return

      if (action === 'open-settings') {
        setActiveTab('settings')
        return
      }
      if (action === 'toggle-shortcuts') {
        toggleShortcutHelp()
        return
      }

      const zoomAction = toDesktopZoomAction(action)
      if (!zoomAction) return

      zoomRef.current = nextDesktopZoom(zoomRef.current, zoomAction)
      saveDesktopZoom(zoomRef.current)
      void applyDesktopZoom(zoomRef.current)
    }

    window.addEventListener(DESKTOP_MENU_ACTION_EVENT, handleDesktopMenuAction)
    return () => window.removeEventListener(DESKTOP_MENU_ACTION_EVENT, handleDesktopMenuAction)
  }, [runtimeMode, setActiveTab, toggleShortcutHelp])
}

function useWaitingNotifications(enabled: boolean, t: (key: I18nKey) => string) {
  const prevWaitingRef = useRef<Set<string>>(new Set())

  return useCallback((data: BootstrapPayload) => {
    const currentWaiting = new Set(
      data.runs.filter((r) => r.state === 'waitingApproval').map((r) => r.id)
    )
    const newWaiting = data.runs.filter(
      (r) => r.state === 'waitingApproval' && !prevWaitingRef.current.has(r.id)
    )
    prevWaitingRef.current = currentWaiting

    if (newWaiting.length === 0) return
    if (!enabled) return
    if (!('Notification' in window)) return
    if (Notification.permission !== 'granted') return

    for (const run of newWaiting) {
      const title = t('notification.approvalTitle').replace('{project}', run.projectName)
      const body = run.lastQuestion ?? run.lastTail ?? t('notification.approvalBody')
      new Notification(title, { body, tag: `octomonitor-wait-${run.id}` })
    }
  }, [enabled, t])
}

export default function App() {
  const runtimeMode = getRuntimeMode()
  const setData = useMonitorStore((s) => s.setData)
  const data = useMonitorStore((s) => s.data)
  const setConnectionStatus = useMonitorStore((s) => s.setConnectionStatus)
  const setActiveTab = useMonitorStore((s) => s.setActiveTab)
  const activeTab = useMonitorStore((s) => s.activeTab)
  const connectionStatus = useMonitorStore((s) => s.connectionStatus)
  const notificationsEnabled = useMonitorStore((s) => s.settings.notificationsEnabled)
  const fontSize = useMonitorStore((s) => s.settings.fontSize)
  const uiDensity = useMonitorStore((s) => s.settings.uiDensity)
  const desktopDisplayMode = useMonitorStore((s) => s.settings.desktopDisplayMode)
  const islandPosition = useMonitorStore((s) => s.settings.islandPosition)
  const { t } = useI18n()
  const [remoteAuthState, setRemoteAuthState] = useState<'checking' | 'required' | 'ready'>(
    runtimeMode === 'remoteViewer' ? 'checking' : 'ready',
  )
  const [authCheckNonce, setAuthCheckNonce] = useState(0)
  const [desktopBootIssue, setDesktopBootIssue] = useState<DesktopBootIssue | null>(() => {
    if (typeof window === 'undefined' || runtimeMode !== 'local') return null
    return readDesktopBootIssue()
  })
  const desktopDisplayAppliedOnBootRef = useRef(false)
  const checkWaitingNotifications = useWaitingNotifications(notificationsEnabled, t)
  const handleWsMessage = useCallback((payload: unknown) => {
    const next = normalizeBootstrapPayload(payload)
    setData(next)
    setConnectionStatus(next.generatedAt ? 'live' : 'connecting')
    checkWaitingNotifications(next)
  }, [setConnectionStatus, setData, checkWaitingNotifications])
  const handleConnectionChange = useCallback((connected: boolean) => {
    setConnectionStatus(connected ? 'connecting' : 'offline')
  }, [setConnectionStatus])
  const wsConnected = useLiveSnapshot(
    runtimeMode === 'local' || remoteAuthState === 'ready',
    handleWsMessage,
    handleConnectionChange,
  )
  useKeyboardShortcuts(runtimeMode, activeTab)
  useDesktopMenuActions(runtimeMode)

  useEffect(() => {
    if (runtimeMode !== 'local') return
    if (desktopDisplayAppliedOnBootRef.current) return
    desktopDisplayAppliedOnBootRef.current = true
    void applyDesktopDisplaySettings({
      mode: desktopDisplayMode,
      position: islandPosition,
    }).catch((err) => {
      console.warn('[OctoMonitor] desktop.displayMode', err)
    })
  }, [desktopDisplayMode, islandPosition, runtimeMode])

  useEffect(() => {
    if (runtimeMode !== 'remoteViewer') return

    let cancelled = false

    async function checkRemoteViewerAccess() {
      try {
        const response = await apiFetch('/api/bootstrap')
        if (cancelled) return

        if (response.status === 401 || response.status === 403) {
          setRemoteAuthState('required')
          setData(null)
          setConnectionStatus('offline')
          return
        }
        if (!response.ok) return

        const payload = await response.json()
        if (cancelled) return
        setRemoteAuthState('ready')
        handleWsMessage(payload)
      } catch {
        // Remote auth check is best-effort; WS reconnect handles the paired case.
      }
    }

    void checkRemoteViewerAccess()
    return () => {
      cancelled = true
    }
  }, [authCheckNonce, handleWsMessage, runtimeMode, setConnectionStatus, setData])

  useEffect(() => {
    if (runtimeMode !== 'remoteViewer' || remoteAuthState !== 'ready' || connectionStatus !== 'offline') {
      return
    }

    let cancelled = false
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const response = await apiFetch('/api/bootstrap')
          if (cancelled) return

          if (response.status === 401 || response.status === 403) {
            setRemoteAuthState('required')
            setData(null)
            return
          }

          if (!response.ok) return
        } catch {
          // Best-effort auth recheck while WS reconnects.
        }
      })()
    }, 250)

    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [connectionStatus, remoteAuthState, runtimeMode, setData])

  useEffect(() => {
    if (runtimeMode === 'remoteViewer' && activeTab !== 'monitor' && activeTab !== 'usage') {
      setActiveTab('monitor')
    }
  }, [activeTab, runtimeMode, setActiveTab])

  useDocumentDataAttr('data-fontsize', fontSize, 'default')
  useDocumentDataAttr('data-density', uiDensity, 'comfortable')

  useEffect(() => {
    if (runtimeMode !== 'local') return

    function handleDesktopBootStatus(event: Event) {
      const detail = (event as CustomEvent<DesktopBootIssue | null>).detail
      setDesktopBootIssue(detail && typeof detail.message === 'string' ? detail : null)
    }

    window.addEventListener(DESKTOP_BOOT_EVENT, handleDesktopBootStatus)
    return () => window.removeEventListener(DESKTOP_BOOT_EVENT, handleDesktopBootStatus)
  }, [runtimeMode])

  const visibleDesktopBootIssue =
    runtimeMode === 'local' && connectionStatus !== 'live' ? desktopBootIssue : null

  function renderMain() {
    if (runtimeMode === 'remoteViewer' && remoteAuthState === 'required') {
      return (
        <RemotePairingGate onPaired={() => {
          setRemoteAuthState('checking')
          setData(null)
          setConnectionStatus('connecting')
          setAuthCheckNonce((value) => value + 1)
        }} />
      )
    }
    if (connectionStatus === 'connecting' && !data) {
      return <LoadingScreen />
    }
    return <TabContent runtimeMode={runtimeMode} tab={activeTab} />
  }

  return (
    <div className="app-shell">
      <StatusBar runtimeMode={runtimeMode} wsConnected={wsConnected} />
      {visibleDesktopBootIssue && (
        <div className="status-notice offline">
          <strong>{t('desktop.startupIssue')}</strong>
          <span>{visibleDesktopBootIssue.message}</span>
        </div>
      )}
      <main className={`main-content${activeTab === 'monitor' ? ' no-scroll' : ''}`}>
        {renderMain()}
      </main>
      <InspectDrawer />
      <ShortcutOverlay />
      <ToastContainer />
    </div>
  )
}
