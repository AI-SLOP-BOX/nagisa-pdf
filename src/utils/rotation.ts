/**
 * 回転ページ座標ユーティリティ（純粋関数）。
 *
 * PDF内部空間: 非回転・左下原点 (x右, y上)。
 * 視覚空間: /Rotate適用後の表示・左上原点DOM (x右, y下)。
 * レンダラ（pdftoppm / PDF.js getViewport）は回転適用済み画素を返すため、
 * オーバーレイとPNG画素は視覚空間で一致する。pageW/pageH は常に
 * 非回転MediaBox寸法で渡すこと（視覚寸法ではない）。
 *
 * 導出は角点の表示変換の逆写像。90/270°では軸が入れ替わるため
 * width/height も入れ替える。回転0では恒等＋y反転のみ。
 */

export interface Box {
  x: number
  y: number
  width: number
  height: number
}

function normRot(rotation: number): number {
  return ((rotation % 360) + 360) % 360
}

/**
 * PDF内部箱（左下原点・非回転）→ 視覚DOM箱（左上原点・回転表示）。
 * バックエンドのテキストブロック表示用。
 */
export function pdfBoxToVisual(box: Box, pageW: number, pageH: number, rotation: number): Box {
  const rot = normRot(rotation)
  if (rot === 90) return { x: box.y, y: box.x, width: box.height, height: box.width }
  if (rot === 180) return { x: pageW - box.x - box.width, y: box.y, width: box.width, height: box.height }
  if (rot === 270) {
    return {
      x: pageH - box.y - box.height,
      y: pageW - box.x - box.width,
      width: box.height,
      height: box.width,
    }
  }
  return { x: box.x, y: pageH - box.y - box.height, width: box.width, height: box.height }
}

/**
 * 視覚空間（左下原点）の点 → PDF内部点（左下原点・非回転）。
 * ドラッグ移動先などの点座標のバックエンド投入用。pageW/pageH は非回転寸法。
 */
export function visualPointToPdf(
  px: number,
  py: number,
  pageW: number,
  pageH: number,
  rotation: number,
): { x: number; y: number } {
  const rot = normRot(rotation)
  if (rot === 90) return { x: pageW - py, y: px }
  if (rot === 180) return { x: pageW - px, y: pageH - py }
  if (rot === 270) return { x: py, y: pageH - px }
  return { x: px, y: py }
}

/**
 * 視覚寸法＋回転角から非回転MediaBox寸法を復元する。
 */
export function unrotatedSize(
  visualW: number,
  visualH: number,
  rotation: number,
): { width: number; height: number } {
  const rot = normRot(rotation)
  if (rot === 90 || rot === 270) return { width: visualH, height: visualW }
  return { width: visualW, height: visualH }
}

/**
 * 視覚DOM箱（左上原点・回転表示）→ PDF内部箱（左下原点・非回転）。
 * オーバーレイ描画矩形のバックエンド投入用。pageW/pageH は非回転寸法。
 * annotationService の mapAnnotationToPdf と同形（相互の往復テストで担保）。
 */
export function visualBoxToPdf(box: Box, pageW: number, pageH: number, rotation: number): Box {
  const rot = normRot(rotation)
  if (rot === 90) return { x: box.y, y: box.x, width: box.height, height: box.width }
  if (rot === 180) {
    return { x: pageW - box.x - box.width, y: box.y, width: box.width, height: box.height }
  }
  if (rot === 270) {
    return {
      x: pageW - box.y - box.height,
      y: pageH - box.x - box.width,
      width: box.height,
      height: box.width,
    }
  }
  return { x: box.x, y: pageH - box.y - box.height, width: box.width, height: box.height }
}
