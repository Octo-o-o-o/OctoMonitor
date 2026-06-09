import { useEffect, useRef, useState } from 'react'
import { buildWsUrl } from './api'

export function useLiveSnapshot(
  enabled: boolean,
  onMessage: (data: unknown) => void,
  onStatusChange: (connected: boolean) => void,
) {
  const [connected, setConnected] = useState(false)
  const retryRef = useRef(0)
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined)

  useEffect(() => {
    if (!enabled) {
      setConnected(false)
      return
    }

    let unmounted = false
    let socket: WebSocket | null = null

    function connect() {
      if (unmounted) return
      const ws = new WebSocket(buildWsUrl('/api/stream'))
      socket = ws

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
        if (unmounted) return
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
      unmounted = true
      clearTimeout(timerRef.current)
      socket?.close()
    }
  }, [enabled, onMessage, onStatusChange])

  return connected
}
