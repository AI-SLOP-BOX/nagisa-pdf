import { describe, it, expect } from 'vitest'
import { pdfBoxToVisual, visualBoxToPdf, visualPointToPdf, unrotatedSize } from '../utils/rotation'

// A4非回転寸法 W=595 H=842。箱 (100,500,60,20)（PDF左下原点）。
// 期待値は表示変換の角点追跡による独立導出:
// 90°: (ux,uy)->視覚(uy,W-ux) / 180°: (W-ux,H-uy)...ではなく底层で検証済みの
// 往復式。ここでは往復不変＋具体値の両方を固定する。
const pageW = 595
const pageH = 842
const box = { x: 100, y: 500, width: 60, height: 20 }

describe('pdfBoxToVisual', () => {
  it('回転0はy反転のみ', () => {
    expect(pdfBoxToVisual(box, pageW, pageH, 0)).toEqual({ x: 100, y: 322, width: 60, height: 20 })
  })

  it('回転90は軸入替', () => {
    expect(pdfBoxToVisual(box, pageW, pageH, 90)).toEqual({ x: 500, y: 100, width: 20, height: 60 })
  })

  it('回転180は左右反転', () => {
    expect(pdfBoxToVisual(box, pageW, pageH, 180)).toEqual({ x: 435, y: 500, width: 60, height: 20 })
  })

  it('回転270は軸入替', () => {
    expect(pdfBoxToVisual(box, pageW, pageH, 270)).toEqual({ x: 322, y: 435, width: 20, height: 60 })
  })
})

describe('visualBoxToPdf', () => {
  it('回転0はy反転のみ', () => {
    expect(visualBoxToPdf({ x: 100, y: 322, width: 60, height: 20 }, pageW, pageH, 0)).toEqual(box)
  })

  it('回転90/180/270の具体値', () => {
    expect(visualBoxToPdf({ x: 500, y: 100, width: 20, height: 60 }, pageW, pageH, 90)).toEqual(box)
    expect(visualBoxToPdf({ x: 435, y: 500, width: 60, height: 20 }, pageW, pageH, 180)).toEqual(box)
    expect(visualBoxToPdf({ x: 322, y: 435, width: 20, height: 60 }, pageW, pageH, 270)).toEqual(box)
  })
})

describe('往復不変', () => {
  it('全回転で pdf->visual->pdf が恒等', () => {
    const cases = [
      box,
      { x: 0, y: 0, width: 10, height: 10 },
      { x: 530, y: 800, width: 60, height: 30 },
    ]
    for (const rot of [0, 90, 180, 270, 360, -90]) {
      for (const b of cases) {
        const v = pdfBoxToVisual(b, pageW, pageH, rot)
        const back = visualBoxToPdf(v, pageW, pageH, rot)
        expect(back.x).toBeCloseTo(b.x, 5)
        expect(back.y).toBeCloseTo(b.y, 5)
        expect(back.width).toBeCloseTo(b.width, 5)
        expect(back.height).toBeCloseTo(b.height, 5)
      }
    }
  })

  it('面積保存・非負', () => {
    for (const rot of [0, 90, 180, 270]) {
      const v = pdfBoxToVisual(box, pageW, pageH, rot)
      expect(v.width * v.height).toBe(box.width * box.height)
      expect(v.width).toBeGreaterThanOrEqual(0)
      expect(v.height).toBeGreaterThanOrEqual(0)
    }
  })
})

describe('visualPointToPdf', () => {
  it('箱の対応角と一致する', () => {
    // pdfBoxToVisual の出力箱の左下角（視覚左下原点）を点写像し、
    // 元箱の対応角に戻ることを確認する（箱テストの具体値と連動）。
    const cases: Array<{ rot: number; vx: number; vy: number; ux: number; uy: number }> = [
      { rot: 0, vx: 100, vy: 500, ux: 100, uy: 500 },
      { rot: 90, vx: 500, vy: 435, ux: 160, uy: 500 },
      { rot: 180, vx: 435, vy: 322, ux: 160, uy: 520 },
      { rot: 270, vx: 322, vy: 100, ux: 100, uy: 520 },
    ]
    for (const c of cases) {
      expect(visualPointToPdf(c.vx, c.vy, pageW, pageH, c.rot)).toEqual({ x: c.ux, y: c.uy })
    }
  })

  it('角4点の全数往復', () => {
    // 視覚箱の4隅すべてが非回転箱の4隅に bijection で戻る。
    // 視覚箱は pdfBoxToVisual の出力（各回転の具体値は箱テストと同一）。
    const visualBoxes: Record<number, { x: number; y: number; width: number; height: number }> = {
      0: { x: 100, y: 322, width: 60, height: 20 },
      90: { x: 500, y: 100, width: 20, height: 60 },
      180: { x: 435, y: 500, width: 60, height: 20 },
      270: { x: 322, y: 435, width: 20, height: 60 },
    }
    // 非回転箱の4隅集合（順序不同で一致すればよい）
    const srcCorners = new Set(['100,500', '160,500', '100,520', '160,520'])
    for (const rot of [0, 90, 180, 270]) {
      const vb = visualBoxes[rot]
      // 視覚左上原点 -> 左下原点へ変換して4隅を点写像
      const vH = rot === 90 || rot === 270 ? pageW : pageH
      const corners = [
        { x: vb.x, y: vb.y },
        { x: vb.x + vb.width, y: vb.y },
        { x: vb.x, y: vb.y + vb.height },
        { x: vb.x + vb.width, y: vb.y + vb.height },
      ]
      const got = new Set(
        corners.map(c => {
          // 左上原点を左下原点へ（y反転）
          const p = visualPointToPdf(c.x, vH - c.y, pageW, pageH, rot)
          return `${Math.round(p.x)},${Math.round(p.y)}`
        }),
      )
      expect(got).toEqual(srcCorners)
    }
  })
})

describe('unrotatedSize', () => {
  it('90/270で入替・他は同一', () => {
    expect(unrotatedSize(842, 595, 90)).toEqual({ width: 595, height: 842 })
    expect(unrotatedSize(842, 595, 270)).toEqual({ width: 595, height: 842 })
    expect(unrotatedSize(595, 842, 0)).toEqual({ width: 595, height: 842 })
    expect(unrotatedSize(595, 842, 180)).toEqual({ width: 595, height: 842 })
  })
})
