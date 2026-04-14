import { useState, useEffect } from 'react'
import { apiFetch } from '../../lib/api'
import { useI18n } from '../../lib/i18n'
import type { StepRun, WorkflowRun, RunRecord, LinkedRunRef } from '../../lib/types'

interface Props {
  step: StepRun
  run: WorkflowRun
  monitorRuns: RunRecord[]
  onAction: (action: string, stepId: string, body?: unknown) => Promise<void>
}

interface LaunchPreview {
  renderedPrompt: string
  inputArtifacts: { mode: string; value: string; required: boolean }[]
  model: string | null
  allowedTools: string[]
  estimatedPromptChars: number
}

const confidenceLabels: Record<string, string> = {
  explicit: 'explicit',
  contextFile: 'context',
  promptMarker: 'prompt',
  workspaceAndArtifact: 'workspace',
  heuristicCandidate: 'heuristic',
}

const confidenceClasses: Record<string, string> = {
  explicit: 'wf-conf-explicit',
  contextFile: 'wf-conf-context',
  promptMarker: 'wf-conf-context',
  workspaceAndArtifact: 'wf-conf-heuristic',
  heuristicCandidate: 'wf-conf-heuristic',
}

export function StepDetail({ step, run, monitorRuns, onAction }: Props) {
  const { t } = useI18n()
  const [showLinkPicker, setShowLinkPicker] = useState(false)
  const [preview, setPreview] = useState<LaunchPreview | null>(null)
  const [loadingPreview, setLoadingPreview] = useState(false)
  const [candidates, setCandidates] = useState<LinkedRunRef[]>([])
  const [loadingCandidates, setLoadingCandidates] = useState(false)

  const defStep = run.definitionSnapshot.steps.find((s) => s.id === step.stepId)
  const isObserve = step.kind === 'observe'
  const isLaunch = step.kind === 'launch'
  const canLink = step.state === 'waitingLink' || step.state === 'ready' || step.state === 'running'
  const canComplete =
    step.state === 'waitingLink' || step.state === 'ready' || step.state === 'running'
  const canSkip = step.state !== 'completed' && step.state !== 'skipped' && step.state !== 'cancelled'
  const canApprove = isLaunch && (step.state === 'waitingApproval' || step.state === 'ready')
  const canRetry = step.state === 'failed'
  const isRunning = step.state === 'running'

  const completionModeLabels: Record<string, string> = {
    manualLink: t('wf.completionManualLink'),
    launcherExit: t('wf.completionLauncherExit'),
    hookEvent: t('wf.completionHookEvent'),
    manualComplete: t('wf.completionManualComplete'),
  }

  // Fetch heuristic candidates when link picker is opened
  useEffect(() => {
    if (showLinkPicker && !loadingCandidates) {
      setLoadingCandidates(true)
      apiFetch(`/api/workflow-runs/${run.id}/steps/${step.stepId}/candidates`)
        .then((res) => (res.ok ? res.json() : []))
        .then((data: LinkedRunRef[]) => setCandidates(data))
        .finally(() => setLoadingCandidates(false))
    }
  }, [showLinkPicker, run.id, step.stepId]) // eslint-disable-line react-hooks/exhaustive-deps

  // Fetch launch preview when step is awaiting approval
  useEffect(() => {
    if (canApprove && !preview && !loadingPreview) {
      setLoadingPreview(true)
      apiFetch(`/api/workflow-runs/${run.id}/steps/${step.stepId}/preview`)
        .then((res) => (res.ok ? res.json() : null))
        .then((data) => setPreview(data))
        .finally(() => setLoadingPreview(false))
    }
  }, [canApprove, preview, loadingPreview, run.id, step.stepId])

  return (
    <div className="wf-detail">
      <div className="wf-panel">
        <div className="wf-panel-title">
          Step {step.order + 1}: {step.label}
        </div>
        <div className="wf-panel-body">
          <div className="wf-detail-header">
            <span className={`wf-tool-icon ${step.tool === 'claude' ? 'wf-tool-claude' : step.tool === 'codex' ? 'wf-tool-codex' : step.tool === 'hermes' ? 'wf-tool-hermes' : 'wf-tool-openclaw'}`}>
              {step.tool === 'claude' ? 'CC' : step.tool === 'codex' ? 'CX' : step.tool === 'hermes' ? 'HM' : 'OC'}
            </span>
            <span className="wf-detail-tool-name">
              {step.tool === 'claude' ? 'Claude Code' : step.tool === 'codex' ? 'Codex' : step.tool === 'hermes' ? 'Hermes' : 'OpenClaw'}
            </span>
            <span className={`wf-step-kind ${isObserve ? 'observe' : 'launch'}`}>
              {isObserve ? `\u{1F441} ${t('wf.stepObserve')}` : `\u{1F680} ${t('wf.stepLaunch')}`}
            </span>
          </div>

          {defStep && (
            <div className="wf-completion-policy">
              <div className="wf-cpol-mode">
                {t('wf.completion')}: {completionModeLabels[defStep.completion.mode] ?? defStep.completion.mode}
              </div>
              {defStep.completion.requiredArtifacts.map((a) => (
                <div key={a} className="wf-cpol-check">
                  {step.artifacts.some((art) => art.path === a) ? '\u2611' : '\u2610'} {a}
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="wf-step-actions">
          {canLink && (
            <button
              className="wf-btn wf-btn-primary"
              onClick={() => setShowLinkPicker(!showLinkPicker)}
            >
              {t('wf.linkRun')}
            </button>
          )}
          {canApprove && (
            <button
              className="wf-btn wf-btn-primary"
              onClick={() => onAction('approve', step.stepId)}
            >
              {t('wf.approveAndLaunch')}
            </button>
          )}
          {canComplete && (
            <button className="wf-btn" onClick={() => onAction('complete', step.stepId)}>
              {t('wf.complete')}
            </button>
          )}
          {canRetry && (
            <button className="wf-btn wf-btn-primary" onClick={() => onAction('retry', step.stepId)}>
              {t('wf.retry')}
            </button>
          )}
          {canSkip && (
            <button className="wf-btn" onClick={() => onAction('skip', step.stepId)}>
              {t('wf.skip')}
            </button>
          )}
        </div>
      </div>

      {/* Running indicator */}
      {isRunning && isLaunch && (
        <div className="wf-panel">
          <div className="wf-panel-body">
            <div className="wf-running-indicator">
              <span className="wf-running-dot" />
              {t('wf.runningCli')}
            </div>
          </div>
        </div>
      )}

      {/* Launch Preview */}
      {canApprove && preview && (
        <div className="wf-panel">
          <div className="wf-panel-title">{t('wf.launchPreview')}</div>
          <div className="wf-panel-body">
            {preview.model && (
              <div className="wf-preview-meta">{t('wf.previewModel')}: {preview.model}</div>
            )}
            {preview.allowedTools.length > 0 && (
              <div className="wf-preview-meta">
                {t('wf.previewTools')}: {preview.allowedTools.join(', ')}
              </div>
            )}
            <div className="wf-preview-meta">
              {t('wf.previewPrompt')}: ~{preview.estimatedPromptChars.toLocaleString()} {t('wf.previewChars')}
            </div>
            <pre className="wf-preview-prompt">{preview.renderedPrompt}</pre>
          </div>
        </div>
      )}

      {/* Linked Runs */}
      <div className="wf-panel">
        <div className="wf-panel-title">{t('wf.linkedRuns')}</div>
        <div className="wf-panel-body">
          {step.linkedRuns.length === 0 ? (
            <div className="wf-empty-sm">{t('wf.noLinkedRuns')}</div>
          ) : (
            step.linkedRuns.map((lr) => (
              <div key={lr.runId} className="wf-linked-run">
                <div className="wf-lr-info">
                  <div className="wf-lr-name">{lr.runId}</div>
                  <div className="wf-lr-meta">{lr.matchedBy}</div>
                </div>
                <span className={`wf-lr-confidence ${confidenceClasses[lr.confidence] ?? ''}`}>
                  {confidenceLabels[lr.confidence] ?? lr.confidence}
                </span>
                <button
                  className="wf-btn-xs"
                  onClick={() =>
                    onAction('unlink', step.stepId, { runId: lr.runId })
                  }
                >
                  {t('wf.unlink')}
                </button>
              </div>
            ))
          )}
        </div>
      </div>

      {/* Link Picker */}
      {showLinkPicker && (
        <>
          {/* Heuristic candidates from server */}
          {candidates.length > 0 && (
            <div className="wf-panel">
              <div className="wf-panel-title">{t('wf.suggestedMatches')}</div>
              <div className="wf-panel-body">
                {candidates.map((c) => (
                  <div key={c.runId} className="wf-candidate">
                    <div className="wf-lr-info">
                      <div className="wf-lr-name">{c.runId}</div>
                      <div className="wf-lr-meta">{c.matchedBy}</div>
                    </div>
                    <span className={`wf-lr-confidence ${confidenceClasses[c.confidence] ?? ''}`}>
                      {confidenceLabels[c.confidence] ?? c.confidence}
                    </span>
                    <button
                      className="wf-btn wf-btn-sm"
                      onClick={() => {
                        void onAction('link', step.stepId, {
                          runId: c.runId,
                          confidence: c.confidence,
                          matchedBy: c.matchedBy,
                        })
                        setShowLinkPicker(false)
                      }}
                    >
                      {t('wf.link')}
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Manual picker — all monitor runs */}
          <div className="wf-panel">
            <div className="wf-panel-title">{t('wf.allRuns')}</div>
            <div className="wf-panel-body">
              {loadingCandidates && <div className="wf-empty-sm">{t('wf.loading')}</div>}
              {monitorRuns.length === 0 ? (
                <div className="wf-empty-sm">{t('wf.noMonitorRuns')}</div>
              ) : (
                monitorRuns
                  .filter((r) => !step.linkedRuns.some((lr) => lr.runId === r.id))
                  .slice(0, 10)
                  .map((r) => (
                    <div key={r.id} className="wf-candidate">
                      <div className="wf-lr-info">
                        <div className="wf-lr-name">
                          {r.tool === 'claude' ? 'Claude' : r.tool === 'codex' ? 'Codex' : 'OpenClaw'}{' '}
                          &mdash; {r.firstQuestion?.slice(0, 60) ?? r.projectName}
                        </div>
                        <div className="wf-lr-meta">
                          {r.id} &middot; {r.state}
                        </div>
                      </div>
                      <button
                        className="wf-btn wf-btn-sm"
                        onClick={() => {
                          void onAction('link', step.stepId, {
                            runId: r.id,
                            confidence: 'explicit',
                            matchedBy: 'user-manual',
                          })
                          setShowLinkPicker(false)
                        }}
                      >
                        {t('wf.link')}
                      </button>
                    </div>
                  ))
              )}
            </div>
          </div>
        </>
      )}

      {/* Artifacts */}
      {step.artifacts.length > 0 && (
        <div className="wf-panel">
          <div className="wf-panel-title">{t('wf.artifacts')}</div>
          <div className="wf-panel-body">
            {step.artifacts.map((a) => (
              <div key={a.id} className="wf-artifact">
                <span className="wf-art-path">{a.path ?? a.kind}</span>
                <span className="wf-art-kind">{a.kind}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Error */}
      {step.error && (
        <div className="wf-panel">
          <div className="wf-panel-title">{t('wf.error')}</div>
          <div className="wf-panel-body">
            <div className="wf-error-text">{step.error}</div>
          </div>
        </div>
      )}
    </div>
  )
}
