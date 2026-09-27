import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { SecurityPanel } from '../components/SecurityPanel'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }))

const exec = vi.fn(async () => undefined)

beforeEach(() => {
  invokeMock.mockReset()
  exec.mockClear()
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'list_keychain_identities') return Promise.resolve([])
    if (cmd === 'add_document_timestamp') return Promise.resolve([9, 9, 9])
    if (cmd === 'verify_document_timestamp') {
      return Promise.resolve({ valid: true, timestamp: '2026-09-27T03:00:00Z', authority: 'Test TSA', hash: 'ab'.repeat(32) })
    }
    return Promise.resolve(null)
  })
})

function renderPanel(onPdfUpdate = vi.fn()) {
  render(<SecurityPanel exec={exec} showToast={() => {}} pdfData={[1, 2, 3]} onPdfUpdate={onPdfUpdate} />)
  return onPdfUpdate
}

describe('SecurityPanel PAdES B-T document timestamp', () => {
  it('TSA URLが無い状態では付与ボタンが無効（偽タイムスタンプを生成しない）', () => {
    renderPanel()
    expect(screen.getByRole('button', { name: '文書タイムスタンプを付与' })).toBeDisabled()
  })

  it('TSA URL入力後は add_document_timestamp が呼び出され、結果が文書に反映される', async () => {
    const onPdfUpdate = renderPanel()
    fireEvent.change(screen.getByPlaceholderText('TSA URL (必須・例: https://timestamp.digicert.com)'), {
      target: { value: 'https://timestamp.digicert.com' },
    })
    fireEvent.click(screen.getByRole('button', { name: '文書タイムスタンプを付与' }))

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      'add_document_timestamp',
      expect.objectContaining({ tsaUrl: 'https://timestamp.digicert.com' }),
    ))
    await waitFor(() => expect(onPdfUpdate).toHaveBeenCalledWith([9, 9, 9]))
  })

  it('検証ボタンは verify_document_timestamp を呼び、有効結果をインライン表示する', async () => {
    renderPanel()
    fireEvent.click(screen.getByRole('button', { name: 'タイムスタンプを検証（ByteRange照合）' }))

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('verify_document_timestamp', { data: [1, 2, 3] }))
    await screen.findByText(/暗号的に有効/)
  })
})
