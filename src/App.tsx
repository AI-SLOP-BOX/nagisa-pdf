import { useState } from 'react'
import { I18nProvider } from './utils/i18nContext'
import type { View, EditorTab } from './types'
import HomeView from './views/HomeView'
import PDFEditorView from './views/PDFEditorView'
import { PDFConvertView } from './views/PDFConvertView'
import { PDFMergeView } from './views/PDFMergeView'
import ScannerView from './views/ScannerView'
import OCRView from './views/OCRView'
import { HomeIcon, FileIcon, CameraIcon, TypeIcon } from './components/Icons'
import { DocumentOpeningOverlay } from './components/DocumentOpeningOverlay'
import { NagisaSidebar } from './components/NagisaSidebar'
import { ToastOverlay } from './components/ToastOverlay'
import { ErrorBoundary } from './components/ErrorBoundary'
import { useToast } from './hooks/useToast'
import { useAppToastListener } from './hooks/useAppToastListener'

export default function App() {
  const [currentView, setCurrentView] = useState<View>('home')
  const [activeFile, setActiveFile] = useState<{ bytes: number[]; name: string; path?: string } | null>(null)
  const [editorInitialTab, setEditorInitialTab] = useState<EditorTab | null>(null)
  const [openingDoc, setOpeningDoc] = useState<{ name: string; size?: string } | null>(null)

  // サービス層（pdfRenderer / annotationService）からの通知を画面表示へブリッジする
  const { toast, toastType, showToast } = useToast(2800)
  useAppToastListener(showToast)

  const handleOpenFileFromHome = (bytes: number[], name: string, path?: string, sizeStr?: string) => {
    setOpeningDoc({ name, size: sizeStr })
    setActiveFile({ bytes, name, path })
    setCurrentView('pdf-editor')
  }

  const handleOpenToolTab = (tab: EditorTab) => {
    setEditorInitialTab(tab)
  }

  return (
    <I18nProvider>
      {/* 最外郭のエラー境界: あらゆるクラッシュでも白屏を防ぐ */}
      <ErrorBoundary>
      <div style={{ display: 'flex', flexDirection: 'row', height: '100vh', width: '100vw', overflow: 'hidden', background: '#f8fafc' }}>
      {/* サービス層由来の通知トースト（アプリ全体で共通） */}
      <ToastOverlay message={toast} type={toastType} />
      {/* Nagisa Opening Transition Animation Overlay */}
      <DocumentOpeningOverlay
        isOpen={!!openingDoc}
        fileName={openingDoc?.name || ''}
        fileSize={openingDoc?.size}
        onComplete={() => setOpeningDoc(null)}
      />

      {/* Persistent Left Nagisa App Sidebar */}
      <NagisaSidebar
        currentView={currentView}
        onNavigateView={setCurrentView}
        onSelectFile={() => {
          // Open file picker or switch to editor
          setCurrentView('home')
        }}
      />

      {/* Main Workspace Area */}
      <main style={{ flex: 1, height: '100vh', overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        {/* ビュー単位のエラー境界: 画面遷移(resetKey)で自動復帰し、サイドバー等は維持する */}
        <ErrorBoundary resetKey={currentView}>
        {currentView === 'home' && (
          <HomeView
            hideSidebar={true}
            onOpenFile={handleOpenFileFromHome}
            onNavigateView={setCurrentView}
            onOpenToolTab={handleOpenToolTab}
          />
        )}
        {currentView === 'pdf-editor' && (
          <PDFEditorView
            currentView={currentView}
            onNavigateView={setCurrentView}
            initialFile={activeFile}
            initialTab={editorInitialTab}
            onOpenStart={(name, size) => setOpeningDoc({ name, size })}
          />
        )}
        {currentView === 'convert' && (
          <PDFConvertView
            onNavigateView={setCurrentView}
            onOpenFile={handleOpenFileFromHome}
          />
        )}
        {currentView === 'merge' && (
          <PDFMergeView
            onNavigateView={setCurrentView}
            onOpenFile={handleOpenFileFromHome}
          />
        )}
        {(currentView === 'split' || currentView === 'compress' || currentView === 'annotate' || currentView === 'protect') && (
          <PDFEditorView
            currentView={currentView}
            onNavigateView={setCurrentView}
            initialFile={activeFile}
            initialTab={currentView === 'annotate' ? 'annotate' : currentView === 'protect' ? 'security' : 'tools'}
            onOpenStart={(name, size) => setOpeningDoc({ name, size })}
          />
        )}
        {currentView === 'scanner' && (
          <ScannerView currentView={currentView} onNavigateView={setCurrentView} />
        )}
        {currentView === 'ocr' && (
          <OCRView currentView={currentView} onNavigateView={setCurrentView} />
        )}
        </ErrorBoundary>
      </main>

      {/* Mobile Bottom Navigation Bar (iPhone / Android Phone <= 640px) */}
      <nav
        className="mobile-bottom-nav"
        style={{
          display: 'none',
          height: 'calc(52px + var(--safe-bottom, 0px))',
          paddingBottom: 'var(--safe-bottom, 0px)',
          background: 'var(--bg-1)',
          borderTop: '1px solid var(--border)',
          alignItems: 'center',
          justifyContent: 'space-around',
          flexShrink: 0,
          zIndex: 100,
        }}
      >
        {([
          { view: 'home', label: 'ホーム', icon: <HomeIcon size={19} /> },
          { view: 'pdf-editor', label: 'PDF編集', icon: <FileIcon size={19} /> },
          { view: 'scanner', label: 'スキャン', icon: <CameraIcon size={19} /> },
          { view: 'ocr', label: 'OCR変換', icon: <TypeIcon size={19} /> },
        ] as const).map(item => {
          const active = currentView === item.view
          return (
            <button
              key={item.view}
              onClick={() => setCurrentView(item.view)}
              style={{
                flex: 1,
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                gap: 3,
                background: 'transparent',
                color: active ? 'var(--accent)' : 'var(--text-muted)',
                padding: '6px 0',
              }}
            >
              {item.icon}
              <span style={{ fontSize: 10, fontWeight: active ? 600 : 500 }}>{item.label}</span>
            </button>
          )
        })}
      </nav>
  </div>
      </ErrorBoundary>
    </I18nProvider>
  )
}
