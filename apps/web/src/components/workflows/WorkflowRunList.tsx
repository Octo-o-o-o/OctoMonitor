import type { WorkflowRunSummary } from '../../lib/types'
import { useI18n, type I18nKey } from '../../lib/i18n'

interface Props {
  summaries: WorkflowRunSummary[]
  selectedRunId: string | undefined
  onSelect: (id: string) => void
  onNewWorkflow: () => void
  onShowHelp: () => void
}

const stateI18nKeys: Record<string, I18nKey> = {
  pending: 'wf.statePending',
  running: 'wf.stateRunning',
  waitingInput: 'wf.stateWaiting',
  waitingApproval: 'wf.stateApproval',
  completed: 'wf.stateDone',
  failed: 'wf.stateFailed',
  cancelled: 'wf.stateCancelled',
}

const stateBadgeClass: Record<string, string> = {
  pending: 'wf-badge-pending',
  running: 'wf-badge-running',
  waitingInput: 'wf-badge-waiting',
  waitingApproval: 'wf-badge-waiting',
  completed: 'wf-badge-done',
  failed: 'wf-badge-failed',
  cancelled: 'wf-badge-done',
}

export function WorkflowRunList({ summaries, selectedRunId, onSelect, onNewWorkflow, onShowHelp }: Props) {
  const { t } = useI18n()

  const active = summaries.filter(
    (s) => s.state === 'running' || s.state === 'waitingInput' || s.state === 'waitingApproval',
  )
  const recent = summaries.filter(
    (s) => s.state === 'completed' || s.state === 'failed' || s.state === 'cancelled' || s.state === 'pending',
  )

  return (
    <div className="wf-sidebar">
      <div className="wf-sidebar-header">
        <span className="wf-sidebar-heading">{t('wf.workflows')}</span>
        <button className="wf-sidebar-help" onClick={onShowHelp} title={t('wf.help')}>?</button>
      </div>
      <div className="wf-sidebar-list">
        {active.length > 0 && (
          <>
            <div className="wf-sidebar-title">{t('wf.sidebarActive')}</div>
            {active.map((s) => (
              <RunItem key={s.id} summary={s} selected={s.id === selectedRunId} onSelect={onSelect} />
            ))}
          </>
        )}
        {recent.length > 0 && (
          <>
            <div className="wf-sidebar-title">{t('wf.sidebarRecent')}</div>
            {recent.map((s) => (
              <RunItem key={s.id} summary={s} selected={s.id === selectedRunId} onSelect={onSelect} />
            ))}
          </>
        )}
      </div>
      <div className="wf-sidebar-footer">
        <button className="wf-new-btn" onClick={onNewWorkflow}>
          + {t('wf.newWorkflow')}
        </button>
      </div>
    </div>
  )
}

function RunItem({
  summary,
  selected,
  onSelect,
}: {
  summary: WorkflowRunSummary
  selected: boolean
  onSelect: (id: string) => void
}) {
  const { t } = useI18n()
  const stateKey = stateI18nKeys[summary.state]
  const stateLabel = stateKey ? t(stateKey) : summary.state

  return (
    <button
      className={`wf-sidebar-item ${selected ? 'active' : ''}`}
      onClick={() => onSelect(summary.id)}
    >
      <div className="wf-sidebar-info">
        <div className="wf-sidebar-name">{summary.workflowName}</div>
        <div className="wf-sidebar-meta">
          {summary.progressLabel} &middot; {summary.currentStep ?? stateLabel}
        </div>
      </div>
      <span className={`wf-badge ${stateBadgeClass[summary.state] ?? 'wf-badge-pending'}`}>
        {stateLabel}
      </span>
    </button>
  )
}
