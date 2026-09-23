import { invoke } from '@tauri-apps/api/core'

import * as pdfjsLib from 'pdfjs-dist'
import pdfWorker from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

if (typeof window !== 'undefined') {
  pdfjsLib.GlobalWorkerOptions.workerSrc = pdfWorker
}

export class PDFJsEngine {
  private static cachedDoc: { key: string; doc: pdfjsLib.PDFDocumentProxy } | null = null
  // Full hashes are computed once per unique input object (byte arrays are stable refs)
  private static hashCache = new WeakMap<object, string>()

  private static computeHashKey(source: Uint8Array | number[], data: Uint8Array): string {
    const cached = this.hashCache.get(source)
    if (cached) return cached
    const len = data.length
    let key: string
    if (len === 0) {
      key = 'empty_0'
    } else {
      // Full FNV-1a 32-bit hash over every byte (sampling could miss real content changes)
      let hash = 0x811c9dc5
      for (let i = 0; i < len; i++) {
        hash ^= data[i]
        hash = Math.imul(hash, 0x01000193)
      }
      key = `${len}_${(hash >>> 0).toString(16)}`
    }
    this.hashCache.set(source, key)
    return key
  }

  static async getDocument(data: number[] | Uint8Array): Promise<pdfjsLib.PDFDocumentProxy> {
    const uint8 = data instanceof Uint8Array ? data : new Uint8Array(data)
    const key = this.computeHashKey(data, uint8)
    if (this.cachedDoc && this.cachedDoc.key === key) {
      return this.cachedDoc.doc
    }
    const base = typeof document !== 'undefined' ? document.baseURI : '/'
    const loadingTask = pdfjsLib.getDocument({
      data: uint8,
      // Local copies (see scripts/copy_pdfjs_assets.mjs) keep CJK cMaps & standard fonts working offline
      cMapUrl: new URL('cmaps/', base).href,
      cMapPacked: true,
      standardFontDataUrl: new URL('standard_fonts/', base).href,
    })
    const doc = await loadingTask.promise
    const prev = this.cachedDoc
    this.cachedDoc = { key, doc }
    // Release the worker/resources of the replaced document
    prev?.doc.destroy().catch(() => {})
    return doc
  }

  static clearCache() {
    const prev = this.cachedDoc
    this.cachedDoc = null
    prev?.doc.destroy().catch(() => {})
  }

  /** Destroy the cached document's worker. Call on app teardown. */
  static destroy() {
    this.clearCache()
  }
}

export interface RenderRequest {
  pdfData?: number[]
  docId?: string
  pageIndex: number
  dpi: number
  signal?: AbortSignal
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

/**
 * DefaultRenderer provides reliable rendering using the backend rendering pipeline
 * with session zero-IPC support, token-based stale-result suppression, and high-fidelity PDF.js client fallback.
 */
export class DefaultRenderer implements PDFRenderer {
  private activeTokens = new Set<number>()
  private tokenSeq = 0
  private createdUrls = new Set<string>()
  public static lastPdfBytes: number[] | null = null

  private trackUrl(url: string): string {
    if (url && url.startsWith('blob:')) {
      this.createdUrls.add(url)
      // If excessive cached URLs exist, cleanup oldest
      if (this.createdUrls.size > 50) {
        const oldest = this.createdUrls.values().next().value
        if (oldest) {
          URL.revokeObjectURL(oldest)
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
      DefaultRenderer.lastPdfBytes = req.pdfData
    }

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
        return this.trackUrl(URL.createObjectURL(blob))
      }

      // Real PDF client rendering via PDF.js
      const sourceBytes = (req.pdfData && req.pdfData.length > 0) ? req.pdfData : DefaultRenderer.lastPdfBytes
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

          if (ctx) {
            // Fill background white
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
                  resolve(this.trackUrl(URL.createObjectURL(blob)))
                } else {
                  resolve('')
                }
              }, 'image/png')
            })
          }
        } catch (pdfErr) {
          console.warn('PDF.js rendering fallback encountered error, rendering placeholder:', pdfErr)
        }
      }

      // Fallback empty document canvas if no bytes available
      const canvas = document.createElement('canvas')
      const width = 595 * (req.dpi / 72)
      const height = 842 * (req.dpi / 72)
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
            resolve(this.trackUrl(URL.createObjectURL(blob)))
          } else {
            resolve('')
          }
        }, 'image/png')
      })
    } finally {
      req.signal?.removeEventListener('abort', abortHandler)
      this.activeTokens.delete(token)
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
      URL.revokeObjectURL(url)
    }
    this.createdUrls.clear()
  }
}

export const defaultRenderer = new DefaultRenderer()
