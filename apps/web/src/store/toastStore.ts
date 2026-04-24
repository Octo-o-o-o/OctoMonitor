import { create } from 'zustand'

export type ToastKind = 'info' | 'error'

export interface ToastEntry {
  id: number
  kind: ToastKind
  message: string
  durationMs: number
}

interface ToastState {
  toasts: ToastEntry[]
  pushToast: (input: { kind?: ToastKind; message: string; durationMs?: number }) => number
  dismissToast: (id: number) => void
}

let nextId = 1

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  pushToast: ({ kind = 'info', message, durationMs = 2400 }) => {
    const id = nextId++
    set((s) => ({ toasts: [...s.toasts, { id, kind, message, durationMs }] }))
    return id
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}))
