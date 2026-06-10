import {
  defaultSettings,
  loadFrontendSettings,
  migratePanelConfig,
  saveFrontendSettings,
} from './preferences'
import { allTools } from './constants'
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
    expect(settings.desktopDisplayMode).toBe('both')
    expect(settings.islandPosition).toBe('auto')
    expect(settings.panelConfig.map((entry) => entry.tool)).toEqual([
      'codex',
      ...allTools.filter((tool) => tool !== 'codex'),
    ])
    expect(settings.filterRules.codex.patterns).toEqual(['octo'])
    expect(settings.filterRules.claude.mode).toBe('off')
  })

  it('deduplicates repeated tools in stored panelConfig', () => {
    const result = migratePanelConfig([
      { tool: 'codex', enabled: false },
      { tool: 'codex', enabled: true },
      { tool: 'claude', enabled: true },
    ])

    expect(result.map((entry) => entry.tool)).toEqual([
      'codex',
      'claude',
      ...allTools.filter((tool) => tool !== 'codex' && tool !== 'claude'),
    ])
    expect(result.find((entry) => entry.tool === 'codex')?.enabled).toBe(false)
  })

  it('persists only the supported preference model', () => {
    saveFrontendSettings({
      ...defaultSettings,
      notificationsEnabled: true,
      uiDensity: 'spacious',
    })

    expect(JSON.parse(localStorage.getItem(STORAGE_KEYS.settings) ?? '{}')).toEqual({
      version: 4,
      ...defaultSettings,
      notificationsEnabled: true,
      uiDensity: 'spacious',
    })
  })
})
