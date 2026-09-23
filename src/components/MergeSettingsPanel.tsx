import React from 'react'
import { ExportIcon, CheckIcon } from './Icons'

interface MergeSettingsPanelProps {
  outputName: string
  setOutputName: (val: string) => void
  savePath: string
  setSavePath?: (val: string) => void
  keepBookmarks: boolean
  setKeepBookmarks: (val: boolean) => void
  handlePassword: boolean
  setHandlePassword: (val: boolean) => void
  insertSeparator: boolean
  setInsertSeparator: (val: boolean) => void
  separatorText: string
  setSeparatorText: (val: string) => void
  isMerging: boolean
  mergeSuccess: boolean
  onStartMerge: () => void
}

export const MergeSettingsPanel: React.FC<MergeSettingsPanelProps> = ({
  outputName,
  setOutputName,
  savePath,
  keepBookmarks,
  setKeepBookmarks,
  handlePassword,
  setHandlePassword,
  insertSeparator,
  setInsertSeparator,
  separatorText,
  setSeparatorText,
  isMerging,
  mergeSuccess,
  onStartMerge,
}) => {
  return (
    <div
      style={{
        width: 320,
        background: '#ffffff',
        borderRadius: 14,
        border: '1px solid #e2e8f0',
        padding: 20,
        display: 'flex',
        flexDirection: 'column',
        gap: 18,
        boxShadow: '0 2px 8px rgba(0,0,0,0.03)',
        boxSizing: 'border-box',
        flexShrink: 0,
      }}
    >
      <span style={{ fontSize: 15, fontWeight: 700, color: '#0f172a' }}>出力設定</span>

      {/* Filename */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <label style={{ fontSize: 12, fontWeight: 600, color: '#475569' }}>出力ファイル名</label>
        <input
          type="text"
          value={outputName}
          onChange={e => setOutputName(e.target.value)}
          style={{
            width: '100%',
            height: 38,
            borderRadius: 8,
            border: '1px solid #cbd5e1',
            padding: '0 12px',
            fontSize: 12.5,
            color: '#1e293b',
            background: '#f8fafc',
            boxSizing: 'border-box',
          }}
        />
      </div>

      {/* Save Path */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <label style={{ fontSize: 12, fontWeight: 600, color: '#475569' }}>保存先</label>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <div
            style={{
              flex: 1,
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              height: 36,
              background: '#f8fafc',
              border: '1px solid #cbd5e1',
              borderRadius: 7,
              padding: '0 10px',
              fontSize: 11.5,
              color: '#475569',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            <span>📁</span>
            <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{savePath}</span>
          </div>
          <button
            style={{
              height: 36,
              padding: '0 12px',
              background: '#eff6ff',
              color: '#2563eb',
              border: '1px solid #bfdbfe',
              borderRadius: 7,
              fontSize: 12,
              fontWeight: 600,
              cursor: 'pointer',
              flexShrink: 0,
            }}
          >
            変更
          </button>
        </div>
      </div>

      {/* Options with iOS-style Toggles */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 14, paddingTop: 6, borderTop: '1px solid #f1f5f9' }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#475569' }}>オプション</span>

        {/* Toggle 1 */}
        <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 10 }}>
          <div style={{ display: 'flex', flexDirection: 'column' }}>
            <span style={{ fontSize: 12.5, fontWeight: 600, color: '#1e293b' }}>しおりを保持</span>
            <span style={{ fontSize: 11, color: '#64748b' }}>各PDFのしおりを結合後も保持します。</span>
          </div>
          <button
            onClick={() => setKeepBookmarks(!keepBookmarks)}
            style={{
              width: 40,
              height: 22,
              borderRadius: 11,
              background: keepBookmarks ? '#2563eb' : '#cbd5e1',
              border: 'none',
              position: 'relative',
              cursor: 'pointer',
              padding: 2,
              transition: 'background-color 0.2s',
              flexShrink: 0,
            }}
          >
            <div
              style={{
                width: 18,
                height: 18,
                borderRadius: '50%',
                background: '#ffffff',
                transform: keepBookmarks ? 'translateX(18px)' : 'translateX(0)',
                transition: 'transform 0.2s',
                boxShadow: '0 1px 3px rgba(0,0,0,0.2)',
              }}
            />
          </button>
        </div>

        {/* Toggle 2 */}
        <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 10 }}>
          <div style={{ display: 'flex', flexDirection: 'column' }}>
            <span style={{ fontSize: 12.5, fontWeight: 600, color: '#1e293b' }}>パスワード付きPDFの扱い</span>
            <span style={{ fontSize: 11, color: '#64748b' }}>パスワードを入力して結合します。</span>
          </div>
          <button
            onClick={() => setHandlePassword(!handlePassword)}
            style={{
              width: 40,
              height: 22,
              borderRadius: 11,
              background: handlePassword ? '#2563eb' : '#cbd5e1',
              border: 'none',
              position: 'relative',
              cursor: 'pointer',
              padding: 2,
              transition: 'background-color 0.2s',
              flexShrink: 0,
            }}
          >
            <div
              style={{
                width: 18,
                height: 18,
                borderRadius: '50%',
                background: '#ffffff',
                transform: handlePassword ? 'translateX(18px)' : 'translateX(0)',
                transition: 'transform 0.2s',
                boxShadow: '0 1px 3px rgba(0,0,0,0.2)',
              }}
            />
          </button>
        </div>

        {/* Toggle 3 */}
        <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 10 }}>
          <div style={{ display: 'flex', flexDirection: 'column' }}>
            <span style={{ fontSize: 12.5, fontWeight: 600, color: '#1e293b' }}>ページ区切りを挿入</span>
            <span style={{ fontSize: 11, color: '#64748b' }}>各ファイルの間に区切りページを挿入します。</span>
          </div>
          <button
            onClick={() => setInsertSeparator(!insertSeparator)}
            style={{
              width: 40,
              height: 22,
              borderRadius: 11,
              background: insertSeparator ? '#2563eb' : '#cbd5e1',
              border: 'none',
              position: 'relative',
              cursor: 'pointer',
              padding: 2,
              transition: 'background-color 0.2s',
              flexShrink: 0,
            }}
          >
            <div
              style={{
                width: 18,
                height: 18,
                borderRadius: '50%',
                background: '#ffffff',
                transform: insertSeparator ? 'translateX(18px)' : 'translateX(0)',
                transition: 'transform 0.2s',
                boxShadow: '0 1px 3px rgba(0,0,0,0.2)',
              }}
            />
          </button>
        </div>

        {insertSeparator && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <span style={{ fontSize: 11, color: '#64748b' }}>区切りページのテキスト (任意)</span>
            <input
              type="text"
              placeholder="ここにテキストを入力..."
              value={separatorText}
              onChange={e => setSeparatorText(e.target.value)}
              style={{
                height: 32,
                borderRadius: 6,
                border: '1px solid #cbd5e1',
                padding: '0 10px',
                fontSize: 12,
              }}
            />
          </div>
        )}
      </div>

      {/* Action Button */}
      <div style={{ marginTop: 'auto', paddingTop: 12 }}>
        <button
          onClick={onStartMerge}
          disabled={isMerging}
          style={{
            width: '100%',
            height: 42,
            borderRadius: 9,
            border: 'none',
            background: mergeSuccess ? '#16a34a' : '#2563eb',
            color: '#ffffff',
            fontSize: 14,
            fontWeight: 700,
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 8,
            boxShadow: '0 4px 14px rgba(37, 99, 235, 0.3)',
            transition: 'all 0.15s ease',
          }}
        >
          {isMerging ? (
            <span>結合処理中...</span>
          ) : mergeSuccess ? (
            <>
              <CheckIcon size={18} color="#fff" />
              <span>結合完了！</span>
            </>
          ) : (
            <>
              <ExportIcon size={18} color="#fff" />
              <span>結合して保存</span>
            </>
          )}
        </button>
      </div>
    </div>
  )
}
