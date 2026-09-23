import React, { useEffect, useRef, useState } from 'react'

// ========== 共通モーダルシェル ==========

interface ModalShellProps {
  onClose: () => void
  labelledBy: string
  children: React.ReactNode
}

/**
 * ネイティブ dialog 相当のオーバーレイシェル。
 * 背景クリックと Escape キーで閉じ、内容は中央カードで表示する。
 */
function ModalShell({ onClose, labelledBy, children }: ModalShellProps) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        onClose()
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [onClose])

  return (
    <div
      role="presentation"
      onClick={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(15, 23, 42, 0.45)',
        backdropFilter: 'blur(4px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1100,
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        onClick={(e) => e.stopPropagation()}
        style={{
          background: '#ffffff',
          borderRadius: 16,
          padding: '22px 24px',
          maxWidth: 440,
          width: '90%',
          boxShadow: '0 20px 40px rgba(0, 0, 0, 0.18)',
          display: 'flex',
          flexDirection: 'column',
          gap: 14,
        }}
      >
        {children}
      </div>
    </div>
  )
}

const primaryBtn = (danger: boolean): React.CSSProperties => ({
  padding: '9px 16px',
  borderRadius: 10,
  border: 'none',
  background: danger ? '#ef4444' : '#2563eb',
  color: '#ffffff',
  fontSize: 13,
  fontWeight: 600,
  cursor: 'pointer',
})

const ghostBtn: React.CSSProperties = {
  padding: '9px 16px',
  borderRadius: 10,
  border: '1px solid #cbd5e1',
  background: '#ffffff',
  color: '#0f172a',
  fontSize: 13,
  fontWeight: 600,
  cursor: 'pointer',
}

// ========== 確認ダイアログ ==========

export interface ConfirmDialogProps {
  isOpen: boolean
  title: string
  message: string
  confirmLabel?: string
  cancelLabel?: string
  /** 破壊的操作（削除など）の場合 true。確認ボタンが赤くなる。 */
  danger?: boolean
  onConfirm: () => void
  onCancel: () => void
}

/**
 * ネイティブ `confirm()` の代替。破壊的操作前の確認に使う。
 */
export function ConfirmDialog({
  isOpen,
  title,
  message,
  confirmLabel = 'OK',
  cancelLabel = 'キャンセル',
  danger = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const confirmRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (isOpen) confirmRef.current?.focus()
  }, [isOpen])

  if (!isOpen) return null

  return (
    <ModalShell onClose={onCancel} labelledBy="confirm-dialog-title">
      <h3 id="confirm-dialog-title" style={{ margin: 0, fontSize: 16, fontWeight: 700, color: '#0f172a' }}>
        {title}
      </h3>
      <p style={{ margin: 0, fontSize: 13, color: '#475569', lineHeight: 1.6 }}>{message}</p>
      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 10, marginTop: 4 }}>
        <button type="button" onClick={onCancel} style={ghostBtn}>
          {cancelLabel}
        </button>
        <button ref={confirmRef} type="button" onClick={onConfirm} style={primaryBtn(danger)}>
          {confirmLabel}
        </button>
      </div>
    </ModalShell>
  )
}

// ========== 入力ダイアログ ==========

export interface InputDialogProps {
  isOpen: boolean
  title: string
  message?: string
  initialValue?: string
  placeholder?: string
  confirmLabel?: string
  cancelLabel?: string
  /** true で複数行テキストエリア（コード入力など） */
  multiline?: boolean
  /** 単一行入力の input type。パスワードは 'password' でマスクする。 */
  inputType?: 'text' | 'password'
  onSubmit: (value: string) => void
  onCancel: () => void
}

/**
 * ネイティブ `prompt()` の代替。テキスト／コード入力に使う。
 * Ctrl/Cmd+Enter でも確定できる。
 */
export function InputDialog({
  isOpen,
  title,
  message,
  initialValue = '',
  placeholder,
  confirmLabel = 'OK',
  cancelLabel = 'キャンセル',
  multiline = false,
  inputType = 'text',
  onSubmit,
  onCancel,
}: InputDialogProps) {
  const [value, setValue] = useState(initialValue)
  const inputRef = useRef<HTMLTextAreaElement | HTMLInputElement | null>(null)

  useEffect(() => {
    if (isOpen) {
      setValue(initialValue)
      // 表示後にフォーカスを当てる
      requestAnimationFrame(() => inputRef.current?.focus())
    }
  }, [isOpen, initialValue])

  if (!isOpen) return null

  const submit = () => onSubmit(value)
  const onFieldKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault()
      submit()
    }
  }

  const fieldStyle: React.CSSProperties = {
    width: '100%',
    boxSizing: 'border-box',
    padding: '10px 12px',
    borderRadius: 10,
    border: '1px solid #cbd5e1',
    fontSize: 13,
    fontFamily: multiline ? 'ui-monospace, SFMono-Regular, Menlo, monospace' : 'inherit',
    resize: multiline ? 'vertical' : 'none',
    outline: 'none',
  }

  return (
    <ModalShell onClose={onCancel} labelledBy="input-dialog-title">
      <h3 id="input-dialog-title" style={{ margin: 0, fontSize: 16, fontWeight: 700, color: '#0f172a' }}>
        {title}
      </h3>
      {message && <p style={{ margin: 0, fontSize: 13, color: '#475569' }}>{message}</p>}
      {multiline ? (
        <textarea
          ref={(el) => { inputRef.current = el }}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={onFieldKeyDown}
          placeholder={placeholder}
          rows={6}
          style={fieldStyle}
        />
      ) : (
        <input
          ref={(el) => { inputRef.current = el }}
          type={inputType}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              submit()
            }
            onFieldKeyDown(e)
          }}
          placeholder={placeholder}
          style={fieldStyle}
        />
      )}
      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 10, marginTop: 4 }}>
        <button type="button" onClick={onCancel} style={ghostBtn}>
          {cancelLabel}
        </button>
        <button type="button" onClick={submit} style={primaryBtn(false)}>
          {confirmLabel}
        </button>
      </div>
    </ModalShell>
  )
}
