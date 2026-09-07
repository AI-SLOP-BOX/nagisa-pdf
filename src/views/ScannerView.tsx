import { useState, useRef, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { CameraIcon, ImageIcon, ChevronUpIcon, ChevronDownIcon, ChevronLeftIcon, ChevronRightIcon, CloseIcon } from '../components/Icons'
import { useToast } from '../hooks/useToast'
import { formatError } from '../utils/errorHandler'

interface ScannedFile {
  path: string
  name: string
  thumbnail?: string
}

import type { View } from '../types'

interface ScannerViewProps {
  currentView?: View
  onNavigateView?: (view: View) => void
}

export default function ScannerView({ currentView = 'scanner', onNavigateView }: ScannerViewProps) {
  const [files, setFiles] = useState<ScannedFile[]>([])
  const [isProcessing, setIsProcessing] = useState(false)
  const { toast, toastType, showToast, showError, showSuccess } = useToast(2800)
  const [removeShadow, setRemoveShadow] = useState(true)
  const [correctPerspective, setCorrectPerspective] = useState(true)
  const [outputDPI, setOutputDPI] = useState(300)
  const [outputFormat, setOutputFormat] = useState<'pdf' | 'png' | 'jpg'>('pdf')
  const [selectedIdx, setSelectedIdx] = useState<number | null>(null)
  const [batchProgress, setBatchProgress] = useState<{ current: number; total: number } | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleAddFiles = useCallback(async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'tiff', 'bmp'] }],
    })
    if (!selected) return
    const paths = Array.isArray(selected) ? selected : [selected]
    const newFiles = paths.map(p => ({
      path: p,
      name: p.split(/[/\\]/).pop() || 'image',
    }))
    setFiles(prev => [...prev, ...newFiles])
    showToast(`${newFiles.length}枚の画像を追加`)
  }, [])

  const handleRemoveFile = useCallback((index: number) => {
    setFiles(prev => prev.filter((_, i) => i !== index))
    if (selectedIdx === index) setSelectedIdx(null)
    else if (selectedIdx !== null && selectedIdx > index) setSelectedIdx(prev => prev! - 1)
  }, [selectedIdx])

  const handleMoveUp = useCallback((index: number) => {
    if (index === 0) return
    setFiles(prev => {
      const next = [...prev]
      ;[next[index - 1], next[index]] = [next[index], next[index - 1]]
      return next
    })
    if (selectedIdx === index) setSelectedIdx(index - 1)
    else if (selectedIdx === index - 1) setSelectedIdx(index)
  }, [selectedIdx])

  const handleMoveDown = useCallback((index: number) => {
    setFiles(prev => {
      if (index >= prev.length - 1) return prev
      const next = [...prev]
      ;[next[index], next[index + 1]] = [next[index + 1], next[index]]
      return next
    })
  }, [])

  const handleProcess = useCallback(async () => {
    if (files.length === 0) return
    setIsProcessing(true)
    setBatchProgress({ current: 0, total: files.length })

    try {
      const result = await invoke<number[]>('process_scanned_images', {
        paths: files.map(f => f.path),
        removeShadow,
        correctPerspective,
        dpi: outputDPI,
      })

      if (outputFormat === 'pdf') {
        const path = await save({
          defaultPath: 'scanned_document.pdf',
          filters: [{ name: 'PDF', extensions: ['pdf'] }],
        })
        if (path) {
          await invoke('write_file_bytes', { path, data: result })
          showSuccess(`${files.length}枚をPDFに変換しました`)
        }
      } else {
        const path = await save({
          defaultPath: `scanned_image.${outputFormat}`,
          filters: [{ name: outputFormat.toUpperCase(), extensions: [outputFormat] }],
        })
        if (path) {
          await invoke('write_file_bytes', { path, data: result })
          showSuccess('画像を保存しました')
        }
      }
    } catch (err) {
      showError(formatError(err, 'スキャン画像の処理に失敗しました'))
    } finally {
      setIsProcessing(false)
      setBatchProgress(null)
    }
  }, [files, removeShadow, correctPerspective, outputDPI, outputFormat, showError, showSuccess])

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    const dropped = Array.from(e.dataTransfer.files)
      .filter(f => /\.(png|jpe?g|webp|tiff|bmp)$/i.test(f.name))
    if (dropped.length > 0) {
      const newFiles = dropped.map(f => ({
        path: f.name,
        name: f.name,
      }))
      setFiles(prev => [...prev, ...newFiles])
      showToast(`${dropped.length}枚の画像を追加`)
    }
  }, [showToast])

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }} onDragOver={e => e.preventDefault()} onDrop={handleDrop}>
      {toast && <div style={{
        position: 'fixed', top: 16, right: 16, zIndex: 1000,
        background: 'var(--bg-2)',
        border: `1px solid ${toastType === 'error' ? 'var(--red, #f85149)' : 'var(--accent)'}`,
        color: toastType === 'error' ? 'var(--red, #f85149)' : 'var(--accent)',
        padding: '10px 20px', borderRadius: 'var(--radius)',
        boxShadow: 'var(--shadow)', fontSize: 13,
        maxWidth: 420,
      }}>{toast}</div>}

      {/* Header */}
      <div style={{
        padding: '8px 16px', background: 'var(--bg-1)', borderBottom: '1px solid var(--border)',
        display: 'flex', alignItems: 'center', gap: 10, flexShrink: 0,
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginRight: 6 }}>
          <img
            src="/favicon.png"
            alt="Nagisa PDF"
            style={{ width: 22, height: 22, borderRadius: 4, boxShadow: '0 2px 8px rgba(47, 129, 247, 0.4)' }}
          />
          <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--text)', letterSpacing: '-0.02em' }}>
            Nagisa PDF
          </span>
        </div>

        {/* Global Workspace View Switcher */}
        {onNavigateView && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 2,
              padding: 2,
              background: 'var(--bg-0)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius-sm)',
            }}
          >
            <button
              onClick={() => onNavigateView('pdf-editor')}
              className={`mode-switcher-btn ${currentView === 'pdf-editor' ? 'active' : ''}`}
              title="PDF 編集・閲覧"
            >
              <span>PDF編集</span>
            </button>
            <button
              onClick={() => onNavigateView('scanner')}
              className={`mode-switcher-btn ${currentView === 'scanner' ? 'active' : ''}`}
              title="カメラ・画像スキャン補正"
            >
              <span>スキャナ</span>
            </button>
            <button
              onClick={() => onNavigateView('ocr')}
              className={`mode-switcher-btn ${currentView === 'ocr' ? 'active' : ''}`}
              title="OCR文字認識 & EPUB/テキスト変換"
            >
              <span>OCR</span>
            </button>
          </div>
        )}

        <div style={{ width: 1, height: 16, background: 'var(--border)', margin: '0 4px' }} />

        <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-dim)' }}>スキャン補正 (影除去・台形補正・高画質化)</div>

        <div style={{ flex: 1 }} />
        <button onClick={handleAddFiles} style={{
          padding: '6px 14px', background: 'var(--accent)', color: 'var(--bg-0)',
          border: 'none', borderRadius: 'var(--radius-sm)', fontSize: 12, fontWeight: 600,
        }}>画像を追加</button>
        <button onClick={handleProcess} disabled={files.length === 0 || isProcessing} style={{
          padding: '6px 14px', background: 'var(--purple)', color: 'var(--bg-0)',
          border: 'none', borderRadius: 'var(--radius-sm)', fontSize: 12, fontWeight: 600,
          opacity: files.length === 0 || isProcessing ? 0.4 : 1,
        }}>
          {isProcessing ? (batchProgress ? `処理中 (${batchProgress.current}/${batchProgress.total})...` : '処理中...') : 'PDF化 / 保存'}
        </button>
      </div>

      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        {/* Left: File List + Settings */}
        <div style={{
          width: 320, background: 'var(--bg-1)', borderRight: '1px solid var(--border)',
          display: 'flex', flexDirection: 'column', flexShrink: 0,
        }}>
          <div style={{ padding: '12px 16px', borderBottom: '1px solid var(--border)', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase' }}>
              スキャン画像 ({files.length})
            </span>
            {files.length > 0 && (
              <button onClick={() => { setFiles([]); setSelectedIdx(null) }}
                style={{ background: 'transparent', border: 'none', color: 'var(--red)', fontSize: 11, cursor: 'pointer' }}>
                全クリア
              </button>
            )}
          </div>

          {/* Thumbnail list */}
          <div style={{ flex: 1, overflowY: 'auto', padding: 8, display: 'flex', flexDirection: 'column', gap: 4 }}>
            {files.map((f, i) => (
              <div
                key={i}
                onClick={() => setSelectedIdx(i)}
                style={{
                  display: 'flex', alignItems: 'center', gap: 8, padding: '6px 8px',
                  borderRadius: 'var(--radius-sm)',
                  background: selectedIdx === i ? 'var(--bg-active)' : 'transparent',
                  border: `1px solid ${selectedIdx === i ? 'var(--accent)' : 'transparent'}`,
                  cursor: 'pointer',
                  transition: 'all 0.12s',
                }}
              >
                <span style={{ fontSize: 11, color: 'var(--text-muted)', minWidth: 16 }}>{i + 1}</span>
                <ImageIcon size={16} color="var(--accent)" />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 12, color: 'var(--text)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{f.name}</div>
                </div>
                <div style={{ display: 'flex', gap: 3, alignItems: 'center' }}>
                  <button
                    onClick={(e) => { e.stopPropagation(); handleMoveUp(i) }}
                    disabled={i === 0}
                    style={{
                      width: 22, height: 22, border: 'none', borderRadius: 4,
                      background: 'var(--bg-3)', color: 'var(--text-dim)',
                      display: 'flex', alignItems: 'center', justifyContent: 'center',
                      cursor: i === 0 ? 'not-allowed' : 'pointer', opacity: i === 0 ? 0.3 : 1
                    }}
                    title="上へ"
                  >
                    <ChevronUpIcon size={12} />
                  </button>
                  <button
                    onClick={(e) => { e.stopPropagation(); handleMoveDown(i) }}
                    disabled={i === files.length - 1}
                    style={{
                      width: 22, height: 22, border: 'none', borderRadius: 4,
                      background: 'var(--bg-3)', color: 'var(--text-dim)',
                      display: 'flex', alignItems: 'center', justifyContent: 'center',
                      cursor: i === files.length - 1 ? 'not-allowed' : 'pointer', opacity: i === files.length - 1 ? 0.3 : 1
                    }}
                    title="下へ"
                  >
                    <ChevronDownIcon size={12} />
                  </button>
                  <button
                    onClick={(e) => { e.stopPropagation(); handleRemoveFile(i) }}
                    style={{
                      width: 22, height: 22, border: 'none', borderRadius: 4,
                      background: 'rgba(248, 81, 73, 0.15)', color: '#f85149',
                      display: 'flex', alignItems: 'center', justifyContent: 'center',
                      cursor: 'pointer'
                    }}
                    title="削除"
                  >
                    <CloseIcon size={12} />
                  </button>
                </div>
              </div>
            ))}
            {files.length === 0 && (
              <div style={{ textAlign: 'center', padding: '32px 16px', color: 'var(--text-muted)', fontSize: 12 }}>
                <div style={{ marginBottom: 12, opacity: 0.3, display: 'flex', justifyContent: 'center' }}>
                  <CameraIcon size={36} />
                </div>
                画像を追加してください<br/>
                <span style={{ fontSize: 11 }}>ドラッグ＆ドロップにも対応</span>
              </div>
            )}
          </div>
        </div>

        {/* Preview */}
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'var(--bg-0)' }}>
          {selectedIdx !== null && files[selectedIdx] ? (
            <div style={{ textAlign: 'center' }}>
              <div style={{
                width: 400, height: 500, background: 'var(--bg-2)', borderRadius: 'var(--radius)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                border: '1px solid var(--border)',
              }}>
                <div style={{ color: 'var(--text-muted)', fontSize: 14 }}>
                  <div style={{ marginBottom: 12, display: 'flex', justifyContent: 'center' }}>
                    <ImageIcon size={48} color="var(--text-dim)" />
                  </div>
                  {files[selectedIdx].name}
                  <div style={{ fontSize: 12, marginTop: 8 }}>
                    {selectedIdx + 1} / {files.length}
                  </div>
                </div>
              </div>
              <div style={{ display: 'flex', gap: 8, justifyContent: 'center', marginTop: 12 }}>
                <button
                  onClick={() => setSelectedIdx(prev => prev! > 0 ? prev! - 1 : prev)}
                  disabled={selectedIdx === 0}
                  style={{ ...navBtnStyle, display: 'inline-flex', alignItems: 'center', gap: 4 }}
                >
                  <ChevronLeftIcon size={14} /> 前へ
                </button>
                <button
                  onClick={() => setSelectedIdx(prev => prev! < files.length - 1 ? prev! + 1 : prev)}
                  disabled={selectedIdx === files.length - 1}
                  style={{ ...navBtnStyle, display: 'inline-flex', alignItems: 'center', gap: 4 }}
                >
                  次へ <ChevronRightIcon size={14} />
                </button>
              </div>
            </div>
          ) : (
            <div style={{ textAlign: 'center', color: 'var(--text-muted)' }}>
              <div style={{ marginBottom: 16, opacity: 0.25, display: 'flex', justifyContent: 'center' }}>
                <CameraIcon size={48} color="var(--text-dim)" />
              </div>
              <p style={{ fontSize: 14 }}>画像を選択してください</p>
              <p style={{ fontSize: 12, marginTop: 8 }}>
                スマホで撮影した書類の影・歪みを自動補正します
              </p>
            </div>
          )}
        </div>

        {/* Settings */}
        <div style={{
          width: 240, background: 'var(--bg-1)', borderLeft: '1px solid var(--border)',
          padding: 16, flexShrink: 0, overflowY: 'auto',
        }}>
          <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase', marginBottom: 12 }}>
            処理設定
          </div>

          <Checkbox label="透視補正（歪み修正）" checked={correctPerspective} onChange={setCorrectPerspective} />
          <Checkbox label="影・裏写り除去" checked={removeShadow} onChange={setRemoveShadow} />

          <div style={{ marginTop: 16 }}>
            <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', marginBottom: 4 }}>出力DPI</label>
            <select value={outputDPI} onChange={e => setOutputDPI(Number(e.target.value))}
              style={{ width: '100%', padding: '6px 8px', background: 'var(--bg-0)', border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)', color: 'var(--text)', fontSize: 13 }}>
              <option value={150}>150 DPI（軽量）</option>
              <option value={200}>200 DPI</option>
              <option value={300}>300 DPI（標準）</option>
              <option value={600}>600 DPI（高画質）</option>
            </select>
          </div>

          <div style={{ marginTop: 16 }}>
            <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', marginBottom: 4 }}>出力形式</label>
            <div style={{ display: 'flex', gap: 4 }}>
              {(['pdf', 'png', 'jpg'] as const).map(fmt => (
                <button key={fmt} onClick={() => setOutputFormat(fmt)}
                  style={{
                    flex: 1, padding: '6px', border: '1px solid',
                    borderColor: outputFormat === fmt ? 'var(--accent)' : 'var(--border)',
                    borderRadius: 'var(--radius-sm)',
                    background: outputFormat === fmt ? 'var(--accent-dim)' : 'var(--bg-0)',
                    color: outputFormat === fmt ? 'var(--accent)' : 'var(--text-dim)',
                    fontSize: 12, fontWeight: 600, textTransform: 'uppercase',
                  }}>{fmt}</button>
              ))}
            </div>
          </div>

          <div style={{ marginTop: 20, padding: 12, background: 'var(--bg-0)', borderRadius: 'var(--radius-sm)', border: '1px solid var(--border)' }}>
            <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-dim)', marginBottom: 8 }}>処理内容</div>
            <ul style={{ fontSize: 11, color: 'var(--text-muted)', lineHeight: 1.8, paddingLeft: 16 }}>
              <li>四隅の自動検出＆透視補正</li>
              <li>影・背景の均一化除去</li>
              <li>ドキュメント領域の切り出し</li>
              <li>指定DPIでリサイズ</li>
              <li>バッチ一括処理</li>
            </ul>
          </div>

          <div style={{ marginTop: 16, padding: 12, background: 'var(--bg-0)', borderRadius: 'var(--radius-sm)', border: '1px solid var(--border)' }}>
            <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-dim)', marginBottom: 4 }}>対応形式</div>
            <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>
              PNG, JPEG, WebP, TIFF, BMP
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

const navBtnStyle: React.CSSProperties = {
  padding: '5px 14px', background: 'var(--bg-2)', border: '1px solid var(--border)',
  borderRadius: 'var(--radius-sm)', color: 'var(--text-dim)', fontSize: 12, cursor: 'pointer',
}

function Checkbox({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 0', fontSize: 13, color: 'var(--text)', cursor: 'pointer' }}>
      <input type="checkbox" checked={checked} onChange={e => onChange(e.target.checked)}
        style={{ width: 16, height: 16, accentColor: 'var(--accent)' }} />
      {label}
    </label>
  )
}
