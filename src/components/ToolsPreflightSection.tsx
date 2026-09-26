import { useState, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { SectionTitle, AccentBtn } from './UIControls'
import { DocumentService } from '../services/documentService'

/** Serialized form of pdf_engine::preflight::PreflightResult. */
export interface PreflightIssue {
  severity: 'error' | 'warning' | string
  category: string
  message: string
}

export interface PreflightReport {
  passed: boolean
  score: number
  issues: PreflightIssue[]
  font_check: {
    total_fonts: number
    embedded_fonts: number
    non_embedded_fonts: string[]
    outlined_fonts: string[]
  }
  color_check: {
    uses_rgb: boolean
    uses_cmyk: boolean
    uses_spot: boolean
    has_icc_profile: boolean
    overprint_enabled: boolean
    max_ink_coverage: number
  }
  image_check: {
    total_images: number
    min_dpi: number
    low_res_images: string[]
    images_without_profile: string[]
  }
}

/** Serialized form of pdf_engine::PdfaValidationReport. */
export interface PdfaReport {
  is_compliant: boolean
  standard: string
  passed_checks: string[]
  violations: string[]
  details: Record<string, unknown>
}

interface ToolsPreflightSectionProps {
  pdfData: number[] | null
  docId?: string | null
  showToast: (msg: string) => void
  onPdfUpdate?: (data: number[]) => void
}

export function ToolsPreflightSection({ pdfData, docId, showToast, onPdfUpdate }: ToolsPreflightSectionProps) {
  const [report, setReport] = useState<PreflightReport | null>(null)
  const [busy, setBusy] = useState(false)

  // PDF/A conformance state
  const [pdfaLevel, setPdfaLevel] = useState<'A' | 'B'>('B')
  const [pdfaReport, setPdfaReport] = useState<PdfaReport | null>(null)
  const [pdfaBusy, setPdfaBusy] = useState(false)

  const getCurrentBytes = useCallback(async (): Promise<number[] | null> => {
    if (docId) return DocumentService.getSessionBytes(docId)
    return pdfData
  }, [docId, pdfData])

  const validatePdfa = async () => {
    setPdfaBusy(true)
    try {
      const bytes = await getCurrentBytes()
      if (!bytes) return
      const result = await invoke<PdfaReport>('validate_pdfa_compliance', {
        data: bytes,
        targetConformance: pdfaLevel,
      })
      setPdfaReport(result)
      showToast(
        result.is_compliant
          ? `PDF/A 適合: ${result.standard}`
          : `PDF/A 不適合: ${result.violations.length} 件の違反`
      )
    } catch (err) {
      showToast(`PDF/A 検証エラー: ${err}`)
    } finally {
      setPdfaBusy(false)
    }
  }

  const convertToPdfa = async () => {
    try {
      const bytes = await getCurrentBytes()
      if (!bytes) return
      const converted = await invoke<number[]>('convert_to_pdfa', { data: bytes })
      await onPdfUpdate?.(converted)
      showToast('PDF/A-1b 長期保存形式へ変換しました')
      // 変換直後に再検証して結果を更新
      const result = await invoke<PdfaReport>('validate_pdfa_compliance', {
        data: converted,
        targetConformance: pdfaLevel,
      })
      setPdfaReport(result)
    } catch (err) {
      showToast(`PDF/A 変換エラー: ${err}`)
    }
  }

  const runPreflight = async () => {
    if (!pdfData) return
    setBusy(true)
    try {
      const result = await invoke<PreflightReport>('run_preflight', { data: pdfData })
      setReport(result)
      showToast(
        result.passed
          ? `プリフライト合格 (スコア ${result.score}/100)`
          : `プリフライトで問題を検出 (スコア ${result.score}/100)`
      )
    } catch (err) {
      showToast(`プリフライトエラー: ${err}`)
    } finally {
      setBusy(false)
    }
  }

  const scoreColor =
    !report ? 'var(--text-muted)'
      : report.score >= 90 ? '#2ea043'
      : report.score >= 70 ? '#e3b341'
      : '#e5484d'

  return (
    <div>
      <SectionTitle>プリフライト検査 (Acrobat Pro相当)</SectionTitle>
      <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 6, lineHeight: 1.5 }}>
        フォント埋め込み・カラースペース・画像解像度・インク被覆率の総合診断
      </div>

      <AccentBtn
        onClick={runPreflight}
        disabled={!pdfData || busy}
        style={{ width: '100%', marginBottom: 10, background: 'var(--accent)' }}
      >
        {busy ? '検査中...' : 'プリフライト実行'}
      </AccentBtn>

      {report && (
        <div style={{
          padding: 12, borderRadius: 8, background: 'var(--bg-1)',
          border: `1px solid ${report.passed ? 'var(--green)' : '#e5484d'}`,
          fontSize: 11, marginBottom: 14,
        }}>
          {/* Score gauge */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 }}>
            <div style={{
              fontSize: 26, fontWeight: 800, color: scoreColor, lineHeight: 1,
              fontVariantNumeric: 'tabular-nums',
            }}>
              {report.score}
            </div>
            <div>
              <div style={{ fontWeight: 700, color: scoreColor }}>
                {report.passed ? '合格 — エラーなし' : '不合格 — エラーあり'}
              </div>
              <div style={{ color: 'var(--text-muted)', fontSize: 10 }}>
                エラー {report.issues.filter(i => i.severity === 'error').length} /
                警告 {report.issues.filter(i => i.severity === 'warning').length}
              </div>
            </div>
          </div>

          {/* Check summary grid */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 6, marginBottom: 10 }}>
            <CheckTile
              label="フォント"
              ok={report.font_check.non_embedded_fonts.length === 0}
              detail={`${report.font_check.embedded_fonts}/${report.font_check.total_fonts} 埋め込み`}
            />
            <CheckTile
              label="カラー"
              ok={report.color_check.max_ink_coverage <= 300}
              detail={`最大インク ${report.color_check.max_ink_coverage.toFixed(0)}%`}
            />
            <CheckTile
              label="画像"
              ok={report.image_check.low_res_images.length === 0}
              detail={report.image_check.total_images > 0
                ? `最低 ${report.image_check.min_dpi.toFixed(0)} DPI`
                : '画像なし'}
            />
          </div>

          {/* Color space detail */}
          <div style={{ color: 'var(--text-muted)', marginBottom: 8 }}>
            カラースペース: {[report.color_check.uses_rgb && 'RGB', report.color_check.uses_cmyk && 'CMYK', report.color_check.uses_spot && 'Spot'].filter(Boolean).join(' + ') || '不明'}
            {' '}{report.color_check.has_icc_profile && '(ICC付与済)'}
          </div>

          {/* Issues list */}
          {report.issues.length > 0 && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4, maxHeight: 200, overflowY: 'auto' }}>
              {report.issues.map((issue, i) => (
                <div key={i} style={{
                  display: 'flex', gap: 6, alignItems: 'flex-start',
                  padding: '4px 8px', borderRadius: 4,
                  background: issue.severity === 'error' ? 'rgba(229,72,77,0.08)' : 'rgba(227,179,65,0.08)',
                  border: `1px solid ${issue.severity === 'error' ? 'rgba(229,72,77,0.3)' : 'rgba(227,179,65,0.3)'}`,
                }}>
                  <span style={{
                    fontSize: 9, fontWeight: 700, flexShrink: 0, padding: '1px 5px', borderRadius: 3,
                    background: issue.severity === 'error' ? '#e5484d' : '#e3b341',
                    color: '#fff', textTransform: 'uppercase',
                  }}>
                    {issue.severity}
                  </span>
                  <span style={{ color: 'var(--text)', lineHeight: 1.4 }}>
                    <b style={{ color: 'var(--text-muted)' }}>{issue.category}:</b> {issue.message}
                  </span>
                </div>
              ))}
            </div>
          )}

          {/* Remediation hints */}
          {report.font_check.non_embedded_fonts.length > 0 && (
            <div style={{ marginTop: 8, padding: '4px 8px', borderRadius: 4, background: 'rgba(47,129,247,0.08)', border: '1px dashed rgba(47,129,247,0.3)', color: 'var(--text-muted)', fontSize: 10 }}>
              修正候補: 「フォントのアウトライン化」で全フォントをベクター化できます
            </div>
          )}
          {report.color_check.max_ink_coverage > 300 && (
            <div style={{ marginTop: 8, padding: '4px 8px', borderRadius: 4, background: 'rgba(47,129,247,0.08)', border: '1px dashed rgba(47,129,247,0.3)', color: 'var(--text-muted)', fontSize: 10 }}>
              修正候補: CMYK変換時のTAC制限でインク被覆率を300%以下に抑制できます
            </div>
          )}
        </div>
      )}

      {/* ===== PDF/A (ISO 19005) conformance ===== */}
      <SectionTitle>PDF/A 長期保存適合性 (ISO 19005)</SectionTitle>
      <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 6, lineHeight: 1.5 }}>
        電子保存の国際標準への適合を検証し、アーカイブ用PDFを生成します
      </div>

      {/* Conformance level selector */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 6, marginBottom: 6 }}>
        {(['A', 'B'] as const).map(level => (
          <button
            key={level}
            onClick={() => setPdfaLevel(level)}
            style={{
              padding: '6px 8px', fontSize: 11, fontWeight: 600, cursor: 'pointer',
              background: pdfaLevel === level ? 'var(--accent)' : 'var(--bg-2)',
              color: pdfaLevel === level ? '#fff' : 'var(--text)',
              border: `1px solid ${pdfaLevel === level ? 'transparent' : 'var(--border)'}`,
              borderRadius: 'var(--radius-sm)',
            }}
          >
            PDF/A-1{level}
          </button>
        ))}
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 6, marginBottom: 12 }}>
        <AccentBtn
          onClick={validatePdfa}
          disabled={pdfaBusy}
          style={{ background: 'var(--bg-2)', color: 'var(--text)', border: '1px solid var(--border)', fontSize: 11 }}
        >
          {pdfaBusy ? '検証中...' : '適合性検証'}
        </AccentBtn>
        <AccentBtn
          onClick={convertToPdfa}
          style={{ background: 'var(--green)', fontSize: 11 }}
        >
          PDF/A-1b へ変換
        </AccentBtn>
      </div>

      {pdfaReport && (
        <div style={{
          padding: 12, borderRadius: 8, background: 'var(--bg-1)',
          border: `1px solid ${pdfaReport.is_compliant ? 'var(--green)' : '#e5484d'}`,
          fontSize: 11, marginBottom: 14,
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
            <span style={{ fontWeight: 700, color: pdfaReport.is_compliant ? '#2ea043' : '#e5484d' }}>
              {pdfaReport.is_compliant ? `適合: ${pdfaReport.standard}` : `不適合: ${pdfaReport.standard}`}
            </span>
            <button
              onClick={() => setPdfaReport(null)}
              style={{ background: 'none', border: 'none', color: 'var(--text-muted)', cursor: 'pointer', fontSize: 12 }}
            >
              [x]
            </button>
          </div>

          {/* Pass/fail counters */}
          <div style={{ color: 'var(--text-muted)', marginBottom: 8 }}>
            合格チェック {pdfaReport.passed_checks.length} /
            違反 {pdfaReport.violations.length}
          </div>

          {/* Violations */}
          {pdfaReport.violations.length > 0 && (
            <div style={{ marginBottom: 8 }}>
              <div style={{ fontSize: 10, fontWeight: 700, color: '#e5484d', marginBottom: 4 }}>違反項目</div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                {pdfaReport.violations.map((v, i) => (
                  <div key={i} style={{
                    padding: '4px 8px', borderRadius: 4, lineHeight: 1.4,
                    background: 'rgba(229,72,77,0.08)',
                    border: '1px solid rgba(229,72,77,0.3)',
                  }}>
                    {v}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Passed checks */}
          {pdfaReport.passed_checks.length > 0 && (
            <div>
              <div style={{ fontSize: 10, fontWeight: 700, color: '#2ea043', marginBottom: 4 }}>合格項目</div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                {pdfaReport.passed_checks.map((c, i) => (
                  <div key={i} style={{
                    padding: '4px 8px', borderRadius: 4, lineHeight: 1.4,
                    background: 'rgba(46,160,67,0.08)',
                    border: '1px solid rgba(46,160,67,0.3)',
                  }}>
                    {c}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Remediation for the most common violation */}
          {pdfaReport.violations.some(v => v.includes('not fully embedded')) && (
            <div style={{ marginTop: 8, padding: '4px 8px', borderRadius: 4, background: 'rgba(47,129,247,0.08)', border: '1px dashed rgba(47,129,247,0.3)', color: 'var(--text-muted)', fontSize: 10 }}>
              修正候補: フォントを埋め込む（または「フォントのアウトライン化」でベクター化）が必要です
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function CheckTile({ label, ok, detail }: { label: string; ok: boolean; detail: string }) {
  return (
    <div style={{
      padding: '6px 8px', borderRadius: 6, textAlign: 'center',
      background: ok ? 'rgba(46,160,67,0.08)' : 'rgba(229,72,77,0.08)',
      border: `1px solid ${ok ? 'rgba(46,160,67,0.35)' : 'rgba(229,72,77,0.35)'}`,
    }}>
      <div style={{ fontSize: 9, color: 'var(--text-muted)', marginBottom: 2 }}>{label}</div>
      <div style={{ fontSize: 13, fontWeight: 700, color: ok ? '#2ea043' : '#e5484d' }}>
        {ok ? 'OK' : 'NG'}
      </div>
      <div style={{ fontSize: 9, color: 'var(--text-muted)', marginTop: 2 }}>{detail}</div>
    </div>
  )
}
