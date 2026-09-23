import type { ToastType } from '../hooks/useToast'

/** `app-toast` CustomEvent のペイロード。アプリ全体の通知で共通。 */
export interface AppToastDetail {
  message: string
  type?: ToastType
  /** 開発者向けの詳細（エラーメッセージ等）。画面には出さずログ用途。 */
  error?: string
}

/**
 * アプリ全体共通のトースト通知を発火する。
 *
 * App ルートの `useAppToastListener` が購読し `ToastOverlay` で表示されるため、
 * サービス層・ビュー層など React コンポーネントの内外を問わずどこからでも使える。
 * ネイティブ `alert()` の代替として、ユーザーへの一時的な通知はこれを使うこと。
 */
export function notifyAppToast(message: string, type: ToastType = 'info', error?: string): void {
  if (typeof window === 'undefined') return
  const detail: AppToastDetail = { message, type }
  if (error !== undefined) detail.error = error
  window.dispatchEvent(new CustomEvent<AppToastDetail>('app-toast', { detail }))
}

export const notifyInfo = (message: string): void => notifyAppToast(message, 'info')
export const notifySuccess = (message: string): void => notifyAppToast(message, 'success')
export const notifyWarning = (message: string, error?: string): void => notifyAppToast(message, 'warning', error)
export const notifyError = (message: string, error?: string): void => notifyAppToast(message, 'error', error)
