import React, { useEffect, useState } from 'react'

interface DocumentOpeningOverlayProps {
  isOpen: boolean
  fileName: string
  fileSize?: string
  onComplete?: () => void
}

export const DocumentOpeningOverlay: React.FC<DocumentOpeningOverlayProps> = ({
  isOpen,
  fileName,
  fileSize,
  onComplete,
}) => {
  const [stageText, setStageText] = useState('渚 (Nagisa) エンジンを起動中...')
  const [progress, setProgress] = useState(10)
  const [isLeaving, setIsLeaving] = useState(false)
  const [visible, setVisible] = useState(isOpen)

  useEffect(() => {
    if (!isOpen) {
      setIsLeaving(false)
      setVisible(false)
      return
    }

    setVisible(true)
    setIsLeaving(false)
    setProgress(15)
    setStageText('渚 (Nagisa) エンジンを起動中...')

    const t1 = setTimeout(() => {
      setProgress(55)
      setStageText('PDFストリームとフォントを解析中...')
    }, 200)

    const t2 = setTimeout(() => {
      setProgress(90)
      setStageText('高精度ベクトルキャンバスを展開中...')
    }, 450)

    const t3 = setTimeout(() => {
      setProgress(100)
      setStageText('準備完了')
    }, 680)

    const t4 = setTimeout(() => {
      setIsLeaving(true)
    }, 780)

    const t5 = setTimeout(() => {
      setVisible(false)
      if (onComplete) onComplete()
    }, 1050)

    return () => {
      clearTimeout(t1)
      clearTimeout(t2)
      clearTimeout(t3)
      clearTimeout(t4)
      clearTimeout(t5)
    }
  }, [isOpen, onComplete])

  if (!visible) return null

  // Truncate fileName gracefully if too long
  const displayFileName = fileName.length > 36 ? fileName.slice(0, 33) + '...' : fileName

  return (
    <div className={`nagisa-opening-overlay ${isLeaving ? 'leaving' : 'entering'}`}>
      {/* Background Ambient Glow */}
      <div
        style={{
          position: 'absolute',
          width: 500,
          height: 500,
          borderRadius: '50%',
          background: 'radial-gradient(circle, rgba(37, 99, 235, 0.22) 0%, rgba(56, 189, 248, 0.08) 50%, transparent 70%)',
          pointerEvents: 'none',
        }}
      />

      {/* Ripple Rings (渚 - Water's Edge Waves) */}
      <div style={{ position: 'relative', width: 140, height: 140, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <div
          style={{
            position: 'absolute',
            width: 140,
            height: 140,
            borderRadius: '50%',
            border: '2px solid rgba(56, 189, 248, 0.55)',
            boxShadow: '0 0 24px rgba(37, 99, 235, 0.45)',
            animation: 'nagisaRipple1 2.2s cubic-bezier(0.16, 1, 0.3, 1) infinite',
            pointerEvents: 'none',
          }}
        />
        <div
          style={{
            position: 'absolute',
            width: 140,
            height: 140,
            borderRadius: '50%',
            border: '1.5px solid rgba(96, 165, 250, 0.45)',
            animation: 'nagisaRipple2 2.2s cubic-bezier(0.16, 1, 0.3, 1) infinite 0.35s',
            pointerEvents: 'none',
          }}
        />
        <div
          style={{
            position: 'absolute',
            width: 140,
            height: 140,
            borderRadius: '50%',
            border: '1px solid rgba(147, 197, 253, 0.3)',
            animation: 'nagisaRipple3 2.2s cubic-bezier(0.16, 1, 0.3, 1) infinite 0.7s',
            pointerEvents: 'none',
          }}
        />

        {/* Central 3D Nagisa Logo */}
        <div
          style={{
            position: 'relative',
            zIndex: 10,
            animation: 'nagisaLogoPulse 3s ease-in-out infinite',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <img
            src="/logo.webp"
            alt="Nagisa Logo"
            style={{
              width: 76,
              height: 76,
              objectFit: 'contain',
              userSelect: 'none',
            }}
            draggable={false}
          />
        </div>
      </div>

      {/* Floating Document Pill Card */}
      <div
        style={{
          marginTop: 28,
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          background: 'rgba(255, 255, 255, 0.92)',
          border: '1px solid rgba(203, 213, 225, 0.8)',
          borderRadius: 30,
          padding: '7px 18px',
          backdropFilter: 'blur(16px)',
          boxShadow: '0 8px 30px rgba(37, 99, 235, 0.12)',
          maxWidth: '85vw',
        }}
      >
        <div
          style={{
            width: 22,
            height: 24,
            background: 'linear-gradient(135deg, #ef4444, #dc2626)',
            borderRadius: 4,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            boxShadow: '0 2px 8px rgba(239, 68, 68, 0.35)',
            flexShrink: 0,
          }}
        >
          <span style={{ fontSize: 9, fontWeight: 900, color: '#fff', letterSpacing: -0.5 }}>PDF</span>
        </div>
        <span
          style={{
            fontSize: 13.5,
            fontWeight: 600,
            color: '#0f172a',
            letterSpacing: -0.2,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {displayFileName || 'ドキュメント'}
        </span>
        {fileSize && (
          <span style={{ fontSize: 11, color: '#64748b', marginLeft: 4 }}>
            ({fileSize})
          </span>
        )}
      </div>

      {/* Status Progress Bar */}
      <div style={{ marginTop: 22, width: 280, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 10 }}>
        <div
          style={{
            width: '100%',
            height: 4,
            background: 'rgba(203, 213, 225, 0.6)',
            borderRadius: 9999,
            overflow: 'hidden',
            position: 'relative',
          }}
        >
          <div
            style={{
              width: `${progress}%`,
              height: '100%',
              background: 'linear-gradient(90deg, #38bdf8 0%, #2563eb 50%, #60a5fa 100%)',
              borderRadius: 9999,
              transition: 'width 0.28s cubic-bezier(0.16, 1, 0.3, 1)',
              boxShadow: '0 0 14px rgba(56, 189, 248, 0.6)',
            }}
          />
        </div>

        {/* Dynamic Status Text */}
        <div
          style={{
            fontSize: 12,
            fontWeight: 500,
            color: '#64748b',
            letterSpacing: 0.2,
            display: 'flex',
            alignItems: 'center',
            gap: 6,
          }}
        >
          <span>{stageText}</span>
        </div>
      </div>
    </div>
  )
}
