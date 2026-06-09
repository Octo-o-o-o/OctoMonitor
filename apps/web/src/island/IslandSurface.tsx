import { useMemo, useRef, useState } from 'react'
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

const toolClass: Record<ToolKind, string> = {
  claude: 'island-tool-claude',
  codex: 'island-tool-codex',
  openClaw: 'island-tool-openclaw',
  hermes: 'island-tool-hermes',
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

  function showSoon() {
    clearTimeout(collapseTimerRef.current)
    clearTimeout(expandTimerRef.current)
    expandTimerRef.current = setTimeout(() => setExpanded(true), 120)
  }

  function hideLater() {
    clearTimeout(expandTimerRef.current)
    clearTimeout(collapseTimerRef.current)
    collapseTimerRef.current = setTimeout(() => setExpanded(false), 280)
  }

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
      className={`island-shell${expanded ? ' is-expanded' : ''}${hasAttention ? ' has-attention' : ''}`}
      role="region"
      aria-label={t('island.region')}
      onMouseEnter={showSoon}
      onMouseLeave={hideLater}
      onFocus={showSoon}
      onBlur={handleBlur}
    >
      <div className="island-collapsed">
        <div className={`island-mark${connected ? ' is-live' : ' is-offline'}`}>
          <span className="island-mark-dot" />
        </div>
        <div className="island-counts">
          <span className="island-count active">{counts.active}</span>
          <span className="island-count waiting">{counts.waiting}</span>
          {counts.unreadDone > 0 && <span className="island-count done">{counts.unreadDone}</span>}
        </div>
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
