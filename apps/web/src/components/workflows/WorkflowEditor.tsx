import { useState } from 'react'
import { apiFetch } from '../../lib/api'
import type { ToolKind, WorkflowStepKind } from '../../lib/types'

interface Props {
  onClose: () => void
  onCreated: (runId: string) => void
}

interface StepDraft {
  key: number
  label: string
  tool: ToolKind
  kind: WorkflowStepKind
  completionMode: string
  approvalRequired: boolean
  promptTemplate: string
}

let stepCounter = 0

function newStep(): StepDraft {
  return {
    key: ++stepCounter,
    label: '',
    tool: 'claude',
    kind: 'observe',
    completionMode: 'manualLink',
    approvalRequired: false,
    promptTemplate: '',
  }
}

export function WorkflowEditor({ onClose, onCreated }: Props) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [workingDir, setWorkingDir] = useState('')
  const [steps, setSteps] = useState<StepDraft[]>([newStep()])
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function addStep() {
    setSteps([...steps, newStep()])
  }

  function removeStep(key: number) {
    setSteps(steps.filter((s) => s.key !== key))
  }

  function updateStep(key: number, patch: Partial<StepDraft>) {
    setSteps(steps.map((s) => (s.key === key ? { ...s, ...patch } : s)))
  }

  function moveStep(key: number, direction: -1 | 1) {
    const idx = steps.findIndex((s) => s.key === key)
    const newIdx = idx + direction
    if (newIdx < 0 || newIdx >= steps.length) return
    const copy = [...steps]
    ;[copy[idx], copy[newIdx]] = [copy[newIdx], copy[idx]]
    setSteps(copy)
  }

  async function save(andRun: boolean) {
    if (!name.trim()) {
      setError('Name is required')
      return
    }
    if (steps.length === 0) {
      setError('At least one step is required')
      return
    }
    if (steps.some((s) => !s.label.trim())) {
      setError('All steps must have a label')
      return
    }

    setSaving(true)
    setError(null)

    try {
      const defBody = {
        id: '',
        name: name.trim(),
        description: description.trim() || null,
        defaultWorkingDir: workingDir.trim() || null,
        steps: steps.map((s, i) => ({
          id: '',
          order: i,
          label: s.label.trim(),
          tool: s.tool,
          kind: s.kind,
          promptTemplate: s.kind === 'launch' && s.promptTemplate.trim() ? s.promptTemplate.trim() : null,
          inputs: [],
          outputs: [],
          approvalRequired: s.approvalRequired,
          autoAdvanceEligible: s.kind === 'launch',
          completion: {
            mode: s.completionMode,
            requiredArtifacts: [],
          },
          launch:
            s.kind === 'launch'
              ? { model: null, timeoutSecs: null, allowedTools: [], args: [] }
              : null,
        })),
        createdAt: '',
        updatedAt: '',
      }

      const res = await apiFetch('/api/workflows', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(defBody),
      })

      if (!res.ok) {
        setError(`Failed to save: ${res.status}`)
        setSaving(false)
        return
      }

      const def = await res.json()

      if (andRun) {
        const runRes = await apiFetch(`/api/workflows/${def.id}/runs`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            workingDir: workingDir.trim() || '.',
            executionMode: 'trackingOnly',
          }),
        })
        if (runRes.ok) {
          const run = await runRes.json()
          onCreated(run.id)
          return
        }
      }

      onClose()
    } catch (e) {
      setError(`Error: ${e}`)
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="wf-editor">
      <div className="wf-editor-header">
        <h2 className="wf-editor-title">New Workflow</h2>
        <button className="wf-btn" onClick={onClose}>
          Cancel
        </button>
      </div>

      <div className="wf-editor-body">
        <div className="wf-field">
          <label className="wf-field-label">Name</label>
          <input
            className="wf-input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Plan Design & Review"
          />
        </div>
        <div className="wf-field">
          <label className="wf-field-label">Description (optional)</label>
          <input
            className="wf-input"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Brief description"
          />
        </div>
        <div className="wf-field">
          <label className="wf-field-label">Working Directory</label>
          <input
            className="wf-input"
            value={workingDir}
            onChange={(e) => setWorkingDir(e.target.value)}
            placeholder="/path/to/repo"
          />
        </div>

        <div className="wf-steps-header">
          <span className="wf-field-label">Steps</span>
          <button className="wf-btn wf-btn-sm" onClick={addStep}>
            + Add Step
          </button>
        </div>

        <div className="wf-steps-list">
          {steps.map((step, i) => (
            <div key={step.key} className="wf-step-edit">
              <div className="wf-step-edit-order">
                <button
                  className="wf-btn-xs"
                  disabled={i === 0}
                  onClick={() => moveStep(step.key, -1)}
                >
                  &uarr;
                </button>
                <span className="wf-step-edit-num">{i + 1}</span>
                <button
                  className="wf-btn-xs"
                  disabled={i === steps.length - 1}
                  onClick={() => moveStep(step.key, 1)}
                >
                  &darr;
                </button>
              </div>
              <div className="wf-step-edit-fields">
                <input
                  className="wf-input wf-input-sm"
                  value={step.label}
                  onChange={(e) => updateStep(step.key, { label: e.target.value })}
                  placeholder="Step label"
                />
                <div className="wf-step-edit-row">
                  <select
                    className="wf-select"
                    value={step.tool}
                    onChange={(e) => updateStep(step.key, { tool: e.target.value as ToolKind })}
                  >
                    <option value="claude">Claude</option>
                    <option value="codex">Codex</option>
                    <option value="openClaw">OpenClaw</option>
                  </select>
                  <select
                    className="wf-select"
                    value={step.kind}
                    onChange={(e) =>
                      updateStep(step.key, { kind: e.target.value as WorkflowStepKind })
                    }
                  >
                    <option value="observe">Observe</option>
                    <option value="launch">Launch</option>
                  </select>
                  <select
                    className="wf-select"
                    value={step.completionMode}
                    onChange={(e) => updateStep(step.key, { completionMode: e.target.value })}
                  >
                    <option value="manualLink">Manual Link</option>
                    <option value="manualComplete">Manual Complete</option>
                    <option value="launcherExit">Launcher Exit</option>
                    <option value="hookEvent">Hook Event</option>
                  </select>
                  <label className="wf-checkbox-label">
                    <input
                      type="checkbox"
                      checked={step.approvalRequired}
                      onChange={(e) =>
                        updateStep(step.key, { approvalRequired: e.target.checked })
                      }
                    />
                    Approval
                  </label>
                </div>
                {step.kind === 'launch' && (
                  <textarea
                    className="wf-input wf-input-sm wf-prompt-input"
                    value={step.promptTemplate}
                    onChange={(e) => updateStep(step.key, { promptTemplate: e.target.value })}
                    placeholder="Prompt template (supports {{workflow.name}}, {{step.label}}, {{file:path}}, {{previous.summary}}, etc.)"
                    rows={3}
                  />
                )}
              </div>
              <button className="wf-btn-xs wf-btn-danger" onClick={() => removeStep(step.key)}>
                &times;
              </button>
            </div>
          ))}
        </div>

        {error && <div className="wf-error-text">{error}</div>}

        <div className="wf-editor-actions">
          <button className="wf-btn" onClick={() => void save(false)} disabled={saving}>
            Save
          </button>
          <button className="wf-btn wf-btn-primary" onClick={() => void save(true)} disabled={saving}>
            Save & Run
          </button>
        </div>
      </div>
    </div>
  )
}
