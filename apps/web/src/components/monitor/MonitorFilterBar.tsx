import type { Ref } from 'react'
import { sourceLabels } from '../../lib/constants'
import type { ToolKind } from '../../lib/types'
import { useI18n, type I18nKey } from '../../lib/i18n'
import {
  useMonitorStore,
  type MonitorQuickFilter,
  type MonitorToolFilter,
} from '../../store/monitorStore'

const filters: { value: MonitorQuickFilter; labelKey: I18nKey }[] = [
  { value: 'all', labelKey: 'monitorFilter.all' },
  { value: 'attention', labelKey: 'monitorFilter.attention' },
  { value: 'active', labelKey: 'monitorFilter.active' },
]

export function MonitorFilterBar({
  searchInputRef,
  visibleTools,
}: {
  searchInputRef?: Ref<HTMLInputElement>
  visibleTools: ToolKind[]
}) {
  const quickFilter = useMonitorStore((s) => s.monitorQuickFilter)
  const setQuickFilter = useMonitorStore((s) => s.setMonitorQuickFilter)
  const toolFilter = useMonitorStore((s) => s.monitorToolFilter)
  const setToolFilter = useMonitorStore((s) => s.setMonitorToolFilter)
  const search = useMonitorStore((s) => s.monitorSearch)
  const setSearch = useMonitorStore((s) => s.setMonitorSearch)
  const { t } = useI18n()
  const effectiveToolFilter = toolFilter === 'all' || visibleTools.includes(toolFilter)
    ? toolFilter
    : 'all'

  return (
    <div className="monitor-filter-bar" role="toolbar" aria-label="monitor filter">
      <div className="monitor-filter-chips">
        {filters.map((f) => (
          <button
            key={f.value}
            type="button"
            className={`monitor-filter-chip${quickFilter === f.value ? ' monitor-filter-chip--active' : ''}`}
            onClick={() => setQuickFilter(f.value)}
            aria-pressed={quickFilter === f.value}
          >
            {t(f.labelKey)}
          </button>
        ))}
      </div>
      <select
        className="monitor-filter-tool"
        value={effectiveToolFilter}
        onChange={(e) => setToolFilter(e.target.value as MonitorToolFilter)}
        aria-label={t('monitorFilter.tool')}
      >
        <option value="all">{t('monitorFilter.allTools')}</option>
        {visibleTools.map((tool) => (
          <option key={tool} value={tool}>{sourceLabels[tool]}</option>
        ))}
      </select>
      <input
        ref={searchInputRef}
        type="search"
        className="monitor-filter-search"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder={t('monitorFilter.searchPlaceholder')}
        aria-label={t('monitorFilter.searchPlaceholder')}
      />
    </div>
  )
}
