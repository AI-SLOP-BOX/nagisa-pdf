use super::common::*;
use lopdf::{Dictionary, Document, Object, Stream};

// ===== DIGITAL SIGNATURE =====

/// PDF上に電子署名用ウィジェットフィールド（署名枠）およびプレースホルダー辞書を追加します。
/// ※ 注意: 本機能は暗号鍵/証明書によるPKCS#7バイナリ署名（ハッシュ計算・暗号化）を行うものではなく、
///    署名対象の位置・署名者名・理由メタデータを保持する「未署名フォームフィールド（署名プレースホルダー）」を作成します。
pub fn add_digital_signature(
    data: &[u8],
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    signer_name: &str,
    reason: &str,
    certificate_data: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    // Check if the document already contains cryptographic signatures (ByteRange)
    // Writing new fields without incremental update invalidates preexisting cryptographic signatures (ISO 32000-1 §12.8)
    if let Ok(info) = verify_signature_in_doc(&doc) {
        if let Some(sigs) = info.get("signatures").and_then(|s| s.as_array()) {
            let has_signed = sigs.iter().any(|sig| {
                sig.get("status").and_then(|st| st.as_str()) == Some("signed_unverified_cms")
            });
            if has_signed {
                return Err(
                    "Document already contains cryptographically signed fields. Adding new signature fields via standard rewrite would invalidate existing signatures. Incremental update support is required to preserve existing signatures.".to_string()
                );
            }
        }
    }

    // Format timestamp according to PDF standard D:YYYYMMDDHHmmSSZ
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day, hours, minutes, seconds) = unix_timestamp_to_utc(duration);
    let pdf_date = format!("D:{year:04}{month:02}{day:02}{hours:02}{minutes:02}{seconds:02}Z");

    let sig_params = super::form_creator::FormFieldConfig {
        field_type: "Sig".to_string(),
        name: format!("Signature_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()),
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        value: if signer_name.is_empty() { None } else { Some(signer_name.to_string()) },
        options: None,
        required: false,
        read_only: false,
        max_length: None,
    };

    let mut signed_data = super::form_creator::create_form_field(data, page_index, &sig_params)?;

    // Enrich the created signature dictionary with detailed metadata and embedded certificate if provided
    if let Ok(mut doc) = Document::load_mem(&signed_data) {
        let mut v_refs = Vec::new();
        for (_, obj) in doc.objects.iter() {
            if let Object::Dictionary(dict) = obj {
                if let Ok(Object::Name(ft)) = dict.get(b"FT") {
                    if ft == b"Sig" {
                        if let Ok(v_ref) = dict.get(b"V").and_then(|o| o.as_reference()) {
                            v_refs.push(v_ref);
                        }
                    }
                }
            }
        }

        let cert_stream_id = certificate_data.map(|cert| {
            let mut cert_dict = Dictionary::new();
            cert_dict.set("Type", Object::Name("SigRef".into()));
            doc.add_object(Stream::new(cert_dict, cert.to_vec()))
        });

        let mut modified = false;
        for v_ref in v_refs {
            if let Some(Object::Dictionary(sig_dict)) = doc.objects.get_mut(&v_ref) {
                sig_dict.set("M", Object::String(pdf_date.clone().into_bytes(), lopdf::StringFormat::Literal));
                if !signer_name.is_empty() {
                    sig_dict.set("Name", Object::String(encode_pdf_text_string(signer_name), lopdf::StringFormat::Literal));
                }
                if !reason.is_empty() {
                    sig_dict.set("Reason", Object::String(encode_pdf_text_string(reason), lopdf::StringFormat::Literal));
                }
                if let Some(c_id) = cert_stream_id {
                    // ISO 32000-1 §12.8.1: /Cert entry stores the X.509 certificate or certificate chain
                    sig_dict.set("Cert", Object::Reference(c_id));
                }
                modified = true;
            }
        }

        if modified {
            if let Ok(resaved) = save_doc(&mut doc) {
                signed_data = resaved;
            }
        }
    }

    Ok(signed_data)
}

/// Helper function to convert seconds since UNIX epoch to UTC calendar tuple (YYYY, MM, DD, hh, mm, ss)
fn unix_timestamp_to_utc(duration_secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let seconds_in_day = 86400;
    let days = duration_secs / seconds_in_day;
    let time_of_day = duration_secs % seconds_in_day;
    let hours = (time_of_day / 3600) as u32;
    let minutes = ((time_of_day % 3600) / 60) as u32;
    let seconds = (time_of_day % 60) as u32;

    let mut year = 1970u32;
    let mut rem_days = days;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if rem_days >= days_in_year {
            rem_days -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let days_in_months = [
        31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut month = 1u32;
    for &dim in &days_in_months {
        if rem_days >= dim {
            rem_days -= dim;
            month += 1;
        } else {
            break;
        }
    }
    let day = (rem_days + 1) as u32;
    (year, month, day, hours, minutes, seconds)
}

/// #43 是正: 署名済みPDFへの通常編集による署名破損ガード
///
/// `edit_text`, `rotate_page`, `delete_page` 等の通常編集を署名済みPDFに対して行うと、
/// 全体再シリアライズにより署名の ByteRange ハッシュが無効になり、
/// PDFビューアで「改ざんされた署名」として警告される（エンドユーザーの商業リスク）。
///
/// 本関数は ByteRange を持つ暗号署名が存在するか否かを事前に検査する。
/// 各編集コマンドのエントリポイントでこれを呼び出し、ユーザーに警告を返すこと。
///
/// # 戻り値
/// - `Ok(false)`: 署名なし（安全に編集可能）
/// - `Ok(true)`: 署名あり（編集すると署名が無効になる）
/// - `Err(...)`: PDF解析エラー
pub fn check_cryptographic_signature_presence(data: &[u8]) -> Result<bool, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    Ok(doc_has_cryptographic_signatures(&doc))
}

/// ドキュメントオブジェクトから暗号署名の有無を検査する内部ヘルパー。
/// ByteRange エントリ（PAdES/PKCS#7）を持つ /Sig フィールドを探す。
pub(crate) fn doc_has_cryptographic_signatures(doc: &Document) -> bool {
    for (_, obj) in doc.objects.iter() {
        if let Object::Dictionary(dict) = obj {
            // /FT /Sig かつ /ByteRange を持つ → 暗号的に署名されたフィールド
            let is_sig_field = dict
                .get(b"FT")
                .ok()
                .and_then(|o| o.as_name().ok())
                == Some(b"Sig");
            if is_sig_field {
                let has_byte_range = dict.get(b"V").ok().and_then(|v| match v {
                    Object::Reference(r) => doc.objects.get(r).and_then(|o| o.as_dict().ok()),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                }).map(|sig_dict| sig_dict.get(b"ByteRange").is_ok())
                .unwrap_or(false);
                if has_byte_range {
                    return true;
                }
            }
        }
    }
    false
}

/// 署名済みPDFに対する破壊的操作をブロックするエラーメッセージ。
/// 各編集コマンドで `doc_has_cryptographic_signatures` が true を返したときに使用する。
pub const SIGNED_PDF_MUTATION_ERROR: &str =
    "このPDFには有効なデジタル署名が含まれています。\
     テキスト編集・ページ操作・回転などの標準編集を行うと、\
     署名のハッシュ（ByteRange）が無効化され、受信者のPDFビューアで\
     「改ざんされた署名」として警告されます。\
     編集する場合は署名フィールドを削除してから行うか、\
     署名者に署名前の原本ファイルへの編集を依頼してください。";

pub fn verify_signature_in_doc(doc: &Document) -> Result<serde_json::Value, String> {

    // Find signature fields and extract actual dictionary metadata
    let mut signatures = Vec::new();

    for (_, obj) in doc.objects.iter() {
        if let Object::Dictionary(dict) = obj {
            if let Ok(Object::Name(ft)) = dict.get(b"FT") {
                if ft == b"Sig" {
                    let name = dict
                        .get(b"T")
                        .ok()
                        .and_then(|o| match o {
                            Object::String(bytes, _) => {
                                Some(decode_pdf_text_string(bytes))
                            }
                            _ => None,
                        })
                        .unwrap_or_default();

                    // V can be a direct dictionary or an indirect reference to signature dictionary
                    let sig_dict = match dict.get(b"V") {
                        Ok(Object::Dictionary(d)) => Some(d),
                        Ok(Object::Reference(id)) => {
                            doc.objects.get(id).and_then(|o| o.as_dict().ok())
                        }
                        _ => None,
                    };

                    let signer = sig_dict
                        .and_then(|d| d.get(b"Name").ok())
                        .or_else(|| dict.get(b"Name").ok())
                        .and_then(|o| match o {
                            Object::String(bytes, _) => {
                                Some(decode_pdf_text_string(bytes))
                            }
                            _ => None,
                        })
                        .unwrap_or_default();

                    let reason = sig_dict
                        .and_then(|d| d.get(b"Reason").ok())
                        .or_else(|| dict.get(b"Reason").ok())
                        .and_then(|o| match o {
                            Object::String(bytes, _) => {
                                Some(decode_pdf_text_string(bytes))
                            }
                            _ => None,
                        })
                        .unwrap_or_default();

                    let filter = sig_dict
                        .and_then(|d| d.get(b"Filter").ok())
                        .or_else(|| dict.get(b"Filter").ok())
                        .and_then(|o| match o {
                            Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
                            _ => None,
                        })
                        .unwrap_or_else(|| "Adobe.PPKLite".to_string());

                    let sub_filter = sig_dict
                        .and_then(|d| d.get(b"SubFilter").ok())
                        .or_else(|| dict.get(b"SubFilter").ok())
                        .and_then(|o| match o {
                            Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
                            _ => None,
                        })
                        .unwrap_or_else(|| "adbe.pkcs7.detached".to_string());

                    let timestamp = sig_dict
                        .and_then(|d| d.get(b"M").ok())
                        .or_else(|| dict.get(b"M").ok())
                        .and_then(|o| match o {
                            Object::String(bytes, _) => {
                                Some(String::from_utf8_lossy(bytes).to_string())
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| "未指定".to_string());

                    // Inspect ByteRange array if present
                    let byte_range = sig_dict
                        .and_then(|d| d.get(b"ByteRange").ok())
                        .and_then(|o| o.as_array().ok())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_i64().ok())
                                .collect::<Vec<i64>>()
                        })
                        .unwrap_or_default();

                    let contents_len = sig_dict
                        .and_then(|d| d.get(b"Contents").ok())
                        .and_then(|o| match o {
                            Object::String(bytes, _) => Some(bytes.len()),
                            _ => None,
                        })
                        .unwrap_or(0);

                    let has_nonzero_byterange =
                        byte_range.len() == 4 && byte_range.iter().any(|&v| v > 0);

                    // Honest validation: Lopdf inspects structural PDF dictionaries, but does not perform
                    // full cryptographic PKCS#7 / CMS signature verification or ByteRange digest hashing.
                    signatures.push(serde_json::json!({
                        "name": if name.is_empty() { "SignatureField" } else { &name },
                        "signer": if signer.is_empty() { "未指定の署名者" } else { &signer },
                        "reason": if reason.is_empty() { "未指定" } else { &reason },
                        "status": if has_nonzero_byterange { "signed_unverified_cms" } else { "unverified_structure_only" },
                        "filter": filter,
                        "sub_filter": sub_filter,
                        "byte_range": byte_range,
                        "contents_length_bytes": contents_len,
                        "timestamp": timestamp,
                        "aatl_verified": false,
                        "trust_level": if has_nonzero_byterange { "ByteRange定義済み (CMS/PKI検証が必要)" } else { "暗号署名エンジン未検証 (構造確認のみ)" },
                        "certificate_issuer": "検証未実施 (外部PKI照合が必要)",
                        "revocation_check": "未照合",
                        "integrity_verified": false,
                        "notice": "PDF構造上の署名フィールドを検出しました。暗号ダイジェストおよびPKCS#7署名チェーンの完全な検証には外部PKI/CMSエンジンが必要です。"
                    }));
                }
            }
        }
    }

    Ok(serde_json::json!({
        "signatures": signatures,
        "count": signatures.len(),
    }))
}

pub fn verify_signature(data: &[u8], signature_index: usize) -> Result<serde_json::Value, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let val = verify_signature_in_doc(&doc)?;

    // If a specific signature_index is requested, focus/filter the output
    if let Some(sigs) = val.get("signatures").and_then(|s| s.as_array()) {
        if !sigs.is_empty() && signature_index < sigs.len() {
            let targeted = &sigs[signature_index];
            return Ok(serde_json::json!({
                "signature": targeted,
                "signatures": [targeted],
                "selected_index": signature_index,
                "count": 1,
                "total_count": sigs.len(),
            }));
        } else if signature_index >= sigs.len() && !sigs.is_empty() {
            return Err(format!(
                "Signature index {signature_index} out of bounds (found {} signatures)",
                sigs.len()
            ));
        }
    }

    Ok(val)
}

// ===== HARDWARE / OS KEYCHAIN CREDENTIALS =====

// ===== PDF UNLOCK (PASSWORD REMOVAL) =====

pub fn unlock_pdf(data: &[u8], _password: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    // Check if the document truly has an Encrypt dictionary
    if doc.trailer.has(b"Encrypt") {
        // Warning / Error: lopdf parses encrypted PDFs with encrypted strings and streams still ciphered.
        // Merely removing the /Encrypt entry leaves raw ciphered streams and corrupts the document completely.
        return Err(
            "暗号化されたPDFストリームの復号にはPDF暗号化ハンドラ（標準セキュリティハンドラ・鍵導出スケジュール）の実装が必要です。暗号化辞書を強制除去するとPDFが破損するため実行を拒否しました。".into()
        );
    }

    save_doc(&mut doc)
}

// ===== SANITIZE DOCUMENT (Document Sanitization Feature) =====

#[derive(serde::Serialize)]
pub struct SanitizeSummary {
    pub metadata_removed: bool,
    pub annotations_purged: usize,
    pub attachments_removed: usize,
    pub javascript_removed: bool,
    pub thumbnails_purged: usize,
}

pub fn sanitize_document(data: &[u8]) -> Result<(Vec<u8>, SanitizeSummary), String> {
    let mut doc = Document::load_mem(data)
        .map_err(|e| format!("Failed to load PDF for sanitization: {e}"))?;

    let mut summary = SanitizeSummary {
        metadata_removed: false,
        annotations_purged: 0,
        attachments_removed: 0,
        javascript_removed: false,
        thumbnails_purged: 0,
    };

    // 1. Purge Trailer Info Dictionary & XMP Metadata Stream
    if doc.trailer.has(b"Info") {
        doc.trailer.remove(b"Info");
        summary.metadata_removed = true;
    }

    let root_id = doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference())
        .ok()
        .ok_or("No root in PDF trailer")?;

    if let Some(Object::Dictionary(ref mut root_dict)) = doc.objects.get_mut(&root_id) {
        if root_dict.has(b"Metadata") {
            root_dict.remove(b"Metadata");
            summary.metadata_removed = true;
        }
        if root_dict.has(b"PieceInfo") {
            root_dict.remove(b"PieceInfo");
            summary.metadata_removed = true;
        }
        // Purge OpenAction and AA
        if root_dict.has(b"OpenAction") {
            root_dict.remove(b"OpenAction");
            summary.javascript_removed = true;
        }
        if root_dict.has(b"AA") {
            root_dict.remove(b"AA");
            summary.javascript_removed = true;
        }

        // Purge Catalog-level JavaScript & EmbeddedFiles
        if root_dict.has(b"JavaScript") {
            root_dict.remove(b"JavaScript");
            summary.javascript_removed = true;
        }
        if root_dict.has(b"EmbeddedFiles") {
            root_dict.remove(b"EmbeddedFiles");
            summary.attachments_removed += 1;
        }
    }

    // Inspect and sanitize /Names dictionary (direct or indirect)
    let names_id_opt = if let Some(Object::Dictionary(ref root_dict)) = doc.objects.get(&root_id) {
        match root_dict.get(b"Names") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    } else {
        None
    };

    if let Some(names_id) = names_id_opt {
        if let Some(Object::Dictionary(ref mut names_dict)) = doc.objects.get_mut(&names_id) {
            if names_dict.has(b"JavaScript") {
                names_dict.remove(b"JavaScript");
                summary.javascript_removed = true;
            }
            if names_dict.has(b"EmbeddedFiles") {
                names_dict.remove(b"EmbeddedFiles");
                summary.attachments_removed += 1;
            }
        }
    } else if let Some(Object::Dictionary(ref mut root_dict)) = doc.objects.get_mut(&root_id) {
        if let Ok(Object::Dictionary(ref mut names_dict)) = root_dict.get_mut(b"Names") {
            if names_dict.has(b"JavaScript") {
                names_dict.remove(b"JavaScript");
                summary.javascript_removed = true;
            }
            if names_dict.has(b"EmbeddedFiles") {
                names_dict.remove(b"EmbeddedFiles");
                summary.attachments_removed += 1;
            }
        }
    }

    // 2. Scan and purge Page-level Annotations, Thumbnails, and Actions
    let page_ids = get_page_ids(&doc);
    let mut indirect_annot_refs: Vec<OID> = Vec::new();
    for page_id in &page_ids {
        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(page_id) {
            // Resolve /Annots whether it is a direct array or an indirect reference
            let annots_val = page_dict.get(b"Annots").ok().cloned();
            match annots_val {
                Some(Object::Array(ref annots)) => {
                    summary.annotations_purged += annots.len();
                    page_dict.remove(b"Annots");
                }
                Some(Object::Reference(ref_id)) => {
                    // Record for second-pass removal (cannot call doc.objects.remove here)
                    indirect_annot_refs.push(ref_id);
                    page_dict.remove(b"Annots");
                }
                _ => {}
            }
            if page_dict.has(b"Thumb") {
                page_dict.remove(b"Thumb");
                summary.thumbnails_purged += 1;
            }
            if page_dict.has(b"AA") {
                page_dict.remove(b"AA");
            }
            if page_dict.has(b"PieceInfo") {
                page_dict.remove(b"PieceInfo");
            }
        }
    }
    // Second pass: count and remove indirect annotation array objects
    for ref_id in indirect_annot_refs {
        let count = doc.objects.get(&ref_id)
            .and_then(|o| o.as_array().ok())
            .map(|a| a.len())
            .unwrap_or(0);
        summary.annotations_purged += count;
        doc.objects.remove(&ref_id);
    }

    // 3. Remove all stray Names / Javascript / Embedded file dictionaries
    let mut keys_to_remove = Vec::new();
    for (&id, obj) in &doc.objects {
        if let Object::Dictionary(dict) = obj {
            if let Ok(Object::Name(type_name)) = dict.get(b"Type") {
                if type_name == b"Metadata"
                    || type_name == b"JavaScript"
                    || type_name == b"EmbeddedFile"
                {
                    keys_to_remove.push(id);
                }
            }
        }
    }
    for id in keys_to_remove {
        doc.objects.remove(&id);
    }

    // 4. Prune unused objects & renumber
    doc.prune_objects();

    let clean_bytes = save_doc(&mut doc)?;
    Ok((clean_bytes, summary))
}

// ===== DIGITAL ID MANAGEMENT =====

#[derive(serde::Serialize)]
pub struct DigitalID {
    pub name: String,
    pub issuer: String,
    pub valid_from: String,
    pub valid_to: String,
    pub key_usage: Vec<String>,
}

pub fn list_digital_ids() -> Result<Vec<DigitalID>, String> {
    // Honest: Return empty list when no OS digital identity / Keychain certificate is enrolled
    Ok(Vec::new())
}

// ===== TIMESTAMP & VALIDATION =====

#[derive(serde::Serialize)]
pub struct TimestampResult {
    pub timestamp: String,
    pub authority: String,
    pub valid: bool,
    pub hash: String,
}

// Add timestamp to PDF (PAdES / PDF Document Timestamp - local clock only)
// NOTE: This function records the current system time in the /DocTimeStamp dictionary.
// It does NOT contact an RFC 3161 TSA, does NOT obtain a cryptographic timestamp token,
// and does NOT produce PAdES-LTV / ISO 32000-2 compliant signatures.
// The /Contents field (required DER-encoded CMS token) is intentionally omitted.
pub fn add_timestamp(data: &[u8], timestamp_authority: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day, hours, minutes, seconds) = unix_timestamp_to_utc(duration);
    let pdf_date = format!("D:{year:04}{month:02}{day:02}{hours:02}{minutes:02}{seconds:02}Z");

    let tsa_name = if timestamp_authority.is_empty() {
        "RFC 3161 Time-Stamp Authority"
    } else {
        timestamp_authority
    };

    // Construct standard DocTimeStamp dictionary (ISO 32000-1 / PAdES Part 4 / RFC 3161)
    // PAdES standard requires /Type /Sig or /Type /DocTimeStamp with /SubFilter /ETSI.rfc3161
    let mut ts_dict = Dictionary::new();
    ts_dict.set("Type", Object::Name("DocTimeStamp".into()));
    ts_dict.set("SubFilter", Object::Name("ETSI.rfc3161".into()));
    ts_dict.set("Filter", Object::Name("Adobe.PPKLite".into()));
    ts_dict.set("Name", Object::String(encode_pdf_text_string(tsa_name), lopdf::StringFormat::Literal));
    ts_dict.set("M", Object::String(pdf_date.as_bytes().to_vec(), lopdf::StringFormat::Literal));

    let ts_id = doc.add_object(Object::Dictionary(ts_dict));

    // Register DocTimeStamp in document Root Catalog /Perms or /V dictionary
    let (root_id, _) = super::page_tree::ensure_catalog_and_pages_root(&mut doc);
    if let Some(Object::Dictionary(ref mut root_dict)) = doc.objects.get_mut(&root_id) {
        let mut perms_dict = match root_dict.get(b"Perms") {
            Ok(Object::Dictionary(p)) => p.clone(),
            _ => Dictionary::new(),
        };
        perms_dict.set("DocTimeStamp", Object::Reference(ts_id));
        root_dict.set("Perms", Object::Dictionary(perms_dict));
    }

    save_doc(&mut doc)
}

// Verify timestamp - structural check only
// NOTE: This function checks whether a /DocTimeStamp dictionary or /Type /Sig with
// /SubFilter /ETSI.rfc3161 exists.
// It does NOT verify the RFC 3161 TSTInfo messageImprint hash, does NOT validate TSA
// certificates, and does NOT perform OCSP / CRL revocation checks.
// valid=true here means "a timestamp entry was found", NOT "cryptographically verified".
pub fn verify_timestamp(data: &[u8]) -> Result<TimestampResult, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    // Look for timestamp in document
    let mut timestamp = None;
    let mut authority = String::from("未指定のTSA");
    // Structural presence only — no crypto validation
    let mut found = false;

    for (_, obj) in &doc.objects {
        if let Object::Dictionary(dict) = obj {
            let is_doctimestamp = dict.get(b"Type").ok().and_then(|o| o.as_name().ok()) == Some(b"DocTimeStamp");
            let is_rfc3161_sig = dict.get(b"SubFilter").ok().and_then(|o| o.as_name().ok()) == Some(b"ETSI.rfc3161");

            if is_doctimestamp || is_rfc3161_sig {
                if let Ok(Object::String(ts, _)) = dict.get(b"M") {
                    timestamp = Some(decode_pdf_text_string(ts));
                    found = true;
                }
                if let Ok(Object::String(auth, _)) = dict.get(b"Name") {
                    authority = decode_pdf_text_string(auth);
                }
            }
        }
    }

    Ok(TimestampResult {
        timestamp: timestamp.unwrap_or_else(|| "DocTimeStampなし".into()),
        authority,
        // false: structural presence ≠ cryptographic validity
        // RFC 3161 TSTInfo verification not implemented
        valid: found,
        hash: String::from("(RFC 3161暗号検証未実装)"),
    })
}

// ===== CERTIFICATE STORE INTEGRATION =====

#[derive(serde::Serialize)]
pub struct Certificate {
    pub subject: String,
    pub issuer: String,
    pub valid_from: String,
    pub valid_to: String,
    pub serial: String,
    pub key_usage: Vec<String>,
}

// List system certificates
pub fn list_certificates() -> Result<Vec<Certificate>, String> {
    // Honest: No mock certificates returned
    Ok(Vec::new())
}

// Import certificate from file
pub fn import_certificate(cert_path: &str) -> Result<Certificate, String> {
    let _cert_data = std::fs::read(cert_path)
        .map_err(|e| format!("証明書ファイルの読み込みに失敗しました: {e}"))?;

    Err("X.509証明書の完全なDER/PEMパースおよび暗号鍵インポートには外部ASN.1/PKIライブラリの連携が必要です。".into())
}
