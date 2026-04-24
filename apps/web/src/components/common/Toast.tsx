import { useEffect } from 'react'
import { useToastStore, type ToastEntry } from '../../store/toastStore'

function ToastItem({ toast }: { toast: ToastEntry }) {
  const dismiss = useToastStore((s) => s.dismissToast)

  useEffect(() => {
    const id = setTimeout(() => dismiss(toast.id), toast.durationMs)
    return () => clearTimeout(id)
  }, [dismiss, toast.id, toast.durationMs])

  return (
    <div className={`toast toast--${toast.kind}`} role="status" aria-live="polite">
      {toast.message}
    </div>
  )
}

export function ToastContainer() {
  const toasts = useToastStore((s) => s.toasts)
  if (toasts.length === 0) return null
  return (
    <div className="toast-container" aria-live="polite">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} />
      ))}
    </div>
  )
}
