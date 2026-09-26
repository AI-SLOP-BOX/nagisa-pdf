use super::*;

// ===== FILE I/O & BATCH PROCESSING TAURI COMMANDS =====

fn validate_safe_path(path_str: &str, for_write: bool) -> Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(path_str);
    if path_str.trim().is_empty() {
        return Err("File path cannot be empty".to_string());
    }

    // Disallow null bytes
    if path_str.contains('\0') {
        return Err("Invalid path containing null bytes".to_string());
    }

    if for_write {
        // #39 是正: 書き込みパスも親ディレクトリを canonicalize したうえで
        // 絶対パスのプレフィックス検証を行い、../等によるディレクトリトラバーサルを遮断する。
        let parent = path.parent().ok_or("Invalid path: no parent directory")?;

        // 親ディレクトリが空文字（カレントを表す相対パス）のときは現在の作業ディレクトリを使用
        let parent_resolved = if parent.as_os_str().is_empty() {
            std::env::current_dir()
                .map_err(|e| format!("Failed to determine current directory: {e}"))?  
        } else {
            parent
                .canonicalize()
                .map_err(|e| format!("Invalid target directory: {e}"))?  
        };

        let file_name = path.file_name().ok_or("Invalid filename: path ends with ..")?;
        let resolved = parent_resolved.join(file_name);

        // 解決済みパスが絶対パスであることを保証する（シンボリックリンク解決は書き込み前でもOK）
        if !resolved.is_absolute() {
            return Err("Resolved write path is not absolute".to_string());
        }

        Ok(resolved)
    } else {
        // For reading, canonicalize to prevent ../ directory traversal
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("File not found or inaccessible: {e}"))?;
        Ok(canonical)
    }
}

#[tauri::command]
pub fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    let safe_path = validate_safe_path(&path, false)?;
    std::fs::read(&safe_path).map_err(|e| format!("Failed to read file: {e}"))
}

#[tauri::command]
pub fn write_file_bytes(path: String, data: Vec<u8>) -> Result<(), String> {
    let safe_path = validate_safe_path(&path, true)?;
    std::fs::write(&safe_path, &data).map_err(|e| format!("Failed to write file: {e}"))
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    let safe_path = validate_safe_path(&path, true)?;
    std::fs::write(&safe_path, &content).map_err(|e| format!("Failed to write file: {e}"))
}

/// Read a PDF file from disk natively and return its metadata (page count,
/// size, version, …) as small JSON. Only the JSON crosses IPC — the file
/// bytes never leave the Rust side (zero-IPC-byte standard).
#[tauri::command]
pub fn get_pdf_file_info(path: String) -> Result<serde_json::Value, String> {
    let safe_path = validate_safe_path(&path, false)?;
    let bytes = std::fs::read(&safe_path).map_err(|e| format!("Failed to read file: {e}"))?;
    pdf_engine::get_pdf_metadata(&bytes)
}

#[tauri::command]
pub fn batch_merge_pdfs(
    paths: Vec<String>,
    output_path: String,
    keep_bookmarks: Option<bool>,
    handle_password: Option<bool>,
    insert_separator: Option<bool>,
    separator_text: Option<String>,
    password: Option<String>,
) -> Result<(), String> {
    let opts = pdf_engine::MergeOptions {
        keep_bookmarks: keep_bookmarks.unwrap_or(true),
        handle_password: handle_password.unwrap_or(true),
        insert_separator: insert_separator.unwrap_or(false),
        separator_text: separator_text.unwrap_or_default(),
        password,
    };
    pdf_engine::batch_merge_pdfs_with_options(&paths, &output_path, &opts)
}

#[tauri::command]
pub fn batch_add_watermark(
    paths: Vec<String>,
    text: String,
    opacity: f32,
    rotation: f32,
    font_size: f32,
    color: String,
) -> Result<Vec<Vec<u8>>, String> {
    pdf_engine::batch_add_watermark(&paths, &text, opacity, rotation, font_size, &color)
}

#[tauri::command]
pub fn batch_protect(paths: Vec<String>, password: String) -> Result<Vec<Vec<u8>>, String> {
    pdf_engine::batch_protect(&paths, &password)
}

#[tauri::command]
pub fn batch_optimize(paths: Vec<String>) -> Result<Vec<Vec<u8>>, String> {
    pdf_engine::batch_optimize(&paths)
}

// ===== CONVERSIONS & EXPORT TAURI COMMANDS =====

#[tauri::command]
pub fn pdf_to_images(
    data: Vec<u8>,
    output_dir: String,
    format: String,
    dpi: u32,
    page_indexes: Option<Vec<usize>>,
) -> Result<Vec<String>, String> {
    pdf_engine::pdf_to_images_ex(&data, &output_dir, &format, dpi, page_indexes.as_deref())
}

#[tauri::command]
pub fn images_to_pdf(image_paths: Vec<String>, output_path: String) -> Result<(), String> {
    pdf_engine::images_to_pdf(&image_paths, &output_path)
}

#[tauri::command]
pub fn html_to_pdf(html_content: String, output_path: String) -> Result<(), String> {
    pdf_engine::html_to_pdf(&html_content, &output_path)
}

#[tauri::command]
pub fn pdf_to_word(
    data: Vec<u8>,
    output_path: String,
    page_indexes: Option<Vec<usize>>,
    keep_images: Option<bool>,
    editable_tables: Option<bool>,
    run_ocr: Option<bool>,
) -> Result<(), String> {
    pdf_engine::pdf_to_word_ex(
        &data,
        &output_path,
        page_indexes.as_deref(),
        keep_images.unwrap_or(false),
        editable_tables.unwrap_or(false),
        run_ocr.unwrap_or(false),
    )
}

#[tauri::command]
pub fn pdf_to_excel(
    data: Vec<u8>,
    output_path: String,
    page_indexes: Option<Vec<usize>>,
    editable_tables: Option<bool>,
    run_ocr: Option<bool>,
) -> Result<(), String> {
    pdf_engine::pdf_to_excel_ex(
        &data,
        &output_path,
        page_indexes.as_deref(),
        editable_tables.unwrap_or(true),
        run_ocr.unwrap_or(false),
    )
}

#[tauri::command]
pub fn pdf_to_powerpoint(
    data: Vec<u8>,
    output_path: String,
    page_indexes: Option<Vec<usize>>,
    run_ocr: Option<bool>,
) -> Result<(), String> {
    pdf_engine::pdf_to_powerpoint_ex(
        &data,
        &output_path,
        page_indexes.as_deref(),
        run_ocr.unwrap_or(false),
    )
}

#[tauri::command]
pub fn create_pdf_portfolio(file_paths: Vec<String>, output_path: String) -> Result<(), String> {
    pdf_engine::create_pdf_portfolio(&file_paths, &output_path)
}

#[tauri::command]
pub fn add_header_footer(
    data: Vec<u8>,
    header_text: String,
    footer_text: String,
    font_size: f32,
    margin: f32,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_header_footer(&data, &header_text, &footer_text, font_size, margin)
}

#[tauri::command]
pub fn add_bookmark(data: Vec<u8>, title: String, page_index: usize) -> Result<Vec<u8>, String> {
    pdf_engine::add_bookmark(&data, &title, page_index)
}

#[tauri::command]
pub fn add_bates_number(
    data: Vec<u8>,
    prefix: String,
    start_number: usize,
    font_size: f32,
    margin: f32,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_bates_number(&data, &prefix, start_number, font_size, margin)
}
