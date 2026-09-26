use super::*;

// ===== PRO SIGNATURE, TOKEN & EXTENDED WORKFLOW TAURI COMMANDS =====

#[tauri::command]
pub fn add_digital_signature(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    signer_name: String,
    reason: String,
    certificate_data: Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_digital_signature(
        &data,
        page_index,
        x,
        y,
        width,
        height,
        &signer_name,
        &reason,
        certificate_data.as_deref(),
    )
}

#[tauri::command]
pub fn verify_signature(
    data: Vec<u8>,
    signature_index: usize,
) -> Result<serde_json::Value, String> {
    pdf_engine::verify_signature(&data, signature_index)
}

#[tauri::command]
pub fn sign_pdf_cms(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    signer_name: String,
    reason: String,
    location: Option<String>,
    contact_info: Option<String>,
    p12_data: Option<Vec<u8>>,
    p12_password: Option<String>,
    private_key_pem: Option<String>,
    certificate_pem: Option<String>,
    tsa_url: Option<String>,
) -> Result<Vec<u8>, String> {
    let seed = pdf_engine::cms_sign::SignatureFieldSeed {
        page_index,
        rect: [x, y, x + width, y + height],
        field_name: format!(
            "Signature_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ),
        signer_name,
        reason,
        location: location.unwrap_or_default(),
        contact_info: contact_info.unwrap_or_default(),
    };

    let req = pdf_engine::cms_sign::CmsSignRequest {
        seed,
        private_key_pem: private_key_pem.map(|s| s.into_bytes()).unwrap_or_default(),
        certificate_pem: certificate_pem.map(|s| s.into_bytes()).unwrap_or_default(),
        chain_pem: Vec::new(),
        p12_der: p12_data,
        p12_password,
        tsa_url,
    };

    pdf_engine::cms_sign::sign_pdf_cms(&req, &data)
}

#[tauri::command]
pub fn verify_pdf_cms(
    data: Vec<u8>,
    signature_index: usize,
    trust_roots_pem: Option<String>,
) -> Result<pdf_engine::cms_sign::CmsVerifyReport, String> {
    pdf_engine::cms_sign::verify_pdf_cms_with_trust(
        &data,
        signature_index,
        trust_roots_pem.as_deref().map(|s| s.as_bytes()),
    )
}

/// Embed PAdES-LTV validation material (cert chains + CRL/OCSP when
/// reachable) into the document catalog's /DSS as an additive update.
#[tauri::command]
pub fn stamp_pdf_ltv(
    data: Vec<u8>,
) -> Result<pdf_engine::cms_sign::LtvStampResult, String> {
    pdf_engine::cms_sign::stamp_ltv_dss(&data)
}

#[tauri::command]
pub fn inspect_compatibility(
    data: Vec<u8>,
) -> Result<pdf_engine::compatibility::CompatibilityReport, String> {
    pdf_engine::compatibility::inspect_pdf(&data)
}

#[tauri::command]
pub fn get_engine_health() -> Result<serde_json::Value, String> {
    Ok(pdf_engine::inspect::engine_health())
}

/// Enumerate code-signing identities available in the OS keychain.
#[tauri::command]
pub fn list_keychain_identities() -> Result<Vec<pdf_engine::cms_sign::KeychainIdentity>, String> {
    pdf_engine::cms_sign::list_keychain_identities()
}

/// Sign a PDF using a private key that never leaves the OS keychain.
#[tauri::command]
pub fn sign_pdf_with_keychain(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    signer_name: String,
    reason: String,
    identity_nickname: String,
    tsa_url: Option<String>,
) -> Result<Vec<u8>, String> {
    let identities = pdf_engine::cms_sign::list_keychain_identities()?;
    let identity = identities
        .iter()
        .find(|i| i.nickname == identity_nickname || i.common_name == identity_nickname)
        .ok_or_else(|| format!("証明書「{identity_nickname}」がキーチェーンに見つかりません"))?
        .clone();

    let seed = pdf_engine::cms_sign::SignatureFieldSeed {
        page_index,
        rect: [x, y, x + width, y + height],
        field_name: format!(
            "Signature_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ),
        signer_name,
        reason,
        location: String::new(),
        contact_info: String::new(),
    };

    pdf_engine::cms_sign::sign_pdf_cms_with_keychain(&data, seed, &identity, tsa_url)
}

/// List PKCS#11 signing certificate/key pairs without requiring the token PIN.
#[tauri::command]
pub fn list_pkcs11_slots() -> Result<Vec<pdf_engine::hsm::Pkcs11Slot>, String> {
    pdf_engine::hsm::list_pkcs11_slots()
}

/// Sign a PDF with a selected PKCS#11 identity. The PIN is used only for the
/// signing session and is never returned or persisted.
#[tauri::command]
pub fn sign_pdf_with_pkcs11(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    signer_name: String,
    reason: String,
    slot_id: u64,
    certificate_id: String,
    pin: String,
    tsa_url: Option<String>,
) -> Result<Vec<u8>, String> {
    let seed = pdf_engine::cms_sign::SignatureFieldSeed {
        page_index,
        rect: [x, y, x + width, y + height],
        field_name: format!(
            "Signature_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ),
        signer_name,
        reason,
        location: String::new(),
        contact_info: String::new(),
    };
    pdf_engine::cms_sign::sign_pdf_cms_with_pkcs11(
        &data,
        seed,
        slot_id,
        &certificate_id,
        // Ownership moves down to cryptoki's zeroizing AuthPin so the PIN
        // buffer is wiped as soon as the token session is done.
        pin,
        tsa_url,
    )
}

#[tauri::command]
pub fn embed_font(data: Vec<u8>, page_index: usize, font_path: String) -> Result<Vec<u8>, String> {
    pdf_engine::embed_font(&data, page_index, &font_path)
}

#[tauri::command]
pub fn add_form_field(
    data: Vec<u8>,
    page_index: usize,
    field_name: String,
    field_type: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    default_value: String,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_form_field(
        &data,
        page_index,
        &field_name,
        &field_type,
        x,
        y,
        width,
        height,
        &default_value,
    )
}

#[tauri::command]
pub fn add_calculated_field(
    data: Vec<u8>,
    page_index: usize,
    field_name: String,
    formula: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_calculated_field(
        &data,
        page_index,
        &field_name,
        &formula,
        x,
        y,
        width,
        height,
    )
}

#[tauri::command]
pub fn export_xfdf(data: Vec<u8>) -> Result<String, String> {
    pdf_engine::export_xfdf(&data)
}

#[tauri::command]
pub fn import_xfdf(data: Vec<u8>, xfdf_content: String) -> Result<Vec<u8>, String> {
    pdf_engine::import_xfdf(&data, &xfdf_content)
}

#[tauri::command]
pub fn repair_pdf(data: Vec<u8>) -> Result<Vec<u8>, String> {
    pdf_engine::repair_pdf(&data)
}

#[tauri::command]
pub fn unlock_pdf(data: Vec<u8>, password: String) -> Result<Vec<u8>, String> {
    pdf_engine::unlock_pdf(&data, &password)
}

#[tauri::command]
pub fn compress_pdf_quality(data: Vec<u8>, quality: u8) -> Result<Vec<u8>, String> {
    pdf_engine::compress_pdf_quality(&data, quality)
}

#[tauri::command]
pub fn add_page_numbers(
    data: Vec<u8>,
    position: String,
    font_size: f32,
    start_number: usize,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_page_numbers(&data, &position, font_size, start_number)
}

#[tauri::command]
pub fn create_action_wizard(name: String, steps: Vec<serde_json::Value>) -> Result<String, String> {
    let action_steps: Vec<pdf_engine::ActionStep> = steps
        .iter()
        .map(|s| pdf_engine::ActionStep {
            action_type: s
                .get("action_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            params: s.get("params").cloned().unwrap_or(serde_json::Value::Null),
        })
        .collect();
    pdf_engine::create_action_wizard(&name, &action_steps)
}

#[tauri::command]
pub fn execute_action_wizard(data: Vec<u8>, wizard_json: String) -> Result<Vec<u8>, String> {
    pdf_engine::execute_action_wizard(&data, &wizard_json)
}

#[tauri::command]
pub fn aggregate_form_data(pdf_paths: Vec<String>) -> Result<serde_json::Value, String> {
    pdf_engine::aggregate_form_data(&pdf_paths)
}

#[tauri::command]
pub fn embed_javascript(data: Vec<u8>, script: String) -> Result<Vec<u8>, String> {
    pdf_engine::embed_javascript(&data, &script)
}

#[tauri::command]
pub fn add_bookmark_tree(
    data: Vec<u8>,
    bookmarks: Vec<serde_json::Value>,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_bookmark_tree(&data, &bookmarks)
}

#[tauri::command]
pub fn visual_diff(data1: Vec<u8>, data2: Vec<u8>, output_path: String) -> Result<(), String> {
    pdf_engine::visual_diff(&data1, &data2, &output_path)
}

#[tauri::command]
pub fn list_digital_ids() -> Result<Vec<pdf_engine::DigitalID>, String> {
    pdf_engine::list_digital_ids()
}
