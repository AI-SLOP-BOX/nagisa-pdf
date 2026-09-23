import React, { createContext, useContext, useReducer, useEffect, ReactNode } from 'react'
import type { Language } from './i18n'
import { translations } from './i18n'

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

  // マウント時に localStorage から復元
  useEffect(() => {
    if (typeof localStorage === 'undefined') return
    const saved = localStorage.getItem('nagisa_lang')
    if (saved === 'en' || saved === 'ja') {
      dispatch({ type: 'SET_LANG', lang: saved })
    }
  }, [])

  // 言語変更時に localStorage へ永続化
  useEffect(() => {
    if (typeof localStorage === 'undefined') return
    localStorage.setItem('nagisa_lang', state.lang)
  }, [state.lang])

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