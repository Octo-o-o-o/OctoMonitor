import type { AttentionItem } from '../../lib/types'
import { useI18n, type I18nKey } from '../../lib/i18n'
import { useMonitorStore } from '../../store/monitorStore'

const sourceLabels: Record<string, string> = {
  claude: 'Claude Code',
  codex: 'Codex',
  openClaw: 'OpenClaw',
}

function dismissKey(item: AttentionItem): string {
  return `${item.id}:${item.since}`
}

function localizedAttentionTitle(item: AttentionItem, t: (key: I18nKey) => string): string {
  switch (item.kind) {
    case 'permission':
      return t('attention.permission')
    case 'error':
      return t('attention.error')
    case 'source':
      return t('attention.source')
    default:
      return item.title
  }
}

export function AttentionBanner({ items }: { items: AttentionItem[] }) {
  const { t } = useI18n()
  const dismissedKeys = useMonitorStore((s) => s.dismissedAttentionKeys)
  const dismissAttention = useMonitorStore((s) => s.dismissAttention)
  const selectRun = useMonitorStore((s) => s.selectRun)

  const visible = items.filter((item) => !dismissedKeys.has(dismissKey(item)))
  if (visible.length === 0) return null

  function handleItemClick(item: AttentionItem) {
    const targetId = item.runId ?? item.id
    const el = document.querySelector(`[data-run-id="${targetId}"]`)
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'center' })
      ;(el as HTMLElement).click()
    } else {
      selectRun(targetId)
    }
  }

  function dismissAll() {
    for (const item of visible) {
      dismissAttention(dismissKey(item))
    }
  }

  return (
    <div className="attention-banner">
      <span className="attention-label">
        {t('attention.required')} ({visible.length})
      </span>
      <div className="attention-items">
        {visible.map((item) => (
          <span key={item.id} className="attention-item-pill">
            <button
              className="attention-item-btn"
              onClick={() => handleItemClick(item)}
            >
              <strong>{sourceLabels[item.tool] ?? item.tool}:</strong>{' '}
              {item.detail ?? localizedAttentionTitle(item, t)}
            </button>
          </span>
        ))}
      </div>
      <button
        className="attention-close"
        onClick={dismissAll}
        aria-label="Dismiss all"
      >
        &#10005;
      </button>
    </div>
  )
}
