import { useMonitorStore, type FontSize, type UiDensity } from '../../../store/monitorStore'
import { useI18n, type Locale } from '../../../lib/i18n'
import { useTheme, builtinThemeOrder, builtinThemeIcons, builtinThemeLabels } from '../../../lib/theme'

const fontSizeOptions: FontSize[] = ['xsmall', 'small', 'default', 'large', 'xlarge']
const densityOptions: UiDensity[] = ['compact', 'comfortable', 'spacious']

export function AppearanceSection() {
  const { locale, setLocale, t } = useI18n()
  const { themeId, setTheme, importVSCodeTheme } = useTheme()
  const settings = useMonitorStore((s) => s.settings)
  const updateSettings = useMonitorStore((s) => s.updateSettings)

  function handleImportTheme() {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = '.json'
    input.onchange = async () => {
      const file = input.files?.[0]
      if (!file) return
      const text = await file.text()
      const id = importVSCodeTheme(text)
      if (!id) alert('Invalid VS Code theme file')
    }
    input.click()
  }

  async function toggleNotifications() {
    if (!settings.notificationsEnabled) {
      if (!('Notification' in window)) return
      let permission = Notification.permission
      if (permission === 'default') {
        permission = await Notification.requestPermission()
      }
      if (permission !== 'granted') {
        updateSettings({ notificationsEnabled: false })
        return
      }
    }

    updateSettings({ notificationsEnabled: !settings.notificationsEnabled })
  }

  return (
    <section className="settings-section">
      <div className="section-label">{t('settings.theme')} &amp; {t('settings.language')}</div>
      <div className="settings-cards-2">
        <div className="settings-card">
          <h3>{t('settings.theme')}</h3>
          <div className="settings-grid-3">
            {builtinThemeOrder.map((id) => (
              <button
                key={id}
                className={`settings-option ${themeId === id ? 'active' : ''}`}
                onClick={() => setTheme(id)}
              >
                {builtinThemeIcons[id]} {builtinThemeLabels[id]}
              </button>
            ))}
          </div>
          <h3 className="settings-subsection-title">{t('settings.language')}</h3>
          <div className="settings-grid-2">
            <button
              className={`settings-option ${locale === 'en' ? 'active' : ''}`}
              onClick={() => setLocale('en' as Locale)}
            >
              English
            </button>
            <button
              className={`settings-option ${locale === 'zh' ? 'active' : ''}`}
              onClick={() => setLocale('zh' as Locale)}
            >
              {'\u4E2D\u6587'}
            </button>
          </div>
          <button className="settings-import-btn" onClick={handleImportTheme}>
            {t('settings.importThemeAction')}
          </button>
        </div>

        <div className="settings-card">
          <h3>{t('settings.uiDensity')}</h3>
          <div className="settings-grid-3">
            {densityOptions.map((density) => (
              <button
                key={density}
                className={`settings-option ${settings.uiDensity === density ? 'active' : ''}`}
                onClick={() => updateSettings({ uiDensity: density })}
              >
                {t(`settings.uiDensity.${density}` as never)}
              </button>
            ))}
          </div>
          <h3 className="settings-subsection-title">{t('settings.fontSize')}</h3>
          <div className="settings-grid-5">
            {fontSizeOptions.map((size) => (
              <button
                key={size}
                className={`settings-option settings-fontsize-${size} ${settings.fontSize === size ? 'active' : ''}`}
                onClick={() => updateSettings({ fontSize: size })}
              >
                {t(`settings.fontSize.${size}` as any)}
              </button>
            ))}
          </div>
          <h3 className="settings-subsection-title">{t('settings.notifications')}</h3>
          <div className="settings-toggle-row">
            <div>
              <div className="toggle-label">{t('settings.notifications')}</div>
              <div className="toggle-hint">{t('settings.notificationsHint')}</div>
            </div>
            <button
              className={`toggle-switch ${settings.notificationsEnabled ? 'on' : ''}`}
              onClick={() => void toggleNotifications()}
              role="switch"
              aria-checked={settings.notificationsEnabled}
              aria-label={t('settings.notifications')}
            >
              <span className="toggle-thumb" />
            </button>
          </div>
        </div>
      </div>
    </section>
  )
}
