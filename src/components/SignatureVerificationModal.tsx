import { SignatureInfo } from '../types'
import { ShieldCheckIcon, CloseIcon, CheckIcon } from './Icons'
import { useT } from '../utils/i18n'

interface SignatureVerificationModalProps {
  signatures: SignatureInfo[]
  onClose: () => void
}

export function SignatureVerificationModal({ signatures, onClose }: SignatureVerificationModalProps) {
  const { t } = useT()
  return (
    <div style={{
      position: 'fixed', top: 0, left: 0, right: 0, bottom: 0,
      background: 'rgba(0,0,0,0.65)', backdropFilter: 'blur(4px)',
      display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 2000,
    }}>
      <div style={{
        background: 'var(--bg-1)', border: '1px solid var(--border)',
        borderRadius: 8, padding: 20, width: 440, maxWidth: '90vw',
        boxShadow: '0 16px 40px rgba(0,0,0,0.5)',
      }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 14, fontWeight: 700, color: 'var(--text)' }}>
            <ShieldCheckIcon size={16} color="var(--accent)" />
            デジタル署名・証明書の検証詳細
          </div>
          <button
            onClick={onClose}
            style={{ background: 'transparent', border: 'none', color: 'var(--text-muted)', cursor: 'pointer' }}
          >
            <CloseIcon size={16} />
          </button>
        </div>
        {signatures.length === 0 ? (
          <p style={{ fontSize: 12, color: 'var(--text-muted)', margin: '16px 0' }}>
            {t().noSignatures}
          </p>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10, maxHeight: 300, overflowY: 'auto' }}>
            {signatures.map((sig, i) => {
              const isCryptoVerified = sig.status === 'valid' && sig.integrity_verified
              return (
                <div key={i} style={{
                  padding: 10, borderRadius: 6, background: 'var(--bg-0)',
                  border: `1px solid ${isCryptoVerified ? 'rgba(0,255,100,0.3)' : 'rgba(255,180,0,0.3)'}`, fontSize: 12,
                }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                    <span style={{
                      display: 'inline-flex', alignItems: 'center', gap: 4,
                      color: isCryptoVerified ? '#00ff88' : '#e3b341', fontWeight: 700
                    }}>
                      <CheckIcon size={13} color={isCryptoVerified ? '#00ff88' : '#e3b341'} />
                      {isCryptoVerified ? '検証済みの署名' : '署名フィールド検出 (暗号ダイジェスト未検証)'}
                    </span>
                    <span style={{ color: 'var(--text-muted)', fontSize: 11 }}>({sig.name || `Signature ${i + 1}`})</span>
                  </div>
                  <div style={{ color: 'var(--text)', fontSize: 12, lineHeight: 1.6 }}>
                    <div><b>署名者:</b> {sig.signer || '未指定'}</div>
                    <div><b>署名理由:</b> {sig.reason || '未指定'}</div>
                    <div><b>署名日時:</b> {sig.timestamp}</div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginTop: 2 }}>
                      <b>信頼性検証:</b>
                      <span style={{
                        fontSize: 10,
                        padding: '1px 6px',
                        borderRadius: 3,
                        background: isCryptoVerified ? 'rgba(0, 255, 136, 0.15)' : 'rgba(255, 180, 0, 0.15)',
                        color: isCryptoVerified ? '#00ff88' : '#e3b341',
                        border: `1px solid ${isCryptoVerified ? 'rgba(0, 255, 136, 0.4)' : 'rgba(255, 180, 0, 0.4)'}`,
                        fontWeight: 600
                      }}>
                        {sig.trust_level || '構造検出'}
                      </span>
                    </div>
                    {sig.certificate_issuer && (
                      <div><b>発行認証局:</b> <span style={{ color: 'var(--text-muted)' }}>{sig.certificate_issuer}</span></div>
                    )}
                    {sig.digest_algorithm && (
                      <div><b>暗号ダイジェスト:</b> <span style={{ color: 'var(--text-muted)' }}>{sig.digest_algorithm} {sig.digest_matches ? '(整合性確認済)' : '(不一致)'}</span></div>
                    )}
                    {sig.tsa_subject && (
                      <div><b>RFC 3161 タイムスタンプ:</b> <span style={{ color: '#00ff88' }}>{sig.tsa_subject} {sig.imprint_matches ? '(検証済)' : ''}</span></div>
                    )}
                    {sig.chain_details && sig.chain_valid === true && (
                      <div><b>証明書チェーン:</b> <span style={{ color: 'var(--text-muted)' }}>{sig.chain_details}</span></div>
                    )}
                    {sig.chain_details && sig.chain_valid === false && (
                      <div><b>証明書チェーン:</b> <span style={{ color: '#e3b341' }}>{sig.chain_details}</span></div>
                    )}
                    {sig.revocation_status && (
                      <div><b>失効状態:</b> <span style={{ color: sig.revocation_status === '未確認' ? '#e3b341' : 'var(--text-muted)' }}>{sig.revocation_status}</span></div>
                    )}
                    {sig.revocation_details && (
                      <div style={{ color: 'var(--text-muted)', fontSize: 11, marginLeft: 8 }}>{sig.revocation_details}</div>
                    )}
                    {sig.notice && (
                      <div style={{
                        marginTop: 6,
                        padding: '4px 8px',
                        borderRadius: 4,
                        background: 'rgba(255, 180, 0, 0.08)',
                        border: '1px dashed rgba(255, 180, 0, 0.3)',
                        fontSize: 11,
                        color: 'var(--text-muted)',
                        lineHeight: 1.4
                      }}>
                        ℹ️ {sig.notice}
                      </div>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        )}
        <button
          onClick={onClose}
          style={{
            marginTop: 16, width: '100%', padding: '8px 16px',
            background: 'var(--accent)', color: '#ffffff',
            border: 'none', borderRadius: 4, fontWeight: 600, cursor: 'pointer',
          }}
        >
          {t().close}
        </button>
      </div>
    </div>
  )
}
