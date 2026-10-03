import { describe, it, expect, vi, beforeEach } from 'vitest'
import { defaultRenderer, PDFJsEngine } from '../services/pdfRenderer'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }))

const revoked: string[] = []
let seq = 1000

beforeEach(() => {
  invokeMock.mockReset()
  revoked.length = 0
  // seqはリセットしない（cancelAllが前テストのURLをrevokeする正当動作と衝突させない）
  vi.stubGlobal('URL', {
    createObjectURL: vi.fn(() => `blob:test-${++seq}`),
    revokeObjectURL: vi.fn((u: string) => {
      revoked.push(u)
    }),
  })
})

describe('PDFJsEngine cache key', () => {
  it('1バイト差の文書は別キーになる', () => {
    const k1 = (PDFJsEngine as unknown as { computeHashKey: (d: Uint8Array) => string }).computeHashKey(
      new Uint8Array([1, 2, 3]),
    )
    const k2 = (PDFJsEngine as unknown as { computeHashKey: (d: Uint8Array) => string }).computeHashKey(
      new Uint8Array([1, 2, 4]),
    )
    expect(k1).not.toBe(k2)
    // 64bitハッシュは16桁hex
    expect(k1.split('_')[1].length).toBeLessThanOrEqual(16)
  })

  it('空文書は固定キー', () => {
    const k = (PDFJsEngine as unknown as { computeHashKey: (d: Uint8Array) => string }).computeHashKey(
      new Uint8Array([]),
    )
    expect(k).toBe('empty_0')
  })
})

describe('DefaultRenderer blob URL lifetime', () => {
  it('返却したURLをfinallyでrevokeしない（revoke-then-return禁止）', async () => {
    invokeMock.mockResolvedValueOnce([137, 80, 78, 71])
    const url = await defaultRenderer.renderPageToUrl({ pdfData: [1, 2, 3], pageIndex: 0, dpi: 72 })
    expect(url).toMatch(/^blob:/)
    expect(revoked).not.toContain(url)
  })

  it('cancelAll後は新規レンダリングが継続できる', async () => {
    invokeMock.mockResolvedValueOnce([1, 2, 3, 4])
    defaultRenderer.cancelAll()
    const url = await defaultRenderer.renderPageToUrl({ pdfData: [5, 6], pageIndex: 0, dpi: 72 })
    expect(url).toMatch(/^blob:/)
    expect(revoked).not.toContain(url)
  })
})
