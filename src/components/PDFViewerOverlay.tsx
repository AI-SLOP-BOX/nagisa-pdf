import React, { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { DocumentService } from '../services/documentService'
import { UserAnnotation } from '../services/annotationService'
import { TextBlock, InteractiveMode } from './PDFViewer'
import { AnnotationOverlayItem } from './AnnotationOverlayItem'

interface PDFViewerOverlayProps {
  interactiveMode: InteractiveMode
  selectedTextBlockId?: number | null
  textBlocks: TextBlock[]
  tempBlockPos: { id: number; x: number; y: number } | null
  pdfToDom: (x: number, y: number, w: number, h: number) => { left: number; top: number; width: number; height: number }
  onSelectTextBlock?: (block: TextBlock | null) => void
  setDraggingBlockId: (id: number | null) => void
  setBlockDragOffset: (offset: { x: number; y: number }) => void
  setTempBlockPos: (pos: { id: number; x: number; y: number } | null) => void
  zoom: number
  editingBlockId: number | null
  setEditingBlockId: (id: number | null) => void
  editingTextVal: string
  setEditingTextVal: (val: string) => void
  pdfData: number[] | null
  docId?: string | null
  currentPage: number
  onPdfUpdate?: (data: number[]) => void
  imgRenderedSize: { width: number; height: number }
  pageSize: { width: number; height: number }
  drawBox: { startX: number; startY: number; currentX: number; currentY: number } | null
  handleOverlayMouseDown: (e: React.MouseEvent<HTMLDivElement>) => void
  handleOverlayMouseMove: (e: React.MouseEvent<HTMLDivElement>) => void
  handleOverlayMouseUp: () => void
  // Annotation system props
  editorMode?: 'inspect' | 'edit'
  selectedEditTool?: string
  annotations?: UserAnnotation[]
  selectedAnnotationId?: string | null
  onSelectAnnotation?: (id: string | null) => void
  onUpdateAnnotation?: (id: string, updates: Partial<UserAnnotation>) => void
  onAddAnnotation?: (ann: UserAnnotation) => void
  onDeleteAnnotation?: (id: string) => void
}

export const PDFViewerOverlay: React.FC<PDFViewerOverlayProps> = ({
  interactiveMode,
  selectedTextBlockId,
  textBlocks,
  tempBlockPos,
  pdfToDom,
  onSelectTextBlock,
  setDraggingBlockId,
  setBlockDragOffset,
  setTempBlockPos,
  zoom,
  editingBlockId,
  setEditingBlockId,
  editingTextVal,
  setEditingTextVal,
  pdfData,
  docId,
  currentPage,
  onPdfUpdate,
  imgRenderedSize,
  pageSize,
  drawBox,
  handleOverlayMouseDown,
  handleOverlayMouseMove,
  handleOverlayMouseUp,
  editorMode = 'inspect',
  selectedEditTool = 'select',
  annotations = [],
  selectedAnnotationId,
  onSelectAnnotation,
  onUpdateAnnotation,
  onAddAnnotation,
  onDeleteAnnotation,
}) => {
  const scaleX = imgRenderedSize.width > 0 && pageSize.width > 0 ? imgRenderedSize.width / pageSize.width : 1
  const scaleY = imgRenderedSize.height > 0 && pageSize.height > 0 ? imgRenderedSize.height / pageSize.height : 1

  // Annotation dragging state
  const [draggingAnn, setDraggingAnn] = useState<{
    id: string
    startX: number
    startY: number
    initX: number
    initY: number
  } | null>(null)

  // Inline text editing state
  const [inlineEditingId, setInlineEditingId] = useState<string | null>(null)

  // Global mouse move and up for annotation dragging
  useEffect(() => {
    if (!draggingAnn) return

    const handleMouseMove = (e: MouseEvent) => {
      const dx = (e.clientX - draggingAnn.startX) / (zoom * scaleX)
      const dy = (e.clientY - draggingAnn.startY) / (zoom * scaleY)
      onUpdateAnnotation?.(draggingAnn.id, {
        x: Math.max(0, Math.round(draggingAnn.initX + dx)),
        y: Math.max(0, Math.round(draggingAnn.initY + dy)),
      })
    }

    const handleMouseUp = () => {
      setDraggingAnn(null)
    }

    window.addEventListener('mousemove', handleMouseMove)
    window.addEventListener('mouseup', handleMouseUp)
    return () => {
      window.removeEventListener('mousemove', handleMouseMove)
      window.removeEventListener('mouseup', handleMouseUp)
    }
  }, [draggingAnn, zoom, scaleX, scaleY, onUpdateAnnotation])

  // Click on canvas in edit mode to place new elements
  const handleCanvasClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (editorMode !== 'edit') return

    const overlayRect = e.currentTarget.getBoundingClientRect()
    const clickDomX = (e.clientX - overlayRect.left) / zoom
    const clickDomY = (e.clientY - overlayRect.top) / zoom
    const pdfX = Math.round(clickDomX / scaleX)
    const pdfY = Math.round(clickDomY / scaleY)

    if (selectedEditTool === 'text') {
      const newAnn: UserAnnotation = {
        id: 'ann_' + Date.now(),
        page: currentPage,
        type: 'text',
        x: Math.max(10, pdfX),
        y: Math.max(10, pdfY),
        width: 180,
        height: 32,
        text: 'ここに入力',
        fontSize: 16,
        color: '#0f172a',
        isBold: false,
        isItalic: false,
      }
      onAddAnnotation?.(newAnn)
      onSelectAnnotation?.(newAnn.id)
      setInlineEditingId(newAnn.id)
    } else if (selectedEditTool === 'highlight') {
      const newAnn: UserAnnotation = {
        id: 'ann_' + Date.now(),
        page: currentPage,
        type: 'highlight',
        x: Math.max(10, pdfX),
        y: Math.max(10, pdfY),
        width: 160,
        height: 24,
      }
      onAddAnnotation?.(newAnn)
      onSelectAnnotation?.(newAnn.id)
    } else if (selectedEditTool === 'shape') {
      const newAnn: UserAnnotation = {
        id: 'ann_' + Date.now(),
        page: currentPage,
        type: 'shape',
        x: Math.max(10, pdfX),
        y: Math.max(10, pdfY),
        width: 140,
        height: 70,
      }
      onAddAnnotation?.(newAnn)
      onSelectAnnotation?.(newAnn.id)
    } else {
      // Select tool clicked empty canvas -> deselect
      onSelectAnnotation?.(null)
      setInlineEditingId(null)
    }
  }

  const isDrawingMode = editorMode === 'edit' && (selectedEditTool === 'text' || selectedEditTool === 'shape' || selectedEditTool === 'highlight')
  const pageAnnotations = annotations.filter(a => a.page === currentPage)

  return (
    <div
      onClick={handleCanvasClick}
      onMouseDown={handleOverlayMouseDown}
      onMouseMove={handleOverlayMouseMove}
      onMouseUp={handleOverlayMouseUp}
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        width: imgRenderedSize.width,
        height: imgRenderedSize.height,
        cursor: editorMode === 'edit'
          ? (selectedEditTool === 'text' ? 'text' : selectedEditTool === 'shape' || selectedEditTool === 'highlight' ? 'crosshair' : 'default')
          : (interactiveMode === 'select-text' ? 'text' : interactiveMode === 'view' ? 'default' : 'crosshair'),
        pointerEvents: isDrawingMode ? 'auto' : 'none',
      }}
    >

      {/* 1. Render User Added Annotations on current page */}
      {pageAnnotations.map(ann => {
        return (
          <React.Fragment key={ann.id}>
            <AnnotationOverlayItem
              annotation={ann}
              isSelected={selectedAnnotationId === ann.id}
              isInlineEditing={inlineEditingId === ann.id}
              scaleX={scaleX}
              scaleY={scaleY}
              isDraggingCurrent={draggingAnn?.id === ann.id}
              onSelect={id => onSelectAnnotation?.(id)}
              onStartInlineEdit={id => setInlineEditingId(id)}
              onEndInlineEdit={() => setInlineEditingId(null)}
              onStartDrag={(item, e) => {
                setDraggingAnn({
                  id: item.id,
                  startX: e.clientX,
                  startY: e.clientY,
                  initX: item.x,
                  initY: item.y,
                })
              }}
              onUpdateText={(id, text) => onUpdateAnnotation?.(id, { text })}
              onUpdateAnnotation={(id, updates) => onUpdateAnnotation?.(id, updates)}
              onDelete={id => onDeleteAnnotation?.(id)}
            />
          </React.Fragment>
        )
      })}

      {/* 2. Interactive Native Text Blocks (Rust Backend IPC) */}
      {(interactiveMode === 'select-text' || selectedTextBlockId !== undefined) &&
        textBlocks.map(block => {
          const isSelected = selectedTextBlockId === block.id
          const isBeingMoved = tempBlockPos && tempBlockPos.id === block.id
          const blockX = isBeingMoved ? tempBlockPos.x : block.x
          const blockY = isBeingMoved ? tempBlockPos.y : block.y

          const dom = pdfToDom(blockX, blockY, block.width, block.height)

          return (
            <div
              key={block.id}
              title={`${block.text} (${Math.round(block.x)}, ${Math.round(block.y)})`}
              onClick={(e) => {
                e.stopPropagation()
                onSelectTextBlock?.(block)
              }}
              onMouseDown={(e) => {
                if (e.button === 0 && !e.altKey && interactiveMode === 'select-text') {
                  e.stopPropagation()
                  onSelectTextBlock?.(block)
                  setDraggingBlockId(block.id)
                  const overlayRect = e.currentTarget.parentElement?.getBoundingClientRect()
                  if (overlayRect) {
                    const clickX = (e.clientX - overlayRect.left) / zoom
                    const clickY = (e.clientY - overlayRect.top) / zoom
                    setBlockDragOffset({
                      x: clickX - dom.left,
                      y: clickY - dom.top,
                    })
                    setTempBlockPos({ id: block.id, x: block.x, y: block.y })
                  }
                }
              }}
              onDoubleClick={(e) => {
                e.stopPropagation()
                setEditingBlockId(block.id)
                setEditingTextVal(block.text)
              }}
              style={{
                position: 'absolute',
                left: dom.left,
                top: dom.top,
                width: dom.width,
                height: dom.height,
                border: isSelected
                  ? '1.5px solid var(--accent)'
                  : '1px dashed rgba(37, 99, 235, 0.25)',
                background: isSelected
                  ? 'rgba(37, 99, 235, 0.08)'
                  : 'transparent',
                borderRadius: 2,
                boxSizing: 'border-box',
                cursor: editingBlockId === block.id ? 'text' : 'move',
                transition: isBeingMoved ? 'none' : 'background 0.15s, border 0.15s',
                zIndex: isSelected || editingBlockId === block.id ? 10 : 1,
              }}
            >
              {editingBlockId === block.id ? (
                <input
                  autoFocus
                  value={editingTextVal}
                  onChange={e => setEditingTextVal(e.target.value)}
                  onKeyDown={async (e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault()
                      e.stopPropagation()
                      const currentBytes = docId ? await DocumentService.getSessionBytes(docId) : pdfData
                      if (currentBytes && editingTextVal !== block.text) {
                        try {
                          const updated = await invoke<number[]>('edit_text_block', {
                            data: currentBytes,
                            pageIndex: currentPage,
                            blockId: block.id,
                            newText: editingTextVal,
                          })
                          onPdfUpdate?.(updated)
                        } catch (err) {
                          console.error('Failed to update text in-place:', err)
                        }
                      }
                      setEditingBlockId(null)
                    } else if (e.key === 'Escape') {
                      e.stopPropagation()
                      setEditingBlockId(null)
                    }
                  }}
                  onBlur={async () => {
                    const currentBytes = docId ? await DocumentService.getSessionBytes(docId) : pdfData
                    if (currentBytes && editingTextVal !== block.text) {
                      try {
                        const updated = await invoke<number[]>('edit_text_block', {
                          data: currentBytes,
                          pageIndex: currentPage,
                          blockId: block.id,
                          newText: editingTextVal,
                        })
                        onPdfUpdate?.(updated)
                      } catch (err) {
                        console.error('Failed to update text in-place:', err)
                      }
                    }
                    setEditingBlockId(null)
                  }}
                  style={{
                    width: '100%',
                    height: '100%',
                    background: '#ffffff',
                    color: block.color || '#0f172a',
                    border: '1.5px solid var(--accent)',
                    outline: 'none',
                    fontSize: Math.max(10, Math.round(block.font_size * scaleY)),
                    fontFamily: block.font_name.toLowerCase().includes('sans') ? 'sans-serif' : 'serif',
                    padding: '0 2px',
                    boxSizing: 'border-box',
                    borderRadius: 2,
                    boxShadow: '0 2px 8px rgba(0,0,0,0.12)',
                  }}
                />
              ) : (
                <>
                  {/* When dragging/moving, show moving text clone with semi-transparent badge */}
                  {isBeingMoved && (
                    <span
                      style={{
                        display: 'inline-block',
                        width: '100%',
                        height: '100%',
                        fontSize: Math.max(10, Math.round(block.font_size * scaleY)),
                        fontFamily: block.font_name.toLowerCase().includes('sans') ? 'sans-serif' : 'serif',
                        color: block.color || '#0f172a',
                        lineHeight: 1.15,
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        pointerEvents: 'none',
                        userSelect: 'none',
                        background: 'rgba(255, 255, 255, 0.85)',
                        boxShadow: '0 2px 6px rgba(0,0,0,0.15)',
                      }}
                    >
                      {block.text}
                    </span>
                  )}
                  {isSelected && (
                    <div style={{
                      position: 'absolute',
                      top: -20,
                      left: 0,
                      background: 'var(--accent)',
                      color: '#ffffff',
                      fontSize: 10,
                      fontWeight: 600,
                      padding: '1px 6px',
                      borderRadius: 3,
                      whiteSpace: 'nowrap',
                      boxShadow: '0 2px 4px rgba(0,0,0,0.12)',
                      pointerEvents: 'none',
                      zIndex: 20,
                    }}>
                      {block.font_name || 'Text'} ({Math.round(blockX)}, {Math.round(blockY)}) [Wクリックで編集]
                    </div>
                  )}
                </>
              )}
            </div>
          )
        })}

      {/* 3. Active Drag-To-Draw Box Preview */}
      {drawBox && (
        <div
          style={{
            position: 'absolute',
            left: Math.min(drawBox.startX, drawBox.currentX),
            top: Math.min(drawBox.startY, drawBox.currentY),
            width: Math.abs(drawBox.currentX - drawBox.startX),
            height: Math.abs(drawBox.currentY - drawBox.startY),
            border: interactiveMode === 'draw-redact'
              ? '2px solid #ff3344'
              : interactiveMode === 'draw-highlight'
                ? '2px solid #ffcc00'
                : interactiveMode === 'place-form'
                  ? '2px dashed #00d2ff'
                  : '2px solid var(--accent)',
            background: interactiveMode === 'draw-redact'
              ? 'rgba(0, 0, 0, 0.75)'
              : interactiveMode === 'draw-highlight'
                ? 'rgba(255, 235, 59, 0.4)'
                : interactiveMode === 'place-form'
                  ? 'rgba(0, 210, 255, 0.2)'
                  : 'rgba(0, 200, 255, 0.2)',
            pointerEvents: 'none',
            zIndex: 20,
          }}
        >
          <span style={{
            position: 'absolute',
            bottom: 2,
            right: 4,
            fontSize: 10,
            color: '#fff',
            textShadow: '0 1px 2px #000',
            fontWeight: 600,
          }}>
            {Math.round(Math.abs(drawBox.currentX - drawBox.startX))} × {Math.round(Math.abs(drawBox.currentY - drawBox.startY))}
          </span>
        </div>
      )}
    </div>
  )
}
