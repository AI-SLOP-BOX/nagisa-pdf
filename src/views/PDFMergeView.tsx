import React, { useState } from 'react'
import type { View } from '../types'
import {
  LinkChainIcon,
  DocumentPlusIcon,
  MoreHorizontalIcon,
} from '../components/Icons'
import { MergeSettingsPanel } from '../components/MergeSettingsPanel'

interface PDFMergeViewProps {
  onNavigateView: (view: View) => void
  onOpenFile?: (bytes: number[], name: string) => void
}

interface MergeItem {
  id: string
  name: string
  pages: number
  size: string
  previewColor?: string
}

export const PDFMergeView: React.FC<PDFMergeViewProps> = ({ onNavigateView }) => {
  const [outputName, setOutputName] = useState('結合されたドキュメント.pdf')
  const [savePath, setSavePath] = useState('C:\\Users\\user\\Documents')
  const [keepBookmarks, setKeepBookmarks] = useState(true)
  const [handlePassword, setHandlePassword] = useState(true)
  const [insertSeparator, setInsertSeparator] = useState(false)
  const [separatorText, setSeparatorText] = useState('')
  const [isMerging, setIsMerging] = useState(false)
  const [mergeSuccess, setMergeSuccess] = useState(false)

  const [items, setItems] = useState<MergeItem[]>([])

  const handleReverseOrder = () => {
    setItems(prev => [...prev].reverse())
  }

  const handleClearAll = () => {
    setItems([])
  }

  const handleStartMerge = () => {
    setIsMerging(true)
    setTimeout(() => {
      setIsMerging(false)
      setMergeSuccess(true)
      setTimeout(() => setMergeSuccess(false), 3000)
    }, 1200)
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', width: '100%', height: '100vh', background: '#f8fafc', overflow: 'hidden' }}>
      {/* 1. Hero Header Banner */}
      <div
        style={{
          position: 'relative',
          padding: '22px 32px 20px 32px',
          background: 'linear-gradient(135deg, #f0f7ff 0%, #e0f2fe 50%, #bae6fd 100%)',
          borderBottom: '1px solid #bae6fd',
          overflow: 'hidden',
          flexShrink: 0,
        }}
      >
        {/* Subtle Wave SVG Accent */}
        <svg
          style={{ position: 'absolute', right: 0, top: 0, height: '100%', opacity: 0.28, pointerEvents: 'none' }}
          viewBox="0 0 600 120"
          preserveAspectRatio="none"
        >
          <path d="M0 50 C180 110 380 10 600 50 L600 120 L0 120 Z" fill="#0284c7" />
          <path d="M0 75 C220 30 420 100 600 75 L600 120 L0 120 Z" fill="#38bdf8" />
        </svg>

        <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: '#0369a1', marginBottom: 4, fontWeight: 600 }}>
          <LinkChainIcon size={14} color="#0369a1" />
          <span>PDFの結合</span>
        </div>
        <h1 style={{ fontSize: 24, fontWeight: 800, color: '#0f172a', margin: '0 0 6px 0', letterSpacing: '-0.02em' }}>
          PDFの結合
        </h1>
        <p style={{ fontSize: 13, color: '#475569', margin: 0, maxWidth: 650, lineHeight: 1.5 }}>
          複数のPDFファイルを1つにまとめます。ドラッグ＆ドロップでファイルを追加し、順序を並べ替えて、1つのPDFに結合できます。
        </p>
      </div>

      {/* 2. Main Content Split */}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden', padding: '20px 32px', gap: 20 }}>
        {/* Left Side: Dropzone + Reorderable File List */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 16, overflowY: 'auto' }}>
          {/* Dropzone Card */}
          <div
            style={{
              borderRadius: 12,
              border: '1.5px dashed #cbd5e1',
              background: '#ffffff',
              padding: '24px 20px',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              boxShadow: '0 2px 6px rgba(0,0,0,0.02)',
              cursor: 'pointer',
            }}
          >
            <div style={{ marginBottom: 8, color: '#94a3b8' }}>
              <DocumentPlusIcon size={32} color="#94a3b8" />
            </div>
            <span style={{ fontSize: 14, fontWeight: 700, color: '#1e293b', marginBottom: 3 }}>
              PDFファイルをドラッグ＆ドロップ
            </span>
            <span style={{ fontSize: 11.5, color: '#64748b', marginBottom: 14 }}>
              または、ボタンからファイルを追加してください。
            </span>
            <div style={{ display: 'flex', gap: 10 }}>
              <button
                onClick={() => {
                  const newId = String(Date.now())
                  setItems(prev => [
                    ...prev,
                    { id: newId, name: `新規ドキュメント_${prev.length + 1}.pdf`, pages: 10, size: '2.0 MB', previewColor: '#60a5fa' }
                  ])
                }}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  background: '#2563eb',
                  color: '#ffffff',
                  border: 'none',
                  borderRadius: 7,
                  padding: '7px 18px',
                  fontSize: 12,
                  fontWeight: 600,
                  cursor: 'pointer',
                  boxShadow: '0 2px 8px rgba(37,99,235,0.25)',
                }}
              >
                + ファイルを追加
              </button>
              <button
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  background: '#f1f5f9',
                  color: '#1e293b',
                  border: '1px solid #cbd5e1',
                  borderRadius: 7,
                  padding: '7px 16px',
                  fontSize: 12,
                  fontWeight: 600,
                  cursor: 'pointer',
                }}
              >
                📁 フォルダを追加
              </button>
            </div>
          </div>

          {/* List Card Header */}
          <div
            style={{
              background: '#ffffff',
              borderRadius: 14,
              border: '1px solid #e2e8f0',
              overflow: 'hidden',
              boxShadow: '0 2px 8px rgba(0,0,0,0.03)',
              display: 'flex',
              flexDirection: 'column',
            }}
          >
            <div
              style={{
                padding: '12px 18px',
                borderBottom: '1px solid #f1f5f9',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
              }}
            >
              <span style={{ fontSize: 13.5, fontWeight: 700, color: '#0f172a' }}>
                追加したファイル {items.length}件
              </span>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <button
                  style={{
                    background: '#f8fafc',
                    border: '1px solid #e2e8f0',
                    color: '#334155',
                    borderRadius: 6,
                    padding: '5px 10px',
                    fontSize: 11.5,
                    fontWeight: 500,
                    cursor: 'pointer',
                  }}
                >
                  ↑↓ 並び替え ▾
                </button>
                <button
                  onClick={handleReverseOrder}
                  style={{
                    background: '#f8fafc',
                    border: '1px solid #e2e8f0',
                    color: '#334155',
                    borderRadius: 6,
                    padding: '5px 10px',
                    fontSize: 11.5,
                    fontWeight: 500,
                    cursor: 'pointer',
                  }}
                >
                  ↑↓ 逆順
                </button>
                <button
                  onClick={handleClearAll}
                  style={{
                    background: 'transparent',
                    border: 'none',
                    color: '#ef4444',
                    padding: '5px 8px',
                    fontSize: 11.5,
                    fontWeight: 500,
                    cursor: 'pointer',
                  }}
                >
                  🗑 すべてクリア
                </button>
              </div>
            </div>

            {/* Reorderable Items List */}
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              {items.length === 0 ? (
                <div style={{ padding: '36px 20px', textAlign: 'center', color: '#94a3b8' }}>
                  <div style={{ fontSize: 24, marginBottom: 8, opacity: 0.6 }}>📂</div>
                  <p style={{ fontSize: 13, fontWeight: 600, color: '#64748b' }}>結合するファイルがまだ追加されていません</p>
                  <p style={{ fontSize: 11.5, color: '#94a3b8', marginTop: 4 }}>「＋ ファイルを追加」ボタンをクリックしてPDFを選択してください</p>
                </div>
              ) : (
                items.map((item, index) => (
                <div
                  key={item.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    padding: '10px 18px',
                    borderBottom: index < items.length - 1 ? '1px solid #f1f5f9' : 'none',
                    background: '#ffffff',
                    transition: 'background-color 0.12s',
                  }}
                  onMouseEnter={e => (e.currentTarget.style.background = '#f8fafc')}
                  onMouseLeave={e => (e.currentTarget.style.background = '#ffffff')}
                >
                  {/* Drag Grip Dots (⋮⋮) */}
                  <div
                    style={{
                      cursor: 'grab',
                      color: '#94a3b8',
                      marginRight: 14,
                      fontSize: 15,
                      letterSpacing: -1,
                      userSelect: 'none',
                    }}
                  >
                    ⋮⋮
                  </div>

                  {/* Thumbnail Miniature */}
                  <div
                    style={{
                      width: 32,
                      height: 42,
                      borderRadius: 4,
                      border: '1px solid #cbd5e1',
                      background: '#f8fafc',
                      marginRight: 14,
                      position: 'relative',
                      overflow: 'hidden',
                      flexShrink: 0,
                      boxShadow: '0 1px 3px rgba(0,0,0,0.08)',
                    }}
                  >
                    <div
                      style={{
                        position: 'absolute',
                        bottom: 0,
                        left: 0,
                        right: 0,
                        height: 18,
                        background: item.previewColor || '#38bdf8',
                        opacity: 0.75,
                        borderRadius: '0 0 3px 3px',
                      }}
                    />
                  </div>

                  {/* Title & Info */}
                  <div style={{ flex: 1, minWidth: 0, marginRight: 16 }}>
                    <div
                      style={{
                        fontSize: 13,
                        fontWeight: 600,
                        color: '#1e293b',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {item.name}
                    </div>
                  </div>

                  {/* Page Count */}
                  <div style={{ width: 80, fontSize: 12, color: '#64748b' }}>
                    {item.pages} ページ
                  </div>

                  {/* Size */}
                  <div style={{ width: 80, fontSize: 12, color: '#64748b' }}>
                    {item.size}
                  </div>

                  {/* Menu */}
                  <button
                    style={{
                      background: 'transparent',
                      border: 'none',
                      color: '#94a3b8',
                      cursor: 'pointer',
                      padding: 4,
                    }}
                  >
                    <MoreHorizontalIcon size={16} />
                  </button>
                </div>
              )))}
            </div>
          </div>
        </div>

        {/* Right Side: Output Settings Panel */}
        <MergeSettingsPanel
          outputName={outputName}
          setOutputName={setOutputName}
          savePath={savePath}
          keepBookmarks={keepBookmarks}
          setKeepBookmarks={setKeepBookmarks}
          handlePassword={handlePassword}
          setHandlePassword={setHandlePassword}
          insertSeparator={insertSeparator}
          setInsertSeparator={setInsertSeparator}
          separatorText={separatorText}
          setSeparatorText={setSeparatorText}
          isMerging={isMerging}
          mergeSuccess={mergeSuccess}
          onStartMerge={handleStartMerge}
        />
      </div>
    </div>
  )
}
