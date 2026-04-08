import type { WorkflowRunSummary } from '../../lib/types'

interface Props {
  summaries: WorkflowRunSummary[]
  selectedRunId: string | undefined
  onSelect: (id: string) => void
  onNewWorkflow: () => void
}

const stateLabels: Record<string, string> = {
  pending: 'Pending',
  running: 'Running',
  waitingInput: 'Waiting',
  waitingApproval: 'Approval',
  completed: 'Done',
  failed: 'Failed',
  cancelled: 'Cancelled',
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

export function WorkflowRunList({ summaries, selectedRunId, onSelect, onNewWorkflow }: Props) {
  const active = summaries.filter(
    (s) => s.state === 'running' || s.state === 'waitingInput' || s.state === 'waitingApproval',
  )
  const recent = summaries.filter(
    (s) => s.state === 'completed' || s.state === 'failed' || s.state === 'cancelled' || s.state === 'pending',
  )

  return (
    <div className="wf-sidebar">
      {active.length > 0 && (
        <>
          <div className="wf-sidebar-title">Active</div>
          {active.map((s) => (
            <RunItem key={s.id} summary={s} selected={s.id === selectedRunId} onSelect={onSelect} />
          ))}
        </>
      )}
      {recent.length > 0 && (
        <>
          <div className="wf-sidebar-title" style={{ marginTop: active.length > 0 ? 8 : 0 }}>
            Recent
          </div>
          {recent.map((s) => (
            <RunItem key={s.id} summary={s} selected={s.id === selectedRunId} onSelect={onSelect} />
          ))}
        </>
      )}
      <button className="wf-new-btn" onClick={onNewWorkflow}>
        + New Workflow
      </button>
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
  return (
    <button
      className={`wf-sidebar-item ${selected ? 'active' : ''}`}
      onClick={() => onSelect(summary.id)}
    >
      <div className="wf-sidebar-info">
        <div className="wf-sidebar-name">{summary.workflowName}</div>
        <div className="wf-sidebar-meta">
          {summary.progressLabel} &middot; {summary.currentStep ?? stateLabels[summary.state] ?? summary.state}
        </div>
      </div>
      <span className={`wf-badge ${stateBadgeClass[summary.state] ?? 'wf-badge-pending'}`}>
        {stateLabels[summary.state] ?? summary.state}
      </span>
    </button>
  )
}
