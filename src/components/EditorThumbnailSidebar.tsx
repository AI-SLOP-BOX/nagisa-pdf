import React, { useEffect, useRef } from 'react'
import { usePageThumbnails } from '../hooks/usePageThumbnails'

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
  // 共有フックで遅延読込（可視範囲のみ描画・上限なし・URL寿命管理込み）
  const { thumbnails, detectedPageCount, requestPage } = usePageThumbnails({
    pdfData,
    docId,
    pageCount,
  })
  const itemRefs = useRef<Map<number, HTMLDivElement>>(new Map())

  const effectiveCount = Math.max(pageCount, detectedPageCount, thumbnails.size, 1)

  // 可視アイテムのみ描画要求（スクロール遅延読込）。当面ページは即時要求。
  useEffect(() => {
    requestPage(currentPage)
    if (typeof IntersectionObserver === 'undefined') {
      for (let i = 0; i < effectiveCount; i++) requestPage(i)
      return
    }
    const observer = new IntersectionObserver(
      entries => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const idx = Number((entry.target as HTMLElement).dataset.pageIndex)
            if (!Number.isNaN(idx)) requestPage(idx)
          }
        }
      },
      { rootMargin: '400px' },
    )
    for (const el of itemRefs.current.values()) observer.observe(el)
    return () => observer.disconnect()
  }, [effectiveCount, currentPage, requestPage])

  if (collapsed) {
    return (
      <div
        onClick={onToggleCollapse}
        title="サムネイルを表示"        style={{
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
              data-page-index={idx}
              ref={el => {
                if (el) itemRefs.current.set(idx, el)
                else itemRefs.current.delete(idx)
              }}
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
