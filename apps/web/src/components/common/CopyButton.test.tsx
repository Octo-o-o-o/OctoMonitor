import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { CopyButton } from './CopyButton'
import { ToastContainer } from './Toast'
import { I18nProvider } from '../../lib/i18n'
import { useToastStore } from '../../store/toastStore'

function renderWithProvider(text: string) {
  return render(
    <I18nProvider>
      <CopyButton text={text} ariaLabel="Copy thing" />
      <ToastContainer />
    </I18nProvider>,
  )
}

describe('CopyButton', () => {
  beforeEach(() => {
    act(() => {
      useToastStore.setState({ toasts: [] })
    })
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('writes text to clipboard and shows success toast', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })

    renderWithProvider('hello-world')
    fireEvent.click(screen.getByRole('button', { name: 'Copy thing' }))

    await waitFor(() => expect(writeText).toHaveBeenCalledWith('hello-world'))
    await waitFor(() => expect(screen.getByText('Copied')).toBeInTheDocument())
  })

  it('shows error toast when clipboard writeText rejects', async () => {
    const writeText = vi.fn().mockRejectedValue(new Error('nope'))
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })

    renderWithProvider('x')
    fireEvent.click(screen.getByRole('button', { name: 'Copy thing' }))

    await waitFor(() => expect(screen.getByText('Copy failed')).toBeInTheDocument())
  })

  it('shows error toast when clipboard API is missing', async () => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: undefined,
    })

    renderWithProvider('x')
    fireEvent.click(screen.getByRole('button', { name: 'Copy thing' }))

    await waitFor(() => expect(screen.getByText('Copy failed')).toBeInTheDocument())
  })

  it('is disabled when text is empty', () => {
    renderWithProvider('')
    expect(screen.getByRole('button', { name: 'Copy thing' })).toBeDisabled()
  })
})
