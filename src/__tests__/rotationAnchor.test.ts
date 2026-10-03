import { describe, it, expect } from 'vitest'
import { PDFDocument, StandardFonts, degrees } from 'pdf-lib'
import * as pdfjsLib from 'pdfjs-dist'
import { pdfBoxToVisual, visualBoxToPdf } from '../utils/rotation'

const W = 595
const H = 842
// 非回転ユーザー空間の基準点（テキスト Tm 原点に相当）
const UX = 100
const UY = 500

async function viewportPoint(rot: 0 | 90 | 180 | 270): Promise<{ vx: number; vy: number }> {
  const pdf = await PDFDocument.create()
  const page = pdf.addPage([W, H])
  if (rot !== 0) page.setRotation(degrees(rot as 90 | 180 | 270))
  const font = await pdf.embedFont(StandardFonts.Helvetica)
  page.drawText('Hi', { x: UX, y: UY, size: 12, font })
  const bytes = await pdf.save()
  const doc = await pdfjsLib.getDocument({ data: new Uint8Array(bytes), isEvalSupported: false })
    .promise
  try {
    const pg = await doc.getPage(1)
    const vp = pg.getViewport({ scale: 1 })
    const [vx, vy] = vp.convertToViewportPoint(UX, UY)
    return { vx, vy }
  } finally {
    await doc.destroy()
  }
}

async function viewportBox(
  rot: 0 | 90 | 180 | 270,
): Promise<{ x: number; y: number; width: number; height: number }> {
  const pdf = await PDFDocument.create()
  const page = pdf.addPage([W, H])
  if (rot !== 0) page.setRotation(degrees(rot as 90 | 180 | 270))
  const bytes = await pdf.save()
  const doc = await pdfjsLib.getDocument({ data: new Uint8Array(bytes), isEvalSupported: false })
    .promise
  try {
    const pg = await doc.getPage(1)
    const vp = pg.getViewport({ scale: 1 })
    // 非回転箱 (100,500,60,20) の4隅を視覚へ写し、外接矩形を取る
    const corners: Array<[number, number]> = [
      [100, 500],
      [160, 500],
      [100, 520],
      [160, 520],
    ]
    const mapped = corners.map(([x, y]) => vp.convertToViewportPoint(x, y))
    const xs = mapped.map(p => p[0])
    const ys = mapped.map(p => p[1])
    const x0 = Math.min(...xs)
    const y0 = Math.min(...ys)
    return { x: x0, y: y0, width: Math.max(...xs) - x0, height: Math.max(...ys) - y0 }
  } finally {
    await doc.destroy()
  }
}

describe('rotation vs pdf.js viewport (external anchor)', () => {
  for (const rot of [0, 90, 180, 270] as const) {
    it(`回転${rot}°: 非回転点の視覚位置がpdf.jsと一致する`, async () => {
      const { vx, vy } = await viewportPoint(rot)
      // 自前写像（退化箱=点）と比較。視覚は左上原点。
      const mine = pdfBoxToVisual({ x: UX, y: UY, width: 0, height: 0 }, W, H, rot)
      expect(mine.x).toBeCloseTo(vx, 3)
      expect(mine.y).toBeCloseTo(vy, 3)
    }, 30000)

    it(`回転${rot}°: 視覚点の逆写像がpdf.jsと一致する`, async () => {
      const { vx, vy } = await viewportPoint(rot)
      const mine = visualBoxToPdf({ x: vx, y: vy, width: 0, height: 0 }, W, H, rot)
      expect(mine.x).toBeCloseTo(UX, 3)
      expect(mine.y).toBeCloseTo(UY, 3)
    }, 30000)

    it(`回転${rot}°: 非ゼロ箱の原点・寸法がpdf.jsと一致する`, async () => {
      // w/h入替を含む箱全体の外部一致（退化箱の点検証だけでは見えない部分）
      const actual = await viewportBox(rot)
      const mine = pdfBoxToVisual({ x: 100, y: 500, width: 60, height: 20 }, W, H, rot)
      expect(mine.x).toBeCloseTo(actual.x, 3)
      expect(mine.y).toBeCloseTo(actual.y, 3)
      expect(mine.width).toBeCloseTo(actual.width, 3)
      expect(mine.height).toBeCloseTo(actual.height, 3)
    }, 30000)
  }
})
