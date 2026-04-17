import type { SnapshotWindow } from '../../store/monitorStore'
import { useI18n, type I18nKey } from '../../lib/i18n'

const windows: SnapshotWindow[] = ['day', 'week', 'month', 'all']
const labelKeys: Record<SnapshotWindow, I18nKey> = {
  day: 'stat.day',
  week: 'stat.week',
  month: 'stat.month',
  all: 'stat.all',
}

export function SnapshotWindowSwitch({
  value,
  onChange,
  compact = false,
}: {
  value: SnapshotWindow
  onChange: (value: SnapshotWindow) => void
  compact?: boolean
}) {
  const { t } = useI18n()

  return (
    <div
      className={`window-switch${compact ? ' compact' : ''}`}
      role="tablist"
      aria-label={t('history.snapshotWindowLabel')}
    >
      {windows.map((window) => (
        <button
          key={window}
          className={`window-switch-btn ${value === window ? 'active' : ''}`}
          onClick={() => onChange(window)}
          role="tab"
          aria-selected={value === window}
        >
          {t(labelKeys[window])}
        </button>
      ))}
    </div>
  )
}
