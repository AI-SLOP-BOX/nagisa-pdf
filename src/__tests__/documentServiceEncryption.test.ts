import { describe, it, expect, vi, beforeEach } from 'vitest'
import { DocumentService } from '../services/documentService'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }))

beforeEach(() => {
  invokeMock.mockReset()
})

describe('DocumentService password-protection wiring', () => {
  it('isEncrypted は is_pdf_encrypted をそのまま委譲する', async () => {
    invokeMock.mockResolvedValueOnce(true)
    await expect(DocumentService.isEncrypted([1, 2, 3])).resolves.toBe(true)
    expect(invokeMock).toHaveBeenCalledWith('is_pdf_encrypted', { data: [1, 2, 3] })
  })

  it('decryptPdf は静文を返し、誤パスワードのエラーを透過する', async () => {
    invokeMock.mockResolvedValueOnce([9, 9, 9])
    await expect(DocumentService.decryptPdf([1, 2, 3], 'pw')).resolves.toEqual([9, 9, 9])
    expect(invokeMock).toHaveBeenCalledWith('decrypt_pdf', { data: [1, 2, 3], password: 'pw' })

    invokeMock.mockRejectedValueOnce('パスワードが正しくありません')
    await expect(DocumentService.decryptPdf([1, 2, 3], 'bad')).rejects.toBe('パスワードが正しくありません')
  })

  it('開錠フローの契約: 暗号化検出 → 解錠 → 再検出で平文になる', async () => {
    invokeMock
      .mockResolvedValueOnce(true) // 1st: encrypted
      .mockResolvedValueOnce([1, 2, 3, 4]) // decrypt_pdf
      .mockResolvedValueOnce(false) // re-probe the decrypted bytes
    expect(await DocumentService.isEncrypted([7, 7, 7])).toBe(true)
    const plain = await DocumentService.decryptPdf([7, 7, 7], 'pw')
    expect(await DocumentService.isEncrypted(plain)).toBe(false)
  })
})
