import { useCallback, useEffect, useState } from 'react'
import { useMonitorStore } from '../../store/monitorStore'
import { apiFetch } from '../../lib/api'
import type { WorkflowRun, WorkflowRunSummary } from '../../lib/types'
import { WorkflowRunList } from './WorkflowRunList'
import { PipelineView } from './PipelineView'
import { StepDetail } from './StepDetail'
import { WorkflowEditor } from './WorkflowEditor'
import './workflows.css'

export function WorkflowsView() {
  const data = useMonitorStore((s) => s.data)
  const connectionStatus = useMonitorStore((s) => s.connectionStatus)
  const [selectedRunId, setSelectedRunId] = useState<string | undefined>()
  const [selectedStepId, setSelectedStepId] = useState<string | undefined>()
  const [runDetail, setRunDetail] = useState<WorkflowRun | null>(null)
  const [showEditor, setShowEditor] = useState(false)

  const summaries: WorkflowRunSummary[] = data?.workflowRuns ?? []

  const fetchRunDetail = useCallback(async (runId: string) => {
    try {
      const res = await apiFetch(`/api/workflow-runs/${runId}`)
      if (res.ok) {
        const detail: WorkflowRun = await res.json()
        setRunDetail(detail)
        if (!detail.steps.find((s) => s.stepId === selectedStepId)) {
          const current = detail.steps.find(
            (s) => s.state !== 'completed' && s.state !== 'skipped' && s.state !== 'pending',
          )
          setSelectedStepId(current?.stepId ?? detail.steps[0]?.stepId)
        }
      }
    } catch {
      // fetch error handled by connection status
    }
  }, [selectedStepId])

  useEffect(() => {
    if (selectedRunId) {
      void fetchRunDetail(selectedRunId)
    } else {
      setRunDetail(null)
      setSelectedStepId(undefined)
    }
  }, [selectedRunId, fetchRunDetail])

  // Refresh when bootstrap updates
  useEffect(() => {
    if (selectedRunId && data) {
      void fetchRunDetail(selectedRunId)
    }
  }, [data?.generatedAt, selectedRunId, fetchRunDetail])

  const handleStepAction = useCallback(
    async (action: string, stepId: string, body?: unknown) => {
      if (!selectedRunId) return
      try {
        const res = await apiFetch(
          `/api/workflow-runs/${selectedRunId}/steps/${stepId}/${action}`,
          {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: body ? JSON.stringify(body) : undefined,
          },
        )
        if (res.ok) {
          const updated: WorkflowRun = await res.json()
          setRunDetail(updated)
        }
      } catch {
        // handled by connection status
      }
    },
    [selectedRunId],
  )

  const handleModeChange = useCallback(
    async (mode: string) => {
      if (!selectedRunId) return
      try {
        const res = await apiFetch(`/api/workflow-runs/${selectedRunId}/mode`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ mode }),
        })
        if (res.ok) {
          const updated: WorkflowRun = await res.json()
          setRunDetail(updated)
        }
      } catch {}
    },
    [selectedRunId],
  )

  const handleCancel = useCallback(async () => {
    if (!selectedRunId) return
    try {
      const res = await apiFetch(`/api/workflow-runs/${selectedRunId}/cancel`, {
        method: 'POST',
      })
      if (res.ok) {
        const updated: WorkflowRun = await res.json()
        setRunDetail(updated)
      }
    } catch {}
  }, [selectedRunId])

  if (showEditor) {
    return (
      <WorkflowEditor
        onClose={() => setShowEditor(false)}
        onCreated={(runId) => {
          setShowEditor(false)
          setSelectedRunId(runId)
        }}
      />
    )
  }

  if (!data && connectionStatus === 'connecting') {
    return <div className="wf-empty">Connecting...</div>
  }

  const selectedStep = runDetail?.steps.find((s) => s.stepId === selectedStepId)
  const monitorRuns = data?.runs ?? []

  return (
    <div className="wf-layout">
      <WorkflowRunList
        summaries={summaries}
        selectedRunId={selectedRunId}
        onSelect={setSelectedRunId}
        onNewWorkflow={() => setShowEditor(true)}
      />
      <div className="wf-center">
        {runDetail ? (
          <>
            <div className="wf-header">
              <div>
                <div className="wf-header-title">{runDetail.workflowName}</div>
                <div className="wf-header-sub">
                  {runDetail.id} &middot;{' '}
                  {runDetail.steps.filter((s) => s.state === 'completed' || s.state === 'skipped').length}/
                  {runDetail.steps.length} &middot; {runDetail.state}
                </div>
              </div>
              <div className="wf-header-right">
                <div className="wf-mode-selector">
                  {(['trackingOnly', 'assisted', 'auto'] as const).map((m) => (
                    <button
                      key={m}
                      className={`wf-mode-opt ${runDetail.executionMode === m ? 'active' : ''} ${m}`}
                      onClick={() => handleModeChange(m)}
                    >
                      {m === 'trackingOnly' ? 'Tracking' : m === 'assisted' ? 'Assisted' : 'Auto'}
                    </button>
                  ))}
                </div>
                {runDetail.state !== 'completed' &&
                  runDetail.state !== 'cancelled' &&
                  runDetail.state !== 'failed' && (
                    <button className="wf-btn wf-btn-danger" onClick={handleCancel}>
                      Cancel
                    </button>
                  )}
              </div>
            </div>
            <PipelineView
              steps={runDetail.steps}
              selectedStepId={selectedStepId}
              onSelectStep={setSelectedStepId}
            />
          </>
        ) : (
          <div className="wf-empty">
            {summaries.length === 0
              ? 'No workflows yet. Create one to get started.'
              : 'Select a workflow run from the list.'}
          </div>
        )}
      </div>
      <div className="wf-right">
        {selectedStep && runDetail ? (
          <StepDetail
            step={selectedStep}
            run={runDetail}
            monitorRuns={monitorRuns}
            onAction={handleStepAction}
          />
        ) : (
          <div className="wf-empty">Select a step to view details.</div>
        )}
      </div>
    </div>
  )
}
