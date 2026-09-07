import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { TypeIcon, FileIcon, CheckIcon, CloseIcon } from '../components/Icons'
import { useToast } from '../hooks/useToast'
import { formatError } from '../utils/errorHandler'

import type { View } from '../types'

interface OCRViewProps {
  currentView?: View
  onNavigateView?: (view: View) => void
}

export default function OCRView({ currentView = 'ocr', onNavigateView }: OCRViewProps) {
  const [files, setFiles] = useState<string[]>([])
  const [isProcessing, setIsProcessing] = useState(false)
  const { toast, toastType, showToast, showError, showSuccess } = useToast(2800)
  const [ocrLanguage, setOcrLanguage] = useState('jpn')
  const [outputMode, setOutputMode] = useState<'epub' | 'txt' | 'searchable-pdf'>('epub')
  const [result, setResult] = useState<{ text: string; confidence: number } | null>(null)

  const handleAddFiles = async () => {
    const selected = await open({
      multiple: true,
      filters: [
        { name: 'PDF', extensions: ['pdf'] },
        { name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'tiff'] },
      ],
    })
    if (!selected) return
    const paths = Array.isArray(selected) ? selected : [selected]
    setFiles(prev => [...prev, ...paths])
  }

  const handleProcess = async () => {
    if (files.length === 0) return
    setIsProcessing(true)
    try {
      const ocrResult = await invoke<{ text: string; confidence: number; pageCount: number }>('ocr_files', {
        paths: files,
        language: ocrLanguage,
      })

      if (outputMode === 'epub') {
        const path = await save({
          defaultPath: 'converted.epub',
          filters: [{ name: 'EPUB', extensions: ['epub'] }],
        })
        if (path) {
          await invoke('create_epub', {
            text: ocrResult.text,
            output_path: path,
            title: 'OCR Converted Document',
          })
          showSuccess(`EPUBを生成しました (信頼度: ${Math.round(ocrResult.confidence)}%)`)
        }
      } else if (outputMode === 'txt') {
        const path = await save({
          defaultPath: 'ocr_result.txt',
          filters: [{ name: 'Text', extensions: ['txt'] }],
        })
        if (path) {
          await invoke('write_text_file', { path, content: ocrResult.text })
          showSuccess('テキストファイルを保存しました')
        }
      } else {
        const path = await save({
          defaultPath: 'searchable.pdf',
          filters: [{ name: 'PDF', extensions: ['pdf'] }],
        })
        if (path) {
          await invoke('create_searchable_pdf', {
            original_paths: files,
            ocr_text: ocrResult.text,
            output_path: path,
          })
          showSuccess('検索可能PDFを作成しました')
        }
      }

      setResult({ text: ocrResult.text, confidence: ocrResult.confidence })
    } catch (err) {
      showError(formatError(err, 'OCR文字認識に失敗しました'))
    } finally {
      setIsProcessing(false)
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
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

        <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-dim)' }}>OCR / EPUB・テキスト変換</div>

        <div style={{ flex: 1 }} />
        <button onClick={handleAddFiles} style={{
          padding: '6px 14px', background: 'var(--accent)', color: 'var(--bg-0)',
          border: 'none', borderRadius: 'var(--radius-sm)', fontSize: 12, fontWeight: 600,
        }}>ファイルを追加</button>
        <button onClick={handleProcess} disabled={files.length === 0 || isProcessing} style={{
          padding: '6px 14px', background: 'var(--purple)', color: 'var(--bg-0)',
          border: 'none', borderRadius: 'var(--radius-sm)', fontSize: 12, fontWeight: 600,
          opacity: files.length === 0 || isProcessing ? 0.4 : 1,
        }}>
          {isProcessing ? 'OCR処理中...' : 'OCR実行'}
        </button>
      </div>

      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        {/* Left: File List + Settings */}
        <div style={{
          width: 300, background: 'var(--bg-1)', borderRight: '1px solid var(--border)',
          display: 'flex', flexDirection: 'column', flexShrink: 0,
        }}>
          <div style={{ padding: '12px', borderBottom: '1px solid var(--border)' }}>
            <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase', marginBottom: 8 }}>
              入力ファイル ({files.length})
            </div>
            <div style={{ maxHeight: 120, overflowY: 'auto' }}>
              {files.map((f, i) => (
                <div key={i} style={{
                  display: 'flex', alignItems: 'center', gap: 6, padding: '4px 6px',
                  borderRadius: 3, fontSize: 12, color: 'var(--text-dim)',
                }}>
                  <FileIcon size={14} color="var(--accent)" />
                  <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {f.split(/[/\\]/).pop()}
                  </span>
                  <button
                    onClick={() => setFiles(prev => prev.filter((_, j) => j !== i))}
                    style={{
                      width: 18, height: 18, border: 'none', borderRadius: 4,
                      background: 'rgba(248, 81, 73, 0.15)', color: '#f85149',
                      display: 'flex', alignItems: 'center', justifyContent: 'center',
                      cursor: 'pointer'
                    }}
                    title="削除"
                  >
                    <CloseIcon size={10} />
                  </button>
                </div>
              ))}
            </div>
          </div>

          <div style={{ padding: 12, flex: 1, overflowY: 'auto' }}>
            <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase', marginBottom: 12 }}>
              OCR設定
            </div>

            <div style={{ marginBottom: 12 }}>
              <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', marginBottom: 4 }}>認識言語</label>
              <select
                value={ocrLanguage}
                onChange={e => setOcrLanguage(e.target.value)}
                style={{
                  width: '100%', padding: '6px 8px', background: 'var(--bg-0)',
                  border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)',
                  color: 'var(--text)', fontSize: 13,
                }}
              >
                <option value="jpn">日本語</option>
                <option value="eng">英語</option>
                <option value="jpn+eng">日本語 + 英語</option>
                <option value="chi_sim">簡体字中国語</option>
                <option value="kor">韓国語</option>
              </select>
            </div>

            <div style={{ marginBottom: 12 }}>
              <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', marginBottom: 6 }}>出力形式</label>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                {([
                  { value: 'epub', label: 'EPUB', desc: '電子書籍リーダー向け' },
                  { value: 'txt', label: 'テキスト', desc: 'プレーンテキスト' },
                  { value: 'searchable-pdf', label: '検索可能PDF', desc: 'OCRレイヤー付きPDF' },
                ] as const).map(opt => (
                  <label
                    key={opt.value}
                    style={{
                      display: 'flex', alignItems: 'center', gap: 8, padding: '8px 10px',
                      borderRadius: 'var(--radius-sm)', cursor: 'pointer',
                      background: outputMode === opt.value ? 'var(--bg-active)' : 'var(--bg-0)',
                      border: `1px solid ${outputMode === opt.value ? 'var(--accent)' : 'var(--border)'}`,
                      transition: 'all 0.12s',
                    }}
                  >
                    <input
                      type="radio"
                      name="output"
                      checked={outputMode === opt.value}
                      onChange={() => setOutputMode(opt.value)}
                      style={{ accentColor: 'var(--accent)' }}
                    />
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text)' }}>{opt.label}</div>
                      <div style={{ fontSize: 10, color: 'var(--text-muted)' }}>{opt.desc}</div>
                    </div>
                  </label>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* Right: Result */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          {result ? (
            <>
              <div style={{
                padding: '8px 16px', background: 'var(--bg-1)', borderBottom: '1px solid var(--border)',
                display: 'flex', alignItems: 'center', gap: 12, fontSize: 12,
              }}>
                <span style={{ color: 'var(--green)', display: 'flex', alignItems: 'center', gap: 4, fontWeight: 600 }}>
                  <CheckIcon size={14} color="var(--green)" /> OCR完了
                </span>
                <span style={{ color: 'var(--text-dim)' }}>信頼度: {Math.round(result.confidence)}%</span>
                <span style={{ color: 'var(--text-dim)' }}>{result.text.length}文字</span>
              </div>
              <div style={{
                flex: 1, overflow: 'auto', padding: 20, background: 'var(--bg-0)',
                fontFamily: 'monospace', fontSize: 13, lineHeight: 1.8, color: 'var(--text)',
                whiteSpace: 'pre-wrap',
              }}>
                {result.text}
              </div>
            </>
          ) : (
            <div style={{
              flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center',
              color: 'var(--text-muted)',
            }}>
              <div style={{ textAlign: 'center' }}>
                <div style={{ marginBottom: 16, opacity: 0.25, display: 'flex', justifyContent: 'center' }}>
                  <TypeIcon size={64} color="var(--text-dim)" />
                </div>
                <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8, color: 'var(--text)' }}>
                  OCRでテキストを認識
                </h2>
                <p style={{ fontSize: 14, marginBottom: 8 }}>
                  スキャンデータやPDFから文字を認識し、EPUB・テキストに変換
                </p>
                <p style={{ fontSize: 12 }}>
                  ルビの崩れ・画像のズレに配慮した変換を目指します
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
