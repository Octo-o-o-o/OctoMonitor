import { endOfDay, startOfDay } from './dateRange'
import type { HistorySelection } from './history'
import type { CommitRecord, RunRecord, UsageBucket } from './types'
import { buildUsageBucketIndex, sliceRunUsage, type UsageBucketIndex } from './usage'

export type HeatmapMetric = 'input' | 'token' | 'commit'
export type HeatmapScope = 'week' | 'month' | 'total'
export type HeatmapCellLevel = 0 | 1 | 2 | 3 | 4

export interface HeatmapCell {
  key: string
  startMs: number
  endMs: number
  date: Date
  hour: number | null
  value: number
  level: HeatmapCellLevel
  hidden?: boolean
}

export interface HeatmapDayTotal {
  date: Date
  total: number
}

export interface HeatmapSummary {
  total: number
  peakCell: HeatmapCell
  topDay: HeatmapDayTotal
  activeDays: number
  longestStreak: number
}

export interface HeatmapHourlyRow {
  key: string
  date: Date
  total: number
  cells: HeatmapCell[]
}

export interface HeatmapMonthMarker {
  key: string
  date: Date
  column: number
}

export interface HeatmapCalendarColumn {
  key: string
  cells: HeatmapCell[]
}

export type HeatmapViewModel =
  | {
    kind: 'hourly'
    scope: Extract<HeatmapScope, 'week' | 'month'>
    range: HistorySelection
    rows: HeatmapHourlyRow[]
    cells: HeatmapCell[]
    summary: HeatmapSummary
  }
  | {
    kind: 'calendar'
    scope: 'total'
    range: HistorySelection
    columns: HeatmapCalendarColumn[]
    cells: HeatmapCell[]
    monthMarkers: HeatmapMonthMarker[]
    summary: HeatmapSummary
  }

interface ParsedCommit {
  record: CommitRecord
  committedAtMs: number
}

interface BuildHeatmapViewOptions {
  scope: HeatmapScope
  metric: HeatmapMetric
  now?: Date
  runs: RunRecord[]
  usageBuckets: UsageBucket[]
  commits: CommitRecord[]
}

const TOTAL_VISIBLE_DAYS = 365
const TOTAL_WEEK_COLUMNS = 53

function addDays(date: Date, days: number): Date {
  const next = new Date(date)
  next.setDate(next.getDate() + days)
  return next
}

function startOfWeek(date: Date): Date {
  const next = startOfDay(date)
  next.setDate(next.getDate() - ((next.getDay() + 6) % 7))
  return next
}

function formatDateKey(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function buildTotalSelection(now: Date): HistorySelection {
  const to = endOfDay(now)
  const visibleStart = startOfDay(addDays(to, -(TOTAL_VISIBLE_DAYS - 1)))
  const from = startOfWeek(visibleStart)
  return { from, to }
}

export function getHeatmapScopeSelection(scope: HeatmapScope, now: Date = new Date()): HistorySelection {
  if (scope === 'week') {
    return {
      from: startOfDay(addDays(now, -6)),
      to: endOfDay(now),
    }
  }
  if (scope === 'month') {
    return {
      from: startOfDay(addDays(now, -30)),
      to: endOfDay(now),
    }
  }
  return buildTotalSelection(now)
}

function parseCommits(commits: CommitRecord[]): ParsedCommit[] {
  return commits
    .map((record) => ({
      record,
      committedAtMs: Date.parse(record.committedAt),
    }))
    .filter((item) => Number.isFinite(item.committedAtMs))
    .sort((a, b) => b.committedAtMs - a.committedAtMs)
}

function measureUsageMetric(
  metric: Extract<HeatmapMetric, 'input' | 'token'>,
  runs: RunRecord[],
  bucketByRunId: UsageBucketIndex,
  startMs: number,
  endMs: number,
): number {
  let total = 0

  for (const run of runs) {
    const slice = sliceRunUsage(run, bucketByRunId.get(run.id), startMs, endMs)
    total += metric === 'input' ? slice.messageCount : slice.totalTokens
  }

  return total
}

function measureCommitMetric(
  parsedCommits: ParsedCommit[],
  startMs: number,
  endMs: number,
): number {
  let count = 0
  for (const commit of parsedCommits) {
    if (commit.committedAtMs < startMs) break
    if (commit.committedAtMs < endMs) {
      count += 1
    }
  }
  return count
}

function buildLevelThresholds(values: number[]): [number, number, number, number] {
  const nonZero = values.filter((value) => value > 0).sort((a, b) => a - b)
  if (nonZero.length === 0) return [0, 0, 0, 0]

  const pick = (percent: number) => {
    const index = Math.min(
      nonZero.length - 1,
      Math.floor((nonZero.length - 1) * percent),
    )
    return nonZero[index]
  }

  return [pick(0.2), pick(0.45), pick(0.7), pick(0.9)]
}

function toLevel(value: number, thresholds: [number, number, number, number]): HeatmapCellLevel {
  if (value <= 0) return 0
  if (value <= thresholds[0]) return 1
  if (value <= thresholds[1]) return 2
  if (value <= thresholds[2]) return 3
  return 4
}

function buildSummary(cells: HeatmapCell[], dayTotals: HeatmapDayTotal[]): HeatmapSummary {
  const visibleCells = cells.filter((cell) => !cell.hidden)
  const peakCell = visibleCells.reduce((best, cell) => {
    if (!best) return cell
    if (cell.value > best.value) return cell
    if (cell.value === best.value && cell.startMs > best.startMs) return cell
    return best
  }, visibleCells[0])
  const topDay = dayTotals.reduce((best, day) => {
    if (!best) return day
    if (day.total > best.total) return day
    if (day.total === best.total && day.date.getTime() > best.date.getTime()) return day
    return best
  }, dayTotals[0])

  let longestStreak = 0
  let currentStreak = 0
  let total = 0
  let activeDays = 0

  for (const day of dayTotals) {
    total += day.total
    if (day.total > 0) {
      currentStreak += 1
      activeDays += 1
      longestStreak = Math.max(longestStreak, currentStreak)
    } else {
      currentStreak = 0
    }
  }

  return {
    total,
    peakCell,
    topDay,
    activeDays,
    longestStreak,
  }
}

function buildHourlyRows(
  scope: Extract<HeatmapScope, 'week' | 'month'>,
  metric: HeatmapMetric,
  range: HistorySelection,
  runs: RunRecord[],
  usageBuckets: UsageBucket[],
  parsedCommits: ParsedCommit[],
): HeatmapViewModel {
  const bucketByRunId = buildUsageBucketIndex(usageBuckets)
  const rows: HeatmapHourlyRow[] = []
  const cells: HeatmapCell[] = []
  const dayTotals: HeatmapDayTotal[] = []

  for (
    let cursor = startOfDay(range.from);
    cursor.getTime() <= range.to.getTime();
    cursor = addDays(cursor, 1)
  ) {
    const rowDate = new Date(cursor)
    const rowCells: HeatmapCell[] = []
    let total = 0

    for (let hour = 0; hour < 24; hour += 1) {
      const cellDate = new Date(
        rowDate.getFullYear(),
        rowDate.getMonth(),
        rowDate.getDate(),
        hour,
        0,
        0,
        0,
      )
      const startMs = cellDate.getTime()
      const endMs = startMs + 60 * 60 * 1000
      const value = metric === 'commit'
        ? measureCommitMetric(parsedCommits, startMs, endMs)
        : measureUsageMetric(metric, runs, bucketByRunId, startMs, endMs)
      const cell: HeatmapCell = {
        key: `${scope}:${metric}:${formatDateKey(rowDate)}:${hour}`,
        startMs,
        endMs,
        date: cellDate,
        hour,
        value,
        level: 0,
      }
      total += value
      rowCells.push(cell)
      cells.push(cell)
    }

    rows.push({
      key: formatDateKey(rowDate),
      date: rowDate,
      total,
      cells: rowCells,
    })
    dayTotals.push({ date: rowDate, total })
  }

  const thresholds = buildLevelThresholds(cells.map((cell) => cell.value))
  for (const cell of cells) {
    cell.level = toLevel(cell.value, thresholds)
  }

  return {
    kind: 'hourly',
    scope,
    range,
    rows,
    cells,
    summary: buildSummary(cells, dayTotals),
  }
}

function buildCalendarColumns(
  metric: HeatmapMetric,
  range: HistorySelection,
  runs: RunRecord[],
  usageBuckets: UsageBucket[],
  parsedCommits: ParsedCommit[],
): HeatmapViewModel {
  const bucketByRunId = buildUsageBucketIndex(usageBuckets)
  const visibleStart = startOfDay(addDays(range.to, -(TOTAL_VISIBLE_DAYS - 1)))
  const columns: HeatmapCalendarColumn[] = []
  const cells: HeatmapCell[] = []
  const visibleDayTotals: HeatmapDayTotal[] = []

  for (let column = 0; column < TOTAL_WEEK_COLUMNS; column += 1) {
    const columnCells: HeatmapCell[] = []
    for (let dayIndex = 0; dayIndex < 7; dayIndex += 1) {
      const cellDate = addDays(range.from, column * 7 + dayIndex)
      const startMs = cellDate.getTime()
      const endMs = startMs + 24 * 60 * 60 * 1000
      const hidden = startMs < visibleStart.getTime() || startMs > range.to.getTime()
      const value = hidden
        ? 0
        : metric === 'commit'
          ? measureCommitMetric(parsedCommits, startMs, endMs)
          : measureUsageMetric(metric, runs, bucketByRunId, startMs, endMs)
      const cell: HeatmapCell = {
        key: `total:${metric}:${formatDateKey(cellDate)}`,
        startMs,
        endMs,
        date: cellDate,
        hour: null,
        value,
        level: 0,
        hidden,
      }
      columnCells.push(cell)
      cells.push(cell)
      if (!hidden) {
        visibleDayTotals.push({ date: cellDate, total: value })
      }
    }
    columns.push({
      key: `week:${column}`,
      cells: columnCells,
    })
  }

  const thresholds = buildLevelThresholds(cells.filter((cell) => !cell.hidden).map((cell) => cell.value))
  for (const cell of cells) {
    if (!cell.hidden) {
      cell.level = toLevel(cell.value, thresholds)
    }
  }

  const monthMarkers: HeatmapMonthMarker[] = []
  let previousMonthKey = ''
  columns.forEach((column, index) => {
    const firstVisibleCell = column.cells.find((cell) => !cell.hidden)
    if (!firstVisibleCell) return
    const monthKey = `${firstVisibleCell.date.getFullYear()}-${firstVisibleCell.date.getMonth()}`
    if (monthKey === previousMonthKey) return
    previousMonthKey = monthKey
    monthMarkers.push({
      key: monthKey,
      date: firstVisibleCell.date,
      column: index,
    })
  })

  return {
    kind: 'calendar',
    scope: 'total',
    range,
    columns,
    cells,
    monthMarkers,
    summary: buildSummary(cells, visibleDayTotals),
  }
}

export function buildHeatmapView({
  scope,
  metric,
  now = new Date(),
  runs,
  usageBuckets,
  commits,
}: BuildHeatmapViewOptions): HeatmapViewModel {
  const range = getHeatmapScopeSelection(scope, now)
  const parsedCommits = parseCommits(commits)

  if (scope === 'total') {
    return buildCalendarColumns(metric, range, runs, usageBuckets, parsedCommits)
  }

  return buildHourlyRows(scope, metric, range, runs, usageBuckets, parsedCommits)
}

export function listCommitsInWindow(
  commits: CommitRecord[],
  startMs: number,
  endMs: number,
): CommitRecord[] {
  return parseCommits(commits)
    .filter((item) => item.committedAtMs >= startMs && item.committedAtMs < endMs)
    .map((item) => item.record)
}
