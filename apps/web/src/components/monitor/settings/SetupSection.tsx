import { useEffect, useMemo, useRef, useState } from 'react'
import { useI18n } from '../../../lib/i18n'
import { apiFetch } from '../../../lib/api'
import { allTools, sourceLabels } from '../../../lib/constants'
import type { DataSourceHealth, RunRecord, ToolKind } from '../../../lib/types'
import { useMonitorStore } from '../../../store/monitorStore'

type SupportLevel = 'monitored' | 'experimental' | 'candidate'
type Capability = {
  tool: string
  detected: boolean
  detectedCommand?: string | null
  mode: string
  supportLevel?: SupportLevel
  notes: string
}

function capabilityLabel(tool: string): string {
  return sourceLabels[tool as ToolKind] ?? tool
}

function latestRunForTool(runs: RunRecord[], tool: ToolKind): RunRecord | undefined {
  return runs
    .filter((run) => run.tool === tool)
    .sort((a, b) => b.lastActivityAt.localeCompare(a.lastActivityAt))[0]
}

function firstDataSource(run?: RunRecord): DataSourceHealth | undefined {
  return run?.dataSources?.[0]
}

function formatDate(value?: string | null): string {
  if (!value) return 'N/A'
  const ms = Date.parse(value)
  if (Number.isNaN(ms)) return value
  return new Date(ms).toLocaleString()
}

function setToolPresence(list: ToolKind[], tool: ToolKind, present: boolean): ToolKind[] {
  const next = new Set(list)
  if (present) next.add(tool)
  else next.delete(tool)
  return [...next]
}

export function SetupSection() {
  const { t } = useI18n()
  const data = useMonitorStore((s) => s.data)
  const setConfig = useMonitorStore((s) => s.setConfig)

  const [capabilities, setCapabilities] = useState<Capability[]>([])
  const [checks, setChecks] = useState<string[]>([])
  const [loading, setLoading] = useState(true)
  const [savingSource, setSavingSource] = useState<string | null>(null)
  const [sourceError, setSourceError] = useState<string | null>(null)
  const [verificationLoading, setVerificationLoading] = useState(false)
  const [verificationStatus, setVerificationStatus] = useState<string | null>(null)
  const mountedRef = useRef(true)
  const supportLabel = (level?: SupportLevel) => {
    switch (level) {
      case 'experimental':
        return t('setup.support.experimental')
      case 'candidate':
        return t('setup.support.candidate')
      case 'monitored':
      default:
        return t('setup.support.monitored')
    }
  }

  useEffect(() => {
    mountedRef.current = true
    Promise.all([
      apiFetch('/api/installer/detect')
        .then((r) => r.json())
        .then((d) => {
          if (mountedRef.current) {
            setCapabilities(d.capabilities ?? [])
          }
        })
        .catch((err) => { console.warn('[OctoMonitor] setup.capabilities', err) }),
      apiFetch('/api/installer/doctor')
        .then((r) => r.json())
        .then((d) => {
          if (mountedRef.current) {
            setChecks(d.checks ?? [])
          }
        })
        .catch((err) => { console.warn('[OctoMonitor] setup.doctor', err) }),
    ]).finally(() => {
      if (mountedRef.current) {
        setLoading(false)
      }
    })
    return () => {
      mountedRef.current = false
    }
  }, [])

  const sourceRows = useMemo(() => {
    const runs = data?.runs ?? []
    const disabled = new Set(data?.config.disabledSources ?? [])
    const hidden = new Set(data?.config.hiddenSources ?? [])
    return allTools.map((tool) => {
      const latest = latestRunForTool(runs, tool)
      const health = data?.adapterHealth.find((item) => item.tool === tool)
      const source = firstDataSource(latest)
      const errors = latest?.dataSources?.reduce((sum, item) => sum + item.errors.length, 0) ?? 0
      const caps = latest?.capabilities?.map((cap) => cap.id) ?? []
      const supportLevel = latest?.toolSpecific?.supportLevel
      const privacyNotes = [
        tool === 'cursor' ? t('setup.privacy.cursor') : null,
        disabled.has(tool) ? t('setup.privacy.disabled') : null,
        errors > 0 ? t('setup.privacy.parseErrors') : null,
        supportLevel === 'detection-only' || supportLevel === 'watchlist-only'
          ? t('setup.privacy.metadataOnly')
          : null,
      ].filter((item): item is string => Boolean(item))
      return {
        tool,
        latest,
        health,
        source,
        scanEnabled: !disabled.has(tool),
        visible: !hidden.has(tool),
        version: latest?.lastTail ?? 'N/A',
        root: source?.path ?? (typeof latest?.toolSpecific?.root === 'string' ? latest.toolSpecific.root : latest?.workspaceShort) ?? 'N/A',
        format: source?.sourceType ?? source?.schemaVersion ?? latest?.sourceMode ?? 'N/A',
        lastSeen: source?.lastSeenAt ?? health?.lastSuccessAt ?? latest?.lastActivityAt ?? null,
        scan: health ? `${health.online ? t('setup.scan.online') : t('setup.scan.missing')} · ${health.mode}` : 'N/A',
        schema: source?.schemaConfidence ?? 'N/A',
        parseErrors: errors,
        hook: health?.mode.includes('hook') || health?.mode.includes('statusline') ? t('setup.hook.ready') : t('setup.hook.none'),
        operations: caps.length > 0 ? caps.join(', ') : 'N/A',
        privacy: privacyNotes.length > 0 ? privacyNotes.join(', ') : t('setup.privacy.none'),
      }
    })
  }, [data, t])

  async function patchSource(tool: ToolKind, patch: { scanEnabled?: boolean; visible?: boolean }) {
    if (!data || savingSource != null) return
    setSavingSource(`${tool}:${patch.scanEnabled == null ? 'visible' : 'scan'}`)
    setSourceError(null)
    try {
      const disabledSources = patch.scanEnabled == null
        ? data.config.disabledSources
        : setToolPresence(data.config.disabledSources, tool, !patch.scanEnabled)
      const hiddenSources = patch.visible == null
        ? data.config.hiddenSources
        : setToolPresence(data.config.hiddenSources, tool, !patch.visible)
      const response = await apiFetch('/api/config', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ disabledSources, hiddenSources }),
      })
      if (!response.ok) throw new Error('source control patch failed')
      setConfig(await response.json())
    } catch (err) {
      console.warn('[OctoMonitor] source.controls', err)
      setSourceError(t('setup.sourceControlsError'))
    } finally {
      setSavingSource(null)
    }
  }

  async function runVerification() {
    setVerificationLoading(true)
    setVerificationStatus(null)
    try {
      const response = await apiFetch('/api/installer/verify')
      if (!response.ok) throw new Error('verification failed')
      const payload = await response.json()
      setChecks(payload.checks ?? [])
      setVerificationStatus(t('setup.verificationPassed'))
    } catch (err) {
      console.warn('[OctoMonitor] setup.verify', err)
      setVerificationStatus(t('setup.verificationFailed'))
    } finally {
      setVerificationLoading(false)
    }
  }

  return (
    <section className="settings-section">
      <div className="section-label">{t('setup.title')}</div>
      <p className="settings-hint">{t('setup.doctorHint')}</p>
      <div className="setup-actions">
        <button
          className="settings-option"
          disabled={verificationLoading}
          onClick={() => void runVerification()}
        >
          {verificationLoading ? t('setup.verifying') : t('setup.runVerification')}
        </button>
        {verificationStatus && <span className="settings-status-text">{verificationStatus}</span>}
      </div>
      {loading && <div className="setup-loading">{t('setup.detecting')}</div>}
      <div className="setup-cards">
        {capabilities.map((cap) => (
          <div key={cap.tool} className="setup-card">
            <div className="setup-card-header">
              <strong>{capabilityLabel(cap.tool)}</strong>
              <span className="setup-status-group">
                <span className={`setup-support ${cap.supportLevel ?? 'monitored'}`}>
                  {supportLabel(cap.supportLevel)}
                </span>
                <span className={`setup-status ${cap.detected ? 'detected' : 'missing'}`}>
                  {cap.detected ? t('setup.detected') : t('setup.missing')}
                </span>
              </span>
            </div>
            <div className="setup-card-meta">
              <span>{cap.mode}</span>
              {cap.notes && <span className="setup-notes">{cap.notes}</span>}
            </div>
          </div>
        ))}
      </div>

      <div className="source-control-panel">
        <div className="source-control-header">
          <h3>{t('setup.sourceControls')}</h3>
          {sourceError && <span className="settings-status-text error">{sourceError}</span>}
        </div>
        <div className="source-control-list">
          {sourceRows.map((row) => (
            <div
              key={row.tool}
              className={`source-control-row ${row.scanEnabled ? '' : 'disabled'} ${row.visible ? '' : 'hidden'}`}
            >
              <div className="source-control-main">
                <strong>{sourceLabels[row.tool]}</strong>
                <span>{row.scan}</span>
              </div>
              <div className="source-control-toggles">
                <button
                  className={`settings-option small ${row.scanEnabled ? 'active' : ''}`}
                  disabled={savingSource != null}
                  onClick={() => void patchSource(row.tool, { scanEnabled: !row.scanEnabled })}
                >
                  {row.scanEnabled ? t('setup.scanEnabled') : t('setup.scanDisabled')}
                </button>
                <button
                  className={`settings-option small ${row.visible ? 'active' : ''}`}
                  disabled={savingSource != null}
                  onClick={() => void patchSource(row.tool, { visible: !row.visible })}
                >
                  {row.visible ? t('setup.visible') : t('setup.hidden')}
                </button>
              </div>
              <div className="source-control-grid">
                <span><b>{t('setup.version')}</b>{row.version}</span>
                <span><b>{t('setup.root')}</b>{row.root}</span>
                <span><b>{t('setup.format')}</b>{row.format}</span>
                <span><b>{t('setup.lastSeen')}</b>{formatDate(row.lastSeen)}</span>
                <span><b>{t('setup.schema')}</b>{row.schema}</span>
                <span><b>{t('setup.parseErrors')}</b>{row.parseErrors}</span>
                <span><b>{t('setup.hookStatus')}</b>{row.hook}</span>
                <span><b>{t('setup.operations')}</b>{row.operations}</span>
                <span className="source-control-privacy"><b>{t('setup.privacy')}</b>{row.privacy}</span>
              </div>
            </div>
          ))}
        </div>
      </div>

      {checks.length > 0 && (
        <div className="setup-checks">
          <h3>{t('setup.doctorChecks')}</h3>
          <div className="check-list">
            {checks.map((item, i) => (
              <div key={i} className="check-item">{item}</div>
            ))}
          </div>
        </div>
      )}
    </section>
  )
}
