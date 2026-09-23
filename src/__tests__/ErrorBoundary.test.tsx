import { render, screen, fireEvent } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { ErrorBoundary } from '../components/ErrorBoundary'

let shouldThrow = false
function MaybeBoom() {
  if (shouldThrow) throw new Error('テスト用エラー')
  return <div>正常コンテンツ</div>
}

describe('ErrorBoundary', () => {
  beforeEach(() => {
    shouldThrow = false
  })

  it('エラーがなければ子をそのまま表示する', () => {
    render(<ErrorBoundary><MaybeBoom /></ErrorBoundary>)
    expect(screen.getByText('正常コンテンツ')).toBeInTheDocument()
  })

  it('子のレンダリングエラーを捕捉して回復UIを表示する', () => {
    shouldThrow = true
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
    render(<ErrorBoundary><MaybeBoom /></ErrorBoundary>)
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(screen.getByText(/予期しないエラーが発生しました/)).toBeInTheDocument()
    expect(screen.getByText(/テスト用エラー/)).toBeInTheDocument()
    spy.mockRestore()
  })

  it('onError コールバックがエラー情報と共に呼ばれる', () => {
    shouldThrow = true
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
    const onError = vi.fn()
    render(<ErrorBoundary onError={onError}><MaybeBoom /></ErrorBoundary>)
    expect(onError).toHaveBeenCalledTimes(1)
    expect(onError.mock.calls[0][0].message).toBe('テスト用エラー')
    spy.mockRestore()
  })

  it('カスタム fallback を表示できる', () => {
    shouldThrow = true
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
    render(
      <ErrorBoundary fallback={(err) => <div>カスタム: {err.message}</div>}>
        <MaybeBoom />
      </ErrorBoundary>
    )
    expect(screen.getByText(/カスタム: テスト用エラー/)).toBeInTheDocument()
    spy.mockRestore()
  })

  it('resetKey が変わるとエラー状態をクリアして再レンダリングする', () => {
    shouldThrow = true
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { rerender } = render(
      <ErrorBoundary resetKey="a"><MaybeBoom /></ErrorBoundary>
    )
    expect(screen.getByRole('alert')).toBeInTheDocument()

    // 障害が解消した状態で resetKey を変更すると復帰する
    shouldThrow = false
    rerender(<ErrorBoundary resetKey="b"><MaybeBoom /></ErrorBoundary>)
    expect(screen.getByText('正常コンテンツ')).toBeInTheDocument()
    spy.mockRestore()
  })

  it('「再試行」ボタンでエラー状態をリセットできる', () => {
    shouldThrow = true
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
    render(<ErrorBoundary><MaybeBoom /></ErrorBoundary>)
    expect(screen.getByRole('alert')).toBeInTheDocument()

    shouldThrow = false
    fireEvent.click(screen.getByRole('button', { name: '再試行' }))
    expect(screen.getByText('正常コンテンツ')).toBeInTheDocument()
    spy.mockRestore()
  })
})
