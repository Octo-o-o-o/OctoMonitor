import { useEffect, useState } from 'react'
import { useI18n } from '../../../lib/i18n'
import { apiFetch } from '../../../lib/api'

type Capability = { tool: string; detected: boolean; mode: string; notes: string }

function capabilityLabel(tool: string): string {
  if (tool === 'hermes') return 'hermes (experimental)'
  return tool
}

export function SetupSection() {
  const { t } = useI18n()

  const [capabilities, setCapabilities] = useState<Capability[]>([])
  const [checks, setChecks] = useState<string[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    Promise.all([
      apiFetch('/api/installer/detect').then((r) => r.json()).then((d) => setCapabilities(d.capabilities ?? [])).catch(() => {}),
      apiFetch('/api/installer/doctor').then((r) => r.json()).then((d) => setChecks(d.checks ?? [])).catch(() => {}),
    ]).finally(() => setLoading(false))
  }, [])

  return (
    <section className="settings-section">
      <div className="section-label">{t('setup.title')}</div>
      <p className="settings-hint">{t('setup.doctorHint')}</p>
      {loading && <div className="setup-loading">{t('setup.detecting')}</div>}
      <div className="setup-cards">
        {capabilities.map((cap) => (
          <div key={cap.tool} className="setup-card">
            <div className="setup-card-header">
              <strong>{capabilityLabel(cap.tool)}</strong>
              <span className={`setup-status ${cap.detected ? 'detected' : 'missing'}`}>
                {cap.detected ? t('setup.detected') : t('setup.missing')}
              </span>
            </div>
            <div className="setup-card-meta">
              <span>{cap.mode}</span>
              {cap.notes && <span className="setup-notes">{cap.notes}</span>}
            </div>
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
    </section>
  )
}
