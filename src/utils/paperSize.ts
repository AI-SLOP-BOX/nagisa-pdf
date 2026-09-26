/**
 * Paper-size helpers shared by the document metadata inspector.
 * Dimensions are PDF points (72 DPI).
 */

/** Recognize common ISO/JIS/US paper sizes from page dimensions in PDF points (72 DPI). */
export function standardPaperName(w: number, h: number): string | null {
  const sizes: Array<[string, number, number]> = [
    ['A3', 841.89, 1190.55],
    ['A4', 595.28, 841.89],
    ['A5', 419.53, 595.28],
    ['B4', 708.66, 1000.63],
    ['B5', 498.9, 708.66],
    ['Letter', 612, 792],
    ['Legal', 612, 1008],
    ['Ledger', 1224, 792],
    ['Tabloid', 792, 1224],
  ]
  for (const [name, sw, sh] of sizes) {
    const direct = Math.abs(w - sw) < 3 && Math.abs(h - sh) < 3
    const rotated = Math.abs(w - sh) < 3 && Math.abs(h - sw) < 3
    if (direct || rotated) return name
  }
  return null
}

/** Convert page dimensions (PDF points) to a human-readable paper size label. */
export function formatPaperSize(dims: { width: number; height: number } | null | undefined): string {
  if (!dims || !dims.width || !dims.height) return '—'
  const mmW = (dims.width * 25.4) / 72
  const mmH = (dims.height * 25.4) / 72
  const label = `${Math.round(mmW)} x ${Math.round(mmH)} mm`
  const name = standardPaperName(dims.width, dims.height)
  return name ? `${name} (${label})` : label
}