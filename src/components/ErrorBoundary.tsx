import React, { Component, ErrorInfo, ReactNode } from 'react'

interface ErrorBoundaryProps {
  children: ReactNode
  /**
   * エラー状態をリセットするためのキー。
   * 値が変わると捕捉済みエラーがクリアされ、子ツリーの再レンダリングを試みる。
   * （例: 現在のビュー名を渡し、画面遷移で自動復帰させる）
   */
  resetKey?: string
  /** カスタムフォールバック。省略時は既定の回復UIを表示する。 */
  fallback?: (error: Error, reset: () => void) => ReactNode
  /** 追加のエラーハンドラ（リモートロギング等）。 */
  onError?: (error: Error, info: ErrorInfo) => void
}

interface ErrorBoundaryState {
  error: Error | null
}

/**
 * 子ツリーのレンダリング／ライフサイクル中の未捕捉エラーを捕捉し、
 * アプリ全体の白屏（クラッシュ）を防いで回復可能なフォールバックUIを表示する。
 *
 * 注意: イベントハンドラや非同期コールバック内のエラーは ErrorBoundary では
 * 捕捉できないため、それらは各所の try/catch とトースト通知で扱うこと。
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error('[ErrorBoundary] 子コンポーネントで未捕捉エラー:', error, info.componentStack)
    this.props.onError?.(error, info)
  }

  componentDidUpdate(prevProps: ErrorBoundaryProps): void {
    // resetKey が変わったらエラー状態をクリアして再試行する
    if (this.state.error && prevProps.resetKey !== this.props.resetKey) {
      this.setState({ error: null })
    }
  }

  private reset = (): void => {
    this.setState({ error: null })
  }

  render(): ReactNode {
    const { error } = this.state
    if (error) {
      if (this.props.fallback) return this.props.fallback(error, this.reset)
      return <ErrorFallback error={error} onReset={this.reset} />
    }
    return this.props.children
  }
}

/**
 * 既定の回復UI。
 * i18n Provider 自体がクラッシュした場合にも描画できるよう、
 * 翻訳には依存せず自己完結した日英併記テキストとする。
 */
function ErrorFallback({ error, onReset }: { error: Error; onReset: () => void }) {
  return (
    <div
      role="alert"
      style={{
        flex: 1,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 16,
        padding: 32,
        background: '#f8fafc',
        color: '#0f172a',
        textAlign: 'center',
        height: '100%',
      }}
    >
      <div
        style={{
          width: 56,
          height: 56,
          borderRadius: '50%',
          background: '#fee2e2',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: 28,
        }}
        aria-hidden="true"
      >
        ⚠️
      </div>
      <div>
        <h2 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>
          予期しないエラーが発生しました
        </h2>
        <p style={{ margin: '6px 0 0', fontSize: 13, color: '#64748b' }}>
          Something went wrong. Your document data has not been sent anywhere.
        </p>
      </div>
      <pre
        style={{
          margin: 0,
          maxWidth: 560,
          width: '100%',
          maxHeight: 160,
          overflow: 'auto',
          background: '#0f172a',
          color: '#fca5a5',
          fontSize: 12,
          lineHeight: 1.5,
          textAlign: 'left',
          padding: '12px 14px',
          borderRadius: 10,
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-word',
        }}
      >
        {error.message || String(error)}
      </pre>
      <div style={{ display: 'flex', gap: 12 }}>
        <button
          type="button"
          onClick={onReset}
          style={{
            padding: '10px 18px',
            borderRadius: 10,
            border: '1px solid #cbd5e1',
            background: '#ffffff',
            color: '#0f172a',
            fontSize: 13,
            fontWeight: 600,
            cursor: 'pointer',
          }}
        >
          再試行
        </button>
        <button
          type="button"
          onClick={() => window.location.reload()}
          style={{
            padding: '10px 18px',
            borderRadius: 10,
            border: 'none',
            background: '#2563eb',
            color: '#ffffff',
            fontSize: 13,
            fontWeight: 600,
            cursor: 'pointer',
          }}
        >
          アプリを再起動
        </button>
      </div>
    </div>
  )
}
