import { describe, it, expect } from 'vitest'
import { formatBytes } from '../utils/format'

describe('formatBytes', () => {
  it(' falsyな値は全角ダッシュを返す', () => {
    expect(formatBytes(0)).toBe('—')
    expect(formatBytes(undefined)).toBe('—')
    expect(formatBytes(null)).toBe('—')
  })

  it('1KB未満はバイト表示', () => {
    expect(formatBytes(512)).toBe('512 B')
    expect(formatBytes(1023)).toBe('1023 B')
  })

  it('1MB未満はKB表示（小数1桁）', () => {
    expect(formatBytes(1024)).toBe('1.0 KB')
    expect(formatBytes(1536)).toBe('1.5 KB')
  })

  it('1MB以上はMB表示（小数1桁）', () => {
    expect(formatBytes(1024 * 1024)).toBe('1.0 MB')
    expect(formatBytes(2.5 * 1024 * 1024)).toBe('2.5 MB')
  })
})
