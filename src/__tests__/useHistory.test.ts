import { renderHook, act } from '@testing-library/react'
import { useHistory } from '../hooks/useHistory'
import { describe, it, expect } from 'vitest'

describe('useHistory', () => {
  it('初期データがあれば過去履歴に含まれ、現在のデータもその値になる', () => {
    const { result } = renderHook(() => useHistory([1, 2, 3], 30))
    expect(result.current.data).toEqual([1, 2, 3])
    expect(result.current.history).toEqual([[1, 2, 3]])
    expect(result.current.historyIndex).toBe(0)
    expect(result.current.canUndo).toBe(false)
    expect(result.current.canRedo).toBe(false)
  })

  it('初期データなしの場合、dataはnullで履歴も空', () => {
    const { result } = renderHook(() => useHistory(null, 30))
    expect(result.current.data).toBeNull()
    expect(result.current.history).toEqual([])
    expect(result.current.historyIndex).toBe(-1)
    expect(result.current.canUndo).toBe(false)
    expect(result.current.canRedo).toBe(false)
  })

  it('pushHistory で新しいデータが追加され、現在のdataになり、undoで戻れる', () => {
    const { result } = renderHook(() => useHistory([1], 30))
    expect(result.current.data).toEqual([1])

    act(() => {
      result.current.pushHistory([2, 3])
    })
    expect(result.current.data).toEqual([2, 3])
    expect(result.current.history).toEqual([
      [1],
      [2, 3],
    ])
    expect(result.current.historyIndex).toBe(1)
    expect(result.current.canUndo).toBe(true)
    expect(result.current.canRedo).toBe(false)

    act(() => {
      result.current.undo()
    })
    expect(result.current.data).toEqual([1])
    expect(result.current.historyIndex).toBe(0)
    expect(result.current.canRedo).toBe(true)
  })

  it('redoするとundoした先の状態に戻る', () => {
    const { result } = renderHook(() => useHistory([1], 30))

    act(() => {
      result.current.pushHistory([2])
    })
    act(() => {
      result.current.pushHistory([3])
    })
    act(() => {
      result.current.undo()
    })
    act(() => {
      result.current.undo()
    })
    expect(result.current.data).toEqual([1])

    act(() => {
      result.current.redo()
    })
    expect(result.current.data).toEqual([2])
    act(() => {
      result.current.redo()
    })
    expect(result.current.data).toEqual([3])
    expect(result.current.canRedo).toBe(false)
  })

  it('resetHistory で履歴が初期化され、新しいデータが現在のデータになる', () => {
    const { result } = renderHook(() => useHistory([1], 30))

    act(() => {
      result.current.pushHistory([2])
    })
    act(() => {
      result.current.pushHistory([3])
    })
    expect(result.current.history.length).toBe(3)

    act(() => {
      result.current.resetHistory([4, 5])
    })
    expect(result.current.data).toEqual([4, 5])
    expect(result.current.history).toEqual([
      [4, 5],
    ])
    expect(result.current.historyIndex).toBe(0)
    expect(result.current.canUndo).toBe(false)
    expect(result.current.canRedo).toBe(false)
  })

  it('履歴はmaxHistoryを超えると古いものから消える', () => {
    const { result } = renderHook(() => useHistory([1], 3))

    act(() => {
      result.current.pushHistory([2])
    })
    act(() => {
      result.current.pushHistory([3])
    })
    act(() => {
      result.current.pushHistory([4])
    })
    expect(result.current.history).toEqual([
      [2],
      [3],
      [4],
    ])
    expect(result.current.history.length).toBe(3)
    expect(result.current.data).toEqual([4])
  })

  it('pushHistory の際に past の未来の履歴は破棄される（redo が無効になる）', () => {
    const { result } = renderHook(() => useHistory([1], 30))

    act(() => {
      result.current.pushHistory([2])
    })
    act(() => {
      result.current.undo()
    })
    expect(result.current.canRedo).toBe(true)

    act(() => {
      result.current.pushHistory([3])
    })
    expect(result.current.canRedo).toBe(false)
    expect(result.current.data).toEqual([3])
  })

  it('canUndo / canRedo は適切に更新される', () => {
    const { result } = renderHook(() => useHistory([1], 30))
    expect(result.current.canUndo).toBe(false)
    expect(result.current.canRedo).toBe(false)

    act(() => {
      result.current.pushHistory([2])
    })
    expect(result.current.canUndo).toBe(true)
    expect(result.current.canRedo).toBe(false)

    act(() => {
      result.current.undo()
    })
    expect(result.current.canUndo).toBe(false)
    expect(result.current.canRedo).toBe(true)

    act(() => {
      result.current.redo()
    })
    expect(result.current.canUndo).toBe(true)
    expect(result.current.canRedo).toBe(false)
  })
})
