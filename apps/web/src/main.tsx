// Set color-scheme early to avoid flash of wrong native chrome
{
  const theme = localStorage.getItem('octomonitor-theme')
  const isLight = theme === 'light' || theme === 'eink' ||
    (!theme && (/Kindle|Silk|BOOX|reMarkable|NOOK|Kobo/i.test(navigator.userAgent) || window.matchMedia?.('(monochrome)')?.matches))
  document.documentElement.style.colorScheme = isLight ? 'light' : 'dark'
}

import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { ErrorBoundary } from './components/ErrorBoundary'
import { bootstrapDesktopWebview } from './lib/desktopZoom'
import { I18nProvider } from './lib/i18n'
import { ThemeProvider } from './lib/theme'
import './styles.css'

void bootstrapDesktopWebview()

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <I18nProvider>
        <ThemeProvider>
          <App />
        </ThemeProvider>
      </I18nProvider>
    </ErrorBoundary>
  </React.StrictMode>
)
