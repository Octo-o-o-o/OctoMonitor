import { useI18n } from '../lib/i18n'

export function LoadingScreen() {
  const { t } = useI18n()

  return (
    <div className="loading-screen">
      <div className="loading-screen-inner">
        <div className="loading-logo">
          <svg width="48" height="48" viewBox="0 0 48 48" fill="none" aria-hidden="true">
            <circle cx="24" cy="24" r="20" stroke="var(--text-secondary)" strokeWidth="2" strokeDasharray="6 4" className="loading-ring" />
            <circle cx="24" cy="24" r="8" fill="var(--text-primary)" className="loading-dot" />
          </svg>
        </div>
        <p className="loading-title">{t('app.loading')}</p>
        <p className="loading-hint">{t('app.loadingHint')}</p>
      </div>
    </div>
  )
}
