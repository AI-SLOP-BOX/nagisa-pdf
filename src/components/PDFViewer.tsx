import { useState, useRef, useCallback, useEffect } from 'react'
import { DocumentService } from '../services/documentService'
import { defaultRenderer } from '../services/pdfRenderer'
import { safeRevokeObjectUrl } from '../utils/objectUrl'
import { FileIcon } from './Icons'
import {
  SearchResult, Bookmark, FormField,
} from './PDFViewerPanels'
import { PDFViewerToolbar, PanelType } from './PDFViewerToolbar'
import { PDFViewerHUD } from './PDFViewerHUD'
import { PDFViewerOverlay } from './PDFViewerOverlay'
import { PDFViewerSidebar } from './PDFViewerSidebar'
import { PDFThumbnailStrip } from './PDFThumbnailStrip'
import { UserAnnotation } from '../services/annotationService'
import { usePDFCoordinates } from '../hooks/usePDFCoordinates'
import { usePDFOCR } from '../hooks/usePDFOCR'

export interface TextBlock {
  id: number
  text: string
  x: number
  y: number
  width: number
  height: number
  font_name: string
  font_size: number
  color: string
  page_index: number
}

export type InteractiveMode =
  | 'view'
  | 'select-text'
  | 'draw-rect'
  | 'draw-highlight'
  | 'draw-redact'
  | 'place-form'

export interface PDFViewerProps {
  pdfData: number[] | null
  docId?: string | null
  revision?: number
  currentPage?: number
  onPageCountChange?: (count: number) => void
  onPageChange?: (page: number) => void
  interactiveMode?: InteractiveMode
  selectedTextBlockId?: number | null
  onSelectTextBlock?: (block: TextBlock | null) => void
  onMoveTextBlock?: (blockId: number, newX: number, newY: number) => void
  onDrawRectComplete?: (rect: { x: number; y: number; width: number; height: number; page: number }) => void
  onPdfUpdate?: (data: number[]) => void
  hideToolbar?: boolean
  hideBottomThumbnails?: boolean
  hideFloatingHUD?: boolean
  // Annotation system props
  editorMode?: 'inspect' | 'edit'
  selectedEditTool?: string
  annotations?: UserAnnotation[]
  selectedAnnotationId?: string | null
  onSelectAnnotation?: (id: string | null) => void
  onUpdateAnnotation?: (id: string, updates: Partial<UserAnnotation>) => void
  onAddAnnotation?: (ann: UserAnnotation) => void
  onDeleteAnnotation?: (id: string) => void
  triggerOCR?: number
  onOCRComplete?: (count: number) => void
}

export default function PDFViewer({
  pdfData,
  docId,
  revision = 0,
  currentPage: externalCurrentPage,
  onPageCountChange,
  onPageChange,
  interactiveMode = 'view',
  selectedTextBlockId,
  onSelectTextBlock,
  onMoveTextBlock,
  onDrawRectComplete,
  onPdfUpdate,
  hideToolbar = false,
  hideBottomThumbnails = false,
  hideFloatingHUD = false,
  editorMode = 'inspect',
  selectedEditTool = 'select',
  annotations = [],
  selectedAnnotationId,
  onSelectAnnotation,
  onUpdateAnnotation,
  onAddAnnotation,
  onDeleteAnnotation,
  triggerOCR,
  onOCRComplete,
}: PDFViewerProps) {
  const [pageCount, setPageCount] = useState(0)
  const [internalCurrentPage, setInternalCurrentPage] = useState(0)
  const currentPage = externalCurrentPage !== undefined ? externalCurrentPage : internalCurrentPage

  const [pageImage, setPageImage] = useState<string | null>(null)
  const [zoom, setZoom] = useState(1.0)
  const [pan, setPan] = useState({ x: 0, y: 0 })
  const [isDragging, setIsDragging] = useState(false)
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 })
  const [loading, setLoading] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<SearchResult[]>([])
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([])
  const [formFields, setFormFields] = useState<FormField[]>([])
  const [metadata, setMetadata] = useState<Record<string, unknown> | null>(null)
  const [activePanel, setActivePanel] = useState<'none' | 'thumbnails' | 'search' | 'bookmarks' | 'forms' | 'info' | 'separations'>('none')

  // Color Separation Preview state (CMYK + TAC)
  const [sepPlates, setSepPlates] = useState<{ c: boolean; m: boolean; y: boolean; k: boolean; tac: boolean }>({
    c: true,
    m: true,
    y: true,
    k: true,
    tac: false,
  })
  const [tacLimit, setTacLimit] = useState(300)

  // In-place direct text editing state
  const [editingBlockId, setEditingBlockId] = useState<number | null>(null)
  const [editingTextVal, setEditingTextVal] = useState('')

  // Text blocks & Page dimensions for interactive canvas overlay
  const [textBlocks, setTextBlocks] = useState<TextBlock[]>([])
  const [pageSize, setPageSize] = useState<{ width: number; height: number }>({ width: 595, height: 842 })
  const [imgRenderedSize, setImgRenderedSize] = useState<{ width: number; height: number }>({ width: 0, height: 0 })

  // 前のページの blob URL を保持し、ページ変更時に revoke する（メモリリーク防止）
  const prevPageUrlRef = useRef<string | null>(null)
  const prevCacheKeyRef = useRef<string | null>(null)

  // Page Cache & Render Token for Instant Page Switching & Thread-safety
  const pageCache = useRef<Map<string, string>>(new Map())
  const renderSeq = useRef(0)

  // Clear cache when pdfData, docId, or revision changes
  useEffect(() => {
    // Revoke old object URLs
    pageCache.current.forEach(url => safeRevokeObjectUrl(url))
    pageCache.current.clear()
  }, [pdfData, docId, revision])

  // Prevent memory leaks: Revoke all object URLs when PDFViewer unmounts
  useEffect(() => {
    return () => {
      pageCache.current.forEach(url => safeRevokeObjectUrl(url))
      pageCache.current.clear()
      // Cancel any in-flight renderer work tied to this app instance
      defaultRenderer.cancelAll()
    }
  }, [])

  // Dragging / Drawing state on overlay
  const [draggingBlockId, setDraggingBlockId] = useState<number | null>(null)
  const [blockDragOffset, setBlockDragOffset] = useState<{ x: number; y: number }>({ x: 0, y: 0 })
  const [tempBlockPos, setTempBlockPos] = useState<{ id: number; x: number; y: number } | null>(null)
  const [drawBox, setDrawBox] = useState<{ startX: number; startY: number; currentX: number; currentY: number } | null>(null)

  const containerRef = useRef<HTMLDivElement>(null)
  const imgRef = useRef<HTMLImageElement>(null)

  const { runOCR } = usePDFOCR({
    imgRef,
    currentPage,
    pageSize,
    onAddAnnotation,
  })

  useEffect(() => {
    if (triggerOCR && triggerOCR > 0) {
      runOCR().then(blocks => {
        onOCRComplete?.(blocks.length)
      })
    }
  }, [triggerOCR, runOCR, onOCRComplete])

  // Load PDF info using docId (zero IPC bytes) or pdfData fallback
  useEffect(() => {
    const handle = (docId && !docId.startsWith('browser-session-'))
      ? docId
      : ((pdfData && pdfData.length > 0) ? pdfData : docId)
    if (!handle) return

    const loadInfo = async () => {
      try {
        const count = await DocumentService.getPageCount(handle)
        setPageCount(count)
        onPageCountChange?.(count)

        const meta = await DocumentService.getPdfMetadata(handle)
        setMetadata(meta)

        const bms = await DocumentService.getBookmarks(handle)
        setBookmarks(bms)

        const fields = await DocumentService.getFormFields(handle)
        setFormFields(fields)
      } catch (err) {
        console.error('Failed to load PDF info:', err)
      }
    }

    loadInfo()
  }, [docId, pdfData, revision, onPageCountChange])

  // Load Page Dimensions and Text Blocks for current page
  useEffect(() => {
    const handle = (docId && !docId.startsWith('browser-session-'))
      ? docId
      : ((pdfData && pdfData.length > 0) ? pdfData : docId)
    if (!handle || currentPage >= pageCount) return

    const loadPageData = async () => {
      try {
        const dims = await DocumentService.getPageDimensions(handle, currentPage)
        if (dims && dims.width && dims.height) {
          setPageSize(dims)
        }
      } catch {
        setPageSize({ width: 595, height: 842 })
      }

      try {
        const blocks = await DocumentService.getTextBlocks(handle, currentPage)
        setTextBlocks(blocks || [])
      } catch {
        setTextBlocks([])
      }
    }

    loadPageData()
  }, [docId, pdfData, revision, currentPage, pageCount])

  // Render current page using PDFRenderer with Instant Cache
  const targetDpi = Math.min(300, Math.max(120, Math.round(150 * Math.min(zoom, 2.0))))

  useEffect(() => {
    const hasSource = !!docId || (!!pdfData && pdfData.length > 0)
    if (!hasSource || currentPage >= pageCount) return

    const isCustomSep = !sepPlates.c || !sepPlates.m || !sepPlates.y || !sepPlates.k || sepPlates.tac
    const cacheKey = `${docId || 'data'}_${revision}@${currentPage}@${targetDpi}_c${sepPlates.c ? 1 : 0}m${sepPlates.m ? 1 : 0}y${sepPlates.y ? 1 : 0}k${sepPlates.k ? 1 : 0}tac${sepPlates.tac ? 1 : 0}_${tacLimit}`
    const cachedUrl = pageCache.current.get(cacheKey)
    if (cachedUrl) {
      // 前のページの blob URL を revoke して refs を更新してからキャッシュヒット画像を表示
      safeRevokeObjectUrl(prevPageUrlRef.current)
      if (prevCacheKeyRef.current) {
        pageCache.current.delete(prevCacheKeyRef.current)
      }
      prevPageUrlRef.current = cachedUrl
      prevCacheKeyRef.current = cacheKey
      setPageImage(cachedUrl)
      setLoading(false)
      return
    }

    const currentToken = ++renderSeq.current
    const abortController = new AbortController()
    setLoading(true)

    // cleanup で revoke する対象を snapshot しておく（新しい effect が refs を更新する前に cleanup が実行されるため）
    const cleanupSnapshotUrl = prevPageUrlRef.current

    const timer = setTimeout(async () => {
      try {
        let url: string
        if (isCustomSep) {
          url = await defaultRenderer.renderSeparationToUrl({
            docId: docId || undefined,
            pdfData: (pdfData && pdfData.length > 0) ? pdfData : undefined,
            pageIndex: currentPage,
            dpi: targetDpi,
            showC: sepPlates.c,
            showM: sepPlates.m,
            showY: sepPlates.y,
            showK: sepPlates.k,
            highlightTac: sepPlates.tac,
            tacLimit,
            signal: abortController.signal,
          })
        } else {
          url = await defaultRenderer.renderPageToUrl({
            docId: docId || undefined,
            pdfData: (pdfData && pdfData.length > 0) ? pdfData : undefined,
            pageIndex: currentPage,
            dpi: targetDpi,
            signal: abortController.signal,
          })
        }

        if (currentToken !== renderSeq.current || abortController.signal.aborted) {
          safeRevokeObjectUrl(url)
          return
        }

        // Cache up to 16 rendered states in memory
        if (pageCache.current.size >= 16) {
          const firstKey = pageCache.current.keys().next().value
          if (firstKey !== undefined) {
            const oldUrl = pageCache.current.get(firstKey)
            safeRevokeObjectUrl(oldUrl)
            pageCache.current.delete(firstKey)
          }
        }

        // 前のページの blob URL とキャッシュをクリーンアップしてから新しい画像を設定
        safeRevokeObjectUrl(prevPageUrlRef.current)
        if (prevCacheKeyRef.current) {
          pageCache.current.delete(prevCacheKeyRef.current)
        }
        prevPageUrlRef.current = url
        prevCacheKeyRef.current = cacheKey
        pageCache.current.set(cacheKey, url)
        setPageImage(url)
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err)
        if (currentToken === renderSeq.current && message !== 'Render cancelled') {
          console.error('Failed to render page:', err)
          setPageImage(null)
        }
      } finally {
        if (currentToken === renderSeq.current && !abortController.signal.aborted) {
          setLoading(false)
        }
      }
    }, zoom !== 1.0 ? 80 : 0)

    return () => {
      clearTimeout(timer)
      abortController.abort()
      // 前のページの blob URL を revoke する（snapshot を使用）
      safeRevokeObjectUrl(cleanupSnapshotUrl)
      if (prevCacheKeyRef.current) {
        pageCache.current.delete(prevCacheKeyRef.current)
      }
      prevPageUrlRef.current = null
      prevCacheKeyRef.current = null
    }
  }, [docId, pdfData, revision, currentPage, pageCount, targetDpi, sepPlates, tacLimit])

  // Update image rendered dimensions
  const handleImageLoad = () => {
    if (imgRef.current) {
      setImgRenderedSize({
        width: imgRef.current.clientWidth,
        height: imgRef.current.clientHeight,
      })
    }
  }

  // Handle zoom
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault()
    const delta = e.deltaY > 0 ? -0.1 : 0.1
    setZoom(prev => Math.max(0.25, Math.min(4.0, prev + delta)))
  }, [])

  // Handle pan when in view mode or holding middle-click / Alt
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 1 || (e.button === 0 && e.altKey) || (interactiveMode === 'view' && e.button === 0 && e.target === containerRef.current)) {
      setIsDragging(true)
      setDragStart({ x: e.clientX - pan.x, y: e.clientY - pan.y })
    }
  }, [pan, interactiveMode])

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (isDragging) {
      setPan({
        x: e.clientX - dragStart.x,
        y: e.clientY - dragStart.y,
      })
    }
  }, [isDragging, dragStart])

  const handleMouseUp = useCallback(() => {
    setIsDragging(false)
  }, [])

  // Page navigation
  const goToPage = useCallback((page: number) => {
    const newPage = Math.max(0, Math.min(pageCount - 1, page))
    setInternalCurrentPage(newPage)
    onPageChange?.(newPage)
    setPan({ x: 0, y: 0 })
  }, [pageCount, onPageChange])

  // Search using docId (zero IPC bytes) or pdfData fallback
  const handleSearch = useCallback(async () => {
    if (!searchQuery.trim()) {
      setSearchResults([])
      return
    }

    const handle = docId || (pdfData && pdfData.length > 0 ? pdfData : null)
    if (!handle) return

    try {
      const results = await DocumentService.searchPdf(handle, searchQuery)
      setSearchResults(results)
    } catch (err) {
      console.error('Search failed:', err)
    }
  }, [docId, pdfData, searchQuery])

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey) {
        switch (e.key) {
          case '=':
          case '+':
            e.preventDefault()
            setZoom(prev => Math.min(4.0, prev + 0.25))
            break
          case '-':
            e.preventDefault()
            setZoom(prev => Math.max(0.25, prev - 0.25))
            break
          case '0':
            e.preventDefault()
            setZoom(1.0)
            setPan({ x: 0, y: 0 })
            break
          case 'ArrowLeft':
            e.preventDefault()
            goToPage(currentPage - 1)
            break
          case 'ArrowRight':
            e.preventDefault()
            goToPage(currentPage + 1)
            break
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [currentPage, goToPage])

  // --- Interactive Canvas Calculations & Mouse Handlers Hook ---
  const {
    scaleX,
    scaleY,
    pdfToDom,
    domToPdf,
    handleOverlayMouseDown,
    handleOverlayMouseMove,
    handleOverlayMouseUp,
  } = usePDFCoordinates({
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
  })

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#f8fafc' }}>
      {!hideToolbar && (
        <PDFViewerToolbar
          activePanel={activePanel}
          setActivePanel={setActivePanel}
          currentPage={currentPage}
          pageCount={pageCount}
          goToPage={goToPage}
          zoom={zoom}
          setZoom={setZoom}
          onResetZoom={() => { setZoom(1.0); setPan({ x: 0, y: 0 }) }}
          interactiveMode={interactiveMode}
          loading={loading}
        />
      )}

      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        {/* Side Panel */}
        <PDFViewerSidebar
          activePanel={activePanel}
          sepPlates={sepPlates}
          setSepPlates={setSepPlates}
          tacLimit={tacLimit}
          setTacLimit={setTacLimit}
          pdfData={pdfData}
          pageCount={pageCount}
          currentPage={currentPage}
          goToPage={goToPage}
          onPdfUpdate={onPdfUpdate}
          searchQuery={searchQuery}
          setSearchQuery={setSearchQuery}
          searchResults={searchResults}
          handleSearch={handleSearch}
          bookmarks={bookmarks}
          formFields={formFields}
          metadata={metadata}
        />

        {/* PDF Canvas with Interactive Overlay Layer */}
        <div
          ref={containerRef}
          onWheel={handleWheel}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
          style={{
            flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center',
            overflow: 'hidden', cursor: isDragging ? 'grabbing' : 'default',
            background: '#eef2f6', position: 'relative',
          }}
        >
          {pageImage ? (
            <div
              key={`page-${currentPage}-${docId || 'local'}`}
              className="nagisa-canvas-entrance"
              style={{
                transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
                transformOrigin: 'center center',
                transition: isDragging ? 'none' : 'transform 0.05s ease',
                position: 'relative',
                display: 'inline-block',
                boxShadow: '0 10px 32px rgba(0, 0, 0, 0.08), 0 2px 8px rgba(0, 0, 0, 0.04)',
                borderRadius: 4,
                background: '#ffffff',
              }}
            >
              {/* Rendered PDF Page Image */}
              <img
                ref={imgRef}
                src={pageImage}
                alt={`Page ${currentPage + 1}`}
                onLoad={handleImageLoad}
                style={{
                  display: 'block',
                  maxWidth: '85vw',
                  maxHeight: '80vh',
                  userSelect: 'none',
                  borderRadius: 4,
                }}
                draggable={false}
              />

              {/* Interactive Vector / Bounding Box Overlay */}
              {imgRenderedSize.width > 0 && imgRenderedSize.height > 0 && (
                <PDFViewerOverlay
                  interactiveMode={interactiveMode}
                  selectedTextBlockId={selectedTextBlockId}
                  textBlocks={textBlocks}
                  tempBlockPos={tempBlockPos}
                  pdfToDom={pdfToDom}
                  onSelectTextBlock={onSelectTextBlock}
                  setDraggingBlockId={setDraggingBlockId}
                  setBlockDragOffset={setBlockDragOffset}
                  setTempBlockPos={setTempBlockPos}
                  zoom={zoom}
                  editingBlockId={editingBlockId}
                  setEditingBlockId={setEditingBlockId}
                  editingTextVal={editingTextVal}
                  setEditingTextVal={setEditingTextVal}
                  pdfData={pdfData}
                  docId={docId}
                  currentPage={currentPage}
                  onPdfUpdate={onPdfUpdate}
                  imgRenderedSize={imgRenderedSize}
                  pageSize={pageSize}
                  drawBox={drawBox}
                  handleOverlayMouseDown={handleOverlayMouseDown}
                  handleOverlayMouseMove={handleOverlayMouseMove}
                  handleOverlayMouseUp={handleOverlayMouseUp}
                  editorMode={editorMode}
                  selectedEditTool={selectedEditTool}
                  annotations={annotations}
                  selectedAnnotationId={selectedAnnotationId}
                  onSelectAnnotation={onSelectAnnotation}
                  onUpdateAnnotation={onUpdateAnnotation}
                  onAddAnnotation={onAddAnnotation}
                  onDeleteAnnotation={onDeleteAnnotation}
                />
              )}
            </div>
          ) : (
            <div style={{ color: '#666', textAlign: 'center' }}>
              <div style={{ marginBottom: 16, opacity: 0.25, display: 'flex', justifyContent: 'center' }}>
                <FileIcon size={48} color="var(--text-dim)" />
              </div>
              <p>ページを読み込み中...</p>
            </div>
          )}

          {/* Apple Floating Glass HUD for Zoom & Navigation */}
          {!hideFloatingHUD && (
            <PDFViewerHUD
              currentPage={currentPage}
              pageCount={pageCount}
              goToPage={goToPage}
              zoom={zoom}
              setZoom={setZoom}
              setPan={setPan}
            />
          )}
        </div>
      </div>

      {/* Thumbnail strip */}
      {!hideBottomThumbnails && (
        <PDFThumbnailStrip
          pageCount={pageCount}
          currentPage={currentPage}
          goToPage={goToPage}
        />
      )}
    </div>
  )
}
