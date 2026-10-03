import { invoke } from '@tauri-apps/api/core'
import { PDFJsEngine } from './pdfRenderer'

export interface ThumbnailSource {
  pdfData?: number[] | null
  docId?: string | null
}

export interface RenderedThumbnail {
  /** 表示用URL（dataURL または blob URL） */
  url: string
  /** blob URLの場合のみ true（解放管理対象） */
  isBlob: boolean
}

async function renderViaPdfJs(
  sourceBytes: number[],
  pageIndex: number,
  dpi: number,
): Promise<string | null> {
  try {
    const doc = await PDFJsEngine.getDocument(sourceBytes)
    if (pageIndex < 0 || pageIndex >= doc.numPages) return null
    const page = await doc.getPage(pageIndex + 1)
    const scale = dpi / 72
    const vp = page.getViewport({ scale })
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.round(vp.width))
    canvas.height = Math.max(1, Math.round(vp.height))
    const ctx = canvas.getContext('2d')
    if (!ctx) return null
    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0, 0, canvas.width, canvas.height)
    await page.render({ canvasContext: ctx, viewport: vp }).promise
    return canvas.toDataURL('image/jpeg', 0.85)
  } catch (err) {
    console.warn(`[thumbnailService] PDF.js描画失敗 (page ${pageIndex + 1}):`, err)
    return null
  }
}

async function renderViaBackend(
  source: ThumbnailSource,
  pageIndex: number,
  dpi: number,
): Promise<RenderedThumbnail | null> {
  try {
    let pngBytes: number[] | null = null
    if (source.docId && !source.docId.startsWith('browser-session-')) {
      pngBytes = await invoke<number[]>('session_render_page_to_png', {
        docId: source.docId,
        pageIndex,
        dpi,
      })
    } else if (source.pdfData && source.pdfData.length > 0) {
      pngBytes = await invoke<number[]>('render_page_to_png', {
        data: source.pdfData,
        pageIndex,
        dpi,
      })
    }
    if (!pngBytes || pngBytes.length === 0) return null
    const blob = new Blob([new Uint8Array(pngBytes)], { type: 'image/png' })
    return { url: URL.createObjectURL(blob), isBlob: true }
  } catch (err) {
    console.warn(`[thumbnailService] バックエンド描画失敗 (page ${pageIndex + 1}):`, err)
    return null
  }
}

/**
 * 単一ページのサムネイルを描画する共有パイプライン。
 * PDF.js（ブラウザ完結・dataURL）優先、失敗時はバックエンド描画に
 * フォールバックする。両実装（EditorThumbnailSidebar / ThumbnailsPanel）
 * の重複コードを一本化した単一入口。
 *
 * 明示ソースのみ使う。`getLatestBytes()` への暗黙フォールバックはしない:
 * 文書切替の狭間に別文書のバイトで描画要求が走り、誤文書のサムネイルが
 * 混入する恐れがあるため。呼出側（usePageThumbnails）は世代ガードで
 * 結果を破棄するが、要求自体を誤文書で実行しない方が正しい。
 */
export async function renderPageThumbnail(
  source: ThumbnailSource,
  pageIndex: number,
  dpi = 40,
): Promise<RenderedThumbnail | null> {
  const bytes = source.pdfData && source.pdfData.length > 0 ? source.pdfData : null
  if (bytes) {
    const url = await renderViaPdfJs(bytes, pageIndex, dpi)
    if (url) return { url, isBlob: false }
    const backend = await renderViaBackend(source, pageIndex, dpi)
    if (backend) return backend
  } else if (source.docId && !source.docId.startsWith('browser-session-')) {
    return renderViaBackend(source, pageIndex, dpi)
  }
  return null
}

/** PDF.js文書の総ページ数（サムネイル側の件数検出用）。取得不可時は null。 */
export async function detectTotalPages(source: ThumbnailSource): Promise<number | null> {
  const bytes = source.pdfData && source.pdfData.length > 0 ? source.pdfData : null
  if (!bytes) return null
  try {
    const doc = await PDFJsEngine.getDocument(bytes)
    return doc.numPages
  } catch {
    return null
  }
}
