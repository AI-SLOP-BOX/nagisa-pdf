use super::*;

// ===== IMAGE, SCANNER & OCR TAURI COMMANDS =====

#[tauri::command]
pub async fn process_scanned_images(
    paths: Vec<String>,
    remove_shadow: bool,
    correct_perspective: bool,
    dpi: u32,
) -> Result<Vec<u8>, NagisaError> {
    let safe_paths: Vec<String> = paths
        .iter()
        .map(|p| {
            super::commands_io::validate_safe_path(p, false)
                .map(|pb| pb.to_string_lossy().to_string())
        })
        .collect::<Result<_, _>>()?;
    tokio::task::spawn_blocking(move || {
        image_engine::process_scanned_images(&safe_paths, remove_shadow, correct_perspective, dpi)
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}

#[tauri::command]
pub async fn ocr_files(paths: Vec<String>, language: String) -> Result<serde_json::Value, NagisaError> {
    let safe_paths: Vec<String> = paths
        .iter()
        .map(|p| {
            super::commands_io::validate_safe_path(p, false)
                .map(|pb| pb.to_string_lossy().to_string())
        })
        .collect::<Result<_, _>>()?;
    tokio::task::spawn_blocking(move || {
        let result = ocr_engine::ocr_files(&safe_paths, &language)?;
        serde_json::to_value(result).map_err(|e| NagisaError::from(e.to_string()))
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}

#[tauri::command]
pub async fn ocr_image_blocks(
    image_bytes: Vec<u8>,
    language: String,
) -> Result<Vec<ocr_engine::OCRLineBlock>, NagisaError> {
    tokio::task::spawn_blocking(move || {
        ocr_engine::ocr_image_blocks(&image_bytes, &language)
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}

#[tauri::command]
pub async fn create_epub(text: String, output_path: String, title: String) -> Result<(), NagisaError> {
    let safe_output = super::commands_io::validate_safe_path(&output_path, true)?;
    let safe_output = safe_output.to_string_lossy().to_string();
    tokio::task::spawn_blocking(move || {
        ocr_engine::create_epub(&text, &safe_output, &title)
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}

#[tauri::command]
pub async fn create_searchable_pdf(
    original_paths: Vec<String>,
    ocr_text: String,
    output_path: String,
) -> Result<(), NagisaError> {
    let safe_paths: Vec<String> = original_paths
        .iter()
        .map(|p| {
            super::commands_io::validate_safe_path(p, false)
                .map(|pb| pb.to_string_lossy().to_string())
        })
        .collect::<Result<_, _>>()?;
    let safe_output = super::commands_io::validate_safe_path(&output_path, true)?;
    let safe_output = safe_output.to_string_lossy().to_string();
    tokio::task::spawn_blocking(move || {
        ocr_engine::create_searchable_pdf(&safe_paths, &ocr_text, &safe_output)
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}

// ===== REDACTION COMMANDS =====
// 注意: 視覚専用の redact_area は IPC 公開しない（lib.rs 参照）。
// エンジン関数自体は pdf_engine::redact_area として残し、テストが
// デコード経路の回帰を guard する。

#[tauri::command]
pub fn redact_text(data: Vec<u8>, search_text: String, replacement: String) -> Result<Vec<u8>, NagisaError> {
    pdf_engine::redact::guard_not_signed(&data)?;
    pdf_engine::redact_text(&data, &search_text, &replacement)
        .map_err(NagisaError::from)
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
) -> Result<Vec<u8>, NagisaError> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::redact::guard_not_signed(&data)?;
        pdf_engine::deep_redact(&data, page_index, x, y, width, height, &color)
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}

#[tauri::command]
pub async fn deep_redact_scanned_pdf(
    data: Vec<u8>,
    search_text: String,
    language: String,
    color: String,
) -> Result<Vec<u8>, NagisaError> {
    tokio::task::spawn_blocking(move || -> Result<Vec<u8>, NagisaError> {
        pdf_engine::redact::guard_not_signed(&data)?;
        let (bytes, _hits) = ocr_engine::deep_redact_scanned_all(
            &data,
            &search_text,
            &language,
            &color,
        )
        .map_err(NagisaError::from)?;
        Ok(bytes)
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}

#[tauri::command]
pub async fn redact_text_deep(data: Vec<u8>, search_text: String, color: String) -> Result<Vec<u8>, NagisaError> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::redact::guard_not_signed(&data)?;
        pdf_engine::redact_text_deep(&data, &search_text, &color)
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}

#[tauri::command]
pub async fn sanitize_document(data: Vec<u8>) -> Result<(Vec<u8>, pdf_engine::SanitizeSummary), NagisaError> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::sanitize_document(&data)
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}

#[tauri::command]
pub async fn convert_fonts_to_outlines(data: Vec<u8>) -> Result<Vec<u8>, NagisaError> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::convert_fonts_to_outlines(&data)
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
    .map_err(NagisaError::from)
}
