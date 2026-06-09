import { useMonitorStore, type DesktopDisplayMode, type IslandPosition } from '../../../store/monitorStore'
import { useI18n, type I18nKey } from '../../../lib/i18n'

const displayModes: DesktopDisplayMode[] = ['both', 'dashboard', 'island']
const displayModeLabels: Record<DesktopDisplayMode, I18nKey> = {
  both: 'settings.desktopDisplayMode.both',
  dashboard: 'settings.desktopDisplayMode.dashboard',
  island: 'settings.desktopDisplayMode.island',
}

const islandPositions: IslandPosition[] = ['auto', 'topCenter']
const islandPositionLabels: Record<IslandPosition, I18nKey> = {
  auto: 'settings.islandPosition.auto',
  topCenter: 'settings.islandPosition.topCenter',
}

export function DesktopDisplaySection() {
  const desktopDisplayMode = useMonitorStore((s) => s.settings.desktopDisplayMode)
  const islandPosition = useMonitorStore((s) => s.settings.islandPosition)
  const updateSettings = useMonitorStore((s) => s.updateSettings)
  const { t } = useI18n()

  return (
    <section className="settings-section">
      <div className="section-label">{t('settings.desktopDisplay')}</div>
      <div className="settings-cards-2">
        <div className="settings-card">
          <h3>{t('settings.desktopDisplayMode')}</h3>
          <p className="settings-hint">{t('settings.desktopDisplayModeHint')}</p>
          <div className="settings-grid-3">
            {displayModes.map((mode) => (
              <button
                key={mode}
                className={`settings-option ${desktopDisplayMode === mode ? 'active' : ''}`}
                aria-pressed={desktopDisplayMode === mode}
                onClick={() => updateSettings({ desktopDisplayMode: mode })}
              >
                {t(displayModeLabels[mode])}
              </button>
            ))}
          </div>
        </div>

        <div className="settings-card">
          <h3>{t('settings.islandPosition')}</h3>
          <p className="settings-hint">{t('settings.islandPositionHint')}</p>
          <div className="settings-grid-2">
            {islandPositions.map((position) => (
              <button
                key={position}
                className={`settings-option ${islandPosition === position ? 'active' : ''}`}
                aria-pressed={islandPosition === position}
                onClick={() => updateSettings({ islandPosition: position })}
              >
                {t(islandPositionLabels[position])}
              </button>
            ))}
          </div>
        </div>
      </div>
    </section>
  )
}
