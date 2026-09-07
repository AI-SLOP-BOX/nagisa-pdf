import React from 'react'
import {
  FolderOpenIcon, SaveIcon, PrinterIcon, UndoIcon, RedoIcon,
  EyeIcon, TypeIcon, HighlightIcon, RectIcon, RedactIcon, SearchIcon,
  EditIcon, AnnotateIcon, FormIcon, OrganizeIcon, FileIcon, LockIcon, ToolsIcon,
  CameraIcon
} from './Icons'
import { ToolBtn } from './UIControls'
import { InteractiveMode } from './PDFViewer'
import { t } from '../utils/i18n'
import type { View } from '../types'

export type Tab = 'edit' | 'annotate' | 'forms' | 'organize' | 'pages' | 'security' | 'text' | 'tools'

interface EditorHeaderProps {
  onOpen: () => void
  onSave: () => void
  onPrint: () => void
  canSave: boolean
  undo: () => void
  redo: () => void
  canUndo: boolean
  canRedo: boolean
  interactiveMode: InteractiveMode
  setInteractiveMode: (mode: InteractiveMode) => void
  fileName: string
  pageCount: number
  currentPage: number
  onOpenCommandPalette: () => void
  activeTab: Tab | null
  onSelectTab: (tab: Tab) => void
  currentView?: View
  onNavigateView?: (view: View) => void
}

export function EditorHeader({
  onOpen,
  onSave,
  onPrint,
  canSave,
  undo,
  redo,
  canUndo,
  canRedo,
  interactiveMode,
  setInteractiveMode,
  fileName,
  pageCount,
  currentPage,
  onOpenCommandPalette,
  activeTab,
  onSelectTab,
  currentView = 'pdf-editor',
  onNavigateView,
}: EditorHeaderProps) {
  const tabs: { tab: Tab; icon: React.ReactNode; label: string }[] = [
    { tab: 'edit', icon: <EditIcon size={13} />, label: t().tabEdit },
    { tab: 'annotate', icon: <AnnotateIcon size={13} />, label: t().tabAnnotate },
    { tab: 'text', icon: <TypeIcon size={13} />, label: t().tabText },
    { tab: 'forms', icon: <FormIcon size={13} />, label: t().tabForms },
    { tab: 'organize', icon: <OrganizeIcon size={13} />, label: t().tabOrganize },
    { tab: 'pages', icon: <FileIcon size={13} />, label: t().tabPages },
    { tab: 'security', icon: <LockIcon size={13} />, label: t().tabSecurity },
    { tab: 'tools', icon: <ToolsIcon size={13} />, label: t().tabTools },
  ]

  return (
    <header
      style={{
        display: 'flex',
        flexDirection: 'column',
        background: 'var(--bg-1)',
        borderBottom: '1px solid var(--border)',
        flexShrink: 0,
        userSelect: 'none',
      }}
    >
      {/* Primary Toolbar (App Brand, File Ops, History, Quick Search & Mode Switcher) */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          padding: '6px 14px',
          borderBottom: '1px solid var(--border-subtle)',
          fontSize: 12,
        }}
      >
        {/* Brand Icon & Title */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginRight: 6 }}>
          <img
            src="/favicon.png"
            alt="Nagisa PDF"
            style={{
              width: 22,
              height: 22,
              borderRadius: 4,
              boxShadow: '0 2px 8px rgba(47, 129, 247, 0.4)',
            }}
          />
          <span
            style={{
              fontSize: 13,
              fontWeight: 700,
              color: 'var(--text)',
              letterSpacing: '-0.02em',
            }}
          >
            Nagisa PDF
          </span>
        </div>

        {/* Global Workspace View Switcher (PDF / Scan / OCR) */}
        {onNavigateView && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 2,
              padding: 2,
              background: 'var(--bg-0)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius-sm)',
              marginRight: 6,
            }}
          >
            <button
              onClick={() => onNavigateView('pdf-editor')}
              className={`mode-switcher-btn ${currentView === 'pdf-editor' ? 'active' : ''}`}
              title="PDF 編集・閲覧"
            >
              <FileIcon size={12} />
              <span>PDF編集</span>
            </button>
            <button
              onClick={() => onNavigateView('scanner')}
              className={`mode-switcher-btn ${currentView === 'scanner' ? 'active' : ''}`}
              title="カメラ・画像スキャン補正"
            >
              <CameraIcon size={12} />
              <span>スキャナ</span>
            </button>
            <button
              onClick={() => onNavigateView('ocr')}
              className={`mode-switcher-btn ${currentView === 'ocr' ? 'active' : ''}`}
              title="OCR文字認識 & EPUB/テキスト変換"
            >
              <TypeIcon size={12} />
              <span>OCR</span>
            </button>
          </div>
        )}

        <div style={{ width: 1, height: 16, background: 'var(--border)' }} />

        {/* File & Edit Actions */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 2 }}>
          <ToolBtn icon={<FolderOpenIcon size={14} />} onClick={onOpen} title={t().open} shortcut="⌘O" />
          <ToolBtn icon={<SaveIcon size={14} />} onClick={onSave} disabled={!canSave} title={t().save} shortcut="⌘S" />
          <ToolBtn icon={<PrinterIcon size={14} />} onClick={onPrint} disabled={!canSave} title={t().print} shortcut="⌘P" />
        </div>

        <div style={{ width: 1, height: 16, background: 'var(--border)' }} />

        {/* Undo / Redo */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 2 }}>
          <ToolBtn icon={<UndoIcon size={14} />} onClick={undo} disabled={!canUndo} title={t().undo} shortcut="⌘Z" />
          <ToolBtn icon={<RedoIcon size={14} />} onClick={redo} disabled={!canRedo} title={t().redo} shortcut="⌘⇧Z" />
        </div>

        <div style={{ flex: 1 }} />

        {/* Loaded Document Info Capsule */}
        {fileName ? (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
              padding: '3px 12px',
              background: 'var(--bg-0)',
              borderRadius: 'var(--radius-sm)',
              border: '1px solid var(--border)',
            }}
          >
            <span style={{ color: 'var(--text)', fontSize: 11, fontWeight: 600, maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {fileName}
            </span>
            {pageCount > 0 && (
              <span style={{ color: 'var(--text-muted)', fontSize: 11 }}>
                {currentPage + 1} / {pageCount}
              </span>
            )}
          </div>
        ) : (
          <span style={{ color: 'var(--text-muted)', fontSize: 11, fontStyle: 'italic' }}>
            未読み込み
          </span>
        )}

        {/* Quick Command Launcher (⌘K) */}
        <button
          onClick={onOpenCommandPalette}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 6,
            padding: '4px 10px',
            background: 'var(--bg-0)',
            border: '1px solid var(--border)',
            borderRadius: 'var(--radius-sm)',
            color: 'var(--text-dim)',
            fontSize: 11,
            cursor: 'pointer',
          }}
          title="ツール・機能検索 (⌘K)"
        >
          <SearchIcon size={12} color="var(--accent)" />
          <span>検索</span>
          <span
            style={{
              padding: '1px 5px',
              borderRadius: 3,
              background: 'rgba(255, 255, 255, 0.08)',
              fontSize: 10,
              fontFamily: 'monospace',
              color: 'var(--text-muted)',
            }}
          >
            ⌘K
          </span>
        </button>
      </div>

      {/* Secondary Ribbon (Functional Workspaces & Canvas Interaction Modes) */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '5px 14px',
          background: 'var(--bg-0)',
          fontSize: 12,
          gap: 12,
        }}
      >
        {/* Main Functional Tabs Bar */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 3, overflowX: 'auto' }}>
          {tabs.map(({ tab, icon, label }) => {
            const isActive = activeTab === tab
            return (
              <button
                key={tab}
                onClick={() => onSelectTab(tab)}
                className={`editor-tab-btn ${isActive ? 'active' : ''}`}
                title={`${label}パネルを表示/非表示`}
              >
                <span style={{ display: 'flex', alignItems: 'center', color: isActive ? 'var(--accent)' : 'inherit' }}>
                  {icon}
                </span>
                <span>{label}</span>
              </button>
            )
          })}
        </div>

        {/* Interaction Modes Segmented Pill */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 1,
            padding: 2,
            background: 'var(--bg-1)',
            border: '1px solid var(--border)',
            borderRadius: 'var(--radius-sm)',
            flexShrink: 0,
          }}
        >
          {([
            ['view', <EyeIcon size={12} />, t().modeView],
            ['select-text', <TypeIcon size={12} />, t().modeText],
            ['draw-highlight', <HighlightIcon size={12} />, t().modeHighlight],
            ['draw-rect', <RectIcon size={12} />, t().modeRect],
            ['draw-redact', <RedactIcon size={12} />, t().modeRedact],
          ] as const).map(([mode, icon, label]) => {
            const active = interactiveMode === mode
            return (
              <button
                key={mode}
                onClick={() => setInteractiveMode(mode as InteractiveMode)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 4,
                  padding: '3px 8px',
                  borderRadius: 4,
                  fontSize: 11,
                  fontWeight: active ? 600 : 500,
                  background: active ? 'var(--bg-2)' : 'transparent',
                  color: active ? 'var(--accent)' : 'var(--text-muted)',
                  boxShadow: active ? '0 1px 3px rgba(0,0,0,0.3)' : 'none',
                }}
              >
                <span style={{ display: 'flex' }}>{icon}</span>
                <span>{label}</span>
              </button>
            )
          })}
        </div>
      </div>
    </header>
  )
}
