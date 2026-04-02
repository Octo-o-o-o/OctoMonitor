import { useState } from 'react'
import { apiFetch } from '../lib/api'
import { useI18n } from '../lib/i18n'

export function RemotePairingGate({ onPaired }: { onPaired: () => void }) {
  const { t } = useI18n()
  const [code, setCode] = useState('')
  const [label, setLabel] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSubmitting(true)
    setError(null)

    try {
      const response = await apiFetch('/api/pair/claim', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code, label: label || undefined }),
      })

      if (!response.ok) {
        setError(t('remote.pair.error'))
        return
      }

      onPaired()
    } catch {
      setError(t('remote.pair.error'))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="remote-pairing-gate">
      <div className="remote-pairing-card">
        <div className="section-label">{t('remote.pair.title')}</div>
        <p className="settings-hint">{t('remote.pair.hint')}</p>
        <form className="remote-pairing-form" onSubmit={handleSubmit}>
          <label className="remote-pairing-field">
            <span>{t('remote.pair.code')}</span>
            <input value={code} onChange={(event) => setCode(event.target.value.toUpperCase())} />
          </label>
          <label className="remote-pairing-field">
            <span>{t('remote.pair.label')}</span>
            <input value={label} onChange={(event) => setLabel(event.target.value)} />
          </label>
          {error && <div className="remote-pairing-error">{error}</div>}
          <button className="settings-import-btn" type="submit" disabled={submitting || code.trim() === ''}>
            {t('remote.pair.submit')}
          </button>
        </form>
      </div>
    </div>
  )
}
