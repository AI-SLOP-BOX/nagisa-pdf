import { useState, useCallback, RefObject } from 'react'
import { OCRService } from '../services/ocrService'
import type { UserAnnotation } from '../services/annotationService'

interface UsePDFOCRProps {
  imgRef: RefObject<HTMLImageElement | null>
  currentPage: number
  pageSize: { width: number; height: number }
  onAddAnnotation?: (ann: UserAnnotation) => void
}

export function usePDFOCR({ imgRef, currentPage, pageSize, onAddAnnotation }: UsePDFOCRProps) {
  const [isOCRing, setIsOCRing] = useState(false)
  const [ocrDetectedCount, setOcrDetectedCount] = useState<number | null>(null)

  const runOCR = useCallback(async () => {
    const imgEl = imgRef.current
    if (!imgEl || !imgEl.naturalWidth || !imgEl.naturalHeight) {
      return []
    }

    setIsOCRing(true)
    try {
      const canvas = document.createElement('canvas')
      canvas.width = imgEl.naturalWidth
      canvas.height = imgEl.naturalHeight
      const ctx = canvas.getContext('2d')
      if (!ctx) return []

      ctx.drawImage(imgEl, 0, 0)
      const blocks = await OCRService.detectTextFromCanvas(
        canvas,
        currentPage,
        pageSize.width,
        pageSize.height
      )

      // Filter for substantial text lines (ignore tiny noise specs)
      const significantBlocks = blocks.filter(b => b.width >= 40 && b.height >= 8)

      // Convert detected blocks into editable annotations
      significantBlocks.forEach((block, idx) => {
        onAddAnnotation?.({
          id: `ocr-edit-${currentPage}-${Date.now()}-${idx}`,
          page: currentPage,
          type: 'text',
          x: block.x,
          y: block.y,
          width: Math.max(block.width, 50),
          height: Math.max(block.height, 18),
          text: block.text || '',
          fontSize: block.fontSize || 14,
          color: '#0f172a',
        })
      })

      setOcrDetectedCount(significantBlocks.length)
      return significantBlocks
    } catch (err) {
      console.error('OCR Detection error:', err)
      return []
    } finally {
      setIsOCRing(false)
    }
  }, [imgRef, currentPage, pageSize, onAddAnnotation])

  return {
    isOCRing,
    ocrDetectedCount,
    runOCR,
    setOcrDetectedCount,
  }
}
