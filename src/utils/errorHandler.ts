import { parseNagisaError } from '../types'

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
    case 'Timeout':
      return '外部処理が制限時間を超過しました（タイムアウト）。ファイルが極端に巨大か複雑な可能性があります。'
    case 'ExternalToolMissing':
      return `必要な外部ツールが見つかりません: ${parsed.details || 'Poppler / Tesseract をインストールしてください。'}`
    case 'PdfParse':
      return 'PDFファイルの構文が破損しているか、非対応の形式です。'
    case 'Io':
      return `ファイル入出力エラーが発生しました: ${parsed.details || ''}`
    case 'InvalidParameter':
      return `指定されたパラメータが無効です: ${parsed.details || ''}`
    case 'General':
    default:
      break
  }

  const rawMessage = typeof err === 'string'
    ? err
    : err instanceof Error
      ? err.message
      : String(err)

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

