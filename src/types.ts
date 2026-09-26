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
  // Full cryptographic CMS inspection fields
  digest_algorithm?: string
  digest_matches?: boolean
  cms_signature_valid?: boolean
  chain_valid?: boolean | null
  chain_details?: string
  revocation_status?: string
  revocation_details?: string
  has_verification_dss?: boolean
  tsa_subject?: string
  imprint_matches?: boolean
}

export interface Pkcs11Certificate {
  certificate_id: string
  label: string
  subject: string
  issuer: string
  serial_number: string
  sha256_fingerprint: string
}

export interface Pkcs11Slot {
  slot_id: number
  description: string
  manufacturer: string
  token_label: string
  token_serial: string
  certificates: Pkcs11Certificate[]
}

export interface CompatibilityReport {
  pdf_version: string
  page_count: number
  signed_signature_count: number
  certified: boolean
  has_xref_stream: boolean
  linearized: boolean
  encrypted: boolean
  has_object_stream: boolean
  has_portfolio: boolean
  has_attachments: boolean
  parseable: boolean
}

export interface EngineHealth {
  version: string
  binaries: Record<string, boolean>
  features: Record<string, boolean>
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
