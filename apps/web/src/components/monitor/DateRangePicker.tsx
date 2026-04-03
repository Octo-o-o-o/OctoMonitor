import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useI18n } from '../../lib/i18n'

export interface DateRange {
  from: Date
  to: Date
}

type PresetKey = '1d' | '3d' | '7d' | '14d' | '30d' | '90d' | '180d' | 'all' | 'custom'
type FixedPresetKey = Exclude<PresetKey, 'custom' | 'all'>
type SelectablePresetKey = Exclude<PresetKey, 'custom'>

const PRESET_DAYS: Record<FixedPresetKey, number> = {
  '1d': 1,
  '3d': 3,
  '7d': 7,
  '14d': 14,
  '30d': 30,
  '90d': 90,
  '180d': 180,
}

function startOfDay(d: Date): Date {
  const r = new Date(d)
  r.setHours(0, 0, 0, 0)
  return r
}

function endOfDay(d: Date): Date {
  const r = new Date(d)
  r.setHours(23, 59, 59, 999)
  return r
}

function isSameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
}

function rangeForPreset(
  key: Exclude<PresetKey, 'custom'>,
  allRange?: DateRange | null,
): DateRange {
  if (key === 'all' && allRange) {
    return { from: startOfDay(allRange.from), to: endOfDay(allRange.to) }
  }

  const to = endOfDay(new Date())
  const from = startOfDay(new Date())
  const days = key === 'all' ? 30 : PRESET_DAYS[key as FixedPresetKey]
  from.setDate(from.getDate() - days + 1)
  return { from, to }
}

function presetForRange(
  value: DateRange,
  allRange: DateRange | null | undefined,
  presets: SelectablePresetKey[],
): PresetKey {
  if (allRange && presets.includes('all')) {
    const all = rangeForPreset('all', allRange)
    if (isSameDay(all.from, value.from) && isSameDay(all.to, value.to)) {
      return 'all'
    }
  }

  for (const key of presets.filter((preset): preset is FixedPresetKey => preset !== 'all')) {
    const preset = rangeForPreset(key, allRange)
    if (isSameDay(preset.from, value.from) && isSameDay(preset.to, value.to)) {
      return key
    }
  }
  return 'custom'
}

function formatShort(d: Date, locale: string): string {
  return d.toLocaleDateString(locale === 'zh' ? 'zh-CN' : 'en-US', {
    month: 'short',
    day: 'numeric',
  })
}

function daysInMonth(year: number, month: number): number {
  return new Date(year, month + 1, 0).getDate()
}

function MiniCalendar({
  value,
  rangeStart,
  rangeEnd,
  onSelect,
  locale,
}: {
  value: Date
  rangeStart: Date | null
  rangeEnd: Date | null
  onSelect: (d: Date) => void
  locale: string
}) {
  const [viewYear, setViewYear] = useState(value.getFullYear())
  const [viewMonth, setViewMonth] = useState(value.getMonth())

  const days = useMemo(() => {
    const total = daysInMonth(viewYear, viewMonth)
    const firstDow = new Date(viewYear, viewMonth, 1).getDay()
    const cells: (Date | null)[] = []
    for (let i = 0; i < firstDow; i++) cells.push(null)
    for (let d = 1; d <= total; d++) cells.push(new Date(viewYear, viewMonth, d))
    return cells
  }, [viewYear, viewMonth])

  const weekLabels = locale === 'zh'
    ? ['日', '一', '二', '三', '四', '五', '六']
    : ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa']

  const monthLabel = new Date(viewYear, viewMonth).toLocaleDateString(
    locale === 'zh' ? 'zh-CN' : 'en-US',
    { month: 'long', year: 'numeric' },
  )

  const prev = () => {
    if (viewMonth === 0) { setViewYear(viewYear - 1); setViewMonth(11) }
    else setViewMonth(viewMonth - 1)
  }
  const next = () => {
    if (viewMonth === 11) { setViewYear(viewYear + 1); setViewMonth(0) }
    else setViewMonth(viewMonth + 1)
  }

  const today = startOfDay(new Date())

  return (
    <div className="drp-calendar">
      <div className="drp-cal-header">
        <button className="drp-cal-nav" onClick={prev}>‹</button>
        <span className="drp-cal-title">{monthLabel}</span>
        <button className="drp-cal-nav" onClick={next}>›</button>
      </div>
      <div className="drp-cal-weekdays">
        {weekLabels.map((w) => (
          <span key={w} className="drp-cal-wd">{w}</span>
        ))}
      </div>
      <div className="drp-cal-grid">
        {days.map((d, i) => {
          if (!d) return <span key={`e${i}`} className="drp-cal-empty" />
          const isToday = isSameDay(d, today)
          const isFuture = d > today
          const isStart = rangeStart && isSameDay(d, rangeStart)
          const isEnd = rangeEnd && isSameDay(d, rangeEnd)
          const inRange = rangeStart && rangeEnd && d >= startOfDay(rangeStart) && d <= endOfDay(rangeEnd)
          const cls = [
            'drp-cal-day',
            isToday && 'is-today',
            isFuture && 'is-future',
            isStart && 'is-start',
            isEnd && 'is-end',
            inRange && !isStart && !isEnd && 'in-range',
          ].filter(Boolean).join(' ')
          return (
            <button
              key={d.getTime()}
              className={cls}
              disabled={isFuture}
              onClick={() => onSelect(d)}
            >
              {d.getDate()}
            </button>
          )
        })}
      </div>
    </div>
  )
}

interface Props {
  value: DateRange
  onChange: (range: DateRange) => void
  allRange?: DateRange | null
  presets?: readonly SelectablePresetKey[]
}

export function DateRangePicker({ value, onChange, allRange, presets }: Props) {
  const { locale } = useI18n()
  const availablePresets = useMemo<SelectablePresetKey[]>(
    () => presets?.filter((preset) => preset !== 'all' || Boolean(allRange)) ?? ['1d', '3d', '7d', '14d', '30d', ...(allRange ? ['all' as const] : [])],
    [allRange, presets],
  )
  const [open, setOpen] = useState(false)
  const [activePreset, setActivePreset] = useState<PresetKey>(() => presetForRange(value, allRange, availablePresets))
  const [customStep, setCustomStep] = useState<'from' | 'to'>('from')
  const [customFrom, setCustomFrom] = useState<Date | null>(null)
  const [customTo, setCustomTo] = useState<Date | null>(null)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    setActivePreset(presetForRange(value, allRange, availablePresets))
  }, [value, allRange, availablePresets])

  useEffect(() => {
    if (!open) return
    const onMouseDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', onMouseDown)
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('mousedown', onMouseDown)
      document.removeEventListener('keydown', onKeyDown)
    }
  }, [open])

  const selectPreset = useCallback((key: Exclude<PresetKey, 'custom'>) => {
    setActivePreset(key)
    setCustomFrom(null)
    setCustomTo(null)
    onChange(rangeForPreset(key, allRange))
    setOpen(false)
  }, [allRange, onChange])

  const enterCustom = useCallback(() => {
    setActivePreset('custom')
    setCustomStep('from')
    setCustomFrom(null)
    setCustomTo(null)
  }, [])

  const onCalendarSelect = useCallback((d: Date) => {
    if (customStep === 'from') {
      setCustomFrom(d)
      setCustomTo(null)
      setCustomStep('to')
    } else {
      let from = customFrom!
      let to = d
      if (to < from) { [from, to] = [to, from] }
      setCustomFrom(from)
      setCustomTo(to)
      onChange({ from: startOfDay(from), to: endOfDay(to) })
      setOpen(false)
    }
  }, [customStep, customFrom, onChange])

  // Display label
  const label = useMemo(() => {
    if (activePreset !== 'custom') {
      if (activePreset === 'all') {
        return locale === 'zh' ? '全部' : 'All'
      }
      const days = PRESET_DAYS[activePreset]
      if (days === 1) {
        return locale === 'zh' ? '今天' : 'Today'
      }
      return locale === 'zh' ? `最近 ${days} 天` : `Last ${days}d`
    }
    const f = formatShort(value.from, locale)
    const t = formatShort(value.to, locale)
    return `${f} – ${t}`
  }, [activePreset, value, locale])

  const presetLabels: Record<Exclude<PresetKey, 'custom'>, string> = locale === 'zh'
    ? { '1d': '今天', '3d': '3天', '7d': '7天', '14d': '14天', '30d': '30天', '90d': '90天', '180d': '180天', 'all': '全部' }
    : { '1d': 'Today', '3d': '3d', '7d': '7d', '14d': '14d', '30d': '30d', '90d': '90d', '180d': '180d', 'all': 'All' }

  const customLabel = locale === 'zh' ? '自定义' : 'Custom'

  return (
    <div className="drp-wrap" ref={ref}>
      <button className="drp-trigger" onClick={() => setOpen(!open)}>
        <svg className="drp-icon" viewBox="0 0 16 16" fill="none" width="14" height="14">
          <rect x="1" y="2.5" width="14" height="12" rx="2" stroke="currentColor" strokeWidth="1.3" />
          <path d="M1 6.5h14" stroke="currentColor" strokeWidth="1.3" />
          <path d="M5 1v3M11 1v3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
        {label}
        <svg className="drp-chevron" viewBox="0 0 10 6" width="8" height="5" fill="none">
          <path d="M1 1l4 4 4-4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {open && (
        <div className="drp-dropdown">
          <div className="drp-presets">
            {availablePresets.map((k) => (
              <button
                key={k}
                className={`drp-preset${activePreset === k ? ' active' : ''}`}
                onClick={() => selectPreset(k)}
              >
                {presetLabels[k]}
              </button>
            ))}
            <button
              className={`drp-preset${activePreset === 'custom' ? ' active' : ''}`}
              onClick={enterCustom}
            >
              {customLabel}
            </button>
          </div>

          {activePreset === 'custom' && (
            <div className="drp-custom-body">
              <div className="drp-custom-hint">
                {customStep === 'from'
                  ? (locale === 'zh' ? '选择起始日期' : 'Select start date')
                  : (locale === 'zh' ? '选择结束日期' : 'Select end date')}
              </div>
              <MiniCalendar
                value={new Date()}
                rangeStart={customFrom}
                rangeEnd={customTo}
                onSelect={onCalendarSelect}
                locale={locale}
              />
            </div>
          )}
        </div>
      )}
    </div>
  )
}
