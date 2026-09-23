import React from 'react'
import type { RecentPDFFile, View } from '../types'
import {
  ArrowRightIcon,
  MoreHorizontalIcon,
  LightbulbIcon,
  GridIcon,
  ChevronRightIcon,
} from './Icons'

interface HomeRecentSectionProps {
  recentFiles: RecentPDFFile[]
  onOpenSelectFile: () => void
  onOpenRecentFile: (file: RecentPDFFile) => void
  onRemoveRecentFile: (id: string) => void
  onOpenGuide: () => void
  onOpenShortcuts: () => void
  onNavigateView: (view: View) => void
}

export const HomeRecentSection: React.FC<HomeRecentSectionProps> = ({
  recentFiles,
  onOpenSelectFile,
  onOpenRecentFile,
  onRemoveRecentFile,
  onOpenGuide,
  onOpenShortcuts,
  onNavigateView,
}) => {
  return (
    <section
      style={{
        display: 'grid',
        gridTemplateColumns: '1.7fr 1fr',
        gap: 16,
        width: '100%',
        flex: 1,
        minHeight: 220,
      }}
    >
      {/* -------------------------------------------------------- */}
      {/* LEFT: RECENT FILES CARD                                  */}
      {/* -------------------------------------------------------- */}
      <div
        id="nagisa-recent-section"
        style={{
          background: '#ffffff',
          borderRadius: 16,
          border: '1px solid #e2e8f0',
          padding: '12px 18px',
          display: 'flex',
          flexDirection: 'column',
          boxShadow: '0 2px 8px rgba(0, 0, 0, 0.02)',
          boxSizing: 'border-box',
        }}
      >
        {/* Header */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 6 }}>
          <span style={{ fontSize: 14, fontWeight: 700, color: '#1e293b' }}>
            最近のファイル
          </span>
          <button
            onClick={onOpenSelectFile}
            style={{
              background: 'transparent',
              border: 'none',
              color: '#2563eb',
              fontSize: 12,
              fontWeight: 500,
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              cursor: 'pointer',
              padding: 0,
            }}
          >
            すべて表示 <ArrowRightIcon size={13} color="#2563eb" />
          </button>
        </div>

        {/* List */}
        <div style={{ display: 'flex', flexDirection: 'column', flex: 1 }}>
          {recentFiles.length > 0 ? (
            recentFiles.map((file, idx) => (
              <div
                key={file.id}
                onClick={() => onOpenRecentFile(file)}
                className="nagisa-recent-item"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  padding: '8px 8px',
                  borderRadius: 7,
                  cursor: 'pointer',
                  borderBottom: idx < recentFiles.length - 1 ? '1px solid #f1f5f9' : 'none',
                }}
              >
                {/* Red PDF File Badge Icon */}
                <div
                  style={{
                    width: 22,
                    height: 24,
                    background: '#ef4444',
                    borderRadius: 4,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    flexShrink: 0,
                    marginRight: 12,
                    boxShadow: '0 2px 4px rgba(239, 68, 68, 0.25)',
                  }}
                >
                  <span style={{ color: '#ffffff', fontSize: 7.5, fontWeight: 800, letterSpacing: '-0.02em' }}>
                    PDF
                  </span>
                </div>

                {/* File Name */}
                <div
                  style={{
                    flex: 1,
                    fontSize: 12.5,
                    fontWeight: 600,
                    color: '#1e293b',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                    marginRight: 16,
                  }}
                >
                  {file.name}
                </div>

                {/* Date */}
                <div style={{ fontSize: 11.5, color: '#94a3b8', width: 145, textAlign: 'right', flexShrink: 0 }}>
                  {file.date}
                </div>

                {/* Size */}
                <div style={{ fontSize: 11.5, color: '#94a3b8', width: 70, textAlign: 'right', flexShrink: 0 }}>
                  {file.size}
                </div>

                {/* Context Menu Button */}
                <button
                  onClick={e => {
                    e.stopPropagation()
                    onRemoveRecentFile(file.id)
                  }}
                  title="履歴から削除"
                  style={{
                    background: 'transparent',
                    border: 'none',
                    color: '#94a3b8',
                    cursor: 'pointer',
                    padding: '4px 6px',
                    borderRadius: 4,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    marginLeft: 8,
                  }}
                >
                  <MoreHorizontalIcon size={16} />
                </button>
              </div>
            ))
          ) : (
            <div style={{ padding: '28px 16px', textAlign: 'center' }}>
              <div style={{ fontSize: 13.5, fontWeight: 600, color: '#64748b' }}>最近開いたファイルはありません</div>
              <div style={{ fontSize: 12, color: '#94a3b8', marginTop: 4 }}>
                PDFファイルを上のエリアにドラッグ＆ドロップするか、「PDFを開く」から選択してください
              </div>
            </div>
          )}
        </div>
      </div>

      {/* -------------------------------------------------------- */}
      {/* RIGHT: HINTS & NOTICES CARD                              */}
      {/* -------------------------------------------------------- */}
      <div
        style={{
          background: '#ffffff',
          borderRadius: 16,
          border: '1px solid #e2e8f0',
          padding: '12px 18px',
          display: 'flex',
          flexDirection: 'column',
          gap: 8,
          boxShadow: '0 2px 8px rgba(0, 0, 0, 0.02)',
          boxSizing: 'border-box',
        }}
      >
        {/* Header */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 2 }}>
          <span style={{ fontSize: 14, fontWeight: 700, color: '#1e293b' }}>
            ヒントとお知らせ
          </span>
          <button
            onClick={onOpenGuide}
            style={{
              background: 'transparent',
              border: 'none',
              color: '#2563eb',
              fontSize: 12,
              fontWeight: 500,
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              cursor: 'pointer',
              padding: 0,
            }}
          >
            すべて見る <ArrowRightIcon size={13} color="#2563eb" />
          </button>
        </div>

        {/* Guide Carousel Card */}
        <div
          onClick={onOpenGuide}
          style={{
            position: 'relative',
            borderRadius: 12,
            overflow: 'hidden',
            background: 'linear-gradient(135deg, #f0f9ff 0%, #e0f2fe 50%, #bae6fd 100%)',
            padding: '10px 14px',
            cursor: 'pointer',
            border: '1px solid #dbeafe',
            display: 'flex',
            flexDirection: 'column',
            justifyContent: 'space-between',
            minHeight: 74,
          }}
        >
          {/* Background Wave Accent */}
          <div style={{ position: 'absolute', right: -10, bottom: -10, width: 140, height: 75, opacity: 0.7, pointerEvents: 'none' }}>
            <svg viewBox="0 0 100 50" preserveAspectRatio="none" style={{ width: '100%', height: '100%' }}>
              <path d="M0,50 Q25,10 50,30 T100,20 L100,50 Z" fill="#60a5fa" fillOpacity="0.4" />
              <path d="M0,50 Q35,25 70,35 T100,10 L100,50 Z" fill="#2563eb" fillOpacity="0.3" />
            </svg>
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: 12, position: 'relative', zIndex: 2 }}>
            <img
              src="/logo.webp"
              alt="nagisa"
              style={{
                width: 38,
                height: 38,
                borderRadius: 9,
                boxShadow: '0 3px 8px rgba(37, 99, 235, 0.2)',
                objectFit: 'cover',
              }}
            />
            <div>
              <div style={{ fontSize: 10, color: '#64748b', fontWeight: 500 }}>
                はじめての方へ
              </div>
              <div style={{ fontSize: 13.5, fontWeight: 700, color: '#0f172a' }}>
                nagisaの使い方ガイド
              </div>
            </div>
          </div>

          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginTop: 10, position: 'relative', zIndex: 2 }}>
            <button
              onClick={e => {
                e.stopPropagation()
                onOpenGuide()
              }}
              style={{
                background: '#ffffff',
                border: '1px solid #cbd5e1',
                borderRadius: 6,
                padding: '3px 10px',
                fontSize: 11,
                fontWeight: 600,
                color: '#2563eb',
                display: 'flex',
                alignItems: 'center',
                gap: 4,
                cursor: 'pointer',
                boxShadow: '0 1px 3px rgba(0,0,0,0.05)',
              }}
            >
              ガイドを見る <ArrowRightIcon size={11} color="#2563eb" />
            </button>

            {/* 4 Carousel Indicator Dots */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
              <div style={{ width: 14, height: 4, borderRadius: 2, background: '#2563eb' }} />
              <div style={{ width: 4, height: 4, borderRadius: '50%', background: '#cbd5e1' }} />
              <div style={{ width: 4, height: 4, borderRadius: '50%', background: '#cbd5e1' }} />
              <div style={{ width: 4, height: 4, borderRadius: '50%', background: '#cbd5e1' }} />
            </div>
          </div>
        </div>

        {/* Hint Item 1: OCR */}
        <div
          onClick={() => onNavigateView('ocr')}
          className="nagisa-hint-item"
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            padding: '6px 10px',
            borderRadius: 8,
            cursor: 'pointer',
            border: '1px solid #f1f5f9',
          }}
        >
          <div
            style={{
              width: 30,
              height: 30,
              borderRadius: '50%',
              background: '#fef3c7',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              flexShrink: 0,
            }}
          >
            <LightbulbIcon size={15} color="#d97706" />
          </div>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 12, fontWeight: 650, color: '#1e293b' }}>
              スキャンした書類をテキスト化
            </div>
            <div style={{ fontSize: 10, color: '#64748b' }}>
              OCRで紙の書類も編集可能に
            </div>
          </div>
          <ChevronRightIcon size={14} color="#94a3b8" />
        </div>

        {/* Hint Item 2: Shortcuts */}
        <div
          onClick={onOpenShortcuts}
          className="nagisa-hint-item"
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            padding: '6px 10px',
            borderRadius: 8,
            cursor: 'pointer',
            border: '1px solid #f1f5f9',
          }}
        >
          <div
            style={{
              width: 30,
              height: 30,
              borderRadius: '50%',
              background: '#e0f2fe',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              flexShrink: 0,
            }}
          >
            <GridIcon size={15} color="#0284c7" />
          </div>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 12, fontWeight: 650, color: '#1e293b' }}>
              より快適にご利用いただくために
            </div>
            <div style={{ fontSize: 10, color: '#64748b' }}>
              便利なショートカットキーのご紹介
            </div>
          </div>
          <ChevronRightIcon size={14} color="#94a3b8" />
        </div>
      </div>
    </section>
  )
}
