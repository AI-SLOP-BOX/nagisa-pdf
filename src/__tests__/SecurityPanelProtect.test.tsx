import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { SecurityPanel } from '../components/SecurityPanel'

const { invokeMock, saveMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  saveMock: vi.fn(async () => '/tmp/protected.pdf' as string | null),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: saveMock }))

const exec = vi.fn(async () => undefined)

beforeEach(() => {
  invokeMock.mockReset()
  saveMock.mockClear()
  exec.mockClear()
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'list_keychain_identities') return Promise.resolve([])
    if (cmd === 'protect_pdf') return Promise.resolve([1, 2, 3, 4])
    if (cmd === 'write_file_bytes') return Promise.resolve(null)
    return Promise.resolve(null)
  })
})

describe('SecurityPanel AES-256 password protection', () => {
  it('パスワード未入力では書き出しボタンが無効', () => {
    render(<SecurityPanel exec={exec} showToast={() => {}} pdfData={[1, 2, 3]} onPdfUpdate={vi.fn()} />)
    expect(screen.getByRole('button', { name: '暗号化コピーを書き出す' })).toBeDisabled()
  })

  it('パスワード入力後は protect_pdf → 保存先へ write_file_bytes が出力される', async () => {
    render(<SecurityPanel exec={exec} showToast={() => {}} pdfData={[1, 2, 3]} onPdfUpdate={vi.fn()} />)
    fireEvent.change(screen.getByPlaceholderText('開くためのパスワード（必須）'), { target: { value: 's3cret' } })
    fireEvent.click(screen.getByRole('button', { name: '暗号化コピーを書き出す' }))

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('protect_pdf', { data: [1, 2, 3], password: 's3cret' }),
    )
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('write_file_bytes', {
        path: '/tmp/protected.pdf',
        data: [1, 2, 3, 4],
      }),
    )
  })
})
