import React, { useState, useEffect, useRef } from 'react'
import { PDFJsEngine } from '../services/pdfRenderer'
import { invoke } from '@tauri-apps/api/core'
import { safeRevokeObjectUrl } from '../utils/objectUrl'

interface EditorThumbnailSidebarProps {
  pageCount: number
  currentPage: number
  onSelectPage: (page: number) => void
  collapsed: boolean
  onToggleCollapse: () => void
  pdfData?: number[] | null
  docId?: string | null
}

export const EditorThumbnailSidebar: React.FC<EditorThumbnailSidebarProps> = ({
  pageCount,
  currentPage,
  onSelectPage,
  collapsed,
  onToggleCollapse,
  pdfData,
  docId,
}) => {
  const [thumbnails, setThumbnails] = useState<Map<number, string>>(new Map())
  const [detectedPageCount, setDetectedPageCount] = useState<number>(0)
  const blobUrlsRef = useRef<Set<string>>(new Set())

  // Revoke any still-tracked blob URLs on unmount
  useEffect(() => {
    return () => {
      for (const url of blobUrlsRef.current) {
        safeRevokeObjectUrl(url)
      }
      blobUrlsRef.current.clear()
    }
  }, [])

  // Prune blob URLs that are no longer referenced by the active thumbnail map.
  // Runs after `thumbnails` state has committed so the DOM never points at a revoked URL.
  useEffect(() => {
    const activeUrls = new Set(thumbnails.values())
    for (const url of Array.from(blobUrlsRef.current)) {
      if (!activeUrls.has(url)) {
        safeRevokeObjectUrl(url)
        blobUrlsRef.current.delete(url)
      }
    }
  }, [thumbnails])

  // Generate real thumbnail images for all pages
  useEffect(() => {
    let isCancelled = false
    const sourceBytes = (pdfData && pdfData.length > 0) ? pdfData : PDFJsEngine.getLatestBytes()

    const loadRealThumbnails = async () => {
      const newMap = new Map<number, string>()

      // 1. Try PDF.js client rendering (works in browser & preview)
      if (sourceBytes && sourceBytes.length > 0) {
        try {
          const doc = await PDFJsEngine.getDocument(sourceBytes)
          const total = Math.max(pageCount, doc.numPages)
          if (!isCancelled) setDetectedPageCount(total)

          for (let i = 0; i < total; i++) {
            if (isCancelled) return
            try {
              const page = await doc.getPage(i + 1)
              const vp = page.getViewport({ scale: 0.18 }) // ~100px width for A4
              const canvas = document.createElement('canvas')
              canvas.width = Math.max(1, Math.round(vp.width))
              canvas.height = Math.max(1, Math.round(vp.height))
              const ctx = canvas.getContext('2d')
              if (ctx) {
                ctx.fillStyle = '#ffffff'
                ctx.fillRect(0, 0, canvas.width, canvas.height)
                await page.render({ canvasContext: ctx, viewport: vp }).promise
                newMap.set(i, canvas.toDataURL('image/jpeg', 0.85))
              }
            } catch (pageErr) {
              console.warn(`Failed to render thumbnail for page ${i + 1}:`, pageErr)
            }
          }

          if (!isCancelled) {
            setThumbnails(new Map(newMap))
          }
          return
        } catch (pdfErr) {
          console.warn('PDF.js thumbnail extraction failed:', pdfErr)
        }
      }

      // 2. Tauri native backend rendering fallback
      if (docId && !docId.startsWith('browser-session-')) {
        const count = pageCount || 1
        for (let i = 0; i < count; i++) {
          if (isCancelled) return
          try {
            const pngBytes = await invoke<number[]>('session_render_page_to_png', {
              docId,
              pageIndex: i,
              dpi: 40,
            })
            if (pngBytes && pngBytes.length > 0) {
              const blob = new Blob([new Uint8Array(pngBytes)], { type: 'image/png' })
              const url = URL.createObjectURL(blob)
              blobUrlsRef.current.add(url)
              newMap.set(i, url)
            }
          } catch (err) {
            console.warn(`[EditorThumbnailSidebar] ページ ${i + 1}のサムネイル生成失敗:`, err)
          }
        }
        if (!isCancelled) {
          setThumbnails(new Map(newMap))
        }
      }
    }

    loadRealThumbnails()

    return () => {
      isCancelled = true
    }
  }, [pdfData, docId, pageCount])

  if (collapsed) {
    return (
      <div
        onClick={onToggleCollapse}
        title="サムネイルを表示"
        style={{
          width: 36,
          height: '100%',
          background: '#ffffff',
          borderRight: '1px solid #e2e8f0',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          paddingTop: 14,
          cursor: 'pointer',
          flexShrink: 0,
        }}
      >
        <span style={{ fontSize: 13, color: '#64748b', fontWeight: 700 }}>≫</span>
      </div>
    )
  }

  const effectiveCount = Math.max(pageCount, detectedPageCount, thumbnails.size, 1)

  return (
    <div
      style={{
        width: 140,
        height: '100%',
        background: '#ffffff',
        borderRight: '1px solid #e2e8f0',
        display: 'flex',
        flexDirection: 'column',
        boxSizing: 'border-box',
        overflowY: 'auto',
        flexShrink: 0,
        userSelect: 'none',
      }}
    >
      {/* Header */}
      <div
        style={{
          padding: '12px 14px',
          borderBottom: '1px solid #f1f5f9',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
        }}
      >
        <span style={{ fontSize: 12.5, fontWeight: 700, color: '#0f172a' }}>
          サムネイル
        </span>
        <button
          onClick={onToggleCollapse}
          title="折りたたむ"
          style={{
            background: 'transparent',
            border: 'none',
            color: '#94a3b8',
            fontSize: 12,
            fontWeight: 700,
            cursor: 'pointer',
            padding: 2,
          }}
        >
          ≪
        </button>
      </div>

      {/* Pages Vertical List */}
      <div style={{ padding: '14px 14px', display: 'flex', flexDirection: 'column', gap: 16, alignItems: 'center' }}>
        {Array.from({ length: effectiveCount }).map((_, idx) => {
          const isActive = currentPage === idx
          const thumbSrc = thumbnails.get(idx)

          return (
            <div
              key={idx}
              onClick={() => onSelectPage(idx)}
              style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                gap: 5,
                cursor: 'pointer',
              }}
            >
              {/* Real Page Miniature Card */}
              <div
                style={{
                  width: 86,
                  height: 116,
                  background: '#ffffff',
                  borderRadius: 4,
                  border: isActive ? '2px solid #2563eb' : '1px solid #cbd5e1',
                  boxShadow: isActive
                    ? '0 4px 14px rgba(37, 99, 235, 0.25)'
                    : '0 1px 4px rgba(0, 0, 0, 0.06)',
                  position: 'relative',
                  overflow: 'hidden',
                  boxSizing: 'border-box',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  transition: 'all 0.15s ease',
                  padding: 2,
                }}
              >
                {thumbSrc ? (
                  <img
                    src={thumbSrc}
                    alt={`ページ ${idx + 1}`}
                    style={{
                      width: '100%',
                      height: '100%',
                      objectFit: 'contain',
                      borderRadius: 2,
                      display: 'block',
                      background: '#ffffff',
                    }}
                  />
                ) : (
                  <div
                    style={{
                      display: 'flex',
                      flexDirection: 'column',
                      alignItems: 'center',
                      justifyContent: 'center',
                      height: '100%',
                      width: '100%',
                      background: '#f8fafc',
                      color: '#94a3b8',
                      fontSize: 11,
                      gap: 4,
                    }}
                  >
                    <div style={{ width: 36, height: 4, background: '#e2e8f0', borderRadius: 2 }} />
                    <div style={{ width: 48, height: 3, background: '#f1f5f9', borderRadius: 2 }} />
                    <span style={{ fontSize: 10, color: '#64748b', marginTop: 4 }}>P.{idx + 1}</span>
                  </div>
                )}
              </div>

              {/* Page Number Label */}
              <span
                style={{
                  fontSize: 11,
                  fontWeight: isActive ? 700 : 500,
                  color: isActive ? '#2563eb' : '#64748b',
                }}
              >
                {idx + 1}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
