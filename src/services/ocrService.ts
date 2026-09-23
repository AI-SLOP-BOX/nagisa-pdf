import { invoke } from '@tauri-apps/api/core'

export interface OCRDetectedBlock {
  id: string
  page: number
  text: string
  x: number // in PDF points (72 DPI)
  y: number
  width: number
  height: number
  fontSize: number
  confidence: number
}

interface NativeOCRLineBlock {
  text: string
  left: number
  top: number
  width: number
  height: number
  font_size: number
  confidence: number
}

/**
 * Intelligent client-side & native OCR text region detector that scans image canvas pixels
 * or invokes native Tesseract to find text lines, bounding boxes, and recognized strings.
 */
export class OCRService {
  /**
   * Detect text regions and strings from a rendered HTML Canvas element.
   */
  static async detectTextFromCanvas(
    canvas: HTMLCanvasElement,
    pageIndex: number,
    pdfWidth: number,
    pdfHeight: number
  ): Promise<OCRDetectedBlock[]> {
    const cWidth = canvas.width
    const cHeight = canvas.height
    if (cWidth === 0 || cHeight === 0) return []

    const scaleX = pdfWidth / cWidth
    const scaleY = pdfHeight / cHeight

    // 1. Try Native Tesseract Engine first via Tauri IPC
    try {
      const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, 'image/png'))
      if (blob) {
        const arrayBuf = await blob.arrayBuffer()
        const imageBytes = Array.from(new Uint8Array(arrayBuf))
        const nativeBlocks = await invoke<NativeOCRLineBlock[]>('ocr_image_blocks', {
          imageBytes,
          language: 'jpn+eng',
        })

        if (nativeBlocks && nativeBlocks.length > 0) {
          const meaningful = nativeBlocks.filter(b => b.text && b.text.trim().length > 0)
          if (meaningful.length > 0) {
            return meaningful.map((block, idx) => ({
              id: `ocr-line-${pageIndex}-${idx + 1}`,
              page: pageIndex,
              text: block.text.trim(),
              x: Math.round(block.left * scaleX),
              y: Math.round(block.top * scaleY),
              width: Math.max(30, Math.round(block.width * scaleX)),
              height: Math.max(16, Math.round(block.height * scaleY)),
              fontSize: Math.max(10, Math.round(block.font_size * scaleY)),
              confidence: Math.round(block.confidence),
            }))
          }
        }
      }
    } catch (err) {
      console.warn('Native Tesseract OCR invocation fallback to pixel projection:', err)
    }

    // 2. Fallback: Pixel-density Projection Detector
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (!ctx) return []

    const imgData = ctx.getImageData(0, 0, cWidth, cHeight)
    const { data } = imgData

    // 1. Calculate horizontal brightness projection to find text lines
    const lineDarkness = new Float32Array(cHeight)
    const rowStep = 4 // sample every 4 pixels horizontally for speed
    for (let y = 0; y < cHeight; y++) {
      let darkCount = 0
      const rowOffset = y * cWidth * 4
      for (let x = 0; x < cWidth; x += rowStep) {
        const idx = rowOffset + x * 4
        // Grayscale conversion
        const brightness = (data[idx] * 299 + data[idx + 1] * 587 + data[idx + 2] * 114) / 1000
        if (brightness < 180 && data[idx + 3] > 100) {
          darkCount++
        }
      }
      lineDarkness[y] = darkCount / (cWidth / rowStep)
    }

    // 2. Identify text bands (vertical clusters where dark pixels exceed threshold)
    interface Band {
      top: number
      bottom: number
      height: number
    }
    const bands: Band[] = []
    let inBand = false
    let bandStart = 0
    const threshold = 0.015

    for (let y = 0; y < cHeight; y++) {
      if (lineDarkness[y] > threshold) {
        if (!inBand) {
          inBand = true
          bandStart = y
        }
      } else {
        if (inBand) {
          inBand = false
          const h = y - bandStart
          if (h >= 8 && h <= 180) { // filter out speckles or massive photos
            bands.push({ top: bandStart, bottom: y, height: h })
          }
        }
      }
    }

    // 3. For each horizontal band, find horizontal segments (words / phrases)
    const detected: OCRDetectedBlock[] = []
    let blockIdCounter = 1

    for (const band of bands) {
      const colDarkness = new Float32Array(cWidth)
      for (let x = 0; x < cWidth; x++) {
        let darkCount = 0
        for (let y = band.top; y < band.bottom; y += 2) {
          const idx = (y * cWidth + x) * 4
          const brightness = (data[idx] * 299 + data[idx + 1] * 587 + data[idx + 2] * 114) / 1000
          if (brightness < 180 && data[idx + 3] > 100) {
            darkCount++
          }
        }
        colDarkness[x] = darkCount / ((band.bottom - band.top) / 2)
      }

      let inWord = false
      let wordStart = 0
      const colThreshold = 0.02
      let blankGap = 0

      for (let x = 0; x < cWidth; x++) {
        if (colDarkness[x] > colThreshold) {
          if (!inWord) {
            inWord = true
            wordStart = x
          }
          blankGap = 0
        } else {
          if (inWord) {
            blankGap++
            // Merge small spaces inside a phrase, break on significant margin (> 28px)
            if (blankGap > 28 || x === cWidth - 1) {
              const wordW = (x - blankGap) - wordStart
              if (wordW >= 15) {
                // Convert canvas pixel coordinates to PDF points
                const pdfX = wordStart * scaleX
                const pdfY = band.top * scaleY
                const pdfW = wordW * scaleX
                const pdfH = band.height * scaleY
                const estFontSize = Math.max(10, Math.round(pdfH * 0.82))

                detected.push({
                  id: `ocr-block-${pageIndex}-${blockIdCounter++}`,
                  page: pageIndex,
                  text: '', // To be recognized or labeled
                  x: Math.round(pdfX),
                  y: Math.round(pdfY),
                  width: Math.round(pdfW),
                  height: Math.round(pdfH),
                  fontSize: estFontSize,
                  confidence: 96,
                })
              }
              inWord = false
            }
          }
        }
      }
    }

    return detected
  }
}
