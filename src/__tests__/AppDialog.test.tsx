import { render, screen, fireEvent } from '@testing-library/react'
import { describe, it, expect, vi } from 'vitest'
import { ConfirmDialog, InputDialog } from '../components/AppDialog'

describe('ConfirmDialog', () => {
  it('isOpen=false では描画しない', () => {
    const { container } = render(
      <ConfirmDialog isOpen={false} title="t" message="m" onConfirm={() => {}} onCancel={() => {}} />
    )
    expect(container.firstChild).toBeNull()
  })

  it('タイトル/メッセージを表示し、確認で onConfirm を呼ぶ', () => {
    const onConfirm = vi.fn()
    render(
      <ConfirmDialog isOpen title="ページを削除" message="本当に？" confirmLabel="削除" onConfirm={onConfirm} onCancel={() => {}} />
    )
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByText('本当に？')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '削除' }))
    expect(onConfirm).toHaveBeenCalledTimes(1)
  })

  it('キャンセルボタンと Escape で onCancel を呼ぶ', () => {
    const onCancel = vi.fn()
    render(<ConfirmDialog isOpen title="t" message="m" onConfirm={() => {}} onCancel={onCancel} />)
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }))
    expect(onCancel).toHaveBeenCalledTimes(1)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledTimes(2)
  })
})

describe('InputDialog', () => {
  it('入力値を onSubmit へ渡す', () => {
    const onSubmit = vi.fn()
    render(<InputDialog isOpen title="t" onSubmit={onSubmit} onCancel={() => {}} />)
    const input = screen.getByRole('textbox')
    fireEvent.change(input, { target: { value: 'hello' } })
    fireEvent.click(screen.getByRole('button', { name: 'OK' }))
    expect(onSubmit).toHaveBeenCalledWith('hello')
  })

  it('multiline で textarea になり Ctrl+Enter で確定する', () => {
    const onSubmit = vi.fn()
    render(<InputDialog isOpen title="t" multiline initialValue="code" onSubmit={onSubmit} onCancel={() => {}} />)
    const ta = screen.getByRole('textbox') as HTMLTextAreaElement
    expect(ta.tagName).toBe('TEXTAREA')
    fireEvent.keyDown(ta, { key: 'Enter', ctrlKey: true })
    expect(onSubmit).toHaveBeenCalledWith('code')
  })

  it('inputType=password で入力がマスクされる', () => {
    render(<InputDialog isOpen title="t" inputType="password" onSubmit={() => {}} onCancel={() => {}} />)
    expect(document.querySelector('input[type="password"]')).toBeInTheDocument()
  })

  it('isOpen=false では描画しない', () => {
    const { container } = render(
      <InputDialog isOpen={false} title="t" onSubmit={() => {}} onCancel={() => {}} />
    )
    expect(container.firstChild).toBeNull()
  })
})
