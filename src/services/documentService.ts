import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { PDFJsEngine } from './pdfRenderer'
import type { SignatureInfo, CompatibilityReport, EngineHealth } from '../types'

export interface SessionInfo {
  id: string
  dirty: boolean
  undoCount: number
  redoCount: number
}

export interface TextBlock {
  id: number
  text: string
  // COORDINATE CONTRACT: PDF user-space points (72 DPI), origin at the
  // page's BOTTOM-LEFT (y grows upward) — matches the Rust backend (`Tm`
  // text matrix) and the `domToPdf`/`pdfToDom` converters in
  // `usePDFCoordinates`, which flip y when rendering to the DOM.
  // Contrasts with `UserAnnotation.y` / `OCRDetectedBlock.y`, which are
  // top-origin: never assign one to the other without converting.
  x: number
  y: number
  width: number
  height: number
  font_name: string
  font_size: number
  color: string
  page_index: number
}

export interface PageDimensions {
  width: number
  height: number
}

export interface SearchResult {
  page: number
  text: string
}

export interface Bookmark {
  title: string
  page: number
}

export interface FormField {
  name: string
  type: string
  value: string
}

export class DocumentService {
  /**
   * Open native file dialog to pick a PDF and read its bytes.
   */
  static async openFileDialog(): Promise<{ path: string; name: string; bytes: number[] } | null> {
    const selected = await open({
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      multiple: false,
    })
    if (!selected || typeof selected !== 'string') return null
    return this.loadFilePath(selected)
  }

  /**
   * Load a PDF file directly from an absolute file path.
   */
  static async loadFilePath(filePath: string): Promise<{ path: string; name: string; bytes: number[] }> {
    const bytes = await invoke<number[]>('read_file_bytes', { path: filePath })
    const name = filePath.split(/[/\\]/).pop() || 'document.pdf'
    return { path: filePath, name, bytes }
  }

  /**
   * Save bytes to native file path picked by user.
   */
  static async saveFileDialog(defaultName: string, data: number[]): Promise<string | null> {
    const path = await save({
      defaultPath: defaultName || 'document.pdf',
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    })
    if (!path) return null
    await invoke('write_file_bytes', { path, data })
    return path
  }

  /**
   * Create an in-memory document session on the Rust backend with per-document RwLock.
   */
  static async createSession(data: number[]): Promise<string> {
    return invoke<string>('session_open_pdf', { data })
  }

  /**
   * Close and evict an in-memory document session.
   */
  static async closeSession(docId: string): Promise<boolean> {
    return invoke<boolean>('session_close', { docId })
  }

  /**
   * Retrieve serialized PDF bytes from active session.
   */
  static async getSessionBytes(docId: string): Promise<number[]> {
    return invoke<number[]>('session_get_bytes', { docId })
  }

  /**
   * Rotate a page using a lightweight delta command.
   */
  static async rotatePage(docId: string, pageIndex: number, degrees: number): Promise<void> {
    return invoke<void>('session_rotate_page', {
      docId,
      pageIndex,
      degrees,
    })
  }

  /**
   * Delete a page using a byte-bounded snapshot backup.
   */
  static async deletePage(docId: string, pageIndex: number): Promise<void> {
    return invoke<void>('session_delete_page', {
      docId,
      pageIndex,
    })
  }

  /**
   * Undo last operation on session. Returns true if an action was undone.
   */
  static async undo(docId: string): Promise<boolean> {
    return invoke<boolean>('session_undo', { docId })
  }

  /**
   * Redo previously undone operation on session. Returns true if an action was redone.
   */
  static async redo(docId: string): Promise<boolean> {
    return invoke<boolean>('session_redo', { docId })
  }

  /**
   * Update active session with newly transformed PDF bytes while recording an Undo FullSnapshot.
   */
  static async updateSessionBytes(docId: string, description: string, data: number[]): Promise<void> {
    return invoke<void>('session_update_bytes', { docId, description, data })
  }

  /**
   * Get undo/redo availability and metrics for active session.
   */
  static async getHistoryStatus(docId: string): Promise<{
    can_undo: boolean
    can_redo: boolean
    undo_count: number
    redo_count: number
    history_bytes: number
  }> {
    return invoke('session_get_history_status', { docId })
  }

  /**
   * Session-based Inspection & Query (Zero IPC byte passing)
   */
  static async getPageCount(docIdOrData: string | number[]): Promise<number> {
    try {
      if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
        return await invoke<number>('session_get_page_count', { docId: docIdOrData })
      }
      if (Array.isArray(docIdOrData)) {
        try {
          return await invoke<number>('get_page_count', { data: docIdOrData })
        } catch (err) {
          console.error('[DocumentService.getPageCount] セッションのページ数取得失敗, PDF.jsフォールバック:', err)
          const doc = await PDFJsEngine.getDocument(docIdOrData)
          return doc.numPages
        }
      }
      if (typeof docIdOrData === 'string' && docIdOrData.startsWith('browser-session-')) {
        const latestBytes = PDFJsEngine.getLatestBytes()
        if (latestBytes) {
          const doc = await PDFJsEngine.getDocument(latestBytes)
          return doc.numPages
        }
      }
    } catch (e) {
      console.error('[DocumentService.getPageCount] 予期しないエラー, デフォルト1を返します:', e)
    }
    return 1
  }

  static async getPageDimensions(docIdOrData: string | number[], pageIndex: number): Promise<PageDimensions> {
    if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
      try {
        return await invoke<PageDimensions>('session_get_page_dimensions', {
          docId: docIdOrData,
          pageIndex,
        })
      } catch (err) {
        console.error(`[DocumentService.getPageDimensions] セッションのページサイズ取得失敗 (docId=${docIdOrData}, pageIndex=${pageIndex}):`, err)
      }
    }
    const sourceBytes = Array.isArray(docIdOrData) ? docIdOrData : PDFJsEngine.getLatestBytes()
    if (sourceBytes && sourceBytes.length > 0) {
      try {
        const doc = await PDFJsEngine.getDocument(sourceBytes)
        const pageNum = Math.min(Math.max(1, pageIndex + 1), doc.numPages)
        const page = await doc.getPage(pageNum)
        const vp = page.getViewport({ scale: 1.0 })
        return { width: Math.round(vp.width), height: Math.round(vp.height) }
      } catch (err) {
        console.error(`[DocumentService.getPageDimensions] PDF.jsによるページサイズ取得失敗 (pageIndex=${pageIndex}):`, err)
      }
    }
    return { width: 595, height: 842 }
  }

  static async getTextBlocks(docIdOrData: string | number[], pageIndex: number): Promise<TextBlock[]> {
    if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
      try {
        return await invoke<TextBlock[]>('session_get_text_blocks', {
          docId: docIdOrData,
          pageIndex,
        })
      } catch (err) {
        console.error(`[DocumentService.getTextBlocks] セッションのテキストブロック取得失敗 (docId=${docIdOrData}, pageIndex=${pageIndex}):`, err)
      }
    }
    const sourceBytes = Array.isArray(docIdOrData) ? docIdOrData : PDFJsEngine.getLatestBytes()
    if (sourceBytes && sourceBytes.length > 0) {
      try {
        const doc = await PDFJsEngine.getDocument(sourceBytes)
        const pageNum = Math.min(Math.max(1, pageIndex + 1), doc.numPages)
        const page = await doc.getPage(pageNum)
        const textContent = await page.getTextContent()
        // getTextContent() yields TextItem | TextMarkedContent; only TextItem has `str`.
        interface PdfTextItemLike {
          str?: string
          transform?: number[]
          width?: number
          height?: number
          fontName?: string
        }
        return textContent.items.map((raw, idx) => {
          const item = raw as PdfTextItemLike
          return {
            id: idx + 1,
            text: item.str || '',
            x: item.transform ? item.transform[4] : 50,
            y: item.transform ? item.transform[5] : 100,
            width: item.width || 100,
            height: item.height || 20,
            font_name: item.fontName || 'Helvetica',
            font_size: item.height || 12,
            color: '#000000',
            page_index: pageIndex,
          }
        })
      } catch (err) {
        console.error('[DocumentService.getTextBlocks] PDF.jsによるテキストブロック取得失敗:', err)
      }
    }
    return []
  }

  static async getPdfMetadata(docIdOrData: string | number[]): Promise<Record<string, unknown>> {
    if (typeof docIdOrData === 'string') {
      return invoke<Record<string, unknown>>('session_get_metadata', { docId: docIdOrData })
    }
    return invoke<Record<string, unknown>>('get_pdf_metadata', { data: docIdOrData })
  }

  static async getBookmarks(docIdOrData: string | number[]): Promise<Bookmark[]> {
    if (typeof docIdOrData === 'string') {
      return invoke<Bookmark[]>('session_get_bookmarks', { docId: docIdOrData })
    }
    return invoke<Bookmark[]>('get_bookmarks', { data: docIdOrData })
  }

  static async getFormFields(docIdOrData: string | number[]): Promise<FormField[]> {
    if (typeof docIdOrData === 'string') {
      return invoke<FormField[]>('session_get_form_fields', { docId: docIdOrData })
    }
    return invoke<FormField[]>('get_form_fields', { data: docIdOrData })
  }

  static async searchPdf(docIdOrData: string | number[], query: string): Promise<SearchResult[]> {
    if (typeof docIdOrData === 'string') {
      return invoke<SearchResult[]>('session_search_text', { docId: docIdOrData, query })
    }
    return invoke<SearchResult[]>('search_text', { data: docIdOrData, query })
  }

  static async verifySignatures(docIdOrData: string | number[]): Promise<{ signatures?: SignatureInfo[]; count?: number }> {
    let rawResult: { signatures?: SignatureInfo[]; count?: number } = { signatures: [], count: 0 }
    if (typeof docIdOrData === 'string') {
      try {
        rawResult = await invoke<{ signatures?: SignatureInfo[]; count?: number }>('session_verify_signature', { docId: docIdOrData })
      } catch (err) {
        console.error('[DocumentService.verifySignatures] セッション署名検証失敗:', err)
        return { signatures: [], count: 0 }
      }
    } else {
      try {
        rawResult = await invoke<{ signatures?: SignatureInfo[]; count?: number }>('verify_signature', { data: docIdOrData, signatureIndex: 0 })
      } catch (err) {
        console.error('[DocumentService.verifySignatures] 直接署名検証失敗:', err)
        return { signatures: [], count: 0 }
      }
    }

    // Try full CMS cryptographic verification for signed signatures
    let bytes: number[] | null = null
    if (typeof docIdOrData === 'string') {
      bytes = await DocumentService.getSessionBytes(docIdOrData)
    } else {
      bytes = docIdOrData
    }

    if (bytes && rawResult.signatures && rawResult.signatures.length > 0) {
      const enriched = await Promise.all(
        rawResult.signatures.map(async (sig, idx) => {
          try {
            const cmsReport = await invoke<{
              cms_signature_valid: boolean
              digest_matches: boolean
              digest_algorithm: string
              signer_subject: string
              signer_issuer: string
              chain_valid: boolean | null
              chain_details: string
               revocation_status: string
               revocation_details: string
              timestamp?: {
                present: boolean
                imprint_matches: boolean
                gen_time: string
                tsa_subject: string
              }
              warnings: string[]
            }>('verify_pdf_cms', { data: bytes, signatureIndex: idx, trustRootsPem: null })

            return {
              ...sig,
              status: cmsReport.cms_signature_valid ? 'valid' : 'invalid',
              integrity_verified: cmsReport.digest_matches,
              digest_algorithm: cmsReport.digest_algorithm,
              digest_matches: cmsReport.digest_matches,
              cms_signature_valid: cmsReport.cms_signature_valid,
               revocation_status: cmsReport.revocation_status,
               chain_details: cmsReport.chain_details,
               revocation_details: cmsReport.revocation_details,
              chain_valid: cmsReport.chain_valid,
              trust_level: cmsReport.cms_signature_valid
                ? (cmsReport.timestamp?.present ? 'PAdES / RFC3161 LTV署名' : 'CMS/PKCS#7 デジタル署名')
                : '暗号署名検証不一致',
              certificate_issuer: cmsReport.signer_issuer || sig.certificate_issuer,
              tsa_subject: cmsReport.timestamp?.tsa_subject,
              imprint_matches: cmsReport.timestamp?.imprint_matches,
              notice: cmsReport.warnings.length > 0 ? cmsReport.warnings.join('; ') : undefined,
            }
          } catch {
            return sig
          }
        })
      )
      return { signatures: enriched, count: enriched.length }
    }

    return rawResult
  }

  static async signPdfCms(params: {
    data: number[]
    pageIndex: number
    x: number
    y: number
    width: number
    height: number
    signerName: string
    reason: string
    location?: string
    contactInfo?: string
    p12Data?: number[]
    p12Password?: string
    privateKeyPem?: string
    certificatePem?: string
    tsaUrl?: string
  }): Promise<number[]> {
    return invoke<number[]>('sign_pdf_cms', {
      data: params.data,
      pageIndex: params.pageIndex,
      x: params.x,
      y: params.y,
      width: params.width,
      height: params.height,
      signerName: params.signerName,
      reason: params.reason,
      location: params.location,
      contactInfo: params.contactInfo,
      p12Data: params.p12Data,
      p12Password: params.p12Password,
      privateKeyPem: params.privateKeyPem,
      certificatePem: params.certificatePem,
      tsaUrl: params.tsaUrl,
    })
  }

  static async inspectCompatibility(data: number[]): Promise<CompatibilityReport> {
    return invoke<CompatibilityReport>('inspect_compatibility', { data })
  }

  static async getEngineHealth(): Promise<EngineHealth> {
    return invoke<EngineHealth>('get_engine_health')
  }


  static async printPdf(docIdOrData: string | number[]): Promise<void> {
    if (typeof docIdOrData === 'string') {
      return invoke<void>('session_print_pdf', { docId: docIdOrData })
    }
    return invoke<void>('print_pdf', { data: docIdOrData })
  }
}
