import { createContext, useCallback, useContext, useEffect, useState, type PropsWithChildren } from 'react'
import { STORAGE_KEYS } from './storageKeys'

export type ThemeId = 'dark' | 'light' | 'eink' | 'kindle-light' | 'kindle-dark' | string

/** Ordered list of built-in themes for cycling */
export const builtinThemeOrder: ThemeId[] = ['dark', 'light', 'eink', 'kindle-light', 'kindle-dark']
export const builtinThemeIcons: Record<string, string> = {
  dark: '\u263E',   // moon
  light: '\u2600',  // sun
  eink: '\u25A3',   // filled square (e-ink symbol)
  'kindle-light': '\u25A1', // white square (kindle light)
  'kindle-dark': '\u25A0',  // black square (kindle dark)
}
export const builtinThemeLabels: Record<string, string> = {
  dark: 'Dark',
  light: 'Light',
  eink: 'E-Ink',
  'kindle-light': 'Kindle Light',
  'kindle-dark': 'Kindle Dark',
}

interface ThemeColors {
  bg: string
  surface: string
  surface2: string
  border: string
  text: string
  muted: string
  accent: string
  warn: string
  danger: string
  barBg: string
}

const builtinThemes: Record<string, ThemeColors> = {
  dark: {
    bg: '#08101f',
    surface: '#131d34',
    surface2: '#17233f',
    border: '#263554',
    text: '#eff5ff',
    muted: '#9fb0cf',
    accent: '#40d39d',
    warn: '#f2bb4a',
    danger: '#ef7b8f',
    barBg: '#0d1528',
  },
  light: {
    bg: '#fafafa',       // zinc-50
    surface: '#ffffff',  // white
    surface2: '#f4f4f5', // zinc-100
    border: '#e4e4e7',   // zinc-200
    text: '#18181b',     // zinc-900
    muted: '#71717a',    // zinc-500
    accent: '#059669',   // emerald-600
    warn: '#d97706',     // amber-600
    danger: '#dc2626',   // red-600
    barBg: '#e4e4e7',    // zinc-200
  },
  eink: {
    bg: '#ffffff',
    surface: '#ffffff',
    surface2: '#f0f0f0',
    border: '#000000',
    text: '#000000',
    muted: '#444444',
    accent: '#000000',
    warn: '#000000',
    danger: '#000000',
    barBg: '#cccccc',
  },
  'kindle-light': {
    bg: '#fdfcf8',       // 米白纸张质感
    surface: '#ffffff',   // 纯白卡片
    surface2: '#f3f0e8',  // 柔和米色背景
    border: '#d6d3cb',   // rgba(0,0,0,0.15) on #fdfcf8
    text: '#1c1c1c',     // 深黑文本
    muted: '#737373',    // 中灰色次要元素
    accent: '#1c1c1c',   // 主色 = 深黑（无彩色）
    warn: '#737373',     // 灰阶警告
    danger: '#1c1c1c',   // 灰阶危险
    barBg: '#e5e5e5',    // 浅灰进度条轨道
  },
  'kindle-dark': {
    bg: '#121212',       // 纯黑底
    surface: '#1c1c1c',  // 深灰卡片
    surface2: '#2a2a2a', // 深灰背景
    border: '#363636',   // rgba(255,255,255,0.15) on #121212
    text: '#e8e6e3',     // 柔和米色文本
    muted: '#8c8c8c',    // 浅灰色次要元素
    accent: '#e8e6e3',   // 主色 = 米色（无彩色）
    warn: '#8c8c8c',     // 灰阶警告
    danger: '#e8e6e3',   // 灰阶危险
    barBg: '#2a2a2a',    // 深灰进度条轨道
  },
}

interface CustomTheme {
  id: string
  name: string
  colors: ThemeColors
}

interface ThemeContextValue {
  themeId: ThemeId
  setTheme: (id: ThemeId) => void
  customThemes: CustomTheme[]
  importVSCodeTheme: (json: string) => string | null
  removeCustomTheme: (id: string) => void
}

const ThemeContext = createContext<ThemeContextValue>({
  themeId: 'dark',
  setTheme: () => {},
  customThemes: [],
  importVSCodeTheme: () => null,
  removeCustomTheme: () => {},
})

const cssVarNames: Record<keyof ThemeColors, string> = {
  bg: '--bg',
  surface: '--surface',
  surface2: '--surface2',
  border: '--border',
  text: '--text',
  muted: '--muted',
  accent: '--accent',
  warn: '--warn',
  danger: '--danger',
  barBg: '--bar-bg',
}

function applyThemeColors(colors: ThemeColors) {
  const root = document.documentElement
  for (const [key, cssVar] of Object.entries(cssVarNames)) {
    root.style.setProperty(cssVar, colors[key as keyof ThemeColors])
  }

  const lum = hexLuminance(colors.bg)
  root.style.colorScheme = lum > 0.5 ? 'light' : 'dark'
}

function applyThemeId(id: ThemeId) {
  document.documentElement.setAttribute('data-theme', id)
}

function hexLuminance(hex: string): number {
  const clean = hex.replace('#', '')
  if (clean.length < 6) return 0
  const r = parseInt(clean.slice(0, 2), 16) / 255
  const g = parseInt(clean.slice(2, 4), 16) / 255
  const b = parseInt(clean.slice(4, 6), 16) / 255
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

function parseVSCodeTheme(json: string): { name: string; colors: ThemeColors } | null {
  try {
    const theme = JSON.parse(json)
    const c = theme.colors ?? {}
    const name: string = theme.name ?? 'Imported Theme'
    const bg = c['editor.background'] ?? c['sideBar.background'] ?? '#1e1e1e'
    const surface = c['sideBar.background'] ?? c['editorGroupHeader.tabsBackground'] ?? bg
    const surface2 = c['editorGroupHeader.tabsBackground'] ?? c['tab.inactiveBackground'] ?? surface
    const border = c['panel.border'] ?? c['sideBar.border'] ?? c['editorGroup.border'] ?? '#333'
    const text = c['editor.foreground'] ?? c['foreground'] ?? '#d4d4d4'
    const muted = c['descriptionForeground'] ?? c['tab.inactiveForeground'] ?? '#888'
    const accent = c['focusBorder'] ?? c['button.background'] ?? c['textLink.foreground'] ?? '#007acc'
    const warn = c['editorWarning.foreground'] ?? c['list.warningForeground'] ?? '#ffc96b'
    const danger = c['editorError.foreground'] ?? c['list.errorForeground'] ?? '#ff6b7a'
    const barBg = c['input.background'] ?? c['dropdown.background'] ?? surface2
    return {
      name,
      colors: { bg, surface, surface2, border, text, muted, accent, warn, danger, barBg },
    }
  } catch {
    return null
  }
}

function loadCustomThemes(): CustomTheme[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.customThemes)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

function saveCustomThemes(themes: CustomTheme[]) {
  try {
    localStorage.setItem(STORAGE_KEYS.customThemes, JSON.stringify(themes))
  } catch (err) {
    console.warn('[OctoMonitor] storage.write.customThemes', err)
  }
}

function detectDefaultTheme(): ThemeId {
  const saved = localStorage.getItem(STORAGE_KEYS.theme)
  if (saved) return saved
  // Auto-detect E-Ink devices: Kindle, Silk browser, BOOX, reMarkable, NOOK, Kobo
  const ua = navigator.userAgent
  if (/Kindle|Silk|KFAPWI|KFONWI|BOOX|reMarkable|NOOK|Kobo/i.test(ua)) return 'eink'
  // CSS media query: monochrome displays
  if (window.matchMedia?.('(monochrome)')?.matches) return 'eink'
  return 'dark'
}

export function ThemeProvider({ children }: PropsWithChildren) {
  const [themeId, setThemeId] = useState<ThemeId>(detectDefaultTheme)
  const [customThemes, setCustomThemes] = useState<CustomTheme[]>(loadCustomThemes)

  const applyCurrentTheme = useCallback(
    (id: ThemeId, customs: CustomTheme[]) => {
      applyThemeId(id)
      const colors = builtinThemes[id]
        ?? customs.find((t) => t.id === id)?.colors
        ?? builtinThemes.dark
      applyThemeColors(colors)
    },
    [],
  )

  useEffect(() => {
    applyCurrentTheme(themeId, customThemes)
  }, [themeId, customThemes, applyCurrentTheme])

  const setTheme = useCallback((id: ThemeId) => {
    try {
      localStorage.setItem(STORAGE_KEYS.theme, id)
    } catch (err) {
      console.warn('[OctoMonitor] storage.write.theme', err)
    }
    setThemeId(id)
  }, [])

  const importVSCodeTheme = useCallback(
    (json: string): string | null => {
      const result = parseVSCodeTheme(json)
      if (!result) return null
      const id = `vscode-${Date.now()}`
      const newTheme: CustomTheme = { id, name: result.name, colors: result.colors }
      const updated = [...customThemes.filter((t) => t.name !== result.name), newTheme]
      setCustomThemes(updated)
      saveCustomThemes(updated)
      setTheme(id)
      return id
    },
    [customThemes, setTheme],
  )

  const removeCustomTheme = useCallback(
    (id: string) => {
      const updated = customThemes.filter((t) => t.id !== id)
      setCustomThemes(updated)
      saveCustomThemes(updated)
      if (themeId === id) setTheme('dark')
    },
    [customThemes, themeId, setTheme],
  )

  return (
    <ThemeContext.Provider value={{ themeId, setTheme, customThemes, importVSCodeTheme, removeCustomTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  return useContext(ThemeContext)
}
