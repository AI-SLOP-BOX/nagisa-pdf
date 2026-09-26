import type { PDFDocument as PDFDocumentType, PDFFont } from 'pdf-lib'
import { safeRevokeObjectUrl } from '../utils/objectUrl'
import { notifyWarning } from '../utils/notify'

export interface UserAnnotation {
  id: string
  page: number
  type: 'text' | 'highlight' | 'shape' | 'note'
  // COORDINATE CONTRACT: PDF points (72 DPI) measured from the page's
  // TOP-LEFT corner (y grows downward, DOM/CSS-like) — NOT standard PDF
  // user space. `AnnotationService.applyAnnotations` flips y at burn time
  // (`pageH - y - height`). Backend commands (add_highlight / redact_area /
  // add_rectangle) expect bottom-left PDF user space: convert with
  // `domToPdf` first. Contrasts with `TextBlock.y` (documentService),
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

      // Map DOM coordinates (top-down, visual orientation) to PDF internal rotated space
      // pdf-lib draw commands are relative to page's unrotated MediaBox
      let targetX = ann.x
      let targetY = pageH - ann.y - ann.height
      let drawRotation = 0

      if (rotationAngle === 90) {
        // Rotated 90 deg clockwise in viewer
        targetX = ann.y
        targetY = ann.x
        drawRotation = 90
      } else if (rotationAngle === 180) {
        targetX = pageW - ann.x - ann.width
        targetY = ann.y
        drawRotation = 180
      } else if (rotationAngle === 270) {
        targetX = pageW - ann.y - ann.height
        targetY = pageH - ann.x - ann.width
        drawRotation = 270
      }

      if (ann.type === 'text' && ann.text) {
        const c = hexToRgb(ann.color || '#0f172a')
        const fSize = ann.fontSize || 16
        const lines = ann.text.split('\n')
        const pdfBaselineY = rotationAngle === 0 ? (pageH - ann.y - (fSize * 0.82) - 2) : targetY

        // If user specified an explicit background color (e.g. text label with colored background), draw it
        if (ann.bgColor) {
          const maxLineChars = Math.max(...lines.map(l => l.length), 1)
          const estimatedTextW = maxLineChars * fSize * 1.05
          const eraseW = Math.max(ann.width, estimatedTextW, 40)
          const totalTextH = Math.max(ann.height, lines.length * fSize * 1.25)
          const bgRgb = hexToRgb(ann.bgColor)

          page.drawRectangle({
            x: Math.max(0, targetX),
            y: Math.max(0, targetY),
            width: eraseW,
            height: totalTextH,
            color: rgb(bgRgb.r, bgRgb.g, bgRgb.b),
            rotate: drawRotation !== 0 ? degrees(drawRotation) : undefined,
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
            // CJK font failed to load: log warning and notify
            console.error('CJK font (ipaexg.ttf / ipaexm.ttf) is unavailable. Falling back safely.')
            notifyWarning('日本語フォントの読み込みに失敗したため、一部の文字を代替表示します。')
          }
        } else {
          if (isMincho) {
            activeFont = ann.isBold ? fontTimesBold : fontTimes
          } else {
            activeFont = ann.isBold ? fontBold : fontRegular
          }
        }

        // Clean multi-line strings
        const rotDeg = ann.rotation || 0
        lines.forEach((line, lineIdx) => {
          let textToDraw = line
          // If non-ASCII exists but no CJK font is available, strip/replace characters that Standard 14 fonts cannot encode
          if (hasNonAscii && !cjkAvailable) {
            // Replace non-ASCII chars with '?' to avoid WinAnsi cannot encode character exception
            textToDraw = line.replace(/[^\u0000-\u007f]/g, '?')
          }

          try {
            page.drawText(textToDraw, {
              x: ann.x,
              y: Math.max(0, pdfBaselineY - lineIdx * (fSize * 1.2)),
              size: fSize,
              font: activeFont,
              color: rgb(c.r, c.g, c.b),
              rotate: rotDeg !== 0 ? degrees(rotDeg) : undefined,
            })
          } catch (drawErr) {
            console.warn('drawText fallback:', drawErr)
            // Last-resort fallback: try drawing sanitized ASCII text
            try {
              page.drawText(textToDraw.replace(/[^\u0020-\u007e]/g, '?'), {
                x: ann.x,
                y: Math.max(0, pdfBaselineY - lineIdx * (fSize * 1.2)),
                size: fSize,
                font: fontRegular,
                color: rgb(c.r, c.g, c.b),
                rotate: rotDeg !== 0 ? degrees(rotDeg) : undefined,
              })
            } catch (fatalErr) {
              console.error('Fatal drawText failed:', fatalErr)
            }
          }
        })
      } else if (ann.type === 'highlight') {
        const pdfY = pageH - ann.y - ann.height
        page.drawRectangle({
          x: ann.x,
          y: Math.max(0, pdfY),
          width: ann.width,
          height: ann.height,
          color: rgb(1, 0.95, 0.2),
          opacity: 0.45,
        })
      } else if (ann.type === 'shape') {
        const pdfY = pageH - ann.y - ann.height
        page.drawRectangle({
          x: ann.x,
          y: Math.max(0, pdfY),
          width: ann.width,
          height: ann.height,
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
