import { useCallback } from 'react'
import { ToastContainer } from '../components/common/Toast'
import { normalizeBootstrapPayload } from '../lib/api'
import { useLiveSnapshot } from '../lib/useLiveSnapshot'
import { useMonitorStore } from '../store/monitorStore'
import { IslandSurface } from './IslandSurface'

export default function IslandApp() {
  const setData = useMonitorStore((s) => s.setData)
  const data = useMonitorStore((s) => s.data)
  const setConnectionStatus = useMonitorStore((s) => s.setConnectionStatus)
  const visitedRunIds = useMonitorStore((s) => s.visitedRunIds)

  const handleSnapshot = useCallback((payload: unknown) => {
    const next = normalizeBootstrapPayload(payload)
    setData(next)
    setConnectionStatus(next.generatedAt ? 'live' : 'connecting')
  }, [setConnectionStatus, setData])

  const handleConnectionChange = useCallback((connected: boolean) => {
    setConnectionStatus(connected ? 'connecting' : 'offline')
  }, [setConnectionStatus])

  const connected = useLiveSnapshot(true, handleSnapshot, handleConnectionChange)

  return (
    <div className="island-app">
      <IslandSurface
        runs={data?.runs ?? []}
        visitedRunIds={visitedRunIds}
        connected={connected}
      />
      <ToastContainer />
    </div>
  )
}
