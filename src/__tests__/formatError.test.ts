import { formatError } from '../utils/errorHandler'
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
})
