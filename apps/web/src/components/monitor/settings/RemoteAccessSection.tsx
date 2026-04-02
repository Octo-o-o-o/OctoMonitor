import { useEffect, useState } from 'react'
import { apiFetch } from '../../../lib/api'
import { useI18n } from '../../../lib/i18n'
import type { RemoteAccessState } from '../../../lib/types'

export function RemoteAccessSection() {
  const { t } = useI18n()
  const [remoteAccess, setRemoteAccess] = useState<RemoteAccessState | null>(null)
  const [busy, setBusy] = useState(false)

  async function refresh() {
    const response = await apiFetch('/api/remote/access')
    if (!response.ok) return
    setRemoteAccess(await response.json() as RemoteAccessState)
  }

  useEffect(() => {
    void refresh()
  }, [])

  async function toggleEnabled() {
    if (busy || !remoteAccess) return
    setBusy(true)
    try {
      const response = await apiFetch('/api/remote/access', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: !remoteAccess.enabled }),
      })
      if (!response.ok) throw new Error('remote access toggle failed')
      setRemoteAccess(await response.json() as RemoteAccessState)
    } finally {
      setBusy(false)
    }
  }

  async function createPairCode() {
    if (busy) return
    setBusy(true)
    try {
      const response = await apiFetch('/api/remote/pairings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      })
      if (!response.ok) throw new Error('pair code creation failed')
      await refresh()
    } finally {
      setBusy(false)
    }
  }

  async function revokeDevice(deviceId: string) {
    if (busy) return
    setBusy(true)
    try {
      await apiFetch(`/api/remote/devices/${deviceId}`, { method: 'DELETE' })
      await refresh()
    } finally {
      setBusy(false)
    }
  }

  return (
    <section className="settings-section">
      <div className="section-label">{t('remote.section')}</div>
      <div className="settings-cards-1">
        <div className="settings-card">
          <h3>{t('remote.enable')}</h3>
          <div className="settings-toggle-row">
            <div>
              <div className="toggle-label">{t('remote.enable')}</div>
              <div className="toggle-hint">{t('remote.enableHint')}</div>
            </div>
            <button
              className={`toggle-switch ${remoteAccess?.enabled ? 'on' : ''}`}
              onClick={() => void toggleEnabled()}
              disabled={!remoteAccess || busy}
              role="switch"
              aria-checked={remoteAccess?.enabled ?? false}
              aria-label={t('remote.enable')}
            >
              <span className="toggle-thumb" />
            </button>
          </div>
          {remoteAccess?.enabled ? (
            <>
              {remoteAccess.addresses.length > 0 && (
                <>
                  <h3 className="settings-subsection-title">{t('remote.addresses')}</h3>
                  <div className="remote-address-list">
                    {remoteAccess.addresses.map((address) => (
                      <div key={address.url} className="config-box">
                        <span className="config-label">{address.label}</span>
                        <span className="config-value">{address.url}</span>
                      </div>
                    ))}
                  </div>
                </>
              )}
              <button className="settings-import-btn" onClick={() => void createPairCode()} disabled={busy}>
                {t('remote.generateCode')}
              </button>
              {remoteAccess.pendingPairings.length > 0 && (
                <>
                  <h3 className="settings-subsection-title">{t('remote.pendingCodes')}</h3>
                  <div className="identity-list">
                    {remoteAccess.pendingPairings.map((pairing) => (
                      <div key={pairing.id} className="identity-row">
                        <div className="identity-info">
                          <strong>{pairing.code}</strong>
                          <span>{pairing.label}</span>
                        </div>
                        <div className="identity-meta">
                          <span>{t('remote.expiresAt')}</span>
                          <span>{pairing.expiresAt}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </>
              )}
              {remoteAccess.devices.length > 0 && (
                <>
                  <h3 className="settings-subsection-title">{t('remote.devices')}</h3>
                  <div className="identity-list">
                    {remoteAccess.devices.map((device) => (
                      <div key={device.id} className="identity-row">
                        <div className="identity-info">
                          <strong>{device.label}</strong>
                          <span>{device.lastSeenAt ?? device.pairedAt}</span>
                        </div>
                        <div className="identity-meta">
                          <span>{device.expiresAt}</span>
                          <button className="settings-inline-btn" onClick={() => void revokeDevice(device.id)}>
                            {t('remote.revoke')}
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                </>
              )}
            </>
          ) : (
            <p className="settings-hint">{t('remote.disabledHint')}</p>
          )}
        </div>
      </div>
    </section>
  )
}
