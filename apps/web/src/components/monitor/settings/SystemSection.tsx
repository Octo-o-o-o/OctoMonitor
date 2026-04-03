import { useState } from 'react'
import { apiFetch } from '../../../lib/api'
import { useI18n } from '../../../lib/i18n'
import { useMonitorStore } from '../../../store/monitorStore'

const historyOptions = [7, 30, 90, 180] as const

export function SystemSection() {
  const config = useMonitorStore((s) => s.data?.config)
  const setConfig = useMonitorStore((s) => s.setConfig)
  const identities = useMonitorStore((s) => s.data?.identities)
  const showFingerprints = useMonitorStore((s) => s.settings.showFingerprints)
  const { t } = useI18n()
  const [savingHistoryDays, setSavingHistoryDays] = useState<number | null>(null)
  const [historyError, setHistoryError] = useState<string | null>(null)

  async function patchHistoryDays(nextHistoryDays: number) {
    if (!config || savingHistoryDays != null || config.historyDays === nextHistoryDays) {
      return
    }

    setSavingHistoryDays(nextHistoryDays)
    setHistoryError(null)
    try {
      const response = await apiFetch('/api/config', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ historyDays: nextHistoryDays }),
      })
      if (!response.ok) {
        throw new Error('history window patch failed')
      }
      setConfig(await response.json())
    } catch {
      setHistoryError(t('settings.historyWindowError'))
    } finally {
      setSavingHistoryDays(null)
    }
  }

  return (
    <section className="settings-section">
      <div className="section-label">{t('settings.serverConfig')}</div>
      <div className="settings-cards-2">
        {identities && identities.length > 0 && (
          <div className="settings-card">
            <h3>{t('settings.identities')}</h3>
            <div className="identity-list">
              {identities.map((id) => (
                <div key={id.tool} className="identity-row">
                  <div className="identity-info">
                    <strong>{id.tool}</strong>
                    <span>
                      {showFingerprints
                        ? (id.fingerprint ?? id.accountAlias ?? '\u2014')
                        : t('identity.hidden')}
                    </span>
                  </div>
                  <div className="identity-meta">
                    <span>{id.authMode}</span>
                    <span className={`identity-status ${id.verified ? 'verified' : ''}`}>
                      {id.verified ? t('identity.verified') : t('identity.configured')}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {config && (
          <div className="settings-card">
            <h3>{t('settings.serverConfig')}</h3>
            <div className="settings-grid-2">
              <div className="config-box">
                <span className="config-label">{t('config.listenHost')}:</span>
                <span className="config-value">{config.listenHost}</span>
              </div>
              <div className="config-box">
                <span className="config-label">{t('config.listenPort')}:</span>
                <span className="config-value">{config.listenPort}</span>
              </div>
            </div>
            <h3 className="settings-subsection-title">{t('settings.historyWindow')}</h3>
            <p className="settings-hint">{t('settings.historyWindowHint')}</p>
            <div className="settings-grid-2">
              {historyOptions.map((days) => (
                <button
                  key={days}
                  className={`settings-option ${config.historyDays === days ? 'active' : ''}`}
                  disabled={savingHistoryDays != null}
                  onClick={() => void patchHistoryDays(days)}
                >
                  {days} {t('config.days')}
                </button>
              ))}
            </div>
            {savingHistoryDays != null && (
              <p className="settings-status-text" role="status">{t('ui.saving')}</p>
            )}
            {historyError && (
              <p className="settings-status-text error" role="alert">{historyError}</p>
            )}
          </div>
        )}
      </div>
    </section>
  )
}
