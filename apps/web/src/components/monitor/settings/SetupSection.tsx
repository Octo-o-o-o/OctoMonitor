import { useEffect, useState } from 'react'
import { useI18n } from '../../../lib/i18n'
import { apiFetch } from '../../../lib/api'

type Capability = { tool: string; detected: boolean; mode: string; notes: string }
type InstallAction = { id: string; kind: string; path: string; description: string }
type InstallPlan = { tool: string; dryRun: boolean; actions: InstallAction[] }
type InstallResult = { tool: string; applied: boolean; paths: string[]; message: string }

export function SetupSection() {
  const { t } = useI18n()

  const [capabilities, setCapabilities] = useState<Capability[]>([])
  const [checks, setChecks] = useState<string[]>([])
  const [plans, setPlans] = useState<Record<string, InstallPlan>>({})
  const [result, setResult] = useState<string>('')
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    Promise.all([
      apiFetch('/api/installer/detect').then((r) => r.json()).then((d) => setCapabilities(d.capabilities ?? [])).catch(() => {}),
      apiFetch('/api/installer/doctor').then((r) => r.json()).then((d) => setChecks(d.checks ?? [])).catch(() => {}),
    ]).finally(() => setLoading(false))
  }, [])

  async function loadPlan(tool: string) {
    try {
      const response = await apiFetch('/api/installer/install-plan', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ tool }),
      })
      if (!response.ok) throw new Error(`HTTP ${response.status}`)
      const data = await response.json()
      setPlans((current) => ({ ...current, [tool]: data.plan }))
    } catch (err) {
      setResult(`Failed to load plan for ${tool}: ${err instanceof Error ? err.message : 'unknown error'}`)
    }
  }

  async function runAction(tool: string, kind: 'install' | 'rollback') {
    try {
      const response = await apiFetch(`/api/installer/${kind}`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ tool }),
      })
      if (!response.ok) throw new Error(`HTTP ${response.status}`)
      const data = await response.json()
      const payload: InstallResult | undefined = data.result
      setResult(payload ? `${payload.message}: ${payload.paths.join(', ')}` : `${kind} completed for ${tool}`)
    } catch (err) {
      setResult(`${kind} failed for ${tool}: ${err instanceof Error ? err.message : 'unknown error'}`)
    }
  }

  return (
    <section className="settings-section">
      <div className="section-label">{t('setup.title')}</div>
      <p className="settings-hint">{t('setup.sandboxHint')}</p>
      {loading && <div className="setup-loading">{t('setup.detecting')}</div>}
      <div className="setup-cards">
        {capabilities.map((cap) => (
          <div key={cap.tool} className="setup-card">
            <div className="setup-card-header">
              <strong>{cap.tool}</strong>
              <span className={`setup-status ${cap.detected ? 'detected' : 'missing'}`}>
                {cap.detected ? t('setup.detected') : t('setup.missing')}
              </span>
            </div>
            <div className="setup-card-meta">
              <span>{cap.mode}</span>
              {cap.notes && <span className="setup-notes">{cap.notes}</span>}
            </div>
            <div className="setup-actions">
              <button className="setup-btn" onClick={() => loadPlan(cap.tool)}>{t('setup.showPlan')}</button>
              <button className="setup-btn" onClick={() => runAction(cap.tool, 'install')}>{t('setup.installSandbox')}</button>
              <button className="setup-btn danger" onClick={() => runAction(cap.tool, 'rollback')}>{t('setup.rollback')}</button>
            </div>
            {plans[cap.tool] && (
              <div className="setup-plan">
                {plans[cap.tool].actions.map((action) => (
                  <div key={action.id} className="setup-plan-item">
                    <span className="plan-kind">{action.kind}</span>
                    <span className="plan-path">{action.path}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>

      {checks.length > 0 && (
        <div className="setup-checks">
          <h3>{t('setup.doctorChecks')}</h3>
          <div className="check-list">
            {checks.map((item, i) => (
              <div key={i} className="check-item">{item}</div>
            ))}
          </div>
        </div>
      )}

      {result && <div className="setup-result">{result}</div>}
    </section>
  )
}
