import { useEffect } from 'react'
import type { ToastType } from './useToast'
import type { AppToastDetail } from '../utils/notify'

export type { AppToastDetail }

/**
 * サービス層（pdfRenderer / annotationService など）やビュー層が発火する
 * `app-toast` CustomEvent を購読し、`showToast` 経由で画面上のトースト表示へ
 * ブリッジするフック。コンポーネントから直接呼べない層の通知を UI へ届ける窓口。
 */
export function useAppToastListener(showToast: (msg: string, type?: ToastType) => void): void {
  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<AppToastDetail>).detail
      if (!detail || typeof detail.message !== 'string' || detail.message.length === 0) return
      showToast(detail.message, detail.type ?? 'info')
    }
    window.addEventListener('app-toast', handler as EventListener)
    return () => window.removeEventListener('app-toast', handler as EventListener)
  }, [showToast])
}
