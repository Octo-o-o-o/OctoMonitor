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
import { RemotePairingGate } from './components/RemotePairingGate'
import { apiFetch, buildWsUrl, normalizeBootstrapPayload } from './lib/api'
import { buildVisiblePanels, buildVisibleRunIds, buildVisibleRunsBySource } from './lib/monitor'
import { getRuntimeMode } from './lib/runtimeMode'
import { useI18n, type I18nKey } from './lib/i18n'

type DesktopBootIssue = {
  title: string
  message: string
}

type DesktopBootWindow = Window & {
  __OCTOMONITOR_DESKTOP_BOOT__?: DesktopBootIssue | null
}

const DESKTOP_BOOT_EVENT = 'octomonitor:desktop-boot-status'

function readDesktopBootIssue(): DesktopBootIssue | null {
  const bootIssue = (window as DesktopBootWindow).__OCTOMONITOR_DESKTOP_BOOT__
  if (!bootIssue || typeof bootIssue.message !== 'string') return null
  return bootIssue
}

function useWebSocket(
  enabled: boolean,
  onMessage: (data: unknown) => void,
  onStatusChange: (connected: boolean) => void,
) {
  const [connected, setConnected] = useState(false)
  const retryRef = useRef(0)
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined)

  useEffect(() => {
    const state = { socket: null as WebSocket | null, unmounted: false }
    if (!enabled) {
      setConnected(false)
      return () => {
        state.unmounted = true
      }
    }

    function connect() {
      if (state.unmounted) return
      const ws = new WebSocket(buildWsUrl('/api/stream'))
      state.socket = ws

      ws.onopen = () => {
        retryRef.current = 0
        setConnected(true)
        onStatusChange(true)
      }
      ws.onmessage = (event) => {
        try {
          const parsed = JSON.parse(event.data)
          if (parsed.type === 'snapshot.replace' && parsed.payload) onMessage(parsed.payload)
        } catch {
          // ignore malformed frames
        }
      }
      ws.onclose = () => {
        setConnected(false)
        onStatusChange(false)
        if (state.unmounted) return
        const delay = Math.min(1000 * 2 ** retryRef.current, 30_000)
        retryRef.current++
        timerRef.current = setTimeout(connect, delay)
      }
      ws.onerror = () => {
        ws.close()
      }
    }

    connect()
    return () => {
      state.unmounted = true
      clearTimeout(timerRef.current)
      state.socket?.close()
    }
  }, [enabled, onMessage, onStatusChange])

  return connected
}

function TabContent({ runtimeMode, tab }: { runtimeMode: ReturnType<typeof getRuntimeMode>; tab: string }) {
  switch (tab) {
    case 'monitor': return <MonitorView />
    case 'usage': return <UsageView />
    case 'commits': return <CommitsView />
    case 'heatmap': return <HeatmapView />
    case 'settings': return runtimeMode === 'local' ? <SettingsView /> : <MonitorView />
    default: return <MonitorView />
  }
}

function useKeyboardShortcuts(runtimeMode: ReturnType<typeof getRuntimeMode>, activeTab: string) {
  const setActiveTab = useMonitorStore((s) => s.setActiveTab)
  const data = useMonitorStore((s) => s.data)
  const monitorPeriod = useMonitorStore((s) => s.settings.monitorPeriod)
  const panelConfig = useMonitorStore((s) => s.settings.panelConfig)
  const filterRules = useMonitorStore((s) => s.settings.filterRules)
  const agentDisplayFormat = useMonitorStore((s) => s.settings.agentDisplayFormat)
  const focusedRunId = useMonitorStore((s) => s.focusedRunId)
  const setFocusedRunId = useMonitorStore((s) => s.setFocusedRunId)
  const selectRun = useMonitorStore((s) => s.selectRun)
  const toggleShortcutHelp = useMonitorStore((s) => s.toggleShortcutHelp)

  const runIds = useMemo(() => {
    if (!data || activeTab !== 'monitor') return []
    const visiblePanels = buildVisiblePanels(panelConfig)
    const sessionsBySource = buildVisibleRunsBySource(data.runs, filterRules, monitorPeriod)
    return buildVisibleRunIds(sessionsBySource, visiblePanels, agentDisplayFormat)
  }, [activeTab, agentDisplayFormat, data, filterRules, monitorPeriod, panelConfig])

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement).tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return

      if (!e.ctrlKey && !e.metaKey && !e.altKey) {
        switch (e.key) {
          case '1': setActiveTab('monitor'); break
          case '2': setActiveTab('usage'); break
          case '3': setActiveTab('commits'); break
          case '4': setActiveTab('heatmap'); break
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
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [runtimeMode, activeTab, setActiveTab, runIds, focusedRunId, setFocusedRunId, selectRun, toggleShortcutHelp])
}

function useWaitingNotifications(enabled: boolean, t: (key: I18nKey) => string) {
  const prevWaitingRef = useRef<Set<string>>(new Set())

  return useCallback((data: BootstrapPayload) => {
    const currentWaiting = new Set(
      data.runs.filter((r) => r.state === 'waitingApproval').map((r) => r.id)
    )
    // Find newly waiting sessions (not in previous set)
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
  const setConnectionStatus = useMonitorStore((s) => s.setConnectionStatus)
  const setActiveTab = useMonitorStore((s) => s.setActiveTab)
  const activeTab = useMonitorStore((s) => s.activeTab)
  const connectionStatus = useMonitorStore((s) => s.connectionStatus)
  const notificationsEnabled = useMonitorStore((s) => s.settings.notificationsEnabled)
  const fontSize = useMonitorStore((s) => s.settings.fontSize)
  const uiDensity = useMonitorStore((s) => s.settings.uiDensity)
  const { t } = useI18n()
  const [remoteAuthState, setRemoteAuthState] = useState<'checking' | 'required' | 'ready'>(
    runtimeMode === 'remoteViewer' ? 'checking' : 'ready',
  )
  const [authCheckNonce, setAuthCheckNonce] = useState(0)
  const [desktopBootIssue, setDesktopBootIssue] = useState<DesktopBootIssue | null>(() => {
    if (typeof window === 'undefined' || runtimeMode !== 'local') return null
    return readDesktopBootIssue()
  })
  const checkWaitingNotifications = useWaitingNotifications(notificationsEnabled, t)
  const handleWsMessage = useCallback((payload: unknown) => {
    const data = normalizeBootstrapPayload(payload)
    setData(data)
    setConnectionStatus(data.generatedAt ? 'live' : 'connecting')
    checkWaitingNotifications(data)
  }, [setConnectionStatus, setData, checkWaitingNotifications])
  const handleConnectionChange = useCallback((connected: boolean) => {
    setConnectionStatus(connected ? 'connecting' : 'offline')
  }, [setConnectionStatus])
  const wsConnected = useWebSocket(
    runtimeMode === 'local' || remoteAuthState === 'ready',
    handleWsMessage,
    handleConnectionChange,
  )
  useKeyboardShortcuts(runtimeMode, activeTab)

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
    if (runtimeMode === 'remoteViewer' && activeTab === 'settings') {
      setActiveTab('monitor')
    }
  }, [activeTab, runtimeMode, setActiveTab])

  useEffect(() => {
    if (fontSize === 'default') {
      document.documentElement.removeAttribute('data-fontsize')
    } else {
      document.documentElement.setAttribute('data-fontsize', fontSize)
    }
  }, [fontSize])

  useEffect(() => {
    if (uiDensity === 'comfortable') {
      document.documentElement.removeAttribute('data-density')
    } else {
      document.documentElement.setAttribute('data-density', uiDensity)
    }
  }, [uiDensity])

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
        {runtimeMode === 'remoteViewer' && remoteAuthState === 'required' ? (
          <RemotePairingGate onPaired={() => {
            setRemoteAuthState('checking')
            setData(null)
            setConnectionStatus('connecting')
            setAuthCheckNonce((value) => value + 1)
          }} />
        ) : (
          <TabContent runtimeMode={runtimeMode} tab={activeTab} />
        )}
      </main>
      <InspectDrawer />
      <ShortcutOverlay />
    </div>
  )
}
