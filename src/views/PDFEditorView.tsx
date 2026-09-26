import { useState, useCallback, useEffect, useMemo, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import PDFViewer, { InteractiveMode, TextBlock } from '../components/PDFViewer'
import { CommandPalette } from '../components/CommandPalette'
import { EditorTopBar } from '../components/EditorTopBar'
import { EditorThumbnailSidebar } from '../components/EditorThumbnailSidebar'
import { EditorRightInspector } from '../components/EditorRightInspector'
import { EditorEmptyDropZone } from '../components/EditorEmptyDropZone'
import { SignatureVerificationModal } from '../components/SignatureVerificationModal'
import { SecurityPanel } from '../components/SecurityPanel'
import { EditPanel } from '../components/EditPanel'
import { AnnotatePanel } from '../components/AnnotatePanel'
import { FormCreatorPanel } from '../components/FormCreatorPanel'
import { OrganizePanel } from '../components/OrganizePanel'
import { PagesPanel } from '../components/PagesPanel'
import { TextEditPanel } from '../components/TextEditPanel'
import { ToolsPanel } from '../components/ToolsPanel'
import {
  EditIcon, AnnotateIcon, FormIcon, OrganizeIcon,
  FileIcon, LockIcon, TypeIcon, ToolsIcon,
} from '../components/Icons'
import type { SignatureInfo } from '../types'
import { useHistory } from '../hooks/useHistory'
import { useToast } from '../hooks/useToast'
import { formatError } from '../utils/errorHandler'
import { useT } from '../utils/i18n'

import { DocumentService } from '../services/documentService'
import { AnnotationService, UserAnnotation } from '../services/annotationService'
import { usePDFEditorAnnotations } from '../hooks/usePDFEditorAnnotations'
import { buildEditorCommandItems } from '../components/editorCommands'

import type { EditorTab } from '../types'

type Tab = EditorTab

import type { View } from '../types'

interface PDFEditorViewProps {
  currentView?: View
  onNavigateView?: (view: View) => void
  initialFile?: { bytes: number[]; name: string } | null
  initialTab?: Tab | null
  onOpenStart?: (name: string, size?: string) => void
}

const DEFAULT_ANNOTATION_COLOR = '#FF0000'
const DEFAULT_STROKE_WIDTH = 2
const DEFAULT_REDACT_COLOR = '#000000'

export default function PDFEditorView({ currentView, onNavigateView, initialFile, initialTab, onOpenStart }: PDFEditorViewProps) {
  const { t } = useT()
  const { data: pdfData, setData: setPdfData, pushHistory, resetHistory, undo: fallbackUndo, redo: fallbackRedo, canUndo: fallbackCanUndo, canRedo: fallbackCanRedo } = useHistory(null, 30)
  const { toast, toastType, showToast, showError, showSuccess } = useToast(2800)

  const [docId, setDocId] = useState<string | null>(null)
  const docIdRef = useRef<string | null>(null)
  docIdRef.current = docId

  const [revision, setRevision] = useState(0)
  const [sessionCanUndo, setSessionCanUndo] = useState(false)
  const [sessionCanRedo, setSessionCanRedo] = useState(false)

  const [fileName, setFileName] = useState('')
  const [pageCount, setPageCount] = useState(0)
  const [currentPage, setCurrentPage] = useState(0)
  const [activeTab, setActiveTab] = useState<Tab | null>(initialTab || 'edit')

  // Light Mode Layout State
  const [editorMode, setEditorMode] = useState<'inspect' | 'edit'>('inspect')
  const [isThumbnailsCollapsed, setIsThumbnailsCollapsed] = useState(false)
  const [isInspectorCollapsed, setIsInspectorCollapsed] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [zoom, setZoom] = useState(1.0)
  const [selectedEditTool, setSelectedEditTool] = useState('select')

  // Panel-local state persisted across tab switches (restored from the original tab drawer design)
  const [annotationColor, setAnnotationColor] = useState(DEFAULT_ANNOTATION_COLOR)
  const [stickyNoteText, setStickyNoteText] = useState('')
  const [strokeWidth, setStrokeWidth] = useState(DEFAULT_STROKE_WIDTH)
  const [watermarkText, setWatermarkText] = useState('CONFIDENTIAL')
  const [watermarkOpacity, setWatermarkOpacity] = useState(0.3)
  const [watermarkRotation, setWatermarkRotation] = useState(-45)
  const [watermarkFontSize, setWatermarkFontSize] = useState(48)
  const [watermarkColor, setWatermarkColor] = useState('#808080')
  const [headerText, setHeaderText] = useState('')
  const [footerText, setFooterText] = useState('Page {page} of {total}')
  const [hfFontSize, setHfFontSize] = useState(10)
  const [batesPrefix, setBatesPrefix] = useState('DOC')
  const [batesStart, setBatesStart] = useState(1)
  const [batesFontSize, setBatesFontSize] = useState(10)
  const [redactColor, setRedactColor] = useState(DEFAULT_REDACT_COLOR)
  const [redactSearchText, setRedactSearchText] = useState('')
  const [redactReplacement, setRedactReplacement] = useState('')

  // User Annotations & In-Place Editing Hook
  const {
    annotations,
    setAnnotations,
    selectedAnnotationId,
    setSelectedAnnotationId,
    handleAddTextAnnotation,
    handleAddHighlightAnnotation,
    handleAddShapeAnnotation,
    handleUpdateAnnotation,
    handleUpdateSelectedAnnotation,
    handleDeleteSelectedAnnotation,
    handleDeleteAnnotation,
    handleAddAnnotation,
    canUndoAnnotation,
    canRedoAnnotation,
    undoAnnotation,
    redoAnnotation,
    resetAnnotations,
  } = usePDFEditorAnnotations({
    currentPage,
    setEditorMode,
    setSelectedEditTool,
    showSuccess,
  })

  // Refresh history capability status whenever revision or docId changes
  const refreshHistoryStatus = useCallback(async (activeId: string) => {
    try {
      const status = await DocumentService.getHistoryStatus(activeId)
      setSessionCanUndo(status.can_undo)
      setSessionCanRedo(status.can_redo)
    } catch {
      setSessionCanUndo(false)
      setSessionCanRedo(false)
    }
  }, [])

  // Cleanup session ONLY when component truly unmounts
  useEffect(() => {
    return () => {
      if (docIdRef.current) {
        DocumentService.closeSession(docIdRef.current).catch((err) => console.debug('セッションのクローズに失敗:', err))
      }
    }
  }, [])

  // Interactive canvas state
  const [interactiveMode, setInteractiveMode] = useState<InteractiveMode>('view')
  const [selectedTextBlock, setSelectedTextBlock] = useState<TextBlock | null>(null)

  // Signature verification inspection modal
  const [verifiedSignatures, setVerifiedSignatures] = useState<SignatureInfo[] | null>(null)

  // OCR on screenshot/scanned image PDF
  const [triggerOCR, setTriggerOCR] = useState(0)
  const handleRunOCR = useCallback(() => {
    setTriggerOCR(c => c + 1)
    showToast('画像からテキスト領域をスキャン中...')
  }, [showToast])
  const handleOCRComplete = useCallback((count: number) => {
    if (count > 0) {
      showSuccess(`${count}箇所のテキストを検出・直接編集可能にしました`)
      setEditorMode('edit')
      setSelectedEditTool('select')
    } else {
      showToast('テキスト領域が見つかりませんでした')
    }
  }, [showSuccess, showToast])

  const loadPdfFromBytes = useCallback(async (bytes: number[], name: string) => {
    try {
      if (docIdRef.current) {
        await DocumentService.closeSession(docIdRef.current).catch((err) => console.debug('セッションのクローズに失敗:', err))
      }
      let newDocId: string | null = null
      try {
        newDocId = await DocumentService.createSession(bytes)
      } catch (sessionErr) {
        // Running in web browser preview without Tauri Rust backend IPC
        console.warn('Tauri Rust backend not connected, continuing in browser preview mode', sessionErr)
        newDocId = 'browser-session-' + Date.now()
      }
      setDocId(newDocId)
      setRevision(r => r + 1)
      setPdfData(bytes)
      setFileName(name)
      // New document ⇒ discard previous document's undo history and overlay annotations
      resetHistory(bytes)
      resetAnnotations()
      if (newDocId && !newDocId.startsWith('browser-session-')) {
        await refreshHistoryStatus(newDocId)
      } else {
        setSessionCanUndo(false)
        setSessionCanRedo(false)
      }
      showSuccess(t().pdfLoaded)
    } catch (err) {
      showError(formatError(err, 'PDFの読み込みに失敗しました'))
    }
  }, [pushHistory, resetHistory, resetAnnotations, refreshHistoryStatus, setPdfData, showError, showSuccess])

  // Load initialFile if provided from HomeView or caller
  useEffect(() => {
    if (initialFile && initialFile.bytes && initialFile.bytes.length > 0) {
      loadPdfFromBytes(initialFile.bytes, initialFile.name)
    }
  }, [initialFile, loadPdfFromBytes])

  // Apply initialTab if provided
  useEffect(() => {
    if (initialTab) {
      setActiveTab(initialTab)
    }
  }, [initialTab])

  // TopBar Handlers
  const handleOpen = useCallback(async () => {
    try {
      const file = await DocumentService.openFileDialog()
      if (file) {
        onOpenStart?.(file.name)
        await loadPdfFromBytes(file.bytes, file.name)
      }
    } catch (err) {
      showError(formatError(err, 'PDFの読み込みに失敗しました'))
    }
  }, [loadPdfFromBytes, onOpenStart, showError])

  const handleSave = useCallback(async () => {
    if (!pdfData) {
      showToast(t().needFileOpen)
      return
    }

    try {
      // If user added annotations, burn them into PDF bytes using client AnnotationService
      let bytesToSave = pdfData
      if (annotations.length > 0) {
        const burnedUint8 = await AnnotationService.applyAnnotations(bytesToSave, annotations)
        bytesToSave = Array.from(burnedUint8)
        setPdfData(bytesToSave)
        pushHistory(bytesToSave)
        setRevision(r => r + 1)
        // Keep the Rust session in sync, otherwise the next exec/undo
        // operates on pre-burn bytes and annotations are lost (or burned twice).
        if (docId && !docId.startsWith('browser-session-')) {
          await DocumentService.updateSessionBytes(docId, '注釈を画像化', bytesToSave)
            .catch(err => console.warn('Failed to sync burned bytes to session:', err))
          await refreshHistoryStatus(docId)
        }
        // Annotations are now flattened into the bytes — clear the overlay
        // so a second save does not burn them again (double burn-in bug).
        resetAnnotations()
      }

      // Try native save dialog first
      let saved = false
      try {
        const path = await DocumentService.saveFileDialog(fileName || 'document.pdf', bytesToSave)
        if (path) saved = true
      } catch {
        // Fallback to browser direct download
      }

      if (!saved) {
        AnnotationService.downloadPdf(new Uint8Array(bytesToSave), fileName || 'edited_document.pdf')
      }
      showSuccess('PDFを保存しました（編集内容を反映）')
    } catch (err) {
      showError(formatError(err, 'PDFの保存に失敗しました'))
    }
  }, [pdfData, annotations, fileName, showToast, showError, showSuccess, setPdfData, pushHistory, resetAnnotations, docId, refreshHistoryStatus])

  const handleUndo = useCallback(async () => {
    // 1. If there is an in-flight annotation history step, undo that first
    if (canUndoAnnotation) {
      undoAnnotation()
      showSuccess('直前の注釈編集を取り消しました')
      return
    }

    // 2. Otherwise undo document-level session operation in Rust backend
    const nativeDocId = docId && !docId.startsWith('browser-session-') ? docId : null
    if (nativeDocId) {
      try {
        const ok = await DocumentService.undo(nativeDocId)
        if (ok) {
          const currentBytes = await DocumentService.getSessionBytes(nativeDocId).catch(() => null)
          if (currentBytes) setPdfData(currentBytes)
          setRevision(r => r + 1)
          await refreshHistoryStatus(nativeDocId)
          showSuccess(t().undo)
        }
      } catch (err) {
        showError(formatError(err, 'Undoに失敗しました'))
      }
    } else {
      fallbackUndo()
    }
  }, [canUndoAnnotation, undoAnnotation, docId, fallbackUndo, refreshHistoryStatus, setPdfData, showError, showSuccess])

  const handleRedo = useCallback(async () => {
    if (canRedoAnnotation) {
      redoAnnotation()
      showSuccess('直前の注釈編集をやり直しました')
      return
    }

    const nativeDocId = docId && !docId.startsWith('browser-session-') ? docId : null
    if (nativeDocId) {
      try {
        const ok = await DocumentService.redo(nativeDocId)
        if (ok) {
          const currentBytes = await DocumentService.getSessionBytes(nativeDocId).catch(() => null)
          if (currentBytes) setPdfData(currentBytes)
          setRevision(r => r + 1)
          await refreshHistoryStatus(nativeDocId)
          showSuccess(t().redo)
        }
      } catch (err) {
        showError(formatError(err, 'Redoに失敗しました'))
      }
    } else {
      fallbackRedo()
    }
  }, [canRedoAnnotation, redoAnnotation, docId, fallbackRedo, refreshHistoryStatus, setPdfData, showError, showSuccess])

  // browser-session-* ids have no Rust session behind them — always use the local fallback there
  const hasNativeSession = !!docId && !docId.startsWith('browser-session-')
  const canUndo = canUndoAnnotation || (hasNativeSession ? sessionCanUndo : fallbackCanUndo)
  const canRedo = canRedoAnnotation || (hasNativeSession ? sessionCanRedo : fallbackCanRedo)

  const handlePrint = useCallback(async () => {
    const target = (docId && !docId.startsWith('browser-session-')) ? docId : pdfData
    if (!target) return
    try {
      await DocumentService.printPdf(target)
    } catch (err) {
      showError(formatError(err, '印刷に失敗しました'))
    }
  }, [docId, pdfData, showError])

  const [isCommandPaletteOpen, setIsCommandPaletteOpen] = useState(false)

  // Keyboard shortcuts (including single-key tool switches when not typing in inputs)
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Ignore single-key shortcuts when focus is in an input, textarea, or contentEditable
      const target = e.target as HTMLElement | null
      const isInput =
        target &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.isContentEditable)

      if (e.metaKey || e.ctrlKey) {
        if (e.key === 'k' || e.key === 'K') {
          e.preventDefault()
          setIsCommandPaletteOpen(prev => !prev)
          return
        }
        if (e.key === 'o') { e.preventDefault(); handleOpen() }
        if (e.key === 's') { e.preventDefault(); handleSave() }
        if (e.key === 'p') { e.preventDefault(); handlePrint() }
        if (e.key === 'z' && !e.shiftKey) { e.preventDefault(); handleUndo() }
        if ((e.key === 'z' && e.shiftKey) || e.key === 'y') { e.preventDefault(); handleRedo() }
        return
      }

      if (!isInput) {
        if (e.key === 'Backspace' || e.key === 'Delete') {
          if (selectedAnnotationId) {
            e.preventDefault()
            handleDeleteSelectedAnnotation()
            return
          }
        }

        const k = e.key.toLowerCase()
        if (k === 'v') {
          setSelectedEditTool('select')
          setEditorMode('edit')
        } else if (k === 't') {
          setSelectedEditTool('text')
          setEditorMode('edit')
          handleAddTextAnnotation()
        } else if (k === 'h') {
          setSelectedEditTool('highlight')
          setEditorMode('edit')
          handleAddHighlightAnnotation()
        } else if (k === 'a') {
          setSelectedEditTool('annotate')
          setEditorMode('edit')
        } else if (k === 'r') {
          setSelectedEditTool('shape')
          setEditorMode('edit')
          handleAddShapeAnnotation()
        } else if (k === 'p') {
          setSelectedEditTool('pages')
          setActiveTab('organize')
        }
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [
    handleOpen,
    handleSave,
    handlePrint,
    handleUndo,
    handleRedo,
    selectedAnnotationId,
    handleDeleteSelectedAnnotation,
    setSelectedEditTool,
    setEditorMode,
    handleAddTextAnnotation,
    handleAddHighlightAnnotation,
    handleAddShapeAnnotation,
    setActiveTab,
  ])

  const exec = useCallback(async (cmd: string, args: Record<string, unknown>) => {
    if (!docId && !pdfData) {
      showToast(t().needFileOpen)
      return
    }
    try {
      const nativeDocId = docId && !docId.startsWith('browser-session-') ? docId : null
      if (nativeDocId) {
        // Fast paths for rotation and page deletion using DocumentSession
        if (cmd === 'rotate_page') {
          const pIdx = (args.pageIndex as number) ?? currentPage
          const deg = (args.degrees as number) ?? 90
          await DocumentService.rotatePage(nativeDocId, pIdx, deg)
          const currentBytes = await DocumentService.getSessionBytes(nativeDocId).catch(() => null)
          if (currentBytes) {
            setPdfData(currentBytes)
            pushHistory(currentBytes)
          }
          setRevision(r => r + 1)
          await refreshHistoryStatus(nativeDocId)
          showSuccess(t().completed)
          return
        } else if (cmd === 'delete_page') {
          const pIdx = (args.pageIndex as number) ?? currentPage
          await DocumentService.deletePage(nativeDocId, pIdx)
          const currentBytes = await DocumentService.getSessionBytes(nativeDocId).catch(() => null)
          if (currentBytes) {
            setPdfData(currentBytes)
            pushHistory(currentBytes)
          }
          setRevision(r => r + 1)
          await refreshHistoryStatus(nativeDocId)
          showSuccess(t().completed)
          return
        }
      }

      // For standard command tools, run against current session bytes and update session in-place
      const currentBytes = nativeDocId ? await DocumentService.getSessionBytes(nativeDocId) : (pdfData as number[])
      const result = await invoke<number[]>(cmd, { data: currentBytes, ...args })

      if (nativeDocId) {
        // Update session in-place with FullSnapshot so Undo/Redo stack is preserved!
        await DocumentService.updateSessionBytes(nativeDocId, `Command ${cmd}`, result)
        setRevision(r => r + 1)
        await refreshHistoryStatus(nativeDocId)
      } else {
        setRevision(r => r + 1)
      }
      pushHistory(result)
      showSuccess(t().completed)
    } catch (err) {
      showError(formatError(err, 'コマンド実行に失敗しました'))
    }
  }, [docId, pdfData, currentPage, pushHistory, refreshHistoryStatus, showError, showSuccess, showToast, setPdfData])

  // Canvas Rect draw handler
  const handleDrawRectComplete = useCallback(async (rect: { x: number; y: number; width: number; height: number; page: number }) => {
    if (!pdfData) return
    try {
      if (interactiveMode === 'draw-redact') {
        await exec('redact_area', {
          pageIndex: rect.page,
          x: rect.x,
          y: rect.y,
          width: rect.width,
          height: rect.height,
          color: DEFAULT_REDACT_COLOR,
        })
        setInteractiveMode('view')
        showSuccess(t().redactApplied)
      } else if (interactiveMode === 'draw-highlight') {
        await exec('add_highlight', {
          pageIndex: rect.page,
          x: rect.x,
          y: rect.y,
          width: rect.width,
          height: rect.height,
          color: DEFAULT_ANNOTATION_COLOR,
        })
        setInteractiveMode('view')
        showSuccess(t().highlightAdded)
      } else if (interactiveMode === 'draw-rect') {
        await exec('add_rectangle', {
          pageIndex: rect.page,
          x: rect.x,
          y: rect.y,
          width: rect.width,
          height: rect.height,
          strokeColor: DEFAULT_ANNOTATION_COLOR,
          fillColor: '#FFFFFF00',
          strokeWidth: DEFAULT_STROKE_WIDTH,
        })
        setInteractiveMode('view')
        showSuccess(t().rectAdded)
      }
    } catch (err) {
      showError(formatError(err, '注釈の追加に失敗しました'))
    }
  }, [pdfData, interactiveMode, exec, showError, showSuccess])

  // Move text block handler from canvas drag with full Session sync & Undo preservation
  const handleMoveTextBlock = useCallback(async (blockId: number, newX: number, newY: number) => {
    if (!docId && !pdfData) return
    try {
      const nativeDocId = docId && !docId.startsWith('browser-session-') ? docId : null
      const currentBytes = nativeDocId ? await DocumentService.getSessionBytes(nativeDocId) : (pdfData as number[])
      const result = await invoke<number[]>('move_text_block', {
        data: currentBytes,
        pageIndex: currentPage,
        blockId: blockId,
        newX: newX,
        newY: newY,
      })
      if (nativeDocId) {
        await DocumentService.updateSessionBytes(nativeDocId, `Move text block #${blockId}`, result)
        setRevision(r => r + 1)
        await refreshHistoryStatus(nativeDocId)
      } else {
        setRevision(r => r + 1)
      }
      pushHistory(result)
      showSuccess(t().textMoved(blockId, newX, newY))
    } catch (err) {
      showError(formatError(err, 'テキスト移動に失敗しました'))
    }
  }, [docId, pdfData, currentPage, pushHistory, refreshHistoryStatus, showError, showSuccess, setPdfData])

  // Switch interactiveMode when changing tabs (toggle to collapse if already active)
  const handleSelectTab = (tab: Tab) => {
    if (activeTab === tab) {
      setActiveTab(null)
      setInteractiveMode('view')
      return
    }
    setActiveTab(tab)
    if (tab === 'text') {
      setInteractiveMode('select-text')
    } else {
      setInteractiveMode('view')
    }
  }

  // Handle PDF byte update from child components (Forms/TextEdit/Tools/Overlay) with session sync
  const handlePdfUpdate = useCallback(async (data: number[]) => {
    if (docId) {
      try {
        await DocumentService.updateSessionBytes(docId, 'Edit Document', data)
        setRevision(r => r + 1)
        await refreshHistoryStatus(docId)
      } catch (err) {
        console.error('Failed to sync updated bytes to session:', err)
        showError(formatError(err, 'セッションの同期に失敗しました'))
        throw err
      }
    }
    pushHistory(data)
  }, [docId, pushHistory, refreshHistoryStatus, showError])

  // Command items for ⌘K Quick Launcher
  const commandItems = useMemo(() => {
    return buildEditorCommandItems({
      onOpen: handleOpen,
      onSave: handleSave,
      onSelectTab: handleSelectTab,
      onSetInteractiveMode: setInteractiveMode,
      onExec: exec,
      hasPdf: !!pdfData,
    })
  }, [handleOpen, handleSave, exec, pdfData])

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', background: '#f8fafc', width: '100%', overflow: 'hidden' }}>
      {toast && (
        <div style={{
          position: 'fixed', top: 16, right: 16, zIndex: 1000,
          background: '#ffffff',
          border: `1px solid ${toastType === 'error' ? '#ef4444' : '#2563eb'}`,
          color: toastType === 'error' ? '#ef4444' : '#2563eb',
          padding: '10px 20px', borderRadius: 10,
          boxShadow: '0 4px 20px rgba(0,0,0,0.12)', fontSize: 13,
          fontWeight: 600,
          maxWidth: 480,
          wordBreak: 'break-word',
          whiteSpace: 'pre-wrap',
          lineHeight: 1.4,
        }}>
          {toast}
        </div>
      )}

      {/* Signature Verification Dialog */}
      {verifiedSignatures && (
        <SignatureVerificationModal
          signatures={verifiedSignatures}
          onClose={() => setVerifiedSignatures(null)}
        />
      )}

      {/* Top Document Tab & Action Bar */}
      <EditorTopBar
        fileName={fileName}
        pageCount={pageCount}
        currentPage={currentPage}
        onPageChange={setCurrentPage}
        zoom={zoom}
        onZoomChange={setZoom}
        onOpen={handleOpen}
        onSave={handleSave}
        onPrint={handlePrint}
        onUndo={handleUndo}
        onRedo={handleRedo}
        canUndo={canUndo}
        canRedo={canRedo}
        editorMode={editorMode}
        setEditorMode={setEditorMode}
        searchQuery={searchQuery}
        setSearchQuery={setSearchQuery}
        onCloseDocument={() => onNavigateView?.('home')}
        onNewTab={handleOpen}
        selectedEditTool={selectedEditTool}
        setSelectedEditTool={setSelectedEditTool}
        onRunOCR={handleRunOCR}
      />

      {/* Editor Tab Strip (contextual tool categories) */}
      {pdfData && (
        <div style={{
          display: 'flex', gap: 2, padding: '4px 10px',
          background: 'var(--bg-1)', borderBottom: '1px solid var(--border)',
          flexShrink: 0, overflowX: 'auto',
        }}>
          {([
            ['edit', <EditIcon size={14} />, t().tabEdit],
            ['annotate', <AnnotateIcon size={14} />, t().tabAnnotate],
            ['forms', <FormIcon size={14} />, t().tabForms],
            ['organize', <OrganizeIcon size={14} />, t().tabOrganize],
            ['pages', <FileIcon size={14} />, t().tabPages],
            ['security', <LockIcon size={14} />, t().tabSecurity],
            ['text', <TypeIcon size={14} />, t().tabText],
            ['tools', <ToolsIcon size={14} />, t().tabTools],
          ] as const).map(([tab, icon, label]) => (
            <button
              key={tab}
              onClick={() => handleSelectTab(tab)}
              className={`editor-tab-btn ${activeTab === tab ? 'active' : ''}`}
            >
              <span style={{ display: 'flex', alignItems: 'center' }}>{icon}</span>
              <span>{label}</span>
            </button>
          ))}
        </div>
      )}

      {/* Main 3-Column Workspace */}
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden', width: '100%', background: '#eef2f6' }}>
        {/* Left Thumbnail Sidebar (only when PDF is loaded) */}
        {pdfData && (
          <EditorThumbnailSidebar
            pageCount={pageCount}
            currentPage={currentPage}
            onSelectPage={setCurrentPage}
            collapsed={isThumbnailsCollapsed}
            onToggleCollapse={() => setIsThumbnailsCollapsed(!isThumbnailsCollapsed)}
            pdfData={pdfData}
            docId={docId}
          />
        )}

        {/* Contextual Tool Drawer (tab panel) */}
        {pdfData && activeTab && (
          <div style={{
            width: 290, background: 'var(--bg-1)', borderRight: '1px solid var(--border)',
            padding: 12, overflowY: 'auto', flexShrink: 0,
          }}>
            {activeTab === 'edit' && (
              <EditPanel exec={exec} pdfData={pdfData} showToast={showToast} currentPage={currentPage} />
            )}
            {activeTab === 'annotate' && (
              <AnnotatePanel
                exec={exec}
                pdfData={pdfData}
                docId={docId}
                annotationColor={annotationColor}
                setAnnotationColor={setAnnotationColor}
                stickyNoteText={stickyNoteText}
                setStickyNoteText={setStickyNoteText}
                strokeWidth={strokeWidth}
                setStrokeWidth={setStrokeWidth}
                onActivateDraw={mode => setInteractiveMode(mode)}
                currentPage={currentPage}
              />
            )}
            {activeTab === 'forms' && (
              <FormCreatorPanel
                pdfData={pdfData}
                docId={docId}
                currentPage={currentPage}
                exec={exec}
                showToast={showToast}
                onPdfUpdate={handlePdfUpdate}
              />
            )}
            {activeTab === 'organize' && (
              <OrganizePanel exec={exec} />
            )}
            {activeTab === 'pages' && (
              <PagesPanel
                watermarkText={watermarkText} setWatermarkText={setWatermarkText}
                watermarkOpacity={watermarkOpacity} setWatermarkOpacity={setWatermarkOpacity}
                watermarkRotation={watermarkRotation} setWatermarkRotation={setWatermarkRotation}
                watermarkFontSize={watermarkFontSize} setWatermarkFontSize={setWatermarkFontSize}
                watermarkColor={watermarkColor} setWatermarkColor={setWatermarkColor}
                headerText={headerText} setHeaderText={setHeaderText}
                footerText={footerText} setFooterText={setFooterText}
                hfFontSize={hfFontSize} setHfFontSize={setHfFontSize}
                batesPrefix={batesPrefix} setBatesPrefix={setBatesPrefix}
                batesStart={batesStart} setBatesStart={setBatesStart}
                batesFontSize={batesFontSize} setBatesFontSize={setBatesFontSize}
                exec={exec}
              />
            )}
            {activeTab === 'security' && (
              <SecurityPanel
                exec={exec}
                pdfData={pdfData}
                docId={docId}
                showToast={showToast}
                onInspectSignatures={(sigs: SignatureInfo[]) => setVerifiedSignatures(sigs)}
                onPdfUpdate={handlePdfUpdate}
              />
            )}
            {activeTab === 'text' && (
              <TextEditPanel
                pdfData={pdfData}
                docId={docId}
                exec={exec}
                showToast={showToast}
                onPdfUpdate={handlePdfUpdate}
                selectedBlockFromCanvas={selectedTextBlock}
                currentPage={currentPage}
              />
            )}
            {activeTab === 'tools' && (
              <ToolsPanel
                exec={exec}
                pdfData={pdfData}
                docId={docId}
                redactColor={redactColor} setRedactColor={setRedactColor}
                redactSearchText={redactSearchText} setRedactSearchText={setRedactSearchText}
                redactReplacement={redactReplacement} setRedactReplacement={setRedactReplacement}
                showToast={showToast}
                onActivateDrawRedact={() => setInteractiveMode('draw-redact')}
                onPdfUpdate={handlePdfUpdate}
              />
            )}
          </div>
        )}

        {/* Center Canvas / Desk Area */}
        <div style={{ flex: 1, display: 'flex', overflow: 'hidden', position: 'relative' }}>
          {pdfData ? (
            <PDFViewer
              pdfData={pdfData}
              docId={docId}
              revision={revision}
              currentPage={currentPage}
              onPageCountChange={setPageCount}
              onPageChange={setCurrentPage}
              interactiveMode={interactiveMode}
              selectedTextBlockId={selectedTextBlock?.id}
              onSelectTextBlock={setSelectedTextBlock}
              onMoveTextBlock={handleMoveTextBlock}
              onDrawRectComplete={handleDrawRectComplete}
              onPdfUpdate={handlePdfUpdate}
              hideToolbar={true}
              hideBottomThumbnails={true}
              hideFloatingHUD={true}
              editorMode={editorMode}
              selectedEditTool={selectedEditTool}
              annotations={annotations}
              selectedAnnotationId={selectedAnnotationId}
              onSelectAnnotation={setSelectedAnnotationId}
              onUpdateAnnotation={handleUpdateAnnotation}
              onAddAnnotation={handleAddAnnotation}
              onDeleteAnnotation={handleDeleteAnnotation}
              triggerOCR={triggerOCR}
              onOCRComplete={handleOCRComplete}
            />
          ) : (
            <EditorEmptyDropZone
              onOpen={handleOpen}
              onNavigateHome={onNavigateView ? () => onNavigateView('home') : undefined}
              onLoadFileBytes={loadPdfFromBytes}
              onOpenStart={onOpenStart}
              onError={msg => showError(msg)}
            />
          )}
        </div>

        {/* Right Inspector Panel (only when PDF is loaded) */}
        {pdfData && (
          <EditorRightInspector
            fileName={fileName}
            pageCount={pageCount}
            fileSize={pdfData && pdfData.length > 0 ? `${(pdfData.length / 1024 / 1024).toFixed(1)} MB` : undefined}
            docId={docId}
            editorMode={editorMode}
            collapsed={isInspectorCollapsed}
            onToggleCollapse={() => setIsInspectorCollapsed(!isInspectorCollapsed)}
            onNavigateView={onNavigateView}
            onActivateAnnotate={() => {
              setInteractiveMode('draw-highlight')
              setEditorMode('edit')
              setSelectedEditTool('highlight')
            }}
            selectedAnnotation={annotations.find(a => a.id === selectedAnnotationId)}
            onUpdateAnnotation={handleUpdateSelectedAnnotation}
            onAddTextAnnotation={handleAddTextAnnotation}
            onAddHighlightAnnotation={handleAddHighlightAnnotation}
            onAddShapeAnnotation={handleAddShapeAnnotation}
            onDeleteSelectedAnnotation={handleDeleteSelectedAnnotation}
          />
        )}
      </div>

      {/* ⌘K Command Palette Modal */}
      <CommandPalette
        isOpen={isCommandPaletteOpen}
        onClose={() => setIsCommandPaletteOpen(false)}
        commands={commandItems}
      />
    </div>
  )
}
