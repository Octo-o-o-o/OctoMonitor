import { Component, type ReactNode, type ErrorInfo } from 'react'
import { useI18n } from '../lib/i18n'

interface Props {
  children: ReactNode
}

interface State {
  error: Error | null
}

function ErrorFallback({ error, onRetry }: { error: Error; onRetry: () => void }) {
  const { t } = useI18n()
  return (
    <div className="error-boundary">
      <h2 className="error-boundary-title">{t('ui.crashed')}</h2>
      <pre className="error-boundary-message">{error.message}</pre>
      <button className="error-boundary-retry" onClick={onRetry}>
        {t('ui.retry')}
      </button>
    </div>
  )
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null }

  static getDerivedStateFromError(error: Error): State {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[OctoMonitor] UI crash:', error, info.componentStack)
  }

  render() {
    if (this.state.error) {
      return <ErrorFallback error={this.state.error} onRetry={() => this.setState({ error: null })} />
    }
    return this.props.children
  }
}
