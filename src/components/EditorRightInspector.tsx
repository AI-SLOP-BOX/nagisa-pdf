import React, { useState } from 'react'
import type { View } from '../types'
import { UserAnnotation } from '../services/annotationService'
import {
  LinkChainIcon,
  ScissorsIcon,
  ConvertIcon,
  CompressIcon,
  AnnotateIcon,
  OcrScanIcon,
  MoreHorizontalIcon,
} from './Icons'

interface EditorRightInspectorProps {
  fileName: string
  pageCount: number
  fileSize?: string
  editorMode: 'inspect' | 'edit'
  collapsed?: boolean
  onToggleCollapse?: () => void
  onNavigateView?: (view: View) => void
  onActivateAnnotate?: () => void
  selectedAnnotation?: UserAnnotation | null
  onUpdateAnnotation?: (updates: Partial<UserAnnotation>) => void
  onAddTextAnnotation?: () => void
  onAddHighlightAnnotation?: () => void
  onAddShapeAnnotation?: () => void
  onDeleteSelectedAnnotation?: () => void
}

export const EditorRightInspector: React.FC<EditorRightInspectorProps> = ({
  fileName,
  pageCount,
  fileSize = '—',
  editorMode,
  collapsed = false,
  onToggleCollapse,
  onNavigateView,
  onActivateAnnotate,
  selectedAnnotation,
  onUpdateAnnotation,
  onAddTextAnnotation,
  onAddHighlightAnnotation,
  onDeleteSelectedAnnotation,
}) => {
  const [activeTab, setActiveTab] = useState<'tools' | 'comments' | 'info'>('tools')
  const [tags, setTags] = useState<string[]>(['提案書', '企画', 'リニューアル'])

  if (collapsed) {
    return (
      <div
        onClick={onToggleCollapse}
        title="インスペクターを展開"
        style={{
          width: 36,
          height: '100%',
          background: '#ffffff',
          borderLeft: '1px solid #e2e8f0',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          paddingTop: 14,
          cursor: 'pointer',
          flexShrink: 0,
          transition: 'all 0.2s ease',
        }}
      >
        <span style={{ fontSize: 13, color: '#64748b', fontWeight: 700 }}>≪</span>
      </div>
    )
  }

  const handleAddTag = () => {
    const t = prompt('新しいタグを入力してください:')
    if (t && t.trim()) {
      setTags(prev => [...prev, t.trim()])
    }
  }

  // Render Real In-Place Text Edit Panel (Image 3)
  if (editorMode === 'edit') {
    const isText = selectedAnnotation?.type === 'text'
    const isHighlight = selectedAnnotation?.type === 'highlight'
    const isShape = selectedAnnotation?.type === 'shape'

    return (
      <div
        style={{
          width: 320,
          background: '#ffffff',
          borderLeft: '1px solid #e2e8f0',
          display: 'flex',
          flexDirection: 'column',
          padding: '20px 18px',
          boxSizing: 'border-box',
          overflowY: 'auto',
          flexShrink: 0,
          transition: 'all 0.25s cubic-bezier(0.16, 1, 0.3, 1)',
          animation: 'slideInRight 0.25s cubic-bezier(0.16, 1, 0.3, 1)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            {onToggleCollapse && (
              <button
                onClick={onToggleCollapse}
                title="インスペクターを閉じる"
                style={{
                  background: 'transparent',
                  border: 'none',
                  color: '#94a3b8',
                  fontSize: 14,
                  cursor: 'pointer',
                  padding: '2px 4px',
                  borderRadius: 4,
                  display: 'flex',
                  alignItems: 'center',
                }}
              >
                ≫
              </button>
            )}
            <span style={{ fontSize: 15, fontWeight: 700, color: '#0f172a' }}>
              {selectedAnnotation ? (isText ? 'テキストの編集' : isHighlight ? 'ハイライトの編集' : '図形の編集') : 'オブジェクトの追加・編集'}
            </span>
          </div>
          {selectedAnnotation && onDeleteSelectedAnnotation && (
            <button
              onClick={onDeleteSelectedAnnotation}
              style={{
                background: '#fee2e2',
                color: '#dc2626',
                border: 'none',
                borderRadius: 6,
                padding: '4px 10px',
                fontSize: 11.5,
                fontWeight: 600,
                cursor: 'pointer',
              }}
            >
              削除
            </button>
          )}
        </div>

        {/* Text Area */}
        <div style={{ marginBottom: 14 }}>
          <label style={{ fontSize: 11.5, fontWeight: 600, color: '#64748b', display: 'block', marginBottom: 4 }}>
            {selectedAnnotation ? 'テキスト内容' : 'テキスト'}
          </label>
          <textarea
            value={selectedAnnotation?.text ?? ''}
            placeholder="ページ上のテキストをクリックして編集、または下の「＋ テキストを追加」をクリックしてください"
            onChange={e => {
              if (onUpdateAnnotation && selectedAnnotation) {
                onUpdateAnnotation({ text: e.target.value })
              }
            }}
            disabled={!selectedAnnotation}
            rows={3}
            style={{
              width: '100%',
              borderRadius: 8,
              border: '1px solid #cbd5e1',
              padding: '10px 12px',
              fontSize: 13.5,
              fontWeight: selectedAnnotation?.isBold ? 700 : 400,
              fontStyle: selectedAnnotation?.isItalic ? 'italic' : 'normal',
              color: selectedAnnotation?.color || '#1e293b',
              background: selectedAnnotation ? '#ffffff' : '#f8fafc',
              resize: 'vertical',
              boxSizing: 'border-box',
              lineHeight: 1.4,
            }}
          />
        </div>

        {/* Font Selection */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginBottom: 14 }}>
          <label style={{ fontSize: 11.5, fontWeight: 600, color: '#64748b' }}>フォント (書体)</label>
          <select
            value={selectedAnnotation?.fontFamily || 'Noto Sans JP'}
            disabled={!selectedAnnotation}
            onChange={e => onUpdateAnnotation?.({ fontFamily: e.target.value })}
            style={{
              width: '100%',
              height: 36,
              borderRadius: 7,
              border: '1px solid #cbd5e1',
              padding: '0 10px',
              fontSize: 13,
              color: '#1e293b',
              background: '#f8fafc',
            }}
          >
            <optgroup label="日本語ゴシック体 (Gothic / Sans)">
              <option value="Noto Sans JP">Noto Sans JP (標準ゴシック)</option>
              <option value="Hiragino Sans">Hiragino Sans (ヒラギノ角ゴ)</option>
              <option value="Yu Gothic">Yu Gothic (游ゴシック)</option>
            </optgroup>
            <optgroup label="日本語明朝体 (Mincho / Serif)">
              <option value="BIZ UDMincho, Hiragino Mincho ProN, serif">明朝体 (BIZ UD / ヒラギノ明朝)</option>
              <option value="Yu Mincho, serif">游明朝 (Yu Mincho)</option>
            </optgroup>
            <optgroup label="欧文フォント (Western)">
              <option value="Helvetica Neue, Arial, sans-serif">Helvetica Neue / Arial</option>
              <option value="Times New Roman, Times, serif">Times New Roman</option>
              <option value="Courier New, monospace">Courier New (等幅)</option>
            </optgroup>
          </select>
        </div>

        {/* Size & Orientation Row */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 14 }}>
          {/* Size */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <label style={{ fontSize: 11.5, fontWeight: 600, color: '#64748b' }}>文字サイズ (pt)</label>
            <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
              <input
                type="number"
                min="6"
                max="120"
                value={selectedAnnotation?.fontSize || 16}
                disabled={!selectedAnnotation}
                onChange={e => {
                  const val = Number(e.target.value)
                  if (!isNaN(val) && val > 0) {
                    onUpdateAnnotation?.({ fontSize: val })
                  }
                }}
                style={{
                  width: '100%',
                  height: 34,
                  borderRadius: 6,
                  border: '1px solid #cbd5e1',
                  padding: '0 8px',
                  fontSize: 13,
                  background: '#f8fafc',
                  color: '#1e293b',
                  boxSizing: 'border-box',
                }}
              />
            </div>
          </div>

          {/* Orientation / Rotation */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <label style={{ fontSize: 11.5, fontWeight: 600, color: '#64748b' }}>向き (回転)</label>
            <select
              value={selectedAnnotation?.rotation || 0}
              disabled={!selectedAnnotation}
              onChange={e => onUpdateAnnotation?.({ rotation: Number(e.target.value) })}
              style={{
                width: '100%',
                height: 34,
                borderRadius: 6,
                border: '1px solid #cbd5e1',
                padding: '0 8px',
                fontSize: 12.5,
                background: '#f8fafc',
                color: '#1e293b',
              }}
            >
              <option value={0}>0° (横書き標準)</option>
              <option value={90}>90° (縦 / 右回転)</option>
              <option value={180}>180° (上下反転)</option>
              <option value={270}>270° (縦 / 左回転)</option>
            </select>
          </div>
        </div>

        {/* Styling & Color Row */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16 }}>

          <div style={{ display: 'flex', alignItems: 'flex-end', gap: 4 }}>
            <button
              onClick={() => onUpdateAnnotation?.({ isBold: !selectedAnnotation?.isBold })}
              disabled={!selectedAnnotation}
              style={{
                width: 32,
                height: 34,
                borderRadius: 6,
                border: '1px solid #cbd5e1',
                background: selectedAnnotation?.isBold ? '#eff6ff' : '#ffffff',
                color: selectedAnnotation?.isBold ? '#2563eb' : '#334155',
                fontWeight: 700,
                fontSize: 13,
                cursor: selectedAnnotation ? 'pointer' : 'default',
              }}
            >
              B
            </button>
            <button
              onClick={() => onUpdateAnnotation?.({ isItalic: !selectedAnnotation?.isItalic })}
              disabled={!selectedAnnotation}
              style={{
                width: 32,
                height: 34,
                borderRadius: 6,
                border: '1px solid #cbd5e1',
                background: selectedAnnotation?.isItalic ? '#eff6ff' : '#ffffff',
                color: selectedAnnotation?.isItalic ? '#2563eb' : '#334155',
                fontStyle: 'italic',
                fontSize: 13,
                cursor: selectedAnnotation ? 'pointer' : 'default',
              }}
            >
              I
            </button>
            {/* Preset Color Swatches instead of ugly black browser input */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 4, height: 34, paddingLeft: 4 }}>
              {[
                { color: '#0f172a', label: 'ネイビー/ブラック' },
                { color: '#2563eb', label: 'ブルー' },
                { color: '#dc2626', label: 'レッド' },
                { color: '#16a34a', label: 'グリーン' },
                { color: '#ea580c', label: 'オレンジ' },
              ].map(swatch => {
                const isCurrent = (selectedAnnotation?.color || '#0f172a').toLowerCase() === swatch.color.toLowerCase()
                return (
                  <button
                    key={swatch.color}
                    onClick={() => onUpdateAnnotation?.({ color: swatch.color })}
                    disabled={!selectedAnnotation}
                    title={swatch.label}
                    style={{
                      width: 20,
                      height: 20,
                      borderRadius: '50%',
                      background: swatch.color,
                      border: isCurrent ? '2px solid #ffffff' : '1.5px solid rgba(0,0,0,0.12)',
                      boxShadow: isCurrent ? '0 0 0 2px #2563eb' : '0 1px 2px rgba(0,0,0,0.1)',
                      cursor: selectedAnnotation ? 'pointer' : 'default',
                      padding: 0,
                      flexShrink: 0,
                      transition: 'transform 0.1s',
                      transform: isCurrent ? 'scale(1.15)' : 'scale(1)',
                    }}
                  />
                )
              })}
            </div>
          </div>
        </div>



        {/* Tip helper */}
        <div style={{ padding: '12px 14px', background: '#f8fafc', borderRadius: 8, border: '1px solid #e2e8f0', marginTop: 10 }}>
          <p style={{ fontSize: 11.5, color: '#64748b', lineHeight: 1.5, margin: 0 }}>
            💡 上部のツールバーからテキスト・OCR・ハイライト・図形を選択してページ上で直接編集・配置できます。
          </p>
        </div>
      </div>
    )
  }

  // Render Default Tools & Inspection Inspector (Image 1 & 2)
  return (
    <div
      style={{
        width: 320,
        background: '#ffffff',
        borderLeft: '1px solid #e2e8f0',
        display: 'flex',
        flexDirection: 'column',
        boxSizing: 'border-box',
        overflowY: 'auto',
        flexShrink: 0,
        userSelect: 'none',
      }}
    >
      {/* 1. Segmented Tabs Header (ツール / コメント / ドキュメント情報) */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          borderBottom: '1px solid #e2e8f0',
          background: '#ffffff',
          padding: '0 8px',
        }}
      >
        {onToggleCollapse && (
          <button
            onClick={onToggleCollapse}
            title="インスペクターを閉じる"
            style={{
              background: 'transparent',
              border: 'none',
              color: '#94a3b8',
              fontSize: 14,
              cursor: 'pointer',
              padding: '2px 6px',
              borderRadius: 4,
              marginRight: 4,
              display: 'flex',
              alignItems: 'center',
            }}
          >
            ≫
          </button>
        )}
        {([
          { key: 'tools', label: 'ツール' },
          { key: 'comments', label: 'コメント' },
          { key: 'info', label: 'ドキュメント情報' },
        ] as const).map(tab => {
          const active = activeTab === tab.key
          return (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              style={{
                flex: 1,
                padding: '12px 6px',
                border: 'none',
                background: 'transparent',
                color: active ? '#2563eb' : '#64748b',
                fontSize: 12.5,
                fontWeight: active ? 700 : 500,
                cursor: 'pointer',
                borderBottom: active ? '2px solid #2563eb' : '2px solid transparent',
                transition: 'all 0.15s ease',
              }}
            >
              {tab.label}
            </button>
          )
        })}
      </div>

      <div style={{ padding: '16px 18px', display: 'flex', flexDirection: 'column', gap: 20 }}>
        {/* TAB 1: ツール */}
        {activeTab === 'tools' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <span style={{ fontSize: 13, fontWeight: 700, color: '#0f172a' }}>
              クイックツール
            </span>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 10 }}>
              {[
                { key: 'merge', label: '結合', icon: <LinkChainIcon size={18} color="#2563eb" />, bg: '#eff6ff', border: '#dbeafe' },
                { key: 'split', label: '分割', icon: <ScissorsIcon size={18} color="#ea580c" />, bg: '#fff7ed', border: '#ffedd5' },
                { key: 'convert', label: '変換', icon: <ConvertIcon size={18} color="#16a34a" />, bg: '#f0fdf4', border: '#dcfce7' },
                { key: 'compress', label: '圧縮', icon: <CompressIcon size={18} color="#e11d48" />, bg: '#fff1f2', border: '#ffe4e6' },
                { key: 'annotate', label: '注釈', icon: <AnnotateIcon size={18} color="#9333ea" />, bg: '#faf5ff', border: '#f3e8ff' },
                { key: 'ocr', label: 'OCR', icon: <OcrScanIcon size={18} color="#0891b2" />, bg: '#ecfeff', border: '#cffafe' },
              ].map(t => (
                <button
                  key={t.key}
                  onClick={() => {
                    if (t.key === 'merge') onNavigateView?.('merge')
                    else if (t.key === 'convert') onNavigateView?.('convert')
                    else if (t.key === 'annotate') onActivateAnnotate?.()
                    else if (t.key === 'ocr') onNavigateView?.('ocr')
                  }}
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    justifyContent: 'center',
                    gap: 6,
                    height: 64,
                    background: t.bg,
                    border: `1px solid ${t.border}`,
                    borderRadius: 10,
                    cursor: 'pointer',
                    transition: 'all 0.15s ease',
                  }}
                  onMouseEnter={e => (e.currentTarget.style.transform = 'translateY(-1px)')}
                  onMouseLeave={e => (e.currentTarget.style.transform = 'none')}
                >
                  {t.icon}
                  <span style={{ fontSize: 12, fontWeight: 650, color: '#1e293b' }}>{t.label}</span>
                </button>
              ))}
            </div>
          </div>
        )}

        {/* TAB: ドキュメント情報 */}
        {(activeTab === 'info' || activeTab === 'tools') && (
          <>
            {/* File Info Card */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10, paddingTop: 6, borderTop: '1px solid #f1f5f9' }}>
              <span style={{ fontSize: 13, fontWeight: 700, color: '#0f172a' }}>
                ファイル情報
              </span>
              <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 4 }}>
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
                <span style={{ fontSize: 13, fontWeight: 600, color: '#1e293b', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {fileName || 'ドキュメント.pdf'}
                </span>
              </div>

              {/* Info Table */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: 7, fontSize: 11.5 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>ファイルの種類</span>
                  <span style={{ color: '#1e293b', fontWeight: 500 }}>PDF (Portable Document Format)</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>ファイルサイズ</span>
                  <span style={{ color: '#1e293b', fontWeight: 500 }}>{fileSize}</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>ページ数</span>
                  <span style={{ color: '#1e293b', fontWeight: 500 }}>{pageCount} ページ</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>作成日時</span>
                  <span style={{ color: '#1e293b', fontWeight: 500 }}>2024年5月28日 14:32</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>更新日時</span>
                  <span style={{ color: '#1e293b', fontWeight: 500 }}>2024年5月28日 14:32</span>
                </div>
              </div>
            </div>

            {/* Tags Section */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8, paddingTop: 6, borderTop: '1px solid #f1f5f9' }}>
              <span style={{ fontSize: 13, fontWeight: 700, color: '#0f172a' }}>
                タグ
              </span>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, alignItems: 'center' }}>
                {tags.map(tag => (
                  <span
                    key={tag}
                    style={{
                      padding: '4px 10px',
                      borderRadius: 16,
                      background: '#eff6ff',
                      color: '#2563eb',
                      fontSize: 11.5,
                      fontWeight: 600,
                    }}
                  >
                    {tag}
                  </span>
                ))}
                <button
                  onClick={handleAddTag}
                  style={{
                    padding: '4px 10px',
                    borderRadius: 16,
                    border: '1px dashed #cbd5e1',
                    background: 'transparent',
                    color: '#64748b',
                    fontSize: 11.5,
                    cursor: 'pointer',
                  }}
                >
                  + タグを追加
                </button>
              </div>
            </div>

            {/* Metadata Section */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10, paddingTop: 6, borderTop: '1px solid #f1f5f9' }}>
              <span style={{ fontSize: 13, fontWeight: 700, color: '#0f172a' }}>
                メタデータ
              </span>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 7, fontSize: 11.5 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>PDFバージョン</span>
                  <span style={{ color: '#1e293b', fontWeight: 600 }}>1.7 (Acrobat 8.x)</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>暗号化</span>
                  <span style={{ color: '#16a34a', fontWeight: 600 }}>なし (標準)</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>作成アプリケーション</span>
                  <span style={{ color: '#1e293b', fontWeight: 600 }}>Nagisa Engine v1.0</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>用紙サイズ</span>
                  <span style={{ color: '#1e293b', fontWeight: 600 }}>A4 (210 x 297 mm)</span>
                </div>
              </div>
            </div>
          </>
        )}

        {/* TAB 2: コメント・注釈一覧 */}
        {activeTab === 'comments' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <span style={{ fontSize: 13, fontWeight: 700, color: '#0f172a' }}>
              注釈とコメント
            </span>
            <div style={{ padding: '12px', background: '#f8fafc', borderRadius: 8, border: '1px solid #e2e8f0', fontSize: 12, color: '#475569' }}>
              上部ツールバーの「テキスト」「注釈」「ハイライト」ツールを使用してドキュメントに直接書き込みができます。
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
