import React, { useState } from 'react'
import type { View } from '../types'
import {
  FileIcon,
  FolderOpenIcon,
  ConvertIcon,
  CheckIcon,
  ChevronDownIcon,
  MoreHorizontalIcon,
  OcrScanIcon,
} from '../components/Icons'

interface PDFConvertViewProps {
  onNavigateView: (view: View) => void
  onOpenFile?: (bytes: number[], name: string) => void
}

interface ConvertFileItem {
  id: string
  name: string
  pages: number
  size: string
  status: '準備完了' | '変換中' | '完了'
  checked: boolean
}

export const PDFConvertView: React.FC<PDFConvertViewProps> = ({ onNavigateView }) => {
  const [selectedFormat, setSelectedFormat] = useState<'word' | 'excel' | 'ppt' | 'image' | 'text' | 'html'>('word')
  const [outputDest, setOutputDest] = useState<'same' | 'custom'>('same')
  const [customPath, setCustomPath] = useState('~/Documents')
  const [pageRangeMode, setPageRangeMode] = useState<'all' | 'custom'>('all')
  const [customPages, setCustomPages] = useState('')
  const [keepImages, setKeepImages] = useState(true)
  const [editableTables, setEditableTables] = useState(true)
  const [runOcr, setRunOcr] = useState(false)
  const [isConverting, setIsConverting] = useState(false)
  const [convertDone, setConvertDone] = useState(false)

  const [files, setFiles] = useState<ConvertFileItem[]>([])

  const formatCards = [
    { key: 'word', title: 'PDF → Word', ext: '.docx に変換', icon: '📝', color: '#2563eb', bg: '#eff6ff' },
    { key: 'excel', title: 'PDF → Excel', ext: '.xlsx に変換', icon: '📊', color: '#16a34a', bg: '#f0fdf4' },
    { key: 'ppt', title: 'PDF → PowerPoint', ext: '.pptx に変換', icon: '📽️', color: '#ea580c', bg: '#fff7ed' },
    { key: 'image', title: 'PDF → 画像', ext: '.jpg / .png に変換', icon: '🖼️', color: '#9333ea', bg: '#faf5ff' },
    { key: 'text', title: 'PDF → テキスト', ext: '.txt に変換', icon: '📄', color: '#0284c7', bg: '#f0f9ff' },
    { key: 'html', title: 'PDF → HTML', ext: '.html に変換', icon: '🌐', color: '#4f46e5', bg: '#eef2ff' },
  ] as const

  const handleToggleCheck = (id: string) => {
    setFiles(prev => prev.map(f => (f.id === id ? { ...f, checked: !f.checked } : f)))
  }

  const handleToggleAll = () => {
    const allChecked = files.every(f => f.checked)
    setFiles(prev => prev.map(f => ({ ...f, checked: !allChecked })))
  }

  const handleClearAll = () => {
    setFiles([])
  }

  const handleStartConvert = () => {
    setIsConverting(true)
    setTimeout(() => {
      setIsConverting(false)
      setConvertDone(true)
      setTimeout(() => setConvertDone(false), 3000)
    }, 1200)
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', width: '100%', height: '100vh', background: '#f8fafc', overflow: 'hidden' }}>
      {/* 1. Hero Wave Header Banner */}
      <div
        style={{
          position: 'relative',
          padding: '24px 32px 20px 32px',
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
          <path d="M0 60 C150 120 350 0 600 60 L600 120 L0 120 Z" fill="#0284c7" />
          <path d="M0 80 C200 40 400 110 600 80 L600 120 L0 120 Z" fill="#38bdf8" />
        </svg>

        <h1 style={{ fontSize: 24, fontWeight: 800, color: '#0f172a', margin: '0 0 6px 0', letterSpacing: '-0.02em' }}>
          PDFの変換
        </h1>
        <p style={{ fontSize: 13, color: '#475569', margin: 0 }}>
          PDFを、さまざまな形式に変換できます。
        </p>
      </div>

      {/* 2. Format Selection Cards Row */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(6, 1fr)',
          gap: 12,
          padding: '16px 32px',
          background: '#ffffff',
          borderBottom: '1px solid #e2e8f0',
          flexShrink: 0,
        }}
      >
        {formatCards.map(c => {
          const active = selectedFormat === c.key
          return (
            <div
              key={c.key}
              onClick={() => setSelectedFormat(c.key)}
              style={{
                borderRadius: 12,
                padding: '12px 14px',
                background: active ? '#eff6ff' : '#f8fafc',
                border: `1.5px solid ${active ? '#2563eb' : '#e2e8f0'}`,
                cursor: 'pointer',
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                textAlign: 'center',
                gap: 4,
                boxShadow: active ? '0 4px 14px rgba(37, 99, 235, 0.12)' : 'none',
                transition: 'all 0.15s ease',
              }}
              onMouseEnter={e => {
                if (!active) e.currentTarget.style.borderColor = '#cbd5e1'
              }}
              onMouseLeave={e => {
                if (!active) e.currentTarget.style.borderColor = '#e2e8f0'
              }}
            >
              <div style={{ fontSize: 22, marginBottom: 2 }}>{c.icon}</div>
              <span style={{ fontSize: 12.5, fontWeight: 700, color: active ? '#1d4ed8' : '#1e293b' }}>
                {c.title}
              </span>
              <span style={{ fontSize: 10.5, color: '#64748b' }}>
                {c.ext}
              </span>
            </div>
          )
        })}
      </div>

      {/* 3. Main Split View: Left File List + Right Settings Panel */}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden', padding: '20px 32px', gap: 20 }}>
        {/* Left Side: File List Card */}
        <div
          style={{
            flex: 1,
            background: '#ffffff',
            borderRadius: 14,
            border: '1px solid #e2e8f0',
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
            boxShadow: '0 2px 8px rgba(0,0,0,0.03)',
          }}
        >
          {/* Card Header & Action Buttons */}
          <div
            style={{
              padding: '14px 20px',
              borderBottom: '1px solid #f1f5f9',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
            }}
          >
            <span style={{ fontSize: 15, fontWeight: 700, color: '#0f172a' }}>
              ファイルを追加
            </span>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <button
                onClick={() => {
                  const newId = String(Date.now())
                  setFiles(prev => [
                    ...prev,
                    { id: newId, name: `追加資料_${prev.length + 1}.pdf`, pages: 6, size: '1.2 MB', status: '準備完了', checked: true }
                  ])
                }}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  background: '#f1f5f9',
                  border: '1px solid #e2e8f0',
                  color: '#1e293b',
                  padding: '6px 14px',
                  borderRadius: 8,
                  fontSize: 12,
                  fontWeight: 600,
                  cursor: 'pointer',
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
                  border: '1px solid #e2e8f0',
                  color: '#1e293b',
                  padding: '6px 14px',
                  borderRadius: 8,
                  fontSize: 12,
                  fontWeight: 600,
                  cursor: 'pointer',
                }}
              >
                📁 フォルダを追加
              </button>
              <button
                onClick={handleClearAll}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  background: 'transparent',
                  border: 'none',
                  color: '#ef4444',
                  padding: '6px 10px',
                  fontSize: 12,
                  fontWeight: 500,
                  cursor: 'pointer',
                }}
              >
                🗑 すべてクリア
              </button>
            </div>
          </div>

          {/* Table */}
          <div style={{ flex: 1, overflowY: 'auto' }}>
            <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: 13 }}>
              <thead>
                <tr style={{ background: '#f8fafc', borderBottom: '1px solid #e2e8f0', color: '#64748b', fontSize: 11.5 }}>
                  <th style={{ padding: '10px 16px', width: 36 }}>
                    <input
                      type="checkbox"
                      checked={files.length > 0 && files.every(f => f.checked)}
                      onChange={handleToggleAll}
                      style={{ cursor: 'pointer' }}
                    />
                  </th>
                  <th style={{ padding: '10px 12px' }}>名前</th>
                  <th style={{ padding: '10px 12px', width: 90 }}>ページ数</th>
                  <th style={{ padding: '10px 12px', width: 90 }}>サイズ</th>
                  <th style={{ padding: '10px 12px', width: 100 }}>状態</th>
                  <th style={{ padding: '10px 16px', width: 40, textAlign: 'center' }}></th>
                </tr>
              </thead>
              <tbody>
                {files.length === 0 ? (
                  <tr>
                    <td colSpan={6} style={{ padding: '36px 20px', textAlign: 'center', color: '#94a3b8' }}>
                      <div style={{ fontSize: 24, marginBottom: 8, opacity: 0.6 }}>📂</div>
                      <p style={{ fontSize: 13, fontWeight: 600, color: '#64748b' }}>変換するファイルがまだ追加されていません</p>
                      <p style={{ fontSize: 11.5, color: '#94a3b8', marginTop: 4 }}>上の「＋ ファイルを追加」ボタンをクリックしてPDFを選択してください</p>
                    </td>
                  </tr>
                ) : (
                  files.map(file => (
                    <tr
                      key={file.id}
                      style={{
                        borderBottom: '1px solid #f1f5f9',
                        transition: 'background-color 0.12s',
                      }}
                      onMouseEnter={e => (e.currentTarget.style.background = '#f8fafc')}
                      onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
                    >
                      <td style={{ padding: '12px 16px' }}>
                        <input
                          type="checkbox"
                          checked={file.checked}
                          onChange={() => handleToggleCheck(file.id)}
                          style={{ cursor: 'pointer' }}
                        />
                      </td>
                      <td style={{ padding: '12px 12px' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                          <div
                            style={{
                              width: 22,
                              height: 24,
                              background: '#ef4444',
                              borderRadius: 4,
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'center',
                              flexShrink: 0,
                            }}
                          >
                            <span style={{ color: '#fff', fontSize: 7.5, fontWeight: 900 }}>PDF</span>
                          </div>
                          <span style={{ fontWeight: 600, color: '#1e293b' }}>{file.name}</span>
                        </div>
                      </td>
                      <td style={{ padding: '12px 12px', color: '#64748b' }}>{file.pages}</td>
                      <td style={{ padding: '12px 12px', color: '#64748b' }}>{file.size}</td>
                      <td style={{ padding: '12px 12px' }}>
                        <span
                          style={{
                            background: '#f1f5f9',
                            color: '#475569',
                            padding: '3px 8px',
                            borderRadius: 6,
                            fontSize: 11,
                            fontWeight: 500,
                          }}
                        >
                          {file.status}
                        </span>
                      </td>
                      <td style={{ padding: '12px 16px', textAlign: 'center' }}>
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
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>

        {/* Right Side: Convert Settings Panel */}
        <div
          style={{
            width: 320,
            background: '#ffffff',
            borderRadius: 14,
            border: '1px solid #e2e8f0',
            padding: 20,
            display: 'flex',
            flexDirection: 'column',
            gap: 18,
            boxShadow: '0 2px 8px rgba(0,0,0,0.03)',
            boxSizing: 'border-box',
            flexShrink: 0,
          }}
        >
          <span style={{ fontSize: 15, fontWeight: 700, color: '#0f172a' }}>
            変換設定
          </span>

          {/* Format Select */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <label style={{ fontSize: 12, fontWeight: 600, color: '#475569' }}>出力形式</label>
            <select
              value={selectedFormat}
              onChange={e =>
                setSelectedFormat(e.target.value as 'word' | 'excel' | 'ppt' | 'image' | 'text' | 'html')
              }
              style={{
                width: '100%',
                height: 38,
                borderRadius: 8,
                border: '1px solid #cbd5e1',
                padding: '0 12px',
                fontSize: 13,
                color: '#1e293b',
                background: '#f8fafc',
                cursor: 'pointer',
              }}
            >
              <option value="word">Word (.docx)</option>
              <option value="excel">Excel (.xlsx)</option>
              <option value="ppt">PowerPoint (.pptx)</option>
              <option value="image">画像 (.png, .jpg)</option>
              <option value="text">テキスト (.txt)</option>
              <option value="html">HTML (.html)</option>
            </select>
          </div>

          {/* Destination */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <label style={{ fontSize: 12, fontWeight: 600, color: '#475569' }}>出力先</label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, color: '#334155', cursor: 'pointer' }}>
              <input
                type="radio"
                name="dest"
                checked={outputDest === 'same'}
                onChange={() => setOutputDest('same')}
              />
              元のフォルダに保存
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, color: '#334155', cursor: 'pointer' }}>
              <input
                type="radio"
                name="dest"
                checked={outputDest === 'custom'}
                onChange={() => setOutputDest('custom')}
              />
              フォルダを指定
            </label>
            {outputDest === 'custom' && (
              <div style={{ display: 'flex', gap: 6, marginTop: 2 }}>
                <input
                  type="text"
                  value={customPath}
                  onChange={e => setCustomPath(e.target.value)}
                  style={{
                    flex: 1,
                    height: 34,
                    borderRadius: 6,
                    border: '1px solid #cbd5e1',
                    padding: '0 10px',
                    fontSize: 11.5,
                    color: '#334155',
                  }}
                />
                <button
                  style={{
                    height: 34,
                    padding: '0 10px',
                    background: '#f1f5f9',
                    border: '1px solid #cbd5e1',
                    borderRadius: 6,
                    fontSize: 12,
                    cursor: 'pointer',
                  }}
                >
                  📁
                </button>
              </div>
            )}
          </div>

          {/* Page Range */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <label style={{ fontSize: 12, fontWeight: 600, color: '#475569' }}>ページ範囲</label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, color: '#334155', cursor: 'pointer' }}>
              <input
                type="radio"
                name="pages"
                checked={pageRangeMode === 'all'}
                onChange={() => setPageRangeMode('all')}
              />
              すべてのページ
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, color: '#334155', cursor: 'pointer' }}>
              <input
                type="radio"
                name="pages"
                checked={pageRangeMode === 'custom'}
                onChange={() => setPageRangeMode('custom')}
              />
              指定したページ
            </label>
            {pageRangeMode === 'custom' && (
              <input
                type="text"
                placeholder="例) 1-5, 8, 11-13"
                value={customPages}
                onChange={e => setCustomPages(e.target.value)}
                style={{
                  height: 34,
                  borderRadius: 6,
                  border: '1px solid #cbd5e1',
                  padding: '0 10px',
                  fontSize: 12,
                  marginTop: 2,
                }}
              />
            )}
          </div>

          {/* Options */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8, paddingTop: 4, borderTop: '1px solid #f1f5f9' }}>
            <label style={{ fontSize: 12, fontWeight: 600, color: '#475569' }}>その他のオプション</label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, color: '#334155', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={keepImages}
                onChange={e => setKeepImages(e.target.checked)}
              />
              画像を保持する
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, color: '#334155', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={editableTables}
                onChange={e => setEditableTables(e.target.checked)}
              />
              表を編集可能にする
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, color: '#334155', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={runOcr}
                onChange={e => setRunOcr(e.target.checked)}
              />
              OCR（スキャンしたPDFをテキスト化）
            </label>
          </div>

          {/* Action Button */}
          <div style={{ marginTop: 'auto', paddingTop: 14 }}>
            <button
              onClick={handleStartConvert}
              disabled={isConverting}
              style={{
                width: '100%',
                height: 42,
                borderRadius: 9,
                border: 'none',
                background: convertDone ? '#16a34a' : '#2563eb',
                color: '#ffffff',
                fontSize: 14,
                fontWeight: 700,
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: 8,
                boxShadow: '0 4px 14px rgba(37, 99, 235, 0.3)',
                transition: 'all 0.15s ease',
              }}
            >
              {isConverting ? (
                <span>変換処理中...</span>
              ) : convertDone ? (
                <>
                  <CheckIcon size={18} color="#fff" />
                  <span>変換完了！</span>
                </>
              ) : (
                <>
                  <ConvertIcon size={18} color="#fff" />
                  <span>変換を開始</span>
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
