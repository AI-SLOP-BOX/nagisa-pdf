import { hexToRgb } from '../services/annotationService'
import { describe, it, expect } from 'vitest'

describe('hexToRgb', () => {
  it('通常の6桁hexをRGBに変換する', () => {
    expect(hexToRgb('#ff0000')).toEqual({ r: 1, g: 0, b: 0 })
    expect(hexToRgb('#00ff00')).toEqual({ r: 0, g: 1, b: 0 })
    expect(hexToRgb('#0000ff')).toEqual({ r: 0, g: 0, b: 1 })
    expect(hexToRgb('#ffffff')).toEqual({ r: 1, g: 1, b: 1 })
    expect(hexToRgb('#000000')).toEqual({ r: 0, g: 0, b: 0 })
  })

  it('16進数の大文字小文字を区別しない', () => {
    expect(hexToRgb('#FF0000')).toEqual({ r: 1, g: 0, b: 0 })
    expect(hexToRgb('#ff0000')).toEqual({ r: 1, g: 0, b: 0 })
  })

  it('3桁hexを6桁に展開して変換する', () => {
    expect(hexToRgb('#f00')).toEqual({ r: 1, g: 0, b: 0 })
    expect(hexToRgb('#0f0')).toEqual({ r: 0, g: 1, b: 0 })
    expect(hexToRgb('#00f')).toEqual({ r: 0, g: 0, b: 1 })
    expect(hexToRgb('#fff')).toEqual({ r: 1, g: 1, b: 1 })
    expect(hexToRgb('#1a2')).toEqual({ r: 1 / 15, g: 10 / 15, b: 2 / 15 })
  })

  it('ハッシュ記号なしでも動作する', () => {
    expect(hexToRgb('ff0000')).toEqual({ r: 1, g: 0, b: 0 })
  })

  it('NaNの場合はデフォルトの濃い紺色を返す', () => {
    expect(hexToRgb('#zzzzzz')).toEqual({ r: 0.06, g: 0.09, b: 0.16 })
    expect(hexToRgb('#gggggg')).toEqual({ r: 0.06, g: 0.09, b: 0.16 })
    expect(hexToRgb('#')).toEqual({ r: 0.06, g: 0.09, b: 0.16 })
  })

  it('値は正規化されていて0〜1の範囲', () => {
    const c = hexToRgb('#808080')
    expect(c.r).toBeCloseTo(0.502, 2)
    expect(c.g).toBeCloseTo(0.502, 2)
    expect(c.b).toBeCloseTo(0.502, 2)
    expect(c.r >= 0 && c.r <= 1).toBe(true)
    expect(c.g >= 0 && c.g <= 1).toBe(true)
    expect(c.b >= 0 && c.b <= 1).toBe(true)
  })
})
