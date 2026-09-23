import type { ToastType } from '../hooks/useToast'

/** トースト種別ごとのアクセントカラー。 */
const TOAST_COLORS: Record<ToastType, string> = {
  info: '#2563eb',
  success: '#16a34a',
  error: '#ef4444',
  warning: '#ea580c',
}

interface ToastOverlayProps {
  message: string | null
  type: ToastType
}

/**
 * 画面右上に固定表示されるトースト。
 *
 * サービス層から `app-toast` で発火された通知も含め、アプリ全体で一貫した
 * 見た目とアクセシビリティ（`role="status"` / `aria-live`）を提供する。
 */
export function ToastOverlay({ message, type }: ToastOverlayProps) {
  if (!message) return null

  const color = TOAST_COLORS[type] ?? TOAST_COLORS.info

  return (
    <div
      role="status"
      aria-live="polite"
      style={{
        position: 'fixed',
        top: 16,
        right: 16,
        zIndex: 2000,
        background: '#ffffff',
        border: `1px solid ${color}`,
        color,
        padding: '10px 20px',
        borderRadius: 10,
        boxShadow: '0 4px 20px rgba(0,0,0,0.12)',
        fontSize: 13,
        fontWeight: 600,
        maxWidth: 480,
        wordBreak: 'break-word',
        whiteSpace: 'pre-wrap',
        lineHeight: 1.4,
        pointerEvents: 'none',
      }}
    >
      {message}
    </div>
  )
}
