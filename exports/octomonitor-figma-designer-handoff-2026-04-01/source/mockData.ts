export type SessionState = 'active' | 'waitingApproval' | 'completed' | 'error'

export interface SessionCardData {
  id: string
  state: SessionState
  title: string
  detail: string
  tag: string
  duration: string
  updated: string
  tokens: string
  messageCount: number
  model?: string
  origin?: string
}

export interface SourceColumnData {
  id: 'claude' | 'codex' | 'openClaw'
  name: string
  status: 'online' | 'offline'
  auth: string
  quotaLabel?: string
  sessions: SessionCardData[]
  scheduled?: Array<{ name: string; schedule: string; agent?: string }>
}

export const handoffData = {
  generatedAt: '2026-04-01 19:20 CST',
  header: {
    eyebrow: 'OctoMonitor / Figma Make Handoff',
    title: 'Local-first monitor for Claude Code, Codex, and OpenClaw',
    description:
      'This page intentionally flattens the monitor, inspect, usage, and settings surfaces into one readable canvas with mock data so Figma Make can restyle the product without depending on live APIs or websocket state.',
    chips: ['LIVE stream connected', '3 sources available', '1 approval queue', '27 tracked runs'],
  },
  attention: [
    'Codex billing spiked 34% in the last 24 hours after batch refactors.',
    'One OpenClaw cron is waiting for human approval before posting to Telegram.',
    'Claude Code probe is healthy but quota is above the 60% warning threshold.',
  ],
  summary: [
    { label: 'Active', value: '9', tone: 'emerald' },
    { label: 'Waiting', value: '2', tone: 'amber' },
    { label: 'Done', value: '16', tone: 'slate' },
    { label: 'Tokens', value: '1.82M', tone: 'blue' },
    { label: 'Spend', value: '$42.68', tone: 'rose' },
    { label: 'TPS', value: '62.4', tone: 'violet' },
  ],
  sources: [
    {
      id: 'claude',
      name: 'Claude Code',
      status: 'online',
      auth: 'Subscription · verified',
      quotaLabel: '5 hrs 62% · 7 days 41%',
      sessions: [
        {
          id: 'claude-1',
          state: 'active',
          title: 'Tighten monitor column spacing for small screens',
          detail: 'Responsive polish pass in apps/web with density-aware cards',
          tag: 'octomonitor-web',
          duration: '18m',
          updated: '2m ago',
          tokens: '38k',
          messageCount: 14,
          model: 'Sonnet 4',
        },
        {
          id: 'claude-2',
          state: 'waitingApproval',
          title: 'Approve brew-prefix check for installer doctor',
          detail: 'Waiting on a shell read before updating setup hints',
          tag: 'installer-probes',
          duration: '6m',
          updated: 'just now',
          tokens: '6.3k',
          messageCount: 3,
          model: 'Sonnet 4',
        },
        {
          id: 'claude-3',
          state: 'completed',
          title: 'Write accessibility audit summary for latest release',
          detail: 'Axe report saved and linked from the release checklist',
          tag: 'release-checks',
          duration: '12m',
          updated: '16m ago',
          tokens: '11k',
          messageCount: 7,
          model: 'Sonnet 4',
        },
      ],
    },
    {
      id: 'codex',
      name: 'Codex',
      status: 'online',
      auth: 'API key · verified',
      quotaLabel: 'Pay as you go',
      sessions: [
        {
          id: 'codex-1',
          state: 'active',
          title: 'Generate dedicated Figma Make handoff surface',
          detail: 'Building static TSX showcase with product-like mock content',
          tag: 'design-handoff',
          duration: '9m',
          updated: '1m ago',
          tokens: '24k',
          messageCount: 8,
          model: 'GPT-5 Codex',
        },
        {
          id: 'codex-2',
          state: 'error',
          title: 'Refactor websocket retry logic into a hook',
          detail: 'Stopped after a transient type mismatch in reconnect timer cleanup',
          tag: 'streaming-core',
          duration: '4m',
          updated: '7m ago',
          tokens: '5.8k',
          messageCount: 2,
          model: 'GPT-5 Codex',
        },
        {
          id: 'codex-3',
          state: 'completed',
          title: 'Improve usage bucketing for custom date ranges',
          detail: 'Merged slicing fix and updated tests for sparse bucket windows',
          tag: 'usage-analytics',
          duration: '23m',
          updated: '32m ago',
          tokens: '42k',
          messageCount: 16,
          model: 'GPT-5 Codex',
        },
      ],
    },
    {
      id: 'openClaw',
      name: 'OpenClaw',
      status: 'online',
      auth: 'Telegram + Cron agents',
      sessions: [
        {
          id: 'openclaw-1',
          state: 'active',
          title: 'Nightly AI release brief is assembling deployment notes',
          detail: 'Cron-driven summary is combining GitHub, Linear, and local health signals',
          tag: '@release-brief:Athena',
          duration: '3m',
          updated: 'just now',
          tokens: '9.1k',
          messageCount: 4,
          model: 'o3-mini',
          origin: 'Cron: Nightly Release Brief',
        },
        {
          id: 'openclaw-2',
          state: 'waitingApproval',
          title: 'Post incident summary to Telegram operations room',
          detail: 'Draft is ready but requires human approval before sending',
          tag: '@ops-watch:Mercury',
          duration: '11m',
          updated: '30s ago',
          tokens: '8.4k',
          messageCount: 6,
          model: 'o3-mini',
          origin: 'Telegram: Ops Room',
        },
        {
          id: 'openclaw-3',
          state: 'completed',
          title: 'Heartbeat monitor refreshed local adapter health snapshot',
          detail: 'No anomalies found across Codex, Claude Code, and companion endpoints',
          tag: '@heartbeat:Pulse',
          duration: '45s',
          updated: '5m ago',
          tokens: '1.2k',
          messageCount: 1,
          model: 'o4-mini',
          origin: 'Heartbeat',
        },
      ],
      scheduled: [
        { name: 'Nightly Release Brief', schedule: 'Weekdays · 09:00', agent: '@release-brief:Athena' },
        { name: 'Cost Spike Alert', schedule: 'Every hour', agent: '@ops-watch:Mercury' },
        { name: 'Heartbeat Sweep', schedule: 'Every 10 min', agent: '@heartbeat:Pulse' },
      ],
    },
  ] satisfies SourceColumnData[],
  inspect: {
    title: 'Selected run detail',
    runName: 'Generate dedicated Figma Make handoff surface',
    tool: 'Codex',
    state: 'active',
    summary:
      'The selected run is generating a static, readable handoff page that exposes the real information architecture of OctoMonitor while avoiding live API dependencies.',
    metadata: [
      { label: 'Workspace', value: '/WorkSpace/OctoMonitor/apps/web' },
      { label: 'Run ID', value: 'codex-1' },
      { label: 'Model', value: 'GPT-5 Codex' },
      { label: 'Started', value: '2026-04-01 19:11' },
      { label: 'Last tail', value: 'Preparing standalone mock showcase for Figma Make' },
    ],
    checklist: [
      'Show monitor, usage, settings, and inspect states on a single canvas.',
      'Keep code simple enough for Figma Make to parse without store or websocket logic.',
      'Preserve OctoMonitor vocabulary: sources, runs, approvals, quota, companion mode.',
    ],
    transcript: [
      { speaker: 'User', text: 'Export a complete frontend page with mock data so Figma Make can generate a better UI.' },
      { speaker: 'Agent', text: 'I am flattening the current tabs into one handoff page instead of exporting a screenshot.' },
      { speaker: 'Agent', text: 'The mock page will run independently from the live server and keep the structure obvious.' },
    ],
  },
  usage: {
    totals: [
      { label: 'Total tokens', value: '1.82M' },
      { label: 'Estimated cost', value: '$42.68' },
      { label: 'Tracked projects', value: '14' },
      { label: 'Peak TPS', value: '71.9' },
    ],
    bySource: [
      {
        source: 'Claude Code',
        total: '690k',
        cost: '$12.91',
        items: [
          { label: 'octomonitor-web', value: 100 },
          { label: 'release-checks', value: 58 },
          { label: 'installer-probes', value: 33 },
        ],
      },
      {
        source: 'Codex',
        total: '910k',
        cost: '$29.12',
        items: [
          { label: 'usage-analytics', value: 100 },
          { label: 'design-handoff', value: 61 },
          { label: 'streaming-core', value: 24 },
        ],
      },
      {
        source: 'OpenClaw',
        total: '220k',
        cost: '$0.65',
        items: [
          { label: '@release-brief:Athena', value: 100 },
          { label: '@ops-watch:Mercury', value: 82 },
          { label: '@heartbeat:Pulse', value: 12 },
        ],
      },
    ],
    timeline: [
      { day: 'Mon', tokens: '180k' },
      { day: 'Tue', tokens: '220k' },
      { day: 'Wed', tokens: '245k' },
      { day: 'Thu', tokens: '260k' },
      { day: 'Fri', tokens: '310k' },
      { day: 'Sat', tokens: '275k' },
      { day: 'Sun', tokens: '330k' },
    ],
  },
  settings: {
    appearance: [
      { label: 'Theme', value: 'Dark terminal, high-contrast accents' },
      { label: 'Language', value: 'English / 中文' },
      { label: 'Density', value: 'Comfortable' },
      { label: 'Font size', value: 'Default' },
    ],
    monitor: [
      { label: 'Monitor period', value: '4 hours' },
      { label: 'Column layout', value: 'Adaptive' },
      { label: 'Visible panels', value: 'Claude Code, Codex, OpenClaw' },
      { label: 'Companion mode', value: 'Enabled on same-origin host' },
    ],
    filters: [
      { source: 'Claude Code', mode: 'Exclude', patterns: ['legacy', 'sandbox-playground'] },
      { source: 'Codex', mode: 'Include', patterns: ['octomonitor', 'design'] },
      { source: 'OpenClaw', mode: 'Off', patterns: [] },
    ],
    identities: [
      { tool: 'Claude Code', identity: 'team@octomonitor.dev', auth: 'subscription', status: 'Verified' },
      { tool: 'Codex', identity: 'om_prod_key', auth: 'api_key', status: 'Configured' },
      { tool: 'OpenClaw', identity: '@octo_ops_bot', auth: 'telegram', status: 'Verified' },
    ],
    server: [
      { label: 'Listen host', value: '0.0.0.0' },
      { label: 'Listen port', value: '46321' },
      { label: 'History retention', value: '14 days' },
      { label: 'Companion host', value: 'monitor.local' },
    ],
    installer: [
      { tool: 'Codex', status: 'Detected', detail: 'CLI present and authenticated' },
      { tool: 'Claude Code', status: 'Detected', detail: 'Gateway reachable with healthy probe cadence' },
      { tool: 'OpenClaw', status: 'Missing', detail: 'Cron helper is not installed on this workstation' },
    ],
  },
  stateGallery: [
    {
      title: 'Offline source',
      text: 'A source can stay visible while its adapter is offline. The design should make that state obvious without removing historical runs.',
    },
    {
      title: 'Approval required',
      text: 'Waiting-for-approval sessions are a first-class state and should look interruptive without breaking dashboard density.',
    },
    {
      title: 'Read-only config',
      text: 'System settings are mostly inspectable. Editable controls should be clearly separated from read-only server facts.',
    },
  ],
} as const
