import { describe, it, expect } from 'vitest'
import { mapAnnotationToPdf } from '../services/annotationService'

// A4想定: pageW=595, pageH=842。ann={x:100,y:50,w:60,h:20}。
// 期待値は /Rotate の表示変換（90/270=CW、原点の逆回転＋90/270で寸法入替）
// からの独立導出。旧実装は原点のみ一致し、寸法入替とanchor回転が誤りだった。
const ann = { x: 100, y: 50, width: 60, height: 20 }
const pageW = 595
const pageH = 842

describe('mapAnnotationToPdf', () => {
  it('回転0: 従来式と同一 (x, pageH-y-h)', () => {
    expect(mapAnnotationToPdf(ann, pageW, pageH, 0)).toEqual({
      x: 100,
      y: 842 - 50 - 20,
      width: 60,
      height: 20,
    })
  })

  it('回転90: 原点(ay,ax)・寸法入替', () => {
    expect(mapAnnotationToPdf(ann, pageW, pageH, 90)).toEqual({
      x: 50,
      y: 100,
      width: 20,
      height: 60,
    })
  })

  it('回転180: 左右反転', () => {
    expect(mapAnnotationToPdf(ann, pageW, pageH, 180)).toEqual({
      x: 595 - 100 - 60,
      y: 50,
      width: 60,
      height: 20,
    })
  })

  it('回転270: 原点・寸法入替', () => {
    expect(mapAnnotationToPdf(ann, pageW, pageH, 270)).toEqual({
      x: 595 - 50 - 20,
      y: 842 - 100 - 60,
      width: 20,
      height: 60,
    })
  })

  it('360/負角は正規化される', () => {
    expect(mapAnnotationToPdf(ann, pageW, pageH, 360)).toEqual(
      mapAnnotationToPdf(ann, pageW, pageH, 0),
    )
    expect(mapAnnotationToPdf(ann, pageW, pageH, -90)).toEqual(
      mapAnnotationToPdf(ann, pageW, pageH, 270),
    )
  })

  it('面積は回転で不変・ページ内に収まる', () => {
    const area = ann.width * ann.height
    for (const rot of [0, 90, 180, 270]) {
      const m = mapAnnotationToPdf(ann, pageW, pageH, rot)
      expect(m.width * m.height).toBe(area)
      expect(m.x).toBeGreaterThanOrEqual(0)
      expect(m.y).toBeGreaterThanOrEqual(0)
      expect(m.x + m.width).toBeLessThanOrEqual(pageW + 1e-6)
      expect(m.y + m.height).toBeLessThanOrEqual(pageH + 1e-6)
    }
  })

  it('角4点が逆写像で復元する（非正方形ページ）', () => {
    // 逆写像（非回転左下原点 → 視覚左上原点）。各回転の表示変換:
    // 0°: (ux,uy)->(ux,H-uy) / 90°: (ux,uy)->(uy,ux) /
    // 180°: (ux,uy)->(W-ux,uy) / 270°: (ux,uy)->(H-uy,W-ux)
    // （視覚寸法は 0/180°でWxH、90/270°でHxW）
    const inverse = (ux: number, uy: number, rot: number): { x: number; y: number } => {
      if (rot === 90) return { x: uy, y: ux }
      if (rot === 180) return { x: pageW - ux, y: uy }
      if (rot === 270) return { x: pageH - uy, y: pageW - ux }
      return { x: ux, y: pageH - uy }
    }
    // 視覚箱の4隅（DOM左上原点）
    const corners = [
      { x: ann.x, y: ann.y },
      { x: ann.x + ann.width, y: ann.y },
      { x: ann.x, y: ann.y + ann.height },
      { x: ann.x + ann.width, y: ann.y + ann.height },
    ]
    for (const rot of [0, 90, 180, 270]) {
      for (const c of corners) {
        // 点写像（w=h=0）で順方向へ
        const fwd = mapAnnotationToPdf({ x: c.x, y: c.y, width: 0, height: 0 }, pageW, pageH, rot)
        const back = inverse(fwd.x, fwd.y, rot)
        expect(back.x).toBeCloseTo(c.x, 5)
        expect(back.y).toBeCloseTo(c.y, 5)
      }
    }
  })
})
