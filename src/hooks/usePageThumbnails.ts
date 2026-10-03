import { useState, useEffect, useRef, useCallback } from 'react'
import { safeRevokeObjectUrl } from '../utils/objectUrl'
import {
  renderPageThumbnail,
  detectTotalPages,
  type ThumbnailSource,
} from '../services/thumbnailService'

export interface UsePageThumbnailsOptions extends ThumbnailSource {
  pageCount: number
  /** サムネイル解像度（dpi）。既定40。 */
  dpi?: number
}

/**
 * ページサムネイルの共有フック（EditorThumbnailSidebar / ThumbnailsPanel 共用）。
 *
 * - 遅延読込: `requestPage(i)` で要求されたページのみバックグラウンド描画する。
 *   上限40件の打切りはしない（大文書の41p以降も到達時に描画される）。
 * - URL寿命: blob URL のみ追跡し、置換・doc切替・アンマウント時に revoke。
 *   dataURL は解放不要。state確定後の prune のためDOMが無効URLを指さない。
 * - 世代ガード: doc切替で旧世代の描画結果を破棄する。
 * - updater内に副作用を持たない（StrictMode safe）。重複抑止はref集合で行う。
 *
 * 呼出契約: pdfData/docId は安定参照で渡すこと。レンダー毎に新規配列を渡すと
 * doc-change effect が毎回発火し、サムネイルが全破棄→再要求を繰り返す。
 * （PDFEditorView/PDFViewer は state 由来の安定参照を渡している）
 */
export function usePageThumbnails({ pdfData, docId, pageCount, dpi = 40 }: UsePageThumbnailsOptions) {
  const [thumbnails, setThumbnails] = useState<Map<number, string>>(new Map())
  const [detectedPageCount, setDetectedPageCount] = useState(0)
  const blobUrlsRef = useRef<Set<string>>(new Set())
  const mapRef = useRef<Map<number, string>>(new Map())
  const pendingRef = useRef<Set<number>>(new Set())
  // 失敗記録（時刻付き）。一定時間後は再要求を許可する（永久欠け防止）。
  const failedRef = useRef<Map<number, number>>(new Map())
  const generationRef = useRef(0)
  const FAILED_RETRY_MS = 30 * 1000

  // 置換・除去されたURLをprune（state確定後なのでDOMは無効URLを指さない）
  useEffect(() => {
    mapRef.current = thumbnails
    const activeUrls = new Set(thumbnails.values())
    for (const url of Array.from(blobUrlsRef.current)) {
      if (!activeUrls.has(url)) {
        safeRevokeObjectUrl(url)
        blobUrlsRef.current.delete(url)
      }
    }
  }, [thumbnails])

  // doc切替: 全破棄＋世代更新（旧世代の描画結果は捨てる）
  useEffect(() => {
    generationRef.current += 1
    pendingRef.current.clear()
    failedRef.current.clear()
    mapRef.current = new Map()
    setThumbnails(new Map())
    setDetectedPageCount(0)
    // revoke自体はprune effectが担う（active集合が空になるため）
  }, [pdfData, docId, pageCount])

  // アンマウント時の一括revoke
  useEffect(() => {
    return () => {
      for (const url of blobUrlsRef.current) {
        safeRevokeObjectUrl(url)
      }
      blobUrlsRef.current.clear()
    }
  }, [])

  // 総ページ数検出（PDF.jsが読める場合のみ上振れ検出）
  useEffect(() => {
    const gen = generationRef.current
    let cancelled = false
    detectTotalPages({ pdfData, docId }).then(total => {
      if (!cancelled && total !== null && gen === generationRef.current) {
        setDetectedPageCount(total)
      }
    })
    return () => {
      cancelled = true
    }
  }, [pdfData, docId])

  const requestPage = useCallback(
    (pageIndex: number) => {
      const failedAt = failedRef.current.get(pageIndex)
      if (failedAt !== undefined && Date.now() - failedAt < FAILED_RETRY_MS) return
      if (
        pageIndex < 0 ||
        mapRef.current.has(pageIndex) ||
        pendingRef.current.has(pageIndex)
      ) {
        return
      }
      const gen = generationRef.current
      pendingRef.current.add(pageIndex)
      void (async () => {
        try {
          const rendered = await renderPageThumbnail({ pdfData, docId }, pageIndex, dpi)
          if (gen !== generationRef.current) return
          if (rendered) {
            if (rendered.isBlob) blobUrlsRef.current.add(rendered.url)
            mapRef.current = new Map(mapRef.current).set(pageIndex, rendered.url)
            setThumbnails(mapRef.current)
          } else {
            failedRef.current.set(pageIndex, Date.now())
          }
        } catch {
          if (gen === generationRef.current) failedRef.current.set(pageIndex, Date.now())
        } finally {
          pendingRef.current.delete(pageIndex)
        }
      })()
    },
    [pdfData, docId, dpi],
  )

  return { thumbnails, detectedPageCount, requestPage }
}
