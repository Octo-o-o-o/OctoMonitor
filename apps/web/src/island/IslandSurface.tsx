import { type CSSProperties, useEffect, useMemo, useRef, useState } from 'react'
import { sourceLabelsUpper } from '../lib/constants'
import { formatDuration } from '../lib/format'
import { buildCodexDeepLink, getRunOpenAffordance } from '../lib/runTarget'
import { isTauriEnvironment } from '../lib/runtimeEnvironment'
import { buildIslandCounts, buildIslandItems, type IslandItem } from '../lib/island'
import { openExternalUrl } from '../lib/openExternal'
import type { RunRecord, ToolKind } from '../lib/types'
import { useMonitorStore } from '../store/monitorStore'
import { useToastStore } from '../store/toastStore'
import { useI18n } from '../lib/i18n'

type IslandSurfaceProps = {
  runs: RunRecord[]
  visitedRunIds: ReadonlySet<string>
  connected?: boolean
}

type IslandChromeMetrics = {
  closedWidth: number
  closedHeight: number
  notched: boolean
}

type IslandExpansionWindow = Window & {
  __OCTOMONITOR_ISLAND_EXPANDED__?: boolean
}

const toolClass: Record<ToolKind, string> = {
  claude: 'island-tool-claude',
  codex: 'island-tool-codex',
  openClaw: 'island-tool-openclaw',
  hermes: 'island-tool-hermes',
}

const defaultIslandChromeMetrics: IslandChromeMetrics = {
  closedWidth: 360,
  closedHeight: 38,
  notched: false,
}
const islandExpansionEvent = 'octomonitor-island-expansion'

function readIslandChromeMetrics(): IslandChromeMetrics {
  if (typeof window === 'undefined') return defaultIslandChromeMetrics
  const params = new URLSearchParams(window.location.search)
  const closedWidth = Number(params.get('closedWidth'))
  const closedHeight = Number(params.get('closedHeight'))
  return {
    closedWidth: Number.isFinite(closedWidth) && closedWidth >= 180 && closedWidth <= 520
      ? closedWidth
      : defaultIslandChromeMetrics.closedWidth,
    closedHeight: Number.isFinite(closedHeight) && closedHeight >= 24 && closedHeight <= 48
      ? closedHeight
      : defaultIslandChromeMetrics.closedHeight,
    notched: params.get('notched') === '1',
  }
}

function itemTitle(run: RunRecord): string {
  return run.projectName || run.workspaceShort || sourceLabelsUpper[run.tool]
}

function itemSubtitle(run: RunRecord): string {
  return run.lastQuestion
    ?? run.lastAction
    ?? run.lastTail
    ?? run.workspaceShort
    ?? sourceLabelsUpper[run.tool]
}

export function IslandSurface({
  runs,
  visitedRunIds,
  connected = true,
}: IslandSurfaceProps) {
  const { t } = useI18n()
  const markRunVisited = useMonitorStore((s) => s.markRunVisited)
  const pushToast = useToastStore((s) => s.pushToast)
  const [expanded, setExpanded] = useState(false)
  const expandTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined)
  const collapseTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined)
  const rootRef = useRef<HTMLDivElement | null>(null)
  const chromeMetrics = useMemo(readIslandChromeMetrics, [])
  const chromeStyle = {
    '--island-width': `${chromeMetrics.closedWidth}px`,
    '--island-collapsed-height': `${chromeMetrics.closedHeight}px`,
  } as CSSProperties

  const counts = useMemo(
    () => buildIslandCounts(runs, visitedRunIds),
    [runs, visitedRunIds],
  )
  const items = useMemo(
    () => buildIslandItems(runs, visitedRunIds, 8),
    [runs, visitedRunIds],
  )
  const hasAttention = counts.waiting > 0
  const hasActivity = counts.active > 0 || counts.waiting > 0 || counts.unreadDone > 0
  const summaryCount = counts.waiting || counts.active || counts.unreadDone

  useEffect(() => {
    return () => {
      clearTimeout(expandTimerRef.current)
      clearTimeout(collapseTimerRef.current)
    }
  }, [])

  function showSoon() {
    clearTimeout(collapseTimerRef.current)
    clearTimeout(expandTimerRef.current)
    expandTimerRef.current = setTimeout(() => setExpanded(true), 150)
  }

  function hideLater() {
    clearTimeout(expandTimerRef.current)
    clearTimeout(collapseTimerRef.current)
    collapseTimerRef.current = setTimeout(() => setExpanded(false), 280)
  }

  useEffect(() => {
    function handleNativeExpansion(event: Event) {
      const expanded = (event as CustomEvent<{ expanded?: boolean }>).detail?.expanded === true
      if (expanded) {
        showSoon()
      } else {
        hideLater()
      }
    }

    window.addEventListener(islandExpansionEvent, handleNativeExpansion)
    if ((window as IslandExpansionWindow).__OCTOMONITOR_ISLAND_EXPANDED__ === true) {
      showSoon()
    }
    return () => window.removeEventListener(islandExpansionEvent, handleNativeExpansion)
  }, [])

  function handleBlur() {
    window.setTimeout(() => {
      if (!rootRef.current?.contains(document.activeElement)) setExpanded(false)
    }, 0)
  }

  async function handleItemClick(item: IslandItem) {
    const affordance = getRunOpenAffordance(item.run)
    try {
      if (affordance === 'openCodex' && isTauriEnvironment() && item.run.threadId) {
        await openExternalUrl(buildCodexDeepLink(item.run.threadId))
      }
    } catch {
      pushToast({ kind: 'error', message: t('drawer.openInCodex.error') })
    } finally {
      markRunVisited(item.run.id)
    }
  }

  return (
    <div
      ref={rootRef}
      className={`island-shell${expanded ? ' is-expanded' : ''}${hasAttention ? ' has-attention' : ''}${chromeMetrics.notched ? ' is-notched' : ''}`}
      style={chromeStyle}
      role="region"
      aria-label={t('island.region')}
      onMouseEnter={showSoon}
      onMouseLeave={hideLater}
      onFocus={showSoon}
      onBlur={handleBlur}
    >
      <div className="island-collapsed">
        <div className={`island-mark${connected ? ' is-live' : ' is-offline'}`} aria-hidden="true">
          <span className="island-mark-dot" />
        </div>
        <div className="island-counts" aria-hidden={chromeMetrics.notched}>
          <span className="island-count active">{counts.active}</span>
          <span className="island-count waiting">{counts.waiting}</span>
          {counts.unreadDone > 0 && <span className="island-count done">{counts.unreadDone}</span>}
        </div>
        <span className="island-count-summary" aria-hidden={!chromeMetrics.notched}>
          {summaryCount}
        </span>
      </div>

      <div className="island-expanded">
        <div className="island-expanded-header">
          <span className={`island-status-dot${connected ? ' is-live' : ' is-offline'}`} />
          <span className="island-header-title">OctoMonitor</span>
          <span className="island-header-count">{items.length}</span>
        </div>
        <div className="island-list">
          {items.length === 0 && (
            <div className="island-empty">
              <span className="island-empty-title">{t('island.empty.title')}</span>
              <span className="island-empty-copy">
                {hasActivity ? t('island.empty.waiting') : t('island.empty.none')}
              </span>
            </div>
          )}
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              className={`island-item ${item.priority}${item.unread ? ' is-unread' : ' is-read'}`}
              onClick={() => void handleItemClick(item)}
              tabIndex={expanded ? 0 : -1}
            >
              <span className={`island-agent-dot ${toolClass[item.run.tool]}`} />
              <span className="island-item-main">
                <span className="island-item-title">{itemTitle(item.run)}</span>
                <span className="island-item-subtitle">{itemSubtitle(item.run)}</span>
              </span>
              <span className="island-item-meta">
                <span className="island-tool-badge">{sourceLabelsUpper[item.run.tool]}</span>
                <span className="island-elapsed">{formatDuration(item.run.elapsedMs)}</span>
                {item.unread && <span className="island-unread-dot" aria-hidden="true" />}
              </span>
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
