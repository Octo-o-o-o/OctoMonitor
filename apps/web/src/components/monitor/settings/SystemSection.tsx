import { useMonitorStore } from '../../../store/monitorStore'
import { useI18n } from '../../../lib/i18n'

export function SystemSection() {
  const config = useMonitorStore((s) => s.data?.config)
  const identities = useMonitorStore((s) => s.data?.identities)
  const showFingerprints = useMonitorStore((s) => s.settings.showFingerprints)
  const { t } = useI18n()

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
            <h3>{t('settings.serverConfig')} <span className="readonly-badge">{t('ui.readOnly')}</span></h3>
            <div className="settings-grid-2">
              <div className="config-box">
                <span className="config-label">{t('config.listenHost')}:</span>
                <span className="config-value">{config.listenHost}</span>
              </div>
              <div className="config-box">
                <span className="config-label">{t('config.listenPort')}:</span>
                <span className="config-value">{config.listenPort}</span>
              </div>
              <div className="config-box">
                <span className="config-label">{t('config.history')}:</span>
                <span className="config-value">{config.historyDays} {t('config.days')}</span>
              </div>
            </div>
          </div>
        )}
      </div>
    </section>
  )
}
