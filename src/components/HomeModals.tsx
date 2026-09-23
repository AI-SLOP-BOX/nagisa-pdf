import React from 'react'
import { SettingsIcon } from './Icons'

interface UsageGuideModalProps {
  isOpen: boolean
  onClose: () => void
}

export const UsageGuideModal: React.FC<UsageGuideModalProps> = ({ isOpen, onClose }) => {
  if (!isOpen) return null

  return (
    <div
      onClick={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(15, 23, 42, 0.45)',
        backdropFilter: 'blur(4px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
    >
      <div
        onClick={e => e.stopPropagation()}
        style={{
          background: '#ffffff',
          borderRadius: 16,
          padding: '24px 28px',
          maxWidth: 480,
          width: '90%',
          boxShadow: '0 20px 40px rgba(0, 0, 0, 0.15)',
          display: 'flex',
          flexDirection: 'column',
          gap: 16,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <img
            src="/logo.webp"
            alt="nagisa"
            style={{
              width: 36,
              height: 36,
              borderRadius: 8,
              objectFit: 'cover',
            }}
          />
          <div>
            <h3 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: '#0f172a' }}>nagisaの使い方ガイド</h3>
            <p style={{ margin: 0, fontSize: 12, color: '#64748b' }}>日々のPDF作業をスマートかつ美しく</p>
          </div>
        </div>
        <div style={{ fontSize: 13, color: '#334155', lineHeight: 1.6, display: 'flex', flexDirection: 'column', gap: 10 }}>
          <div><strong>1. ファイルを開く</strong>: ドラッグ＆ドロップまたは「ファイルを選択」からPDFを即座に開きます。</div>
          <div><strong>2. 編集・注釈</strong>: テキスト直接編集、墨消し、ハイライト、電子署名、ページ結合・分割に対応。</div>
          <div><strong>3. 100%ローカル処理</strong>: nagisaはお客様のPDFデータを外部サーバーに一切送信せず、お使いの端末内で超高速に処理します。</div>
        </div>
        <button
          onClick={onClose}
          style={{
            background: '#2563eb',
            color: '#fff',
            border: 'none',
            borderRadius: 8,
            padding: '9px 18px',
            fontSize: 13,
            fontWeight: 600,
            cursor: 'pointer',
            alignSelf: 'flex-end',
          }}
        >
          閉じる
        </button>
      </div>
    </div>
  )
}

interface ShortcutKeysModalProps {
  isOpen: boolean
  onClose: () => void
}

export const ShortcutKeysModal: React.FC<ShortcutKeysModalProps> = ({ isOpen, onClose }) => {
  if (!isOpen) return null

  return (
    <div
      onClick={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(15, 23, 42, 0.45)',
        backdropFilter: 'blur(4px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
    >
      <div
        onClick={e => e.stopPropagation()}
        style={{
          background: '#ffffff',
          borderRadius: 16,
          padding: '24px 28px',
          maxWidth: 440,
          width: '90%',
          boxShadow: '0 20px 40px rgba(0, 0, 0, 0.15)',
          display: 'flex',
          flexDirection: 'column',
          gap: 16,
        }}
      >
        <h3 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: '#0f172a' }}>ショートカットキー一覧</h3>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr auto', gap: '8px 16px', fontSize: 12.5, color: '#334155' }}>
          <span>ファイルを開く</span><kbd style={{ background: '#f1f5f9', padding: '2px 6px', borderRadius: 4, border: '1px solid #cbd5e1' }}>⌘ + O</kbd>
          <span>保存</span><kbd style={{ background: '#f1f5f9', padding: '2px 6px', borderRadius: 4, border: '1px solid #cbd5e1' }}>⌘ + S</kbd>
          <span>元に戻す</span><kbd style={{ background: '#f1f5f9', padding: '2px 6px', borderRadius: 4, border: '1px solid #cbd5e1' }}>⌘ + Z</kbd>
          <span>やり直す</span><kbd style={{ background: '#f1f5f9', padding: '2px 6px', borderRadius: 4, border: '1px solid #cbd5e1' }}>⌘ + ⇧ + Z</kbd>
          <span>コマンドパレット</span><kbd style={{ background: '#f1f5f9', padding: '2px 6px', borderRadius: 4, border: '1px solid #cbd5e1' }}>⌘ + K</kbd>
        </div>
        <button
          onClick={onClose}
          style={{
            background: '#2563eb',
            color: '#fff',
            border: 'none',
            borderRadius: 8,
            padding: '9px 18px',
            fontSize: 13,
            fontWeight: 600,
            cursor: 'pointer',
            alignSelf: 'flex-end',
          }}
        >
          閉じる
        </button>
      </div>
    </div>
  )
}

interface SettingsModalProps {
  isOpen: boolean
  onClose: () => void
}

export const SettingsModal: React.FC<SettingsModalProps> = ({ isOpen, onClose }) => {
  if (!isOpen) return null

  return (
    <div
      onClick={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(15, 23, 42, 0.45)',
        backdropFilter: 'blur(4px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
    >
      <div
        onClick={e => e.stopPropagation()}
        style={{
          background: '#ffffff',
          borderRadius: 16,
          padding: '24px 28px',
          maxWidth: 420,
          width: '90%',
          boxShadow: '0 20px 40px rgba(0, 0, 0, 0.15)',
          display: 'flex',
          flexDirection: 'column',
          gap: 16,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <SettingsIcon size={22} color="#2563eb" />
          <h3 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: '#0f172a' }}>nagisa 設定</h3>
        </div>
        <div style={{ fontSize: 13, color: '#475569', display: 'flex', flexDirection: 'column', gap: 12 }}>
          <label style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span>ローカルエンジン処理モード</span>
            <span style={{ color: '#16a34a', fontWeight: 600 }}>高精度 (高速)</span>
          </label>
          <label style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span>バージョン</span>
            <span style={{ color: '#64748b' }}>v1.0.0 (Apple Silicon Optimized)</span>
          </label>
        </div>
        <button
          onClick={onClose}
          style={{
            background: '#2563eb',
            color: '#fff',
            border: 'none',
            borderRadius: 8,
            padding: '8px 18px',
            fontSize: 13,
            fontWeight: 600,
            cursor: 'pointer',
            alignSelf: 'flex-end',
          }}
        >
          完了
        </button>
      </div>
    </div>
  )
}
