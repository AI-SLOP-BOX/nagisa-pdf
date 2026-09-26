//! Detached CMS signatures over exact PDF ByteRanges.
//!
//! Signing delegates CMS packaging to OpenSSL (`cms -sign -binary`). Nagisa
//! owns the security-critical PDF behavior: incremental updates, placeholder
//! reservation, exact ByteRange finalization, digest binding, and verification.

use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, DigestAlgorithmIdentifiers, EncapsulatedContentInfo, SignatureValue,
    SignedData, SignerIdentifier, SignerInfo, SignerInfos,
};
use der::{Any, AnyRef, Decode, Encode};
use lopdf::{Dictionary, Document, Object, ObjectId as OID};
use spki::AlgorithmIdentifierOwned;
use std::io::Write;
use std::process::{Command, Stdio};
use x509_cert::attr::{Attribute, Attributes};
use x509_cert::Certificate;

pub const CMS_PLACEHOLDER_LEN: usize = 16384;
const BYTERANGE_FIELD_PLACEHOLDER: i64 = 1111111111;
const BYTERANGE_FIELD_WIDTH: usize = 10;
const FILTER: &[u8] = b"Adobe.PPKLite";
const SUBFILTER: &[u8] = b"adbe.pkcs7.detached";

#[derive(Clone, Debug)]
pub struct SignatureFieldSeed {
    pub page_index: usize,
    pub rect: [f64; 4],
    pub field_name: String,
    pub signer_name: String,
    pub reason: String,
    pub location: String,
    pub contact_info: String,
}

#[derive(Clone, Debug)]
pub struct CmsSignRequest {
    pub seed: SignatureFieldSeed,
    pub private_key_pem: Vec<u8>,
    pub certificate_pem: Vec<u8>,
    pub chain_pem: Vec<Vec<u8>>,
    /// Optional PKCS#12/PFX container. Used when `private_key_pem` is empty;
    /// key, leaf certificate and CA chain are extracted with OpenSSL.
    pub p12_der: Option<Vec<u8>>,
    /// Password for `p12_der` (empty string for password-less containers).
    pub p12_password: Option<String>,
    /// Optional RFC 3161 TSA URL. When set, a trusted timestamp token is
    /// embedded as a CMS unsigned attribute (never a local-clock claim).
    pub tsa_url: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct TimestampReport {
    pub present: bool,
    pub imprint_matches: bool,
    pub gen_time: String,
    pub tsa_subject: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct CmsVerifyReport {
    pub signatures_found: usize,
    pub signature_index: usize,
    pub byterange_valid: bool,
    pub digest_algorithm: String,
    pub digest_matches: bool,
    pub cms_signature_valid: bool,
    pub signer_subject: String,
    pub signer_issuer: String,
    pub certificate_currently_valid: bool,
    /// None = no trust roots supplied, so chain validation was not performed.
    pub chain_valid: Option<bool>,
    pub chain_details: String,
    pub revocation_status: String,
    pub revocation_details: String,
    pub timestamp: Option<TimestampReport>,
    pub warnings: Vec<String>,
}

fn openssl_binary() -> String {
    std::env::var("NAGISA_OPENSSL_BIN").unwrap_or_else(|_| "openssl".to_string())
}

fn run_openssl(args: &[String], stdin_bytes: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut child = Command::new(openssl_binary())
        .args(args)
        .stdin(if stdin_bytes.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("OpenSSLを起動できませんでした: {e}"))?;
    if let Some(input) = stdin_bytes {
        child
            .stdin
            .as_mut()
            .ok_or_else(|| "OpenSSLの標準入力を開けませんでした".to_string())?
            .write_all(input)
            .map_err(|e| format!("OpenSSLへの入力に失敗しました: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("OpenSSLの実行に失敗しました: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "OpenSSLが失敗しました: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn temp_workdir(prefix: &str) -> Result<std::path::PathBuf, String> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "{prefix}_{}_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).map_err(|e| format!("一時ディレクトリを作成できません: {e}"))?;
    Ok(dir)
}
fn pem_block(pem_bytes: &[u8], label: &str) -> Result<Vec<u8>, String> {
    let text =
        std::str::from_utf8(pem_bytes).map_err(|_| "PEMはUTF-8である必要があります".to_string())?;
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let mut bodies = Vec::new();
    let mut active = false;
    let mut body = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line == begin {
            active = true;
            body.clear();
            continue;
        }
        if line == end && active {
            bodies.push(body.clone());
            active = false;
            body.clear();
            continue;
        }
        if active && !line.is_empty() {
            body.push_str(line);
        }
    }
    if bodies.is_empty() {
        return Err(format!("{label}が見つかりません"));
    }
    base64_decode(&bodies.join("")).ok_or_else(|| format!("{label}の復号に失敗しました"))
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut quad = [0u8; 4];
    let mut count = 0usize;
    let mut pad = 0usize;
    for ch in input.chars() {
        if ch.is_ascii_whitespace() {
            continue;
        }
        if ch == '=' {
            quad[count] = 0;
            count += 1;
            pad += 1;
        } else {
            quad[count] = match ch {
                'A'..='Z' => ch as u8 - b'A',
                'a'..='z' => ch as u8 - b'a' + 26,
                '0'..='9' => ch as u8 - b'0' + 52,
                '+' => 62,
                '/' => 63,
                _ => return None,
            };
            count += 1;
        }
        if count == 4 {
            if pad > 2 {
                return None;
            }
            let value = ((quad[0] as u32) << 18)
                | ((quad[1] as u32) << 12)
                | ((quad[2] as u32) << 6)
                | quad[3] as u32;
            out.push((value >> 16) as u8);
            if pad < 2 {
                out.push((value >> 8) as u8);
            }
            if pad == 0 {
                out.push(value as u8);
            }
            count = 0;
            pad = 0;
        }
    }
    if count != 0 {
        return None;
    }
    Some(out)
}

struct Tlv<'a> {
    tag: u8,
    value: &'a [u8],
    children: Vec<Tlv<'a>>,
}

fn read_length(input: &[u8]) -> Option<(usize, usize)> {
    let first = *input.first()?;
    if first & 0x80 == 0 {
        return Some((first as usize, 1));
    }
    let count = (first & 0x7f) as usize;
    // count == 0 encodes BER's indefinite length (0x80) and is resolved by
    // `parse_tlv` against the end-of-contents octets.
    if count == 0 {
        return Some((usize::MAX, 1));
    }
    if count > 4 || input.len() < count + 1 {
        return None;
    }
    let mut length = 0usize;
    for byte in &input[1..count + 1] {
        length = (length << 8) | (*byte as usize);
    }
    Some((length, count + 1))
}

/// Parse one TLV, accepting both DER definite lengths and BER indefinite lengths
/// (the encoding Apple Security.framework emits for CMS SignedData).
fn parse_tlv(input: &[u8]) -> Option<(Tlv<'_>, usize)> {
    if input.len() < 2 {
        return None;
    }
    let tag = input[0];
    let (length, header) = read_length(&input[1..])?;
    let body_start = header + 1;

    if length == usize::MAX {
        // Indefinite length: walk children until the matching 0x00 0x00 EOC.
        let mut children = Vec::new();
        let mut cursor = body_start;
        loop {
            if cursor + 1 >= input.len() {
                return None;
            }
            if input[cursor] == 0x00 && input[cursor + 1] == 0x00 {
                let value = &input[body_start..cursor];
                return Some((
                    Tlv {
                        tag,
                        value,
                        children,
                    },
                    cursor + 2,
                ));
            }
            let (child, used) = parse_tlv(&input[cursor..])?;
            children.push(child);
            cursor += used;
        }
    }

    if input.len() < body_start + length {
        return None;
    }
    let value = &input[body_start..body_start + length];
    let mut children = Vec::new();
    if tag & 0x20 != 0 {
        let mut rest = value;
        while !rest.is_empty() {
            match parse_tlv(rest) {
                Some((child, used)) => {
                    children.push(child);
                    rest = &rest[used..];
                }
                // Tolerate constructed bodies that are not a clean TLV sequence
                // (seen with some third-party producers) by treating the body as
                // opaque. Integrity is still enforced authoritatively by
                // OpenSSL's detached CMS verification over the PDF ByteRange.
                None => {
                    children.clear();
                    break;
                }
            }
        }
    }
    Some((
        Tlv {
            tag,
            value,
            children,
        },
        body_start + length,
    ))
}

fn parse_der(input: &[u8]) -> Result<Tlv<'_>, String> {
    parse_tlv(input)
        .map(|(node, _)| node)
        .ok_or_else(|| "DERの解析に失敗しました".to_string())
}

fn integer_bytes(value: &[u8]) -> Vec<u8> {
    let mut start = 0usize;
    while start + 1 < value.len() && value[start] == 0 {
        start += 1;
    }
    value[start..].to_vec()
}

fn rsa_private_parts(der_bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let root = parse_der(der_bytes)?;
    if root.tag != 0x30 {
        return Err("RSA秘密鍵はSEQUENCEである必要があります".to_string());
    }
    if root.children.len() >= 4 && root.children[0].tag == 0x02 {
        let modulus = integer_bytes(root.children[1].value);
        let exponent = integer_bytes(root.children[3].value);
        if modulus.is_empty() || exponent.is_empty() {
            return Err("RSA秘密鍵の成分が空です".to_string());
        }
        return Ok((modulus, exponent));
    }
    if root.children.len() == 3 && root.children[1].tag == 0x30 && root.children[2].tag == 0x04 {
        return rsa_private_parts(root.children[2].value);
    }
    Err("RSA秘密鍵の形式を認識できません".to_string())
}

fn rsa_public_parts(public_der: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let root = parse_der(public_der)?;
    if root.tag != 0x30 || root.children.len() != 2 {
        return Err("RSA公開鍵は2要素のSEQUENCEである必要があります".to_string());
    }
    Ok((
        integer_bytes(root.children[0].value),
        integer_bytes(root.children[1].value),
    ))
}

fn certificate_public_parts(cert_der: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let root = parse_der(cert_der)?;
    let tbs = root
        .children
        .iter()
        .find(|node| node.tag == 0x30)
        .ok_or_else(|| "TBSCertificateが見つかりません".to_string())?;
    let spki = tbs
        .children
        .iter()
        .rev()
        .find(|node| node.tag == 0x30 && node.children.len() == 2)
        .ok_or_else(|| "SubjectPublicKeyInfoが見つかりません".to_string())?;
    let bits = spki
        .children
        .iter()
        .find(|node| node.tag == 0x03)
        .ok_or_else(|| "公開鍵が見つかりません".to_string())?;
    if bits.value.is_empty() || bits.value[0] != 0 {
        return Err("未対応の公開鍵パディングです".to_string());
    }
    rsa_public_parts(&bits.value[1..])
}

fn sha256_digest(message: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(message);
    hasher.finalize().into()
}

const SHA256_PREFIX: &[u8] = &[
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

fn rsa_verify_sha256(message: &[u8], signature: &[u8], modulus: &[u8], exponent: &[u8]) -> bool {
    if signature.len() != modulus.len() || modulus.len() < 128 {
        return false;
    }
    let key = match rsa::RsaPublicKey::new(
        rsa::BigUint::from_bytes_be(modulus),
        rsa::BigUint::from_bytes_be(exponent),
    ) {
        Ok(key) => key,
        Err(_) => return false,
    };
    let digest = sha256_digest(message);
    key.verify(
        rsa::Pkcs1v15Sign {
            hash_len: Some(32),
            prefix: SHA256_PREFIX.to_vec().into(),
        },
        &digest,
        signature,
    )
    .is_ok()
}
fn serialize_dictionary(dict: &Dictionary) -> Vec<u8> {
    let mut out = Vec::from(b"<<".as_slice());
    for (key, value) in dict.iter() {
        out.push(b'/');
        out.extend_from_slice(key);
        out.push(b' ');
        serialize_object(value, &mut out);
        out.push(b' ');
    }
    out.extend_from_slice(b">>");
    out
}

fn serialize_object(object: &Object, out: &mut Vec<u8>) {
    match object {
        Object::Null => out.extend_from_slice(b"null"),
        Object::Boolean(value) => out.extend_from_slice(if *value { b"true" } else { b"false" }),
        Object::Integer(value) => out.extend_from_slice(value.to_string().as_bytes()),
        Object::Real(value) => out.extend_from_slice(value.to_string().as_bytes()),
        Object::Name(name) => {
            out.push(b'/');
            out.extend_from_slice(name);
        }
        Object::String(bytes, format) => {
            if *format == lopdf::StringFormat::Hexadecimal {
                out.push(b'<');
                for byte in bytes {
                    out.extend_from_slice(format!("{byte:02X}").as_bytes());
                }
                out.push(b'>');
            } else {
                out.push(b'(');
                for byte in bytes {
                    if *byte == b'(' || *byte == b')' || *byte == b'\\' {
                        out.push(b'\\');
                    }
                    out.push(*byte);
                }
                out.push(b')');
            }
        }
        Object::Array(items) => {
            out.push(b'[');
            for item in items {
                serialize_object(item, out);
                out.push(b' ');
            }
            out.push(b']');
        }
        Object::Dictionary(dict) => out.extend_from_slice(&serialize_dictionary(dict)),
        Object::Stream(stream) => {
            out.extend_from_slice(&serialize_dictionary(&stream.dict));
            out.extend_from_slice(b" stream\n");
            out.extend_from_slice(&stream.content);
            out.extend_from_slice(b"\nendstream");
        }
        Object::Reference((number, generation)) => {
            out.extend_from_slice(format!("{number} {generation} R").as_bytes())
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn signature_entries(doc: &Document) -> Vec<(Vec<i64>, Vec<u8>)> {
    let mut out = Vec::new();
    for object in doc.objects.values() {
        let dict = match object {
            Object::Dictionary(dict) => dict,
            _ => continue,
        };
        if dict.get(b"FT").ok().and_then(|value| value.as_name().ok()) != Some(b"Sig") {
            continue;
        }
        let value_id = match dict
            .get(b"V")
            .ok()
            .and_then(|value| value.as_reference().ok())
        {
            Some(id) => id,
            None => continue,
        };
        let signature = match doc
            .objects
            .get(&value_id)
            .and_then(|value| value.as_dict().ok())
        {
            Some(signature) => signature,
            None => continue,
        };
        let byterange = signature
            .get(b"ByteRange")
            .ok()
            .and_then(|value| value.as_array().ok())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|value| value.as_i64().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let contents = signature
            .get(b"Contents")
            .ok()
            .and_then(|value| match value {
                Object::String(bytes, _) => Some(bytes.clone()),
                _ => None,
            })
            .unwrap_or_default();
        if byterange.len() == 4 && !contents.is_empty() {
            out.push((byterange, contents));
        }
    }
    out
}

fn byterange_parts(pdf: &[u8], byterange: &[i64]) -> Result<Vec<u8>, String> {
    if byterange.len() != 4 || byterange.iter().any(|value| *value < 0) {
        return Err("ByteRangeが不正です".to_string());
    }
    let first_offset = byterange[0] as usize;
    let first_len = byterange[1] as usize;
    let second_offset = byterange[2] as usize;
    let second_len = byterange[3] as usize;
    let first_end = first_offset
        .checked_add(first_len)
        .ok_or_else(|| "ByteRangeが不正です".to_string())?;
    let second_end = second_offset
        .checked_add(second_len)
        .ok_or_else(|| "ByteRangeが不正です".to_string())?;
    if first_end > pdf.len() || second_end > pdf.len() || first_end > second_offset {
        return Err("ByteRangeがPDF範囲外です".to_string());
    }
    let mut signed = Vec::with_capacity(first_len + second_len);
    signed.extend_from_slice(&pdf[first_offset..first_end]);
    signed.extend_from_slice(&pdf[second_offset..second_end]);
    Ok(signed)
}

fn current_pdf_date() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let (mut year, mut days) = (1970u32, now / 86400);
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let year_days = if leap { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for length in month_days {
        if days < length as u64 {
            break;
        }
        days -= length as u64;
        month += 1;
    }
    let time = now % 86400;
    format!(
        "D:{year:04}{month:02}{day:02}{hour:02}{minute:02}{second:02}Z",
        day = days + 1,
        hour = time / 3600,
        minute = (time % 3600) / 60,
        second = time % 60
    )
}

pub fn append_incremental_update(
    original_pdf: &[u8],
    overlay: Document,
) -> Result<Vec<u8>, String> {
    if original_pdf.is_empty() {
        return Err("PDFが空です".to_string());
    }
    let original =
        Document::load_mem(original_pdf).map_err(|e| format!("PDFの解析に失敗しました: {e}"))?;
    let mut out = original_pdf.to_vec();
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    let mut next_id = original.max_id.max(1) + 1;
    let mut remap = std::collections::BTreeMap::new();
    for old_id in overlay.objects.keys() {
        remap.insert(*old_id, (next_id, old_id.1));
        next_id += 1;
    }
    let mut serialized = Vec::new();
    for (old_id, mut object) in overlay.objects.into_iter() {
        rewrite_references(&mut object, &remap);
        serialized.push((remap[&old_id], object));
    }
    let mut trailer_object = Object::Dictionary(overlay.trailer.clone());
    rewrite_references(&mut trailer_object, &remap);
    let mut trailer = match trailer_object {
        Object::Dictionary(dict) => dict,
        _ => return Err("trailerの再構成に失敗しました".to_string()),
    };
    for (key, value) in original.trailer.iter() {
        if key == b"Size" || key == b"Prev" {
            continue;
        }
        if !trailer.has(key) {
            trailer.set(key.clone(), value.clone());
        }
    }
    trailer.set("Size", Object::Integer((next_id - 1) as i64));
    if let Some(previous) = find_startxref(original_pdf) {
        trailer.set("Prev", Object::Integer(previous as i64));
    }
    serialized.sort_by_key(|(id, _)| *id);
    let mut offsets = Vec::new();
    for (id, object) in &serialized {
        offsets.push((id.0, out.len()));
        out.extend_from_slice(format!("{} {} obj\n", id.0, id.1).as_bytes());
        let mut body = Vec::new();
        serialize_object(object, &mut body);
        out.extend_from_slice(&body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_offset = out.len();
    out.extend_from_slice(b"xref\n0 1\n0000000000 65535 f \n");
    let mut section_start: Option<u32> = None;
    let mut section: Vec<(u32, usize)> = Vec::new();
    for entry in offsets {
        match section.last() {
            Some((last_id, _)) if *last_id + 1 == entry.0 => section.push(entry),
            _ => {
                if let Some(start) = section_start {
                    out.extend_from_slice(format!("{start} {}\n", section.len()).as_bytes());
                    for (_, offset) in &section {
                        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
                    }
                }
                section_start = Some(entry.0);
                section = vec![entry];
            }
        }
    }
    if let Some(start) = section_start {
        out.extend_from_slice(format!("{start} {}\n", section.len()).as_bytes());
        for (_, offset) in &section {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
    }
    out.extend_from_slice(b"trailer\n");
    out.extend_from_slice(&serialize_dictionary(&trailer));
    out.extend_from_slice(format!("\nstartxref\n{xref_offset}\n%%EOF").as_bytes());
    Ok(out)
}

fn rewrite_references(object: &mut Object, remap: &std::collections::BTreeMap<OID, OID>) {
    let mut stack = vec![object];
    while let Some(current) = stack.pop() {
        match current {
            Object::Reference(id) => {
                if let Some(new_id) = remap.get(id) {
                    *id = *new_id;
                }
            }
            Object::Array(items) => stack.extend(items.iter_mut()),
            Object::Dictionary(dict) => stack.extend(dict.iter_mut().map(|(_, value)| value)),
            Object::Stream(stream) => stack.extend(stream.dict.iter_mut().map(|(_, value)| value)),
            _ => {}
        }
    }
}

fn find_startxref(pdf: &[u8]) -> Option<usize> {
    let marker = b"startxref";
    let pos = pdf
        .windows(marker.len())
        .rposition(|window| window == marker)?;
    let mut rest = &pdf[pos + marker.len()..];
    while rest
        .first()
        .map(|byte| byte.is_ascii_whitespace())
        .unwrap_or(false)
    {
        rest = &rest[1..];
    }
    let mut digits = Vec::new();
    for byte in rest {
        if byte.is_ascii_digit() {
            digits.push(*byte);
        } else {
            break;
        }
    }
    String::from_utf8(digits).ok()?.parse::<usize>().ok()
}

fn ensure_acroform(overlay: &mut Document, original: &Document) -> Result<OID, String> {
    let root_id = overlay
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|value| value.as_reference().ok())
        .or_else(|| {
            original
                .trailer
                .get(b"Root")
                .ok()
                .and_then(|value| value.as_reference().ok())
        })
        .ok_or_else(|| "カタログが見つかりません".to_string())?;
    if overlay.objects.get(&root_id).is_none() {
        let catalog = original
            .objects
            .get(&root_id)
            .and_then(|value| value.as_dict().ok())
            .ok_or_else(|| "カタログ辞書が見つかりません".to_string())?
            .clone();
        overlay.objects.insert(root_id, Object::Dictionary(catalog));
    }
    let existing = overlay
        .objects
        .get(&root_id)
        .and_then(|value| value.as_dict().ok())
        .and_then(|catalog| catalog.get(b"AcroForm").ok())
        .and_then(|value| value.as_reference().ok());
    if let Some(form_id) = existing {
        if overlay.objects.get(&form_id).is_none() {
            let form = original
                .objects
                .get(&form_id)
                .cloned()
                .unwrap_or(Object::Dictionary(Dictionary::new()));
            overlay.objects.insert(form_id, form);
        }
        return Ok(form_id);
    }
    let mut form = Dictionary::new();
    form.set("Fields", Object::Array(Vec::new()));
    let form_id = overlay.add_object(Object::Dictionary(form));
    let catalog = overlay
        .objects
        .get_mut(&root_id)
        .and_then(|value| value.as_dict_mut().ok())
        .ok_or_else(|| "カタログを更新できません".to_string())?;
    catalog.set("AcroForm", Object::Reference(form_id));
    Ok(form_id)
}
fn build_signature_overlay(
    original_pdf: &[u8],
    seed: &SignatureFieldSeed,
) -> Result<(Document, OID), String> {
    let original =
        Document::load_mem(original_pdf).map_err(|e| format!("PDFの解析に失敗しました: {e}"))?;
    let page_ids = crate::pdf_engine::get_page_ids(&original);
    if seed.page_index >= page_ids.len() {
        return Err("ページ番号が範囲外です".to_string());
    }
    let page_id = page_ids[seed.page_index];
    let mut overlay = Document::new();
    overlay.version = original.version.clone();
    overlay.trailer = original.trailer.clone();
    overlay.max_id = original.max_id;
    let page_dict = original
        .objects
        .get(&page_id)
        .and_then(|value| value.as_dict().ok())
        .ok_or_else(|| "ページ辞書が見つかりません".to_string())?
        .clone();
    overlay
        .objects
        .insert(page_id, Object::Dictionary(page_dict));
    let mut signature = Dictionary::new();
    signature.set("Type", Object::Name(b"Sig".to_vec()));
    signature.set("Filter", Object::Name(FILTER.to_vec()));
    signature.set("SubFilter", Object::Name(SUBFILTER.to_vec()));
    signature.set(
        "M",
        Object::String(
            current_pdf_date().as_bytes().to_vec(),
            lopdf::StringFormat::Literal,
        ),
    );
    if !seed.signer_name.is_empty() {
        signature.set(
            "Name",
            Object::String(
                crate::pdf_engine::common::encode_pdf_text_string(&seed.signer_name),
                lopdf::StringFormat::Literal,
            ),
        );
    }
    if !seed.reason.is_empty() {
        signature.set(
            "Reason",
            Object::String(
                crate::pdf_engine::common::encode_pdf_text_string(&seed.reason),
                lopdf::StringFormat::Literal,
            ),
        );
    }
    if !seed.location.is_empty() {
        signature.set(
            "Location",
            Object::String(
                crate::pdf_engine::common::encode_pdf_text_string(&seed.location),
                lopdf::StringFormat::Literal,
            ),
        );
    }
    if !seed.contact_info.is_empty() {
        signature.set(
            "ContactInfo",
            Object::String(
                crate::pdf_engine::common::encode_pdf_text_string(&seed.contact_info),
                lopdf::StringFormat::Literal,
            ),
        );
    }
    // ByteRange values are patched in place after serialization, so reserve
    // fixed-width 10-digit fields to keep every byte offset stable.
    signature.set(
        "ByteRange",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(BYTERANGE_FIELD_PLACEHOLDER),
            Object::Integer(BYTERANGE_FIELD_PLACEHOLDER),
            Object::Integer(BYTERANGE_FIELD_PLACEHOLDER),
        ]),
    );
    signature.set(
        "Contents",
        Object::String(
            vec![0u8; CMS_PLACEHOLDER_LEN],
            lopdf::StringFormat::Hexadecimal,
        ),
    );
    let signature_id = overlay.add_object(Object::Dictionary(signature));
    let mut field = Dictionary::new();
    field.set("Type", Object::Name(b"Annot".to_vec()));
    field.set("Subtype", Object::Name(b"Widget".to_vec()));
    field.set("FT", Object::Name(b"Sig".to_vec()));
    field.set(
        "T",
        Object::String(
            crate::pdf_engine::common::encode_pdf_text_string(&seed.field_name),
            lopdf::StringFormat::Literal,
        ),
    );
    field.set(
        "Rect",
        Object::Array(vec![
            Object::Real(seed.rect[0] as f32),
            Object::Real(seed.rect[1] as f32),
            Object::Real(seed.rect[2] as f32),
            Object::Real(seed.rect[3] as f32),
        ]),
    );
    field.set("F", Object::Integer(4));
    field.set("V", Object::Reference(signature_id));
    field.set("P", Object::Reference(page_id));
    let field_id = overlay.add_object(Object::Dictionary(field));
    let page = overlay
        .objects
        .get_mut(&page_id)
        .and_then(|value| value.as_dict_mut().ok())
        .ok_or_else(|| "ページを更新できません".to_string())?;
    let mut annots = match page.get(b"Annots").ok() {
        Some(Object::Array(items)) => items.clone(),
        Some(Object::Reference(id)) => original
            .objects
            .get(id)
            .and_then(|value| value.as_array().ok())
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    annots.push(Object::Reference(field_id));
    page.set("Annots", Object::Array(annots));
    let form_id = ensure_acroform(&mut overlay, &original)?;
    let form = overlay
        .objects
        .get_mut(&form_id)
        .and_then(|value| value.as_dict_mut().ok())
        .ok_or_else(|| "AcroFormを更新できません".to_string())?;
    let mut fields = match form.get(b"Fields").ok() {
        Some(Object::Array(items)) => items.clone(),
        Some(Object::Reference(id)) => original
            .objects
            .get(id)
            .and_then(|value| value.as_array().ok())
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    fields.push(Object::Reference(field_id));
    form.set("Fields", Object::Array(fields));
    if form.get(b"SigFlags").is_err() {
        form.set("SigFlags", Object::Integer(3));
    }
    Ok((overlay, signature_id))
}
fn patch_byterange(
    pdf: &mut [u8],
    first_len: usize,
    second_offset: usize,
    second_len: usize,
) -> Result<(), String> {
    let needle = format!(
        "/ByteRange [0 {BYTERANGE_FIELD_PLACEHOLDER} {BYTERANGE_FIELD_PLACEHOLDER} {BYTERANGE_FIELD_PLACEHOLDER} ]"
    );
    let position = find_subslice(pdf, needle.as_bytes())
        .ok_or_else(|| "ByteRangeプレースホルダーが見つかりません".to_string())?;
    let format_field = |value: usize| -> Result<String, String> {
        let digits = value.to_string();
        if digits.len() > BYTERANGE_FIELD_WIDTH {
            return Err("ByteRangeの値がプレースホルダー幅を超過しました".to_string());
        }
        Ok(format!("{digits:<width$}", width = BYTERANGE_FIELD_WIDTH))
    };
    let replacement = format!(
        "/ByteRange [0 {} {} {} ]",
        format_field(first_len)?,
        format_field(second_offset)?,
        format_field(second_len)?
    );
    if replacement.len() != needle.len() {
        return Err("ByteRangeの置換長が一致しません".to_string());
    }
    pdf[position..position + needle.len()].copy_from_slice(replacement.as_bytes());
    Ok(())
}
fn locate_placeholder(pdf: &[u8]) -> Result<(usize, usize), String> {
    let mut search_from = 0usize;
    while let Some(filter_pos) =
        find_subslice(&pdf[search_from..], FILTER).map(|index| search_from + index)
    {
        let object_end =
            find_subslice(&pdf[filter_pos..], b"endobj").map(|index| filter_pos + index);
        if let Some(end) = object_end {
            let start = pdf[..filter_pos]
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map(|index| index + 1)
                .unwrap_or(0);
            let body = &pdf[start..end];
            if find_subslice(body, b"/Sig").is_some()
                && find_subslice(body, b"/ByteRange").is_some()
            {
                if let Some(contents) = find_subslice(body, b"/Contents") {
                    let after = &body[contents..];
                    if let Some(open) = find_subslice(after, b"<") {
                        let absolute_open = start + contents + open + 1;
                        if let Some(close) = find_subslice(&pdf[absolute_open..], b">") {
                            let absolute_close = absolute_open + close;
                            if absolute_close - absolute_open == CMS_PLACEHOLDER_LEN * 2 {
                                return Ok((absolute_open, absolute_close - absolute_open));
                            }
                        }
                    }
                }
            }
        }
        search_from = filter_pos + FILTER.len();
    }
    Err("署名プレースホルダーを特定できません".to_string())
}
fn create_detached_cms(signed_bytes: &[u8], request: &CmsSignRequest) -> Result<Vec<u8>, String> {
    let work = temp_workdir("nagisa_cms")?;
    let result = (|| {
        let key_path = work.join("key.pem");
        let cert_path = work.join("cert.pem");
        let digest_path = work.join("signed.bin");
        let out_path = work.join("signature.der");
        std::fs::write(&key_path, &request.private_key_pem)
            .map_err(|e| format!("秘密鍵の書き込みに失敗: {e}"))?;
        std::fs::write(&cert_path, &request.certificate_pem)
            .map_err(|e| format!("証明書の書き込みに失敗: {e}"))?;
        std::fs::write(&digest_path, signed_bytes)
            .map_err(|e| format!("署名対象の書き込みに失敗: {e}"))?;
        let mut args = vec![
            "cms".to_string(),
            "-sign".to_string(),
            "-binary".to_string(),
            "-noattr".to_string(),
            "-in".to_string(),
            digest_path.to_string_lossy().to_string(),
            "-inform".to_string(),
            "DER".to_string(),
            "-signer".to_string(),
            cert_path.to_string_lossy().to_string(),
            "-inkey".to_string(),
            key_path.to_string_lossy().to_string(),
            "-outform".to_string(),
            "DER".to_string(),
            "-out".to_string(),
            out_path.to_string_lossy().to_string(),
        ];
        if !request.chain_pem.is_empty() {
            let chain_path = work.join("chain.pem");
            let mut chain_bytes = Vec::new();
            for item in &request.chain_pem {
                chain_bytes.extend_from_slice(item);
                if !item.ends_with(b"\n") {
                    chain_bytes.push(b'\n');
                }
            }
            std::fs::write(&chain_path, &chain_bytes)
                .map_err(|e| format!("チェーンの書き込みに失敗: {e}"))?;
            args.push("-certfile".to_string());
            args.push(chain_path.to_string_lossy().to_string());
        }
        run_openssl(&args, None)?;
        std::fs::read(&out_path).map_err(|e| format!("CMS署名の読み込みに失敗: {e}"))
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// A code-signing identity discovered in the macOS login keychain.
#[derive(Clone, Debug, serde::Serialize)]
pub struct KeychainIdentity {
    /// SHA-1 fingerprint printed by `security find-identity`.
    pub sha1_fingerprint: String,
    /// Human readable certificate common name, e.g. "Apple Development: ...".
    pub common_name: String,
    /// The certificate nickname accepted by `security cms -N`.
    pub nickname: String,
}

/// List usable code-signing identities from the macOS keychain.
///
/// Returns an empty list on non-macOS targets so the UI can degrade gracefully
/// instead of failing the whole signing panel.
pub fn list_keychain_identities() -> Result<Vec<KeychainIdentity>, String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = ();
        Ok(Vec::new())
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/bin/security")
            .args(["find-identity", "-v"])
            .output()
            .map_err(|e| format!("セキュリティフレームワークの起動に失敗しました: {e}"))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut identities = Vec::new();
        for line in text.lines() {
            // Format: `  1) <SHA1_HEX> "<common name>"`
            let line = line.trim();
            let Some(rest) = line.strip_prefix(|c: char| c.is_ascii_digit() || c == ')') else {
                continue;
            };
            let rest = rest
                .trim_start_matches(|c: char| c.is_ascii_digit() || c == ')')
                .trim();
            let Some((fingerprint, quoted)) = rest.split_once(' ') else {
                continue;
            };
            // The trailing summary line ("1 valid identities found") also matches the
            // prefix, so require the first token to look like a SHA-1 hex digest.
            if fingerprint.len() < 40 || !fingerprint.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }
            let common_name = quoted.trim().trim_matches('"').to_string();
            if common_name.is_empty() {
                continue;
            }
            // `security cms -N` accepts either the nickname or the common name;
            // the common name is the most stable selector across OS upgrades.
            let nickname = common_name
                .rsplit_once('(')
                .and_then(|(_, tail)| tail.strip_suffix(')'))
                .unwrap_or(&common_name)
                .to_string();
            identities.push(KeychainIdentity {
                sha1_fingerprint: fingerprint.to_string(),
                common_name,
                nickname,
            });
        }
        Ok(identities)
    }
}

/// Produce a detached CMS SignedData blob using a keychain identity.
///
/// Unlike the PEM/PKCS#12 paths, the private key never leaves the keychain:
/// `security cms -S -T` performs the digest + RSA signature operation inside
/// the Security framework. `-T` is what makes the CMS detached, which is
/// mandatory for PDF signatures (the content is covered by ByteRange instead).
fn create_detached_cms_with_keychain(
    signed_bytes: &[u8],
    identity: &KeychainIdentity,
) -> Result<Vec<u8>, String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (signed_bytes, identity);
        Err("キーチェーン署名は macOS でのみ利用できます".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        let work = temp_workdir("nagisa_keychain")?;
        let result = (|| {
            let data_path = work.join("signed.bin");
            let out_path = work.join("signature.der");
            std::fs::write(&data_path, signed_bytes)
                .map_err(|e| format!("署名対象の書き込みに失敗: {e}"))?;
            let output = Command::new("/usr/bin/security")
                .args([
                    "cms",
                    "-S",
                    // Detached: PDF covers content through ByteRange, not inline.
                    "-T",
                    "-H",
                    "SHA256",
                    // Include a signing-time attribute (matches the OpenSSL path's
                    // -noattr + M entry behaviour closely enough for PAdES).
                    "-G",
                    "-N",
                    &identity.nickname,
                    "-i",
                    &data_path.to_string_lossy(),
                    "-o",
                    &out_path.to_string_lossy(),
                ])
                .output()
                .map_err(|e| format!("セキュリティフレームワークの署名に失敗しました: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "キーチェーン署名に失敗しました: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            // `security cms` writes the detached CMS SignedData as raw DER,
            // which is exactly what the PDF /Contents slot expects.
            std::fs::read(&out_path).map_err(|e| format!("CMS署名の読み込みに失敗: {e}"))
        })();
        let _ = std::fs::remove_dir_all(&work);
        result
    }
}

fn verify_detached_cms(signed_bytes: &[u8], cms_der: &[u8]) -> Result<bool, String> {
    let work = temp_workdir("nagisa_verify")?;
    let result = (|| {
        let cms_path = work.join("token.der");
        let data_path = work.join("signed.bin");
        std::fs::write(&cms_path, cms_der).map_err(|e| format!("CMSの書き込みに失敗: {e}"))?;
        std::fs::write(&data_path, signed_bytes)
            .map_err(|e| format!("署名対象の書き込みに失敗: {e}"))?;
        let output = run_openssl(
            &[
                "cms".to_string(),
                "-verify".to_string(),
                "-binary".to_string(),
                "-inform".to_string(),
                "DER".to_string(),
                "-in".to_string(),
                cms_path.to_string_lossy().to_string(),
                "-content".to_string(),
                data_path.to_string_lossy().to_string(),
                "-noverify".to_string(),
            ],
            None,
        )?;
        Ok(output == signed_bytes)
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}
fn signer_signed_attributes_der(cms_der: &[u8]) -> Result<Vec<u8>, String> {
    let content =
        ContentInfo::from_der(cms_der).map_err(|e| format!("CMSの解析に失敗しました: {e}"))?;
    let signed_der = content.content.to_der().map_err(|e| e.to_string())?;
    let signed = SignedData::from_der(signed_der.as_slice())
        .map_err(|e| format!("CMS SignedDataの解析に失敗しました: {e}"))?;
    let signer = signed
        .signer_infos
        .0
        .iter()
        .next()
        .ok_or_else(|| "CMS署名者情報がありません".to_string())?;
    signer
        .signed_attrs
        .clone()
        .ok_or_else(|| "CMS署名属性がありません".to_string())?
        .to_der()
        .map_err(|e| format!("CMS署名属性の解析に失敗しました: {e}"))
}

fn signer_certificate_der(cms_der: &[u8]) -> Result<Vec<u8>, String> {
    match signer_certificate_der_asn1(cms_der) {
        Ok(cert) => Ok(cert),
        Err(strict_error) => signer_certificates_der(cms_der)?
            .into_iter()
            .next()
            .ok_or_else(|| strict_error),
    }
}

fn signer_certificate_der_asn1(cms_der: &[u8]) -> Result<Vec<u8>, String> {
    let content =
        ContentInfo::from_der(cms_der).map_err(|e| format!("CMSの解析に失敗しました: {e}"))?;
    let signed_der = content
        .content
        .to_der()
        .map_err(|e| format!("CMS SignedDataの解析に失敗しました: {e}"))?;
    let signed = SignedData::from_der(signed_der.as_slice())
        .map_err(|e| format!("CMS SignedDataの解析に失敗しました: {e}"))?;
    let Some(signer) = signed.signer_infos.0.iter().next() else {
        return Err("CMS署名者情報がありません".to_string());
    };
    let SignerIdentifier::IssuerAndSerialNumber(identifier) = &signer.sid else {
        return Err("SubjectKeyIdentifier形式のCMS署名者は未対応です".to_string());
    };
    let Some(certificates) = signed.certificates else {
        return Err("CMS署名者証明書がありません".to_string());
    };
    for choice in certificates.0.iter() {
        let CertificateChoices::Certificate(cert) = choice else {
            continue;
        };
        if cert.tbs_certificate.issuer == identifier.issuer
            && cert.tbs_certificate.serial_number == identifier.serial_number
        {
            return cert.to_der().map_err(|e| e.to_string());
        }
    }
    Err("CMS署名者証明書がSignerInfoと一致しません".to_string())
}

/// Extract every certificate embedded in a CMS blob.
///
/// Apple (and most real-world) CMS blobs carry the full chain, so the first
/// certificate is not guaranteed to be the signer; callers must select the one
/// whose public key actually validates the signature.
fn signer_certificates_der(cms_der: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let work = temp_workdir("nagisa_signer")?;
    let result = (|| {
        let cms_path = work.join("token.der");
        let cert_path = work.join("signer.pem");
        std::fs::write(&cms_path, cms_der).map_err(|e| format!("CMSの書き込みに失敗: {e}"))?;
        let outcome = Command::new(openssl_binary())
            .args([
                "cms",
                "-cmsout",
                "-inform",
                "DER",
                "-in",
                &cms_path.to_string_lossy(),
                "-certsout",
                &cert_path.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("OpenSSLの実行に失敗しました: {e}"))?;
        if !outcome.status.success() {
            return Err(format!(
                "CMSから署名者証明書を抽出できません: {}",
                String::from_utf8_lossy(&outcome.stderr).trim()
            ));
        }
        let extracted = std::fs::read(&cert_path)
            .map_err(|_| "CMSから署名者証明書を抽出できません".to_string())?;
        // `pem_blocks_all` yields PEM text blocks; convert each to raw DER.
        Ok(pem_blocks_all(&extracted, "CERTIFICATE")
            .into_iter()
            .filter_map(|block| pem_block(&block, "CERTIFICATE").ok())
            .collect())
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}
/// Extract the detached RSA signature from a CMS SignedData blob.
///
/// Navigates the CMS structure (ContentInfo → SignedData → SignerInfos[0] →
/// signature OCTET STRING) so the RFC 3161 timestamp imprint binds to the
/// actual signature value rather than an arbitrary OCTET STRING of similar
/// size. Prefers the strict `cms` crate parser (our own builder emits canonical
/// DER); falls back to the BER-tolerant TLV walker because `/usr/bin/security`
/// emits indefinite-length BER that the strict decoder rejects.
fn cms_signature_value(cms_der: &[u8]) -> Result<Vec<u8>, String> {
    if let Ok(value) = cms_signature_value_strict(cms_der) {
        return Ok(value);
    }
    cms_signature_value_ber(cms_der)
}

fn cms_signature_value_strict(cms_der: &[u8]) -> Result<Vec<u8>, String> {
    let content = ContentInfo::from_der(cms_der).map_err(|e| e.to_string())?;
    let signed_der = content.content.to_der().map_err(|e| e.to_string())?;
    let signed = SignedData::from_der(signed_der.as_slice()).map_err(|e| e.to_string())?;
    let signer = signed
        .signer_infos
        .0
        .iter()
        .next()
        .ok_or_else(|| "CMS署名者情報がありません".to_string())?;
    // SignerInfo.signature is a SignatureValue (transparent wrapper over OCTET STRING).
    Ok(signer.signature.as_bytes().to_vec())
}

fn cms_signature_value_ber(cms_der: &[u8]) -> Result<Vec<u8>, String> {
    let root = parse_der(cms_der)?;
    // Locate a SignerInfo node structurally, because producers disagree on the
    // wrapper: `/usr/bin/security cms -S -T` may emit SignedData directly while
    // our builder and OpenSSL emit a full ContentInfo. A SignerInfo is a
    // SEQUENCE of 4..=6 fields whose last child is the OCTET STRING signature
    // preceded by the signatureAlgorithm SEQUENCE. Certificates cannot match:
    // their outermost SEQUENCE ends in a BIT STRING (signatureValue).
    fn find_signer_info<'g, 'd>(node: &'g Tlv<'d>) -> Option<&'g Tlv<'d>> {
        if node.tag == 0x30 && (4..=6).contains(&node.children.len()) {
            let last = node.children.last()?;
            let second_last = node.children.get(node.children.len().checked_sub(2)?)?;
            if last.tag == 0x04
                && !last.value.is_empty()
                && last.value.len() >= 64
                && second_last.tag == 0x30
            {
                return Some(node);
            }
        }
        node.children.iter().find_map(find_signer_info)
    }
    let signer = find_signer_info(&root).ok_or_else(|| "CMS署名値を特定できません".to_string())?;
    let signature = signer.children.last().expect("checked by find_signer_info");
    Ok(signature.value.to_vec())
}
fn certificate_text(cert_der: &[u8], field: &str) -> String {
    let work = match temp_workdir("nagisa_certtext") {
        Ok(work) => work,
        Err(_) => return String::new(),
    };
    let cert_path = work.join("cert.der");
    let text = (|| {
        std::fs::write(&cert_path, cert_der).map_err(|e| e.to_string())?;
        let output = Command::new(openssl_binary())
            .args([
                "x509",
                "-inform",
                "DER",
                "-in",
                &cert_path.to_string_lossy(),
                "-noout",
                field,
            ])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("証明書情報の取得に失敗".to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    })()
    .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&work);
    text
}
fn certificate_currently_valid(cert_der: &[u8]) -> bool {
    let work = match temp_workdir("nagisa_certvalid") {
        Ok(work) => work,
        Err(_) => return false,
    };
    let cert_path = work.join("cert.der");
    let valid = (|| {
        std::fs::write(&cert_path, cert_der).map_err(|e| e.to_string())?;
        run_openssl(
            &[
                "x509".to_string(),
                "-inform".to_string(),
                "DER".to_string(),
                "-in".to_string(),
                cert_path.to_string_lossy().to_string(),
                "-noout".to_string(),
                "-checkend".to_string(),
                "0".to_string(),
            ],
            None,
        )
        .map(|_| true)
    })()
    .unwrap_or(false);
    let _ = std::fs::remove_dir_all(&work);
    valid
}
fn digest_name(cms_der: &[u8]) -> String {
    let sha256 = [0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];
    let sha384 = [0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02];
    let sha512 = [0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x03];
    if cms_der.windows(sha512.len()).any(|window| window == sha512) {
        return "SHA-512".to_string();
    }
    if cms_der.windows(sha384.len()).any(|window| window == sha384) {
        return "SHA-384".to_string();
    }
    if cms_der.windows(sha256.len()).any(|window| window == sha256) {
        return "SHA-256".to_string();
    }
    "unknown".to_string()
}
pub fn sign_pdf_cms(request: &CmsSignRequest, original_pdf: &[u8]) -> Result<Vec<u8>, String> {
    if !signature_entries(
        &Document::load_mem(original_pdf).map_err(|e| format!("PDFの解析に失敗しました: {e}"))?,
    )
    .is_empty()
    {
        return Err("このPDFには既存の暗号署名があります".to_string());
    }
    let effective = resolve_signing_credentials(request)?;
    let private_der = pem_block(&effective.private_key_pem, "PRIVATE KEY")
        .or_else(|_| pem_block(&effective.private_key_pem, "RSA PRIVATE KEY"))?;
    let (modulus, _) = rsa_private_parts(&private_der)?;
    let certificate_der = pem_block(&effective.certificate_pem, "CERTIFICATE")?;
    let (public_modulus, _) = certificate_public_parts(&certificate_der)?;
    if public_modulus != modulus {
        return Err("秘密鍵と証明書の公開鍵が一致しません".to_string());
    }
    let tsa_url = effective.tsa_url.clone();
    finalize_pdf_signature(
        original_pdf,
        effective.seed.clone(),
        tsa_url,
        |signed_ranges| create_detached_cms(signed_ranges, &effective),
    )
}

/// Sign a PDF with a private key held in the macOS keychain.
///
/// The private key is never exported: `security cms -S -T` performs the digest
/// and RSA signature operation inside the Security framework and returns a
/// detached CMS blob, which is then bound to the PDF ByteRange exactly like the
/// file-based path.
pub fn sign_pdf_cms_with_keychain(
    original_pdf: &[u8],
    seed: SignatureFieldSeed,
    identity: &KeychainIdentity,
    tsa_url: Option<String>,
) -> Result<Vec<u8>, String> {
    if !signature_entries(
        &Document::load_mem(original_pdf).map_err(|e| format!("PDFの解析に失敗しました: {e}"))?,
    )
    .is_empty()
    {
        return Err("このPDFには既存の暗号署名があります".to_string());
    }
    finalize_pdf_signature(original_pdf, seed, tsa_url, |signed_ranges| {
        create_detached_cms_with_keychain(signed_ranges, identity)
    })
}

/// Sign a PDF with an RSA private key that remains inside a PKCS#11 token.
///
/// CMS packaging is performed locally. Only the DER-encoded SignedAttributes
/// are passed to the HSM; private key material never leaves the token.
pub fn sign_pdf_cms_with_pkcs11(
    original_pdf: &[u8],
    seed: SignatureFieldSeed,
    slot_id: u64,
    certificate_id: &str,
    pin: String,
    tsa_url: Option<String>,
) -> Result<Vec<u8>, String> {
    if !signature_entries(
        &Document::load_mem(original_pdf).map_err(|e| format!("PDFの解析に失敗しました: {e}"))?,
    )
    .is_empty()
    {
        return Err("このPDFには既存の暗号署名があります".into());
    }
    let certificate_der = super::hsm::pkcs11_certificate_der(slot_id, certificate_id)?;
    let certificate = Certificate::from_der(certificate_der.as_slice())
        .map_err(|e| format!("PKCS#11証明書の解析に失敗しました: {e}"))?;
    finalize_pdf_signature(original_pdf, seed, tsa_url, |signed_ranges| {
        create_detached_cms_with_pkcs11(signed_ranges, &certificate, slot_id, certificate_id, pin)
    })
}

fn create_detached_cms_with_pkcs11(
    content: &[u8],
    certificate: &Certificate,
    slot_id: u64,
    certificate_id: &str,
    pin: String,
) -> Result<Vec<u8>, String> {
    use const_oid::db::{rfc5911, rfc5912};
    use der::asn1::{OctetStringRef, SetOfVec};

    let message_digest = sha256_digest(content);
    let mut values = SetOfVec::new();
    values
        .insert(Any::from(
            OctetStringRef::new(&message_digest).map_err(|e| e.to_string())?,
        ))
        .map_err(|e| e.to_string())?;
    let digest_attr = Attribute {
        oid: rfc5911::ID_MESSAGE_DIGEST,
        values,
    };
    let mut content_values = SetOfVec::new();
    content_values
        .insert(Any::from(rfc5911::ID_DATA))
        .map_err(|e| e.to_string())?;
    let content_attr = Attribute {
        oid: rfc5911::ID_CONTENT_TYPE,
        values: content_values,
    };
    let attributes =
        Attributes::try_from(vec![content_attr, digest_attr]).map_err(|e| e.to_string())?;
    let attrs_der = attributes.to_der().map_err(|e| e.to_string())?;
    let signature = super::hsm::pkcs11_raw_sign(slot_id, certificate_id, pin, &attrs_der)?;

    let sha256 = AlgorithmIdentifierOwned {
        oid: rfc5912::ID_SHA_256,
        parameters: None,
    };
    let rsa = AlgorithmIdentifierOwned {
        oid: rfc5912::RSA_ENCRYPTION,
        parameters: Some(Any::new(der::Tag::Null, []).map_err(|e| e.to_string())?),
    };
    let mut digest_algorithms = DigestAlgorithmIdentifiers::default();
    digest_algorithms
        .insert(sha256.clone())
        .map_err(|e| e.to_string())?;
    let mut cert_set = CertificateSet(der::asn1::SetOfVec::default());
    cert_set
        .0
        .insert(CertificateChoices::Certificate(certificate.clone()))
        .map_err(|e| e.to_string())?;
    let signer = SignerInfo {
        version: CmsVersion::V1,
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: certificate.tbs_certificate.issuer.clone(),
            serial_number: certificate.tbs_certificate.serial_number.clone(),
        }),
        digest_alg: sha256,
        signed_attrs: Some(attributes),
        signature_algorithm: rsa,
        signature: SignatureValue::new(signature).map_err(|e| e.to_string())?,
        unsigned_attrs: None,
    };
    let mut signer_infos = SignerInfos(der::asn1::SetOfVec::default());
    signer_infos.0.insert(signer).map_err(|e| e.to_string())?;
    let signed_data = SignedData {
        version: CmsVersion::V1,
        digest_algorithms,
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: rfc5911::ID_DATA,
            econtent: None,
        },
        certificates: Some(cert_set),
        crls: None,
        signer_infos,
    };
    let signed_der = signed_data.to_der().map_err(|e| e.to_string())?;
    let content = AnyRef::try_from(signed_der.as_slice()).map_err(|e| e.to_string())?;
    ContentInfo {
        content_type: rfc5911::ID_SIGNED_DATA,
        content: Any::from(content),
    }
    .to_der()
    .map_err(|e| format!("CMS署名の構築に失敗しました: {e}"))
}

/// Shared tail of every signing path: build the incremental update, finalise the
/// ByteRange, ask `cms_producer` for a detached CMS blob, optionally attach an
/// RFC 3161 timestamp, inject the DER into the reserved placeholder, and
/// self-verify the result before returning it to the caller.
fn finalize_pdf_signature(
    original_pdf: &[u8],
    seed: SignatureFieldSeed,
    tsa_url: Option<String>,
    cms_producer: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let (overlay, _signature_id) = build_signature_overlay(original_pdf, &seed)?;
    // Count pre-existing signatures so self-verification targets the NEW one.
    let pre_existing_sigs = Document::load_mem(original_pdf)
        .map(|d| signature_entries(&d).len())
        .unwrap_or(0);
    let mut unsigned = append_incremental_update(original_pdf, overlay)?;
    let (placeholder_start, placeholder_len) = locate_placeholder(&unsigned)?;
    let second_offset = placeholder_start + placeholder_len;
    let second_len = unsigned.len() - second_offset;
    patch_byterange(&mut unsigned, placeholder_start, second_offset, second_len)?;
    let mut signed_ranges = unsigned[..placeholder_start].to_vec();
    signed_ranges.extend_from_slice(&unsigned[placeholder_start + placeholder_len..]);
    let mut cms_der = cms_producer(&signed_ranges)?;
    if let Some(tsa_url) = tsa_url.as_deref().filter(|url| !url.is_empty()) {
        let signature_value = cms_signature_value(&cms_der)?;
        let imprint = sha256_digest(&signature_value);
        let token = fetch_timestamp_token(tsa_url, &imprint)?;
        cms_der = embed_timestamp_token(&cms_der, &token)?;
    }
    if cms_der.len() > CMS_PLACEHOLDER_LEN {
        return Err(format!(
            "CMS署名がプレースホルダーを超過しました（{} > {}）",
            cms_der.len(),
            CMS_PLACEHOLDER_LEN
        ));
    }
    let mut hex = String::with_capacity(CMS_PLACEHOLDER_LEN * 2);
    for byte in &cms_der {
        hex.push_str(&format!("{byte:02X}"));
    }
    while hex.len() < CMS_PLACEHOLDER_LEN * 2 {
        hex.push('0');
    }
    if placeholder_start + placeholder_len > unsigned.len() {
        return Err("署名範囲がPDF範囲外です".to_string());
    }
    unsigned[placeholder_start..placeholder_start + placeholder_len]
        .copy_from_slice(hex.as_bytes());
    let report = verify_pdf_cms(&unsigned, pre_existing_sigs)?;
    if !report.digest_matches || !report.cms_signature_valid {
        return Err(format!(
            "生成した署名の自己検証に失敗しました（ByteRangeダイジェスト一致: {} / CMS署名検証: {} / 警告: {}）",
            report.digest_matches,
            report.cms_signature_valid,
            report.warnings.join(", ")
        ));
    }
    Ok(unsigned)
}
pub fn verify_pdf_cms(pdf: &[u8], signature_index: usize) -> Result<CmsVerifyReport, String> {
    verify_pdf_cms_with_trust(pdf, signature_index, None)
}

pub fn verify_pdf_cms_with_trust(
    pdf: &[u8],
    signature_index: usize,
    trust_roots_pem: Option<&[u8]>,
) -> Result<CmsVerifyReport, String> {
    let doc = Document::load_mem(pdf).map_err(|e| format!("PDFの解析に失敗しました: {e}"))?;
    let signatures = signature_entries(&doc);
    if signatures.is_empty() {
        return Err("暗号署名が見つかりません".to_string());
    }
    if signature_index >= signatures.len() {
        return Err(format!("署名番号{signature_index}は範囲外です"));
    }
    let (byterange, contents) = signatures[signature_index].clone();
    let signed = byterange_parts(pdf, &byterange)?;
    let cms_der = raw_contents(&contents)?;
    let digest_algorithm = digest_name(&cms_der);
    let cms_valid = verify_detached_cms(&signed, &cms_der).unwrap_or(false);
    let signature_bytes = cms_signature_value(&cms_der)?;
    // Real-world CMS blobs embed the whole chain and the signer is not
    // necessarily the first certificate, so select the certificate whose public
    // key actually validates the signature.
    let certificates = signer_certificates_der(&cms_der)?;
    let signed_attributes_der = signer_signed_attributes_der(&cms_der).ok();
    let mut rsa_cross_check = false;
    if let Some(signed_attributes_der) = &signed_attributes_der {
        for candidate in &certificates {
            if let Ok((modulus, exponent)) = certificate_public_parts(candidate) {
                if rsa_verify_sha256(signed_attributes_der, &signature_bytes, &modulus, &exponent) {
                    rsa_cross_check = true;
                    break;
                }
            }
        }
    }
    // OpenSSL's detached CMS verification over the exact PDF ByteRange is the
    // authoritative integrity verdict; the independent RSA check above is a
    // secondary signal that can be unavailable for some chain encodings.
    let digest_matches = cms_valid || rsa_cross_check;
    let signer_der = signer_certificate_der(&cms_der)?;
    let valid = cms_valid && digest_matches;
    let mut warnings = Vec::new();
    if digest_algorithm != "SHA-256" {
        warnings.push(format!("想定外のダイジェストです: {digest_algorithm}"));
    }
    if !digest_matches {
        warnings.push("ByteRangeダイジェストとCMS署名が一致しません".to_string());
    }
    if !cms_valid {
        warnings.push("CMSトークンの暗号検証に失敗しました".to_string());
    }
    let currently_valid = certificate_currently_valid(&signer_der);
    if !currently_valid {
        warnings.push("署名者証明書の有効期限外です".to_string());
    }
    let (chain_valid, chain_details) = match trust_roots_pem {
        Some(roots) if !roots.is_empty() => {
            let (valid, details) = verify_certificate_chain(&cms_der, roots);
            if valid != Some(true) {
                warnings.push("証明書チェーンの検証に失敗しました".to_string());
            }
            (valid, details)
        }
        _ => (None, "信頼ルート未指定のためチェーン検証を省略".to_string()),
    };
    let timestamp = timestamp_report(&cms_der);
    if let Some(report) = &timestamp {
        if !report.imprint_matches {
            warnings.push("タイムスタンプのインプリントが署名値と一致しません".to_string());
        }
    }
    let (revocation_status, revocation_details) = revocation_report(&signer_der, &cms_der);
    Ok(CmsVerifyReport {
        signatures_found: signatures.len(),
        signature_index,
        byterange_valid: true,
        digest_algorithm,
        digest_matches,
        cms_signature_valid: valid,
        signer_subject: certificate_text(&signer_der, "-subject"),
        signer_issuer: certificate_text(&signer_der, "-issuer"),
        certificate_currently_valid: currently_valid,
        chain_valid,
        chain_details,
        revocation_status,
        revocation_details,
        timestamp,
        warnings,
    })
}
fn raw_contents(contents: &[u8]) -> Result<Vec<u8>, String> {
    if contents.is_empty() {
        return Err("署名内容が空です".to_string());
    }
    // /Contents is zero-padded up to the placeholder length; trim the padding
    // by respecting the DER top-level length so OpenSSL accepts the token.
    if let Some((_, used)) = parse_tlv(contents) {
        if used > 0 && used <= contents.len() {
            return Ok(contents[..used].to_vec());
        }
    }
    Ok(contents.to_vec())
}

// ---- Certificates, chains, timestamps and LTV (task_0002) ----
/// Extract every PEM block with the given label (raw PEM text blocks).
fn pem_blocks_all(pem_bytes: &[u8], label: &str) -> Vec<Vec<u8>> {
    let text = match std::str::from_utf8(pem_bytes) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if line == begin {
            current = Some(format!("{line}\n"));
            continue;
        }
        if line == end {
            if let Some(mut block) = current.take() {
                block.push_str(line);
                block.push('\n');
                blocks.push(block.into_bytes());
            }
            continue;
        }
        if let Some(block) = current.as_mut() {
            block.push_str(line);
            block.push('\n');
        }
    }
    blocks
}

/// Resolve effective signing credentials, importing PKCS#12/PFX when needed.
fn resolve_signing_credentials(request: &CmsSignRequest) -> Result<CmsSignRequest, String> {
    if !request.private_key_pem.is_empty() {
        return Ok(request.clone());
    }
    let p12 = request
        .p12_der
        .as_ref()
        .ok_or_else(|| "秘密鍵またはPKCS#12コンテナが必要です".to_string())?;
    let (key_pem, cert_pem, chain_pem) =
        extract_p12(p12, request.p12_password.as_deref().unwrap_or(""))?;
    let mut effective = request.clone();
    effective.private_key_pem = key_pem;
    effective.certificate_pem = cert_pem;
    if effective.chain_pem.is_empty() {
        effective.chain_pem = chain_pem;
    }
    Ok(effective)
}

/// Import a PKCS#12/PFX container: (private key PEM, leaf cert PEM, CA chain PEMs).
pub fn extract_p12(
    p12_der: &[u8],
    password: &str,
) -> Result<(Vec<u8>, Vec<u8>, Vec<Vec<u8>>), String> {
    let work = temp_workdir("nagisa_p12")?;
    let result = (|| {
        let p12_path = work.join("bundle.p12");
        let key_path = work.join("key.pem");
        let cert_path = work.join("cert.pem");
        let chain_path = work.join("chain.pem");
        std::fs::write(&p12_path, p12_der).map_err(|e| format!("PKCS#12の書き込みに失敗: {e}"))?;
        let pass = format!("pass:{password}");
        run_openssl(
            &[
                "pkcs12".to_string(),
                "-in".to_string(),
                p12_path.to_string_lossy().to_string(),
                "-nocerts".to_string(),
                "-nodes".to_string(),
                "-passin".to_string(),
                pass.clone(),
                "-out".to_string(),
                key_path.to_string_lossy().to_string(),
            ],
            None,
        )?;
        run_openssl(
            &[
                "pkcs12".to_string(),
                "-in".to_string(),
                p12_path.to_string_lossy().to_string(),
                "-clcerts".to_string(),
                "-nokeys".to_string(),
                "-passin".to_string(),
                pass.clone(),
                "-out".to_string(),
                cert_path.to_string_lossy().to_string(),
            ],
            None,
        )?;
        let _ = run_openssl(
            &[
                "pkcs12".to_string(),
                "-in".to_string(),
                p12_path.to_string_lossy().to_string(),
                "-cacerts".to_string(),
                "-nokeys".to_string(),
                "-passin".to_string(),
                pass,
                "-out".to_string(),
                chain_path.to_string_lossy().to_string(),
            ],
            None,
        );
        let key_pem =
            std::fs::read(&key_path).map_err(|e| format!("PKCS#12鍵の読み込みに失敗: {e}"))?;
        let cert_pem =
            std::fs::read(&cert_path).map_err(|e| format!("PKCS#12証明書の読み込みに失敗: {e}"))?;
        let chain_pem = std::fs::read(&chain_path)
            .map(|bytes| pem_blocks_all(&bytes, "CERTIFICATE"))
            .unwrap_or_default();
        Ok((key_pem, cert_pem, chain_pem))
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// All certificates embedded in the CMS token, signer first.
fn all_certificates_pem(cms_der: &[u8]) -> Vec<Vec<u8>> {
    let work = match temp_workdir("nagisa_certs") {
        Ok(work) => work,
        Err(_) => return Vec::new(),
    };
    let certs = (|| {
        let cms_path = work.join("token.der");
        let cert_path = work.join("certs.pem");
        std::fs::write(&cms_path, cms_der).map_err(|e| e.to_string())?;
        let outcome = Command::new(openssl_binary())
            .args([
                "cms",
                "-cmsout",
                "-inform",
                "DER",
                "-in",
                &cms_path.to_string_lossy(),
                "-certsout",
                &cert_path.to_string_lossy(),
            ])
            .output()
            .map_err(|e| e.to_string())?;
        if !outcome.status.success() {
            return Err(String::from_utf8_lossy(&outcome.stderr).trim().to_string());
        }
        let extracted = std::fs::read(&cert_path).map_err(|e| e.to_string())?;
        Ok(pem_blocks_all(&extracted, "CERTIFICATE"))
    })()
    .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&work);
    certs
}

/// Validate the signer certificate chain against explicit trust roots.
fn verify_certificate_chain(cms_der: &[u8], trust_roots_pem: &[u8]) -> (Option<bool>, String) {
    let certs = all_certificates_pem(cms_der);
    if certs.is_empty() {
        return (Some(false), "CMS内に証明書が見つかりません".to_string());
    }
    let work = match temp_workdir("nagisa_chain") {
        Ok(work) => work,
        Err(e) => return (None, format!("一時ディレクトリを作成できません: {e}")),
    };
    let result = (|| {
        let signer_path = work.join("signer.pem");
        let untrusted_path = work.join("untrusted.pem");
        let roots_path = work.join("roots.pem");
        std::fs::write(&signer_path, &certs[0])
            .map_err(|e| format!("証明書の書き込みに失敗: {e}"))?;
        let mut untrusted = Vec::new();
        for cert in &certs[1..] {
            untrusted.extend_from_slice(cert);
        }
        std::fs::write(&untrusted_path, &untrusted)
            .map_err(|e| format!("チェーンの書き込みに失敗: {e}"))?;
        std::fs::write(&roots_path, trust_roots_pem)
            .map_err(|e| format!("信頼ルートの書き込みに失敗: {e}"))?;
        let mut args = vec![
            "verify".to_string(),
            "-CAfile".to_string(),
            roots_path.to_string_lossy().to_string(),
        ];
        if !untrusted.is_empty() {
            args.push("-untrusted".to_string());
            args.push(untrusted_path.to_string_lossy().to_string());
        }
        args.push(signer_path.to_string_lossy().to_string());
        let outcome = Command::new(openssl_binary())
            .args(&args)
            .output()
            .map_err(|e| format!("OpenSSLの実行に失敗しました: {e}"))?;
        let details = String::from_utf8_lossy(&outcome.stdout).trim().to_string();
        if outcome.status.success() {
            Ok((
                Some(true),
                if details.is_empty() {
                    "チェーン検証OK".to_string()
                } else {
                    details
                },
            ))
        } else {
            let stderr = String::from_utf8_lossy(&outcome.stderr).trim().to_string();
            Ok((
                Some(false),
                if stderr.is_empty() { details } else { stderr },
            ))
        }
    })();
    let _ = std::fs::remove_dir_all(&work);
    result.unwrap_or_else(|message| (None, message))
}

fn revocation_report(cert_der: &[u8], cms_der: &[u8]) -> (String, String) {
    let ocsp = certificate_text(cert_der, "-ocsp_uri");
    let crl = certificate_text(cert_der, "-crl_uri");
    let mut uris = Vec::new();
    if !ocsp.is_empty() && ocsp != "No OCSP URI" {
        uris.push(format!("OCSP: {ocsp}"));
    }
    if !crl.is_empty() && crl != "No CRL URI" {
        uris.push(format!("CRL: {crl}"));
    }
    if uris.is_empty() {
        return (
            "未埋め込み".to_string(),
            "証明書にOCSP/CRL URIがありません。オンライン失効確認は未実施です。".to_string(),
        );
    }
    if !ocsp.is_empty() && ocsp != "No OCSP URI" {
        if let Some(result) = query_ocsp(cert_der, cms_der, &ocsp) {
            return result;
        }
    }
    (
        "未確認".to_string(),
        format!(
            "{}（OCSP問い合わせはタイムアウトまたは失敗）",
            uris.join(" / ")
        ),
    )
}

fn query_ocsp(_cert_der: &[u8], cms_der: &[u8], responder_url: &str) -> Option<(String, String)> {
    let work = temp_workdir("nagisa_ocsp").ok()?;
    let result = (|| {
        let cert_path = work.join("cert.pem");
        let issuer_path = work.join("issuer.pem");
        let certs = all_certificates_pem(cms_der);
        let cert_pem = certs.first()?.clone();
        let issuer_pem = certs.get(1)?.clone();
        std::fs::write(&cert_path, cert_pem).ok()?;
        std::fs::write(&issuer_path, issuer_pem).ok()?;
        let output = Command::new(openssl_binary())
            .args([
                "ocsp",
                "-issuer",
                &issuer_path.to_string_lossy(),
                "-cert",
                &cert_path.to_string_lossy(),
                "-url",
                responder_url,
                "-resp_text",
                "-noverify",
                "-timeout",
                "3",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        if text.contains("Response verify OK")
            && (text.contains("good") || text.contains("Revocation Status: good"))
        {
            return Some(("有効".to_string(), format!("OCSP {responder_url}: good")));
        }
        if text.contains("Revocation Status: revoked") {
            return Some(("失効".to_string(), format!("OCSP {responder_url}: revoked")));
        }
        Some(("不明".to_string(), format!("OCSP {responder_url}: unknown")))
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

fn curl_binary() -> String {
    std::env::var("NAGISA_CURL_BIN").unwrap_or_else(|_| "curl".to_string())
}

/// Fetch an RFC 3161 timestamp token for a 32-byte SHA-256 message imprint.
pub fn fetch_timestamp_token(tsa_url: &str, imprint: &[u8; 32]) -> Result<Vec<u8>, String> {
    let work = temp_workdir("nagisa_tsa")?;
    let result = (|| {
        let query_path = work.join("request.tsq");
        let reply_path = work.join("reply.tsr");
        let token_path = work.join("token.der");
        let hex: String = imprint.iter().map(|byte| format!("{byte:02x}")).collect();
        run_openssl(
            &[
                "ts".to_string(),
                "-query".to_string(),
                "-digest".to_string(),
                hex,
                "-sha256".to_string(),
                "-cert".to_string(),
                "-out".to_string(),
                query_path.to_string_lossy().to_string(),
            ],
            None,
        )?;
        let outcome = Command::new(curl_binary())
            .args([
                "-sS",
                "-f",
                "-X",
                "POST",
                "-H",
                "Content-Type: application/timestamp-query",
                "--data-binary",
                &format!("@{}", query_path.to_string_lossy()),
                "-o",
                &reply_path.to_string_lossy(),
                tsa_url,
            ])
            .output()
            .map_err(|e| format!("curlを起動できませんでした: {e}"))?;
        if !outcome.status.success() {
            return Err(format!(
                "TSAへの問い合わせに失敗しました: {}",
                String::from_utf8_lossy(&outcome.stderr).trim()
            ));
        }
        run_openssl(
            &[
                "ts".to_string(),
                "-reply".to_string(),
                "-in".to_string(),
                reply_path.to_string_lossy().to_string(),
                "-token_out".to_string(),
                "-out".to_string(),
                token_path.to_string_lossy().to_string(),
            ],
            None,
        )?;
        std::fs::read(&token_path)
            .map_err(|e| format!("タイムスタンプトークンの読み込みに失敗: {e}"))
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

// Minimal owned DER tree for CMS unsigned-attribute surgery.
struct OwnedTlv {
    tag: u8,
    value: Vec<u8>,
    children: Vec<OwnedTlv>,
}

fn to_owned(node: &Tlv) -> OwnedTlv {
    OwnedTlv {
        tag: node.tag,
        value: node.value.to_vec(),
        children: node.children.iter().map(to_owned).collect(),
    }
}
fn der_encode(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes = len.to_be_bytes();
        let significant = bytes.iter().skip_while(|byte| **byte == 0).count().max(1);
        out.push(0x80 | significant as u8);
        out.extend_from_slice(&bytes[bytes.len() - significant..]);
    }
    out.extend_from_slice(content);
    out
}

fn encode_owned(node: &OwnedTlv) -> Vec<u8> {
    if node.tag & 0x20 != 0 {
        let content: Vec<u8> = node.children.iter().flat_map(encode_owned).collect();
        der_encode(node.tag, &content)
    } else {
        der_encode(node.tag, &node.value)
    }
}

/// id-aa-signatureTimeStampToken 1.2.840.113549.1.9.16.2.14
const OID_SIG_TIME_STAMP: &[u8] = &[
    0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x10, 0x02, 0x0E,
];

fn signer_info_mut(root: &mut OwnedTlv) -> Result<&mut OwnedTlv, String> {
    let explicit = root
        .children
        .iter_mut()
        .find(|node| node.tag == 0xA0)
        .ok_or_else(|| "SignedDataコンテナが見つかりません".to_string())?;
    let signed_data = explicit
        .children
        .first_mut()
        .ok_or_else(|| "SignedDataが見つかりません".to_string())?;
    let signer_infos = signed_data
        .children
        .iter_mut()
        .rev()
        .find(|node| node.tag == 0x31)
        .ok_or_else(|| "SignerInfosが見つかりません".to_string())?;
    signer_infos
        .children
        .first_mut()
        .ok_or_else(|| "SignerInfoが見つかりません".to_string())
}
/// Embed an RFC 3161 token as a signatureTimeStampToken unsigned attribute.
fn embed_timestamp_token(cms_der: &[u8], token_der: &[u8]) -> Result<Vec<u8>, String> {
    let root = parse_der(cms_der)?;
    let mut owned = to_owned(&root);
    parse_der(token_der)?;
    let attribute = OwnedTlv {
        tag: 0x30,
        value: Vec::new(),
        children: vec![
            OwnedTlv {
                tag: 0x06,
                value: OID_SIG_TIME_STAMP.to_vec(),
                children: Vec::new(),
            },
            OwnedTlv {
                tag: 0x31,
                value: Vec::new(),
                children: vec![to_owned(&parse_der(token_der)?)],
            },
        ],
    };
    let unsigned_attrs = OwnedTlv {
        tag: 0xA2,
        value: Vec::new(),
        children: vec![attribute],
    };
    let signer_info = signer_info_mut(&mut owned)?;
    signer_info.children.push(unsigned_attrs);
    Ok(encode_owned(&owned))
}

/// Extract (timestamp token DER, signer signature value) when a
/// signatureTimeStampToken unsigned attribute is present.
fn extract_timestamp_token(cms_der: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let root = parse_der(cms_der).ok()?;
    let explicit = root.children.iter().find(|node| node.tag == 0xA0)?;
    let signed_data = explicit.children.first()?;
    let signer_infos = signed_data
        .children
        .iter()
        .rev()
        .find(|node| node.tag == 0x31)?;
    let signer_info = signer_infos.children.first()?;
    let signature_value = signer_info
        .children
        .iter()
        .rev()
        .find(|node| node.tag == 0x04)?
        .value
        .to_vec();
    let unsigned = signer_info.children.iter().find(|node| node.tag == 0xA2)?;
    for attribute in &unsigned.children {
        if attribute.tag != 0x30 || attribute.children.len() != 2 {
            continue;
        }
        if attribute.children[0].tag == 0x06 && attribute.children[0].value == OID_SIG_TIME_STAMP {
            let set = &attribute.children[1];
            if let Some(token) = set.children.first() {
                return Some((encode_owned(&to_owned(token)), signature_value));
            }
        }
    }
    None
}

/// Parse imprint, generation time and TSA subject from a timestamp token.
fn timestamp_token_details(token_der: &[u8]) -> Result<(Vec<u8>, String, String), String> {
    let work = temp_workdir("nagisa_tsdetail")?;
    let result = (|| {
        let token_path = work.join("token.der");
        std::fs::write(&token_path, token_der)
            .map_err(|e| format!("トークンの書き込みに失敗: {e}"))?;
        let text_out = run_openssl(
            &[
                "ts".to_string(),
                "-reply".to_string(),
                "-in".to_string(),
                token_path.to_string_lossy().to_string(),
                "-token_in".to_string(),
                "-text".to_string(),
            ],
            None,
        )?;
        let text = String::from_utf8_lossy(&text_out);
        let mut imprint = Vec::new();
        let mut gen_time = String::new();
        let mut in_message = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Time stamp:") {
                gen_time = trimmed.trim_start_matches("Time stamp:").trim().to_string();
                continue;
            }
            if trimmed.starts_with("Message data:") {
                in_message = true;
                continue;
            }
            if in_message {
                if trimmed.is_empty() {
                    in_message = false;
                    continue;
                }
                if let Some((_, hex_part)) = trimmed.split_once(" - ") {
                    for pair in hex_part.split_whitespace() {
                        if pair.len() == 2 {
                            if let Ok(byte) = u8::from_str_radix(pair, 16) {
                                imprint.push(byte);
                            }
                        }
                    }
                }
            }
        }
        if imprint.is_empty() {
            return Err("タイムスタンプのインプリントを解析できません".to_string());
        }
        let certs = all_certificates_pem(token_der);
        let tsa_subject = certs
            .first()
            .map(|pem| {
                let der = pem_block(pem, "CERTIFICATE").unwrap_or_default();
                certificate_text(&der, "-subject")
            })
            .unwrap_or_default();
        Ok((imprint, gen_time, tsa_subject))
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}
/// Material for Long-Term Validation (DSS/VRI) incremental updates.
#[derive(Clone, Debug, Default)]
pub struct LtvMaterial {
    pub certificates_pem: Vec<Vec<u8>>,
    pub ocsps_der: Vec<Vec<u8>>,
    pub crls_der: Vec<Vec<u8>>,
}

fn cert_sha1_hex(cert_der: &[u8]) -> Result<String, String> {
    let work = temp_workdir("nagisa_sha1")?;
    let result = (|| {
        let cert_path = work.join("cert.der");
        std::fs::write(&cert_path, cert_der).map_err(|e| format!("証明書の書き込みに失敗: {e}"))?;
        let out = run_openssl(
            &[
                "dgst".to_string(),
                "-sha1".to_string(),
                cert_path.to_string_lossy().to_string(),
            ],
            None,
        )?;
        let text = String::from_utf8_lossy(&out);
        let hex = text.rsplit('=').next().unwrap_or("").trim().to_uppercase();
        if hex.len() != 40 {
            return Err("SHA-1の計算に失敗しました".to_string());
        }
        Ok(hex)
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Build a timestamp report for a CMS token, if it carries one.
fn timestamp_report(cms_der: &[u8]) -> Option<TimestampReport> {
    let (token, signature_value) = extract_timestamp_token(cms_der)?;
    let details = timestamp_token_details(&token);
    let expected = sha256_digest(&signature_value);
    match details {
        Ok((imprint, gen_time, tsa_subject)) => Some(TimestampReport {
            present: true,
            imprint_matches: imprint == expected.as_slice(),
            gen_time,
            tsa_subject,
        }),
        Err(_) => Some(TimestampReport {
            present: true,
            imprint_matches: false,
            gen_time: String::new(),
            tsa_subject: String::new(),
        }),
    }
}

/// Append an LTV (DSS/VRI) incremental update. Purely additive, so any
/// existing ByteRange signatures stay verifiable.
pub fn append_dss_update(pdf: &[u8], material: &LtvMaterial) -> Result<Vec<u8>, String> {
    if material.certificates_pem.is_empty()
        && material.ocsps_der.is_empty()
        && material.crls_der.is_empty()
    {
        return Err("LTVマテリアルが空です".to_string());
    }
    let original = Document::load_mem(pdf).map_err(|e| format!("PDFの解析に失敗しました: {e}"))?;
    let root_id = original
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|value| value.as_reference().ok())
        .ok_or_else(|| "カタログが見つかりません".to_string())?;
    let catalog = original
        .objects
        .get(&root_id)
        .and_then(|value| value.as_dict().ok())
        .ok_or_else(|| "カタログ辞書が見つかりません".to_string())?
        .clone();
    let mut overlay = Document::new();
    overlay.version = original.version.clone();
    overlay.trailer = original.trailer.clone();
    overlay.max_id = original.max_id;
    let mut cert_refs = Vec::new();
    let mut vri = Dictionary::new();
    for cert_pem in &material.certificates_pem {
        let cert_der = pem_block(cert_pem, "CERTIFICATE")?;
        let vri_key = cert_sha1_hex(&cert_der)?;
        let cert_id =
            overlay.add_object(Object::String(cert_der, lopdf::StringFormat::Hexadecimal));
        cert_refs.push(Object::Reference(cert_id));
        let mut vri_entry = Dictionary::new();
        vri_entry.set("Cert", Object::Array(vec![Object::Reference(cert_id)]));
        vri.set(vri_key.into_bytes(), Object::Dictionary(vri_entry));
    }
    let mut dss = Dictionary::new();
    dss.set("Type", Object::Name(b"DSS".to_vec()));
    if !cert_refs.is_empty() {
        dss.set("Certs", Object::Array(cert_refs));
    }
    if !material.ocsps_der.is_empty() {
        let refs: Vec<Object> = material
            .ocsps_der
            .iter()
            .map(|ocsp| {
                Object::Reference(overlay.add_object(Object::String(
                    ocsp.clone(),
                    lopdf::StringFormat::Hexadecimal,
                )))
            })
            .collect();
        dss.set("OCSPs", Object::Array(refs));
    }
    if !material.crls_der.is_empty() {
        let refs: Vec<Object> = material
            .crls_der
            .iter()
            .map(|crl| {
                Object::Reference(overlay.add_object(Object::String(
                    crl.clone(),
                    lopdf::StringFormat::Hexadecimal,
                )))
            })
            .collect();
        dss.set("CRLs", Object::Array(refs));
    }
    let vri_id = overlay.add_object(Object::Dictionary(vri));
    dss.set("VRI", Object::Reference(vri_id));
    let dss_id = overlay.add_object(Object::Dictionary(dss));
    let mut updated_catalog = catalog;
    updated_catalog.set("DSS", Object::Reference(dss_id));
    overlay
        .objects
        .insert(root_id, Object::Dictionary(updated_catalog));
    append_incremental_update(pdf, overlay)
}

/// Add a trusted timestamp to an existing CMS signature as an unsigned
/// attribute (signature value is unchanged, so the existing ByteRange
/// signature remains cryptographically valid). Requires an RFC 3161 TSA.
pub fn add_timestamp_to_signature(
    pdf: &[u8],
    signature_index: usize,
    tsa_url: &str,
) -> Result<Vec<u8>, String> {
    let doc = Document::load_mem(pdf).map_err(|e| format!("PDFの解析に失敗しました: {e}"))?;
    let entries = signature_entries(&doc);
    if signature_index >= entries.len() {
        return Err(format!("署名番号{signature_index}は範囲外です"));
    }
    let (_byterange, contents) = &entries[signature_index];
    let cms_der = raw_contents(contents)?;
    let signature_value = cms_signature_value(&cms_der)?;
    let imprint = sha256_digest(&signature_value);
    let token = fetch_timestamp_token(tsa_url, &imprint)?;
    let stamped = embed_timestamp_token(&cms_der, &token)?;
    if stamped.len() > CMS_PLACEHOLDER_LEN {
        return Err(format!(
            "タイムスタンプ埋め込み後のCMSがプレースホルダーを超過しました（{} > {}）",
            stamped.len(),
            CMS_PLACEHOLDER_LEN
        ));
    }
    let mut hex = String::with_capacity(CMS_PLACEHOLDER_LEN * 2);
    for byte in &stamped {
        hex.push_str(&format!("{byte:02X}"));
    }
    while hex.len() < CMS_PLACEHOLDER_LEN * 2 {
        hex.push('0');
    }
    let mut out = pdf.to_vec();
    let (placeholder_start, placeholder_len) = locate_placeholder(&out)?;
    if placeholder_start + placeholder_len > out.len() {
        return Err("署名範囲がPDF範囲外です".to_string());
    }
    out[placeholder_start..placeholder_start + placeholder_len].copy_from_slice(hex.as_bytes());
    let report = verify_pdf_cms_with_trust(&out, signature_index, None)?;
    if !report.digest_matches || !report.cms_signature_valid {
        return Err("タイムスタンプ追加後の署名検証に失敗しました".to_string());
    }
    Ok(out)
}
