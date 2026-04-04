import type { ToolKind } from './types'

export const allTools: ToolKind[] = ['claude', 'codex', 'openClaw']

/** Title-case labels for each tool/source, suitable for prose or mixed-case UI. */
export const sourceLabels: Record<ToolKind, string> = {
  claude: 'Claude Code',
  codex: 'Codex',
  openClaw: 'OpenClaw',
}

/** Upper-case labels for section headers and stat tables. */
export const sourceLabelsUpper: Record<ToolKind, string> = {
  claude: 'CLAUDE CODE',
  codex: 'CODEX',
  openClaw: 'OPENCLAW',
}
