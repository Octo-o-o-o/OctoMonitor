import type { RunRecord } from './types'
import type { AgentDisplayFormat } from '../store/monitorStore'

export function formatTokens(n: number): string {
  const value = Math.round(n)
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}

export function formatCost(n: number | null | undefined): string {
  if (n == null) return '—'
  return `$${n.toFixed(2)}`
}

export function formatDuration(ms: number): string {
  const totalSec = Math.floor(ms / 1000)
  const h = Math.floor(totalSec / 3600)
  const m = Math.floor((totalSec % 3600) / 60)
  const s = totalSec % 60
  if (h > 0) return `${h}h${String(m).padStart(2, '0')}m`
  return `${m}m${String(s).padStart(2, '0')}s`
}

const pad2 = (n: number) => String(n).padStart(2, '0')

/** Full ISO → "YYYY-MM-DD HH:MM:SS" */
export function formatDateTime(iso: string): string {
  const d = new Date(iso)
  if (isNaN(d.getTime())) return '—'
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`
}

/** Relative: "Today HH:MM:SS" or full date */
export function formatLastUpdated(iso: string): string {
  const d = new Date(iso)
  if (isNaN(d.getTime())) return ''
  const now = new Date()
  const time = `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`
  if (d.getFullYear() === now.getFullYear() && d.getMonth() === now.getMonth() && d.getDate() === now.getDate()) {
    return `Today ${time}`
  }
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${time}`
}

export function formatAgentTag(run: RunRecord, format: AgentDisplayFormat): string {
  if (!run.agentName) return run.workspaceShort
  const id = run.agentName
  const name = run.agentDisplayName
  switch (format) {
    case 'name': return name ? `@${name}` : `@${id}`
    case 'id:name': return name ? `@${id}:${name}` : `@${id}`
    default: return `@${id}`
  }
}

export function getGroupKey(run: RunRecord, agentDisplayFormat: AgentDisplayFormat): string {
  if (run.tool === 'openClaw') return formatAgentTag(run, agentDisplayFormat)
  return run.workspaceShort
}
