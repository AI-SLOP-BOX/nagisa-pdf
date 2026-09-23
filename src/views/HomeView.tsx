import React, { useState, useEffect, useRef } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import type { View, RecentPDFFile, EditorTab } from '../types'
import { HomeSidebar } from '../components/HomeSidebar'
import { HomeHeroDropZone } from '../components/HomeHeroDropZone'
import { HomeToolCards } from '../components/HomeToolCards'
import { HomeRecentSection } from '../components/HomeRecentSection'
import { UsageGuideModal, ShortcutKeysModal, SettingsModal } from '../components/HomeModals'

interface HomeViewProps {
  onOpenFile: (bytes: number[], name: string, path?: string, sizeStr?: string) => void
  onNavigateView: (view: View) => void
  onOpenToolTab?: (tab: EditorTab) => void
  hideSidebar?: boolean
}

export default function HomeView({ onOpenFile, onNavigateView, onOpenToolTab, hideSidebar = false }: HomeViewProps) {
  const [isDragging, setIsDragging] = useState(false)
  const [activeNav, setActiveNav] = useState<'home' | 'open' | 'recent'>('home')
  const [activeTool, setActiveTool] = useState<string | null>(null)
  const [recentFiles, setRecentFiles] = useState<RecentPDFFile[]>([])
  const [showSettingsModal, setShowSettingsModal] = useState(false)
  const [showGuideModal, setShowGuideModal] = useState(false)
  const [showShortcutModal, setShowShortcutModal] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  // Load user's actual recent files from localStorage, purging any old mock samples
  useEffect(() => {
    try {
      const saved = localStorage.getItem('nagisa_recent_files')
      if (saved) {
        const parsed = JSON.parse(saved)
        if (Array.isArray(parsed)) {
          const validRealFiles = parsed.filter(f =>
            f && f.name &&
            !f.id?.startsWith('sample-')
          )
          setRecentFiles(validRealFiles)
          try {
            localStorage.setItem('nagisa_recent_files', JSON.stringify(validRealFiles))
          } catch {}
          return
        }
      }
    } catch {
      // ignore
    }
    setRecentFiles([])
  }, [])

  const saveRecentFile = (item: RecentPDFFile) => {
    setRecentFiles(prev => {
      const filtered = prev.filter(f => f.name !== item.name)
      const updated = [item, ...filtered].slice(0, 10)
      try {
        localStorage.setItem('nagisa_recent_files', JSON.stringify(updated))
      } catch {}
      return updated
    })
  }

  const formatCurrentDate = () => {
    const d = new Date()
    const year = d.getFullYear()
    const month = d.getMonth() + 1
    const day = d.getDate()
    const hours = String(d.getHours()).padStart(2, '0')
    const mins = String(d.getMinutes()).padStart(2, '0')
    return `${year}年${month}月${day}日 ${hours}:${mins}`
  }

  // Native or fallback file open dialog
  const handleSelectFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'PDF Document', extensions: ['pdf'] }],
      })
      if (selected && typeof selected === 'string') {
        try {
          const { readFile } = await import('@tauri-apps/plugin-fs')
          const fileBytes = await readFile(selected)
          const name = selected.split(/[/\\]/).pop() || 'document.pdf'
          const sizeStr = formatSize(fileBytes.byteLength)

          saveRecentFile({
            id: String(Date.now()),
            name,
            date: formatCurrentDate(),
            size: sizeStr,
            filePath: selected,
          })
          onOpenFile(Array.from(fileBytes), name, selected, sizeStr)
          return
        } catch (readErr) {
          // File was chosen but could not be read (moved/permission) — fall back to chooser
          console.error('Failed to read selected file:', readErr)
          alert('ファイルの読み込みに失敗しました')
        }
      }
    } catch (err) {
      // Dialog itself failed (or running outside Tauri) — fall back to browser input
      console.warn('Native file dialog unavailable, falling back to browser input:', err)
    }
    if (fileInputRef.current) {
      fileInputRef.current.value = ''
      fileInputRef.current.click()
    }
  }

  const handleBrowserFileInput = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    // Reset so selecting the same file again still fires onChange
    e.target.value = ''
    if (!file) return
    try {
      const arrayBuffer = await file.arrayBuffer()
      const bytes = Array.from(new Uint8Array(arrayBuffer))
      const sizeStr = formatSize(file.size)

      saveRecentFile({
        id: String(Date.now()),
        name: file.name,
        date: formatCurrentDate(),
        size: sizeStr,
      })
      onOpenFile(bytes, file.name, undefined, sizeStr)
    } catch (err) {
      console.error('Failed to read selected file:', err)
      alert('ファイルの読み込みに失敗しました')
    }
  }

  const formatSize = (bytes: number) => {
    const sizeMb = (bytes / (1024 * 1024)).toFixed(1)
    return Number(sizeMb) < 1 ? `${Math.round(bytes / 1024)} KB` : `${sizeMb} MB`
  }

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragging(false)
    const file = Array.from(e.dataTransfer.files).find(f => /\.pdf$/i.test(f.name))
    if (!file) return
    try {
      const arrayBuffer = await file.arrayBuffer()
      const bytes = Array.from(new Uint8Array(arrayBuffer))
      const sizeStr = formatSize(file.size)

      saveRecentFile({
        id: String(Date.now()),
        name: file.name,
        date: formatCurrentDate(),
        size: sizeStr,
      })
      onOpenFile(bytes, file.name, undefined, sizeStr)
    } catch (err) {
      console.error('Failed to read dropped file:', err)
      alert('ファイルの読み込みに失敗しました')
    }
  }

  const handleOpenRecentFile = async (file: RecentPDFFile) => {
    if (file.filePath) {
      try {
        const { readFile } = await import('@tauri-apps/plugin-fs')
        const fileBytes = await readFile(file.filePath)
        onOpenFile(Array.from(fileBytes), file.name, file.filePath, file.size)
        return
      } catch (err) {
        console.warn('Failed to read recent file from path:', file.filePath, err)
      }
    }
    // If file path is missing or file was moved/deleted, launch file chooser
    handleSelectFile()
  }

  const handleRemoveRecentFile = (id: string) => {
    setRecentFiles(prev => {
      const updated = prev.filter(f => f.id !== id)
      try {
        localStorage.setItem('nagisa_recent_files', JSON.stringify(updated))
      } catch {}
      return updated
    })
  }

  // Quick Action click handlers
  const handleToolClick = (toolKey: string) => {
    setActiveTool(toolKey)
    if (toolKey === 'ocr') {
      onNavigateView('ocr')
      return
    }
    if (toolKey === 'merge') {
      onNavigateView('merge')
      return
    }
    if (toolKey === 'convert') {
      onNavigateView('convert')
      return
    }
    if (toolKey === 'split') {
      onNavigateView('split')
      return
    }
    if (toolKey === 'compress') {
      onNavigateView('compress')
      return
    }
    if (toolKey === 'edit') {
      handleSelectFile()
      return
    }
    if (toolKey === 'protect') {
      if (onOpenToolTab) onOpenToolTab('security')
      onNavigateView('pdf-editor')
      return
    }
  }

  return (
    <div className="nagisa-home-container" style={{ display: 'flex', width: '100%', height: '100vh', overflow: 'hidden', background: '#eef2f6' }}>
      {/* Hidden File Input for browser fallback */}
      <input
        type="file"
        ref={fileInputRef}
        accept=".pdf"
        style={{ display: 'none' }}
        onChange={handleBrowserFileInput}
      />

      {/* Conditional Sidebar */}
      {!hideSidebar && (
        <HomeSidebar
          activeNav={activeNav}
          setActiveNav={setActiveNav}
          activeTool={activeTool}
          setActiveTool={setActiveTool}
          onOpenSelectFile={handleSelectFile}
          onToolClick={handleToolClick}
          onOpenSettings={() => setShowSettingsModal(true)}
        />
      )}

      {/* Main Content Area */}
      <main
        style={{
          flex: 1,
          height: '100vh',
          overflowY: 'auto',
          background: '#f8fafc',
          padding: '14px 24px 18px 24px',
          boxSizing: 'border-box',
          display: 'flex',
          flexDirection: 'column',
          gap: 20,
        }}
      >
        {/* Hero & Dropzone */}
        <HomeHeroDropZone
          isDragging={isDragging}
          setIsDragging={setIsDragging}
          onDrop={handleDrop}
          onSelectFile={handleSelectFile}
        />

        {/* Quick Tool Cards (6 cards matching design) */}
        <HomeToolCards onToolClick={handleToolClick} />

        {/* Recent Files & Hints */}
        <HomeRecentSection
          recentFiles={recentFiles}
          onOpenSelectFile={handleSelectFile}
          onOpenRecentFile={handleOpenRecentFile}
          onRemoveRecentFile={handleRemoveRecentFile}
          onOpenGuide={() => setShowGuideModal(true)}
          onOpenShortcuts={() => setShowShortcutModal(true)}
          onNavigateView={onNavigateView}
        />
      </main>

      {/* Modals */}
      <UsageGuideModal isOpen={showGuideModal} onClose={() => setShowGuideModal(false)} />
      <ShortcutKeysModal isOpen={showShortcutModal} onClose={() => setShowShortcutModal(false)} />
      <SettingsModal isOpen={showSettingsModal} onClose={() => setShowSettingsModal(false)} />
    </div>
  )
}
