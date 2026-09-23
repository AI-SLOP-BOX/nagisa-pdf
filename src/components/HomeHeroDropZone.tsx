import React from 'react'
import { DocumentPlusIcon } from './Icons'

interface HomeHeroDropZoneProps {
  isDragging: boolean
  setIsDragging: (isDragging: boolean) => void
  onDrop: (e: React.DragEvent) => void
  onSelectFile: () => void
}

export const HomeHeroDropZone: React.FC<HomeHeroDropZoneProps> = ({
  isDragging,
  setIsDragging,
  onDrop,
  onSelectFile,
}) => {
  return (
    <>
      {/* 1. HERO BANNER WITH BEAUTIFUL SMOOTH WAVES (渚) */}
      <section
        style={{
          position: 'relative',
          width: '100%',
          height: 134,
          borderRadius: 16,
          overflow: 'hidden',
          background: 'linear-gradient(135deg, #ffffff 0%, #f0f9ff 35%, #e0f2fe 100%)',
          border: '1px solid rgba(226, 232, 240, 0.9)',
          boxShadow: '0 4px 20px rgba(148, 163, 184, 0.08)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '18px 30px',
          boxSizing: 'border-box',
          flexShrink: 0,
        }}
      >
        {/* Layered Smooth Waves Graphics */}
        <div style={{ position: 'absolute', top: 0, right: 0, bottom: 0, width: '65%', pointerEvents: 'none' }}>
          <svg
            viewBox="0 0 700 200"
            preserveAspectRatio="none"
            style={{ width: '100%', height: '100%', display: 'block' }}
          >
            <defs>
              <linearGradient id="heroWaveGradient1" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stopColor="#bae6fd" stopOpacity="0.45" />
                <stop offset="60%" stopColor="#60a5fa" stopOpacity="0.65" />
                <stop offset="100%" stopColor="#2563eb" stopOpacity="0.85" />
              </linearGradient>
              <linearGradient id="heroWaveGradient2" x1="0%" y1="0%" x2="100%" y2="80%">
                <stop offset="0%" stopColor="#e0f2fe" stopOpacity="0.7" />
                <stop offset="50%" stopColor="#38bdf8" stopOpacity="0.8" />
                <stop offset="100%" stopColor="#1d4ed8" stopOpacity="0.95" />
              </linearGradient>
              <linearGradient id="heroWaveGradient3" x1="10%" y1="20%" x2="90%" y2="100%">
                <stop offset="0%" stopColor="#ffffff" stopOpacity="0.95" />
                <stop offset="100%" stopColor="#93c5fd" stopOpacity="0.5" />
              </linearGradient>
            </defs>
            <path d="M 0,160 C 220,170 380,80 700,50 L 700,200 L 0,200 Z" fill="url(#heroWaveGradient1)" />
            <path d="M 60,195 C 240,160 410,110 560,95 C 620,90 660,94 700,105 L 700,200 L 60,200 Z" fill="url(#heroWaveGradient2)" />
            <path d="M 120,200 C 280,175 430,128 580,108 C 630,102 670,107 700,118 L 700,125 C 665,116 625,112 575,118 C 420,138 275,188 120,200 Z" fill="url(#heroWaveGradient3)" />
          </svg>
        </div>

        {/* Left Text Content */}
        <div style={{ position: 'relative', zIndex: 2, display: 'flex', flexDirection: 'column' }}>
          <span style={{ fontSize: 11, color: '#64748b', fontWeight: 500, letterSpacing: '0.04em', marginBottom: 4 }}>
            書類が、もっと自由になる。
          </span>
          <h1 style={{ fontSize: 24, fontWeight: 800, color: '#0f172a', letterSpacing: '-0.03em', margin: '0 0 6px 0', lineHeight: 1.2 }}>
            PDFを、もっとなめらかに。
          </h1>
          <p style={{ fontSize: 12, color: '#475569', lineHeight: 1.45, margin: 0, fontWeight: 450 }}>
            nagisaは、日々のPDF作業をシンプルに、<br />
            やさしく、美しくするアプリです。
          </p>
        </div>

        {/* Right Text Content */}
        <div style={{ position: 'relative', zIndex: 2, display: 'flex', flexDirection: 'column', alignItems: 'flex-end', textAlign: 'right' }}>
          <span style={{ fontSize: 12, color: '#475569', fontWeight: 500, lineHeight: 1.4 }}>
            流れるように、<br />
            はかどる毎日を。
          </span>
          <span style={{ fontSize: 9, color: '#94a3b8', fontWeight: 700, letterSpacing: '0.12em', marginTop: 6 }}>
            A SMOOTHER<br />
            WAY WITH PDFS.
          </span>
        </div>
      </section>

      {/* 2. DRAG & DROP ZONE CARD */}
      <section
        onDragOver={e => { e.preventDefault(); e.stopPropagation(); setIsDragging(true); }}
        onDragLeave={e => { e.preventDefault(); e.stopPropagation(); setIsDragging(false); }}
        onDrop={onDrop}
        onClick={onSelectFile}
        className={`nagisa-dropzone ${isDragging ? 'dragging' : ''}`}
        style={{
          width: '100%',
          height: 118,
          borderRadius: 14,
          background: isDragging ? '#eff6ff' : '#ffffff',
          border: `1.5px dashed ${isDragging ? '#2563eb' : '#cbd5e1'}`,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          cursor: 'pointer',
          padding: 12,
          boxSizing: 'border-box',
          boxShadow: '0 2px 8px rgba(0, 0, 0, 0.02)',
          transition: 'all 0.2s cubic-bezier(0.16, 1, 0.3, 1)',
          flexShrink: 0,
        }}
      >
        <div style={{ marginBottom: 4, color: '#94a3b8' }}>
          <DocumentPlusIcon size={30} color="#94a3b8" />
        </div>
        <div style={{ fontSize: 13.5, fontWeight: 700, color: '#1e293b', marginBottom: 2 }}>
          PDFファイルをドラッグ＆ドロップ
        </div>
        <div style={{ fontSize: 11.5, color: '#64748b', marginBottom: 8 }}>
          または、ファイルを選択して開いてください。
        </div>
        <button
          onClick={e => { e.stopPropagation(); onSelectFile(); }}
          style={{
            background: '#2563eb',
            color: '#ffffff',
            border: 'none',
            borderRadius: 7,
            padding: '6px 20px',
            fontSize: 12,
            fontWeight: 600,
            cursor: 'pointer',
            boxShadow: '0 2px 8px rgba(37, 99, 235, 0.3)',
            transition: 'all 0.15s ease',
          }}
          onMouseEnter={e => (e.currentTarget.style.background = '#1d4ed8')}
          onMouseLeave={e => (e.currentTarget.style.background = '#2563eb')}
        >
          ファイルを選択
        </button>
      </section>
    </>
  )
}
