import { PDFDocument, rgb, StandardFonts, degrees } from 'pdf-lib'
import fs from 'fs'
import path from 'path'

async function createWeirdPDF() {
  const pdfDoc = await PDFDocument.create()
  const helvetica = await pdfDoc.embedFont(StandardFonts.Helvetica)
  const helveticaBold = await pdfDoc.embedFont(StandardFonts.HelveticaBold)
  const courier = await pdfDoc.embedFont(StandardFonts.CourierBold)
  const times = await pdfDoc.embedFont(StandardFonts.TimesRomanBold)

  // ================= PAGE 1: Wild Top Secret Spec Sheet =================
  const page1 = pdfDoc.addPage([600, 840])
  const { width: p1W, height: p1H } = page1.getSize()

  // Background subtle tint
  page1.drawRectangle({
    x: 0,
    y: 0,
    width: p1W,
    height: p1H,
    color: rgb(0.98, 0.98, 0.95),
  })

  // Warning Header Strip
  page1.drawRectangle({
    x: 0,
    y: p1H - 45,
    width: p1W,
    height: 45,
    color: rgb(0.85, 0.15, 0.15),
  })
  page1.drawText('*** TOP SECRET // OPERATION PENGUIN 2026 ***', {
    x: 70,
    y: p1H - 28,
    size: 15,
    font: courier,
    color: rgb(1, 1, 1),
  })

  // Giant Watermark in Background
  page1.drawText('CLASSIFIED // DO NOT LEAK', {
    x: 60,
    y: 260,
    size: 38,
    font: helveticaBold,
    color: rgb(0.92, 0.90, 0.88),
    rotate: degrees(35),
  })

  // Big Title
  page1.drawText('PROJECT NEOCORTEX: WEIRD SPEC V9', {
    x: 40,
    y: p1H - 90,
    size: 20,
    font: helveticaBold,
    color: rgb(0.1, 0.15, 0.25),
  })

  page1.drawText('Document ID: PX-88910-OMEGA | Security Clearance Level: ULTRA', {
    x: 40,
    y: p1H - 110,
    size: 10,
    font: courier,
    color: rgb(0.4, 0.45, 0.5),
  })

  // Decorative Shapes & Neon Badges
  page1.drawRectangle({
    x: 40,
    y: p1H - 230,
    width: 250,
    height: 95,
    color: rgb(0.90, 0.95, 1.0),
    borderColor: rgb(0.15, 0.45, 0.9),
    borderWidth: 2,
  })
  page1.drawText('Target Subject:', { x: 50, y: p1H - 150, size: 11, font: helveticaBold, color: rgb(0.15, 0.45, 0.9) })
  page1.drawText('Autonomous Quantum Space Penguin', { x: 50, y: p1H - 170, size: 12, font: helvetica, color: rgb(0.1, 0.1, 0.1) })
  page1.drawText('Max Velocity: 0.94c (Warp Factor 4)', { x: 50, y: p1H - 190, size: 10, font: courier, color: rgb(0.2, 0.2, 0.2) })
  page1.drawText('Primary Weapon: Sardine Laser Array', { x: 50, y: p1H - 210, size: 10, font: courier, color: rgb(0.2, 0.2, 0.2) })

  // Strange Table
  const tableTop = p1H - 260
  page1.drawRectangle({
    x: 40,
    y: tableTop - 130,
    width: 520,
    height: 130,
    color: rgb(1, 1, 1),
    borderColor: rgb(0.3, 0.35, 0.4),
    borderWidth: 1.5,
  })

  // Table header
  page1.drawRectangle({
    x: 40,
    y: tableTop - 28,
    width: 520,
    height: 28,
    color: rgb(0.15, 0.20, 0.30),
  })
  page1.drawText('SYSTEM MODULE', { x: 50, y: tableTop - 20, size: 10, font: helveticaBold, color: rgb(1, 1, 1) })
  page1.drawText('STATUS', { x: 220, y: tableTop - 20, size: 10, font: helveticaBold, color: rgb(1, 1, 1) })
  page1.drawText('ANOMALY RATING', { x: 340, y: tableTop - 20, size: 10, font: helveticaBold, color: rgb(1, 1, 1) })
  page1.drawText('PASS CODE', { x: 460, y: tableTop - 20, size: 10, font: helveticaBold, color: rgb(1, 1, 1) })

  // Rows
  const rows = [
    ['Antigravity Drive #1', 'ONLINE (99.8%)', 'CRITICAL HIGH', 'KEY-9021'],
    ['Sub-space Radar Matrix', 'CALIBRATING', 'MODERATE', 'RAD-4402'],
    ['Cryo Sardine Freezer', 'OPTIMAL (-80C)', 'SAFE ZERO', 'ICE-0077'],
    ['Target Text To Replace', 'ACTIVE UNSTABLE', 'EXTREME DANGER', 'PENGUIN-404'],
  ]

  rows.forEach((r, idx) => {
    const y = tableTop - 50 - (idx * 24)
    page1.drawText(r[0], { x: 50, y, size: 9.5, font: helvetica, color: rgb(0.1, 0.1, 0.1) })
    page1.drawText(r[1], { x: 220, y, size: 9, font: courier, color: rgb(0.1, 0.5, 0.2) })
    page1.drawText(r[2], { x: 340, y, size: 9, font: helveticaBold, color: idx === 3 ? rgb(0.9, 0.1, 0.1) : rgb(0.2, 0.2, 0.2) })
    page1.drawText(r[3], { x: 460, y, size: 9.5, font: courier, color: rgb(0.1, 0.2, 0.8) })
  })

  // Simulated Barcode
  for (let i = 0; i < 40; i++) {
    const barW = (i % 3 === 0 ? 3 : (i % 2 === 0 ? 1.5 : 2.5))
    page1.drawRectangle({
      x: 340 + (i * 5.2),
      y: p1H - 180,
      width: barW,
      height: 36,
      color: rgb(0.1, 0.1, 0.1),
    })
  }
  page1.drawText('BARCODE-SEC-991284-XYZ', { x: 350, y: p1H - 195, size: 8, font: courier, color: rgb(0.3, 0.3, 0.3) })

  // Giant Rotated Red Stamp
  page1.drawRectangle({
    x: 320,
    y: 110,
    width: 220,
    height: 60,
    borderColor: rgb(0.85, 0.1, 0.1),
    borderWidth: 4,
    rotate: degrees(-15),
  })
  page1.drawText('APPROVED BY NAGISA', {
    x: 335,
    y: 132,
    size: 14,
    font: helveticaBold,
    color: rgb(0.85, 0.1, 0.1),
    rotate: degrees(-15),
  })

  // ================= PAGE 2: Blueprint & Geometry Matrix =================
  const page2 = pdfDoc.addPage([600, 840])
  const { width: p2W, height: p2H } = page2.getSize()

  // Dark Blueprint Grid Background
  page2.drawRectangle({
    x: 0,
    y: 0,
    width: p2W,
    height: p2H,
    color: rgb(0.08, 0.18, 0.32),
  })

  // Grid Lines
  for (let x = 0; x < p2W; x += 30) {
    page2.drawLine({
      start: { x, y: 0 },
      end: { x, y: p2H },
      color: rgb(0.12, 0.24, 0.40),
      thickness: 0.75,
    })
  }
  for (let y = 0; y < p2H; y += 30) {
    page2.drawLine({
      start: { x: 0, y },
      end: { x: p2W, y },
      color: rgb(0.12, 0.24, 0.40),
      thickness: 0.75,
    })
  }

  // Blueprint Title
  page2.drawText('CYBERNETIC ROBOTIC COMPONENT SCHEMATICS', {
    x: 40,
    y: p2H - 50,
    size: 15,
    font: courier,
    color: rgb(0.4, 0.85, 1.0),
  })
  page2.drawText('REV: 4.12-B // DIMENSIONS IN PARSECS', {
    x: 40,
    y: p2H - 70,
    size: 10,
    font: courier,
    color: rgb(0.3, 0.6, 0.8),
  })

  // Blueprint Central Circle & Target Crosshair
  const cx = p2W / 2
  const cy = p2H / 2 + 30
  page2.drawCircle({
    x: cx,
    y: cy,
    size: 120,
    borderColor: rgb(0.2, 0.7, 0.95),
    borderWidth: 2,
  })
  page2.drawCircle({
    x: cx,
    y: cy,
    size: 70,
    borderColor: rgb(0.4, 0.85, 1.0),
    borderWidth: 1.5,
  })
  page2.drawLine({
    start: { x: cx - 150, y: cy },
    end: { x: cx + 150, y: cy },
    color: rgb(0.3, 0.75, 1.0),
    thickness: 1,
  })
  page2.drawLine({
    start: { x: cx, y: cy - 150 },
    end: { x: cx, y: cy + 150 },
    color: rgb(0.3, 0.75, 1.0),
    thickness: 1,
  })

  page2.drawText('CORE FUSION CELL [98.2%]', {
    x: cx - 60,
    y: cy - 4,
    size: 9,
    font: courier,
    color: rgb(1, 1, 1),
  })

  // Blueprint Spec Card
  page2.drawRectangle({
    x: 40,
    y: 80,
    width: 520,
    height: 140,
    color: rgb(0.05, 0.12, 0.22),
    borderColor: rgb(0.3, 0.6, 0.9),
    borderWidth: 1.5,
  })
  page2.drawText('DIAGNOSTIC LOG & CODE BLOCK:', {
    x: 55,
    y: 195,
    size: 11,
    font: courier,
    color: rgb(0.3, 0.9, 0.6),
  })
  page2.drawText('> SYSTEM STATUS: NOMINAL. READY FOR EDITING & ANNOTATIONS.', {
    x: 55,
    y: 175,
    size: 9.5,
    font: courier,
    color: rgb(0.8, 0.9, 1.0),
  })
  page2.drawText('> REPLACEABLE_TARGET_KEY = "SECRET_PENGUIN_ACCESS_99"', {
    x: 55,
    y: 155,
    size: 9.5,
    font: courier,
    color: rgb(1.0, 0.8, 0.3),
  })
  page2.drawText('> NAGISA HIGH ACCURACY VECTOR RENDERER: VERIFIED', {
    x: 55,
    y: 135,
    size: 9.5,
    font: courier,
    color: rgb(0.4, 0.8, 1.0),
  })
  page2.drawText('> ALL EDITS LOCAL TO BROWSER / TAURI ENGINE', {
    x: 55,
    y: 115,
    size: 9.5,
    font: courier,
    color: rgb(0.7, 0.7, 0.8),
  })

  // Save to public and current dir
  const pdfBytes = await pdfDoc.save()
  const outPublic = path.resolve('public/weird_experimental_doc.pdf')
  const outDist = path.resolve('dist/weird_experimental_doc.pdf')
  const outRoot = path.resolve('weird_experimental_doc.pdf')

  fs.writeFileSync(outPublic, pdfBytes)
  if (fs.existsSync('dist')) {
    fs.writeFileSync(outDist, pdfBytes)
  }
  fs.writeFileSync(outRoot, pdfBytes)
  console.log(`Generated weird PDF successfully (${pdfBytes.length} bytes)!`)
}

createWeirdPDF().catch(err => {
  console.error(err)
  process.exit(1)
})
