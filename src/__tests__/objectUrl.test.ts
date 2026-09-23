import { safeRevokeObjectUrl } from '../utils/objectUrl'
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

describe('safeRevokeObjectUrl', () => {
  let revokeSpy: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    revokeSpy = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})
  })

  afterEach(() => {
    revokeSpy.mockRestore()
  })

  it('blob URL を渡すと URL.revokeObjectURL を呼ぶ', () => {
    safeRevokeObjectUrl('blob:http://localhost/abc-123')
    expect(revokeSpy).toHaveBeenCalledTimes(1)
    expect(revokeSpy).toHaveBeenCalledWith('blob:http://localhost/abc-123')
  })

  it('null を渡しても例外を投げず、revoke しない', () => {
    expect(() => safeRevokeObjectUrl(null)).not.toThrow()
    expect(revokeSpy).not.toHaveBeenCalled()
  })

  it('undefined を渡しても例外を投げず、revoke しない', () => {
    expect(() => safeRevokeObjectUrl(undefined)).not.toThrow()
    expect(revokeSpy).not.toHaveBeenCalled()
  })

  it('空文字を渡しても例外を投げず、revoke しない', () => {
    expect(() => safeRevokeObjectUrl('')).not.toThrow()
    expect(revokeSpy).not.toHaveBeenCalled()
  })

  it('revokeObjectURL 自体が throw しても外へは伝播しない', () => {
    revokeSpy.mockImplementation(() => {
      throw new Error('revoke failed')
    })
    expect(() => safeRevokeObjectUrl('blob:http://localhost/boom')).not.toThrow()
  })
})
