use super::common::*;
use lopdf::{Dictionary, Document, Object, Stream};

// ===== BATCH PROCESSING & PAGE FORMATTING (Separated to batch_ops.rs) =====
pub use super::batch_ops::*;

// ===== OPTIMIZE =====

pub fn optimize_pdf(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    // Safe and standard PDF optimization:
    // 1. Recompress FlateDecode streams with maximum compression where beneficial
    for (_, obj) in doc.objects.iter_mut() {
        if let Object::Stream(ref mut stream) = obj {
            let should_recompress = if let Ok(filter) = stream.dict.get(b"Filter") {
                if let Ok(filter_name) = filter.as_name() {
                    filter_name == b"FlateDecode"
                } else {
                    false
                }
            } else {
                true // Uncompressed stream, compress with FlateDecode
            };

            if should_recompress {
                let raw_data = if stream.dict.get(b"Filter").is_ok() {
                    if stream.decompress().is_ok() {
                        stream.content.clone()
                    } else {
                        continue;
                    }
                } else {
                    stream.content.clone()
                };

                let mut encoder =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
                if std::io::Write::write_all(&mut encoder, &raw_data).is_ok() {
                    if let Ok(compressed) = encoder.finish() {
                        if compressed.len() < stream.content.len() || stream.dict.get(b"Filter").is_err() {
                            stream.set_content(compressed);
                            stream.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
                        }
                    }
                }
            }
        }
    }

    // 2. Safely prune unreachable / unreferenced isolated objects without breaking reference chains
    doc.prune_objects();

    save_doc(&mut doc)
}

// ===== COMPARE =====

#[derive(serde::Serialize)]
pub struct CompareResult {
    pub page_count_diff: bool,
    pub pages_same: usize,
    pub pages_different: usize,
    pub size_diff: bool,
    pub original_size: usize,
    pub modified_size: usize,
}

pub fn compare_pdfs(data1: &[u8], data2: &[u8]) -> Result<CompareResult, String> {
    let doc1 = Document::load_mem(data1).map_err(|e| format!("Failed to load PDF1: {e}"))?;
    let doc2 = Document::load_mem(data2).map_err(|e| format!("Failed to load PDF2: {e}"))?;

    let pages1 = get_page_ids(&doc1);
    let pages2 = get_page_ids(&doc2);

    let page_count_diff = pages1.len() != pages2.len();
    let mut pages_same = 0;
    let mut pages_different = 0;

    let min_pages = pages1.len().min(pages2.len());
    for i in 0..min_pages {
        // Compare pages by resolving and hashing their content stream bytes
        // to avoid false positives from OID ordering differences.
        let bytes1 = resolve_page_content_bytes(&doc1, pages1[i]);
        let bytes2 = resolve_page_content_bytes(&doc2, pages2[i]);
        if bytes1 == bytes2 {
            pages_same += 1;
        } else {
            pages_different += 1;
        }
    }

    Ok(CompareResult {
        page_count_diff,
        pages_same,
        pages_different,
        size_diff: data1.len() != data2.len(),
        original_size: data1.len(),
        modified_size: data2.len(),
    })
}

/// Resolve all content streams of a page into a single concatenated byte sequence for comparison.
fn resolve_page_content_bytes(doc: &Document, page_id: OID) -> Vec<u8> {
    let content_ids: Vec<OID> = if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
        match dict.get(b"Contents") {
            Ok(Object::Reference(id)) => vec![*id],
            Ok(Object::Array(arr)) => arr.iter().filter_map(|o| o.as_reference().ok()).collect(),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let mut all_bytes = Vec::new();
    for cid in content_ids {
        if let Some(Object::Stream(stream)) = doc.objects.get(&cid) {
            // Use raw content bytes (after decode) for comparison
            let bytes = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
            all_bytes.extend_from_slice(&bytes);
        }
    }
    all_bytes
}

// ===== PDF RENDERING =====

pub fn get_page_count_from_data(data: &[u8]) -> Result<usize, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    Ok(get_page_ids(&doc).len())
}

pub fn get_page_dimensions_from_data(data: &[u8], page_index: usize) -> Result<(f32, f32), String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }
    Ok(get_page_dimensions(&doc, page_ids[page_index]))
}

pub fn render_page_to_png(data: &[u8], page_index: usize, dpi: u32) -> Result<Vec<u8>, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();

    let temp_dir = std::env::temp_dir();
    let temp_pdf = temp_dir.join(format!("nagisa_{pid}_{id}.pdf"));
    let temp_prefix = temp_dir.join(format!("nagisa_page_{pid}_{id}"));

    std::fs::write(&temp_pdf, data).map_err(|e| format!("Failed to write temp PDF: {e}"))?;

    let output = find_tool_command("pdftoppm")
        .args([
            "-png",
            "-r",
            &dpi.to_string(),
            "-f",
            &(page_index + 1).to_string(),
            "-l",
            &(page_index + 1).to_string(),
            temp_pdf.to_str().unwrap_or(""),
            temp_prefix.to_str().unwrap_or(""),
        ])
        .output();

    let _ = std::fs::remove_file(&temp_pdf);

    let output = match output {
        Ok(out) => out,
        Err(e) => {
            return Err(format!(
                "Failed to execute pdftoppm: {e}. Ensure poppler is installed."
            ))
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pdftoppm failed: {stderr}"));
    }

    // Match output file (pdftoppm creates format: prefix-1.png, prefix-01.png, or prefix-000001.png)
    let mut found_file = None;
    if let Ok(entries) = std::fs::read_dir(&temp_dir) {
        let prefix_stem = temp_prefix
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(stem) = path.file_stem() {
                if stem.to_string_lossy().starts_with(&*prefix_stem)
                    && path.extension().and_then(|s| s.to_str()) == Some("png")
                {
                    found_file = Some(path);
                    break;
                }
            }
        }
    }

    let png_path = found_file
        .ok_or_else(|| "Failed to locate rendered PNG output from pdftoppm".to_string())?;
    let png_data =
        std::fs::read(&png_path).map_err(|e| format!("Failed to read rendered PNG: {e}"))?;
    let _ = std::fs::remove_file(&png_path);

    Ok(png_data)
}

/// コンテンツストリームにテキスト描画演算子(Tj/TJ/'/")が含まれ得るかを高速事前判定する。
/// 巨大なベクター描画のみのページ（CAD図面等、数百万オペレータ）で
/// Content::decode の全走査をスキップするためのもの。
/// 誤検出（文字列・名前中の "Tj" 等）は起こり得るがデコードに倒れるだけで安全。
/// 逆に検出漏れは演算子トークンが空白を跨げないため起こらない。
fn may_contain_text_ops(data: &[u8]) -> bool {
    if data.contains(&b'\'') || data.contains(&b'"') {
        return true;
    }
    let mut idx = 0;
    while idx < data.len() {
        match data[idx..].iter().position(|&b| b == b'T') {
            Some(pos) => {
                let abs = idx + pos;
                if abs + 1 < data.len() && (data[abs + 1] == b'j' || data[abs + 1] == b'J') {
                    return true;
                }
                idx = abs + 1;
            }
            None => return false,
        }
    }
    false
}

/// パース済み Document から指定ページのテキストを抽出する内部ヘルパー。
/// 全頁処理時にページ毎の全文書再パース（O(n²)）を避けるため分離。
fn page_text_from_doc(doc: &Document, page_id: lopdf::ObjectId) -> String {
    let mut text = String::new();
    let content_ids = resolve_page_content_stream_ids(doc, page_id);
    for cid in content_ids {
        if let Some(Object::Stream(stream)) = doc.objects.get(&cid) {
            let decomp = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
            if !may_contain_text_ops(&decomp) {
                continue;
            }
            if let Ok(content) = lopdf::content::Content::decode(&decomp) {
                for op in &content.operations {
                    match op.operator.as_str() {
                        "Tj" => {
                            if let Some(Object::String(bytes, _)) = op.operands.first() {
                                text.push_str(&String::from_utf8_lossy(bytes));
                            }
                        }
                        "TJ" => {
                            if let Some(Object::Array(arr)) = op.operands.first() {
                                for item in arr {
                                    if let Object::String(bytes, _) = item {
                                        text.push_str(&String::from_utf8_lossy(bytes));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    text
}

pub fn get_page_text(data: &[u8], page_index: usize) -> Result<String, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }
    Ok(page_text_from_doc(&doc, page_ids[page_index]))
}

/// 全ページのテキストを1回のパースで抽出する。
/// `get_page_text` を全ページ分ループするとページ毎に全文書を再パースして
/// O(ページ数²) になり、1000ページ級のPDFで実質ハングするためのバッチ版。
pub fn extract_all_text(data: &[u8]) -> Result<Vec<String>, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    Ok(page_ids
        .iter()
        .map(|&pid| page_text_from_doc(&doc, pid))
        .collect())
}

pub fn search_text_in_doc(doc: &Document, query: &str) -> Result<Vec<serde_json::Value>, String> {
    let page_ids = get_page_ids(doc);
    let mut results = Vec::new();

    for (i, &page_id) in page_ids.iter().enumerate() {
        // 抽出ロジックは page_text_from_doc に統一（テキスト無しページのデコードをスキップ）
        let page_text = page_text_from_doc(doc, page_id);

        if page_text.contains(query) {
            results.push(serde_json::json!({
                "page": i,
                "text": page_text,
                "matches": page_text.matches(query).count(),
            }));
        }
    }

    Ok(results)
}

pub fn search_text(data: &[u8], query: &str) -> Result<Vec<serde_json::Value>, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    search_text_in_doc(&doc, query)
}

pub fn get_bookmarks_from_doc(doc: &Document) -> Result<Vec<serde_json::Value>, String> {
    let mut bookmarks = Vec::new();

    let root_id = match doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        Ok(id) => id,
        Err(_) => return Ok(bookmarks),
    };

    let page_ids = get_page_ids(doc);

    if let Some(root) = doc.objects.get(&root_id) {
        if let Ok(root_dict) = root.as_dict() {
            let outline_id = match root_dict.get(b"Outlines") {
                Ok(Object::Reference(id)) => Some(*id),
                _ => None,
            };

            if let Some(out_id) = outline_id {
                if let Some(Object::Dictionary(outline_dict)) = doc.objects.get(&out_id) {
                    let first_item = match outline_dict.get(b"First") {
                        Ok(Object::Reference(id)) => Some(*id),
                        _ => None,
                    };

                    let mut queue = Vec::new();
                    if let Some(f_id) = first_item {
                        queue.push(f_id);
                    }

                    while let Some(item_id) = queue.pop() {
                        if let Some(Object::Dictionary(item)) = doc.objects.get(&item_id) {
                            let title = item
                                .get(b"Title")
                                .ok()
                                .and_then(|o| match o {
                                    Object::String(bytes, _) => {
                                        Some(decode_pdf_text_string(bytes))
                                    }
                                    _ => None,
                                })
                                .unwrap_or_else(|| "Untitled".to_string());

                            // Extract destination page
                            let mut page_num = 0usize;
                            if let Ok(dest_obj) = item.get(b"Dest") {
                                match dest_obj {
                                    Object::Array(arr) if !arr.is_empty() => {
                                        if let Ok(target_p_ref) = arr[0].as_reference() {
                                            if let Some(idx) = page_ids.iter().position(|&pid| pid == target_p_ref) {
                                                page_num = idx;
                                            }
                                        }
                                    }
                                    Object::Reference(target_p_ref) => {
                                        if let Some(idx) = page_ids.iter().position(|&pid| pid == *target_p_ref) {
                                            page_num = idx;
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            // If there are children (/First), queue the first child
                            if let Ok(Object::Reference(child_id)) = item.get(b"First") {
                                queue.push(*child_id);
                            }

                            // If there is a sibling (/Next), queue the next sibling
                            if let Ok(Object::Reference(next_id)) = item.get(b"Next") {
                                queue.push(*next_id);
                            }

                            bookmarks.push(serde_json::json!({
                                "title": title,
                                "page": page_num,
                            }));
                        }
                    }
                }
            }
        }
    }

    Ok(bookmarks)
}

pub fn get_bookmarks(data: &[u8]) -> Result<Vec<serde_json::Value>, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    get_bookmarks_from_doc(&doc)
}

pub fn get_form_fields_from_doc(doc: &Document) -> Result<Vec<serde_json::Value>, String> {
    let mut fields = Vec::new();

    let root_id = match doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        Ok(id) => id,
        Err(_) => return Ok(fields),
    };

    if let Some(root) = doc.objects.get(&root_id) {
        if let Ok(root_dict) = root.as_dict() {
            let acroform_dict = match root_dict.get(b"AcroForm") {
                Ok(Object::Reference(acroform_id)) => {
                    doc.objects.get(acroform_id).and_then(|o| o.as_dict().ok()).cloned()
                }
                Ok(Object::Dictionary(d)) => Some(d.clone()),
                _ => None,
            };

            if let Some(acroform) = acroform_dict {
                if let Ok(Object::Array(field_refs)) = acroform.get(b"Fields") {
                    for field_ref in field_refs {
                        if let Object::Reference(field_id) = field_ref {
                            if let Some(Object::Dictionary(field)) = doc.objects.get(&field_id) {
                                let name = field
                                    .get(b"T")
                                    .ok()
                                    .and_then(|o| match o {
                                        Object::String(bytes, _) => {
                                            Some(decode_pdf_text_string(bytes))
                                        }
                                        _ => None,
                                    })
                                    .unwrap_or_default();
                                let field_type = field
                                    .get(b"FT")
                                    .ok()
                                    .and_then(|o| match o {
                                        Object::Name(bytes) => {
                                            Some(String::from_utf8_lossy(bytes).to_string())
                                        }
                                        _ => None,
                                    })
                                    .unwrap_or_default();
                                let value = field
                                    .get(b"V")
                                    .ok()
                                    .and_then(|o| match o {
                                        Object::String(bytes, _) => {
                                            Some(decode_pdf_text_string(bytes))
                                        }
                                        Object::Name(bytes) => {
                                            Some(String::from_utf8_lossy(bytes).to_string())
                                        }
                                        _ => None,
                                    })
                                    .unwrap_or_default();

                                fields.push(serde_json::json!({
                                    "name": name,
                                    "type": field_type,
                                    "value": value,
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(fields)
}

pub fn get_form_fields(data: &[u8]) -> Result<Vec<serde_json::Value>, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    get_form_fields_from_doc(&doc)
}

pub fn set_form_field(data: &[u8], field_name: &str, value: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let root_id = match doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        Ok(id) => id,
        Err(_) => return Err("No root".into()),
    };

    // Collect field IDs first to avoid borrow issues
    let mut target_field_id = None;

    if let Some(root) = doc.objects.get(&root_id) {
        if let Ok(root_dict) = root.as_dict() {
            if let Ok(Object::Reference(acroform_id)) = root_dict.get(b"AcroForm") {
                if let Some(Object::Dictionary(acroform)) = doc.objects.get(&acroform_id) {
                    if let Ok(Object::Array(field_refs)) = acroform.get(b"Fields") {
                        for field_ref in field_refs {
                            if let Object::Reference(field_id) = field_ref {
                                if let Some(Object::Dictionary(field)) = doc.objects.get(&field_id)
                                {
                                    if let Ok(Object::String(bytes, _)) = field.get(b"T") {
                                        let name = String::from_utf8_lossy(bytes);
                                        if name == field_name {
                                            target_field_id = Some(*field_id);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Now update the field
    if let Some(field_id) = target_field_id {
        if let Some(Object::Dictionary(ref mut f)) = doc.objects.get_mut(&field_id) {
            f.set(
                "V",
                Object::String(value.as_bytes().to_vec(), lopdf::StringFormat::Literal),
            );
        }
    }

    save_doc(&mut doc)
}

pub fn flatten_form(data: &[u8]) -> Result<Vec<u8>, String> {
    // Real flattening: bake each widget annotation's Appearance Stream (/AP /N)
    // into the page content, then remove the widget annotation and /AcroForm.
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let page_ids = get_page_ids(&doc);

    for &page_id in &page_ids {
        // Collect widget annotation IDs on this page
        let annot_ids: Vec<OID> = {
            if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
                let arr = match dict.get(b"Annots") {
                    Ok(Object::Array(a)) => a.clone(),
                    Ok(Object::Reference(rid)) => {
                        doc.objects
                            .get(rid)
                            .and_then(|o| o.as_array().ok())
                            .cloned()
                            .unwrap_or_default()
                    }
                    _ => Vec::new(),
                };
                arr.iter().filter_map(|o| o.as_reference().ok()).collect()
            } else {
                Vec::new()
            }
        };

        for annot_id in &annot_ids {
            // Only process Widget annotations (form fields)
            let is_widget = doc.objects.get(annot_id)
                .and_then(|o| o.as_dict().ok())
                .and_then(|d| d.get(b"Subtype").ok())
                .map(|o| matches!(o, Object::Name(n) if n == b"Widget"))
                .unwrap_or(false);

            if !is_widget {
                continue;
            }

            // Locate /AP /N appearance stream
            let ap_stream_id: Option<OID> = doc.objects.get(annot_id)
                .and_then(|o| o.as_dict().ok())
                .and_then(|d| d.get(b"AP").ok())
                .and_then(|ap| {
                    match ap {
                        Object::Dictionary(ap_dict) => ap_dict.get(b"N").ok().and_then(|n| n.as_reference().ok()),
                        _ => None,
                    }
                });

            if let Some(ap_id) = ap_stream_id {
                // Read the appearance stream bbox and matrix to place content
                let (bbox, matrix, ap_bytes) = {
                    if let Some(Object::Stream(ref ap_stream)) = doc.objects.get(&ap_id) {
                        let raw = ap_stream.decompressed_content().unwrap_or_else(|_| ap_stream.content.clone());
                        let bbox_arr = ap_stream.dict.get(b"BBox")
                            .ok()
                            .and_then(|o| o.as_array().ok())
                            .cloned()
                            .unwrap_or_default();
                        let matrix_arr = ap_stream.dict.get(b"Matrix")
                            .ok()
                            .and_then(|o| o.as_array().ok())
                            .cloned()
                            .unwrap_or_default();
                        (bbox_arr, matrix_arr, raw)
                    } else {
                        continue;
                    }
                };

                // Get widget Rect to position the appearance stream on the page
                let rect: Vec<Object> = doc.objects.get(annot_id)
                    .and_then(|o| o.as_dict().ok())
                    .and_then(|d| d.get(b"Rect").ok())
                    .and_then(|o| o.as_array().ok())
                    .cloned()
                    .unwrap_or_default();

                let tx = rect.first().and_then(|v| v.as_float().ok()).unwrap_or(0.0);
                let ty = rect.get(1).and_then(|v| v.as_float().ok()).unwrap_or(0.0);
                let bx1 = bbox.first().and_then(|v| v.as_float().ok()).unwrap_or(0.0);
                let by1 = bbox.get(1).and_then(|v| v.as_float().ok()).unwrap_or(0.0);

                // Build a wrapping content stream: q + cm (translate to rect position) + Do
                // Place appearance stream content directly using Do operator via a Form XObject.
                let xobj_res_name = format!("FlatWgt_{}_{}" , ap_id.0, ap_id.1);

                // Register the appearance stream as a Form XObject on the page resources
                let resources = resolve_page_resources(&doc, page_id);
                let mut xobj_dict = match resources.get(b"XObject") {
                    Ok(Object::Dictionary(d)) => d.clone(),
                    _ => Dictionary::new(),
                };
                xobj_dict.set(xobj_res_name.as_bytes().to_vec(), Object::Reference(ap_id));

                let mut res = resources;
                res.set("XObject", Object::Dictionary(xobj_dict));
                if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
                    page_dict.set("Resources", Object::Dictionary(res));
                }

                // Matrix default is [1 0 0 1 0 0]
                let ma = matrix.first().and_then(|v| v.as_float().ok()).unwrap_or(1.0);
                let mb = matrix.get(1).and_then(|v| v.as_float().ok()).unwrap_or(0.0);
                let mc = matrix.get(2).and_then(|v| v.as_float().ok()).unwrap_or(0.0);
                let md = matrix.get(3).and_then(|v| v.as_float().ok()).unwrap_or(1.0);
                let me = matrix.get(4).and_then(|v| v.as_float().ok()).unwrap_or(0.0);
                let mf = matrix.get(5).and_then(|v| v.as_float().ok()).unwrap_or(0.0);

                let _ = (ap_bytes, bx1, by1, ma, mb, mc, md, me, mf);

                let flat_ops = vec![
                    lopdf::content::Operation::new("q", vec![]),
                    lopdf::content::Operation::new("cm", vec![
                        Object::Real(1.0), Object::Real(0.0),
                        Object::Real(0.0), Object::Real(1.0),
                        Object::Real(tx - bx1), Object::Real(ty - by1),
                    ]),
                    lopdf::content::Operation::new("Do", vec![
                        Object::Name(xobj_res_name.into_bytes()),
                    ]),
                    lopdf::content::Operation::new("Q", vec![]),
                ];

                let flat_content = lopdf::content::Content { operations: flat_ops };
                let flat_bytes = flat_content.encode().map_err(|e| format!("Flatten encode error: {e}"))?;
                let flat_stream = Stream::new(Dictionary::new(), flat_bytes);
                let flat_id = doc.add_object(flat_stream);
                append_page_content(&mut doc, page_id, flat_id)?;
            }
        }

        // Remove all annotations from page /Annots
        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.remove(b"Annots");
        }
    }

    // Remove /AcroForm from document catalog
    let root_id = match doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        Ok(id) => id,
        Err(_) => return save_doc(&mut doc),
    };
    if let Some(root) = doc.objects.get_mut(&root_id) {
        if let Ok(dict) = root.as_dict_mut() {
            dict.remove(b"AcroForm");
        }
    }

    save_doc(&mut doc)
}

pub fn add_stamp(
    data: &[u8],
    page_index: usize,
    text: &str,
    x: f64,
    y: f64,
    rotation: f32,
    color: &str,
    font_size: f32,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let (r, g, b) = parse_hex_color(color, (1.0, 0.0, 0.0));

    // Build rotation matrix: [cos -sin sin cos tx ty]
    let rad = rotation * std::f32::consts::PI / 180.0;
    let cos_r = rad.cos();
    let sin_r = rad.sin();

    // Use /F1 as the font resource name for Helvetica-Bold
    let font_res_name = "F1";

    let operations = vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new(
            "cm",
            vec![
                Object::Real(cos_r),
                Object::Real(sin_r),
                Object::Real(-sin_r),
                Object::Real(cos_r),
                Object::Real(x as f32),
                Object::Real(y as f32),
            ],
        ),
        lopdf::content::Operation::new("BT", vec![]),
        lopdf::content::Operation::new(
            "Tf",
            vec![
                Object::Name(font_res_name.into()),
                Object::Real(font_size),
            ],
        ),
        lopdf::content::Operation::new(
            "rg",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        lopdf::content::Operation::new("Td", vec![Object::Real(0.0), Object::Real(0.0)]),
        lopdf::content::Operation::new(
            "Tj",
            vec![Object::String(
                text.as_bytes().to_vec(),
                lopdf::StringFormat::Literal,
            )],
        ),
        lopdf::content::Operation::new("ET", vec![]),
        lopdf::content::Operation::new("Q", vec![]),
    ];

    let content = lopdf::content::Content { operations };
    let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

    let mut stream = Stream::new(Dictionary::new(), content_bytes);
    stream.dict.set("Type", Object::Name("Content".into()));
    let content_id = doc.add_object(stream);

    let page_id = page_ids[page_index];

    // Register Helvetica-Bold as /F1 in page /Resources /Font
    // (Standard 14 font: no embedding required per ISO 32000-1 §9.6.2.2)
    let mut resources_dict = resolve_page_resources(&doc, page_id);
    let mut fonts_dict = match resources_dict.get(b"Font") {
        Ok(Object::Dictionary(fd)) => fd.clone(),
        Ok(Object::Reference(f_ref)) => {
            doc.objects
                .get(f_ref)
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default()
        }
        _ => Dictionary::new(),
    };

    // Only register if not already present to avoid clobbering existing /F1
    if fonts_dict.get(font_res_name.as_bytes()).is_err() {
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name(b"Font".to_vec()));
        font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_dict.set("BaseFont", Object::Name(b"Helvetica-Bold".to_vec()));
        font_dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        let font_id = doc.add_object(Object::Dictionary(font_dict));
        fonts_dict.set(font_res_name, Object::Reference(font_id));
    }
    resources_dict.set("Font", Object::Dictionary(fonts_dict));
    if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
        page_dict.set("Resources", Object::Dictionary(resources_dict));
    }

    // Append stamp content AFTER existing page content (non-destructive)
    append_page_content(&mut doc, page_id, content_id)?;

    save_doc(&mut doc)
}

pub fn print_pdf(data: &[u8]) -> Result<(), String> {
    let temp_dir = std::env::temp_dir();
    let temp_pdf = temp_dir.join("nagisa_print.pdf");

    std::fs::write(&temp_pdf, data).map_err(|e| format!("Failed to write temp: {e}"))?;

    let pdf_str = temp_pdf
        .to_str()
        .ok_or_else(|| "Invalid temp path".to_string())?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-p", pdf_str])
            .spawn()
            .map_err(|e| format!("Failed to print: {e}"))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", "/p", pdf_str])
            .spawn()
            .map_err(|e| format!("Failed to print: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("lp")
            .arg(pdf_str)
            .spawn()
            .map_err(|e| format!("Failed to print: {e}"))?;
    }

    Ok(())
}

pub fn get_pdf_metadata_from_doc(doc: &Document) -> Result<serde_json::Value, String> {
    let page_count = get_page_ids(doc).len();

    let mut title = String::new();
    let mut author = String::new();
    let mut creator = String::new();
    let mut producer = String::new();

    if let Ok(Object::Reference(info_id)) = doc.trailer.get(b"Info") {
        if let Some(Object::Dictionary(info)) = doc.objects.get(&info_id) {
            if let Ok(Object::String(bytes, _)) = info.get(b"Title") {
                title = String::from_utf8_lossy(bytes).to_string();
            }
            if let Ok(Object::String(bytes, _)) = info.get(b"Author") {
                author = String::from_utf8_lossy(bytes).to_string();
            }
            if let Ok(Object::String(bytes, _)) = info.get(b"Creator") {
                creator = String::from_utf8_lossy(bytes).to_string();
            }
            if let Ok(Object::String(bytes, _)) = info.get(b"Producer") {
                producer = String::from_utf8_lossy(bytes).to_string();
            }
        }
    }

    // A document is encrypted iff its trailer references an /Encrypt dictionary.
    let encrypted = doc.trailer.get(b"Encrypt").is_ok();

    Ok(serde_json::json!({
        "page_count": page_count,
        "title": title,
        "author": author,
        "creator": creator,
        "producer": producer,
        "encrypted": encrypted,
        "version": doc.version,
    }))
}

pub fn get_pdf_metadata(data: &[u8]) -> Result<serde_json::Value, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let mut val = get_pdf_metadata_from_doc(&doc)?;
    if let Some(obj) = val.as_object_mut() {
        obj.insert("size".to_string(), serde_json::json!(data.len()));
    }
    Ok(val)
}


/// Release-readiness / engine health matrix, suitable for a `nagisa-cli health`
/// smoke command and offline assertion in tests.
fn binary_available(name: &str) -> bool {
    let candidates = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/snap/bin"];
    for dir in &candidates {
        if std::path::Path::new(&format!("{dir}/{name}")).exists() {
            return true;
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
            if dir.join(exe).exists() {
                return true;
            }
        }
    }
    false
}

pub fn engine_health() -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "binaries": {
            "openssl": binary_available("openssl"),
            "qpdf": binary_available("qpdf"),
            "pdftoppm": binary_available("pdftoppm"),
            "libreoffice": binary_available("libreoffice"),
            "ghostscript": binary_available("gs"),
            "tesseract": binary_available("tesseract"),
        },
        "features": {
            "cms_sign": true,
            "compatibility": true,
            "repair": true,
            "preflight": true,
            "dss_ltv": true,
        },
    })
}
