import { useState, useMemo } from 'react'
import { apiFetch } from '../../lib/api'
import { useMonitorStore } from '../../store/monitorStore'
import { useI18n } from '../../lib/i18n'
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
  const { t } = useI18n()
  const data = useMonitorStore((s) => s.data)
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [workingDir, setWorkingDir] = useState('')
  const [steps, setSteps] = useState<StepDraft[]>([newStep()])
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [showDirDropdown, setShowDirDropdown] = useState(false)

  // Collect unique project paths from monitor runs
  const projectPaths = useMemo(() => {
    if (!data?.runs) return []
    const paths = new Set<string>()
    for (const run of data.runs) {
      if (run.workspacePath) paths.add(run.workspacePath)
    }
    return Array.from(paths).sort()
  }, [data?.runs])

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
      setError(t('wf.errorNameRequired'))
      return
    }
    if (steps.length === 0) {
      setError(t('wf.errorStepRequired'))
      return
    }
    if (steps.some((s) => !s.label.trim())) {
      setError(t('wf.errorStepLabel'))
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
        setError(t('wf.errorSave').replace('{status}', String(res.status)))
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

  const filteredPaths = projectPaths.filter(
    (p) => !workingDir || p.toLowerCase().includes(workingDir.toLowerCase()),
  )

  return (
    <div className="wf-editor">
      <div className="wf-editor-header">
        <h2 className="wf-editor-title">{t('wf.newWorkflow')}</h2>
        <button className="wf-btn" onClick={onClose}>
          {t('wf.cancel')}
        </button>
      </div>

      <div className="wf-editor-body">
        <div className="wf-field">
          <label className="wf-field-label">{t('wf.name')}</label>
          <input
            className="wf-input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t('wf.namePlaceholder')}
          />
        </div>
        <div className="wf-field">
          <label className="wf-field-label">{t('wf.description')}</label>
          <input
            className="wf-input"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder={t('wf.descriptionPlaceholder')}
          />
        </div>
        <div className="wf-field wf-field-dir">
          <label className="wf-field-label">{t('wf.workingDir')}</label>
          <div className="wf-dir-wrapper">
            <input
              className="wf-input"
              value={workingDir}
              onChange={(e) => {
                setWorkingDir(e.target.value)
                setShowDirDropdown(true)
              }}
              onFocus={() => setShowDirDropdown(true)}
              onBlur={() => setTimeout(() => setShowDirDropdown(false), 200)}
              placeholder={t('wf.workingDirPlaceholder')}
            />
            {showDirDropdown && filteredPaths.length > 0 && (
              <div className="wf-dir-dropdown">
                {filteredPaths.map((p) => (
                  <button
                    key={p}
                    className="wf-dir-option"
                    onMouseDown={(e) => {
                      e.preventDefault()
                      setWorkingDir(p)
                      setShowDirDropdown(false)
                    }}
                  >
                    {p}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        <div className="wf-steps-header">
          <span className="wf-field-label">{t('wf.steps')}</span>
          <button className="wf-btn wf-btn-sm" onClick={addStep}>
            {t('wf.addStep')}
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
                  placeholder={t('wf.stepLabel')}
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
                    <option value="observe">{t('wf.stepObserve')}</option>
                    <option value="launch">{t('wf.stepLaunch')}</option>
                  </select>
                  <select
                    className="wf-select"
                    value={step.completionMode}
                    onChange={(e) => updateStep(step.key, { completionMode: e.target.value })}
                  >
                    <option value="manualLink">{t('wf.completionManualLink')}</option>
                    <option value="manualComplete">{t('wf.completionManualComplete')}</option>
                    <option value="launcherExit">{t('wf.completionLauncherExit')}</option>
                    <option value="hookEvent">{t('wf.completionHookEvent')}</option>
                  </select>
                  <label className="wf-checkbox-label">
                    <input
                      type="checkbox"
                      checked={step.approvalRequired}
                      onChange={(e) =>
                        updateStep(step.key, { approvalRequired: e.target.checked })
                      }
                    />
                    {t('wf.approval')}
                  </label>
                </div>
                {step.kind === 'launch' && (
                  <textarea
                    className="wf-input wf-input-sm wf-prompt-input"
                    value={step.promptTemplate}
                    onChange={(e) => updateStep(step.key, { promptTemplate: e.target.value })}
                    placeholder={t('wf.promptPlaceholder')}
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
            {t('wf.save')}
          </button>
          <button className="wf-btn wf-btn-primary" onClick={() => void save(true)} disabled={saving}>
            {t('wf.saveAndRun')}
          </button>
        </div>
      </div>
    </div>
  )
}
