import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { SecurityPanel } from '../components/SecurityPanel'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }))

const exec = vi.fn(async () => undefined)

const identity = {
  sha1_fingerprint: 'DC69A5644F16C00A098A47132B05EB04FA9412E0',
  common_name: 'Apple Development: dev@example.com (3XML8MHG6L)',
  nickname: '3XML8MHG6L',
}

beforeEach(() => {
  invokeMock.mockReset()
  exec.mockClear()
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'list_keychain_identities') return Promise.resolve([identity])
    if (cmd === 'sign_pdf_with_keychain') return Promise.resolve([7, 7, 7])
    return Promise.resolve(null)
  })
})

function renderPanel() {
  return render(<SecurityPanel exec={exec} showToast={() => {}} pdfData={[1, 2, 3]} onPdfUpdate={vi.fn()} />)
}

describe('SecurityPanel signature placement', () => {
  it('既定の配置（2ページ目相当なし: 1ページ目 50,50 200x60）を全署名コマンドへ渡す', async () => {
    renderPanel()
    await screen.findByRole('option', { name: identity.common_name })
    fireEvent.click(screen.getByRole('button', { name: 'キーチェーンで署名' }))

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      'sign_pdf_with_keychain',
      expect.objectContaining({ pageIndex: 0, x: 50, y: 50, width: 200, height: 60 }),
    ))
  })

  it('配置UIの変更が署名コマンドに反映される（ページは0始まり変換・小数は切り捨て）', async () => {
    renderPanel()
    await screen.findByRole('option', { name: identity.common_name })

    fireEvent.change(screen.getByLabelText('ページ (1始まり)'), { target: { value: '3' } })
    fireEvent.change(screen.getByLabelText('X (pt)'), { target: { value: '120.7' } })
    fireEvent.change(screen.getByLabelText('Y (pt)'), { target: { value: '350' } })
    fireEvent.change(screen.getByLabelText('幅 (pt)'), { target: { value: '180' } })
    fireEvent.change(screen.getByLabelText('高さ (pt)'), { target: { value: '45' } })

    fireEvent.click(screen.getByRole('button', { name: 'キーチェーンで署名' }))

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      'sign_pdf_with_keychain',
      expect.objectContaining({ pageIndex: 2, x: 120, y: 350, width: 180, height: 45 }),
    ))
  })

  it('不正な数値入力（空欄・0）は安全な下限にクランプされる', async () => {
    renderPanel()
    await screen.findByRole('option', { name: identity.common_name })

    fireEvent.change(screen.getByLabelText('ページ (1始まり)'), { target: { value: '' } })
    fireEvent.change(screen.getByLabelText('X (pt)'), { target: { value: '-10' } })
    fireEvent.change(screen.getByLabelText('幅 (pt)'), { target: { value: '1' } })

    fireEvent.click(screen.getByRole('button', { name: 'キーチェーンで署名' }))

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      'sign_pdf_with_keychain',
      expect.objectContaining({ pageIndex: 0, x: 0, width: 40, height: 60 }),
    ))
  })

  it('署名フィールド枠の配置も同じ設定を共有する', async () => {
    renderPanel()
    await screen.findByRole('option', { name: identity.common_name })

    fireEvent.change(screen.getByLabelText('Y (pt)'), { target: { value: '700' } })
    fireEvent.change(screen.getByPlaceholderText('署名予定者名 (例: Taro Yamada)'), { target: { value: 'Taro' } })
    fireEvent.click(screen.getByRole('button', { name: '署名フィールド枠を追加' }))

    expect(exec).toHaveBeenCalledWith('add_digital_signature', expect.objectContaining({
      pageIndex: 0, x: 50, y: 700, width: 200, height: 60, signerName: 'Taro',
    }))
  })
})
