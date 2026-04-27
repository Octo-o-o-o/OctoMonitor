import { useEffect } from 'react'
import { useMonitorStore } from '../store/monitorStore'
import { useI18n } from '../lib/i18n'
import { getRuntimeMode } from '../lib/runtimeMode'
import { isTauriEnvironment } from '../lib/runtimeEnvironment'

export function ShortcutOverlay() {
  const show = useMonitorStore((s) => s.showShortcutHelp)
  const toggle = useMonitorStore((s) => s.toggleShortcutHelp)
  const { t } = useI18n()
  const runtimeMode = getRuntimeMode()
  const showDesktopShortcuts = isTauriEnvironment()
  const isRemote = runtimeMode === 'remoteViewer'
  const shortcuts = [
    {
      key: isRemote ? '1 / 2' : '1 / 2 / 3 / 4 / 5',
      action: t(isRemote ? 'shortcut.switchTabRemote' : 'shortcut.switchTab'),
    },
    { key: 'j / k', action: t('shortcut.navigate') },
    { key: 'Enter', action: t('shortcut.openDrawer') },
    { key: 'Esc', action: t('shortcut.closeDrawer') },
    { key: '?', action: t('shortcut.toggleHelp') },
    ...(showDesktopShortcuts ? [
      { key: 'Cmd / Ctrl + ,', action: t('shortcut.openSettings') },
      { key: 'Cmd / Ctrl + + / - / 0', action: t('shortcut.zoom') },
      { key: 'Cmd / Ctrl + Z / X / C / V / A', action: t('shortcut.nativeEdit') },
    ] : []),
  ] as const

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
              <span className="shortcut-action">{s.action}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
