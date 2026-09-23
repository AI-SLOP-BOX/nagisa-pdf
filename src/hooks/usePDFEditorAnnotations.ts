import { useState, useCallback } from 'react'
import { UserAnnotation } from '../services/annotationService'

interface UsePDFEditorAnnotationsProps {
  currentPage: number
  setEditorMode: (mode: 'inspect' | 'edit') => void
  setSelectedEditTool: (tool: string) => void
  showSuccess: (msg: string) => void
}

/**
 * Annotation undo/redo state kept in a SINGLE state object so every history
 * updater stays pure. (Side effects inside a `setState` updater are invoked
 * twice under React StrictMode and duplicated the undo stack.)
 */
interface AnnotationHistory {
  past: UserAnnotation[][]
  present: UserAnnotation[]
  future: UserAnnotation[][]
}

const MAX_HISTORY = 50

/** ページ番号とタイムスタンプ＋ランダム文字列で一意な注釈IDを生成する（ホットリロード対策）。 */
function generateAnnotationId(page: number): string {
  return `ann_${page}_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 9)}`
}

export function usePDFEditorAnnotations({
  currentPage,
  setEditorMode,
  setSelectedEditTool,
  showSuccess,
}: UsePDFEditorAnnotationsProps) {
  const [hist, setHist] = useState<AnnotationHistory>({ past: [], present: [], future: [] })
  const [selectedAnnotationId, setSelectedAnnotationId] = useState<string | null>(null)

  const annotations = hist.present

  const setAnnotationsWithHistory = useCallback((updater: (prev: UserAnnotation[]) => UserAnnotation[]) => {
    setHist(h => ({
      past: [...h.past, h.present].slice(-MAX_HISTORY),
      present: updater(h.present),
      future: [],
    }))
  }, [])

  /** Raw setter (no history entry) — kept for API compatibility. */
  const setAnnotations = useCallback(
    (next: UserAnnotation[] | ((prev: UserAnnotation[]) => UserAnnotation[])) => {
      setHist(h => ({
        ...h,
        present: typeof next === 'function' ? (next as (p: UserAnnotation[]) => UserAnnotation[])(h.present) : next,
      }))
    },
    [],
  )

  /** Full reset — used when a new document is loaded or annotations are flattened into bytes. */
  const resetAnnotations = useCallback(() => {
    setHist({ past: [], present: [], future: [] })
    setSelectedAnnotationId(null)
  }, [])

  const undoAnnotation = useCallback(() => {
    if (hist.past.length === 0) return false
    setHist(h =>
      h.past.length === 0
        ? h
        : { past: h.past.slice(0, -1), present: h.past[h.past.length - 1], future: [h.present, ...h.future] },
    )
    return true
  }, [hist.past.length, hist.present])

  const redoAnnotation = useCallback(() => {
    if (hist.future.length === 0) return false
    setHist(h =>
      h.future.length === 0
        ? h
        : { past: [...h.past, h.present], present: h.future[0], future: h.future.slice(1) },
    )
    return true
  }, [hist.future.length, hist.present])

  const handleAddTextAnnotation = useCallback(() => {
    const newAnn: UserAnnotation = {
      id: generateAnnotationId(currentPage),
      page: currentPage,
      type: 'text',
      x: 80,
      y: 120,
      width: 200,
      height: 36,
      text: '新しいテキスト',
      fontSize: 18,
      fontFamily: 'Inter, sans-serif',
      color: '#0f172a',
      isBold: false,
      isItalic: false,
    }
    setAnnotationsWithHistory(prev => [...prev, newAnn])
    setSelectedAnnotationId(newAnn.id)
    setEditorMode('edit')
    setSelectedEditTool('text')
    showSuccess('テキストボックスを追加しました')
  }, [currentPage, setAnnotationsWithHistory, setEditorMode, setSelectedEditTool, showSuccess])

  const handleAddHighlightAnnotation = useCallback(() => {
    const newAnn: UserAnnotation = {
      id: generateAnnotationId(currentPage),
      page: currentPage,
      type: 'highlight',
      x: 80,
      y: 160,
      width: 220,
      height: 24,
    }
    setAnnotationsWithHistory(prev => [...prev, newAnn])
    setSelectedAnnotationId(newAnn.id)
    setEditorMode('edit')
    setSelectedEditTool('highlight')
    showSuccess('ハイライトを追加しました')
  }, [currentPage, setAnnotationsWithHistory, setEditorMode, setSelectedEditTool, showSuccess])

  const handleAddShapeAnnotation = useCallback(() => {
    const newAnn: UserAnnotation = {
      id: generateAnnotationId(currentPage),
      page: currentPage,
      type: 'shape',
      x: 80,
      y: 180,
      width: 180,
      height: 90,
    }
    setAnnotationsWithHistory(prev => [...prev, newAnn])
    setSelectedAnnotationId(newAnn.id)
    setEditorMode('edit')
    setSelectedEditTool('shape')
    showSuccess('四角形枠を追加しました')
  }, [currentPage, setAnnotationsWithHistory, setEditorMode, setSelectedEditTool, showSuccess])

  const handleUpdateAnnotation = useCallback((id: string, updates: Partial<UserAnnotation>) => {
    setAnnotationsWithHistory(prev => prev.map(a => (a.id === id ? { ...a, ...updates } : a)))
  }, [setAnnotationsWithHistory])

  const handleUpdateSelectedAnnotation = useCallback((updates: Partial<UserAnnotation>) => {
    if (!selectedAnnotationId) return
    setAnnotationsWithHistory(prev => prev.map(a => (a.id === selectedAnnotationId ? { ...a, ...updates } : a)))
  }, [selectedAnnotationId, setAnnotationsWithHistory])

  const handleDeleteSelectedAnnotation = useCallback(() => {
    if (!selectedAnnotationId) return
    setAnnotationsWithHistory(prev => prev.filter(a => a.id !== selectedAnnotationId))
    setSelectedAnnotationId(null)
    showSuccess('要素を削除しました')
  }, [selectedAnnotationId, setAnnotationsWithHistory, showSuccess])

  const handleDeleteAnnotation = useCallback((id: string) => {
    setAnnotationsWithHistory(prev => prev.filter(a => a.id !== id))
    setSelectedAnnotationId(curr => (curr === id ? null : curr))
  }, [setAnnotationsWithHistory])

  const handleAddAnnotation = useCallback((ann: UserAnnotation) => {
    setAnnotationsWithHistory(prev => [...prev, ann])
  }, [setAnnotationsWithHistory])

  return {
    annotations,
    setAnnotations,
    resetAnnotations,
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
    canUndoAnnotation: hist.past.length > 0,
    canRedoAnnotation: hist.future.length > 0,
    undoAnnotation,
    redoAnnotation,
  }
}

