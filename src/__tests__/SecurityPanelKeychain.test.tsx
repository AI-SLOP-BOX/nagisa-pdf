import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { SecurityPanel } from '../components/SecurityPanel'

const { invokeMock, openMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openMock: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock, save: vi.fn() }))

const exec = vi.fn(async () => undefined)

const identity = {
  sha1_fingerprint: 'DC69A5644F16C00A098A47132B05EB04FA9412E0',
  common_name: 'Apple Development: dev@example.com (3XML8MHG6L)',
  nickname: '3XML8MHG6L',
}

const token = {
  slot_id: 4,
  description: 'YubiKey',
  manufacturer: 'Yubico',
  token_label: 'Nagisa Token',
  token_serial: '0123456789',
  certificates: [{
    certificate_id: 'a1b2c3',
    label: 'Signing Certificate',
    subject: 'CN=Test Signer',
    issuer: 'CN=Test CA',
    serial_number: '01',
    sha256_fingerprint: 'AA:BB',
  }],
}

beforeEach(() => {
  invokeMock.mockReset()
  openMock.mockReset()
  exec.mockClear()
})

describe('SecurityPanel keychain signing', () => {
  it('起動時にキーチェーンの証明書を読み込み、既定で選択する', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_keychain_identities') return Promise.resolve([identity])
      return Promise.resolve(null)
    })

    render(<SecurityPanel exec={exec} showToast={() => {}} pdfData={[1, 2, 3]} />)

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('list_keychain_identities'))
    await screen.findByRole('option', { name: identity.common_name })
    expect(screen.getByRole('button', { name: 'キーチェーンで署名' })).toBeInTheDocument()
  })

  it('キーチェーンで署名すると秘密鍵を使わずに sign_pdf_with_keychain を呼ぶ', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_keychain_identities') return Promise.resolve([identity])
      if (cmd === 'sign_pdf_with_keychain') return Promise.resolve([7, 7, 7])
      return Promise.resolve(null)
    })
    const onPdfUpdate = vi.fn()
    const showToast = vi.fn()

    render(
      <SecurityPanel
        exec={exec}
        showToast={showToast}
        pdfData={[1, 2, 3]}
        onPdfUpdate={onPdfUpdate}
      />
    )

    await screen.findByRole('option', { name: identity.common_name })
    fireEvent.click(screen.getByRole('button', { name: 'キーチェーンで署名' }))

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      'sign_pdf_with_keychain',
      expect.objectContaining({
        data: [1, 2, 3],
        identityNickname: '3XML8MHG6L',
        tsaUrl: null,
      }),
    ))
    await waitFor(() => expect(onPdfUpdate).toHaveBeenCalledWith([7, 7, 7]))
    expect(showToast).toHaveBeenCalledWith(
      'キーチェーンの証明書で署名しました（秘密鍵は外部に抽出されません）',
    )
    expect(openMock).not.toHaveBeenCalled()
  })

  it('PKCS#11トークンを検出し、選択した証明書とPINでHSM署名を呼ぶ', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_keychain_identities') return Promise.resolve([identity])
      if (cmd === 'list_pkcs11_slots') return Promise.resolve([token])
      if (cmd === 'sign_pdf_with_pkcs11') return Promise.resolve([9, 9, 9])
      return Promise.resolve(null)
    })
    const onPdfUpdate = vi.fn()
    render(<SecurityPanel exec={exec} showToast={() => {}} pdfData={[1, 2, 3]} onPdfUpdate={onPdfUpdate} />)
    await screen.findByRole('option', { name: identity.common_name })

    fireEvent.click(screen.getByRole('button', { name: 'PKCS#11' }))
    await screen.findByLabelText('PKCS#11トークン')
    expect(screen.getByLabelText('PKCS#11署名証明書')).toHaveValue('a1b2c3')

    const pin = screen.getByLabelText('PKCS#11 PIN')
    fireEvent.change(pin, { target: { value: '1234' } })
    fireEvent.click(screen.getByRole('button', { name: 'PKCS#11 / HSMで署名' }))

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      'sign_pdf_with_pkcs11',
      expect.objectContaining({
        data: [1, 2, 3], slotId: 4, certificateId: 'a1b2c3', pin: '1234', tsaUrl: null,
      }),
    ))
    await waitFor(() => expect(onPdfUpdate).toHaveBeenCalledWith([9, 9, 9]))
    expect(pin).toHaveValue('')
  })

  it('PKCS#12 ファイル方式へ切り替えるとファイル選択UIが出る', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_keychain_identities') return Promise.resolve([identity])
      return Promise.resolve(null)
    })

    render(<SecurityPanel exec={exec} showToast={() => {}} pdfData={[1]} />)

    await screen.findByRole('option', { name: identity.common_name })
    fireEvent.click(screen.getByRole('button', { name: 'PKCS#12 ファイル' }))

    expect(screen.getByRole('button', { name: '📁 証明書ファイル (.p12/.pfx) を選択' })).toBeInTheDocument()
    const signBtn = screen.getByRole('button', { name: '証明書ファイルで署名' })
    expect(signBtn).toBeInTheDocument()
    expect(signBtn).toBeDisabled()
  })

  it('証明書が0件でもUIは例外なく描画され、署名ボタンを無効化する', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_keychain_identities') return Promise.resolve([])
      return Promise.resolve(null)
    })
    const showToast = vi.fn()

    render(<SecurityPanel exec={exec} showToast={showToast} pdfData={[1]} />)

    await screen.findByRole('option', { name: '証明書がありません' })
    expect(screen.getByRole('button', { name: 'キーチェーンで署名' })).toBeDisabled()
    expect(showToast).toHaveBeenCalledWith('キーチェーンに署名用証明書が見つかりません')
  })

  it('署名エラー時は busy 状態から復帰し、UI が固まらない', async () => {
    // NOTE: this runner fails a test as soon as a rejecting Promise is created,
    // so the backend failure is surfaced synchronously to exercise the same
    // try/catch + finally path in the component.
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_keychain_identities') return Promise.resolve([identity])
      if (cmd === 'sign_pdf_with_keychain') throw 'backend refused the identity'
      return Promise.resolve(null)
    })
    const showToast = vi.fn()

    render(<SecurityPanel exec={exec} showToast={showToast} pdfData={[1]} />)

    await screen.findByRole('option', { name: identity.common_name })
    fireEvent.click(screen.getByRole('button', { name: 'キーチェーンで署名' }))

    await waitFor(() => expect(showToast).toHaveBeenCalledWith(
      'キーチェーン署名エラー: backend refused the identity',
    ))
    await waitFor(() => expect(screen.getByRole('button', { name: 'キーチェーンで署名' })).toBeEnabled())
  })
})
