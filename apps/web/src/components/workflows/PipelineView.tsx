import type { StepRun } from '../../lib/types'

interface Props {
  steps: StepRun[]
  selectedStepId: string | undefined
  onSelectStep: (id: string) => void
}

const toolLabels: Record<string, { short: string; cls: string }> = {
  claude: { short: 'CC', cls: 'wf-tool-claude' },
  codex: { short: 'CX', cls: 'wf-tool-codex' },
  openClaw: { short: 'OC', cls: 'wf-tool-openclaw' },
}

const stateClasses: Record<string, string> = {
  completed: 'wf-step-completed',
  running: 'wf-step-running',
  waitingLink: 'wf-step-waiting-link',
  waitingApproval: 'wf-step-waiting-approval',
  ready: 'wf-step-ready',
  failed: 'wf-step-failed',
  skipped: 'wf-step-skipped',
  cancelled: 'wf-step-cancelled',
  pending: 'wf-step-pending',
}

const stateLabels: Record<string, string> = {
  pending: 'Pending',
  ready: 'Ready',
  waitingLink: 'Waiting Link',
  waitingApproval: 'Approval',
  running: 'Running',
  completed: 'Completed',
  failed: 'Failed',
  skipped: 'Skipped',
  cancelled: 'Cancelled',
}

function edgeClass(prevState: string, nextState: string): string {
  if (prevState === 'completed' || prevState === 'skipped') {
    if (nextState === 'completed' || nextState === 'skipped') return 'wf-edge-done'
    if (nextState !== 'pending') return 'wf-edge-active'
  }
  return 'wf-edge-pending'
}

export function PipelineView({ steps, selectedStepId, onSelectStep }: Props) {
  return (
    <div className="wf-pipeline">
      <div className="wf-pipe-row">
        {steps.map((step, i) => {
          const tool = toolLabels[step.tool] ?? { short: '?', cls: '' }
          const isSelected = step.stepId === selectedStepId
          const isObserve = step.kind === 'observe'

          return (
            <div key={step.stepId} style={{ display: 'contents' }}>
              <button
                className="wf-step"
                onClick={() => onSelectStep(step.stepId)}
              >
                <div
                  className={[
                    'wf-step-card',
                    isObserve ? 'wf-step-observe' : '',
                    stateClasses[step.state] ?? '',
                    isSelected ? 'wf-step-selected' : '',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                >
                  <div className="wf-step-top">
                    <span className={`wf-tool-icon ${tool.cls}`}>{tool.short}</span>
                    <span className="wf-step-label">{step.label}</span>
                  </div>
                  <span className={`wf-step-kind ${isObserve ? 'observe' : 'launch'}`}>
                    {isObserve ? '\u{1F441} observe' : '\u{1F680} launch'}
                  </span>
                  <div className="wf-step-bottom">
                    <span className={`wf-step-state ${stateClasses[step.state] ?? ''}`}>
                      {stateLabels[step.state] ?? step.state}
                    </span>
                    <span className="wf-step-dur">
                      {step.completedAt && step.startedAt
                        ? formatDuration(step.startedAt, step.completedAt)
                        : '\u2014'}
                    </span>
                  </div>
                  {(step.linkedRuns.length > 0 || step.artifacts.length > 0) && (
                    <div className="wf-step-links">
                      {step.linkedRuns.length} linked &middot; {step.artifacts.length} artifacts
                    </div>
                  )}
                </div>
              </button>
              {i < steps.length - 1 && (
                <div className={`wf-edge ${edgeClass(step.state, steps[i + 1].state)}`}>
                  <div className="wf-edge-line" />
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}

function formatDuration(start: string, end: string): string {
  const ms = new Date(end).getTime() - new Date(start).getTime()
  if (ms < 0) return '\u2014'
  const secs = Math.floor(ms / 1000)
  if (secs < 60) return `${secs}s`
  const mins = Math.floor(secs / 60)
  const remSecs = secs % 60
  return `${mins}m ${remSecs}s`
}
