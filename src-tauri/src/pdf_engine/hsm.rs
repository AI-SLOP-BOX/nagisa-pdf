//! PKCS#11 hardware security module (HSM) support for PDF signing.
//!
//! Scope and honesty notes:
//! * Private keys never leave the token. The raw RSA signature is produced by the
//!   module and wrapped into CMS by Nagisa, exactly like the keychain path.
//! * Nagisa does **not** bundle vendor middleware. A PKCS#11 module (OpenSC,
//!   Yubico, p11-kit, …) must be installed; discovery probes a known path list.
//! * A machine without a connected token still works: slot enumeration returns
//!   an empty list rather than erroring, so the UI degrades gracefully.

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::object::{Attribute, AttributeType, ObjectClass};
use cryptoki::session::UserType;
use cryptoki::types::AuthPin;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Candidate PKCS#11 module locations, in priority order.
fn candidate_module_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // Explicit override wins, mirroring Acrobat's own preference.
    if let Ok(custom) = std::env::var("NAGISA_PKCS11_MODULE") {
        paths.push(PathBuf::from(custom));
    }
    let roots = [
        "/opt/homebrew/lib",
        "/usr/local/lib",
        "/usr/lib",
        "/usr/lib64",
        "/Library/Frameworks",
    ];
    let names = [
        "p11-kit-client.so",
        "opensc-pkcs11.so",
        "libopensc-pkcs11.so",
        "pkcs11.so",
        "libykcs11.so",
        "opensc-pkcs11.dylib",
        "libopensc-pkcs11.dylib",
    ];
    for root in roots {
        for name in names {
            let candidate = PathBuf::from(root).join(name);
            if candidate.exists() {
                paths.push(candidate);
            }
        }
    }
    paths
}

/// Load and initialise a PKCS#11 context from the first module that works.
fn load_first_module() -> Result<Pkcs11, String> {
    let mut last_error = "PKCS#11モジュールが見つかりません".to_string();
    for path in candidate_module_paths() {
        match Pkcs11::new(&path) {
            Ok(pkcs11) => {
                if let Err(e) =
                    pkcs11.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
                {
                    last_error = format!("{}: 初期化に失敗 ({e})", path.display());
                    continue;
                }
                return Ok(pkcs11);
            }
            Err(e) => last_error = format!("{}: ロードに失敗 ({e})", path.display()),
        }
    }
    Err(last_error)
}
/// A certificate/key pair discovered on a PKCS#11 token.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Pkcs11Certificate {
    pub certificate_id: String,
    pub label: String,
    pub subject: String,
    pub issuer: String,
    pub serial_number: String,
    pub sha256_fingerprint: String,
}

/// A PKCS#11 slot that exposes signing identities.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Pkcs11Slot {
    pub slot_id: u64,
    pub description: String,
    pub manufacturer: String,
    pub token_label: String,
    pub token_serial: String,
    pub certificates: Vec<Pkcs11Certificate>,
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

/// Return the DER certificate bound to the selected CKA_ID.
pub fn pkcs11_certificate_der(slot_id: u64, certificate_id: &str) -> Result<Vec<u8>, String> {
    let pkcs11 = load_first_module()?;
    let slot = pkcs11
        .get_all_slots()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|s| s.id() == slot_id)
        .ok_or_else(|| format!("スロット{slot_id}が見つかりません"))?;
    let session = pkcs11
        .open_ro_session(slot)
        .map_err(|e| format!("セッションのオープンに失敗しました: {e}"))?;
    let template = vec![
        Attribute::Class(ObjectClass::CERTIFICATE),
        Attribute::Id(hex_decode(certificate_id)?),
    ];
    for cert in session
        .find_objects(&template)
        .map_err(|e| format!("証明書の検索に失敗しました: {e}"))?
    {
        if let Ok(attrs) = session.get_attributes(cert, &[AttributeType::Value]) {
            if let Some(der) = attribute_bytes(&attrs, AttributeType::Value) {
                if !der.is_empty() {
                    return Ok(der);
                }
            }
        }
    }
    Err("選択した署名証明書が見つかりません".into())
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 {
        return Err("証明書IDが不正です".into());
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| "証明書IDが不正です".into()))
        .collect()
}

fn certificate_template() -> Vec<Attribute> {
    vec![
        Attribute::Class(ObjectClass::CERTIFICATE),
        Attribute::Token(true),
    ]
}

fn attribute_bytes(attrs: &[Attribute], wanted: AttributeType) -> Option<Vec<u8>> {
    attrs.iter().find_map(|a| match (a, wanted) {
        (Attribute::Id(v), AttributeType::Id)
        | (Attribute::Value(v), AttributeType::Value)
        | (Attribute::Subject(v), AttributeType::Subject)
        | (Attribute::Issuer(v), AttributeType::Issuer)
        | (Attribute::SerialNumber(v), AttributeType::SerialNumber) => Some(v.clone()),
        _ => None,
    })
}

/// Enumerate signing certificate/key pairs available on inserted PKCS#11 tokens.
pub fn list_pkcs11_slots() -> Result<Vec<Pkcs11Slot>, String> {
    let pkcs11 = match load_first_module() {
        Ok(p) => p,
        Err(_) => return Ok(Vec::new()),
    };
    let mut result = Vec::new();
    for slot in pkcs11.get_all_slots().map_err(|e| e.to_string())? {
        let token_info = match pkcs11.get_token_info(slot) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let session = match pkcs11.open_ro_session(slot) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let cert_template = certificate_template();
        let certs = session.find_objects(&cert_template).unwrap_or_default();

        let mut certificates = Vec::new();
        for cert in &certs {
            let cert_attrs = match session.get_attributes(
                *cert,
                &[
                    AttributeType::Id,
                    AttributeType::Value,
                    AttributeType::Subject,
                    AttributeType::Issuer,
                    AttributeType::SerialNumber,
                    AttributeType::Label,
                ],
            ) {
                Ok(v) => v,
                Err(_) => continue,
            };
            // Some HSMs hide private-key objects until the user logs in. Certificates
            // are public, so expose them during discovery and resolve the matching
            // private key only in the authenticated signing session.
            let Some(key_id) = attribute_bytes(&cert_attrs, AttributeType::Id) else {
                continue;
            };
            let der = attribute_bytes(&cert_attrs, AttributeType::Value).unwrap_or_default();
            let label = cert_attrs
                .iter()
                .find_map(|a| match a {
                    Attribute::Label(v) => Some(String::from_utf8_lossy(v).trim().to_string()),
                    _ => None,
                })
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "署名証明書".into());
            let subject = cert_attrs
                .iter()
                .find_map(|a| match a {
                    Attribute::Subject(v) => Some(String::from_utf8_lossy(v).into_owned()),
                    _ => None,
                })
                .unwrap_or_default();
            let issuer = cert_attrs
                .iter()
                .find_map(|a| match a {
                    Attribute::Issuer(v) => Some(String::from_utf8_lossy(v).into_owned()),
                    _ => None,
                })
                .unwrap_or_default();
            let serial_number = hex(attribute_bytes(&cert_attrs, AttributeType::SerialNumber)
                .as_deref()
                .unwrap_or_default());
            let fingerprint = openssl_fingerprint(&der);
            certificates.push(Pkcs11Certificate {
                certificate_id: hex(&key_id),
                label,
                subject,
                issuer,
                serial_number,
                sha256_fingerprint: fingerprint,
            });
        }
        if !certificates.is_empty() {
            result.push(Pkcs11Slot {
                slot_id: slot.id(),
                description: pkcs11
                    .get_slot_info(slot)
                    .map(|v| v.slot_description().to_string())
                    .unwrap_or_default(),
                manufacturer: token_info.manufacturer_id().to_string(),
                token_label: token_info.label().to_string(),
                token_serial: token_info.serial_number().to_string(),
                certificates,
            });
        }
    }
    Ok(result)
}

fn openssl_fingerprint(der: &[u8]) -> String {
    use std::io::Write;
    let mut child = match Command::new("openssl")
        .args([
            "x509",
            "-inform",
            "DER",
            "-noout",
            "-fingerprint",
            "-sha256",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(der);
    }
    let out = child
        .wait_with_output()
        .ok()
        .map(|v| String::from_utf8_lossy(&v.stdout).into_owned())
        .unwrap_or_default();
    out.lines()
        .next()
        .and_then(|v| v.split_once('=').map(|(_, h)| h.replace(':', "")))
        .unwrap_or_default()
}

/// Sign `payload` with the private key in the given PKCS#11 slot.
///
/// Returns the raw RSA signature; the caller wraps it into CMS. The private key
/// material never leaves the module.
pub fn pkcs11_raw_sign(
    slot_id: u64,
    certificate_id: &str,
    pin: String,
    payload: &[u8],
) -> Result<Vec<u8>, String> {
    let pkcs11 = load_first_module()?;
    let slot = pkcs11
        .get_all_slots()
        .map_err(|e| format!("PKCS#11スロットの列挙に失敗しました: {e}"))?
        .into_iter()
        .find(|s| s.id() == slot_id)
        .ok_or_else(|| format!("スロット{slot_id}が見つかりません"))?;

    let session = pkcs11
        .open_rw_session(slot)
        .map_err(|e| format!("セッションのオープンに失敗しました: {e}"))?;
    session
        .login(UserType::User, Some(&AuthPin::from(pin)))
        .map_err(|e| format!("トークンへのログインに失敗しました: {e}"))?;

    let key_id = hex_decode(certificate_id)?;
    let template = vec![
        Attribute::Class(ObjectClass::PRIVATE_KEY),
        Attribute::Id(key_id),
        Attribute::Token(true),
        Attribute::Sign(true),
    ];
    let key = session
        .find_objects(&template)
        .map_err(|e| format!("秘密鍵の検索に失敗しました: {e}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "選択した証明書に対応する署名鍵が見つかりません".to_string())?;

    session
        .sign(&cryptoki::mechanism::Mechanism::Sha256RsaPkcs, key, payload)
        .map_err(|e| format!("HSM署名に失敗しました: {e}"))
}

/// True when a usable PKCS#11 module can be loaded. Used by the engine health
/// report so the UI can explain *why* hardware signing is unavailable.
pub fn pkcs11_available() -> bool {
    load_first_module().is_ok()
}
