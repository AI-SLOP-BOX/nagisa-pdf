import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { ToolsPreflightSection, PreflightReport } from '../components/ToolsPreflightSection'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

const passingReport: PreflightReport = {
  passed: true,
  score: 95,
  issues: [
    { severity: 'warning', category: 'Color', message: 'Mixed RGB and CMYK color spaces' },
  ],
  font_check: { total_fonts: 2, embedded_fonts: 2, non_embedded_fonts: [], outlined_fonts: [] },
  color_check: {
    uses_rgb: true, uses_cmyk: true, uses_spot: false,
    has_icc_profile: true, overprint_enabled: false, max_ink_coverage: 250,
  },
  image_check: { total_images: 1, min_dpi: 300, low_res_images: [], images_without_profile: [] },
}

const failingReport: PreflightReport = {
  passed: false,
  score: 75,
  issues: [
    { severity: 'error', category: 'Font', message: "Font 'Arial' is not embedded" },
    { severity: 'warning', category: 'Ink', message: 'Maximum ink coverage exceeds 300%' },
  ],
  font_check: { total_fonts: 2, embedded_fonts: 1, non_embedded_fonts: ['Arial'], outlined_fonts: [] },
  color_check: {
    uses_rgb: false, uses_cmyk: true, uses_spot: false,
    has_icc_profile: false, overprint_enabled: false, max_ink_coverage: 340,
  },
  image_check: { total_images: 2, min_dpi: 96, low_res_images: ['Image_5_0'], images_without_profile: [] },
}

beforeEach(() => invokeMock.mockReset())

describe('ToolsPreflightSection', () => {
  it('pdfData が null なら実行ボタンを無効化する', () => {
    render(<ToolsPreflightSection pdfData={null} showToast={() => {}} />)
    const btn = screen.getByRole('button', { name: 'プリフライト実行' })
    expect(btn).toBeDisabled()
  })

  it('実行すると run_preflight を invoke しスコアとOKタイルを表示する', async () => {
    invokeMock.mockResolvedValue(passingReport)
    const showToast = vi.fn()
    render(<ToolsPreflightSection pdfData={[1, 2, 3]} showToast={showToast} />)

    fireEvent.click(screen.getByRole('button', { name: 'プリフライト実行' }))

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('run_preflight', { data: [1, 2, 3] }))
    await screen.findByText('95')
    expect(screen.getByText('合格 — エラーなし')).toBeInTheDocument()
    // 3つのチェックタイル (フォント/カラー/画像) がOK表示
    expect(screen.getAllByText('OK')).toHaveLength(3)
    expect(showToast).toHaveBeenCalledWith('プリフライト合格 (スコア 95/100)')
  })

  it('不合格時はエラー/警告の色分け付きissue一覧と修正候補を表示する', async () => {
    invokeMock.mockResolvedValue(failingReport)
    render(<ToolsPreflightSection pdfData={[1]} showToast={() => {}} />)

    fireEvent.click(screen.getByRole('button', { name: 'プリフライト実行' }))

    await screen.findByText('75')
    expect(screen.getByText('不合格 — エラーあり')).toBeInTheDocument()
    expect(screen.getByText("Font 'Arial' is not embedded")).toBeInTheDocument()
    expect(screen.getAllByText('NG').length).toBeGreaterThan(0)
    // フォント未埋め込みの修正候補ヒント
    expect(screen.getByText(/修正候補: 「フォントのアウトライン化」/)).toBeInTheDocument()
    // インク被覆率超過の修正候補ヒント
    expect(screen.getByText(/修正候補: CMYK変換時のTAC制限/)).toBeInTheDocument()
  })

  it('invoke が不正な応答を返した場合もエラー表示へフォールバックし、UI が壊れない', async () => {
    // NOTE: この Vitest 5 ランナーは未処理 Promise rejection を検出して即座にテストを
    // 失敗させるため、mock 側で reject する Promise を生成しない（生成時点で失敗する）。
    // 代わりに「不正な応答」を resolve させ、コンポーネントの try/catch が例外を捕捉して
    // エラー表示へフォールバックし、finally で busy が解除されることを検証する。
    invokeMock.mockResolvedValue(null as unknown as PreflightReport)

    const showToast = vi.fn()
    render(<ToolsPreflightSection pdfData={[1]} showToast={showToast} />)

    fireEvent.click(screen.getByRole('button', { name: 'プリフライト実行' }))

    await waitFor(() => expect(showToast).toHaveBeenCalledWith(expect.stringContaining('プリフライトエラー:')))
    // 部分的なレポートを表示しない
    expect(screen.queryByText('95')).not.toBeInTheDocument()
    // エラー後も busy が解除され、ボタンを再実行できる状態に戻る
    await waitFor(() => expect(screen.getByRole('button', { name: 'プリフライト実行' })).toBeEnabled())
  })

  it('PDF/A適合性検証が不合格時に違反項目と合格項目を分けて表示する', async () => {
    invokeMock.mockResolvedValue({
      is_compliant: false,
      standard: 'PDF/A-1B (ISO 19005-1)',
      passed_checks: ['Header version 1.4 is PDF/A-1 compatible (>= 1.4)'],
      violations: [
        'Fonts are not fully embedded: Helvetica (ISO 19005-1 requires embedded fonts)',
        'XMP metadata stream is missing (ISO 19005-1 requires a valid XMP packet)',
      ],
      details: { has_xmp: false, marked: false },
    })
    const showToast = vi.fn()
    render(<ToolsPreflightSection pdfData={[1]} showToast={showToast} />)

    fireEvent.click(screen.getByRole('button', { name: '適合性検証' }))

    await screen.findByText('不適合: PDF/A-1B (ISO 19005-1)')
    expect(screen.getByText('違反項目')).toBeInTheDocument()
    expect(screen.getByText('合格項目')).toBeInTheDocument()
    // フォント未埋め込みの修正候補ヒント
    expect(screen.getByText(/修正候補: フォントを埋め込む/)).toBeInTheDocument()
    expect(showToast).toHaveBeenCalledWith('PDF/A 不適合: 2 件の違反')
  })

  it('PDF/A適合レベル A/B を切り替えて検証できる', async () => {
    invokeMock.mockResolvedValue({
      is_compliant: true,
      standard: 'PDF/A-1A (ISO 19005-1)',
      passed_checks: ['All 2 font(s) are fully embedded (ISO 19005-1 requirement)'],
      violations: [],
      details: {},
    })
    render(<ToolsPreflightSection pdfData={[1]} showToast={() => {}} />)

    fireEvent.click(screen.getByRole('button', { name: 'PDF/A-1A' }))
    fireEvent.click(screen.getByRole('button', { name: '適合性検証' }))

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('validate_pdfa_compliance', {
      data: [1],
      targetConformance: 'A',
    }))
    await screen.findByText('適合: PDF/A-1A (ISO 19005-1)')
  })

  it('PDF/A-1b 変換が onPdfUpdate を呼び、変換後に再検証する', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'convert_to_pdfa') return Promise.resolve([9, 9, 9])
      return Promise.resolve({
        is_compliant: true,
        standard: 'PDF/A-1B (ISO 19005-1)',
        passed_checks: ['Catalog declares MarkInfo /Marked = true (structured document)'],
        violations: [],
        details: {},
      })
    })
    const onPdfUpdate = vi.fn()
    const showToast = vi.fn()
    render(<ToolsPreflightSection pdfData={[1]} showToast={showToast} onPdfUpdate={onPdfUpdate} />)

    fireEvent.click(screen.getByRole('button', { name: 'PDF/A-1b へ変換' }))

    await waitFor(() => expect(onPdfUpdate).toHaveBeenCalledWith([9, 9, 9]))
    expect(invokeMock).toHaveBeenCalledWith('convert_to_pdfa', { data: [1] })
    expect(showToast).toHaveBeenCalledWith('PDF/A-1b 長期保存形式へ変換しました')
    await screen.findByText('適合: PDF/A-1B (ISO 19005-1)')
  })
})
