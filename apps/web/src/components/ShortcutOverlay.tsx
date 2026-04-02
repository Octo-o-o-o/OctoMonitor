import { useEffect } from 'react'
import { useMonitorStore } from '../store/monitorStore'
import { useI18n } from '../lib/i18n'

const shortcuts = [
  { key: '1 / 2 / 3 / 4', action: 'shortcut.switchTab' },
  { key: 'j / k', action: 'shortcut.navigate' },
  { key: 'Enter', action: 'shortcut.openDrawer' },
  { key: 'Esc', action: 'shortcut.closeDrawer' },
  { key: '?', action: 'shortcut.toggleHelp' },
] as const

export function ShortcutOverlay() {
  const show = useMonitorStore((s) => s.showShortcutHelp)
  const toggle = useMonitorStore((s) => s.toggleShortcutHelp)
  const { t } = useI18n()

  useEffect(() => {
    if (!show) return
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape' || e.key === '?') {
        e.preventDefault()
        toggle()
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [show, toggle])

  if (!show) return null

  return (
    <div className="shortcut-overlay" onClick={(e) => {
      if (e.target === e.currentTarget) toggle()
    }}>
      <div className="shortcut-panel" role="dialog" aria-modal="true" aria-labelledby="shortcut-title">
        <h3 id="shortcut-title" className="shortcut-title">{t('shortcut.title')}</h3>
        <div className="shortcut-list">
          {shortcuts.map((s) => (
            <div key={s.key} className="shortcut-row">
              <kbd className="shortcut-key">{s.key}</kbd>
              <span className="shortcut-action">{t(s.action)}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
