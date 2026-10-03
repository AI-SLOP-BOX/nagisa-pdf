import { formatError, stripAbsolutePaths } from '../utils/errorHandler'
import { describe, it, expect } from 'vitest'

describe('formatError', () => {
  it('undefined / null を渡すとフォールバックメッセージを返す', () => {
    expect(formatError(undefined, 'フォールバック')).toBe('フォールバック')
    expect(formatError(null as unknown as Error, 'フォールバック')).toBe('フォールバック')
  })

  it('文字列のエラーをそのまま使用する', () => {
    expect(formatError('何かがおかしい', 'デフォルト')).toBe('デフォルト: 何かがおかしい')
  })

  it('Error オブジェクトの message を使用する', () => {
    const err = new Error('構造化されたエラー')
    expect(formatError(err, 'デフォルト')).toBe('デフォルト: 構造化されたエラー')
  })

  it('文字列以外のオブジェクトは String() で変換する', () => {
    const obj = { code: 500, message: 'サーバーエラー' }
    expect(formatError(obj, 'デフォルト')).toBe('デフォルト: [object Object]')
  })

  it('poppler / pdftocairo がないエラーは具体的なメッセージ', () => {
    expect(formatError(new Error('pdftocairo: No such file or directory'), 'エラー')).toBe(
      'Poppler (pdftocairo) がシステムに見つかりません。brew install poppler または apt install poppler-utils でインストールしてください。'
    )
    expect(formatError(new Error('poppler not found'), 'エラー')).toBe(
      'Poppler (pdftocairo) がシステムに見つかりません。brew install poppler または apt install poppler-utils でインストールしてください。'
    )
  })

  it('tesseract がないエラーは具体的なメッセージ', () => {
    expect(formatError(new Error('tesseract: not found'), 'エラー')).toBe(
      'Tesseract OCR がシステムに見つかりません。brew install tesseract tesseract-lang または apt install tesseract-ocr でインストールしてください。'
    )
  })

  it('Invalid PDF / syntax error は破損メッセージ', () => {
    expect(formatError(new Error('Invalid PDF structure'), 'エラー')).toBe(
      'PDFファイルの形式が破損しているか、対応していない暗号化が施されています。'
    )
    expect(formatError(new Error('Failed to load PDF'), 'エラー')).toBe(
      'PDFファイルの形式が破損しているか、対応していない暗号化が施されています。'
    )
    expect(formatError(new Error('syntax error at line 12'), 'エラー')).toBe(
      'PDFファイルの形式が破損しているか、対応していない暗号化が施されています。'
    )
  })

  it('password / encrypted はパスワードメッセージ', () => {
    expect(formatError(new Error('password required'), 'エラー')).toBe(
      'パスワードで保護されているか、権限が不足しています。正しいパスワードを入力してください。'
    )
    expect(formatError(new Error('encrypted document'), 'エラー')).toBe(
      'パスワードで保護されているか、権限が不足しています。正しいパスワードを入力してください。'
    )
  })

  it('out of range / index out of / page index はページ範囲メッセージ', () => {
    expect(formatError(new Error('page index out of range'), 'エラー')).toBe(
      '指定されたページ番号がドキュメントの範囲外です。'
    )
    expect(formatError(new Error('index out of bounds'), 'エラー')).toBe(
      '指定されたページ番号がドキュメントの範囲外です。'
    )
  })

  it('Permission denied は権限メッセージ', () => {
    expect(formatError(new Error('Permission denied'), 'エラー')).toBe(
      'ファイルへのアクセス権限がありません。保存先フォルダの書き込み権限をご確認ください。'
    )
  })

  it('それ以外のエラーはフォールバック＋生メッセージ', () => {
    expect(formatError(new Error('何か予期しないエラー'), '処理中にエラーが発生しました')).toBe(
      '処理中にエラーが発生しました: 何か予期しないエラー'
    )
  })

  it('構造化バックエンドエラー（Tauri IPCオブジェクト）に対応する', () => {
    // 新形式: バックエンド NagisaError のシリアライズ形 { type, details }
    expect(formatError({ type: 'PasswordRequired' }, 'エラー')).toBe(
      'PDFがパスワードで保護されています。閲覧・編集用のパスワードを入力してください。'
    )
    expect(formatError({ type: 'SignedPdfMutationBlocked', details: 'x' }, 'エラー')).toBe(
      '電子署名（暗号署名）で保護されたPDFです。直接変更すると法的効力やハッシュ整合性が破損するため、この操作は制限されています。'
    )
    expect(formatError({ type: 'Timeout', details: 't' }, 'エラー')).toBe(
      '外部処理が制限時間を超過しました（タイムアウト）。ファイルが極端に巨大か複雑な可能性があります。'
    )
    expect(formatError({ type: 'PdfParse' }, 'エラー')).toBe(
      'PDFファイルの構文が破損しているか、非対応の形式です。'
    )
    expect(formatError({ type: 'Io', details: 'deny' }, 'エラー')).toBe(
      'ファイル入出力エラーが発生しました: deny'
    )
    expect(formatError({ type: 'SessionNotFound', details: 'x' }, 'エラー')).toBe(
      '編集セッションが見つからないか期限切れです。ファイルを再度開き直してください。'
    )
  })

  it('構造化InvalidParameterのページ範囲は専用メッセージ', () => {
    expect(
      formatError({ type: 'InvalidParameter', details: 'Page index 9 out of range (total pages: 3)' }, 'エラー')
    ).toBe('指定されたページ番号がドキュメントの範囲外です。')
    expect(formatError({ type: 'InvalidParameter', details: 'bad arg' }, 'エラー')).toBe(
      '指定されたパラメータが無効です: bad arg'
    )
  })

  it('旧形式の日本語メッセージもページ範囲に写像する', () => {
    expect(formatError(new Error('ページ範囲外です'), 'エラー')).toBe(
      '指定されたページ番号がドキュメントの範囲外です。'
    )
  })

  it('旧形式の文字列センチネルも引き続き判別できる', () => {    expect(formatError('PASSWORD_REQUIRED', 'エラー')).toBe(
      'PDFがパスワードで保護されています。閲覧・編集用のパスワードを入力してください。'
    )
    expect(formatError('SIGNED_PDF_MUTATION_ERROR: foo', 'エラー')).toBe(
      '電子署名（暗号署名）で保護されたPDFです。直接変更すると法的効力やハッシュ整合性が破損するため、この操作は制限されています。'
    )
  })

  it('絶対パスはbasenameのみ表示（個人情報漏洩防止）', () => {
    expect(stripAbsolutePaths('Invalid target directory: /tmp/shared/docs: denied')).toBe(
      'Invalid target directory: docs: denied',
    )
    expect(stripAbsolutePaths('Failed: C:\\Temp\\file.pdf')).toBe('Failed: file.pdf')
    expect(stripAbsolutePaths('see https://example.com/a/b for help')).toBe(
      'see https://example.com/a/b for help',
    )
    expect(formatError({ type: 'Io', details: 'Failed to read file: /tmp/shared/a.pdf' }, 'エラー')).toBe(
      'ファイル入出力エラーが発生しました: Failed to read file: a.pdf',
    )
  })
})
