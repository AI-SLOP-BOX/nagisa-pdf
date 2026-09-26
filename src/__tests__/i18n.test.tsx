import { t, setLanguage, getLanguage, LANG_CHANGE_EVENT, useT } from '../utils/i18n'
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { render, screen, act, cleanup } from '@testing-library/react'
import React from 'react'

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

describe('i18n reactive (useT + LANG_CHANGE_EVENT)', () => {
  beforeEach(() => {
    setLanguage('ja')
  })

  afterEach(() => {
    cleanup()
    setLanguage('ja')
  })

  it('setLanguage は LANG_CHANGE_EVENT を発行して購読者に通知する', () => {
    let received: string | null = null
    const handler = (e: Event) => {
      received = (e as CustomEvent<{ lang: string }>).detail.lang
    }
    window.addEventListener(LANG_CHANGE_EVENT, handler)
    try {
      setLanguage('en')
      expect(received).toBe('en')
    } finally {
      window.removeEventListener(LANG_CHANGE_EVENT, handler)
    }
  })

  it('useT は setLanguage で再レンダリングし、新しい言語の翻訳を返す', () => {
    const Probe: React.FC = () => {
      const { t: tt, lang } = useT()
      return <span data-testid="i18n-probe">{`${lang}:${tt().workspace}`}</span>
    }
    render(<Probe />)
    expect(screen.getByTestId('i18n-probe').textContent).toBe('ja:ワークスペース')
    act(() => {
      setLanguage('en')
    })
    expect(screen.getByTestId('i18n-probe').textContent).toBe('en:Workspace')
  })
})
