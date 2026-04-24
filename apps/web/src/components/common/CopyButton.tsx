import { useCallback, useState } from 'react'
import { useI18n } from '../../lib/i18n'
import { useToastStore } from '../../store/toastStore'

interface CopyButtonProps {
  text: string
  ariaLabel: string
  disabled?: boolean
  className?: string
}

export function CopyButton({ text, ariaLabel, disabled, className }: CopyButtonProps) {
  const { t } = useI18n()
  const pushToast = useToastStore((s) => s.pushToast)
  const [busy, setBusy] = useState(false)

  const handleCopy = useCallback(async () => {
    if (busy || disabled || !text) return
    setBusy(true)
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error('clipboard unavailable')
      }
      await navigator.clipboard.writeText(text)
      pushToast({ kind: 'info', message: t('toast.copied') })
    } catch {
      pushToast({ kind: 'error', message: t('toast.copyFailed') })
    } finally {
      setBusy(false)
    }
  }, [busy, disabled, pushToast, t, text])

  return (
    <button
      type="button"
      className={`copy-button${className ? ` ${className}` : ''}`}
      onClick={handleCopy}
      disabled={disabled || busy || !text}
      aria-label={ariaLabel}
      title={ariaLabel}
    >
      <span aria-hidden>⧉</span>
    </button>
  )
}
