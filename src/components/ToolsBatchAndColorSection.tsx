import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { DocumentService } from '../services/documentService'
import { safeRevokeObjectUrl } from '../utils/objectUrl'
import { SectionTitle, AccentBtn } from './UIControls'
import { InputDialog } from './AppDialog'
import type { PdfExec } from '../types'

interface ToolsBatchAndColorSectionProps {
  exec: PdfExec
  pdfData: number[] | null
  docId?: string | null
  showToast: (msg: string) => void
}

export function ToolsBatchAndColorSection({
  exec,
  pdfData,
  docId,
  showToast,
}: ToolsBatchAndColorSectionProps) {
  const [batchPaths, setBatchPaths] = useState<string[]>([])
  const [protectDialogOpen, setProtectDialogOpen] = useState(false)

  const getCurrentBytes = (): Promise<number[] | null> =>
    DocumentService.getCurrentBytes(docId, pdfData)

  const handleBatchProtect = async (password: string) => {
    setProtectDialogOpen(false)
    if (!password || batchPaths.length === 0) return
    try {
      const results = await invoke<number[][]>('batch_protect', { paths: batchPaths, password })
      // Results are real encrypted bytes now: persist them instead of dropping.
      const outputDir = await open({ directory: true, multiple: false, title: '暗号化PDFの保存先フォルダを選択' })
      if (typeof outputDir !== 'string' || !outputDir) {
        showToast('保存先が未選択のため書き込みを中止しました（暗号化自体は成功）')
        return
      }
      for (let i = 0; i < results.length; i++) {
        const base = batchPaths[i].split('/').pop() ?? `file${i + 1}`
        const outPath = `${outputDir}/${base.replace(/\.pdf$/i, '')}_protected.pdf`
        await invoke('write_file_bytes', { path: outPath, data: results[i] })
      }
      showToast(`${results.length}ファイルをAES-256暗号化して保存しました`)
    } catch (err) { showToast(`エラー: ${err}`) }
  }

  return (
    <div>
      <SectionTitle>バッチ一括処理</SectionTitle>
      <AccentBtn onClick={async () => {
        const paths = await open({ filters: [{ name: 'PDF', extensions: ['pdf'] }], multiple: true })
        if (paths) setBatchPaths(Array.isArray(paths) ? paths : [paths])
      }}>
        ファイルを選択
      </AccentBtn>
      {batchPaths.length > 0 && (
        <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 4, marginBottom: 8 }}>
          {batchPaths.length}ファイルを選択中
        </div>
      )}
      <AccentBtn onClick={async () => {
        if (batchPaths.length === 0) { showToast('ファイルを選択してください'); return }
        const outputPath = await save({ defaultPath: 'merged.pdf', filters: [{ name: 'PDF', extensions: ['pdf'] }] })
        if (outputPath) {
          await invoke('batch_merge_pdfs', { paths: batchPaths, outputPath: outputPath })
          showToast(`${batchPaths.length}ファイルを結合しました`)
        }
      }} disabled={batchPaths.length < 2}>
        一括結合
      </AccentBtn>
      <AccentBtn onClick={async () => {
        if (batchPaths.length === 0) { showToast('ファイルを選択してください'); return }
        try {
          const results = await invoke<number[][]>('batch_optimize', { paths: batchPaths })
          showToast(`${results.length}ファイルを最適化しました`)
        } catch (err) { showToast(`エラー: ${err}`) }
      }} disabled={batchPaths.length === 0}>
        一括最適化
      </AccentBtn>
      <AccentBtn onClick={() => {
        if (batchPaths.length === 0) { showToast('ファイルを選択してください'); return }
        setProtectDialogOpen(true)
      }} disabled={batchPaths.length === 0}>
        一括暗号化
      </AccentBtn>

      <SectionTitle>PDF/A変換</SectionTitle>
      <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 8, lineHeight: 1.5 }}>
        長期保存用フォーマットに変換します
      </div>
      <AccentBtn onClick={() => exec('convert_to_pdfa', {})} style={{ background: 'var(--green)' }}>
        PDF/A-1bに変換
      </AccentBtn>

      <SectionTitle>XFDF インポート/エクスポート</SectionTitle>
      <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 8, lineHeight: 1.5 }}>
        注釈をXFDF形式で書き出し/取り込み
      </div>
      <AccentBtn onClick={async () => {
        const bytes = await getCurrentBytes()
        if (!bytes) return
        try {
          const xfdf = await invoke<string>('export_xfdf', { data: bytes })
          const blob = new Blob([xfdf], { type: 'application/xfdf' })
          const url = URL.createObjectURL(blob)
          const a = document.createElement('a')
          a.href = url
          a.download = 'annotations.xfdf'
          a.click()
          // ダウンロード開始後に解放（即時revokeはDLを壊すため遅延）
          setTimeout(() => safeRevokeObjectUrl(url), 1000)
          showToast('XFDFをエクスポートしました')
        } catch (err) { showToast(`エラー: ${err}`) }
      }}>
        XFDFエクスポート
      </AccentBtn>
      <AccentBtn onClick={async () => {
        const bytes = await getCurrentBytes()
        if (!bytes) return
        const input = document.createElement('input')
        input.type = 'file'
        input.accept = '.xfdf,.fdf'
        input.onchange = async (e) => {
          const file = (e.target as HTMLInputElement).files?.[0]
          if (file) {
            const text = await file.text()
            try {
              await invoke<number[]>('import_xfdf', { data: bytes, xfdfContent: text })
              showToast('XFDFをインポートしました')
            } catch (err) { showToast(`エラー: ${err}`) }
          }
        }
        input.click()
      }}>
        XFDFインポート
      </AccentBtn>

      <SectionTitle>カラーマネジメント</SectionTitle>
      <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 8, lineHeight: 1.5 }}>
        CMYK変換・ICCプロファイル
      </div>
      <AccentBtn onClick={() => exec('convert_to_cmyk', {})}>
        CMYKに変換
      </AccentBtn>
      <AccentBtn onClick={() => exec('embed_icc_profile', { profileName: 'sRGB IEC61966-2.1' })}>
        ICCプロファイル埋め込み
      </AccentBtn>

      <InputDialog
        isOpen={protectDialogOpen}
        title="一括暗号化"
        message={`選択した ${batchPaths.length} 件のPDFに設定するパスワードを入力してください。`}
        inputType="password"
        confirmLabel="暗号化"
        cancelLabel="キャンセル"
        onSubmit={(v) => { void handleBatchProtect(v) }}
        onCancel={() => setProtectDialogOpen(false)}
      />
    </div>
  )
}
