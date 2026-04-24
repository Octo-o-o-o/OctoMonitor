import type { Ref } from 'react'
import { useI18n, type I18nKey } from '../../lib/i18n'
import {
  useMonitorStore,
  type MonitorQuickFilter,
} from '../../store/monitorStore'

const filters: { value: MonitorQuickFilter; labelKey: I18nKey }[] = [
  { value: 'all', labelKey: 'monitorFilter.all' },
  { value: 'attention', labelKey: 'monitorFilter.attention' },
  { value: 'active', labelKey: 'monitorFilter.active' },
]

export function MonitorFilterBar({
  searchInputRef,
}: {
  searchInputRef?: Ref<HTMLInputElement>
}) {
  const quickFilter = useMonitorStore((s) => s.monitorQuickFilter)
  const setQuickFilter = useMonitorStore((s) => s.setMonitorQuickFilter)
  const search = useMonitorStore((s) => s.monitorSearch)
  const setSearch = useMonitorStore((s) => s.setMonitorSearch)
  const { t } = useI18n()

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
