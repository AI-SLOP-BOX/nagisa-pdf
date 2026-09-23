import React from 'react'
import { FileIcon } from './Icons'
import { t } from '../utils/i18n'

interface EditorEmptyDropZoneProps {
  onOpen: () => void
  onNavigateHome?: () => void
  onLoadFileBytes: (bytes: number[], name: string) => Promise<void>
  onOpenStart?: (name: string) => void
  onError: (msg: string) => void
}

export const EditorEmptyDropZone: React.FC<EditorEmptyDropZoneProps> = ({
  onOpen,
  onNavigateHome,
  onLoadFileBytes,
  onOpenStart,
  onError,
}) => {
  return (
    <div
      onDragOver={e => {
        e.preventDefault()
        e.stopPropagation()
      }}
      onDrop={async e => {
        e.preventDefault()
        e.stopPropagation()
        const file = Array.from(e.dataTransfer.files).find(f => /\.pdf$/i.test(f.name))
        if (!file) return
        try {
          const arrayBuffer = await file.arrayBuffer()
          const bytes = Array.from(new Uint8Array(arrayBuffer))
          onOpenStart?.(file.name)
          await onLoadFileBytes(bytes, file.name)
        } catch {
          onError('PDFの読み込みに失敗しました')
        }
      }}
      style={{
        flex: 1,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: '#f8fafc',
        border: '2px dashed #cbd5e1',
        margin: 24,
        borderRadius: 14,
      }}
    >
      <div style={{ textAlign: 'center', color: '#64748b' }}>
        <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'center' }}>
          <FileIcon size={56} color="#94a3b8" />
        </div>
        <p style={{ fontSize: 16, fontWeight: 700, color: '#1e293b' }}>{t().noFileLoaded}</p>
        <p style={{ fontSize: 12.5, color: '#64748b', marginTop: 4 }}>
          PDFファイルをここにドラッグ＆ドロップ、またはボタンをクリック
        </p>
        <div style={{ display: 'flex', gap: 12, justifyContent: 'center', marginTop: 18 }}>
          <button
            onClick={onOpen}
            style={{
              padding: '9px 24px',
              background: '#2563eb',
              color: '#fff',
              border: 'none',
              borderRadius: 8,
              fontSize: 13,
              fontWeight: 600,
              cursor: 'pointer',
              boxShadow: '0 4px 12px rgba(37, 99, 235, 0.25)',
            }}
          >
            {t().openFileBtn}
          </button>
          {onNavigateHome && (
            <button
              onClick={onNavigateHome}
              style={{
                padding: '9px 20px',
                background: '#ffffff',
                color: '#334155',
                border: '1px solid #cbd5e1',
                borderRadius: 8,
                fontSize: 13,
                fontWeight: 500,
                cursor: 'pointer',
              }}
            >
              ホームに戻る
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
