import React, { createContext, useContext, useReducer, useEffect, useRef, ReactNode } from 'react'
import type { Language } from './i18n'
import { translations, setLanguage, LANG_CHANGE_EVENT } from './i18n'

// ========== 状態とreducer ==========

interface I18nState {
  lang: Language
}

type I18nAction = { type: 'SET_LANG'; lang: Language }

function i18nReducer(state: I18nState, action: I18nAction): I18nState {
  switch (action.type) {
    case 'SET_LANG':
      return { lang: action.lang }
    default:
      return state
  }
}

// ========== Context ==========

interface I18nContextValue {
  lang: Language
  setLang: (lang: Language) => void
}

const TranslationsContext = createContext<I18nContextValue | null>(null)

// ========== Provider ==========

export function I18nProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(i18nReducer, { lang: 'ja' })
  // Guard against feedback loop: provider's own useEffect calls setLanguage()
  // which dispatches LANG_CHANGE_EVENT. We ignore events triggered by ourselves.
  const selfDispatchRef = useRef(false)

  // マウント時に localStorage から復元（モジュール側 currentLang とも同期）
  useEffect(() => {
    if (typeof localStorage === 'undefined') return
    const saved = localStorage.getItem('nagisa_lang')
    if (saved === 'en' || saved === 'ja') {
      dispatch({ type: 'SET_LANG', lang: saved })
      setLanguage(saved)
    }
  }, [])

  // 言語変更時に localStorage へ永続化（モジュール側 currentLang とも同期）
  useEffect(() => {
    if (typeof localStorage === 'undefined') return
    localStorage.setItem('nagisa_lang', state.lang)
    selfDispatchRef.current = true
    setLanguage(state.lang)
    selfDispatchRef.current = false
  }, [state.lang])

  // Subscribe to external setLanguage() calls (e.g. from useT().setLang())
  // so that Context consumers re-render when language changes outside the provider.
  useEffect(() => {
    if (typeof window === 'undefined') return
    const handler = (e: Event) => {
      if (selfDispatchRef.current) return
      const detail = (e as CustomEvent<{ lang: Language }>).detail
      if (detail && (detail.lang === 'en' || detail.lang === 'ja')) {
        dispatch({ type: 'SET_LANG', lang: detail.lang })
      }
    }
    window.addEventListener(LANG_CHANGE_EVENT, handler)
    return () => window.removeEventListener(LANG_CHANGE_EVENT, handler)
  }, [])

  const setLang = (lang: Language) => dispatch({ type: 'SET_LANG', lang })

  return (
    <TranslationsContext.Provider value={{ lang: state.lang, setLang }}>
      {children}
    </TranslationsContext.Provider>
  )
}

// ========== Hook ==========

export function useTranslations() {
  const ctx = useContext(TranslationsContext)
  if (!ctx) {
    throw new Error('useTranslations(): I18nProvider が木に含まれていません')
  }
  return translations[ctx.lang] || translations.ja
}

export function useLanguage(): Language {
  const ctx = useContext(TranslationsContext)
  if (!ctx) {
    throw new Error('useLanguage(): I18nProvider が木に含まれていません')
  }
  return ctx.lang
}