use super::*;

// ===== SESSION LIFECYCLE & EDITING TAURI COMMANDS =====

#[tauri::command]
pub fn session_open_pdf(
    data: Vec<u8>,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<String, NagisaError> {
    manager.create_session(&data)
}

#[tauri::command]
pub fn session_close(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<bool, NagisaError> {
    manager.close_session(&doc_id)
}

#[tauri::command]
pub fn session_get_bytes(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<Vec<u8>, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let mut session = session_arc.write()?;
    session.save_to_bytes()
}

#[tauri::command]
pub fn session_rotate_page(
    doc_id: String,
    page_index: usize,
    degrees: i32,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<(), NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let mut session = session_arc.write()?;

    if pdf_engine::doc_has_password_encryption(&session.doc) {
        return Err(NagisaError::PasswordRequired);
    }

    let page_ids = crate::pdf_engine::get_page_ids(&session.doc);
    if page_index >= page_ids.len() {
        return Err(NagisaError::InvalidParameter(format!(
            "Page index {page_index} out of range (total pages: {})",
            page_ids.len()
        )));
    }
    let page_id = page_ids[page_index];

    let current_rot =
        if let Some(lopdf::Object::Dictionary(ref dict)) = session.doc.objects.get(&page_id) {
            match dict.get(b"Rotate") {
                Ok(lopdf::Object::Integer(r)) => *r as i32,
                _ => 0,
            }
        } else {
            0
        };

    let new_rot = (current_rot + degrees).rem_euclid(360);
    session.mutate(|doc| crate::pdf_engine::rotate_page_in_doc(doc, page_index, degrees))?;

    session.push_undo(crate::session::EditCommand::RotatePage {
        page: page_index,
        from_degrees: current_rot,
        to_degrees: new_rot,
    });

    Ok(())
}

#[tauri::command]
pub fn session_delete_page(
    doc_id: String,
    page_index: usize,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<(), NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let mut session = session_arc.write()?;

    if pdf_engine::doc_has_password_encryption(&session.doc) {
        return Err(NagisaError::PasswordRequired);
    }

    // 1. Take snapshot before modification
    let snapshot = session.save_to_bytes()?;

    // 2. Perform deletion on document model via mutate (auto-invalidates cache)
    session.mutate(|doc| crate::pdf_engine::delete_page_in_doc(doc, page_index))?;

    // 3. Only if deletion succeeded, push undo snapshot
    session.push_undo(crate::session::EditCommand::FullSnapshot {
        description: format!("Delete page {}", page_index + 1),
        data: snapshot,
    });

    Ok(())
}

#[tauri::command]
pub fn session_undo(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<bool, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let mut session = session_arc.write()?;
    session.undo()
}

#[tauri::command]
pub fn session_redo(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<bool, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let mut session = session_arc.write()?;
    session.redo()
}

#[tauri::command]
pub fn session_update_bytes(
    doc_id: String,
    description: String,
    data: Vec<u8>,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<(), NagisaError> {
    apply_session_bytes(&manager, &doc_id, description, data)
}

/// session_update_bytes の実体。`&SessionManager` 受けで session_exec からも
/// 同一ファンネルを使う（Tauri State は値ムーブになるため直接呼べない）。
pub fn apply_session_bytes(
    manager: &crate::session::SessionManager,
    doc_id: &str,
    description: String,
    data: Vec<u8>,
) -> Result<(), NagisaError> {
    let session_arc = manager.get_session(doc_id)?;
    let mut session = session_arc.write()?;

    // 0. Guard against mutating digitally signed documents (preserves ISO 32000-1 ByteRange integrity)
    if pdf_engine::doc_has_cryptographic_signatures(&session.doc) {
        return Err(NagisaError::SignedPdfMutationBlocked(
            pdf_engine::SIGNED_PDF_MUTATION_ERROR.to_string(),
        ));
    }

    // 0b. Guard against mutating password-encrypted documents. lopdf parses
    // them without decrypting, so edits would corrupt ciphered streams.
    // (Reached e.g. right after protect_pdf; reopen via decrypt to edit.)
    if pdf_engine::doc_has_password_encryption(&session.doc) {
        return Err(NagisaError::PasswordRequired);
    }

    // 1. Take snapshot of current state before applying new bytes
    let snapshot = session.save_to_bytes()?;

    // 2. Parse new document
    let new_doc = lopdf::Document::load_mem(&data)
        .map_err(|e| NagisaError::PdfParse(format!("Failed to parse updated PDF: {e}")))?;

    // 3. Push undo snapshot and update doc in place
    session.push_undo(crate::session::EditCommand::FullSnapshot {
        description,
        data: snapshot,
    });
    session.doc = new_doc;
    session.invalidate_cache();
    drop(session);
    // 全セッション合計の履歴天井も守る（単文書512MB capだけでは
    // 32文書で16GBに達するため）。追い出しは LRU 順の文書単位。
    manager.enforce_global_budget();
    Ok(())
}

#[tauri::command]
pub fn session_get_history_status(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<serde_json::Value, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let session = session_arc.read()?;
    Ok(serde_json::json!({
        "can_undo": !session.undo_stack.is_empty(),
        "can_redo": !session.redo_stack.is_empty(),
        "undo_count": session.undo_stack.len(),
        "redo_count": session.redo_stack.len(),
        "history_bytes": session.total_history_bytes,
    }))
}

// ===== SESSION QUERY & RENDER TAURI COMMANDS =====

#[tauri::command]
pub fn session_get_page_count(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<usize, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let session = session_arc.read()?;
    Ok(pdf_engine::get_page_ids(&session.doc).len())
}

#[tauri::command]
pub fn session_get_page_dimensions(
    doc_id: String,
    page_index: usize,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<serde_json::Value, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let session = session_arc.read()?;
    let page_ids = pdf_engine::get_page_ids(&session.doc);
    if page_index >= page_ids.len() {
        return Err(NagisaError::InvalidParameter(format!(
            "Page index {page_index} out of range (total pages: {})",
            page_ids.len()
        )));
    }
    let (w, h, rotation) = pdf_engine::get_page_viewport(&session.doc, page_ids[page_index]);
    Ok(serde_json::json!({ "width": w, "height": h, "rotation": rotation }))
}

#[tauri::command]
pub fn session_get_text_blocks(
    doc_id: String,
    page_index: usize,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<Vec<pdf_engine::TextBlock>, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let session = session_arc.read()?;
    pdf_engine::get_text_blocks_from_doc(&session.doc, page_index)
}

#[tauri::command]
pub fn session_get_metadata(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<serde_json::Value, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let session = session_arc.read()?;
    pdf_engine::get_pdf_metadata_from_doc(&session.doc)
}

#[tauri::command]
pub fn session_get_bookmarks(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<Vec<serde_json::Value>, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let session = session_arc.read()?;
    pdf_engine::get_bookmarks_from_doc(&session.doc)
}

#[tauri::command]
pub fn session_get_form_fields(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<Vec<serde_json::Value>, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let session = session_arc.read()?;
    pdf_engine::get_form_fields_from_doc(&session.doc)
}

#[tauri::command]
pub fn session_search_text(
    doc_id: String,
    query: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<Vec<serde_json::Value>, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let session = session_arc.read()?;
    pdf_engine::search_text_in_doc(&session.doc, &query)
}

#[tauri::command]
pub async fn session_render_page_to_png(
    doc_id: String,
    page_index: usize,
    dpi: u32,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<Vec<u8>, NagisaError> {
    let dpi = dpi.clamp(24, 600);
    let session_arc = manager.get_session(&doc_id)?;
    tokio::task::spawn_blocking(move || -> Result<Vec<u8>, NagisaError> {
        // 1. Ensure cache with brief write lock (no heavy rasterization under write lock)
        {
            let mut session = session_arc.write()?;
            session.ensure_cached_bytes()?;
        }
        // 2. Perform CPU-heavy raster rendering concurrently under shared read lock
        let session = session_arc.read()?;
        session
            .peek_cached_bytes(|bytes| pdf_engine::render_page_to_png(bytes, page_index, dpi))
            .ok_or_else(|| NagisaError::General("Cached document buffer unavailable".to_string()))?
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
}

#[tauri::command]
pub async fn session_render_color_separation(
    doc_id: String,
    page_index: usize,
    dpi: u32,
    show_c: bool,
    show_m: bool,
    show_y: bool,
    show_k: bool,
    highlight_tac: bool,
    tac_limit: u32,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<Vec<u8>, NagisaError> {
    let dpi = dpi.clamp(24, 600);
    let session_arc = manager.get_session(&doc_id)?;
    tokio::task::spawn_blocking(move || -> Result<Vec<u8>, NagisaError> {
        // 1. Ensure cache with brief write lock
        {
            let mut session = session_arc.write()?;
            session.ensure_cached_bytes()?;
        }
        // 2. Perform color separation under shared read lock
        let session = session_arc.read()?;
        session
            .peek_cached_bytes(|bytes| {
                pdf_engine::render_color_separation(
                    bytes,
                    page_index,
                    dpi,
                    show_c,
                    show_m,
                    show_y,
                    show_k,
                    highlight_tac,
                    tac_limit,
                )
            })
            .ok_or_else(|| NagisaError::General("Cached document buffer unavailable".to_string()))?
    })
    .await
    .map_err(|e| NagisaError::General(format!("Task failed: {e}")))?
}

#[tauri::command]
pub fn session_verify_signature(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<serde_json::Value, NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let session = session_arc.read()?;
    pdf_engine::verify_signature_in_doc(&session.doc)
}

// ===== SESSION-NATIVE MUTATION DISPATCH (zero-IPC-byte exec) =====
//
// exec() の旧汎用経路は getSessionBytes → invoke(data) → updateSessionBytes と
// 全文バイトを IPC 越しに 3 往復させる。JSON number[] は約 8 倍に膨張する
// ため巨大 PDF で UI が固まる。session_exec は小さい args JSON だけを IPC
// 越しに受け、文書バイトは Rust メモリ内だけで回す。Undo/署名ガードは
// session_update_bytes と同一ファンネルを通す。
const SESSION_EXEC_OPS: &[&str] = &[
    "reorder_pages",
    "duplicate_page",
    "crop_page",
    "extract_pages",
    "merge_pdfs",
    "add_bookmark_to_pdf",
    "set_form_field",
    "flatten_form",
    "add_stamp",
    "optimize_pdf",
    "protect_pdf",
    "convert_to_pdfa",
    "convert_to_pdfx",
    "convert_to_pdfx_standard",
    "add_text",
    "edit_text",
    "edit_text_block",
    "move_text_block",
    "delete_text_block",
    "reflow_text",
    "change_text_color",
    "change_font_size",
    "replace_font",
    "add_highlight",
    "add_underline",
    "add_sticky_note",
    "add_rectangle",
    "add_circle",
    "add_line",
    "add_watermark",
    "remove_watermarks",
    "add_annotation_reply",
    "set_annotation_status",
    "delete_annotation",
    "convert_to_cmyk",
    "flatten_transparency",
    "flatten_content",
    "downsample_images",
    "remove_metadata",
    "repair_corrupt_pdf",
    "enhance_scanned_pdf",
    "fix_accessibility_issues",
    "embed_icc_profile",
    "add_header_footer",
    "add_bookmark",
    "add_bates_number",
    "add_page_numbers",
    "add_digital_signature",
    "embed_javascript",
    "embed_font",
    "compress_pdf_quality",
    "redact_text",
    "redact_text_deep",
    "deep_redact",
    "deep_redact_scanned_pdf",
    "add_image_to_page",
    "add_form_field",
    "add_calculated_field",
    "import_xfdf",
    "convert_fonts_to_outlines",
];

/// フロントの exec() から参照する対応表。未対応 op は旧バイト経路へ。
pub fn is_session_exec_supported(op: &str) -> bool {
    SESSION_EXEC_OPS.contains(&op)
}

fn session_arg<T: serde::de::DeserializeOwned>(
    args: &serde_json::Value,
    key: &str,
) -> Result<T, NagisaError> {
    args.get(key)
        .ok_or_else(|| NagisaError::InvalidParameter(format!("session_exec: missing arg '{key}'")))
        .and_then(|v| {
            serde_json::from_value(v.clone()).map_err(|e| {
                NagisaError::InvalidParameter(format!("session_exec: bad arg '{key}': {e}"))
            })
        })
}

/// spawn_blocking 内で実行する同期ディスパッチ。各コマンド関数と同一の
/// pdf_engine 関数・同一引数で呼ぶ（引数形の二重管理を避けるため、
/// 追加時は対応する commands_* 関数のシグネチャと突き合わせること）。
fn dispatch_session_op(
    op: &str,
    data: &[u8],
    args: &serde_json::Value,
) -> Result<Vec<u8>, NagisaError> {
    let page_index = || session_arg::<usize>(args, "pageIndex");
    match op {
        "reorder_pages" => pdf_engine::reorder_pages(
            data,
            session_arg(args, "fromIndex")?,
            session_arg(args, "toIndex")?,
        ),
        "duplicate_page" => pdf_engine::duplicate_page(data, page_index()?),
        "crop_page" => pdf_engine::crop_page(
            data,
            page_index()?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "width")?,
            session_arg(args, "height")?,
        ),
        "extract_pages" => {
            let indices: Vec<usize> = session_arg(args, "indices")?;
            pdf_engine::extract_pages(data, &indices)
        }
        "merge_pdfs" => {
            let paths: Vec<String> = session_arg(args, "paths")?;
            let safe: Vec<String> = paths
                .iter()
                .map(|p| {
                    super::commands_io::validate_safe_path(p, false)
                        .map(|pb| pb.to_string_lossy().to_string())
                })
                .collect::<Result<_, _>>()?;
            pdf_engine::merge_pdfs(&safe)
        }
        "add_bookmark_to_pdf" => {
            pdf_engine::add_bookmark(data, &session_arg::<String>(args, "title")?, page_index()?)
        }
        "set_form_field" => pdf_engine::set_form_field(
            data,
            &session_arg::<String>(args, "fieldName")?,
            &session_arg::<String>(args, "value")?,
        ),
        "flatten_form" => pdf_engine::flatten_form(data),
        "add_stamp" => pdf_engine::add_stamp(
            data,
            page_index()?,
            &session_arg::<String>(args, "text")?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "rotation")?,
            &session_arg::<String>(args, "color")?,
            session_arg(args, "fontSize")?,
        ),
        "optimize_pdf" => pdf_engine::optimize_pdf(data),
        "protect_pdf" => pdf_engine::encrypt::encrypt_pdf(
            data,
            &session_arg::<String>(args, "password")?,
            &session_arg::<String>(args, "password")?,
        ),
        "convert_to_pdfa" => pdf_engine::convert_to_pdfa(data),
        "convert_to_pdfx" => {
            pdf_engine::convert_to_pdfx(data, &session_arg::<String>(args, "outputIntent")?)
        }
        "convert_to_pdfx_standard" => pdf_engine::convert_to_pdfx_standard(
            data,
            &session_arg::<String>(args, "standard")?,
            &session_arg::<String>(args, "outputIntent")?,
        ),
        "add_text" => pdf_engine::add_text(
            data,
            page_index()?,
            &session_arg::<String>(args, "text")?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "size")?,
            &session_arg::<String>(args, "color")?,
        ),
        "edit_text" => pdf_engine::edit_text(
            data,
            page_index()?,
            &session_arg::<String>(args, "searchText")?,
            &session_arg::<String>(args, "replacement")?,
            &session_arg::<String>(args, "fontName")?,
            session_arg(args, "fontSize")?,
            &session_arg::<String>(args, "color")?,
        ),
        "edit_text_block" => pdf_engine::edit_text_block(
            data,
            page_index()?,
            session_arg(args, "blockId")?,
            &session_arg::<String>(args, "newText")?,
        ),
        "move_text_block" => pdf_engine::move_text_block(
            data,
            page_index()?,
            session_arg(args, "blockId")?,
            session_arg(args, "newX")?,
            session_arg(args, "newY")?,
        ),
        "delete_text_block" => {
            pdf_engine::delete_text_block(data, page_index()?, session_arg(args, "blockId")?)
        }
        "reflow_text" => pdf_engine::reflow_text(
            data,
            page_index()?,
            &session_arg::<String>(args, "newText")?,
            session_arg(args, "startX")?,
            session_arg(args, "startY")?,
            session_arg(args, "maxWidth")?,
            session_arg(args, "fontSize")?,
            session_arg(args, "lineHeight")?,
            &session_arg::<String>(args, "color")?,
        ),
        "change_text_color" => pdf_engine::change_text_color(
            data,
            page_index()?,
            &session_arg::<String>(args, "oldColor")?,
            &session_arg::<String>(args, "newColor")?,
        ),
        "change_font_size" => pdf_engine::change_font_size(
            data,
            page_index()?,
            session_arg(args, "oldSize")?,
            session_arg(args, "newSize")?,
        ),
        "replace_font" => pdf_engine::replace_font(
            data,
            &session_arg::<String>(args, "oldFont")?,
            &session_arg::<String>(args, "newFont")?,
        ),
        "add_highlight" => pdf_engine::add_highlight(
            data,
            page_index()?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "width")?,
            session_arg(args, "height")?,
            &session_arg::<String>(args, "color")?,
        ),
        "add_underline" => pdf_engine::add_underline(
            data,
            page_index()?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "width")?,
            &session_arg::<String>(args, "color")?,
        ),
        "add_sticky_note" => pdf_engine::add_sticky_note(
            data,
            page_index()?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            &session_arg::<String>(args, "text")?,
            &session_arg::<String>(args, "color")?,
        ),
        "add_rectangle" => pdf_engine::add_rectangle(
            data,
            page_index()?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "width")?,
            session_arg(args, "height")?,
            &session_arg::<String>(args, "strokeColor")?,
            &session_arg::<String>(args, "fillColor")?,
            session_arg(args, "strokeWidth")?,
        ),
        "add_circle" => pdf_engine::add_circle(
            data,
            page_index()?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "width")?,
            session_arg(args, "height")?,
            &session_arg::<String>(args, "strokeColor")?,
            &session_arg::<String>(args, "fillColor")?,
            session_arg(args, "strokeWidth")?,
        ),
        "add_line" => pdf_engine::add_line(
            data,
            page_index()?,
            session_arg(args, "x1")?,
            session_arg(args, "y1")?,
            session_arg(args, "x2")?,
            session_arg(args, "y2")?,
            &session_arg::<String>(args, "color")?,
            session_arg(args, "width")?,
        ),
        "add_watermark" => pdf_engine::add_watermark(
            data,
            &session_arg::<String>(args, "text")?,
            session_arg(args, "opacity")?,
            session_arg(args, "rotation")?,
            session_arg(args, "fontSize")?,
            &session_arg::<String>(args, "color")?,
            session_arg::<bool>(args, "allPages").unwrap_or(true),
            &session_arg::<Vec<usize>>(args, "pageIndices").unwrap_or_default(),
        ),
        "remove_watermarks" => pdf_engine::remove_watermarks(data),
        "add_annotation_reply" => pdf_engine::add_annotation_reply(
            data,
            session_arg(args, "annotationId")?,
            &session_arg::<String>(args, "author")?,
            &session_arg::<String>(args, "contents")?,
        ),
        "set_annotation_status" => pdf_engine::set_annotation_status(
            data,
            session_arg(args, "annotationId")?,
            &session_arg::<String>(args, "status")?,
        ),
        "delete_annotation" => {
            pdf_engine::delete_annotation(data, session_arg(args, "annotationId")?)
        }
        "convert_to_cmyk" => pdf_engine::convert_to_cmyk(data),
        "flatten_transparency" => pdf_engine::flatten_transparency(data),
        "flatten_content" => pdf_engine::flatten_content(data),
        "downsample_images" => pdf_engine::downsample_images(
            data,
            session_arg(args, "targetDpi")?,
            session_arg(args, "quality")?,
        ),
        "remove_metadata" => pdf_engine::remove_metadata(data),
        "repair_corrupt_pdf" => pdf_engine::repair_corrupt_pdf(data),
        "enhance_scanned_pdf" => pdf_engine::enhance_scanned_pdf(
            data,
            &session_arg::<pdf_engine::ScanEnhanceOptions>(args, "options")?,
        ),
        "fix_accessibility_issues" => pdf_engine::fix_accessibility_issues(
            data,
            &session_arg::<String>(args, "defaultTitle")?,
            &session_arg::<String>(args, "defaultLang")?,
        ),
        "embed_icc_profile" => {
            pdf_engine::embed_icc_profile(data, &session_arg::<String>(args, "profileName")?)
        }
        "add_header_footer" => pdf_engine::add_header_footer(
            data,
            &session_arg::<String>(args, "headerText")?,
            &session_arg::<String>(args, "footerText")?,
            session_arg(args, "fontSize")?,
            session_arg(args, "margin")?,
        ),
        "add_bookmark" => {
            pdf_engine::add_bookmark(data, &session_arg::<String>(args, "title")?, page_index()?)
        }
        "add_bates_number" => pdf_engine::add_bates_number(
            data,
            &session_arg::<String>(args, "prefix")?,
            session_arg(args, "startNumber")?,
            session_arg(args, "fontSize")?,
            session_arg(args, "margin")?,
        ),
        "add_page_numbers" => pdf_engine::add_page_numbers(
            data,
            &session_arg::<String>(args, "position")?,
            session_arg(args, "fontSize")?,
            session_arg(args, "startNumber")?,
        ),
        "add_digital_signature" => {
            let cert: Option<Vec<u8>> = args.get("certificateData").and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    serde_json::from_value(v.clone()).ok()
                }
            });
            pdf_engine::add_digital_signature(
                data,
                page_index()?,
                session_arg(args, "x")?,
                session_arg(args, "y")?,
                session_arg(args, "width")?,
                session_arg(args, "height")?,
                &session_arg::<String>(args, "signerName")?,
                &session_arg::<String>(args, "reason")?,
                cert.as_deref(),
            )
        }
        "embed_javascript" => {
            pdf_engine::embed_javascript(data, &session_arg::<String>(args, "script")?)
        }
        "embed_font" => {
            let font_path: String = session_arg(args, "fontPath")?;
            let safe = super::commands_io::validate_safe_path(&font_path, false)?;
            pdf_engine::embed_font(data, page_index()?, safe.to_string_lossy().as_ref())
        }
        "compress_pdf_quality" => {
            pdf_engine::compress_pdf_quality(data, session_arg(args, "quality")?)
        }
        "redact_text" => pdf_engine::redact_text(
            data,
            &session_arg::<String>(args, "searchText")?,
            &session_arg::<String>(args, "replacement")?,
        ),
        "redact_text_deep" => pdf_engine::redact_text_deep(
            data,
            &session_arg::<String>(args, "searchText")?,
            &session_arg::<String>(args, "color")?,
        ),
        "deep_redact" => pdf_engine::deep_redact(
            data,
            page_index()?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "width")?,
            session_arg(args, "height")?,
            &session_arg::<String>(args, "color")?,
        ),
        "deep_redact_scanned_pdf" => {
            let (bytes, _hits) = crate::ocr_engine::deep_redact_scanned_all(
                data,
                &session_arg::<String>(args, "searchText")?,
                &session_arg::<String>(args, "language")?,
                &session_arg::<String>(args, "color")?,
            )?;
            Ok(bytes)
        }
        "add_image_to_page" => pdf_engine::add_image_to_page(
            data,
            page_index()?,
            &session_arg::<Vec<u8>>(args, "imageData")?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "width")?,
            session_arg(args, "height")?,
        ),
        "add_form_field" => pdf_engine::add_form_field(
            data,
            page_index()?,
            &session_arg::<String>(args, "fieldName")?,
            &session_arg::<String>(args, "fieldType")?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "width")?,
            session_arg(args, "height")?,
            &session_arg::<String>(args, "defaultValue")?,
        ),
        "add_calculated_field" => pdf_engine::add_calculated_field(
            data,
            page_index()?,
            &session_arg::<String>(args, "fieldName")?,
            &session_arg::<String>(args, "formula")?,
            session_arg(args, "x")?,
            session_arg(args, "y")?,
            session_arg(args, "width")?,
            session_arg(args, "height")?,
        ),
        "import_xfdf" => {
            pdf_engine::import_xfdf(data, &session_arg::<String>(args, "xfdfContent")?)
        }
        "convert_fonts_to_outlines" => pdf_engine::preflight::convert_fonts_to_outlines(data),
        other => Err(NagisaError::InvalidParameter(format!(
            "session_exec: unsupported op '{other}'"
        ))),
    }
}

#[tauri::command]
pub async fn session_exec(
    doc_id: String,
    op: String,
    args: serde_json::Value,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<Vec<u8>, NagisaError> {
    if !is_session_exec_supported(&op) {
        return Err(NagisaError::InvalidParameter(format!(
            "session_exec: unsupported op '{op}'"
        )));
    }
    let session_arc = manager.get_session(&doc_id)?;
    // 1. 署名ガード＋シリアライズは短い write lock のみ。重い処理中は
    // ロックを保持しない（render 系と同一方針）。
    let current: Vec<u8> = {
        let mut session = session_arc.write()?;
        if pdf_engine::doc_has_cryptographic_signatures(&session.doc) {
            return Err(NagisaError::SignedPdfMutationBlocked(
                pdf_engine::SIGNED_PDF_MUTATION_ERROR.to_string(),
            ));
        }
        if pdf_engine::doc_has_password_encryption(&session.doc) {
            return Err(NagisaError::PasswordRequired);
        }
        session.save_to_bytes()?
    };
    // 2. CPU-heavy 処理はロック外（アップロード方向のバイト転送は無い）。
    // なお同時実行の合流は last-writer-wins（旧バイト経路と同一）。
    let desc = format!("Command {op}");
    let result = tokio::task::spawn_blocking(move || dispatch_session_op(&op, &current, &args))
        .await
        .map_err(|e| NagisaError::General(format!("Task failed: {e}")))??;
    // 3. session_update_bytes と同一ファンネル（署名ガード＋FullSnapshot＋全体予算）。
    // 戻りは表示用に1往復だけ返す（旧3往復→1往復。アップロードはゼロ）。
    apply_session_bytes(&manager, &doc_id, desc, result)?;
    let session_arc = manager.get_session(&doc_id)?;
    let mut session = session_arc.write()?;
    session.save_to_bytes()
}

#[tauri::command]
pub fn session_print_pdf(
    doc_id: String,
    manager: tauri::State<'_, crate::session::SessionManager>,
) -> Result<(), NagisaError> {
    let session_arc = manager.get_session(&doc_id)?;
    let mut session = session_arc.write()?;
    session.with_bytes(pdf_engine::print_pdf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_pdf() -> Vec<u8> {
        let mut doc = lopdf::Document::with_version("1.7");
        let pages_id = doc.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::new()));
        let content_id = doc.add_object(lopdf::Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"BT /F1 12 Tf 10 10 Td (hi) Tj ET".to_vec(),
        )));
        let mut page_dict = lopdf::Dictionary::new();
        page_dict.set("Type", lopdf::Object::Name("Page".into()));
        page_dict.set("Parent", lopdf::Object::Reference(pages_id));
        page_dict.set(
            "MediaBox",
            lopdf::Object::Array(vec![
                lopdf::Object::Real(0.0),
                lopdf::Object::Real(0.0),
                lopdf::Object::Real(100.0),
                lopdf::Object::Real(100.0),
            ]),
        );
        page_dict.set("Contents", lopdf::Object::Reference(content_id));
        let page_id = doc.add_object(lopdf::Object::Dictionary(page_dict));
        let mut pages_dict = lopdf::Dictionary::new();
        pages_dict.set("Type", lopdf::Object::Name("Pages".into()));
        pages_dict.set("Count", lopdf::Object::Integer(1));
        pages_dict.set(
            "Kids",
            lopdf::Object::Array(vec![lopdf::Object::Reference(page_id)]),
        );
        doc.objects
            .insert(pages_id, lopdf::Object::Dictionary(pages_dict));
        let mut catalog = lopdf::Dictionary::new();
        catalog.set("Type", lopdf::Object::Name("Catalog".into()));
        catalog.set("Pages", lopdf::Object::Reference(pages_id));
        let cat_id = doc.add_object(lopdf::Object::Dictionary(catalog));
        doc.trailer.set("Root", lopdf::Object::Reference(cat_id));
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn session_exec_rejects_unknown_op() {
        let err = dispatch_session_op("nope", &[], &serde_json::json!({})).unwrap_err();
        assert!(matches!(err, NagisaError::InvalidParameter(_)));
        assert!(!is_session_exec_supported("nope"));
        assert!(is_session_exec_supported("deep_redact"));
    }

    #[test]
    fn session_exec_requires_typed_args() {
        let pdf = tiny_pdf();
        let err = dispatch_session_op("add_highlight", &pdf, &serde_json::json!({})).unwrap_err();
        assert!(matches!(err, NagisaError::InvalidParameter(_)));
    }

    #[test]
    fn session_exec_remove_metadata_roundtrip() {
        let pdf = tiny_pdf();
        let out =
            dispatch_session_op("remove_metadata", &pdf, &serde_json::json!({})).expect("op works");
        assert!(!out.is_empty());
        // 結果は再パース可能なPDFでなければならない（update funnel 前提）
        assert!(lopdf::Document::load_mem(&out).is_ok());
    }

    #[test]
    fn session_exec_table_and_match_do_not_diverge() {
        // SESSION_EXEC_OPS に載っている op は必ず dispatch の腕に到達する
        // こと（`other` 落ち＝表と実装の乖離を検出）。空argsなので
        // missing-arg 系エラーになるはずで、unsupported は不可。
        let pdf = tiny_pdf();
        for op in SESSION_EXEC_OPS {
            let err = match dispatch_session_op(op, &pdf, &serde_json::json!({})) {
                Ok(_) => continue, // 引数不要op（flatten等）は成功してよい
                Err(e) => e,
            };
            let msg = format!("{err:?}");
            assert!(
                !msg.contains("unsupported op"),
                "op '{op}' is listed but not dispatched"
            );
        }
    }

    #[test]
    fn password_encryption_guard_detects_encrypt_dict() {
        let pdf = tiny_pdf();
        let mut doc = lopdf::Document::load_mem(&pdf).unwrap();
        assert!(!pdf_engine::doc_has_password_encryption(&doc));
        doc.trailer.set(
            "Encrypt",
            lopdf::Object::Dictionary(lopdf::Dictionary::new()),
        );
        assert!(pdf_engine::doc_has_password_encryption(&doc));
    }
}
