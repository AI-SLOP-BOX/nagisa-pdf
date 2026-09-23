import {
  FolderOpenIcon,
  SaveIcon,
  PrinterIcon,
  UndoIcon,
  RedoIcon,
  ExportIcon,
  SearchIcon,
  EditIcon,
  TypeIcon,
  HighlightIcon,
  RectIcon,
  AnnotateIcon,
  OrganizeIcon,
} from './Icons'

interface EditorTopBarProps {
  fileName: string
  pageCount: number
  currentPage: number
  onPageChange: (page: number) => void
  zoom: number
  onZoomChange: (fn: (prev: number) => number) => void
  onOpen: () => void
  onSave: () => void
  onPrint?: () => void
  onUndo: () => void
  onRedo: () => void
  canUndo: boolean
  canRedo: boolean
  editorMode: 'inspect' | 'edit'
  setEditorMode: (mode: 'inspect' | 'edit') => void
  searchQuery: string
  setSearchQuery: (q: string) => void
  onCloseDocument?: () => void
  onNewTab?: () => void
  selectedEditTool?: string
  setSelectedEditTool?: (t: string) => void
  onRunOCR?: () => void
}

export const EditorTopBar: React.FC<EditorTopBarProps> = ({
  fileName,
  pageCount,
  currentPage,
  onPageChange,
  zoom,
  onZoomChange,
  onOpen,
  onSave,
  onPrint,
  onUndo,
  onRedo,
  canUndo,
  canRedo,
  editorMode,
  setEditorMode,
  searchQuery,
  setSearchQuery,
  onCloseDocument,
  onNewTab,
  selectedEditTool = 'select',
  setSelectedEditTool,
  onRunOCR,
}) => {
  const hasDocument = Boolean(fileName && pageCount > 0)
  const effectivePageCount = Math.max(pageCount, 1)

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        background: '#ffffff',
        borderBottom: '1px solid #e2e8f0',
        flexShrink: 0,
        userSelect: 'none',
        zIndex: 20,
      }}
    >
      {/* 1. Document Tab Row */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          height: 38,
          background: '#f8fafc',
          borderBottom: '1px solid #e2e8f0',
          padding: '0 12px',
          gap: 6,
        }}
      >
        {/* Active Document Tab */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            height: 30,
            padding: '0 12px',
            background: '#ffffff',
            borderRadius: '7px 7px 0 0',
            border: '1px solid #e2e8f0',
            borderBottom: '1px solid #ffffff',
            boxShadow: '0 -1px 3px rgba(0,0,0,0.02)',
            fontSize: 12.5,
            fontWeight: 600,
            color: '#0f172a',
            cursor: 'pointer',
          }}
        >
          {/* Red PDF Icon Badge */}
          <div
            style={{
              width: 17,
              height: 19,
              background: '#ef4444',
              borderRadius: 3,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              flexShrink: 0,
            }}
          >
            <span style={{ color: '#fff', fontSize: 6.5, fontWeight: 900 }}>PDF</span>
          </div>
          <span style={{ maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {fileName || '名称未設定.pdf'}
          </span>
          {onCloseDocument && (
            <button
              onClick={e => {
                e.stopPropagation()
                onCloseDocument()
              }}
              title="閉じる"
              style={{
                background: 'transparent',
                border: 'none',
                color: '#94a3b8',
                fontSize: 14,
                cursor: 'pointer',
                padding: '0 2px',
                marginLeft: 4,
                lineHeight: 1,
              }}
            >
              ×
            </button>
          )}
        </div>

        {/* Add Tab Button */}
        <button
          onClick={onNewTab || onOpen}
          title="新規タブで開く"
          style={{
            width: 26,
            height: 26,
            borderRadius: 6,
            background: 'transparent',
            border: 'none',
            color: '#64748b',
            fontSize: 16,
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
          onMouseEnter={e => (e.currentTarget.style.background = '#e2e8f0')}
          onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
        >
          +
        </button>
      </div>

      {/* 2. Primary Action Row: Left (File/History) | Center (Edit Tools) | Right (View/Export) */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          height: 48,
          padding: '0 16px',
          gap: 8,
          position: 'relative',
        }}
      >
        {/* Left Section: File Operations & Minimal Icon Undo/Redo */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
          {/* Open Button */}
          <button
            onClick={onOpen}
            title="PDFファイルを開く (⌘O)"
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              padding: '6px 12px',
              borderRadius: 7,
              background: hasDocument ? '#f8fafc' : '#2563eb',
              border: `1px solid ${hasDocument ? '#e2e8f0' : '#1d4ed8'}`,
              fontSize: 12.5,
              fontWeight: 600,
              color: hasDocument ? '#1e293b' : '#ffffff',
              cursor: 'pointer',
              boxShadow: hasDocument ? 'none' : '0 2px 6px rgba(37, 99, 235, 0.25)',
            }}
            onMouseEnter={e => (e.currentTarget.style.background = hasDocument ? '#f1f5f9' : '#1d4ed8')}
            onMouseLeave={e => (e.currentTarget.style.background = hasDocument ? '#f8fafc' : '#2563eb')}
          >
            <FolderOpenIcon size={15} color={hasDocument ? '#475569' : '#ffffff'} />
            <span>開く</span>
          </button>

          {/* Save Button */}
          {hasDocument && (
            <button
              onClick={onSave}
              title="保存 (⌘S)"
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 5,
                padding: '6px 11px',
                borderRadius: 7,
                background: '#f8fafc',
                border: '1px solid #e2e8f0',
                fontSize: 12.5,
                fontWeight: 600,
                color: '#1e293b',
                cursor: 'pointer',
              }}
              onMouseEnter={e => (e.currentTarget.style.background = '#f1f5f9')}
              onMouseLeave={e => (e.currentTarget.style.background = '#f8fafc')}
            >
              <SaveIcon size={15} color="#475569" />
              <span>保存</span>
            </button>
          )}

          {/* Print Button */}
          {hasDocument && onPrint && (
            <button
              onClick={onPrint}
              title="印刷 (⌘P)"
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 5,
                padding: '6px 11px',
                borderRadius: 7,
                background: '#f8fafc',
                border: '1px solid #e2e8f0',
                fontSize: 12.5,
                fontWeight: 600,
                color: '#1e293b',
                cursor: 'pointer',
              }}
              onMouseEnter={e => (e.currentTarget.style.background = '#f1f5f9')}
              onMouseLeave={e => (e.currentTarget.style.background = '#f8fafc')}
            >
              <PrinterIcon size={15} color="#475569" />
              <span>印刷</span>
            </button>
          )}

          {/* Compact 28x28 Undo / Redo Icon Buttons */}
          {hasDocument && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                background: '#f8fafc',
                border: '1px solid #e2e8f0',
                borderRadius: 7,
                padding: 2,
                marginLeft: 4,
                gap: 1,
              }}
            >
              <button
                onClick={onUndo}
                disabled={!canUndo}
                title="元に戻す (⌘Z)"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  width: 26,
                  height: 26,
                  borderRadius: 5,
                  background: 'transparent',
                  border: 'none',
                  color: canUndo ? '#334155' : '#cbd5e1',
                  cursor: canUndo ? 'pointer' : 'default',
                }}
                onMouseEnter={e => {
                  if (canUndo) e.currentTarget.style.background = '#e2e8f0'
                }}
                onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
              >
                <UndoIcon size={14} />
              </button>
              <button
                onClick={onRedo}
                disabled={!canRedo}
                title="やり直す (⌘⇧Z)"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  width: 26,
                  height: 26,
                  borderRadius: 5,
                  background: 'transparent',
                  border: 'none',
                  color: canRedo ? '#334155' : '#cbd5e1',
                  cursor: canRedo ? 'pointer' : 'default',
                }}
                onMouseEnter={e => {
                  if (canRedo) e.currentTarget.style.background = '#e2e8f0'
                }}
                onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
              >
                <RedoIcon size={14} />
              </button>
            </div>
          )}
        </div>

        {/* Center Section: Compact Icon-Only Tool Palette (Figma / Mac Style) */}
        {hasDocument && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              background: '#f8fafc',
              border: '1px solid #e2e8f0',
              borderRadius: 8,
              padding: '2px 4px',
              gap: 2,
              boxShadow: '0 1px 2px rgba(0,0,0,0.03)',
            }}
          >
            {[
              { key: 'select', label: '選択・テキストコピー (V)', icon: <span style={{ fontSize: 13, fontWeight: 900 }}>↖</span> },
              { key: 'text', label: 'テキスト追記・文字追加 (T) - クリックして新規テキストを入力', icon: <TypeIcon size={14} /> },
              { key: 'highlight', label: 'ハイライトマーカー (H) - ドラッグして蛍光ペンを引く', icon: <HighlightIcon size={14} /> },
              { key: 'annotate', label: '注釈・付箋メモ (A) - メモ欄を配置', icon: <AnnotateIcon size={14} /> },
              { key: 'shape', label: '図形・枠線描画 (R) - 矩形や枠線を描く', icon: <RectIcon size={14} /> },
              { key: 'ocr', label: 'OCR文字抽出 - スキャン文書からテキストを検出', icon: <span style={{ fontSize: 13 }}>🔍</span> },
              { key: 'pages', label: 'ページ整理・回転 (P)', icon: <OrganizeIcon size={14} /> },
            ].map(tool => {
              const active = selectedEditTool === tool.key
              return (
                <button
                  key={tool.key}
                  title={tool.label}
                  onClick={() => {
                    if (editorMode !== 'edit') {
                      setEditorMode('edit')
                    }
                    setSelectedEditTool && setSelectedEditTool(tool.key)
                    if (tool.key === 'ocr' && onRunOCR) {
                      onRunOCR()
                    }
                  }}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    width: 32,
                    height: 28,
                    borderRadius: 6,
                    background: active ? '#2563eb' : 'transparent',
                    border: 'none',
                    color: active ? '#ffffff' : '#475569',
                    cursor: 'pointer',
                    transition: 'all 0.15s ease',
                  }}
                  onMouseEnter={e => {
                    if (!active) e.currentTarget.style.background = '#e2e8f0'
                  }}
                  onMouseLeave={e => {
                    if (!active) e.currentTarget.style.background = 'transparent'
                  }}
                >
                  {tool.icon}
                </button>
              )
            })}
          </div>
        )}

        {/* Right Section: Page Navigation, Zoom, Search & Export */}
        {hasDocument && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }}>
            {/* Page Navigation: ‹ 1 / N › */}
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                background: '#f8fafc',
                border: '1px solid #e2e8f0',
                borderRadius: 7,
                height: 30,
                padding: '0 4px',
                gap: 2,
              }}
            >
              <button
                onClick={() => onPageChange(Math.max(0, currentPage - 1))}
                disabled={currentPage === 0}
                title="前のページ"
                style={{
                  background: 'transparent',
                  border: 'none',
                  width: 22,
                  height: 22,
                  color: currentPage > 0 ? '#334155' : '#cbd5e1',
                  fontSize: 13,
                  cursor: currentPage > 0 ? 'pointer' : 'default',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                ‹
              </button>
              <input
                type="text"
                value={currentPage + 1}
                onChange={e => {
                  const p = parseInt(e.target.value, 10)
                  if (!isNaN(p) && p >= 1 && p <= effectivePageCount) {
                    onPageChange(p - 1)
                  }
                }}
                style={{
                  width: 26,
                  height: 20,
                  textAlign: 'center',
                  background: '#ffffff',
                  border: '1px solid #cbd5e1',
                  borderRadius: 4,
                  fontSize: 11.5,
                  fontWeight: 600,
                  color: '#0f172a',
                  outline: 'none',
                  padding: 0,
                }}
              />
              <span style={{ fontSize: 11.5, color: '#64748b', paddingRight: 2 }}>
                / {effectivePageCount}
              </span>
              <button
                onClick={() => onPageChange(Math.min(effectivePageCount - 1, currentPage + 1))}
                disabled={currentPage >= effectivePageCount - 1}
                title="次のページ"
                style={{
                  background: 'transparent',
                  border: 'none',
                  width: 22,
                  height: 22,
                  color: currentPage < effectivePageCount - 1 ? '#334155' : '#cbd5e1',
                  fontSize: 13,
                  cursor: currentPage < effectivePageCount - 1 ? 'pointer' : 'default',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                ›
              </button>
            </div>

            {/* Zoom Controls: − [ 100% ▾ ] ＋ */}
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                background: '#f8fafc',
                border: '1px solid #e2e8f0',
                borderRadius: 7,
                height: 30,
                padding: '0 2px',
              }}
            >
              <button
                onClick={() => onZoomChange(z => Math.max(0.4, Number((z - 0.1).toFixed(1))))}
                title="縮小"
                style={{
                  background: 'transparent',
                  border: 'none',
                  width: 22,
                  height: 22,
                  fontSize: 14,
                  color: '#475569',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                −
              </button>
              <select
                value={Number(zoom.toFixed(2))}
                onChange={e => {
                  const val = parseFloat(e.target.value)
                  if (!isNaN(val)) {
                    onZoomChange(() => val)
                  }
                }}
                title="表示倍率を変更"
                style={{
                  fontSize: 11.5,
                  fontWeight: 600,
                  color: '#1e293b',
                  background: 'transparent',
                  border: 'none',
                  outline: 'none',
                  cursor: 'pointer',
                  padding: '0 2px',
                  textAlign: 'center',
                }}
              >
                <option value={0.5}>50%</option>
                <option value={0.75}>75%</option>
                <option value={1.0}>100% (原寸)</option>
                <option value={1.25}>125%</option>
                <option value={1.5}>150%</option>
                <option value={2.0}>200%</option>
              </select>
              <button
                onClick={() => onZoomChange(z => Math.min(3.0, Number((z + 0.1).toFixed(1))))}
                title="拡大"
                style={{
                  background: 'transparent',
                  border: 'none',
                  width: 22,
                  height: 22,
                  fontSize: 14,
                  color: '#475569',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                +
              </button>
            </div>

            {/* Search Input (Compact) */}
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 5,
                background: '#f8fafc',
                border: '1px solid #e2e8f0',
                borderRadius: 7,
                height: 30,
                padding: '0 8px',
                width: 120,
              }}
            >
              <SearchIcon size={13} color="#94a3b8" />
              <input
                type="text"
                placeholder="検索..."
                value={searchQuery}
                onChange={e => setSearchQuery(e.target.value)}
                style={{
                  border: 'none',
                  background: 'transparent',
                  fontSize: 11.5,
                  color: '#1e293b',
                  width: '100%',
                  outline: 'none',
                  padding: 0,
                }}
              />
            </div>

            {/* Export Action Button */}
            <button
              onClick={onSave}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 5,
                background: '#2563eb',
                color: '#ffffff',
                border: 'none',
                borderRadius: 7,
                padding: '6px 12px',
                fontSize: 12,
                fontWeight: 600,
                cursor: 'pointer',
                boxShadow: '0 2px 6px rgba(37, 99, 235, 0.25)',
              }}
              onMouseEnter={e => (e.currentTarget.style.background = '#1d4ed8')}
              onMouseLeave={e => (e.currentTarget.style.background = '#2563eb')}
            >
              <ExportIcon size={13} color="#fff" />
              <span>書き出し</span>
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
