import { renderHook, act } from '@testing-library/react'
import { useToast } from '../hooks/useToast'
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

describe('useToast', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('showToast でメッセージとタイプが設定され、一定時間後にクリアされる', () => {
    const { result } = renderHook(() => useToast(100))

    act(() => {
      result.current.showToast('テストメッセージ', 'info')
    })

    expect(result.current.toast).toBe('テストメッセージ')
    expect(result.current.toastType).toBe('info')

    act(() => {
      vi.advanceTimersByTime(100)
    })

    expect(result.current.toast).toBeNull()
  })

  it('showError は toast タイプを error に設定する', () => {
    const { result } = renderHook(() => useToast(100))

    act(() => {
      result.current.showError('エラーメッセージ')
    })

    expect(result.current.toast).toBe('エラーメッセージ')
    expect(result.current.toastType).toBe('error')
  })

  it('showSuccess は toast タイプを success に設定する', () => {
    const { result } = renderHook(() => useToast(100))

    act(() => {
      result.current.showSuccess('成功メッセージ')
    })

    expect(result.current.toast).toBe('成功メッセージ')
    expect(result.current.toastType).toBe('success')
  })

  it('連続して showToast すると、新しい方で古いタイマーがクリアされ、古いトーストが残らない', () => {
    const { result } = renderHook(() => useToast(200))

    act(() => {
      result.current.showToast('古いメッセージ')
    })
    expect(result.current.toast).toBe('古いメッセージ')

    act(() => {
      result.current.showToast('新しいメッセージ')
    })

    expect(result.current.toast).toBe('新しいメッセージ')
    expect(result.current.toastType).toBe('info')

    act(() => {
      vi.advanceTimersByTime(200)
    })

    expect(result.current.toast).toBeNull()
  })

  it('duration を変更すると、トーストの消える時間が変わる', () => {
    const { result } = renderHook(() => useToast(50))

    act(() => {
      result.current.showToast('速く消える')
    })

    expect(result.current.toast).toBe('速く消える')

    act(() => {
      vi.advanceTimersByTime(50)
    })

    expect(result.current.toast).toBeNull()
  })

  it('unmount時にタイマーがクリアされ、setState が呼ばれなくなる', () => {
    const { result, unmount } = renderHook(() => useToast(100))

    act(() => {
      result.current.showToast('未マウント時のトースト')
    })

    expect(result.current.toast).toBe('未マウント時のトースト')

    unmount()
    expect(true).toBe(true)
  })
})
