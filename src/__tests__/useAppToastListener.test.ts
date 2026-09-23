import { renderHook, act } from '@testing-library/react'
import { useAppToastListener } from '../hooks/useAppToastListener'
import { describe, it, expect, vi } from 'vitest'

describe('useAppToastListener', () => {
  it('app-toast イベントを受信すると showToast を呼ぶ', () => {
    const showToast = vi.fn()
    renderHook(() => useAppToastListener(showToast))

    act(() => {
      window.dispatchEvent(new CustomEvent('app-toast', {
        detail: { message: 'テスト通知', type: 'warning' },
      }))
    })

    expect(showToast).toHaveBeenCalledTimes(1)
    expect(showToast).toHaveBeenCalledWith('テスト通知', 'warning')
  })

  it('type 未指定なら info として扱う', () => {
    const showToast = vi.fn()
    renderHook(() => useAppToastListener(showToast))

    act(() => {
      window.dispatchEvent(new CustomEvent('app-toast', {
        detail: { message: 'タイプなし' },
      }))
    })

    expect(showToast).toHaveBeenCalledWith('タイプなし', 'info')
  })

  it('message が空または欠落している場合は showToast を呼ばない', () => {
    const showToast = vi.fn()
    renderHook(() => useAppToastListener(showToast))

    act(() => {
      window.dispatchEvent(new CustomEvent('app-toast', { detail: { message: '' } }))
      window.dispatchEvent(new CustomEvent('app-toast', { detail: {} }))
      window.dispatchEvent(new CustomEvent('app-toast'))
    })

    expect(showToast).not.toHaveBeenCalled()
  })

  it('unmount 後はイベントを購読解除し showToast を呼ばない', () => {
    const showToast = vi.fn()
    const { unmount } = renderHook(() => useAppToastListener(showToast))

    unmount()

    act(() => {
      window.dispatchEvent(new CustomEvent('app-toast', {
        detail: { message: 'unmount後' },
      }))
    })

    expect(showToast).not.toHaveBeenCalled()
  })
})
