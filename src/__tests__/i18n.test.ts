import { t, setLanguage, getLanguage } from '../utils/i18n'
import { describe, it, expect, beforeEach } from 'vitest'

describe('i18n (legacy API)', () => {
  beforeEach(() => {
    // 各テスト開始時に言語をjaにリセット
    setLanguage('ja')
  })

  it('getLanguage のデフォルトは ja', () => {
    expect(getLanguage()).toBe('ja')
  })

  it('setLanguage(\'en\') で言語が切り替わり、getLanguage が en を返す', () => {
    setLanguage('en')
    expect(getLanguage()).toBe('en')
    setLanguage('ja')
    expect(getLanguage()).toBe('ja')
  })

  it('t() は現在の言語の翻訳オブジェクトを返す', () => {
    setLanguage('ja')
    expect(t().workspace).toBe('ワークスペース')
    expect(t().open).toBe('開く')

    setLanguage('en')
    expect(t().workspace).toBe('Workspace')
    expect(t().open).toBe('Open')
  })

  it('t() の関数型キーは引数を取って文字列を返す', () => {
    setLanguage('ja')
    expect(t().textMoved(1, 100, 200)).toBe('テキストブロック #1 を (100, 200) に移動しました')

    setLanguage('en')
    expect(t().textMoved(1, 100, 200)).toBe('Moved text block #1 to (100, 200)')
  })

  it('t() の verifyCount はカウントに応じた文字列を返す', () => {
    setLanguage('ja')
    expect(t().verifyCount(3)).toBe('3件の署名を検証しました')

    setLanguage('en')
    expect(t().verifyCount(3)).toBe('Verified 3 digital signature(s)')
  })

  it('setLanguage はローカルストレージに保存する（環境がjsdomであれば）', () => {
    if (typeof localStorage !== 'undefined') {
      setLanguage('en')
      expect(localStorage.getItem('nagisa_lang')).toBe('en')
      setLanguage('ja')
      expect(localStorage.getItem('nagisa_lang')).toBe('ja')
    }
  })
})
