//! Standard Security Handler password encryption (ISO 32000-1 §7.6 /
//! ISO 32000-2 §7.6.4).
//!
//! * [`encrypt_pdf`] — AES-256 (V=5 / R=6), the handler Acrobat Pro and qpdf
//!   emit for "AES-256" document protection. The 32-byte file key is random
//!   and independent of the password; /O /U /OE /UE carry password
//!   verification material and /Perms carries the encrypted /P flags.
//! * [`decrypt_pdf`] — R=5/R=6 natively; R=2..=4 (RC4 / AESV2) via lopdf.
//!
//! The test suite cross-validates both directions against qpdf (encrypt with
//! qpdf → decrypt here, and encrypt here → `qpdf --check/--decrypt`), so the
//! implementation is checked against an independent, battle-tested producer.

use lopdf::{Document, Object, StringFormat};
use sha2::{Digest, Sha256, Sha384, Sha512};

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::{Aes128, Aes256};

const ZERO_IV: [u8; 16] = [0u8; 16];

/// User-facing permission flags (ISO 32000-1 Table 22 bit mapping,
/// empirically cross-checked against `qpdf --show-encryption`).
#[derive(Debug, Clone, Copy)]
pub struct PermissionFlags {
    /// Print the document (clears bit 3 low-quality + bit 12 high-quality).
    pub print: bool,
    /// Modify contents, fill forms and assemble pages (bits 4, 9, 11).
    pub modify: bool,
    /// Copy / extract text and graphics (bit 5).
    pub copy: bool,
    /// Add or modify annotations (bit 6).
    pub annotate: bool,
}

impl Default for PermissionFlags {
    fn default() -> Self {
        PermissionFlags { print: true, modify: true, copy: true, annotate: true }
    }
}

impl PermissionFlags {
    /// Build the signed 32-bit /P integer. Base 0xFFFFFFFC = "everything
    /// allowed" (bits 1-2 reserved 0, bits 11-32 reserved 1); bit 8 (extract
    /// for accessibility) is always left set, matching qpdf and the PDF/UA
    /// requirement that accessibility extraction cannot be restricted.
    fn to_p(self) -> i32 {
        let mut p: u32 = 0xFFFF_FFFC;
        if !self.print {
            p &= !0x0000_0004; // bit 3  print
            p &= !0x0000_0800; // bit 12 print high quality
        }
        if !self.modify {
            p &= !0x0000_0008; // bit 4  modify contents
            p &= !0x0000_0100; // bit 9  fill in forms
            p &= !0x0000_0400; // bit 11 assemble pages
        }
        if !self.copy {
            p &= !0x0000_0010; // bit 5  copy / extract
        }
        if !self.annotate {
            p &= !0x0000_0020; // bit 6  annotations
        }
        // bit 8 (accessibility) stays set.
        p as i32
    }
}

// ===== raw AES block primitives (CBC / ECB, no external modes crate) =====

fn encrypt_block(key: &[u8], block: &mut [u8; 16]) -> Result<(), String> {
    match key.len() {
        16 => Aes128::new(GenericArray::from_slice(key)).encrypt_block(GenericArray::from_mut_slice(block)),
        32 => Aes256::new(GenericArray::from_slice(key)).encrypt_block(GenericArray::from_mut_slice(block)),
        n => return Err(format!("AES key length {n} is not 128 or 256 bits")),
    }
    Ok(())
}

fn decrypt_block(key: &[u8], block: &mut [u8; 16]) -> Result<(), String> {
    match key.len() {
        16 => Aes128::new(GenericArray::from_slice(key)).decrypt_block(GenericArray::from_mut_slice(block)),
        32 => Aes256::new(GenericArray::from_slice(key)).decrypt_block(GenericArray::from_mut_slice(block)),
        n => return Err(format!("AES key length {n} is not 128 or 256 bits")),
    }
    Ok(())
}

/// AES-CBC without padding; `data` must be a multiple of 16 bytes.
fn cbc_encrypt(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() % 16 != 0 {
        return Err("CBC input length must be a multiple of 16".into());
    }
    let mut prev = *iv;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(16) {
        let mut block = [0u8; 16];
        for (i, b) in chunk.iter().enumerate() {
            block[i] = *b ^ prev[i];
        }
        encrypt_block(key, &mut block)?;
        out.extend_from_slice(&block);
        prev = block;
    }
    Ok(out)
}

/// AES-CBC without padding.
fn cbc_decrypt(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() % 16 != 0 {
        return Err("CBC input length must be a multiple of 16".into());
    }
    let mut prev = *iv;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(16) {
        let mut block: [u8; 16] = chunk.try_into().expect("chunks_exact(16)");
        let cipher_block = block;
        decrypt_block(key, &mut block)?;
        for (i, b) in block.iter().enumerate() {
            out.push(b ^ prev[i]);
        }
        prev = cipher_block;
    }
    Ok(out)
}

fn ecb_encrypt_256(key: &[u8], data: &[u8; 16]) -> Result<[u8; 16], String> {
    let mut block = *data;
    encrypt_block(key, &mut block)?;
    Ok(block)
}

fn pkcs7_pad(data: &[u8]) -> Vec<u8> {
    let pad = 16 - (data.len() % 16);
    let mut out = Vec::with_capacity(data.len() + pad);
    out.extend_from_slice(data);
    out.extend(std::iter::repeat(pad as u8).take(pad));
    out
}

fn pkcs7_unpad(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() || data.len() % 16 != 0 {
        return Err("AES暗号ブロックが壊れています".into());
    }
    let pad = *data.last().unwrap() as usize;
    if pad == 0 || pad > 16 || !data.ends_with(&vec![pad as u8; pad]) {
        return Err("AESパディング検証に失敗しました（パスワードまたはデータ不一致）".into());
    }
    Ok(data[..data.len() - pad].to_vec())
}

// ===== ISO 32000-2 password hashing (R=6) =====

/// Algorithm 8 (ISO 32000-2): iterative SHA-256/384/512 hash with an
/// AES-128-CBC mixing round. R=5 (deprecated Adobe AES-256 variant) returns
/// the plain initial SHA-256. Loop termination follows qpdf/pypdf:
/// at least 64 rounds, stop once `E.last() <= count - 32`.
fn calculate_hash(revision: i64, password: &[u8], salt: &[u8], udata: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(password);
    hasher.update(salt);
    hasher.update(udata);
    let mut k = hasher.finalize().to_vec();
    if revision < 6 {
        return k;
    }
    let mut count: i64 = 0;
    loop {
        count += 1;
        let mut unit = Vec::with_capacity(password.len() + k.len() + udata.len());
        unit.extend_from_slice(password);
        unit.extend_from_slice(&k);
        unit.extend_from_slice(udata);
        let mut k1 = Vec::with_capacity(unit.len() * 64);
        for _ in 0..64 {
            k1.extend_from_slice(&unit);
        }
        let block_key: [u8; 16] = k[0..16].try_into().expect("k >= 32 bytes");
        let block_iv: [u8; 16] = k[16..32].try_into().expect("k >= 32 bytes");
        let e = cbc_encrypt(&block_key, &block_iv, &k1).expect("k1 is a multiple of 16");
        let sum: u32 = e[..16].iter().map(|&b| b as u32).sum();
        k = match sum % 3 {
            0 => Sha256::digest(&e).to_vec(),
            1 => Sha384::digest(&e).to_vec(),
            _ => Sha512::digest(&e).to_vec(),
        };
        if count >= 64 && (e[e.len() - 1] as i64) <= count - 32 {
            break;
        }
    }
    k.truncate(32);
    k
}

/// Passwords: UTF-8, truncated to 127 bytes (ISO 32000-2 §7.6.4.3.3 step a).
/// SASLprep normalization is not applied — it only affects exotic edge cases
/// and qpdf skips it too.
fn encode_password(password: &str) -> Vec<u8> {
    let mut bytes = password.as_bytes().to_vec();
    bytes.truncate(127);
    bytes
}

fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).map_err(|e| format!("乱数生成に失敗しました: {e}"))?;
    Ok(buf)
}

/// Algorithm 3.8: U (48 bytes: hash + validation salt + key salt) and UE.
fn build_u(revision: i64, password: &[u8], file_key: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let salts = random_bytes::<16>()?;
    let (val_salt, key_salt) = (&salts[0..8], &salts[8..16]);
    let h = calculate_hash(revision, password, val_salt, &[]);
    let u = [h.as_slice(), val_salt, key_salt].concat();
    let tmp_key = calculate_hash(revision, password, key_salt, &[]);
    let ue = cbc_encrypt(&tmp_key, &ZERO_IV, file_key)?;
    Ok((u, ue))
}

/// Algorithm 3.9: O (48 bytes) and OE. `u` is the 48-byte U string.
fn build_o(revision: i64, password: &[u8], file_key: &[u8], u: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let salts = random_bytes::<16>()?;
    let (val_salt, key_salt) = (&salts[0..8], &salts[8..16]);
    let h = calculate_hash(revision, password, val_salt, u);
    let o = [h.as_slice(), val_salt, key_salt].concat();
    let tmp_key = calculate_hash(revision, password, key_salt, u);
    let oe = cbc_encrypt(&tmp_key, &ZERO_IV, file_key)?;
    Ok((o, oe))
}

/// Algorithm 3.10: encrypted permissions blob (16 bytes, AES-256-ECB).
fn build_perms(file_key: &[u8], p: i32, encrypt_metadata: bool) -> Result<Vec<u8>, String> {
    let mut block = [0u8; 16];
    block[0..4].copy_from_slice(&(p as u32).to_le_bytes());
    block[4..8].copy_from_slice(&[0xFF; 4]);
    block[8] = if encrypt_metadata { b'T' } else { b'F' };
    block[9..12].copy_from_slice(b"adb");
    block[12..16].copy_from_slice(&random_bytes::<4>()?);
    Ok(ecb_encrypt_256(file_key, &block)?.to_vec())
}

/// Derive the file key from a user or owner password for R>=5.
/// Returns None when neither password hash matches.
fn derive_file_key(
    revision: i64,
    password: &[u8],
    o: &[u8],
    oe: &[u8],
    u: &[u8],
    ue: &[u8],
) -> Option<Vec<u8>> {
    if u.len() < 48 || o.len() < 48 {
        return None;
    }
    // Try the user password: hash(password, UserValidationSalt) == U[:32].
    if calculate_hash(revision, password, &u[32..40], &[]) == u[0..32] {
        let tmp = calculate_hash(revision, password, &u[40..48], &[]);
        if let Ok(key) = cbc_decrypt(&tmp, &ZERO_IV, ue) {
            if key.len() == 32 {
                return Some(key);
            }
        }
    }
    // Try the owner password: hash(password, OwnerValidationSalt, U[:48]) == O[:32].
    if calculate_hash(revision, password, &o[32..40], &u[0..48]) == o[0..32] {
        let tmp = calculate_hash(revision, password, &o[40..48], &u[0..48]);
        if let Ok(key) = cbc_decrypt(&tmp, &ZERO_IV, oe) {
            if key.len() == 32 {
                return Some(key);
            }
        }
    }
    None
}

/// AES-256-CBC + PKCS#7 with a random 16-byte IV prefix (ISO 32000-1 §7.6.4).
fn aes_object_encrypt(key: &[u8], plain: &[u8]) -> Result<Vec<u8>, String> {
    let iv = random_bytes::<16>()?;
    let ct = cbc_encrypt(key, &iv, &pkcs7_pad(plain))?;
    let mut out = Vec::with_capacity(16 + ct.len());
    out.extend_from_slice(&iv);
    out.extend_from_slice(&ct);
    Ok(out)
}

fn aes_object_decrypt(key: &[u8], payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.len() < 32 || (payload.len() - 16) % 16 != 0 {
        return Err("AES暗号ペイロード長が不正です".into());
    }
    let (iv, ct) = payload.split_at(16);
    let iv: [u8; 16] = iv.try_into().expect("16 bytes");
    let padded = cbc_decrypt(key, &iv, ct)?;
    pkcs7_unpad(&padded)
}

// ===== object graph walk (V=5 uses the file key directly, no per-object KDF) =====

fn walk_crypt(obj: &mut Object, key: &[u8], encrypt: bool, encrypt_metadata: bool) -> Result<(), String> {
    match obj {
        Object::String(data, format) => {
            if encrypt {
                *data = aes_object_encrypt(key, data)?;
                *format = StringFormat::Hexadecimal;
            } else {
                *data = aes_object_decrypt(key, data)?;
            }
            Ok(())
        }
        Object::Stream(stream) => {
            let type_name = stream.dict.get(b"Type").ok().and_then(|o| o.as_name().ok());
            // XRef streams are never encrypted (ISO 32000-1 §7.6.4): readers
            // must parse them in the clear to locate /Encrypt, and lopdf
            // regenerates them on save anyway.
            if type_name == Some(b"XRef".as_slice()) {
                return Ok(());
            }
            let is_metadata = type_name == Some(b"Metadata".as_slice());
            if !encrypt_metadata && is_metadata {
                return Ok(());
            }
            // Strings inside the stream dictionary are encrypted as part of
            // this indirect object; the stream payload itself follows.
            for (_, value) in stream.dict.iter_mut() {
                walk_crypt(value, key, encrypt, encrypt_metadata)?;
            }
            let payload = stream.content.clone();
            let new_data = if encrypt {
                aes_object_encrypt(key, &payload)?
            } else {
                aes_object_decrypt(key, &payload)?
            };
            let len = new_data.len() as i64;
            stream.set_content(new_data);
            stream.dict.set("Length", Object::Integer(len));
            Ok(())
        }
        Object::Dictionary(dict) => {
            for (_, value) in dict.iter_mut() {
                walk_crypt(value, key, encrypt, encrypt_metadata)?;
            }
            Ok(())
        }
        Object::Array(items) => {
            for item in items.iter_mut() {
                walk_crypt(item, key, encrypt, encrypt_metadata)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn dict_str(dict: &lopdf::Dictionary, key: &[u8]) -> Result<Vec<u8>, String> {
    let name = String::from_utf8_lossy(key);
    let value = dict
        .get(key)
        .map_err(|_| format!("暗号化辞書の /{name} が存在しません"))?;
    value
        .as_str()
        .map(|s| s.to_vec())
        .map_err(|_| format!("暗号化辞書の /{name} が不正です"))
}

/// Encrypt a PDF with the Standard Security Handler, AES-256 (V=5 / R=6).
///
/// `user_password` is required to open the document; `owner_password`
/// (may be empty → equals the user password) unlocks permission changes.
/// All permissions are granted by default — the point is open-with-password,
/// not print restrictions. Use [`encrypt_pdf_with_permissions`] for /P flags.
pub fn encrypt_pdf(data: &[u8], user_password: &str, owner_password: &str) -> Result<Vec<u8>, String> {
    encrypt_pdf_with_permissions(data, user_password, owner_password, PermissionFlags::default())
}

pub fn encrypt_pdf_with_permissions(
    data: &[u8],
    user_password: &str,
    owner_password: &str,
    permissions: PermissionFlags,
) -> Result<Vec<u8>, String> {
    if user_password.is_empty() {
        return Err("開くためのユーザーパスワードを指定してください（空パスワードでは暗号化できません）".into());
    }
    let mut doc = Document::load_mem(data).map_err(|e| format!("PDF解析に失敗しました: {e}"))?;
    if doc.is_encrypted() {
        return Err("このPDFは既に暗号化されています".into());
    }

    let file_key = random_bytes::<32>()?;
    let user_pw = encode_password(user_password);
    let owner_pw = if owner_password.is_empty() { user_pw.clone() } else { encode_password(owner_password) };
    let p = permissions.to_p();

    let (u, ue) = build_u(6, &user_pw, &file_key)?;
    let (o, oe) = build_o(6, &owner_pw, &file_key, &u)?;
    let perms_blob = build_perms(&file_key, p, true)?;

    let mut encrypt_dict = lopdf::Dictionary::new();
    encrypt_dict.set("Filter", Object::Name(b"Standard".to_vec()));
    encrypt_dict.set("V", Object::Integer(5));
    encrypt_dict.set("R", Object::Integer(6));
    encrypt_dict.set("Length", Object::Integer(256));
    encrypt_dict.set("P", Object::Integer(p as i64));
    encrypt_dict.set("O", Object::String(o, StringFormat::Hexadecimal));
    encrypt_dict.set("U", Object::String(u, StringFormat::Hexadecimal));
    encrypt_dict.set("OE", Object::String(oe, StringFormat::Hexadecimal));
    encrypt_dict.set("UE", Object::String(ue, StringFormat::Hexadecimal));
    encrypt_dict.set("Perms", Object::String(perms_blob, StringFormat::Hexadecimal));

    let mut std_cf = lopdf::Dictionary::new();
    std_cf.set("AuthEvent", Object::Name(b"DocOpen".to_vec()));
    std_cf.set("CFM", Object::Name(b"AESV3".to_vec()));
    std_cf.set("Length", Object::Integer(32));
    let mut cf = lopdf::Dictionary::new();
    cf.set("StdCF", Object::Dictionary(std_cf));
    encrypt_dict.set("CF", Object::Dictionary(cf));
    encrypt_dict.set("StmF", Object::Name(b"StdCF".to_vec()));
    encrypt_dict.set("StrF", Object::Name(b"StdCF".to_vec()));

    let encrypt_id = doc.add_object(Object::Dictionary(encrypt_dict));
    doc.trailer.set(b"Encrypt", Object::Reference(encrypt_id));

    // A file ID is mandatory for encrypted documents (readers use it for
    // cache identity even though V=5 does not derive keys from it).
    if doc.trailer.get(b"ID").is_err() {
        let id = random_bytes::<16>()?;
        let hex: String = id.iter().map(|b| format!("{b:02x}")).collect();
        doc.trailer.set(
            b"ID",
            Object::Array(vec![
                Object::String(hex.clone().into_bytes(), StringFormat::Hexadecimal),
                Object::String(hex.into_bytes(), StringFormat::Hexadecimal),
            ]),
        );
    }

    for (&oid, obj) in doc.objects.iter_mut() {
        if oid == encrypt_id {
            continue; // the encryption dictionary itself is plaintext
        }
        walk_crypt(obj, &file_key, true, true)?;
    }

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("暗号化PDFの書き出しに失敗しました: {e}"))?;
    Ok(out)
}

/// Decrypt a password-protected PDF. Supports R=5/R=6 (native) and
/// R=2..=4 (RC4 / AESV2, delegated to lopdf). Already-plaintext input is
/// returned unchanged.
pub fn decrypt_pdf(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("PDF解析に失敗しました: {e}"))?;
    if !doc.is_encrypted() {
        return Ok(data.to_vec());
    }

    let encrypt_id = doc
        .trailer
        .get(b"Encrypt")
        .and_then(Object::as_reference)
        .map_err(|_| "暗号化辞書の参照が不正です（オブジェクト直書きの暗号化辞書には未対応です）".to_string())?;

    let (revision, o, oe, u, ue, encrypt_metadata) = {
        let enc = doc.get_object(encrypt_id).map_err(|_| "暗号化辞書が存在しません".to_string())?;
        let dict = enc.as_dict().map_err(|_| "暗号化辞書が不正です".to_string())?;
        let revision = dict
            .get(b"R")
            .map_err(|_| "暗号化辞書の /R が存在しません".to_string())?
            .as_i64()
            .map_err(|_| "暗号化辞書の /R が不正です".to_string())?;
        let filter = dict
            .get(b"Filter")
            .map_err(|_| "暗号化辞書の /Filter が存在しません".to_string())?
            .as_name()
            .map_err(|_| "暗号化辞書の /Filter が不正です".to_string())?;
        if filter != b"Standard" {
            return Err(format!(
                "未対応のセキュリティハンドラです: /{}（Standardのみ対応）",
                String::from_utf8_lossy(filter)
            ));
        }
        let encrypt_metadata = dict
            .get(b"EncryptMetadata")
            .ok()
            .and_then(|v| v.as_bool().ok())
            .unwrap_or(true);
        if revision >= 5 {
            (
                revision,
                dict_str(dict, b"O")?,
                dict_str(dict, b"OE")?,
                dict_str(dict, b"U")?,
                dict_str(dict, b"UE")?,
                encrypt_metadata,
            )
        } else {
            (revision, Vec::new(), Vec::new(), Vec::new(), Vec::new(), encrypt_metadata)
        }
    };

    if revision >= 5 {
        let pw = encode_password(password);
        let file_key = derive_file_key(revision, &pw, &o, &oe, &u, &ue)
            .ok_or_else(|| "パスワードが正しくありません".to_string())?;
        for (&oid, obj) in doc.objects.iter_mut() {
            if oid == encrypt_id {
                continue;
            }
            walk_crypt(obj, &file_key, false, encrypt_metadata)?;
        }
        doc.trailer.remove(b"Encrypt");
        doc.objects.remove(&encrypt_id);
    } else {
        // R=2..4: lopdf's built-in handler (RC4 / AESV2). It removes the
        // trailer /Encrypt entry itself on success.
        doc.decrypt(password).map_err(|e| {
            let msg = e.to_string();
            if msg.to_lowercase().contains("password") {
                "パスワードが正しくありません".to_string()
            } else {
                format!("復号に失敗しました: {msg}")
            }
        })?;
        doc.objects.remove(&encrypt_id);
    }

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("復号PDFの書き出しに失敗しました: {e}"))?;
    Ok(out)
}



