import { useState, useCallback, useEffect, useRef } from 'react'

export type ToastType = 'info' | 'error' | 'success' | 'warning'

export function useToast(duration = 2800) {
  const [toast, setToast] = useState<string | null>(null)
  const [toastType, setToastType] = useState<ToastType>('info')
  const timerRef = useRef<number | null>(null)

  const showToast = useCallback((msg: string, type: ToastType = 'info') => {
    setToast(msg)
    setToastType(type)
    // Replace any pending timer so a newer toast is not cleared by an older one.
    if (timerRef.current !== null) window.clearTimeout(timerRef.current)
    timerRef.current = window.setTimeout(() => {
      setToast(null)
      timerRef.current = null
    }, duration)
  }, [duration])

  // Clear the pending timer on unmount to avoid setState after unmount.
  useEffect(() => {
    return () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current)
    }
  }, [])

  const showError = useCallback((msg: string) => {
    showToast(msg, 'error')
  }, [showToast])

  const showSuccess = useCallback((msg: string) => {
    showToast(msg, 'success')
  }, [showToast])

  return {
    toast,
    toastType,
    showToast,
    showError,
    showSuccess,
  }
}
