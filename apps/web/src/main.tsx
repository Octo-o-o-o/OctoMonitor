import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { ErrorBoundary } from './components/ErrorBoundary'
import { bootstrapDesktopWebview } from './lib/desktopZoom'
import { I18nProvider } from './lib/i18n'
import { getRuntimeMode } from './lib/runtimeMode'
import { STORAGE_KEYS } from './lib/storageKeys'
import { ThemeProvider } from './lib/theme'
import IslandApp from './island/IslandApp'
import './styles.css'
import './island/island.css'

// Set color-scheme early to avoid flash of wrong native chrome
{
  const theme = localStorage.getItem(STORAGE_KEYS.theme)
  const isLight = theme === 'light' || theme === 'eink' ||
    (!theme && (/Kindle|Silk|BOOX|reMarkable|NOOK|Kobo/i.test(navigator.userAgent) || window.matchMedia?.('(monochrome)')?.matches))
  document.documentElement.style.colorScheme = isLight ? 'light' : 'dark'
}

void bootstrapDesktopWebview()

const surface = new URLSearchParams(window.location.search).get('surface')
const isIslandSurface = surface === 'island' && getRuntimeMode() === 'local'
if (isIslandSurface) {
  document.documentElement.dataset.surface = 'island'
}
const RootApp = isIslandSurface ? IslandApp : App

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <I18nProvider>
        <ThemeProvider>
          <RootApp />
        </ThemeProvider>
      </I18nProvider>
    </ErrorBoundary>
  </React.StrictMode>
)
