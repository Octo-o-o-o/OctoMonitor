import type { ToolKind } from './types'

export const allTools: ToolKind[] = ['claude', 'codex', 'openClaw', 'hermes']

/** Title-case labels for each tool/source, suitable for prose or mixed-case UI. */
export const sourceLabels: Record<ToolKind, string> = {
  claude: 'Claude Code',
  codex: 'Codex',
  openClaw: 'OpenClaw',
  hermes: 'Hermes',
}

export const toolBadges: Record<ToolKind, { short: string; cls: string }> = {
  claude: { short: 'CC', cls: 'wf-tool-claude' },
  codex: { short: 'CX', cls: 'wf-tool-codex' },
  openClaw: { short: 'OC', cls: 'wf-tool-openclaw' },
  hermes: { short: 'HM', cls: 'wf-tool-hermes' },
}

/** Upper-case labels for section headers and stat tables. */
export const sourceLabelsUpper: Record<ToolKind, string> = {
  claude: 'CLAUDE CODE',
  codex: 'CODEX',
  openClaw: 'OPENCLAW',
  hermes: 'HERMES',
}
