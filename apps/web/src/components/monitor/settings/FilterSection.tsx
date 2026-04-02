import { useState } from 'react'
import { useMonitorStore, type FilterMode, type FilterRules } from '../../../store/monitorStore'
import { useI18n } from '../../../lib/i18n'
import type { ToolKind } from '../../../lib/types'

const panelLabels: Record<ToolKind, string> = {
  claude: 'Claude Code', codex: 'Codex', openClaw: 'OpenClaw',
}

export function FilterSection() {
  const settings = useMonitorStore((s) => s.settings)
  const updateSettings = useMonitorStore((s) => s.updateSettings)
  const { t } = useI18n()
  const [filterInputs, setFilterInputs] = useState<Record<string, string>>({})

  function updateFilter(tool: ToolKind, patch: Partial<FilterRules[ToolKind]>) {
    const current = settings.filterRules[tool]
    updateSettings({
      filterRules: { ...settings.filterRules, [tool]: { ...current, ...patch } },
    })
  }

  function addFilterPattern(tool: ToolKind) {
    const val = (filterInputs[tool] ?? '').trim()
    if (!val) return
    const current = settings.filterRules[tool]
    if (current.patterns.includes(val)) return
    updateFilter(tool, { patterns: [...current.patterns, val] })
    setFilterInputs((prev) => ({ ...prev, [tool]: '' }))
  }

  function removeFilterPattern(tool: ToolKind, pattern: string) {
    const current = settings.filterRules[tool]
    updateFilter(tool, { patterns: current.patterns.filter((p) => p !== pattern) })
  }

  return (
    <section className="settings-section">
      <div className="section-label">{t('settings.filterRules')}</div>
      <div className="settings-cards-1">
        <p className="settings-hint">{t('settings.filterRulesHint')}</p>
        <div className="filter-rules-list">
          {(['claude', 'codex', 'openClaw'] as ToolKind[]).map((tool) => {
            const filter = settings.filterRules[tool]
            const isProject = tool !== 'openClaw'
            const placeholder = isProject
              ? t('settings.filterPlaceholder.project')
              : t('settings.filterPlaceholder.agent')
            return (
              <div key={tool} className="filter-rule-group">
                <div className="filter-rule-header">
                  <div className="filter-rule-title">
                    <strong>{panelLabels[tool]}</strong>
                    <span className="filter-dimension">{isProject ? t('ui.project') : t('ui.agent')}</span>
                  </div>
                  <div className="filter-mode-row">
                    {(['off', 'include', 'exclude'] as FilterMode[]).map((mode) => (
                      <button
                        key={mode}
                        className={`settings-option small ${filter.mode === mode ? 'active' : ''}`}
                        onClick={() => updateFilter(tool, { mode })}
                      >
                        {t(`settings.filterMode.${mode}` as any)}
                      </button>
                    ))}
                  </div>
                </div>
                {filter.mode !== 'off' && (
                  <>
                    <div className="filter-patterns">
                      {filter.patterns.length === 0 && (
                        <span className="filter-empty">{t('settings.filterEmpty')}</span>
                      )}
                      {filter.patterns.map((p) => (
                        <span key={p} className="filter-chip">
                          {p}
                          <button className="filter-chip-remove" onClick={() => removeFilterPattern(tool, p)}>&times;</button>
                        </span>
                      ))}
                    </div>
                    <div className="filter-input-row">
                      <input
                        className="filter-input"
                        placeholder={placeholder}
                        value={filterInputs[tool] ?? ''}
                        onChange={(e) => setFilterInputs((prev) => ({ ...prev, [tool]: e.target.value }))}
                        onKeyDown={(e) => { if (e.key === 'Enter') addFilterPattern(tool) }}
                      />
                      <button className="filter-add-btn" onClick={() => addFilterPattern(tool)}>+</button>
                    </div>
                  </>
                )}
              </div>
            )
          })}
        </div>
      </div>
    </section>
  )
}
