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
  /** /Rotate 適用後の表示回転角（0/90/180/270）。欠番時は0扱い。 */
  rotation?: number
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
   * Detect Standard Security Handler password protection without exposing
   * any content. False when the backend is unavailable (browser preview).
   */
  static async isEncrypted(data: number[]): Promise<boolean> {
    return invoke<boolean>('is_pdf_encrypted', { data })
  }

  /**
   * Remove password protection. Throws the backend's honest error on a
   * wrong password ("パスワードが正しくありません").
   */
  static async decryptPdf(data: number[], password: string): Promise<number[]> {
    return invoke<number[]>('decrypt_pdf', { data, password })
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
   * ネイティブセッション判定。`docId` の有無ではなくこの判定で分岐する
   * こと（browser-session-* に getSessionBytes 等を投げると SessionNotFound）。
   */
  static isNativeDoc(docId: string | null | undefined): docId is string {
    return !!docId && !docId.startsWith('browser-session-')
  }

  /**
   * 突然変異系コマンドの共通入口。ネイティブ時は `session_exec`
   * （args のみ IPC 越し・文書バイト転送なし・戻り1往復）、
   * プレビュー時は旧バイト経路。戻りは新バイト列。
   */
  static async invokeOp(
    op: string,
    docId: string | null | undefined,
    pdfData: number[] | null,
    args: Record<string, unknown>,
  ): Promise<number[]> {
    const native = docId && !docId.startsWith('browser-session-') ? docId : null
    if (native) {
      return invoke<number[]>('session_exec', { docId: native, op, args })
    }
    const bytes =
      pdfData && pdfData.length > 0
        ? pdfData
        : await DocumentService.requirePreviewBytes().catch(() => null)
    if (!bytes) throw new Error('対象PDFデータがありません')
    return invoke<number[]>(op, { data: bytes, ...args })
  }

  /**
   * セッション優先で現行バイトを取得する共通入口。browser-session-* には
   * Rustセッションが無いため手元バイトにフォールバックする。各パネルの
   * `docId ? getSessionBytes : pdfData` をこれに統一し、プレビュー時の
   * SessionNotFound 全滅を防ぐ。取得失敗時は null（呼出側で無操作）。
   */
  static async getCurrentBytes(
    docId: string | null | undefined,
    pdfData: number[] | null,
  ): Promise<number[] | null> {
    if (docId && !docId.startsWith('browser-session-')) {
      try {
        return await DocumentService.getSessionBytes(docId)
      } catch (err) {
        console.error(`[DocumentService.getCurrentBytes] セッション読込失敗 (docId=${docId}):`, err)
        return null
      }
    }
    return pdfData
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
    let lastErr: unknown = null
    try {
      if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
        return await invoke<number>('session_get_page_count', { docId: docIdOrData })
      }
      if (Array.isArray(docIdOrData)) {
        try {
          return await invoke<number>('get_page_count', { data: docIdOrData })
        } catch (err) {
          console.error('[DocumentService.getPageCount] セッションのページ数取得失敗, PDF.jsフォールバック:', err)
          lastErr = err
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
      console.error('[DocumentService.getPageCount] 予期しないエラー:', e)
      lastErr = e
    }
    // 全経路失敗を「1ページ」偽装しない。呼出側でcatchし空表示と区別する。
    throw lastErr instanceof Error ? lastErr : new Error(`ページ数の取得に失敗しました: ${String(lastErr)}`)
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
    let lastErr: unknown = null
    if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
      try {
        return await invoke<TextBlock[]>('session_get_text_blocks', {
          docId: docIdOrData,
          pageIndex,
        })
      } catch (err) {
        console.error(`[DocumentService.getTextBlocks] セッションのテキストブロック取得失敗 (docId=${docIdOrData}, pageIndex=${pageIndex}):`, err)
        lastErr = err
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
        lastErr = err
      }
    }
    // 全経路失敗を「空ページ」偽装しない。正常な空ページは上流で [] が返る。
    throw lastErr instanceof Error ? lastErr : new Error(`テキストブロックの取得に失敗しました: ${String(lastErr)}`)
  }

  static async getPdfMetadata(docIdOrData: string | number[]): Promise<Record<string, unknown>> {
    if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
      return invoke<Record<string, unknown>>('session_get_metadata', { docId: docIdOrData })
    }
    if (typeof docIdOrData === 'string') {
      return invoke<Record<string, unknown>>('get_pdf_metadata', {
        data: await DocumentService.requirePreviewBytes(),
      })
    }
    return invoke<Record<string, unknown>>('get_pdf_metadata', { data: docIdOrData })
  }

  static async getBookmarks(docIdOrData: string | number[]): Promise<Bookmark[]> {
    if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
      return invoke<Bookmark[]>('session_get_bookmarks', { docId: docIdOrData })
    }
    if (typeof docIdOrData === 'string') {
      return invoke<Bookmark[]>('get_bookmarks', {
        data: await DocumentService.requirePreviewBytes(),
      })
    }
    return invoke<Bookmark[]>('get_bookmarks', { data: docIdOrData })
  }

  /**
   * browser-session 経路の投機バイトを取り出す。文書オープン時に
   * clearLatestBytes() で破棄されるため残差レースは残るが、旧文書の
   * 誤読よりは「無い」と明示する方がまし、という判断。
   */
  private static async requirePreviewBytes(): Promise<number[]> {
    const latest = PDFJsEngine.getLatestBytes()
    if (!latest || latest.length === 0) {
      throw new Error('プレビュー文書のデータがありません（再読み込みしてください）')
    }
    return latest
  }

  static async getFormFields(docIdOrData: string | number[]): Promise<FormField[]> {
    if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
      return invoke<FormField[]>('session_get_form_fields', { docId: docIdOrData })
    }
    if (typeof docIdOrData === 'string') {
      const latest = PDFJsEngine.getLatestBytes()
      if (latest) return invoke<FormField[]>('get_form_fields', { data: latest })
      throw new Error('フォーム取得対象のPDFデータがありません')
    }
    return invoke<FormField[]>('get_form_fields', { data: docIdOrData })
  }

  static async searchPdf(docIdOrData: string | number[], query: string): Promise<SearchResult[]> {
    if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
      return invoke<SearchResult[]>('session_search_text', { docId: docIdOrData, query })
    }
    if (typeof docIdOrData === 'string') {
      return invoke<SearchResult[]>('search_text', {
        data: await DocumentService.requirePreviewBytes(),
        query,
      })
    }
    return invoke<SearchResult[]>('search_text', { data: docIdOrData, query })
  }

  static async verifySignatures(docIdOrData: string | number[]): Promise<{ signatures?: SignatureInfo[]; count?: number }> {
    const isPreviewString =
      typeof docIdOrData === 'string' && docIdOrData.startsWith('browser-session-')
    let rawResult: { signatures?: SignatureInfo[]; count?: number } = { signatures: [], count: 0 }
    if (typeof docIdOrData === 'string' && !isPreviewString) {
      try {
        rawResult = await invoke<{ signatures?: SignatureInfo[]; count?: number }>('session_verify_signature', { docId: docIdOrData })
      } catch (err) {
        console.error('[DocumentService.verifySignatures] セッション署名検証失敗:', err)
        // 検証失敗を「署名なし」と偽装しない。呼び出し側のcatchで検証エラー表示へ。
        throw err
      }
    } else {
      try {
        const data =
          typeof docIdOrData === 'string' ? await DocumentService.requirePreviewBytes() : docIdOrData
        rawResult = await invoke<{ signatures?: SignatureInfo[]; count?: number }>('verify_signature', { data, signatureIndex: 0 })
      } catch (err) {
        console.error('[DocumentService.verifySignatures] 直接署名検証失敗:', err)
        // 検証失敗を「署名なし」と偽装しない。呼び出し側のcatchで検証エラー表示へ。
        throw err
      }
    }

    // Try full CMS cryptographic verification for signed signatures
    let bytes: number[] | null = null
    if (typeof docIdOrData === 'string' && !isPreviewString) {
      bytes = await DocumentService.getSessionBytes(docIdOrData)
    } else if (typeof docIdOrData === 'string') {
      bytes = await DocumentService.requirePreviewBytes()
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
              has_verification_dss: boolean
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
              has_verification_dss: cmsReport.has_verification_dss,
              // Honest PAdES profile: LTV only when a /DSS with validation
              // material is actually embedded; B-T requires a timestamp.
              trust_level: cmsReport.cms_signature_valid
                ? cmsReport.has_verification_dss
                  ? 'PAdES-LTV（DSS検証材料を埋め込み済み）'
                  : cmsReport.timestamp?.present
                    ? 'PAdES-B-T (RFC3161タイムスタンプ)'
                    : 'PAdES-B-B（基礎署名・失効情報なし）'
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
    if (typeof docIdOrData === 'string' && !docIdOrData.startsWith('browser-session-')) {
      return invoke<void>('session_print_pdf', { docId: docIdOrData })
    }
    if (typeof docIdOrData === 'string') {
      return invoke<void>('print_pdf', { data: await DocumentService.requirePreviewBytes() })
    }
    return invoke<void>('print_pdf', { data: docIdOrData })
  }
}
