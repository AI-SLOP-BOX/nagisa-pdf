import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach, vi } from 'vitest'

// React 18+ の自然なクリーンアップ
afterEach(() => {
  cleanup()
})

// テスト環境で Tauri API が必要になる可能性に備えて、モックを最小確保
vi.stubGlobal('invoke', vi.fn())
vi.stubGlobal('open', vi.fn())
vi.stubGlobal('save', vi.fn())

// ローカルストレージが jsdom で使えることを確認
// （jsdom は localStorage をサポートしている）