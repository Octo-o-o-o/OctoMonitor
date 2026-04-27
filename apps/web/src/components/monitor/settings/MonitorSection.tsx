import { useMonitorStore, type AgentDisplayFormat, type MonitorPeriod, type ColumnLayout } from '../../../store/monitorStore'
import { useI18n } from '../../../lib/i18n'
import { agentDisplayLabelKeys } from '../../../lib/i18nMaps'
import { sourceLabels } from '../../../lib/constants'
import { monitorPeriodLongLabels } from '../../../lib/monitor'

const periods: MonitorPeriod[] = ['30m', '1h', '2h', '4h', '8h', '24h']
const columnLayouts: ColumnLayout[] = ['fixed', 'adaptive']
const columnLayoutLabels: Record<ColumnLayout, string> = {
  fixed: 'Fixed (Equal)', adaptive: 'Adaptive',
}
const agentDisplayFormats: AgentDisplayFormat[] = ['id', 'name', 'id:name']
const agentDisplayExamples: Record<AgentDisplayFormat, string> = {
  id: '@dev',
  name: '@Athena',
  'id:name': '@dev:Athena',
}

export function MonitorSection() {
  const monitorPeriod = useMonitorStore((s) => s.settings.monitorPeriod)
  const columnLayout = useMonitorStore((s) => s.settings.columnLayout)
  const panelConfig = useMonitorStore((s) => s.settings.panelConfig)
  const showFingerprints = useMonitorStore((s) => s.settings.showFingerprints)
  const agentDisplayFormat = useMonitorStore((s) => s.settings.agentDisplayFormat)
  const updateSettings = useMonitorStore((s) => s.updateSettings)
  const { t } = useI18n()

  const enabledCount = panelConfig.filter((p) => p.enabled).length

  function movePanel(idx: number, delta: -1 | 1) {
    const next = [...panelConfig]
    ;[next[idx + delta], next[idx]] = [next[idx], next[idx + delta]]
    updateSettings({ panelConfig: next })
  }

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
                className={`settings-option ${monitorPeriod === p ? 'active' : ''}`}
                onClick={() => updateSettings({ monitorPeriod: p })}
              >
                {monitorPeriodLongLabels[p]}
              </button>
            ))}
          </div>
          <h3 className="settings-subsection-title">{t('settings.columnLayout')}</h3>
          <p className="settings-hint">{t('settings.columnLayoutHint')}</p>
          <div className="settings-grid-2">
            {columnLayouts.map((l) => (
              <button
                key={l}
                className={`settings-option ${columnLayout === l ? 'active' : ''}`}
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
            {panelConfig.map((entry, idx) => (
              <div key={entry.tool} className="panel-config-row">
                <div className="panel-config-reorder">
                  <button
                    className="panel-move-btn"
                    disabled={idx === 0}
                    onClick={() => movePanel(idx, -1)}
                    title={t('ui.moveUp')}
                  >{'▲'}</button>
                  <button
                    className="panel-move-btn"
                    disabled={idx === panelConfig.length - 1}
                    onClick={() => movePanel(idx, 1)}
                    title={t('ui.moveDown')}
                  >{'▼'}</button>
                </div>
                <span className="panel-config-label">{sourceLabels[entry.tool]}</span>
                <button
                  className={`toggle-switch ${entry.enabled ? 'on' : ''}`}
                  disabled={entry.enabled && enabledCount <= 1}
                  onClick={() => {
                    const next = panelConfig.map((p, i) =>
                      i === idx ? { ...p, enabled: !p.enabled } : p,
                    )
                    updateSettings({ panelConfig: next })
                  }}
                  role="switch"
                  aria-checked={entry.enabled}
                  aria-label={sourceLabels[entry.tool]}
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
                className={`toggle-switch ${showFingerprints ? 'on' : ''}`}
                onClick={() => updateSettings({ showFingerprints: !showFingerprints })}
                role="switch"
                aria-checked={showFingerprints}
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
                className={`agent-display-option ${agentDisplayFormat === fmt ? 'active' : ''}`}
                onClick={() => updateSettings({ agentDisplayFormat: fmt })}
              >
                <span className="agent-display-format-label">{t(agentDisplayLabelKeys[fmt])}</span>
                <span className="agent-display-example">{agentDisplayExamples[fmt]}</span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </section>
  )
}
