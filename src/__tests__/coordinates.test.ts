import { describe, it, expect } from 'vitest'
import { renderHook } from '@testing-library/react'
import { usePDFCoordinates } from '../hooks/usePDFCoordinates'

const baseParams: Parameters<typeof usePDFCoordinates>[0] = {
  imgRenderedSize: { width: 595, height: 842 },
  pageSize: { width: 595, height: 842 },
  zoom: 1,
  interactiveMode: 'view',
  textBlocks: [],
  draggingBlockId: null,
  setDraggingBlockId: () => {},
  tempBlockPos: null,
  setTempBlockPos: () => {},
  blockDragOffset: { x: 0, y: 0 },
  drawBox: null,
  setDrawBox: () => {},
  currentPage: 0,
}

/**
 * TextBlock の座標契約を固定する回帰テスト。
 * TextBlock.y は「ページ左下が原点・y 上向き」の PDF ユーザー空間。
 * UserAnnotation.y / OCRDetectedBlock.y（上原点）と混ざらないこと。
 */
describe('usePDFCoordinates 座標変換契約 (TextBlock = 下原点)', () => {
  it('pdfToDom は PDF下原点 (y 上向き) を DOM上原点 (y 下向き) へ変換する', () => {
    const { result } = renderHook(() => usePDFCoordinates(baseParams))
    // PDF y=700（上寄り）・高さ20 → DOM top = 842 - (700+20) = 122
    const dom = result.current.pdfToDom(100, 700, 50, 20, 842)
    expect(dom).toEqual({ left: 100, top: 122, width: 50, height: 20 })
  })

  it('domToPdf は pdfToDom の厳密な逆変換になる（往復不変）', () => {
    const { result } = renderHook(() => usePDFCoordinates(baseParams))
    const dom = result.current.pdfToDom(100, 700, 50, 20, 842)
    const back = result.current.domToPdf(dom.left, dom.top, dom.width, dom.height, 842)
    expect(back).toEqual({ x: 100, y: 700, width: 50, height: 20 })
  })

  it('PDF下端寄りのブロックはDOMでは画面上部、上端寄りは下部に描画される', () => {
    const { result } = renderHook(() => usePDFCoordinates(baseParams))
    const nearBottom = result.current.pdfToDom(0, 10, 100, 20, 842) // PDF y=10 ≒ 下端
    const nearTop = result.current.pdfToDom(0, 800, 100, 20, 842) // PDF y=800 ≒ 上端
    expect(nearBottom.top).toBeGreaterThan(nearTop.top)
    expect(nearBottom.top).toBe(842 - (10 + 20))
    expect(nearTop.top).toBe(842 - (800 + 20))
  })
})