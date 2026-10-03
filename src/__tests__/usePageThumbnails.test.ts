import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { usePageThumbnails } from '../hooks/usePageThumbnails'

const { renderMock } = vi.hoisted(() => ({ renderMock: vi.fn() }))
vi.mock('../services/thumbnailService', () => ({
  renderPageThumbnail: renderMock,
  detectTotalPages: vi.fn(async () => null),
}))

const revoked: string[] = []

beforeEach(() => {
  renderMock.mockReset()
  revoked.length = 0
  vi.stubGlobal('URL', {
    createObjectURL: vi.fn(() => 'blob:x'),
    revokeObjectURL: vi.fn((u: string) => {
      revoked.push(u)
    }),
  })
  vi.stubGlobal('IntersectionObserver', undefined)
})

describe('usePageThumbnails', () => {
  it('requestPageで描画しmapに格納する', async () => {
    renderMock.mockResolvedValueOnce({ url: 'data:image/0', isBlob: false })
    // 注意: pdfDataは安定参照で渡す。インライン配列は毎レンダーで
    // doc-change effectを再発火させ無限ループになる。
    const props = { pdfData: [1, 2, 3], pageCount: 3 }
    const { result } = renderHook(() => usePageThumbnails(props))

    await act(async () => {
      result.current.requestPage(1)
    })
    expect(renderMock).toHaveBeenCalledTimes(1)
    expect(result.current.thumbnails.get(1)).toBe('data:image/0')
  })

  it('同一ページの重複要求は1回のみ', async () => {
    renderMock.mockResolvedValue({ url: 'data:image/1', isBlob: false })
    const props = { pdfData: [1], pageCount: 2 }
    const { result } = renderHook(() => usePageThumbnails(props))

    await act(async () => {
      result.current.requestPage(0)
      result.current.requestPage(0)
    })
    expect(renderMock).toHaveBeenCalledTimes(1)
  })

  it('blob URLは置換時にrevokeされる', async () => {
    renderMock
      .mockResolvedValueOnce({ url: 'blob:old', isBlob: true })
      .mockResolvedValueOnce({ url: 'blob:new', isBlob: true })
    const pdfA = [1]
    const pdfB = [2]
    const { result, rerender } = renderHook(
      ({ pdfData }) => usePageThumbnails({ pdfData, pageCount: 1 }),
      { initialProps: { pdfData: pdfA } },
    )

    await act(async () => {
      result.current.requestPage(0)
    })
    expect(result.current.thumbnails.get(0)).toBe('blob:old')

    rerender({ pdfData: pdfB })
    await act(async () => {
      result.current.requestPage(0)
    })
    expect(result.current.thumbnails.get(0)).toBe('blob:new')
    expect(revoked).toContain('blob:old')
    expect(revoked).not.toContain('blob:new')
  })

  it('失敗ページは再要求しない', async () => {
    renderMock.mockResolvedValue(null)
    const props = { pdfData: [1], pageCount: 1 }
    const { result } = renderHook(() => usePageThumbnails(props))

    await act(async () => {
      result.current.requestPage(0)
    })
    await act(async () => {
      result.current.requestPage(0)
    })
    expect(renderMock).toHaveBeenCalledTimes(1)
    expect(result.current.thumbnails.has(0)).toBe(false)
  })

  it('失敗から30秒後は再要求する', async () => {
    vi.useFakeTimers()
    try {
      renderMock.mockResolvedValue(null)
      const props = { pdfData: [1], pageCount: 1 }
      const { result } = renderHook(() => usePageThumbnails(props))

      await act(async () => {
        result.current.requestPage(0)
      })
      expect(renderMock).toHaveBeenCalledTimes(1)

      vi.setSystemTime(Date.now() + 31 * 1000)
      renderMock.mockResolvedValue({ url: 'data:retry', isBlob: false })
      await act(async () => {
        result.current.requestPage(0)
      })
      expect(renderMock).toHaveBeenCalledTimes(2)
      expect(result.current.thumbnails.get(0)).toBe('data:retry')
    } finally {
      vi.useRealTimers()
    }
  })
})
