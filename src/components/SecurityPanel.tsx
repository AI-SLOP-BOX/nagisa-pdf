import { useState, useEffect } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import type { SignatureInfo, PdfExec, Pkcs11Slot } from '../types'
import { DocumentService } from '../services/documentService'
import { Input, AccentBtn } from './UIControls'

export type { SignatureInfo }

export function SecurityPanel({
  exec,
  showToast,
  onInspectSignatures,
  pdfData,
  docId,
  onPdfUpdate,
}: {
  exec: PdfExec
  showToast: (msg: string) => void
  onInspectSignatures?: (sigs: SignatureInfo[]) => void
  pdfData: number[] | null
  docId?: string | null
  onPdfUpdate?: (data: number[]) => void
}) {
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [signerName, setSignerName] = useState('')
  const [signReason, setSignReason] = useState('')

  // CMS / PKCS#12 signing states
  const [cmsSignerName, setCmsSignerName] = useState('')
  const [cmsReason, setCmsReason] = useState('承認済み')
  const [p12Path, setP12Path] = useState<string | null>(null)
  const [p12Password, setP12Password] = useState('')
  const [tsaUrl, setTsaUrl] = useState('')
  const [isSigning, setIsSigning] = useState(false)

  // OS keychain signing states: the private key never leaves the Secure Enclave
  // / keychain, so no file or password is ever handled.
  const [signSource, setSignSource] = useState<'keychain' | 'pkcs11' | 'p12'>('keychain')
  const [keychainIdentities, setKeychainIdentities] = useState<Array<{ sha1_fingerprint: string; common_name: string; nickname: string }>>([])
  const [selectedIdentity, setSelectedIdentity] = useState('')
  const [pkcs11Slots, setPkcs11Slots] = useState<Pkcs11Slot[]>([])
  const [selectedSlot, setSelectedSlot] = useState<number | null>(null)
  const [selectedCertificate, setSelectedCertificate] = useState('')
  const [pkcs11Pin, setPkcs11Pin] = useState('')
  const [pkcs11Loaded, setPkcs11Loaded] = useState(false)
  const [keychainLoaded, setKeychainLoaded] = useState(false)

  const loadKeychainIdentities = async () => {
    try {
      const identities = await invoke<Array<{ sha1_fingerprint: string; common_name: string; nickname: string }>>('list_keychain_identities')
      setKeychainIdentities(identities)
      setKeychainLoaded(true)
      if (identities.length === 0) {
        showToast('キーチェーンに署名用証明書が見つかりません')
      } else {
        setSelectedIdentity(prev => prev || identities[0].nickname)
      }
    } catch (err) {
      setKeychainLoaded(true)
      showToast(`証明書の読み込みに失敗しました: ${err}`)
    }
  }

  // Load the keychain certificates as soon as the panel is opened.
  useEffect(() => { void loadKeychainIdentities() }, [])

  const handleKeychainSign = async () => {
    if (!selectedIdentity) {
      showToast('使用する証明書を選択してください')
      return
    }
    const currentBytes = docId ? await DocumentService.getSessionBytes(docId) : pdfData
    if (!currentBytes || currentBytes.length === 0) {
      showToast('署名対象のPDFデータが見つかりません')
      return
    }
    try {
      setIsSigning(true)
      const signedBytes = await invoke<number[]>('sign_pdf_with_keychain', {
        data: currentBytes,
        pageIndex: 0,
        x: 50,
        y: 50,
        width: 200,
        height: 60,
        signerName: cmsSignerName || '署名者',
        reason: cmsReason || '承認',
        identityNickname: selectedIdentity,
        tsaUrl: tsaUrl.trim() ? tsaUrl.trim() : null,
      })
      await onPdfUpdate?.(signedBytes)
      showToast('キーチェーンの証明書で署名しました（秘密鍵は外部に抽出されません）')
    } catch (err) {
      showToast(`キーチェーン署名エラー: ${err}`)
    } finally {
      setIsSigning(false)
    }
  }

  const loadPkcs11Slots = async () => {
    try {
      const slots = await invoke<Pkcs11Slot[]>('list_pkcs11_slots')
      setPkcs11Slots(slots)
      setPkcs11Loaded(true)
      if (slots.length > 0) {
        setSelectedSlot(slots[0].slot_id)
        setSelectedCertificate(slots[0].certificates[0]?.certificate_id || '')
      } else {
        setSelectedSlot(null)
        setSelectedCertificate('')
      }
    } catch (err) {
      setPkcs11Loaded(true)
      showToast(`PKCS#11トークンの読み込みに失敗しました: ${err}`)
    }
  }

  useEffect(() => {
    if (signSource === 'pkcs11' && !pkcs11Loaded) void loadPkcs11Slots()
  }, [signSource, pkcs11Loaded])

  const handlePkcs11Sign = async () => {
    if (selectedSlot === null || !selectedCertificate || !pkcs11Pin) {
      showToast('トークン・証明書・PINを選択してください')
      return
    }
    const currentBytes = docId ? await DocumentService.getSessionBytes(docId) : pdfData
    if (!currentBytes?.length) {
      showToast('署名対象のPDFデータが見つかりません')
      return
    }
    try {
      setIsSigning(true)
      const signedBytes = await invoke<number[]>('sign_pdf_with_pkcs11', {
        data: currentBytes,
        pageIndex: 0,
        x: 50,
        y: 50,
        width: 200,
        height: 60,
        signerName: cmsSignerName || '署名者',
        reason: cmsReason || '承認',
        slotId: selectedSlot,
        certificateId: selectedCertificate,
        pin: pkcs11Pin,
        tsaUrl: tsaUrl.trim() || null,
      })
      await onPdfUpdate?.(signedBytes)
      showToast('PKCS#11トークンで署名しました（秘密鍵はHSM内に保持されます）')
    } catch (err) {
      showToast(`PKCS#11署名エラー: ${err}`)
    } finally {
      setPkcs11Pin('')
      setIsSigning(false)
    }
  }

  const handleSelectP12 = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'PKCS#12 証明書', extensions: ['p12', 'pfx'] }],
      })
      if (typeof selected === 'string') {
        setP12Path(selected)
        showToast('PKCS#12 証明書ファイルを選択しました')
      }
    } catch (err) {
      showToast(`ファイル選択エラー: ${err}`)
    }
  }

  const handleCmsSign = async () => {
    if (!p12Path) {
      showToast('.p12 または .pfx 証明書ファイルを選択してください')
      return
    }
    const currentBytes = docId ? await DocumentService.getSessionBytes(docId) : pdfData
    if (!currentBytes || currentBytes.length === 0) {
      showToast('署名対象のPDFデータが見つかりません')
      return
    }

    try {
      setIsSigning(true)
      const p12Bytes = await invoke<number[]>('read_file_bytes', { path: p12Path })
      const signedBytes = await DocumentService.signPdfCms({
        data: currentBytes,
        pageIndex: 0,
        x: 50,
        y: 50,
        width: 200,
        height: 60,
        signerName: cmsSignerName || '署名者',
        reason: cmsReason || '承認',
        p12Data: p12Bytes,
        p12Password: p12Password,
        tsaUrl: tsaUrl.trim() ? tsaUrl.trim() : undefined,
      })

      if (onPdfUpdate) {
        onPdfUpdate(signedBytes)
      } else {
        await exec('update_pdf', { data: signedBytes })
      }
      showToast('CMS暗号署名（PAdES互換）を付与しました')
      setP12Path(null)
      setP12Password('')
    } catch (err) {
      showToast(`署名失敗: ${err}`)
    } finally {
      setIsSigning(false)
    }
  }

  const handleVerify = async () => {
    const target = docId || pdfData
    if (!target) return
    try {
      const result = await DocumentService.verifySignatures(target)
      if (result && result.signatures) {
        onInspectSignatures?.(result.signatures)
        showToast(`${result.signatures.length}件の署名を検証しました`)
      } else {
        onInspectSignatures?.([])
        showToast('署名は検出されませんでした')
      }
    } catch (err) {
      showToast(`検証エラー: ${err}`)
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      {/* Card 1: Password Encryption */}
      <div className="inspector-card">
        <div className="inspector-card-header">
          <span>パスワード暗号化</span>
          <span style={{ fontSize: 9, color: 'var(--red)', fontWeight: 600 }}>AES-128</span>
        </div>
        <div className="inspector-card-desc">PDF閲覧にパスワード保護を設定</div>
        <input
          type="password"
          value={password}
          onChange={e => setPassword(e.target.value)}
          placeholder="パスワードを入力"
          style={{
            width: '100%', padding: '6px 10px', background: 'var(--bg-0)',
            border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)',
            color: 'var(--text)', fontSize: 12, marginBottom: 6,
            boxShadow: 'inset 0 1px 3px rgba(0,0,0,0.25)'
          }}
        />
        <input
          type="password"
          value={confirmPassword}
          onChange={e => setConfirmPassword(e.target.value)}
          placeholder="確認のため再入力"
          style={{
            width: '100%', padding: '6px 10px', background: 'var(--bg-0)',
            border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)',
            color: 'var(--text)', fontSize: 12, marginBottom: 6,
            boxShadow: 'inset 0 1px 3px rgba(0,0,0,0.25)'
          }}
        />
        <AccentBtn
          onClick={() => {
            if (password !== confirmPassword) {
              showToast('パスワードが一致しません')
              return
            }
            exec('protect_pdf', { password })
          }}
          disabled={!password || password !== confirmPassword}
        >
          暗号化を実行
        </AccentBtn>
      </div>

      {/* Card 2: CMS/PKCS#12 Cryptographic Signature (Acrobat Pro Parity) */}
      <div className="inspector-card">
        <div className="inspector-card-header">
          <span>電子署名 (CMS / PAdES)</span>
          <span style={{ fontSize: 9, color: '#00ff88', fontWeight: 600 }}>LTV</span>
        </div>
        <div className="inspector-card-desc">暗号ハッシュによる公式デジタル署名。秘密鍵はキーチェーンまたはPKCS#11/HSM内に留まります。</div>

        {/* Credential source switch */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 6, marginBottom: 8 }}>
          {([['keychain', 'キーチェーン'], ['pkcs11', 'PKCS#11'], ['p12', 'PKCS#12 ファイル']] as const).map(([key, label]) => (
            <button
              key={key}
              onClick={() => setSignSource(key)}
              style={{
                padding: '6px 8px', fontSize: 11, fontWeight: 600, cursor: 'pointer',
                background: signSource === key ? 'var(--accent)' : 'var(--bg-2)',
                color: signSource === key ? '#fff' : 'var(--text)',
                border: `1px solid ${signSource === key ? 'transparent' : 'var(--border)'}`,
                borderRadius: 'var(--radius-sm)',
              }}
            >
              {label}
            </button>
          ))}
        </div>

        <Input value={cmsSignerName} onChange={setCmsSignerName} placeholder="署名者氏名 (例: 渚 太郎)" />
        <Input value={cmsReason} onChange={setCmsReason} placeholder="署名理由 (例: 最終承認)" />

        {signSource === 'keychain' ? (
          <div style={{ marginBottom: 6 }}>
            <select
              value={selectedIdentity}
              onChange={e => setSelectedIdentity(e.target.value)}
              style={{ width: '100%', padding: '6px 8px', background: 'var(--bg-0)', border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)', color: 'var(--text)', fontSize: 11, marginBottom: 6 }}
            >
              {keychainIdentities.length === 0 && <option value="">{keychainLoaded ? '証明書がありません' : '読み込み中...'}</option>}
              {keychainIdentities.map(identity => <option key={identity.sha1_fingerprint} value={identity.nickname}>{identity.common_name}</option>)}
            </select>
            <button onClick={loadKeychainIdentities} style={{ width: '100%', padding: '4px 8px', fontSize: 10, background: 'transparent', border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)', color: 'var(--text-muted)', cursor: 'pointer' }}>証明書を再読み込み</button>
          </div>
        ) : signSource === 'pkcs11' ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginBottom: 6 }}>
            <select
              aria-label="PKCS#11トークン"
              value={selectedSlot ?? ''}
              onChange={e => {
                const id = Number(e.target.value)
                const slot = pkcs11Slots.find(item => item.slot_id === id)
                setSelectedSlot(id)
                setSelectedCertificate(slot?.certificates[0]?.certificate_id || '')
              }}
              style={{ width: '100%', padding: '6px 8px', background: 'var(--bg-0)', border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)', color: 'var(--text)', fontSize: 11 }}
            >
              {pkcs11Slots.length === 0 && <option value="">{pkcs11Loaded ? '署名可能トークンがありません' : 'トークンを検索中...'}</option>}
              {pkcs11Slots.map(slot => <option key={slot.slot_id} value={slot.slot_id}>{slot.token_label || slot.description} ({slot.token_serial})</option>)}
            </select>
            <select
              aria-label="PKCS#11署名証明書"
              value={selectedCertificate}
              onChange={e => setSelectedCertificate(e.target.value)}
              style={{ width: '100%', padding: '6px 8px', background: 'var(--bg-0)', border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)', color: 'var(--text)', fontSize: 11 }}
            >
              {pkcs11Slots.find(slot => slot.slot_id === selectedSlot)?.certificates.map(cert => (
                <option key={cert.certificate_id} value={cert.certificate_id}>{cert.label} — {cert.subject || cert.sha256_fingerprint}</option>
              ))}
            </select>
            <input type="password" aria-label="PKCS#11 PIN" value={pkcs11Pin} onChange={e => setPkcs11Pin(e.target.value)} placeholder="トークンPIN" autoComplete="off" style={{ width: '100%', padding: '6px 10px', background: 'var(--bg-0)', border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)', color: 'var(--text)', fontSize: 12, boxShadow: 'inset 0 1px 3px rgba(0,0,0,0.25)' }} />
            <button onClick={() => void loadPkcs11Slots()} style={{ padding: '4px 8px', fontSize: 10, background: 'transparent', border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)', color: 'var(--text-muted)', cursor: 'pointer' }}>トークンを再読み込み</button>
          </div>
        ) : (
          <>
            <div style={{ display: 'flex', gap: 6, marginBottom: 6 }}>
              <button onClick={handleSelectP12} style={{ flex: 1, padding: '6px 8px', fontSize: 11, background: 'var(--bg-0)', border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)', color: 'var(--text)', cursor: 'pointer', textAlign: 'left', textOverflow: 'ellipsis', overflow: 'hidden', whiteSpace: 'nowrap' }}>
                {p12Path ? p12Path.split('/').pop() : '📁 証明書ファイル (.p12/.pfx) を選択'}
              </button>
            </div>
            {p12Path && <input type="password" value={p12Password} onChange={e => setP12Password(e.target.value)} placeholder="証明書のパスワード (空欄可)" style={{ width: '100%', padding: '6px 10px', background: 'var(--bg-0)', border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)', color: 'var(--text)', fontSize: 12, marginBottom: 6, boxShadow: 'inset 0 1px 3px rgba(0,0,0,0.25)' }} />}
          </>
        )}

        <Input value={tsaUrl} onChange={setTsaUrl} placeholder="RFC 3161 TSAタイムスタンプURL (任意)" />
        {signSource === 'keychain' ? (
          <AccentBtn onClick={handleKeychainSign} disabled={!selectedIdentity || isSigning} style={{ marginTop: 4, background: '#1f6feb', color: '#fff' }}>
            {isSigning ? '署名処理中...' : 'キーチェーンで署名'}
          </AccentBtn>
        ) : signSource === 'pkcs11' ? (
          <AccentBtn onClick={handlePkcs11Sign} disabled={selectedSlot === null || !selectedCertificate || !pkcs11Pin || isSigning} style={{ marginTop: 4, background: '#1f6feb', color: '#fff' }}>
            {isSigning ? 'トークン署名処理中...' : 'PKCS#11 / HSMで署名'}
          </AccentBtn>
        ) : (
          <AccentBtn onClick={handleCmsSign} disabled={!p12Path || isSigning} style={{ marginTop: 4, background: '#1f6feb', color: '#fff' }}>
            {isSigning ? '署名処理中...' : '証明書ファイルで署名'}
          </AccentBtn>
        )}
      </div>

      {/* Card 3: Digital Signature Fields */}
      <div className="inspector-card">
        <div className="inspector-card-header">
          <span>署名検証 & フィールド枠</span>
          <span style={{ fontSize: 9, color: 'var(--text-muted)', fontWeight: 600 }}>FIELD</span>
        </div>
        <div className="inspector-card-desc">署名用ウィジェット枠・理由メタデータを配置</div>
        <Input value={signerName} onChange={setSignerName} placeholder="署名予定者名 (例: Taro Yamada)" />
        <Input value={signReason} onChange={setSignReason} placeholder="署名理由 (例: 承認済み)" />
        <AccentBtn
          onClick={() => exec('add_digital_signature', { pageIndex: 0, x: 400, y: 50, width: 150, height: 60, signerName: signerName, reason: signReason })}
          disabled={!signerName}
          style={{ marginTop: 6 }}
        >
          署名フィールド枠を追加
        </AccentBtn>
        <AccentBtn
          onClick={handleVerify}
          style={{ marginTop: 6, background: 'rgba(46, 160, 67, 0.15)', color: '#2ea043', border: '1px solid rgba(46, 160, 67, 0.4)' }}
        >
          署名検証インスペクターを開く
        </AccentBtn>
      </div>
    </div>
  )
}
