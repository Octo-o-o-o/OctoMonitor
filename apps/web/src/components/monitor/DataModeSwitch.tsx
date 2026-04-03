import type { DataMode } from '../../lib/history'
import { useI18n } from '../../lib/i18n'

export function DataModeSwitch({
  mode,
  onChange,
}: {
  mode: DataMode
  onChange: (mode: DataMode) => void
}) {
  const { t } = useI18n()

  return (
    <div className="data-mode-switch" role="tablist" aria-label={t('history.modeLabel')}>
      <button
        className={`data-mode-btn ${mode === 'snapshot' ? 'active' : ''}`}
        onClick={() => onChange('snapshot')}
        role="tab"
        aria-selected={mode === 'snapshot'}
      >
        {t('history.snapshot')}
      </button>
      <button
        className={`data-mode-btn ${mode === 'history' ? 'active' : ''}`}
        onClick={() => onChange('history')}
        role="tab"
        aria-selected={mode === 'history'}
      >
        {t('history.history')}
      </button>
    </div>
  )
}
