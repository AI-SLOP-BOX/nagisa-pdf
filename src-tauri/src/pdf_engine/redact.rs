use super::common::*;
use super::font_unicode::parse_tounicode_cmap;
use super::page_tree::materialize_inherited_page_attrs;
use lopdf::{Dictionary, Document, Object, Stream};
use std::collections::HashMap;

#[derive(Clone, Default)]
struct FontInfo {
    is_type0: bool,
    cmap: Option<HashMap<u16, String>>,
}

impl FontInfo {
    fn decode(&self, bytes: &[u8]) -> String {
        if let Some(ref cmap) = self.cmap {
            if self.is_type0 || bytes.len() >= 2 {
                let mut decoded = String::new();
                for chunk in bytes.chunks(2) {
                    let cid = if chunk.len() == 2 {
                        u16::from_be_bytes([chunk[0], chunk[1]])
                    } else {
                        chunk[0] as u16
                    };
                    if let Some(s) = cmap.get(&cid) {
                        decoded.push_str(s);
                    } else if let Some(ch) = char::from_u32(cid as u32) {
                        decoded.push(ch);
                    }
                }
                if !decoded.is_empty() {
                    return decoded;
                }
            } else {
                let mut decoded = String::new();
                for &b in bytes {
                    let cid = b as u16;
                    if let Some(s) = cmap.get(&cid) {
                        decoded.push_str(s);
                    } else {
                        decoded.push(b as char);
                    }
                }
                if !decoded.is_empty() {
                    return decoded;
                }
            }
        }

        // Fallback UTF-16BE
        if bytes.len() >= 2 && bytes.len() % 2 == 0 {
            if bytes.starts_with(&[0xFE, 0xFF]) {
                let u16s: Vec<u16> = bytes[2..]
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                if let Ok(s) = String::from_utf16(&u16s) {
                    return s;
                }
            }
        }

        String::from_utf8_lossy(bytes).to_string()
    }
}

fn extract_font_infos(
    res_dict: Option<&Dictionary>,
    doc: &Document,
) -> HashMap<Vec<u8>, FontInfo> {
    let mut fonts = HashMap::new();
    if let Some(res) = res_dict {
        let font_sub = res.get(b"Font").ok().and_then(|f| match f {
            Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()),
            Object::Dictionary(d) => Some(d),
            _ => None,
        });
        if let Some(fsub) = font_sub {
            for (fname, fobj) in fsub.iter() {
                let fdict = match fobj {
                    Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                };
                if let Some(fd) = fdict {
                    let is_type0 = fd.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) == Some(b"Type0");
                    let mut cmap = None;
                    if let Ok(to_unicode_ref) = fd.get(b"ToUnicode").and_then(|o| o.as_reference()) {
                        if let Some(Object::Stream(st)) = doc.objects.get(&to_unicode_ref) {
                            let decompressed = st
                                .decompressed_content()
                                .unwrap_or_else(|_| st.content.clone());
                            let parsed = parse_tounicode_cmap(&decompressed);
                            if !parsed.is_empty() {
                                cmap = Some(parsed);
                            }
                        }
                    }
                    fonts.insert(fname.clone(), FontInfo { is_type0, cmap });
                }
            }
        }
    }
    fonts
}

// ===== REDACTION =====

pub fn redact_area(
    data: &[u8],
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: &str,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let (r, g, b) = parse_hex_color(color, (0.0, 0.0, 0.0));
    let page_id = page_ids[page_index];

    // Decode existing page content if present, so we preserve existing page content
    let mut operations = Vec::new();
    let mut content_ids: Vec<OID> = Vec::new();
    if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
        if let Ok(contents_obj) = dict.get(b"Contents") {
            content_ids = match contents_obj {
                Object::Reference(id) => vec![*id],
                Object::Array(arr) => arr.iter().filter_map(|o| o.as_reference().ok()).collect(),
                _ => vec![],
            };
            for cid in &content_ids {
                if let Some(Object::Stream(ref stream)) = doc.objects.get(cid) {
                    if let Ok(c) = lopdf::content::Content::decode(&stream.content) {
                        operations.extend(c.operations);
                    }
                }
            }
        }
    }

    // Append redaction box operations
    operations.push(lopdf::content::Operation::new("q", vec![]));
    operations.push(lopdf::content::Operation::new(
        "rg",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    ));
    operations.push(lopdf::content::Operation::new(
        "re",
        vec![
            Object::Real(x as f32),
            Object::Real(y as f32),
            Object::Real(width as f32),
            Object::Real(height as f32),
        ],
    ));
    operations.push(lopdf::content::Operation::new("f", vec![]));
    operations.push(lopdf::content::Operation::new("Q", vec![]));

    let content = lopdf::content::Content { operations };
    let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

    let mut stream = Stream::new(Dictionary::new(), content_bytes);
    stream.dict.set("Type", Object::Name("Content".into()));
    let content_id = doc.add_object(stream);

    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        dict.set("Contents", Object::Reference(content_id));
    }

    // Clean up old unreferenced content objects
    for cid in content_ids {
        if cid != content_id {
            doc.objects.remove(&cid);
        }
    }

    save_doc(&mut doc)
}

pub fn redact_text(data: &[u8], search_text: &str, replacement: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc).clone();

    // 1. Modify existing annotations containing the search text
    let mut refs_to_modify: Vec<OID> = Vec::new();
    for &page_id in &page_ids {
        if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
            if let Ok(Object::Array(annots)) = dict.get(b"Annots") {
                for annot_ref in annots {
                    if let Object::Reference(ref_id) = annot_ref {
                        if let Some(Object::Dictionary(ref annot_dict)) = doc.objects.get(ref_id) {
                            if let Ok(Object::String(bytes, _)) = annot_dict.get(b"Contents") {
                                let content_str = String::from_utf8_lossy(bytes);
                                if content_str.contains(search_text) {
                                    refs_to_modify.push(*ref_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for ref_id in refs_to_modify {
        if let Some(Object::Dictionary(ref mut annot_dict)) = doc.objects.get_mut(&ref_id) {
            if let Ok(Object::String(bytes, _)) = annot_dict.get(b"Contents") {
                let content_str = String::from_utf8_lossy(bytes);
                let updated = content_str.replace(search_text, replacement);
                annot_dict.set(
                    "Contents",
                    Object::String(updated.into_bytes(), lopdf::StringFormat::Literal),
                );
            }
        }
    }

    // 2. Modify page content streams to replace the target text in body content
    for &page_id in &page_ids {
        let mut content_ids: Vec<OID> = Vec::new();
        if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
            if let Ok(contents_obj) = dict.get(b"Contents") {
                match contents_obj {
                    Object::Reference(id) => content_ids.push(*id),
                    Object::Array(arr) => {
                        for o in arr {
                            if let Ok(id) = o.as_reference() {
                                content_ids.push(id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if content_ids.is_empty() {
            continue;
        }

        let mut operations = Vec::new();
        for cid in &content_ids {
            if let Some(Object::Stream(stream)) = doc.objects.get(cid) {
                let bytes = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                if let Ok(c) = lopdf::content::Content::decode(&bytes) {
                    operations.extend(c.operations);
                }
            }
        }

        let page_attrs = materialize_inherited_page_attrs(&doc, page_id);
        let res_dict = page_attrs.get(b"Resources").ok().and_then(|r| match r {
            Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()),
            Object::Dictionary(d) => Some(d),
            _ => None,
        });
        let font_map = extract_font_infos(res_dict, &doc);

        let mut new_operations = Vec::new();
        let mut modified = false;
        let mut current_font_info: Option<&FontInfo> = None;

        for mut op in operations {
            match op.operator.as_str() {
                "Tf" => {
                    if let Some(Object::Name(fname)) = op.operands.first() {
                        current_font_info = font_map.get(fname);
                    }
                    new_operations.push(op);
                }
                "Tj" => {
                    if let Some(Object::String(bytes, _)) = op.operands.first() {
                        let is_cid = current_font_info.map(|fi| fi.is_type0).unwrap_or(false);
                        let text = if let Some(fi) = current_font_info {
                            fi.decode(bytes)
                        } else {
                            String::from_utf8_lossy(bytes).to_string()
                        };
                        let raw_lossy = String::from_utf8_lossy(bytes);
                        if text.contains(search_text) || raw_lossy.contains(search_text) {
                            if is_cid {
                                // CID/Type0 フォント: UTF-8 をそのままバイト列として書き戻すと
                                // 2バイト CID ストリームが破壊される。このオペランドはスキップ。
                                // (テキスト検索はできるがインライン置換は非対応)
                            } else {
                                let replaced = if text.contains(search_text) {
                                    text.replace(search_text, replacement)
                                } else {
                                    raw_lossy.replace(search_text, replacement)
                                };
                                op.operands[0] =
                                    Object::String(replaced.into_bytes(), lopdf::StringFormat::Literal);
                                modified = true;
                            }
                        }
                    }
                    new_operations.push(op);
                }
                "TJ" => {
                    if let Some(Object::Array(ref mut arr)) = op.operands.first_mut() {
                        let is_cid = current_font_info.map(|fi| fi.is_type0).unwrap_or(false);
                        let mut has_match = false;
                        if !is_cid {
                            for item in arr.iter() {
                                if let Object::String(bytes, _) = item {
                                    let text = if let Some(fi) = current_font_info {
                                        fi.decode(bytes)
                                    } else {
                                        String::from_utf8_lossy(bytes).to_string()
                                    };
                                    let raw_lossy = String::from_utf8_lossy(bytes);
                                    if text.contains(search_text) || raw_lossy.contains(search_text) {
                                        has_match = true;
                                        break;
                                    }
                                }
                            }
                        }
                        if has_match {
                            for item in arr.iter_mut() {
                                if let Object::String(bytes, _) = item {
                                    let text = if let Some(fi) = current_font_info {
                                        fi.decode(bytes)
                                    } else {
                                        String::from_utf8_lossy(bytes).to_string()
                                    };
                                    let raw_lossy = String::from_utf8_lossy(bytes);
                                    if text.contains(search_text) || raw_lossy.contains(search_text) {
                                        let replaced = if text.contains(search_text) {
                                            text.replace(search_text, replacement)
                                        } else {
                                            raw_lossy.replace(search_text, replacement)
                                        };
                                        *item = Object::String(
                                            replaced.into_bytes(),
                                            lopdf::StringFormat::Literal,
                                        );
                                    }
                                }
                            }
                            modified = true;
                        }
                    }
                    new_operations.push(op);
                }
                _ => new_operations.push(op),
            }
        }

        if modified {
            let content = lopdf::content::Content {
                operations: new_operations,
            };
            if let Ok(content_bytes) = content.encode() {
                let mut stream = Stream::new(Dictionary::new(), content_bytes);
                stream.dict.set("Type", Object::Name("Content".into()));
                let new_content_id = doc.add_object(stream);
                if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
                    dict.set("Contents", Object::Reference(new_content_id));
                }
                // Do NOT call doc.objects.remove(&cid) directly, as content streams might be shared across pages.
                // prune_objects() will cleanly remove streams that are no longer referenced by any page.
            }
        }
    }

    doc.prune_objects();
    save_doc(&mut doc)
}

// ===== DEEP REDACTION (Complete Data Purging - Permanent Removal) =====

/// True when the document carries an applied digital signature
/// (an annotation with /FT or /Subtype /Sig plus a /ByteRange).
pub fn has_active_signature(data: &[u8]) -> bool {
    let doc = match Document::load_mem(data) {
        Ok(d) => d,
        Err(_) => return false,
    };
    doc.objects.iter().any(|(_, obj)| {
        match obj.as_dict() {
            Ok(d) => {
                let sig_ft = matches!(d.get(b"FT"), Ok(Object::Name(n)) if n == b"Sig");
                let sig_subtype = matches!(d.get(b"Subtype"), Ok(Object::Name(n)) if n == b"Sig");
                (sig_ft || sig_subtype) && d.get(b"ByteRange").is_ok()
            }
            Err(_) => false,
        }
    })
}

/// Redaction rewrites the whole file, so any applied signature's signed byte
/// ranges would be destroyed and the signature permanently invalidated with
/// no way back. Acrobat rejects applying redactions to signed documents for
/// the same reason; we mirror that behaviour instead of silently corrupting
/// the signature.
pub fn guard_not_signed(data: &[u8]) -> Result<(), String> {
    if has_active_signature(data) {
        return Err(
            "このPDFには適用済みのデジタル署名があります。redaction はファイル全体を書き換えるため、\
             署名の署名済みバイト列が壊れて署名が二度と検証できなくなります。\n\
             先に署名を除去するか、署名前にこの操作を行ってください。"
                .to_string(),
        );
    }
    Ok(())
}

fn rect_array_intersects(rect: &[Object], x: f64, y: f64, width: f64, height: f64) -> bool {
    if rect.len() < 4 {
        return false;
    }
    let num = |o: &Object| -> f64 {
        match o {
            Object::Real(v) => *v as f64,
            Object::Integer(v) => *v as f64,
            _ => 0.0,
        }
    };
    let (x0, y0, x1, y1) = (num(&rect[0]), num(&rect[1]), num(&rect[2]), num(&rect[3]));
    let (lx, hx) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (ly, hy) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    lx < x + width && hx > x && ly < y + height && hy > y
}

fn field_tree_intersects(doc: &Document, id: lopdf::ObjectId, x: f64, y: f64, width: f64, height: f64) -> bool {
    if let Some(Object::Dictionary(dict)) = doc.objects.get(&id) {
        if let Ok(Object::Array(rect)) = dict.get(b"Rect") {
            if rect_array_intersects(rect, x, y, width, height) {
                return true;
            }
        }
        if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
            for kid in kids {
                if let Ok(kid_id) = kid.as_reference() {
                    if field_tree_intersects(doc, kid_id, x, y, width, height) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn collect_field_tree_ids(doc: &Document, id: lopdf::ObjectId) -> Vec<lopdf::ObjectId> {
    let mut ids = vec![id];
    if let Some(Object::Dictionary(dict)) = doc.objects.get(&id) {
        if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
            for kid in kids {
                if let Ok(kid_id) = kid.as_reference() {
                    ids.extend(collect_field_tree_ids(doc, kid_id));
                }
            }
        }
    }
    ids
}

pub fn deep_redact(
    data: &[u8],
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: &str,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let (r, g, b) = parse_hex_color(color, (0.0, 0.0, 0.0));

    let page_id = page_ids[page_index];

    // Step 1: Get existing content stream and remove text in redacted area
    let mut content_ids: Vec<OID> = Vec::new();
    if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
        if let Ok(contents_obj) = dict.get(b"Contents") {
            match contents_obj {
                Object::Reference(id) => content_ids.push(*id),
                Object::Array(arr) => {
                    for o in arr {
                        if let Ok(id) = o.as_reference() {
                            content_ids.push(id);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut new_operations = Vec::new();

    for cid in &content_ids {
        if let Some(Object::Stream(stream)) = doc.objects.get(cid) {
            let stream_bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            if let Ok(content) = lopdf::content::Content::decode(&stream_bytes) {
                let mut current_x = 0.0f32;
                let mut current_y = 0.0f32;
                let mut in_text = false;

                let as_num = |obj: &Object| -> Option<f32> {
                    match obj {
                        Object::Real(f) => Some(*f),
                        Object::Integer(i) => Some(*i as f32),
                        _ => None,
                    }
                };

                for op in &content.operations {
                    match op.operator.as_str() {
                        "BT" => {
                            in_text = true;
                            new_operations.push(op.clone());
                        }
                        "ET" => {
                            in_text = false;
                            new_operations.push(op.clone());
                        }
                        "Tm" => {
                            // Text matrix
                            if op.operands.len() >= 6 {
                                if let (Some(_a), Some(_b), Some(_c), Some(_d), Some(e), Some(f)) = (
                                    as_num(&op.operands[0]),
                                    as_num(&op.operands[1]),
                                    as_num(&op.operands[2]),
                                    as_num(&op.operands[3]),
                                    as_num(&op.operands[4]),
                                    as_num(&op.operands[5]),
                                ) {
                                    current_x = e;
                                    current_y = f;
                                }
                            }
                            new_operations.push(op.clone());
                        }
                        "Td" | "TD" => {
                            if let (Some(dx), Some(dy)) = (
                                op.operands.first().and_then(as_num),
                                op.operands.get(1).and_then(as_num),
                            ) {
                                current_x += dx;
                                current_y += dy;
                            }
                            new_operations.push(op.clone());
                        }
                        "Tj" | "TJ" => {
                            if in_text {
                                // Check if text is in redacted area
                                let text_in_area = current_x >= x as f32
                                    && current_x <= (x + width) as f32
                                    && current_y >= y as f32
                                    && current_y <= (y + height) as f32;

                                if text_in_area {
                                    // Skip this text operation (remove it completely)
                                    continue;
                                }
                            }
                            new_operations.push(op.clone());
                        }
                        _ => new_operations.push(op.clone()),
                    }
                }
            }
        }
    }

    // Step 2: Physical raster eradication — overwrite the actual pixels under
    // the redaction rectangle (CTM-aware placement, Form XObjects, SMask/Mask
    // coverage, lossless Flate re-encode) instead of relying on a black box
    // drawn over intact raster data.
    let fill_rgb = (
        (r * 255.0).round().clamp(0.0, 255.0) as u8,
        (g * 255.0).round().clamp(0.0, 255.0) as u8,
        (b * 255.0).round().clamp(0.0, 255.0) as u8,
    );
    let _raster = crate::pdf_engine::raster_redact::pixel_redact_page(
        &mut doc,
        page_id,
        (x, y, x + width, y + height),
        fill_rgb,
        2,
    )?;
    // Step 3: Draw opaque redaction box
    new_operations.push(lopdf::content::Operation::new("q", vec![]));
    new_operations.push(lopdf::content::Operation::new(
        "rg",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    ));
    new_operations.push(lopdf::content::Operation::new(
        "re",
        vec![
            Object::Real(x as f32),
            Object::Real(y as f32),
            Object::Real(width as f32),
            Object::Real(height as f32),
        ],
    ));
    new_operations.push(lopdf::content::Operation::new("f", vec![]));
    new_operations.push(lopdf::content::Operation::new("Q", vec![]));

    let content = lopdf::content::Content {
        operations: new_operations,
    };
    let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

    let mut stream = Stream::new(Dictionary::new(), content_bytes);
    stream.dict.set("Type", Object::Name("Content".into()));
    let new_content_id = doc.add_object(stream);

    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        dict.set("Contents", Object::Reference(new_content_id));
    }

    // Step 4: Remove all annotations in the redacted area
    let mut annots_to_keep = Vec::new();
    let mut annots_to_remove = Vec::new();

    // First pass: collect which annotations to remove
    if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
        if let Ok(Object::Array(annots)) = dict.get(b"Annots") {
            for annot_ref in annots {
                if let Object::Reference(ref_id) = annot_ref {
                    let mut should_remove = false;
                    if let Some(Object::Dictionary(annot_dict)) = doc.objects.get(ref_id) {
                        if let Ok(Object::Array(rect)) = annot_dict.get(b"Rect") {
                            if rect.len() >= 4 {
                                let ax = match &rect[0] {
                                    Object::Real(v) => *v as f64,
                                    Object::Integer(v) => *v as f64,
                                    _ => 0.0,
                                };
                                let ay = match &rect[1] {
                                    Object::Real(v) => *v as f64,
                                    Object::Integer(v) => *v as f64,
                                    _ => 0.0,
                                };
                                let aw = match &rect[2] {
                                    Object::Real(v) => *v as f64,
                                    Object::Integer(v) => *v as f64,
                                    _ => 0.0,
                                };
                                let ah = match &rect[3] {
                                    Object::Real(v) => *v as f64,
                                    Object::Integer(v) => *v as f64,
                                    _ => 0.0,
                                };

                                // Check if annotation overlaps with redaction area
                                if ax < x + width && ax + aw > x && ay < y + height && ay + ah > y {
                                    should_remove = true;
                                    annots_to_remove.push(*ref_id);
                                }
                            }
                        }
                    }
                    if !should_remove {
                        annots_to_keep.push(annot_ref.clone());
                    }
                }
            }
        }
    }

    // Second pass: remove the annotation objects
    for ref_id in annots_to_remove {
        doc.objects.remove(&ref_id);
    }

    // Update annotations array
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        dict.set("Annots", Object::Array(annots_to_keep));
    }

    // Clean up old unreferenced content stream objects
    for cid in content_ids {
        if cid != new_content_id {
            doc.objects.remove(&cid);
        }
    }

    // Step 5: Eradicate form fields whose widget (or inherited) Rect
    // intersects the redacted area. Widget appearance streams live outside
    // the page content stream, so without this pass a redaction could leave
    // field text on screen or recoverable through /V default values.
    let acroform_id: Option<lopdf::ObjectId> = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|r| r.as_reference().ok())
        .and_then(|root_id| doc.objects.get(&root_id))
        .and_then(|catalog| catalog.as_dict().ok())
        .and_then(|c| c.get(b"AcroForm").ok())
        .and_then(|a| a.as_reference().ok());
    if let Some(form_id) = acroform_id {
        let fields: Vec<Object> = doc
            .objects
            .get(&form_id)
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(b"Fields").ok())
            .and_then(|f| f.as_array().ok())
            .cloned()
            .unwrap_or_default();
        // Fields whose objects were already destroyed by the annotation pass
        // (widget annotations share objects with the field tree) count as
        // doomed too, otherwise /Fields would keep dangling references.
        let doomed: Vec<lopdf::ObjectId> = fields
            .iter()
            .filter_map(|f| f.as_reference().ok())
            .filter(|fid| match doc.objects.get(fid) {
                None => true,
                Some(_) => field_tree_intersects(&doc, *fid, x, y, width, height),
            })
            .collect();
        if !doomed.is_empty() {
            let mut dead_ids: Vec<lopdf::ObjectId> = Vec::new();
            for fid in &doomed {
                dead_ids.extend(collect_field_tree_ids(&doc, *fid));
            }
            // Strip widget references from every page /Annots array first so
            // the removed objects cannot leave dangling references behind.
            let dead_fields: Vec<Object> = fields
                .iter()
                .filter(|f| match f.as_reference() {
                    Ok(id) => !doomed.contains(&id),
                    Err(_) => true,
                })
                .cloned()
                .collect();
            for pid in &page_ids {
                let cleaned: Option<(Vec<Object>, usize)> = doc
                    .objects
                    .get(pid)
                    .and_then(|o| o.as_dict().ok())
                    .and_then(|d| d.get(b"Annots").ok())
                    .cloned()
                    .and_then(|a| a.as_array().ok().cloned())
                    .map(|arr| {
                        let kept: Vec<Object> = arr
                            .iter()
                            .filter(|o| match o.as_reference() {
                                Ok(id) => !dead_ids.contains(&id),
                                Err(_) => true,
                            })
                            .cloned()
                            .collect();
                        (kept, arr.len())
                    });
                if let Some((kept, original_len)) = cleaned {
                    if kept.len() != original_len {
                        if let Some(Object::Dictionary(d)) = doc.objects.get_mut(pid) {
                            d.set("Annots", Object::Array(kept));
                        }
                    }
                }
            }
            for id in dead_ids {
                doc.objects.remove(&id);
            }
            if let Some(Object::Dictionary(form_dict)) = doc.objects.get_mut(&form_id) {
                form_dict.set("Fields", Object::Array(dead_fields));
            }
        }
    }

    // Prune unreferenced objects across the document
    doc.prune_objects();

    save_doc(&mut doc)
}

// ===== REDACTION WITH TEXT SEARCH =====

pub fn redact_text_deep(data: &[u8], search_text: &str, color: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let (_r, _g, _b) = parse_hex_color(color, (0.0, 0.0, 0.0));

    let page_ids = get_page_ids(&doc).clone();

    for &page_id in &page_ids {
        // Get all content stream IDs
        let mut content_ids: Vec<OID> = Vec::new();
        if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
            if let Ok(contents_obj) = dict.get(b"Contents") {
                match contents_obj {
                    Object::Reference(id) => content_ids.push(*id),
                    Object::Array(arr) => {
                        for o in arr {
                            if let Ok(id) = o.as_reference() {
                                content_ids.push(id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if content_ids.is_empty() {
            continue;
        }

        let mut operations = Vec::new();
        for cid in &content_ids {
            if let Some(Object::Stream(stream)) = doc.objects.get(cid) {
                let bytes = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                if let Ok(c) = lopdf::content::Content::decode(&bytes) {
                    operations.extend(c.operations);
                }
            }
        }

        let page_attrs = materialize_inherited_page_attrs(&doc, page_id);
        let res_dict = page_attrs.get(b"Resources").ok().and_then(|r| match r {
            Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()),
            Object::Dictionary(d) => Some(d),
            _ => None,
        });
        let font_map = extract_font_infos(res_dict, &doc);

        let mut new_operations = Vec::new();
        let mut in_text = false;
        let mut current_x = 0.0f32;
        let mut current_y = 0.0f32;
        let mut current_font_info: Option<&FontInfo> = None;

        for op in &operations {
            match op.operator.as_str() {
                "BT" => {
                    in_text = true;
                    new_operations.push(op.clone());
                }
                "ET" => {
                    in_text = false;
                    new_operations.push(op.clone());
                }
                "Tf" => {
                    if let Some(Object::Name(fname)) = op.operands.first() {
                        current_font_info = font_map.get(fname);
                    }
                    new_operations.push(op.clone());
                }
                "Tm" => {
                    if op.operands.len() >= 6 {
                        if let Object::Real(e) = &op.operands[4] {
                            current_x = *e;
                        }
                        if let Object::Real(f) = &op.operands[5] {
                            current_y = *f;
                        }
                    }
                    new_operations.push(op.clone());
                }
                "Td" | "TD" => {
                    if let (Some(Object::Real(dx)), Some(Object::Real(dy))) =
                        (op.operands.first(), op.operands.get(1))
                    {
                        current_x += dx;
                        current_y += dy;
                    }
                    new_operations.push(op.clone());
                }
                "Tj" => {
                    if in_text {
                        if let Some(Object::String(bytes, _)) = op.operands.first() {
                            let text = if let Some(fi) = current_font_info {
                                fi.decode(bytes)
                            } else {
                                String::from_utf8_lossy(bytes).to_string()
                            };
                            let raw_lossy = String::from_utf8_lossy(bytes);
                            if text.contains(search_text) || raw_lossy.contains(search_text) {
                                // Remove this text completely
                                continue;
                            }
                        }
                    }
                    new_operations.push(op.clone());
                }
                "TJ" => {
                    if in_text {
                        if let Some(Object::Array(arr)) = op.operands.first() {
                            let mut combined_decoded = String::new();
                            let mut combined_lossy = String::new();
                            for item in arr {
                                if let Object::String(bytes, _) = item {
                                    let text = if let Some(fi) = current_font_info {
                                        fi.decode(bytes)
                                    } else {
                                        String::from_utf8_lossy(bytes).to_string()
                                    };
                                    combined_decoded.push_str(&text);
                                    combined_lossy.push_str(&String::from_utf8_lossy(bytes));
                                }
                            }
                            if combined_decoded.contains(search_text) || combined_lossy.contains(search_text) {
                                continue;
                            }
                        }
                    }
                    new_operations.push(op.clone());
                }
                _ => new_operations.push(op.clone()),
            }
        }

        // Create new content stream
        let content = lopdf::content::Content {
            operations: new_operations,
        };
        let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

        let mut stream = Stream::new(Dictionary::new(), content_bytes);
        stream.dict.set("Type", Object::Name("Content".into()));
        let new_content_id = doc.add_object(stream);

        if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
            dict.set("Contents", Object::Reference(new_content_id));
        }

        // Avoid removing cid manually to protect shared streams; prune_objects handles unreferenced streams safely
    }

    // Also traverse and purge target text from all Form XObjects in the document (font-aware!)
    let form_xobject_ids: Vec<OID> = doc
        .objects
        .iter()
        .filter_map(|(&oid, obj)| {
            if let Object::Stream(ref stream) = obj {
                if stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|s| s.as_name().ok())
                    == Some(b"Form")
                {
                    Some(oid)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    for xoid in form_xobject_ids {
        // First inspect Form XObject resources for fonts (extract owned FontInfo)
        let xobj_font_map = if let Some(Object::Stream(ref stream)) = doc.objects.get(&xoid) {
            let res = stream.dict.get(b"Resources").ok().and_then(|r| match r {
                Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()),
                Object::Dictionary(d) => Some(d),
                _ => None,
            });
            extract_font_infos(res, &doc)
        } else {
            HashMap::new()
        };

        if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&xoid) {
            let bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            if let Ok(content) = lopdf::content::Content::decode(&bytes) {
                let mut new_ops = Vec::new();
                let mut modified = false;
                let mut current_font_info: Option<&FontInfo> = None;

                for op in content.operations {
                    match op.operator.as_str() {
                        "Tf" => {
                            if let Some(Object::Name(fname)) = op.operands.first() {
                                current_font_info = xobj_font_map.get(fname);
                            }
                            new_ops.push(op);
                        }
                        "Tj" => {
                            if let Some(Object::String(b, _)) = op.operands.first() {
                                let decoded = if let Some(fi) = current_font_info {
                                    fi.decode(b)
                                } else {
                                    String::from_utf8_lossy(b).to_string()
                                };
                                let lossy = String::from_utf8_lossy(b);
                                if decoded.contains(search_text) || lossy.contains(search_text) {
                                    modified = true;
                                    continue;
                                }
                            }
                            new_ops.push(op);
                        }
                        "TJ" => {
                            if let Some(Object::Array(arr)) = op.operands.first() {
                                let mut combined_decoded = String::new();
                                let mut combined_lossy = String::new();
                                for item in arr {
                                    if let Object::String(b, _) = item {
                                        let text = if let Some(fi) = current_font_info {
                                            fi.decode(b)
                                        } else {
                                            String::from_utf8_lossy(b).to_string()
                                        };
                                        combined_decoded.push_str(&text);
                                        combined_lossy.push_str(&String::from_utf8_lossy(b));
                                    }
                                }
                                if combined_decoded.contains(search_text) || combined_lossy.contains(search_text) {
                                    modified = true;
                                    continue;
                                }
                            }
                            new_ops.push(op);
                        }
                        _ => new_ops.push(op),
                    }
                }

                if modified {
                    let updated = lopdf::content::Content {
                        operations: new_ops,
                    };
                    if let Ok(encoded) = updated.encode() {
                        stream.set_plain_content(encoded); // #49 是正: FlateDecode Filter残存防止
                    }
                }
            }
        }
    }

    // Prune any unreferenced objects across the document
    doc.prune_objects();

    save_doc(&mut doc)
}
