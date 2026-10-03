import { parseNagisaError } from '../types'

/**
 * エラー文中の絶対パスを basename のみに削る。トーストはスクショ共有
 * されるため `/home/xxx/...` のような個人情報を表示しない。
 * URL (http/https) と拡張子なし単語は触らない。
 */
export function stripAbsolutePaths(msg: string): string {
  return msg
    .split(/(\s+)/)
    .map(tok => {
      if (/^\s*$/.test(tok) || /^https?:\/\//i.test(tok) || !/[\\/]/.test(tok)) return tok
      const lead = tok.match(/^["'({\[]+/)?.[0] ?? ''
      const trail = tok.match(/["')}:\].,;:!?]+$/)?.[0] ?? ''
      const core = tok.slice(lead.length, tok.length - (trail.length || 0))
      const parts = core.split(/[\\/]/).filter(p => p.length > 0 && p !== '.' && p !== '..')
      if (parts.length <= 1) return tok
      return lead + parts[parts.length - 1] + trail
    })
    .join('')
}

/**
 * Formats low-level errors and structured backend errors into human-readable, actionable diagnostic messages.
 */
export function formatError(err: unknown, fallbackMessage = '処理中にエラーが発生しました'): string {
  if (!err) return fallbackMessage

  const parsed = parseNagisaError(err)
  switch (parsed.type) {
    case 'PasswordRequired':
      return 'PDFがパスワードで保護されています。閲覧・編集用のパスワードを入力してください。'
    case 'InvalidPassword':
      return '入力されたパスワードが正しくありません。再度ご確認ください。'
    case 'SignedPdfMutationBlocked':
      return '電子署名（暗号署名）で保護されたPDFです。直接変更すると法的効力やハッシュ整合性が破損するため、この操作は制限されています。'
    case 'SessionNotFound':
      return '編集セッションが見つからないか期限切れです。ファイルを再度開き直してください。'
    case 'Timeout':
      return '外部処理が制限時間を超過しました（タイムアウト）。ファイルが極端に巨大か複雑な可能性があります。'
    case 'ExternalToolMissing':
      return `必要な外部ツールが見つかりません: ${parsed.details || 'Poppler / Tesseract をインストールしてください。'}`
    case 'PdfParse':
      return 'PDFファイルの構文が破損しているか、非対応の形式です。'
    case 'Io':
      return `ファイル入出力エラーが発生しました: ${stripAbsolutePaths(parsed.details || '')}`
    case 'InvalidParameter': {
      const d = parsed.details || ''
      if (/page index|out of range|out of bounds|範囲外|ページ番号/i.test(d)) {
        return '指定されたページ番号がドキュメントの範囲外です。'
      }
      return `指定されたパラメータが無効です: ${stripAbsolutePaths(d)}`
    }
    case 'General':
    default:
      break
  }

  const rawMessage = typeof err === 'string'
    ? err
    : err instanceof Error
      ? err.message
      : String(err)

  // Raw string / Error pattern matching (handles both unstructured Tauri errors and standard exceptions)
  if (rawMessage.includes('Invalid PDF') || rawMessage.includes('Failed to load PDF') || rawMessage.includes('syntax error')) {
    return 'PDFファイルの形式が破損しているか、対応していない暗号化が施されています。'
  }
  if (rawMessage.includes('password') || rawMessage.includes('encrypted')) {
    return 'パスワードで保護されているか、権限が不足しています。正しいパスワードを入力してください。'
  }
  if (rawMessage.includes('out of range') || rawMessage.includes('index out of') || rawMessage.includes('page index')) {
    return '指定されたページ番号がドキュメントの範囲外です。'
  }
  if (rawMessage.includes('範囲外') || rawMessage.includes('ページ番号')) {
    return '指定されたページ番号がドキュメントの範囲外です。'
  }
  if (rawMessage.includes('Permission denied')) {
    return 'ファイルへのアクセス権限がありません。保存先フォルダの書き込み権限をご確認ください。'
  }

  // System command missing / not installed
  if (rawMessage.includes('No such file or directory') || rawMessage.includes('not found')) {
    if (rawMessage.includes('pdftocairo') || rawMessage.includes('poppler')) {
      return 'Poppler (pdftocairo) がシステムに見つかりません。brew install poppler または apt install poppler-utils でインストールしてください。'
    }
    if (rawMessage.includes('tesseract')) {
      return 'Tesseract OCR がシステムに見つかりません。brew install tesseract tesseract-lang または apt install tesseract-ocr でインストールしてください。'
    }
    return `依存プログラムまたはファイルが見つかりません: ${rawMessage}`
  }

  // Return clean formatted error
  return `${fallbackMessage}: ${rawMessage}`
}

