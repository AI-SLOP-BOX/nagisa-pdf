export type View =
  | 'home'
  | 'pdf-editor'
  | 'convert'
  | 'merge'
  | 'split'
  | 'compress'
  | 'annotate'
  | 'ocr'
  | 'protect'
  | 'scanner'
  | 'recent'
  | 'favorites'

export interface RecentPDFFile {
  id: string
  name: string
  date: string
  size: string
  filePath?: string
}

/** Editor sidebar tabs shared between App, HomeView and PDFEditorView. */
export type EditorTab =
  | 'edit'
  | 'annotate'
  | 'forms'
  | 'organize'
  | 'pages'
  | 'security'
  | 'text'
  | 'tools'

/**
 * Backend command executor. Every command takes a `data` payload injected by
 * the executor itself plus command-specific camelCase arguments (Tauri v2
 * `#[tauri::command]` expects camelCase keys by default).
 */
export type PdfExec = (cmd: string, args: Record<string, unknown>) => Promise<unknown>

/** Digital signature inspection result (shared across panels & services). */
export interface SignatureInfo {
  name: string
  signer: string
  reason: string
  status: string
  timestamp: string
  aatl_verified?: boolean
  trust_level?: string
  certificate_issuer?: string
  revocation_check?: string
  integrity_verified?: boolean
  notice?: string
}

export interface PDFPageInfo {
  index: number
  width: number
  height: number
  rotation: number
}

export interface ScanResult {
  originalPath: string
  correctedPath: string
  width: number
  height: number
}

export interface OCRResult {
  text: string
  confidence: number
  pageCount: number
}

export interface AppSettings {
  outputDir: string
  defaultDPI: number
  ocrLanguage: string
}
