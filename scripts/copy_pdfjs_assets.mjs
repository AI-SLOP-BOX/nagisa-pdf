// Copy pdf.js cMaps and standard fonts from node_modules into public/
// so the app works fully offline (no jsdelivr CDN dependency).
import { cp, mkdir, rm } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const assets = [
  { from: 'node_modules/pdfjs-dist/cmaps', to: 'public/cmaps' },
  { from: 'node_modules/pdfjs-dist/standard_fonts', to: 'public/standard_fonts' },
]

for (const { from, to } of assets) {
  const src = join(root, from)
  const dst = join(root, to)
  if (!existsSync(src)) {
    console.warn(`[copy_pdfjs_assets] skip (not found): ${from}`)
    continue
  }
  await rm(dst, { recursive: true, force: true })
  await mkdir(dirname(dst), { recursive: true })
  await cp(src, dst, { recursive: true })
  console.log(`[copy_pdfjs_assets] ${from} -> ${to}`)
}
