use super::*;

// ===== ANNOTATIONS & DRAWING TAURI COMMANDS =====

#[tauri::command]
pub fn add_highlight(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: String,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_highlight(&data, page_index, x, y, width, height, &color)
}

#[tauri::command]
pub fn add_underline(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    color: String,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_underline(&data, page_index, x, y, width, &color)
}

#[tauri::command]
pub fn add_sticky_note(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    text: String,
    color: String,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_sticky_note(&data, page_index, x, y, &text, &color)
}

#[tauri::command]
pub fn add_rectangle(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    stroke_color: String,
    fill_color: String,
    stroke_width: f32,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_rectangle(
        &data,
        page_index,
        x,
        y,
        width,
        height,
        &stroke_color,
        &fill_color,
        stroke_width,
    )
}

#[tauri::command]
pub fn add_circle(
    data: Vec<u8>,
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    stroke_color: String,
    fill_color: String,
    stroke_width: f32,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_circle(
        &data,
        page_index,
        x,
        y,
        width,
        height,
        &stroke_color,
        &fill_color,
        stroke_width,
    )
}

#[tauri::command]
pub fn add_line(
    data: Vec<u8>,
    page_index: usize,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    color: String,
    width: f32,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_line(&data, page_index, x1, y1, x2, y2, &color, width)
}

#[tauri::command]
pub fn add_watermark(
    data: Vec<u8>,
    text: String,
    opacity: f32,
    rotation: f32,
    font_size: f32,
    color: String,
    all_pages: bool,
    page_indices: Vec<usize>,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_watermark(
        &data,
        &text,
        opacity,
        rotation,
        font_size,
        &color,
        all_pages,
        &page_indices,
    )
}

#[tauri::command]
pub fn remove_watermarks(data: Vec<u8>) -> Result<Vec<u8>, String> {
    pdf_engine::remove_watermarks(&data)
}

#[tauri::command]
pub fn get_annotations(data: Vec<u8>) -> Result<Vec<serde_json::Value>, String> {
    pdf_engine::get_annotations(&data)
}

#[tauri::command]
pub fn add_annotation_reply(
    data: Vec<u8>,
    annotation_id: (u32, u16),
    author: String,
    contents: String,
) -> Result<Vec<u8>, String> {
    pdf_engine::add_annotation_reply(&data, annotation_id, &author, &contents)
}

#[tauri::command]
pub fn set_annotation_status(
    data: Vec<u8>,
    annotation_id: (u32, u16),
    status: String,
) -> Result<Vec<u8>, String> {
    pdf_engine::set_annotation_status(&data, annotation_id, &status)
}

#[tauri::command]
pub fn delete_annotation(data: Vec<u8>, annotation_id: (u32, u16)) -> Result<Vec<u8>, String> {
    pdf_engine::delete_annotation(&data, annotation_id)
}
