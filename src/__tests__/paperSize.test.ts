import { describe, it, expect } from 'vitest'
import { standardPaperName, formatPaperSize } from '../utils/paperSize'

describe('paperSize', () => {
  it('A4 の縦横を認識する', () => {
    expect(standardPaperName(595.28, 841.89)).toBe('A4')
    expect(standardPaperName(841.89, 595.28)).toBe('A4') // landscape
  })

  it('Letter / Legal を認識する', () => {
    expect(standardPaperName(612, 792)).toBe('Letter')
    expect(standardPaperName(612, 1008)).toBe('Legal')
  })

  it('非標準サイズは null を返す', () => {
    expect(standardPaperName(500, 700)).toBeNull()
  })

  it('formatPaperSize は mm 表記に用紙名を付ける', () => {
    expect(formatPaperSize({ width: 595.28, height: 841.89 })).toBe('A4 (210 x 297 mm)')
  })

  it('寸法が不明なら「—」を返す', () => {
    expect(formatPaperSize(null)).toBe('—')
    expect(formatPaperSize({ width: 0, height: 0 })).toBe('—')
  })
})