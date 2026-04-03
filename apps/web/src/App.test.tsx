import { render, screen } from '@testing-library/react'
import App from './App'
import { I18nProvider } from './lib/i18n'
import { ThemeProvider } from './lib/theme'

describe('App', () => {
  it('renders monitor view by default', async () => {
    render(<I18nProvider><ThemeProvider><App /></ThemeProvider></I18nProvider>)
    expect(await screen.findByText('MONITOR')).toBeInTheDocument()
    expect(await screen.findByText('USAGE')).toBeInTheDocument()
    expect(await screen.findByText('COMMITS')).toBeInTheDocument()
    expect(await screen.findByText('HEATMAP')).toBeInTheDocument()
  })
})
