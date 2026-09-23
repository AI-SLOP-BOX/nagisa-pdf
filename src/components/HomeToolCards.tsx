import React from 'react'
import {
  EditIcon,
  ConvertIcon,
  LinkChainIcon,
  ScissorsIcon,
  OcrScanIcon,
  CompressIcon,
  ArrowRightIcon,
} from './Icons'

interface HomeToolCardsProps {
  onToolClick: (toolKey: string) => void
}

export const HomeToolCards: React.FC<HomeToolCardsProps> = ({ onToolClick }) => {
  const tools = [
    {
      key: 'edit',
      title: 'PDFを編集',
      desc: 'テキストや画像を\n編集できます',
      icon: <EditIcon size={20} color="#2563eb" />,
      bg: '#eff6ff',
      border: '#dbeafe',
      arrowColor: '#2563eb',
    },
    {
      key: 'convert',
      title: 'PDFを変換',
      desc: 'Word・Excel・\n画像などに変換',
      icon: <ConvertIcon size={20} color="#16a34a" />,
      bg: '#f0fdf4',
      border: '#dcfce7',
      arrowColor: '#16a34a',
    },
    {
      key: 'merge',
      title: 'PDFを結合',
      desc: '複数のPDFを\n1つにまとめます',
      icon: <LinkChainIcon size={20} color="#9333ea" />,
      bg: '#faf5ff',
      border: '#f3e8ff',
      arrowColor: '#9333ea',
    },
    {
      key: 'split',
      title: 'PDFを分割',
      desc: 'ページを分割して\n複数のPDFに',
      icon: <ScissorsIcon size={20} color="#ea580c" />,
      bg: '#fff7ed',
      border: '#ffedd5',
      arrowColor: '#ea580c',
    },
    {
      key: 'ocr',
      title: 'OCR',
      desc: 'スキャンされたPDFを\nテキスト化',
      icon: <OcrScanIcon size={20} color="#0891b2" />,
      bg: '#ecfeff',
      border: '#cffafe',
      arrowColor: '#0891b2',
    },
    {
      key: 'compress',
      title: '圧縮',
      desc: 'ファイルサイズを\n小さくします',
      icon: <CompressIcon size={20} color="#e11d48" />,
      bg: '#fff1f2',
      border: '#ffe4e6',
      arrowColor: '#e11d48',
    },
  ]

  return (
    <section
      style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))',
        gap: 10,
        width: '100%',
        flexShrink: 0,
      }}
    >
      {tools.map(tool => (
        <div
          key={tool.key}
          onClick={() => onToolClick(tool.key)}
          className="nagisa-tool-card"
          style={{
            background: tool.bg,
            border: `1px solid ${tool.border}`,
            height: 104,
            padding: '11px 12px',
          }}
        >
          {/* Icon */}
          <div style={{ marginBottom: 6, display: 'flex', alignItems: 'center' }}>
            {tool.icon}
          </div>
          {/* Title & Desc */}
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
            <span style={{ fontSize: 12.5, fontWeight: 700, color: '#1e293b', marginBottom: 2 }}>
              {tool.title}
            </span>
            <span style={{ fontSize: 10, color: '#64748b', lineHeight: 1.3, whiteSpace: 'pre-line' }}>
              {tool.desc}
            </span>
          </div>
          {/* Bottom Right Arrow */}
          <div style={{ alignSelf: 'flex-end', color: tool.arrowColor, marginTop: 2 }}>
            <ArrowRightIcon size={13} color={tool.arrowColor} />
          </div>
        </div>
      ))}
    </section>
  )
}
