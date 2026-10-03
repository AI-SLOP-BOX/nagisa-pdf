import { useState, useCallback, useRef } from 'react'

// フロント側 Undo は number[] 全スナップショット保持のため巨大PDFでメモリ爆発する。
// バックエンド SessionManager (512MB cap) に倣い、フロントもバイト数上限で古い履歴を破棄する。
const MAX_HISTORY_BYTES = 200 * 1024 * 1024

export function useHistory(initialData: number[] | null = null, maxHistory = 30, maxBytes = MAX_HISTORY_BYTES) {
  const [data, setData] = useState<number[] | null>(initialData)
  const [history, setHistory] = useState<number[][]>(initialData ? [initialData] : [])
  const [historyIndex, setHistoryIndex] = useState(initialData ? 0 : -1)
  // updater外で同期計算するためのrefミラー。updater内副作用（StrictModeの
  // double-invokeで二重適用される）を排除し、setHistory/setHistoryIndexは
  // 計算済み値を直接セットする。
  const stateRef = useRef<{ history: number[][]; index: number }>({
    history: initialData ? [initialData] : [],
    index: initialData ? 0 : -1,
  })

  const pushHistory = useCallback((newData: number[]) => {
    const { history: prev, index } = stateRef.current
    let next = [...prev.slice(0, index + 1), newData]
    let nextIndex = index + 1
    if (next.length > maxHistory) {
      const drop = next.length - maxHistory
      next = next.slice(drop)
      nextIndex -= drop
    }
    // バイト数上限: 超過分は最古から破棄し、indexを前にずらす
    let total = next.reduce((s, d) => s + d.length, 0)
    while (next.length > 1 && total > maxBytes) {
      const evicted = next.shift()!
      total -= evicted.length
      nextIndex -= 1
    }
    nextIndex = Math.max(0, Math.min(nextIndex, next.length - 1))
    stateRef.current = { history: next, index: nextIndex }
    setData(newData)
    setHistory(next)
    setHistoryIndex(nextIndex)
  }, [maxHistory, maxBytes])

  const undo = useCallback(() => {
    const { history: h, index } = stateRef.current
    if (index > 0) {
      const nextIndex = index - 1
      stateRef.current = { history: h, index: nextIndex }
      setHistoryIndex(nextIndex)
      setData(h[nextIndex])
    }
  }, [])

  const redo = useCallback(() => {
    const { history: h, index } = stateRef.current
    if (index < h.length - 1) {
      const nextIndex = index + 1
      stateRef.current = { history: h, index: nextIndex }
      setHistoryIndex(nextIndex)
      setData(h[nextIndex])
    }
  }, [])

  const resetHistory = useCallback((newData: number[]) => {
    stateRef.current = { history: [newData], index: 0 }
    setData(newData)
    setHistory([newData])
    setHistoryIndex(0)
  }, [])

  return {
    data,
    // 注意: setData は表示バイトのみ更新し、履歴(stateRef/history/index)と連動しない。
    // undo対象の変更は pushHistory、文書切替は resetHistory と併用すること。
    // 直叩き後の fallback undo は古いスナップショットに戻る。
    setData,
    history,
    historyIndex,
    pushHistory,
    undo,
    redo,
    resetHistory,
    canUndo: historyIndex > 0,
    canRedo: historyIndex < history.length - 1,
  }
}
