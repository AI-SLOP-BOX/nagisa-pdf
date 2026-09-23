import React from 'react'
import type { View } from '../types'
import {
  HomeIcon,
  FolderOpenIcon,
  ClockIcon,
  StarIcon,
  EditIcon,
  ConvertIcon,
  LinkChainIcon,
  ScissorsIcon,
  CompressIcon,
  AnnotateIcon,
  OcrScanIcon,
  ShieldIcon,
  SettingsIcon,
  HelpCircleIcon,
} from './Icons'

interface NagisaSidebarProps {
  currentView: View
  onNavigateView: (view: View) => void
  onSelectFile?: () => void
  onOpenSettings?: () => void
  onOpenHelp?: () => void
}

export const NagisaSidebar: React.FC<NagisaSidebarProps> = ({
  currentView,
  onNavigateView,
  onSelectFile,
  onOpenSettings,
  onOpenHelp,
}) => {
  const mainNavItems = [
    { key: 'home', label: 'ホーム', icon: <HomeIcon size={18} /> },
    { key: 'open', label: 'PDFを開く', icon: <FolderOpenIcon size={18} />, action: onSelectFile },
    { key: 'recent', label: '最近のファイル', icon: <ClockIcon size={18} /> },
  ] as const

  const toolNavItems = [
    { key: 'merge', label: '結合', icon: <LinkChainIcon size={18} /> },
    { key: 'split', label: '分割', icon: <ScissorsIcon size={18} /> },
    { key: 'convert', label: '変換', icon: <ConvertIcon size={18} /> },
    { key: 'compress', label: '圧縮', icon: <CompressIcon size={18} /> },
    { key: 'ocr', label: 'OCR', icon: <OcrScanIcon size={18} /> },
    { key: 'protect', label: '保護', icon: <ShieldIcon size={18} /> },
  ] as const

  const handleItemClick = (item: { key: string; action?: () => void }) => {
    if (item.action) {
      item.action()
      return
    }
    onNavigateView(item.key as View)
  }

  return (
    <aside
      style={{
        width: 170,
        height: '100vh',
        background: '#f8fafc',
        borderRight: '1px solid #e2e8f0',
        display: 'flex',
        flexDirection: 'column',
        padding: '12px 14px 14px 14px',
        boxSizing: 'border-box',
        flexShrink: 0,
        userSelect: 'none',
        zIndex: 50,
      }}
    >
      {/* 2. Nagisa 3D Logo & Title Brand Header */}
      <div
        onClick={() => onNavigateView('home')}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          cursor: 'pointer',
          padding: '4px 6px 12px 6px',
        }}
      >
        <img
          src="/logo.webp"
          alt="nagisa logo"
          style={{ width: 36, height: 36, objectFit: 'contain', flexShrink: 0 }}
          draggable={false}
        />
        <div style={{ display: 'flex', flexDirection: 'column' }}>
          <span style={{ fontSize: 17, fontWeight: 800, color: '#0f172a', letterSpacing: '-0.02em', lineHeight: 1.1 }}>
            nagisa
          </span>
          <span style={{ fontSize: 9.5, color: '#64748b', lineHeight: 1.25, marginTop: 2 }}>
            Smooth PDFs<br />for a brighter day
          </span>
        </div>
      </div>

      {/* 3. Main Navigation List */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 2, marginTop: 4 }}>
        {mainNavItems.map(item => {
          const active = currentView === item.key
          return (
            <button
              key={item.key}
              onClick={() => handleItemClick(item)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 11,
                width: '100%',
                height: 36,
                padding: '0 12px',
                borderRadius: 8,
                border: 'none',
                background: active ? '#eff6ff' : 'transparent',
                color: active ? '#1d4ed8' : '#334155',
                fontSize: 13,
                fontWeight: active ? 600 : 500,
                cursor: 'pointer',
                textAlign: 'left',
                transition: 'all 0.15s ease',
              }}
              onMouseEnter={e => {
                if (!active) e.currentTarget.style.background = '#f1f5f9'
              }}
              onMouseLeave={e => {
                if (!active) e.currentTarget.style.background = 'transparent'
              }}
            >
              <span style={{ color: active ? '#1d4ed8' : '#475569', display: 'flex', alignItems: 'center' }}>
                {item.icon}
              </span>
              {item.label}
            </button>
          )
        })}
      </div>

      {/* 4. Tools Section Header */}
      <div
        style={{
          fontSize: 10.5,
          fontWeight: 700,
          color: '#94a3b8',
          textTransform: 'uppercase',
          letterSpacing: '0.04em',
          padding: '16px 12px 6px 12px',
        }}
      >
        ツール
      </div>

      {/* 5. Tool Items List */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 2, overflowY: 'auto', flex: 1 }}>
        {toolNavItems.map(tool => {
          const active = currentView === tool.key
          return (
            <button
              key={tool.key}
              onClick={() => handleItemClick(tool)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 11,
                width: '100%',
                height: 35,
                padding: '0 12px',
                borderRadius: 8,
                border: 'none',
                background: active ? '#eff6ff' : 'transparent',
                color: active ? '#1d4ed8' : '#334155',
                fontSize: 13,
                fontWeight: active ? 600 : 500,
                cursor: 'pointer',
                textAlign: 'left',
                transition: 'all 0.15s ease',
              }}
              onMouseEnter={e => {
                if (!active) e.currentTarget.style.background = '#f1f5f9'
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
      </div>

      {/* 6. Settings Pinned at Bottom */}
      <div style={{ marginTop: 'auto', paddingTop: 10, borderTop: '1px solid #e2e8f0' }}>
        <button
          onClick={onOpenSettings}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 11,
            width: '100%',
            height: 35,
            padding: '0 12px',
            borderRadius: 8,
            border: 'none',
            background: 'transparent',
            color: '#334155',
            fontSize: 13,
            fontWeight: 500,
            cursor: 'pointer',
            textAlign: 'left',
            transition: 'all 0.15s ease',
          }}
          onMouseEnter={e => (e.currentTarget.style.background = '#f1f5f9')}
          onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
        >
          <SettingsIcon size={18} color="#475569" />
          設定
        </button>
      </div>
    </aside>
  )
}
