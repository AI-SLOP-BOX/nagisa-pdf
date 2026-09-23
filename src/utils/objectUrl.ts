/**
 * Blob URL を安全に解放する。
 *
 * `URL.revokeObjectURL()` は通常は例外を投げないが、null / undefined を渡した場合や
 * 非対応環境では例外になり得る。リソース解放の失敗が呼び出し元の処理全体を
 * 壊さないよう、ここで防御的に処理する。
 *
 * 呼び出し元ごとに `try { ... } catch {}` を書くと「エラーを握りつぶす空 catch」が
 * 散在してしまうため、必ずこのヘルパーを経由すること。
 */
export function safeRevokeObjectUrl(url: string | null | undefined): void {
  if (typeof url !== 'string' || url.length === 0) return
  try {
    URL.revokeObjectURL(url)
  } catch (err) {
    console.warn('[objectUrl] blob URL の解放に失敗:', err)
  }
}
