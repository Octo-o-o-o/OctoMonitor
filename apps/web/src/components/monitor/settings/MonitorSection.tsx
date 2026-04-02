import { useMonitorStore, type AgentDisplayFormat, type MonitorPeriod, type ColumnLayout } from '../../../store/monitorStore'
import { useI18n } from '../../../lib/i18n'
import type { ToolKind } from '../../../lib/types'

const periods: MonitorPeriod[] = ['30m', '1h', '2h', '4h', '8h', '24h']
const periodLabels: Record<MonitorPeriod, string> = {
  '30m': '30 min', '1h': '1 hour', '2h': '2 hours',
  '4h': '4 hours', '8h': '8 hours', '24h': '24 hours',
}
const columnLayouts: ColumnLayout[] = ['fixed', 'adaptive']
const columnLayoutLabels: Record<ColumnLayout, string> = {
  fixed: 'Fixed (Equal)', adaptive: 'Adaptive',
}
const panelLabels: Record<ToolKind, string> = {
  claude: 'Claude Code', codex: 'Codex', openClaw: 'OpenClaw',
}
const agentDisplayFormats: AgentDisplayFormat[] = ['id', 'name', 'id:name']
const agentDisplayExamples: Record<AgentDisplayFormat, string> = {
  id: '@dev',
  name: '@Athena',
  'id:name': '@dev:Athena',
}

export function MonitorSection() {
  const settings = useMonitorStore((s) => s.settings)
  const updateSettings = useMonitorStore((s) => s.updateSettings)
  const { t } = useI18n()

  const enabledCount = settings.panelConfig.filter((p) => p.enabled).length

  return (
    <section className="settings-section">
      <div className="section-label">{t('tab.monitor')}</div>
      <div className="settings-cards-2">
        <div className="settings-card">
          <h3>{t('settings.monitorPeriod')}</h3>
          <p className="settings-hint">{t('settings.periodHint')}</p>
          <div className="settings-grid-3">
            {periods.map((p) => (
              <button
                key={p}
                className={`settings-option ${settings.monitorPeriod === p ? 'active' : ''}`}
                onClick={() => updateSettings({ monitorPeriod: p })}
              >
                {periodLabels[p]}
              </button>
            ))}
          </div>
          <h3 className="settings-subsection-title">{t('settings.columnLayout')}</h3>
          <p className="settings-hint">{t('settings.columnLayoutHint')}</p>
          <div className="settings-grid-2">
            {columnLayouts.map((l) => (
              <button
                key={l}
                className={`settings-option ${settings.columnLayout === l ? 'active' : ''}`}
                onClick={() => updateSettings({ columnLayout: l })}
              >
                {columnLayoutLabels[l]}
              </button>
            ))}
          </div>
        </div>

        <div className="settings-card">
          <h3>{t('settings.panelConfig')}</h3>
          <p className="settings-hint">{t('settings.panelConfigHint')}</p>
          <div className="panel-config-list">
            {settings.panelConfig.map((entry, idx) => (
              <div key={entry.tool} className="panel-config-row">
                <div className="panel-config-reorder">
                  <button
                    className="panel-move-btn"
                    disabled={idx === 0}
                    onClick={() => {
                      const next = [...settings.panelConfig]
                      ;[next[idx - 1], next[idx]] = [next[idx], next[idx - 1]]
                      updateSettings({ panelConfig: next })
                    }}
                    title={t('ui.moveUp')}
                  >{'\u25B2'}</button>
                  <button
                    className="panel-move-btn"
                    disabled={idx === settings.panelConfig.length - 1}
                    onClick={() => {
                      const next = [...settings.panelConfig]
                      ;[next[idx], next[idx + 1]] = [next[idx + 1], next[idx]]
                      updateSettings({ panelConfig: next })
                    }}
                    title={t('ui.moveDown')}
                  >{'\u25BC'}</button>
                </div>
                <span className="panel-config-label">{panelLabels[entry.tool]}</span>
                <button
                  className={`toggle-switch ${entry.enabled ? 'on' : ''}`}
                  disabled={entry.enabled && enabledCount <= 1}
                  onClick={() => {
                    const next = settings.panelConfig.map((p, i) =>
                      i === idx ? { ...p, enabled: !p.enabled } : p,
                    )
                    updateSettings({ panelConfig: next })
                  }}
                  role="switch"
                  aria-checked={entry.enabled}
                  aria-label={panelLabels[entry.tool]}
                >
                  <span className="toggle-thumb" />
                </button>
              </div>
            ))}
          </div>

          <h3 className="settings-subsection-title">{t('settings.displayOptions')}</h3>
          <div className="settings-toggles">
            <div className="settings-toggle-row">
              <div>
                <div className="toggle-label">{t('settings.showFingerprints')}</div>
                <div className="toggle-hint">{t('settings.fingerprintsHint')}</div>
              </div>
              <button
                className={`toggle-switch ${settings.showFingerprints ? 'on' : ''}`}
                onClick={() => updateSettings({ showFingerprints: !settings.showFingerprints })}
                role="switch"
                aria-checked={settings.showFingerprints}
                aria-label={t('settings.showFingerprints')}
              >
                <span className="toggle-thumb" />
              </button>
            </div>
          </div>

          <h3 className="settings-subsection-title">{t('settings.agentDisplay.label')}</h3>
          <p className="settings-hint">{t('settings.agentDisplay.hint')}</p>
          <div className="agent-display-options">
            {agentDisplayFormats.map((fmt) => (
              <button
                key={fmt}
                className={`agent-display-option ${settings.agentDisplayFormat === fmt ? 'active' : ''}`}
                onClick={() => updateSettings({ agentDisplayFormat: fmt })}
              >
                <span className="agent-display-format-label">{t(`settings.agentDisplay.${fmt}` as any)}</span>
                <span className="agent-display-example">{agentDisplayExamples[fmt]}</span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </section>
  )
}
