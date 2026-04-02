import { useEffect, useMemo, useState } from 'react'
import { useMonitorStore } from '../../store/monitorStore'
import { useI18n } from '../../lib/i18n'
import { buildCommitDateRange, endOfDay, startOfDay } from '../../lib/dateRange'
import { formatCost, formatDateTime, formatDuration, formatTokens } from '../../lib/format'
import type { CommitAttributionLink, CommitRecord, RunRecord, SourceConfidence, ToolKind } from '../../lib/types'
import { DateRangePicker, type DateRange } from './DateRangePicker'
import { CommitsSkeleton } from './Skeleton'

const sourceLabels: Record<ToolKind, string> = {
  claude: 'CLAUDE CODE',
  codex: 'CODEX',
  openClaw: 'OPENCLAW',
}

type ProjectOption = {
  repoId: string
  repoName: string
  repoRoot: string
  commitCount: number
  totalTokens: number
  latestCommittedAt: string
}

function sourceClass(tool: ToolKind): string {
  return tool === 'openClaw' ? 'openclaw' : tool
}

function confidenceLabel(value: SourceConfidence) {
  switch (value) {
    case 'heuristic': return 'commits.confidence.heuristic'
    case 'derived': return 'commits.confidence.derived'
    case 'live': return 'commits.confidence.live'
    case 'official': return 'commits.confidence.official'
    case 'estimated': return 'commits.confidence.estimated'
  }
}

function sessionCountLabel(count: number, sessionLabel: string, sessionsLabel: string) {
  return `${count} ${count === 1 ? sessionLabel : sessionsLabel}`
}

function linkedSessionHandle(linkedRun: RunRecord | undefined, link: CommitAttributionLink): string {
  return linkedRun?.sessionId ?? linkedRun?.threadId ?? link.runId
}

function formatShare(score: number) {
  return `${Math.round(score * 100)}%`
}

function getLinkedRun(runsById: Map<string, RunRecord>, link: CommitAttributionLink): RunRecord | undefined {
  return runsById.get(link.runId)
}

function safeLinks(commit: CommitRecord): CommitAttributionLink[] {
  return Array.isArray(commit.links) ? commit.links : []
}

function safeSources(commit: CommitRecord) {
  return Array.isArray(commit.sources) ? commit.sources : []
}

export function CommitsView() {
  const data = useMonitorStore((s) => s.data)
  const connectionStatus = useMonitorStore((s) => s.connectionStatus)
  const { t } = useI18n()
  const commits = Array.isArray(data?.commits) ? data.commits : []
  const runs = Array.isArray(data?.runs) ? data.runs : []
  const [selectedRepoId, setSelectedRepoId] = useState('all')
  const [expandedCommitIds, setExpandedCommitIds] = useState<string[]>([])
  const [dateRange, setDateRange] = useState<DateRange>(() => {
    const to = endOfDay(new Date())
    const from = startOfDay(new Date())
    return { from, to }
  })
  const [dateRangeLocked, setDateRangeLocked] = useState(false)

  const allRange = useMemo(() => buildCommitDateRange(commits), [commits])

  useEffect(() => {
    if (!allRange || dateRangeLocked) return
    setDateRange(allRange)
  }, [allRange, dateRangeLocked])

  const filteredCommits = useMemo(() => {
    const from = dateRange.from.getTime()
    const to = dateRange.to.getTime()
    return commits.filter((commit) => {
      const ts = new Date(commit.committedAt).getTime()
      return Number.isFinite(ts) && ts >= from && ts <= to
    })
  }, [commits, dateRange])

  const projectOptions = useMemo((): ProjectOption[] => {
    const grouped = new Map<string, ProjectOption>()
    for (const commit of filteredCommits) {
      const existing = grouped.get(commit.repoId)
      if (existing) {
        existing.commitCount += 1
        existing.totalTokens += commit.attributedTokens
        if (commit.committedAt > existing.latestCommittedAt) {
          existing.latestCommittedAt = commit.committedAt
        }
      } else {
        grouped.set(commit.repoId, {
          repoId: commit.repoId,
          repoName: commit.repoName,
          repoRoot: commit.repoRoot,
          commitCount: 1,
          totalTokens: commit.attributedTokens,
          latestCommittedAt: commit.committedAt,
        })
      }
    }

    return [...grouped.values()].sort((a, b) => {
      if (a.latestCommittedAt !== b.latestCommittedAt) {
        return b.latestCommittedAt.localeCompare(a.latestCommittedAt)
      }
      return b.totalTokens - a.totalTokens
    })
  }, [filteredCommits])

  useEffect(() => {
    if (selectedRepoId === 'all') return
    if (!projectOptions.some((project) => project.repoId === selectedRepoId)) {
      setSelectedRepoId('all')
    }
  }, [projectOptions, selectedRepoId])

  const visibleCommits = useMemo(() => {
    return filteredCommits
      .filter((commit) => selectedRepoId === 'all' || commit.repoId === selectedRepoId)
      .sort((a, b) => b.committedAt.localeCompare(a.committedAt))
  }, [filteredCommits, selectedRepoId])

  const visibleProjectCount = useMemo(() => new Set(visibleCommits.map((commit) => commit.repoId)).size, [visibleCommits])

  const runsById = useMemo(() => new Map(runs.map((run) => [run.id, run])), [runs])

  useEffect(() => {
    setExpandedCommitIds((prev) => prev.filter((id) => visibleCommits.some((commit) => commit.id === id)))
  }, [visibleCommits])

  const totals = useMemo(() => {
    const totalTokens = visibleCommits.reduce((sum, commit) => sum + commit.attributedTokens, 0)
    const totalCost = visibleCommits.reduce((sum, commit) => sum + (commit.attributedCostUsd ?? 0), 0)
    return {
      commits: visibleCommits.length,
      projects: visibleProjectCount,
      tokens: totalTokens,
      cost: totalCost,
    }
  }, [visibleCommits, visibleProjectCount])

  const switcherLayout = projectOptions.length <= 6 ? 'horizontal' : 'grid'

  function handleDateRangeChange(nextRange: DateRange) {
    setDateRangeLocked(true)
    setDateRange(nextRange)
  }

  function toggleCommitSessions(commitId: string) {
    setExpandedCommitIds((prev) => (
      prev.includes(commitId)
        ? prev.filter((id) => id !== commitId)
        : [...prev, commitId]
    ))
  }

  if (!data) {
    if (connectionStatus === 'connecting') return <CommitsSkeleton />
    return (
      <div className="commits-view commits-view-empty">
        <div className="status-notice offline">
          <strong>{t('commits.offlineTitle')}</strong>
          <span>{t('commits.offlineHint')}</span>
        </div>
      </div>
    )
  }

  return (
    <div className="commits-view">
      {connectionStatus === 'offline' && (
        <div className="status-notice offline">
          <strong>{t('commits.offlineTitle')}</strong>
          <span>{t('commits.offlineHint')}</span>
        </div>
      )}

      <div className="commits-overview-head">
        <div className="commits-summary-grid">
          <div className="commit-summary-card summary-stat">
            <span className="commit-summary-label summary-label">{t('commits.totalCommits')}</span>
            <strong className="commit-summary-value summary-value">{totals.commits}</strong>
          </div>
          <div className="commit-summary-card summary-stat">
            <span className="commit-summary-label summary-label">{t('commits.totalTokens')}</span>
            <strong className="commit-summary-value summary-value">{formatTokens(totals.tokens)}</strong>
          </div>
          <div className="commit-summary-card summary-stat">
            <span className="commit-summary-label summary-label">{t('commits.totalCost')}</span>
            <strong className="commit-summary-value summary-value">{formatCost(totals.cost)}</strong>
          </div>
          <div className="commit-summary-card summary-stat">
            <span className="commit-summary-label summary-label">{t('commits.projects')}</span>
            <strong className="commit-summary-value summary-value">{totals.projects}</strong>
          </div>
        </div>
        <DateRangePicker value={dateRange} onChange={handleDateRangeChange} allRange={allRange} />
      </div>

      <section className="commits-history">
        <div className="commits-history-head">
          <span className="commits-history-label">{t('commits.history')}</span>
          <span className="commits-history-count">
            {totals.commits} {t('commits.totalCommits').toLowerCase()}
          </span>
        </div>

        <div className="commits-history-shell">
          <div className="commits-switcher-strip">
            <div
              className={`commits-project-tabs layout-${switcherLayout}`}
              role="tablist"
              aria-label={t('commits.projectTabs')}
            >
              <button
                className={`commit-project-tab${selectedRepoId === 'all' ? ' active' : ''}`}
                onClick={() => setSelectedRepoId('all')}
                role="tab"
                aria-selected={selectedRepoId === 'all'}
                title={t('commits.allProjects')}
              >
                <span>{t('commits.allProjects')}</span>
                <strong>{projectOptions.length}</strong>
              </button>
              {projectOptions.map((project) => (
                <button
                  key={project.repoId}
                  className={`commit-project-tab${selectedRepoId === project.repoId ? ' active' : ''}`}
                  onClick={() => setSelectedRepoId(project.repoId)}
                  role="tab"
                  aria-selected={selectedRepoId === project.repoId}
                  title={project.repoRoot}
                >
                  <span>{project.repoName}</span>
                  <strong>{project.commitCount}</strong>
                </button>
              ))}
            </div>
          </div>

          {visibleCommits.length === 0 ? (
            <div className="empty-state-panel">
              <strong>{t('commits.noCommits')}</strong>
              <span>{t('commits.noCommitsHint')}</span>
            </div>
          ) : (
            <div className="commit-list">
              {visibleCommits.map((commit) => {
                const links = safeLinks(commit)
                const sources = safeSources(commit)
                const sessionCount = Math.max(links.length, commit.runCount)
                const isExpanded = expandedCommitIds.includes(commit.id)

                return (
                  <article key={commit.id} className="commit-card">
                    <div className="commit-card-top">
                      <div className="commit-card-meta">
                        <span className="commit-meta-strong">{commit.repoName}</span>
                        {commit.authorName && (
                          <>
                            <span className="commit-meta-sep">•</span>
                            <span>{commit.authorName}</span>
                          </>
                        )}
                        <span className="commit-meta-sep">•</span>
                        <span className="commit-time">{formatDateTime(commit.committedAt)}</span>
                        <span className="commit-meta-sep">•</span>
                        <span className="commit-sha">{commit.shortSha}</span>
                        {commit.worktreeName && (
                          <>
                            <span className="commit-meta-sep">•</span>
                            <span className="commit-worktree">{t('commits.worktree')} {commit.worktreeName}</span>
                          </>
                        )}
                      </div>
                      <div className="commit-card-flags">
                        <span className="commit-number commit-session-count">
                          {sessionCountLabel(sessionCount, t('commits.session'), t('commits.sessionsLabel'))}
                        </span>
                        <span className={`commit-confidence confidence-${commit.confidence}`}>
                          {t(confidenceLabel(commit.confidence))}
                        </span>
                      </div>
                    </div>

                    <div className="commit-card-title-row">
                      <div className="commit-summary">{commit.summary}</div>
                    </div>

                    <div className="commit-card-body">
                      <div className="commit-metrics-grid">
                        <div className="commit-metric-cell">
                          <span className="commit-metric-label">{t('commits.filesMetric')}</span>
                          <strong className="commit-metric-value">{commit.filesChanged}</strong>
                        </div>
                        <div className="commit-metric-cell">
                          <span className="commit-metric-label">{t('commits.added')}</span>
                          <strong className="commit-metric-value commit-metric-value-positive">+{commit.insertions}</strong>
                        </div>
                        <div className="commit-metric-cell">
                          <span className="commit-metric-label">{t('commits.deleted')}</span>
                          <strong className="commit-metric-value commit-metric-value-muted">-{commit.deletions}</strong>
                        </div>
                        <div className="commit-metric-cell">
                          <span className="commit-metric-label">{t('commits.runSessions')}</span>
                          <strong className="commit-metric-value">{sessionCount}</strong>
                        </div>
                        <div className="commit-metric-cell">
                          <span className="commit-metric-label">{t('commits.tokensMetric')}</span>
                          <strong className="commit-metric-value">{formatTokens(commit.attributedTokens)}</strong>
                        </div>
                        <div className="commit-metric-cell">
                          <span className="commit-metric-label">{t('commits.costMetric')}</span>
                          <strong className="commit-metric-value">{formatCost(commit.attributedCostUsd)}</strong>
                        </div>
                      </div>

                      <div className="commit-categories">
                        <div className="commit-categories-row">
                          <span className="commit-subsection-label">{t('commits.categories')}</span>
                          {links.length > 0 && (
                            <button
                              className="commit-sessions-toggle"
                              onClick={() => toggleCommitSessions(commit.id)}
                              aria-expanded={isExpanded}
                            >
                              {isExpanded ? t('commits.hideSessions') : t('commits.viewSessions')}
                            </button>
                          )}
                        </div>
                        <div className="commit-category-list">
                          {sources.length === 0 ? (
                            <span className="commit-category-pill commit-category-pill-muted">
                              {t('commits.unattributed')}
                            </span>
                          ) : (
                            sources.map((source) => (
                              <span
                                key={`${commit.id}-${source.tool}`}
                                className={`commit-category-pill tool-${sourceClass(source.tool)}`}
                              >
                                {sourceLabels[source.tool]}
                              </span>
                            ))
                          )}
                          <span className="commit-category-pill commit-category-pill-secondary">
                            {t('commits.allocation')}
                          </span>
                        </div>
                      </div>
                    </div>

                    {isExpanded && links.length > 0 && (
                      <div className="commit-sessions-panel">
                        <h4 className="commit-subsection-label">
                          {t('commits.sessionDetails')} ({links.length})
                        </h4>
                        <div className="commit-link-list">
                          {links.map((link) => {
                            const linkedRun = getLinkedRun(runsById, link)
                            const linkWorktree = linkedRun?.vcs?.worktreeName ?? commit.worktreeName
                            const sessionHandle = linkedSessionHandle(linkedRun, link)
                            return (
                              <div key={`${commit.id}-${link.runId}`} className="commit-link-row">
                                <div className="commit-link-main">
                                  <div className="commit-link-head">
                                    <span className="commit-link-id">{sessionHandle}</span>
                                    <span className={`commit-link-tool tool-${sourceClass(link.tool)}`}>
                                      {sourceLabels[link.tool]}
                                    </span>
                                  </div>
                                  <div className="commit-link-label" title={link.sessionLabel}>
                                    {link.sessionLabel}
                                  </div>
                                  <div className="commit-link-meta-line">
                                    {linkedRun?.model && <span className="commit-link-model">{linkedRun.model}</span>}
                                    {linkedRun?.elapsedMs != null && <span>{formatDuration(linkedRun.elapsedMs)}</span>}
                                    {linkWorktree && <span>{t('commits.worktree')} {linkWorktree}</span>}
                                  </div>
                                  <div className="commit-link-times">
                                    {linkedRun?.startedAt && (
                                      <span>{t('commits.started')}: {formatDateTime(linkedRun.startedAt)}</span>
                                    )}
                                    {linkedRun?.lastActivityAt && (
                                      <span>{t('commits.ended')}: {formatDateTime(linkedRun.lastActivityAt)}</span>
                                    )}
                                  </div>
                                </div>
                                <div className="commit-link-tail">
                                  <strong className="commit-link-total">{formatTokens(link.allocatedTokens)}</strong>
                                  <span className="commit-link-cost">{formatCost(link.allocatedCostUsd)}</span>
                                  <span className="commit-link-share">{formatShare(link.score)}</span>
                                </div>
                              </div>
                            )
                          })}
                        </div>
                      </div>
                    )}
                  </article>
                )
              })}
            </div>
          )}
        </div>
      </section>
    </div>
  )
}
