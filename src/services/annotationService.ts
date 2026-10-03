import type { PDFDocument as PDFDocumentType, PDFFont } from 'pdf-lib'
import { safeRevokeObjectUrl } from '../utils/objectUrl'
import { visualBoxToPdf } from '../utils/rotation'

export interface UserAnnotation {
  id: string
  page: number
  type: 'text' | 'highlight' | 'shape' | 'note'
  // COORDINATE CONTRACT: PDF points (72 DPI) measured from the page's
  // TOP-LEFT corner (y grows downward, DOM/CSS-like) — NOT standard PDF
  // user space. `AnnotationService.applyAnnotations` maps to unrotated
  // user space at burn time via `mapAnnotationToPdf` (= rotation-aware
  // `visualBoxToPdf`). Backend commands expect bottom-left PDF user space.
  // Contrasts with `TextBlock.y` (documentService),
  // which IS bottom-origin PDF user space.
  x: number
  y: number
  width: number
  height: number
  text?: string
  fontSize?: number
  fontFamily?: string
  color?: string
  bgColor?: string
  isBold?: boolean
  isItalic?: boolean
  rotation?: number // in degrees: 0, 90, 180, 270
}

/**
 * DOM座標（左上原点・視覚方向＝/Rotate適用後の見た目）→ PDF内部の
 * 非回転MediaBox座標（左下原点）へ変換する。
 * pdf-lib の描画は非回転 MediaBox 基準のため、/Rotate 付きページでは
 * 角点の逆回転写像が必要。90/270°では軸が入れ替わるため width/height も
 * 入れ替える。戻り値は軸平行ボックスであり、描画側で pdf-lib の rotate
 * を付けてはならない（anchor回転はボックスをずらす）。
 *
 * 前提: DOMが視覚回転済み空間であること。バックエンドPNGが回転適用済みかは
 * 別途Viewer側の責務（未検証）。回転0では従来式と完全一致する。
 */
export interface MappedAnnotationBox {
  x: number
  y: number
  width: number
  height: number
}

export function mapAnnotationToPdf(
  ann: Pick<UserAnnotation, 'x' | 'y' | 'width' | 'height'>,
  pageW: number,
  pageH: number,
  rotationAngle: number,
): MappedAnnotationBox {
  // 実体は utils/rotation の visualBoxToPdf（角点逆写像・往復テスト済み）。
  // この別名は既存呼出との互換のために残す。
  return visualBoxToPdf(ann, pageW, pageH, rotationAngle)
}

export function hexToRgb(hex: string): { r: number; g: number; b: number } {
  let clean = hex.replace('#', '')
  if (clean.length === 3) {
    clean = clean.split('').map(c => c + c).join('')
  }
  const num = parseInt(clean, 16)
  if (isNaN(num)) return { r: 0.06, g: 0.09, b: 0.16 }
  return {
    r: ((num >> 16) & 255) / 255,
    g: ((num >> 8) & 255) / 255,
    b: (num & 255) / 255,
  }
}

// Cache embedded font buffers in memory (Promiseキャッシュで並行呼び出し時の重複fetchを防止)
let gothicBytesPromise: Promise<ArrayBuffer | null> | null = null
let minchoBytesPromise: Promise<ArrayBuffer | null> | null = null

async function getCjkGothicBytes(): Promise<ArrayBuffer | null> {
  if (gothicBytesPromise) return gothicBytesPromise
  gothicBytesPromise = (async () => {
    try {
      const res = await fetch('/ipaexg.ttf')
      if (res.ok) return await res.arrayBuffer()
    } catch (e) {
      console.warn('Could not load /ipaexg.ttf from local public assets:', e)
    }
    return null
  })()
  return gothicBytesPromise
}

async function getCjkMinchoBytes(): Promise<ArrayBuffer | null> {
  if (minchoBytesPromise) return minchoBytesPromise
  minchoBytesPromise = (async () => {
    try {
      const res = await fetch('/ipaexm.ttf')
      if (res.ok) return await res.arrayBuffer()
    } catch (e) {
      console.warn('Could not load /ipaexm.ttf from local public assets:', e)
    }
    return null
  })()
  return minchoBytesPromise
}

export class AnnotationService {
  /**
   * Burn annotations directly into PDF byte stream using pdf-lib with full CJK font support.
   */
  static async applyAnnotations(
    sourceBytes: number[] | Uint8Array,
    annotations: UserAnnotation[],
  ): Promise<Uint8Array> {
    // pdf-lib / fontkit は保存（注釈の焼き込み）時にしか使わないため動的 import とし、
    // アプリ起動時のバンドルサイズを削減する。
    const { PDFDocument, rgb, StandardFonts, degrees } = await import('pdf-lib')
    const fontkit = (await import('@pdf-lib/fontkit')).default

    const uint8 = sourceBytes instanceof Uint8Array ? sourceBytes : new Uint8Array(sourceBytes)
    let pdfDoc: PDFDocumentType
    try {
      pdfDoc = await PDFDocument.load(uint8, { ignoreEncryption: true })
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      throw new Error(`PDFDocument.load に失敗しました: ${msg}`)
    }
    pdfDoc.registerFontkit(fontkit)

    const pages = pdfDoc.getPages()
    const fontRegular = await pdfDoc.embedFont(StandardFonts.Helvetica)
    const fontBold = await pdfDoc.embedFont(StandardFonts.HelveticaBold)
    const fontTimes = await pdfDoc.embedFont(StandardFonts.TimesRoman)
    const fontTimesBold = await pdfDoc.embedFont(StandardFonts.TimesRomanBold)

    // Load CJK fonts for Japanese characters (Gothic & Mincho)
    let cjkGothicFont: PDFFont | null = null
    let cjkMinchoFont: PDFFont | null = null

    const [gothicBytes, minchoBytes] = await Promise.all([
      getCjkGothicBytes(),
      getCjkMinchoBytes(),
    ])

    if (gothicBytes) {
      try {
        cjkGothicFont = await pdfDoc.embedFont(gothicBytes)
      } catch (err) {
        console.warn('Failed to embed CJK Gothic font:', err)
      }
    }

    if (minchoBytes) {
      try {
        cjkMinchoFont = await pdfDoc.embedFont(minchoBytes)
      } catch (err) {
        console.warn('Failed to embed CJK Mincho font:', err)
      }
    }

    for (const ann of annotations) {
      if (ann.page < 0 || ann.page >= pages.length) continue
      const page = pages[ann.page]
      const { width: pageW, height: pageH } = page.getSize()
      const rotationAngle = (page.getRotation()?.angle || 0) % 360

      // Map DOM box (top-left, visually rotated space) to an axis-aligned box
      // in unrotated MediaBox space. No pdf-lib rotation is applied: the mapped
      // box already covers the intended region (rotation about the anchor would
      // displace it). Rotation 0 is identical to the legacy computation.
      const mapped = mapAnnotationToPdf(ann, pageW, pageH, rotationAngle)
      const targetX = mapped.x
      const targetY = mapped.y

      if (ann.type === 'text' && ann.text) {
        const c = hexToRgb(ann.color || '#0f172a')
        const fSize = ann.fontSize || 16
        const lines = ann.text.split('\n')
        // 視覚下方向の単位ベクトル（非回転座標系）とテキスト回転。
        // 回転0では従来式と一致する。
        const rot = ((rotationAngle % 360) + 360) % 360
        const downDx = rot === 90 ? 1 : rot === 270 ? -1 : 0
        const downDy = rot === 180 ? 1 : rot === 90 || rot === 270 ? 0 : -1
        const baseOff = fSize * 0.82 + 2
        // 回転ページ上のテキスト起点：視覚左上点を写像し視覚下へ1行分ずらす。
        // 回転0では (ann.x, pageH-ann.y-baseOff) で従来通り。
        const textPt = mapAnnotationToPdf(
          { x: ann.x, y: ann.y, width: 0, height: 0 },
          pageW,
          pageH,
          rotationAngle,
        )
        const anchorX = textPt.x + downDx * baseOff
        const anchorY = textPt.y + downDy * baseOff
        const textRot = (rot + (ann.rotation || 0)) % 360

        // If user specified an explicit background color (e.g. text label with colored background), draw it
        if (ann.bgColor) {
          const maxLineChars = Math.max(...lines.map(l => l.length), 1)
          const estimatedTextW = maxLineChars * fSize * 1.05
          const eraseW = Math.max(ann.width, estimatedTextW, 40)
          const totalTextH = Math.max(ann.height, lines.length * fSize * 1.25)
          const bgRgb = hexToRgb(ann.bgColor)
          // 消去矩形も視覚箱として写像する（回転0では従来値と同一）。
          const bgBox = rot === 0
            ? { x: targetX, y: targetY, width: eraseW, height: totalTextH }
            : mapAnnotationToPdf(
              { x: ann.x, y: ann.y, width: eraseW, height: totalTextH },
              pageW,
              pageH,
              rotationAngle,
            )

          page.drawRectangle({
            x: Math.max(0, bgBox.x),
            y: Math.max(0, bgBox.y),
            width: bgBox.width,
            height: bgBox.height,
            color: rgb(bgRgb.r, bgRgb.g, bgRgb.b),
          })
        }


        // Detect if text contains non-ASCII characters (Japanese kanji/kana)
        const hasNonAscii = /[^\u0000-\u007f]/.test(ann.text)
        const isMincho = (ann.fontFamily || '').toLowerCase().includes('mincho') || (ann.fontFamily || '').includes('明朝')
        const cjkAvailable = Boolean(cjkGothicFont || cjkMinchoFont)

        let activeFont: PDFFont = fontRegular
        if (hasNonAscii) {
          if (cjkGothicFont || cjkMinchoFont) {
            activeFont = isMincho ? (cjkMinchoFont ?? cjkGothicFont)! : (cjkGothicFont ?? cjkMinchoFont)!
          } else {
            // CJKフォント欠品時は '?' 置換の焼き込みを続行しない。破壊的書込みで
            // 文字化けPDFを生成するより、呼出側のcatchで明示エラーにする。
            throw new Error('日本語フォント (ipaexg.ttf / ipaexm.ttf) が読めないため焼き込みを中止しました')
          }
        } else {
          if (isMincho) {
            activeFont = ann.isBold ? fontTimesBold : fontTimes
          } else {
            activeFont = ann.isBold ? fontBold : fontRegular
          }
        }

        // Clean multi-line strings
        lines.forEach((line, lineIdx) => {
          let textToDraw = line
          // If non-ASCII exists but no CJK font is available, strip/replace characters that Standard 14 fonts cannot encode
          if (hasNonAscii && !cjkAvailable) {
            // Replace non-ASCII chars with '?' to avoid WinAnsi cannot encode character exception
            textToDraw = line.replace(/[^\u0000-\u007f]/g, '?')
          }
          // 行送りは視覚下方向へ。回転0では従来式 (x=ann.x, y=base-lineIdx*h) と一致。
          const lx = anchorX + downDx * lineIdx * (fSize * 1.2)
          const ly = anchorY + downDy * lineIdx * (fSize * 1.2)

          try {
            page.drawText(textToDraw, {
              x: lx,
              y: Math.max(0, ly),
              size: fSize,
              font: activeFont,
              color: rgb(c.r, c.g, c.b),
              rotate: textRot !== 0 ? degrees(textRot) : undefined,
            })
          } catch (drawErr) {
            console.warn('drawText fallback:', drawErr)
            // Last-resort fallback: try drawing sanitized ASCII text
            try {
              page.drawText(textToDraw.replace(/[^\u0020-\u007e]/g, '?'), {
                x: lx,
                y: Math.max(0, ly),
                size: fSize,
                font: fontRegular,
                color: rgb(c.r, c.g, c.b),
                rotate: textRot !== 0 ? degrees(textRot) : undefined,
              })
            } catch (fatalErr) {
              console.error('Fatal drawText failed:', fatalErr)
            }
          }
        })
      } else if (ann.type === 'highlight') {
        page.drawRectangle({
          x: targetX,
          y: Math.max(0, targetY),
          width: mapped.width,
          height: mapped.height,
          color: rgb(1, 0.95, 0.2),
          opacity: 0.45,
        })
      } else if (ann.type === 'shape') {
        page.drawRectangle({
          x: targetX,
          y: Math.max(0, targetY),
          width: mapped.width,
          height: mapped.height,
          borderColor: rgb(0.14, 0.39, 0.92),
          borderWidth: 2,
          color: rgb(0.93, 0.96, 1.0),
          opacity: 0.2,
        })
      }
    }

    let saved: Uint8Array
    try {
      saved = await pdfDoc.save()
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      throw new Error(`PDF の保存（save）に失敗しました: ${msg}`)
    }
    return saved
  }

  /**
   * Trigger local browser download of PDF bytes.
   */
  static downloadPdf(bytes: Uint8Array, filename: string) {
    const blob = new Blob([bytes as unknown as BlobPart], { type: 'application/pdf' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename.endsWith('.pdf') ? filename : `${filename}.pdf`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    setTimeout(() => safeRevokeObjectUrl(url), 1000)
  }
}
