use super::*;

// ===== IMAGE, SCANNER & OCR TAURI COMMANDS =====

#[tauri::command]
pub async fn process_scanned_images(
    paths: Vec<String>,
    remove_shadow: bool,
    correct_perspective: bool,
    dpi: u32,
) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        image_engine::process_scanned_images(&paths, remove_shadow, correct_perspective, dpi)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn ocr_files(paths: Vec<String>, language: String) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(move || {
        let result = ocr_engine::ocr_files(&paths, &language)?;
        serde_json::to_value(result).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn ocr_image_blocks(
    image_bytes: Vec<u8>,
    language: String,
) -> Result<Vec<ocr_engine::OCRLineBlock>, String> {
    tokio::task::spawn_blocking(move || {
        ocr_engine::ocr_image_blocks(&image_bytes, &language)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn create_epub(text: String, output_path: String, title: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        ocr_engine::create_epub(&text, &output_path, &title)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn create_searchable_pdf(
    original_paths: Vec<String>,
    ocr_text: String,
    output_path: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        ocr_engine::create_searchable_pdf(&original_paths, &ocr_text, &output_path)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

// ===== REDACTION COMMANDS =====

#[tauri::command]
pub fn redact_area(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: String,
) -> Result<Vec<u8>, String> {
    pdf_engine::redact::guard_not_signed(&data)?;
    pdf_engine::redact_area(&data, page_index, x, y, width, height, &color)
}

#[tauri::command]
pub fn redact_text(data: Vec<u8>, search_text: String, replacement: String) -> Result<Vec<u8>, String> {
    pdf_engine::redact::guard_not_signed(&data)?;
    pdf_engine::redact_text(&data, &search_text, &replacement)
}

#[tauri::command]
pub async fn deep_redact(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: String,
) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::redact::guard_not_signed(&data)?;
        pdf_engine::deep_redact(&data, page_index, x, y, width, height, &color)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn deep_redact_scanned_pdf(
    data: Vec<u8>,
    search_text: String,
    language: String,
    color: String,
) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::redact::guard_not_signed(&data)?;
        let (bytes, _hits) =
            ocr_engine::deep_redact_scanned_all(&data, &search_text, &language, &color)?;
        Ok(bytes)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn redact_text_deep(data: Vec<u8>, search_text: String, color: String) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::redact::guard_not_signed(&data)?;
        pdf_engine::redact_text_deep(&data, &search_text, &color)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn sanitize_document(data: Vec<u8>) -> Result<(Vec<u8>, pdf_engine::SanitizeSummary), String> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::sanitize_document(&data)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn convert_fonts_to_outlines(data: Vec<u8>) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::convert_fonts_to_outlines(&data)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}
