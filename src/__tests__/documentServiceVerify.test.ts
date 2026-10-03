import { describe, it, expect, vi, beforeEach } from 'vitest'
import { DocumentService } from '../services/documentService'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }))

beforeEach(() => {
  invokeMock.mockReset()
})

describe('DocumentService.verifySignatures error honesty', () => {
  it('バックエンド検証失敗は「署名なし」に偽装せず例外を透過する (session)', async () => {
    invokeMock.mockRejectedValueOnce(new Error('session gone'))
    await expect(DocumentService.verifySignatures('doc-1')).rejects.toThrow('session gone')
  })

  it('バックエンド検証失敗は「署名なし」に偽装せず例外を透過する (direct bytes)', async () => {
    invokeMock.mockRejectedValueOnce(new Error('ipc broken'))
    await expect(DocumentService.verifySignatures([1, 2, 3])).rejects.toThrow('ipc broken')
  })

  it('正常時は署名リストをそのまま返す', async () => {
    invokeMock
      .mockResolvedValueOnce({ signatures: [], count: 0 })
      .mockResolvedValueOnce(null)
    await expect(DocumentService.verifySignatures('doc-1')).resolves.toEqual({ signatures: [], count: 0 })
  })
})
