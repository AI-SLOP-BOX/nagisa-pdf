import { invoke } from '@tauri-apps/api/core'
import { safeRevokeObjectUrl } from '../utils/objectUrl'
import { notifyWarning } from '../utils/notify'

import * as pdfjsLib from 'pdfjs-dist'
import pdfWorker from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

if (typeof window !== 'undefined') {
  pdfjsLib.GlobalWorkerOptions.workerSrc = pdfWorker
}

export class PDFJsEngine {
  // LRUキャッシュ：最大4文書まで保持。キーはFNV-1aハッシュ＋バイト長の複合。
  private static docCache = new Map<string, { doc: pdfjsLib.PDFDocumentProxy; refTime: number }>()
  private static readonly MAX_CACHE_SIZE = 4

  /** byte配列からFNV-1a 32bitハッシュを計算し、キャッシュキーとして利用する。 */
  private static computeHashKey(data: Uint8Array): string {
    const len = data.length
    if (len === 0) return 'empty_0'
    let hash = 0x811c9dc5
    for (let i = 0; i < len; i++) {
      hash ^= data[i]
      hash = Math.imul(hash, 0x01000193)
    }
    return `${len}_${(hash >>> 0).toString(16)}`
  }

  static async getDocument(data: number[] | Uint8Array): Promise<pdfjsLib.PDFDocumentProxy> {
    const uint8 = data instanceof Uint8Array ? data : new Uint8Array(data)
    const key = this.computeHashKey(uint8)

    if (this.docCache.has(key)) {
      const entry = this.docCache.get(key)!
      entry.refTime = Date.now()
      return entry.doc
    }

    const base = typeof document !== 'undefined' ? document.baseURI : '/'
    const loadingTask = pdfjsLib.getDocument({
      data: uint8,
      cMapUrl: new URL('cmaps/', base).href,
      cMapPacked: true,
      standardFontDataUrl: new URL('standard_fonts/', base).href,
    })
    const doc = await loadingTask.promise

    if (this.docCache.size >= this.MAX_CACHE_SIZE) {
      let oldestKey: string | null = null
      let oldestTime = Infinity
      for (const [k, v] of this.docCache.entries()) {
        if (v.refTime < oldestTime) {
          oldestTime = v.refTime
          oldestKey = k
        }
      }
      if (oldestKey) {
        const oldest = this.docCache.get(oldestKey)
        if (oldest) oldest.doc.destroy().catch((err) => console.debug('PDF文書(LRU追出)の破棄に失敗:', err))
        this.docCache.delete(oldestKey)
      }
    }

    this.docCache.set(key, { doc, refTime: Date.now() })
    return doc
  }

  /** 最新のレンダリングで使用されたPDFバイト列を返す。 */
  static getLatestBytes(): number[] | null {
    return getLatestBytes()
  }

  static clearCache(): void {
    for (const entry of this.docCache.values()) {
      entry.doc.destroy().catch((err) => console.debug('PDF文書(キャッシュクリア)の破棄に失敗:', err))
    }
    this.docCache.clear()
  }

  /** アプリ終了時に呼ぶ。キャッシュ内の全文書を破棄する。 */
  static destroy(): void {
    this.clearCache()
  }
}

export interface RenderRequest {
  pdfData?: number[]
  docId?: string
  pageIndex: number
  dpi: number
  signal?: AbortSignal
  /** レンダリング対象ページのサイズ（pt）。指定なしの場合はデフォルトA4（595×842）を使用。 */
  pageSize?: { width: number; height: number }
}

export interface SeparationRenderRequest extends RenderRequest {
  showC: boolean
  showM: boolean
  showY: boolean
  showK: boolean
  highlightTac: boolean
  tacLimit: number
}

export interface PDFRenderer {
  renderPageToUrl(req: RenderRequest): Promise<string>
  renderSeparationToUrl(req: SeparationRenderRequest): Promise<string>
  cancelAll(): void
}

/** DefaultRenderer が最後に使用した pdfData（投機的フォールバック用）。 */
let _latestPdfBytes: number[] | null = null

/** DefaultRenderer が最後に使用した pdfData を返す（PDFJsEngine.getLatestBytes 経由でもアクセス可）。 */
export function getLatestBytes(): number[] | null {
  return _latestPdfBytes
}

/** PDF.js 描画失敗のユーザー通知をスロットルするための、最後に通知した時刻。 */
let lastRenderFallbackNoticeAt = 0
const RENDER_FALLBACK_NOTICE_INTERVAL_MS = 5000

/**
 * PDF.js による描画が失敗し、代替（プレースホルダー）を表示したことをユーザーへ通知する。
 *
 * アプリ側は `app-toast` CustomEvent を購読してトースト表示に利用する。
 * ページごとに発火すると通知が氾濫するため、一定間隔でスロットルする。
 */
function notifyRenderFallback(pageNumber: number, err: unknown): void {
  const now = Date.now()
  if (now - lastRenderFallbackNoticeAt < RENDER_FALLBACK_NOTICE_INTERVAL_MS) return
  lastRenderFallbackNoticeAt = now
  const detail = err instanceof Error ? err.message : String(err)
  notifyWarning(`ページ ${pageNumber} の描画に失敗したため、プレビューを代替表示しています。`, detail)
}

/**
 * DefaultRenderer provides reliable rendering using the backend rendering pipeline
 * with session zero-IPC support, token-based stale-result suppression, and high-fidelity PDF.js client fallback.
 */
export class DefaultRenderer implements PDFRenderer {
  private activeTokens = new Set<number>()
  private tokenSeq = 0
  private createdUrls = new Set<string>()

  private trackUrl(url: string): string {
    if (url && url.startsWith('blob:')) {
      this.createdUrls.add(url)
      // If excessive cached URLs exist, cleanup oldest
      if (this.createdUrls.size > 50) {
        const oldest = this.createdUrls.values().next().value
        if (oldest) {
          safeRevokeObjectUrl(oldest)
          this.createdUrls.delete(oldest)
        }
      }
    }
    return url
  }

  async renderPageToUrl(req: RenderRequest): Promise<string> {
    const token = ++this.tokenSeq
    this.activeTokens.add(token)

    if (req.signal?.aborted) {
      this.activeTokens.delete(token)
      throw new Error('Render cancelled')
    }

    const abortHandler = () => {
      this.activeTokens.delete(token)
    }
    req.signal?.addEventListener('abort', abortHandler, { once: true })

    if (req.pdfData && req.pdfData.length > 0) {
      _latestPdfBytes = req.pdfData
    }

    // このレンダリングで作成された blob URL を追跡し、完了時に revoke する
    const localBlobUrls: string[] = []

    try {
      let pngBytes: number[] | null = null
      try {
        if (req.docId && !req.docId.startsWith('browser-session-')) {
          pngBytes = await invoke<number[]>('session_render_page_to_png', {
            docId: req.docId,
            pageIndex: req.pageIndex,
            dpi: req.dpi,
          })
        } else if (req.pdfData) {
          pngBytes = await invoke<number[]>('render_page_to_png', {
            data: req.pdfData,
            pageIndex: req.pageIndex,
            dpi: req.dpi,
          })
        }
      } catch (backendErr) {
        // Rust backend unavailable in browser preview, fall through to PDF.js rendering
      }

      if (pngBytes && pngBytes.length > 0) {
        if (!this.activeTokens.has(token) || req.signal?.aborted) {
          throw new Error('Render cancelled')
        }
        const blob = new Blob([new Uint8Array(pngBytes)], { type: 'image/png' })
        const url = URL.createObjectURL(blob)
        localBlobUrls.push(url)
        return this.trackUrl(url)
      }

      // Real PDF client rendering via PDF.js
      const sourceBytes = (req.pdfData && req.pdfData.length > 0) ? req.pdfData : _latestPdfBytes
      if (sourceBytes && sourceBytes.length > 0) {
        try {
          const pdfDoc = await PDFJsEngine.getDocument(sourceBytes)
          const targetPageNum = Math.min(Math.max(1, req.pageIndex + 1), pdfDoc.numPages)
          const page = await pdfDoc.getPage(targetPageNum)
          const scale = req.dpi / 72
          const viewport = page.getViewport({ scale })

          const canvas = document.createElement('canvas')
          canvas.width = viewport.width
          canvas.height = viewport.height
          const ctx = canvas.getContext('2d')
          if (!ctx) throw new Error('Canvas 2D context unavailable')

          ctx.fillStyle = '#ffffff'
          ctx.fillRect(0, 0, canvas.width, canvas.height)

          await page.render({
            canvasContext: ctx,
            viewport,
          }).promise

          if (!this.activeTokens.has(token) || req.signal?.aborted) {
            throw new Error('Render cancelled')
          }

          return new Promise<string>((resolve) => {
            canvas.toBlob((blob) => {
              if (blob) {
                const url = URL.createObjectURL(blob)
                localBlobUrls.push(url)
                resolve(this.trackUrl(url))
              } else {
                resolve('')
              }
            }, 'image/png')
          })
        } catch (pdfErr) {
          console.warn('PDF.js rendering fallback encountered error, rendering placeholder:', pdfErr)
          // ユーザーには空白ページの理由が分からないため、1回だけ通知する
          notifyRenderFallback(req.pageIndex + 1, pdfErr)
        }
      }

      // Fallback empty document canvas if no bytes available
      const pageSize = req.pageSize || { width: 595, height: 842 } // default: A4
      const canvas = document.createElement('canvas')
      const width = Math.max(1, pageSize.width * (req.dpi / 72))
      const height = Math.max(1, pageSize.height * (req.dpi / 72))
      canvas.width = width
      canvas.height = height
      const ctx = canvas.getContext('2d')
      if (ctx) {
        ctx.fillStyle = '#ffffff'
        ctx.fillRect(0, 0, width, height)
        ctx.strokeStyle = '#e2e8f0'
        ctx.lineWidth = 1
        ctx.strokeRect(1, 1, width - 2, height - 2)
        ctx.fillStyle = '#0f172a'
        ctx.font = `bold ${20 * (req.dpi / 72)}px -apple-system, BlinkMacSystemFont, sans-serif`
        ctx.fillText('Nagisa PDF Preview', 40 * (req.dpi / 72), 80 * (req.dpi / 72))
      }

      return new Promise<string>((resolve) => {
        canvas.toBlob((blob) => {
          if (blob) {
            const url = URL.createObjectURL(blob)
            localBlobUrls.push(url)
            resolve(this.trackUrl(url))
          } else {
            resolve('')
          }
        }, 'image/png')
      })
    } finally {
      req.signal?.removeEventListener('abort', abortHandler)
      this.activeTokens.delete(token)
      // このレンダリングで作成された blob URL をすべて revoke する
      for (const url of localBlobUrls) {
        safeRevokeObjectUrl(url)
      }
    }
  }

  async renderSeparationToUrl(req: SeparationRenderRequest): Promise<string> {
    const token = ++this.tokenSeq
    this.activeTokens.add(token)

    if (req.signal?.aborted) {
      this.activeTokens.delete(token)
      throw new Error('Render cancelled')
    }

    const abortHandler = () => {
      this.activeTokens.delete(token)
    }
    req.signal?.addEventListener('abort', abortHandler, { once: true })

    try {
      let pngBytes: number[]
      if (req.docId) {
        pngBytes = await invoke<number[]>('session_render_color_separation', {
          docId: req.docId,
          pageIndex: req.pageIndex,
          dpi: req.dpi,
          showC: req.showC,
          showM: req.showM,
          showY: req.showY,
          showK: req.showK,
          highlightTac: req.highlightTac,
          tacLimit: req.tacLimit,
        })
      } else if (req.pdfData) {
        pngBytes = await invoke<number[]>('render_color_separation', {
          data: req.pdfData,
          pageIndex: req.pageIndex,
          dpi: req.dpi,
          showC: req.showC,
          showM: req.showM,
          showY: req.showY,
          showK: req.showK,
          highlightTac: req.highlightTac,
          tacLimit: req.tacLimit,
        })
      } else {
        throw new Error('Neither docId nor pdfData provided for rendering')
      }

      if (!this.activeTokens.has(token) || req.signal?.aborted) {
        throw new Error('Render cancelled')
      }

      const blob = new Blob([new Uint8Array(pngBytes)], { type: 'image/png' })
      return this.trackUrl(URL.createObjectURL(blob))
    } finally {
      req.signal?.removeEventListener('abort', abortHandler)
      this.activeTokens.delete(token)
    }
  }

  cancelAll(): void {
    this.activeTokens.clear()
    for (const url of this.createdUrls) {
      safeRevokeObjectUrl(url)
    }
    this.createdUrls.clear()
  }
}

export const defaultRenderer = new DefaultRenderer()
