import React, { useCallback } from 'react'
import { TextBlock, InteractiveMode } from '../components/PDFViewer'

interface UsePDFCoordinatesParams {
  imgRenderedSize: { width: number; height: number }
  pageSize: { width: number; height: number }
  zoom: number
  interactiveMode: InteractiveMode
  textBlocks: TextBlock[]
  draggingBlockId: number | null
  setDraggingBlockId: (id: number | null) => void
  tempBlockPos: { id: number; x: number; y: number } | null
  setTempBlockPos: (pos: { id: number; x: number; y: number } | null) => void
  blockDragOffset: { x: number; y: number }
  drawBox: { startX: number; startY: number; currentX: number; currentY: number } | null
  setDrawBox: React.Dispatch<React.SetStateAction<{ startX: number; startY: number; currentX: number; currentY: number } | null>>
  currentPage: number
  onSelectTextBlock?: (block: TextBlock | null) => void
  onMoveTextBlock?: (blockId: number, newX: number, newY: number) => void
  onDrawRectComplete?: (rect: { x: number; y: number; width: number; height: number; page: number }) => void
}

export function usePDFCoordinates({
  imgRenderedSize,
  pageSize,
  zoom,
  interactiveMode,
  textBlocks,
  draggingBlockId,
  setDraggingBlockId,
  tempBlockPos,
  setTempBlockPos,
  blockDragOffset,
  drawBox,
  setDrawBox,
  currentPage,
  onSelectTextBlock,
  onMoveTextBlock,
  onDrawRectComplete,
}: UsePDFCoordinatesParams) {
  const scaleX = imgRenderedSize.width > 0 ? imgRenderedSize.width / pageSize.width : 1
  const scaleY = imgRenderedSize.height > 0 ? imgRenderedSize.height / pageSize.height : 1

  const pdfToDom = useCallback((pdfX: number, pdfY: number, pdfW: number, pdfH: number) => {
    const domX = pdfX * scaleX
    // PDF Y is bottom-up; DOM Y is top-down
    const domY = (pageSize.height - (pdfY + pdfH)) * scaleY
    const domW = Math.max(pdfW * scaleX, 10)
    const domH = Math.max(pdfH * scaleY, 12)
    return { left: domX, top: domY, width: domW, height: domH }
  }, [scaleX, scaleY, pageSize.height])

  const domToPdf = useCallback((domX: number, domY: number, domW: number, domH: number) => {
    const pdfX = scaleX > 0 ? domX / scaleX : 0
    const pdfW = scaleX > 0 ? domW / scaleX : 0
    const pdfH = scaleY > 0 ? domH / scaleY : 0
    const pdfY = scaleY > 0 ? pageSize.height - ((domY + domH) / scaleY) : 0
    return {
      x: Math.round(pdfX),
      y: Math.round(pdfY),
      width: Math.round(pdfW),
      height: Math.round(pdfH),
    }
  }, [scaleX, scaleY, pageSize.height])

  // Drawing mouse handlers on overlay
  const handleOverlayMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0 || e.altKey) return

    const overlayRect = e.currentTarget.getBoundingClientRect()
    const clickX = (e.clientX - overlayRect.left) / zoom
    const clickY = (e.clientY - overlayRect.top) / zoom

    if (interactiveMode === 'select-text') {
      onSelectTextBlock?.(null)
    } else if (
      interactiveMode === 'draw-rect' ||
      interactiveMode === 'draw-highlight' ||
      interactiveMode === 'draw-redact' ||
      interactiveMode === 'place-form'
    ) {
      setDrawBox({
        startX: clickX,
        startY: clickY,
        currentX: clickX,
        currentY: clickY,
      })
    }
  }, [zoom, interactiveMode, onSelectTextBlock, setDrawBox])

  const handleOverlayMouseMove = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const overlayRect = e.currentTarget.getBoundingClientRect()
    const currentX = (e.clientX - overlayRect.left) / zoom
    const currentY = (e.clientY - overlayRect.top) / zoom

    // Handle drag-and-drop moving of a selected text block
    if (draggingBlockId !== null && tempBlockPos) {
      const newDomX = currentX - blockDragOffset.x
      const newDomY = currentY - blockDragOffset.y
      const block = textBlocks.find(b => b.id === draggingBlockId)
      if (block) {
        const domW = block.width * scaleX
        const domH = block.height * scaleY
        const pdfCoords = domToPdf(newDomX, newDomY, domW, domH)
        setTempBlockPos({ id: draggingBlockId, x: pdfCoords.x, y: pdfCoords.y })
      }
      return
    }

    // Handle box drawing
    if (drawBox) {
      setDrawBox(prev => (prev ? { ...prev, currentX, currentY } : null))
    }
  }, [zoom, draggingBlockId, tempBlockPos, blockDragOffset, textBlocks, scaleX, scaleY, domToPdf, setTempBlockPos, drawBox, setDrawBox])

  const handleOverlayMouseUp = useCallback(() => {
    if (draggingBlockId !== null && tempBlockPos) {
      onMoveTextBlock?.(tempBlockPos.id, tempBlockPos.x, tempBlockPos.y)
      setDraggingBlockId(null)
      setTempBlockPos(null)
      return
    }

    if (drawBox) {
      const minX = Math.min(drawBox.startX, drawBox.currentX)
      const minY = Math.min(drawBox.startY, drawBox.currentY)
      const w = Math.abs(drawBox.currentX - drawBox.startX)
      const h = Math.abs(drawBox.currentY - drawBox.startY)

      if (w > 5 && h > 5) {
        const pdfRect = domToPdf(minX, minY, w, h)
        onDrawRectComplete?.({
          ...pdfRect,
          page: currentPage,
        })
      }
      setDrawBox(null)
    }
  }, [draggingBlockId, tempBlockPos, onMoveTextBlock, setDraggingBlockId, setTempBlockPos, drawBox, domToPdf, onDrawRectComplete, currentPage, setDrawBox])

  return {
    scaleX,
    scaleY,
    pdfToDom,
    domToPdf,
    handleOverlayMouseDown,
    handleOverlayMouseMove,
    handleOverlayMouseUp,
  }
}
