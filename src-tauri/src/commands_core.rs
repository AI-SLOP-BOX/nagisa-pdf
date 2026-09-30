use super::*;

// ===== PDF CORE PAGE & DOCUMENT OPERATIONS =====

#[tauri::command]
pub fn merge_pdfs(paths: Vec<String>) -> Result<Vec<u8>, String> {
    pdf_engine::merge_pdfs(&paths)
}

#[tauri::command]
pub fn delete_page(data: Vec<u8>, page_index: usize) -> Result<Vec<u8>, String> {
    pdf_engine::delete_page(&data, page_index)
}

#[tauri::command]
pub fn rotate_page(data: Vec<u8>, page_index: usize, degrees: i32) -> Result<Vec<u8>, String> {
    pdf_engine::rotate_page(&data, page_index, degrees)
}

#[tauri::command]
pub fn reorder_pages(data: Vec<u8>, from_index: usize, to_index: usize) -> Result<Vec<u8>, String> {
    pdf_engine::reorder_pages(&data, from_index, to_index)
}

#[tauri::command]
pub fn extract_pages(data: Vec<u8>, indices: Vec<usize>) -> Result<Vec<u8>, String> {
    pdf_engine::extract_pages(&data, &indices)
}

#[tauri::command]
pub fn duplicate_page(data: Vec<u8>, page_index: usize) -> Result<Vec<u8>, String> {
    pdf_engine::duplicate_page(&data, page_index)
}

#[tauri::command]
pub fn create_blank_pdf(width: f64, height: f64, page_count: usize) -> Result<Vec<u8>, String> {
    pdf_engine::create_blank_pdf(width, height, page_count)
}

#[tauri::command]
pub fn crop_page(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<Vec<u8>, String> {
    pdf_engine::crop_page(&data, page_index, x, y, width, height)
}

// ===== PDF RENDERING & METADATA =====

#[tauri::command]
pub fn get_page_count(data: Vec<u8>) -> Result<usize, String> {
    pdf_engine::get_page_count_from_data(&data)
}

#[tauri::command]
pub fn get_page_dimensions(data: Vec<u8>, page_index: usize) -> Result<serde_json::Value, String> {
    let (w, h) = pdf_engine::get_page_dimensions_from_data(&data, page_index)?;
    Ok(serde_json::json!({"width": w, "height": h}))
}

#[tauri::command]
pub async fn render_page_to_png(data: Vec<u8>, page_index: usize, dpi: u32) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::render_page_to_png(&data, page_index, dpi)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub fn get_page_text(data: Vec<u8>, page_index: usize) -> Result<String, String> {
    pdf_engine::get_page_text(&data, page_index)
}

#[tauri::command]
pub fn search_text(data: Vec<u8>, query: String) -> Result<Vec<serde_json::Value>, String> {
    let results = pdf_engine::search_text(&data, &query)?;
    Ok(results)
}

#[tauri::command]
pub fn get_bookmarks(data: Vec<u8>) -> Result<Vec<serde_json::Value>, String> {
    let bookmarks = pdf_engine::get_bookmarks(&data)?;
    Ok(bookmarks)
}

#[tauri::command]
pub fn add_bookmark_to_pdf(data: Vec<u8>, title: String, page_index: usize) -> Result<Vec<u8>, String> {
    pdf_engine::add_bookmark(&data, &title, page_index)
}

#[tauri::command]
pub fn get_form_fields(data: Vec<u8>) -> Result<Vec<serde_json::Value>, String> {
    let fields = pdf_engine::get_form_fields(&data)?;
    Ok(fields)
}

#[tauri::command]
pub fn set_form_field(data: Vec<u8>, field_name: String, value: String) -> Result<Vec<u8>, String> {
    pdf_engine::set_form_field(&data, &field_name, &value)
}

#[tauri::command]
pub fn flatten_form(data: Vec<u8>) -> Result<Vec<u8>, String> {
    pdf_engine::flatten_form(&data)
}

#[tauri::command]
pub fn add_stamp(
    data: Vec<u8>,
    page_index: usize,
    text: String,
    x: f64,
    y: f64,
    rotation: f32,
    color: String,
    font_size: f32,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_stamp(&data, page_index, &text, x, y, rotation, &color, font_size)
}

#[tauri::command]
pub fn print_pdf(data: Vec<u8>) -> Result<(), String> {
    pdf_engine::print_pdf(&data)
}

#[tauri::command]
pub fn get_pdf_metadata(data: Vec<u8>) -> Result<serde_json::Value, String> {
    let meta = pdf_engine::get_pdf_metadata(&data)?;
    Ok(meta)
}

#[tauri::command]
pub async fn optimize_pdf(data: Vec<u8>) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::optimize_pdf(&data)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub fn protect_pdf(data: Vec<u8>, password: String) -> Result<Vec<u8>, String> {
    pdf_engine::encrypt::encrypt_pdf(&data, &password, &password)
}

#[tauri::command]
pub async fn compare_pdfs(data1: Vec<u8>, data2: Vec<u8>) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(move || {
        let result = pdf_engine::compare_pdfs(&data1, &data2)?;
        serde_json::to_value(result).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn convert_to_pdfa(data: Vec<u8>) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        pdf_engine::convert_to_pdfa(&data)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}
