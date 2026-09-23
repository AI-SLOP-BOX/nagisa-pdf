import React from 'react'
import {
  HomeIcon,
  FolderOpenIcon,
  ClockIcon,
  EditIcon,
  LinkChainIcon,
  ScissorsIcon,
  ConvertIcon,
  CompressIcon,
  OcrScanIcon,
  ShieldIcon,
  SettingsIcon,
} from './Icons'

interface HomeSidebarProps {
  activeNav: 'home' | 'open' | 'recent'
  setActiveNav: (nav: 'home' | 'open' | 'recent') => void
  activeTool: string | null
  setActiveTool: (tool: string | null) => void
  onOpenSelectFile: () => void
  onToolClick: (toolKey: string) => void
  onOpenSettings: () => void
}

export const HomeSidebar: React.FC<HomeSidebarProps> = ({
  activeNav,
  setActiveNav,
  activeTool,
  setActiveTool,
  onOpenSelectFile,
  onToolClick,
  onOpenSettings,
}) => {
  return (
    <aside
      style={{
        width: 220,
        background: '#f4f6fa',
        borderRight: '1px solid #e1e7ee',
        display: 'flex',
        flexDirection: 'column',
        flexShrink: 0,
        padding: '16px 14px',
        boxSizing: 'border-box',
        userSelect: 'none',
      }}
    >
      {/* nagisa Brand Logo & Catchphrase */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '4px 6px', marginBottom: 22 }}>
        <img
          src="/logo.webp"
          alt="nagisa"
          style={{
            width: 42,
            height: 42,
            borderRadius: 10,
            boxShadow: '0 4px 14px rgba(29, 78, 216, 0.22)',
            objectFit: 'cover',
          }}
        />
        <div style={{ display: 'flex', flexDirection: 'column' }}>
          <span style={{ fontSize: 20, fontWeight: 750, color: '#0f172a', letterSpacing: '-0.02em', lineHeight: 1.15 }}>
            nagisa
          </span>
          <span style={{ fontSize: 9.5, color: '#64748b', fontWeight: 500, lineHeight: 1.25, marginTop: 2 }}>
            Smooth PDFs<br />for a brighter day
          </span>
        </div>
      </div>

      {/* Main Navigation Menu */}
      <nav style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        <button
          onClick={() => { setActiveNav('home'); setActiveTool(null); }}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 12,
            width: '100%',
            height: 38,
            padding: '0 12px',
            borderRadius: 8,
            border: 'none',
            background: activeNav === 'home' && !activeTool ? '#dbeafe' : 'transparent',
            color: activeNav === 'home' && !activeTool ? '#1d4ed8' : '#334155',
            fontSize: 13.5,
            fontWeight: activeNav === 'home' && !activeTool ? 650 : 500,
            cursor: 'pointer',
            textAlign: 'left',
            transition: 'all 0.15s ease',
          }}
        >
          <HomeIcon size={18} color={activeNav === 'home' && !activeTool ? '#1d4ed8' : '#475569'} filled={activeNav === 'home' && !activeTool} />
          ホーム
        </button>

        <button
          onClick={() => { setActiveNav('open'); onOpenSelectFile(); }}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 12,
            width: '100%',
            height: 38,
            padding: '0 12px',
            borderRadius: 8,
            border: 'none',
            background: 'transparent',
            color: '#334155',
            fontSize: 13.5,
            fontWeight: 500,
            cursor: 'pointer',
            textAlign: 'left',
            transition: 'all 0.15s ease',
          }}
          onMouseEnter={e => (e.currentTarget.style.background = '#e9eef5')}
          onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
        >
          <FolderOpenIcon size={18} color="#475569" />
          PDFを開く
        </button>

        <button
          onClick={() => {
            setActiveNav('recent');
            const el = document.getElementById('nagisa-recent-section')
            if (el) el.scrollIntoView({ behavior: 'smooth' })
          }}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 12,
            width: '100%',
            height: 38,
            padding: '0 12px',
            borderRadius: 8,
            border: 'none',
            background: 'transparent',
            color: '#334155',
            fontSize: 13.5,
            fontWeight: 500,
            cursor: 'pointer',
            textAlign: 'left',
            transition: 'all 0.15s ease',
          }}
          onMouseEnter={e => (e.currentTarget.style.background = '#e9eef5')}
          onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
        >
          <ClockIcon size={18} color="#475569" />
          最近のファイル
        </button>
      </nav>

      {/* Tools Section Header & Divider */}
      <div style={{ margin: '22px 0 10px', padding: '0 8px', display: 'flex', alignItems: 'center', gap: 10 }}>
        <span style={{ fontSize: 11, fontWeight: 600, color: '#94a3b8', letterSpacing: '0.02em' }}>
          ツール
        </span>
        <div style={{ flex: 1, height: 1, background: '#e2e8f0' }} />
      </div>

      {/* Tools Menu List */}
      <nav style={{ display: 'flex', flexDirection: 'column', gap: 2, flex: 1 }}>
        {[
          { key: 'merge', label: '結合', icon: <LinkChainIcon size={17} /> },
          { key: 'split', label: '分割', icon: <ScissorsIcon size={17} /> },
          { key: 'convert', label: '変換', icon: <ConvertIcon size={17} /> },
          { key: 'compress', label: '圧縮', icon: <CompressIcon size={17} /> },
          { key: 'ocr', label: 'OCR', icon: <OcrScanIcon size={17} /> },
          { key: 'protect', label: '保護', icon: <ShieldIcon size={17} /> },
        ].map(tool => {
          const active = activeTool === tool.key
          return (
            <button
              key={tool.key}
              onClick={() => onToolClick(tool.key)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 12,
                width: '100%',
                height: 36,
                padding: '0 12px',
                borderRadius: 7,
                border: 'none',
                background: active ? '#e0edff' : 'transparent',
                color: active ? '#1d4ed8' : '#334155',
                fontSize: 13.5,
                fontWeight: active ? 600 : 500,
                cursor: 'pointer',
                textAlign: 'left',
                transition: 'all 0.15s ease',
              }}
              onMouseEnter={e => {
                if (!active) e.currentTarget.style.background = '#e9eef5'
              }}
              onMouseLeave={e => {
                if (!active) e.currentTarget.style.background = 'transparent'
              }}
            >
              <span style={{ color: active ? '#1d4ed8' : '#475569', display: 'flex', alignItems: 'center' }}>
                {tool.icon}
              </span>
              {tool.label}
            </button>
          )
        })}
      </nav>

      {/* Settings Button pinned at the bottom */}
      <div style={{ marginTop: 'auto', paddingTop: 12, borderTop: '1px solid #e2e8f0' }}>
        <button
          onClick={onOpenSettings}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 12,
            width: '100%',
            height: 38,
            padding: '0 12px',
            borderRadius: 8,
            border: 'none',
            background: 'transparent',
            color: '#334155',
            fontSize: 13.5,
            fontWeight: 500,
            cursor: 'pointer',
            textAlign: 'left',
            transition: 'all 0.15s ease',
          }}
          onMouseEnter={e => (e.currentTarget.style.background = '#e9eef5')}
          onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
        >
          <SettingsIcon size={18} color="#475569" />
          設定
        </button>
      </div>
    </aside>
  )
}
