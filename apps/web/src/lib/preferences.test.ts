import {
  defaultSettings,
  loadFrontendSettings,
  saveFrontendSettings,
} from './preferences'
import { STORAGE_KEYS } from './storageKeys'

describe('frontend preferences', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('loads defaults when storage is empty', () => {
    expect(loadFrontendSettings()).toEqual(defaultSettings)
  })

  it('migrates legacy settings while preserving supported fields', () => {
    localStorage.setItem(STORAGE_KEYS.settings, JSON.stringify({
      monitorPeriod: '4h',
      usageWindow: 'month',
      uiDensity: 'compact',
      companionEnabled: true,
      panelConfig: [{ tool: 'codex', enabled: true }],
      filterRules: {
        codex: { mode: 'include', patterns: ['octo'] },
      },
      showFingerprints: false,
    }))

    const settings = loadFrontendSettings()

    expect(settings.monitorPeriod).toBe('4h')
    expect(settings.snapshotWindow).toBe('month')
    expect(settings.uiDensity).toBe('compact')
    expect(settings.showFingerprints).toBe(false)
    expect(settings.notificationsEnabled).toBe(false)
    expect(settings.panelConfig.map((entry) => entry.tool)).toEqual(['codex', 'claude', 'openClaw', 'hermes'])
    expect(settings.filterRules.codex.patterns).toEqual(['octo'])
    expect(settings.filterRules.claude.mode).toBe('off')
  })

  it('persists only the supported preference model', () => {
    saveFrontendSettings({
      ...defaultSettings,
      notificationsEnabled: true,
      uiDensity: 'spacious',
    })

    expect(JSON.parse(localStorage.getItem(STORAGE_KEYS.settings) ?? '{}')).toEqual({
      version: 3,
      ...defaultSettings,
      notificationsEnabled: true,
      uiDensity: 'spacious',
    })
  })
})
