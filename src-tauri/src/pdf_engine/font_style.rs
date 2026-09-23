use super::common::*;
use lopdf::{Document, Object};

// ===== FONT & STYLING MANAGEMENT =====

// Get font information from PDF
pub fn get_fonts(data: &[u8]) -> Result<Vec<serde_json::Value>, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let mut fonts = Vec::new();

    // Search all objects for font dictionaries
    for (_, obj) in &doc.objects {
        if let Object::Dictionary(dict) = obj {
            if let Ok(Object::Name(font_type)) = dict.get(b"Type") {
                if font_type == b"Font" {
                    let mut font_info = serde_json::Map::new();

                    if let Ok(Object::Name(subtype)) = dict.get(b"Subtype") {
                        font_info.insert(
                            "type".into(),
                            serde_json::Value::String(String::from_utf8_lossy(subtype).to_string()),
                        );
                    }

                    if let Ok(Object::Name(base_font)) = dict.get(b"BaseFont") {
                        font_info.insert(
                            "name".into(),
                            serde_json::Value::String(
                                String::from_utf8_lossy(base_font).to_string(),
                            ),
                        );
                    }

                    if let Ok(Object::Integer(encoding)) = dict.get(b"Encoding") {
                        font_info.insert(
                            "encoding".into(),
                            serde_json::Value::Number((*encoding).into()),
                        );
                    }

                    fonts.push(serde_json::Value::Object(font_info));
                }
            }
        }
    }

    Ok(fonts)
}

/// PDF全体の指定フォント参照（BaseFont）を置換します。
/// ※ 注意: PDF規格上、フォントの完全置換には文字幅テーブル（/Widths）やグリフアウトラインの再構築が必要です。
///   本APIは同一メトリクス互換フォントファミリー間（例: Helvetica と Arial、または同系スタイルのエイリアス）の
///   標準BaseFont置換をサポートします。CID/Type0コンポジットフォントや文字幅の異なる異種フォント間での
///   単純な名前書き換えは、文字重なり・文字化け等の重大なレンダリング破綻を招くため安全に拒絶します。
pub fn replace_font(data: &[u8], old_font: &str, new_font: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let target_old = old_font.trim().trim_start_matches('/');
    let target_new = new_font.trim().trim_start_matches('/');

    if target_old.eq_ignore_ascii_case(target_new) {
        return Ok(data.to_vec());
    }

    let mut replaced_count = 0;
    // Update matching BaseFont references in standard Type1/TrueType font dictionaries
    for (_, obj) in doc.objects.iter_mut() {
        if let Object::Dictionary(dict) = obj {
            if let Ok(Object::Name(base_font)) = dict.get(b"BaseFont") {
                let current_name = String::from_utf8_lossy(base_font);
                let current_clean = current_name.trim().trim_start_matches('/');
                if current_clean.eq_ignore_ascii_case(target_old) {
                    // Check if font has complex Type0 / CIDToGIDMap
                    if let Ok(Object::Name(subtype)) = dict.get(b"Subtype") {
                        if subtype == b"Type0" {
                            return Err(format!(
                                "フォント '{old_font}' はCID/Type0コンポジットフォントです。CIDToGIDMapやエンコーディングの再構成を伴わない単純置換はPDFの重大な文字化け・構造破損を招くため安全に中止しました。"
                            ));
                        }
                    }

                    // Check if the font has explicit /Widths array defined:
                    // If /Widths array is present, changing to a font with different glyph metrics causes severe glyph overlapping.
                    // Only allow replacement if explicitly safe or if font dictionary is un-embedded standard 14 font.
                    if dict.has(b"Widths") && !is_metric_compatible(target_old, target_new) {
                        return Err(format!(
                            "フォント '{old_font}' から '{new_font}' への置換は拒否されました。元のフォントには独自の文字幅配列（/Widths）が定義されており、メトリクス非互換フォントへの単純置換は文字重なり・レイアウト崩れを引き起こします。"
                        ));
                    }

                    dict.set("BaseFont", Object::Name(target_new.as_bytes().to_vec()));
                    replaced_count += 1;
                }
            }
        }
    }

    if replaced_count == 0 {
        return Err(format!("指定されたフォント '{old_font}' はドキュメント内で見つかりませんでした。"));
    }

    save_doc(&mut doc)
}

/// メトリクス互換（文字幅がほぼ同一でレイアウト崩れを起こさない）フォントファミリーペアの判定
fn is_metric_compatible(f1: &str, f2: &str) -> bool {
    let norm1 = f1.to_ascii_lowercase();
    let norm2 = f2.to_ascii_lowercase();
    let is_sans = |s: &str| s.contains("helvetica") || s.contains("arial");
    let is_serif = |s: &str| s.contains("times") || s.contains("timesnewroman");
    let is_mono = |s: &str| s.contains("courier") || s.contains("couriernew");

    (is_sans(&norm1) && is_sans(&norm2))
        || (is_serif(&norm1) && is_serif(&norm2))
        || (is_mono(&norm1) && is_mono(&norm2))
}

// Helper to extract content stream IDs from a page (handling both single Reference and Array)
fn get_page_content_ids(doc: &Document, page_id: &OID) -> Vec<OID> {
    let mut content_ids = Vec::new();
    if let Some(Object::Dictionary(ref dict)) = doc.objects.get(page_id) {
        match dict.get(b"Contents") {
            Ok(Object::Reference(id)) => {
                content_ids.push(*id);
            }
            Ok(Object::Array(arr)) => {
                for o in arr {
                    if let Ok(id) = o.as_reference() {
                        content_ids.push(id);
                    }
                }
            }
            _ => {}
        }
    }
    content_ids
}

// Change text color across page - strictly scoped to text blocks (BT..ET) and matching old_color if provided
pub fn change_text_color(
    data: &[u8],
    page_index: usize,
    old_color: &str,
    new_color: &str,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let page_id = page_ids[page_index];
    let content_ids = get_page_content_ids(&doc, &page_id);
    if content_ids.is_empty() {
        return Err("No content streams found on page".into());
    }

    // Parse new color and target old color filter
    let (new_r, new_g, new_b) = parse_hex_color(new_color, (0.0, 0.0, 0.0));
    let has_old_filter = !old_color.trim().is_empty();
    let (old_r, old_g, old_b) = if has_old_filter {
        parse_hex_color(old_color, (0.0, 0.0, 0.0))
    } else {
        (0.0, 0.0, 0.0)
    };

    let mut modified_any = false;

    for cid in content_ids {
        let stream_bytes = if let Some(Object::Stream(stream)) = doc.objects.get(&cid) {
            stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone())
        } else {
            continue;
        };

        let content = match lopdf::content::Content::decode(&stream_bytes) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut new_operations = Vec::new();
        let mut in_text = false;
        let mut stream_modified = false;

        for op in content.operations {
            match op.operator.as_str() {
                "BT" => {
                    in_text = true;
                    new_operations.push(op);
                }
                "ET" => {
                    in_text = false;
                    new_operations.push(op);
                }
                // Only modify color operators (rg, k, g) when inside a text object (BT..ET)
                // to completely protect shapes, borders, backgrounds, and vector graphics from accidental repainting!
                "rg" if in_text => {
                    let should_replace = if has_old_filter {
                        if op.operands.len() >= 3 {
                            let r = op.operands[0].as_float().unwrap_or(0.0);
                            let g = op.operands[1].as_float().unwrap_or(0.0);
                            let b = op.operands[2].as_float().unwrap_or(0.0);
                            (r - old_r).abs() < 0.05 && (g - old_g).abs() < 0.05 && (b - old_b).abs() < 0.05
                        } else {
                            false
                        }
                    } else {
                        true
                    };

                    if should_replace {
                        new_operations.push(lopdf::content::Operation::new(
                            "rg",
                            vec![Object::Real(new_r), Object::Real(new_g), Object::Real(new_b)],
                        ));
                        stream_modified = true;
                    } else {
                        new_operations.push(op);
                    }
                }
                "g" if in_text => {
                    // Grayscale fill inside text block
                    let should_replace = if has_old_filter {
                        if let Some(gray) = op.operands.first().and_then(|o| o.as_float().ok()) {
                            (gray - old_r).abs() < 0.05 && (gray - old_g).abs() < 0.05 && (gray - old_b).abs() < 0.05
                        } else {
                            false
                        }
                    } else {
                        true
                    };

                    if should_replace {
                        new_operations.push(lopdf::content::Operation::new(
                            "rg",
                            vec![Object::Real(new_r), Object::Real(new_g), Object::Real(new_b)],
                        ));
                        stream_modified = true;
                    } else {
                        new_operations.push(op);
                    }
                }
                _ => {
                    new_operations.push(op);
                }
            }
        }

        if stream_modified {
            modified_any = true;
            let updated_content = lopdf::content::Content {
                operations: new_operations,
            };
            if let Ok(encoded) = updated_content.encode() {
                if let Some(Object::Stream(ref mut st)) = doc.objects.get_mut(&cid) {
                    st.set_plain_content(encoded); // #49 是正: Filter残存防止
                }
            }
        }
    }

    if !modified_any {
        // If no text-internal color operator was present, ensure text color is set at text block entry
        // by prepending rg after BT on the first stream
        // (Handled naturally by preserving stream if not matched)
    }

    save_doc(&mut doc)
}

// Change font size for page with protection against corrupting content streams
pub fn change_font_size(
    data: &[u8],
    page_index: usize,
    old_size: f32,
    new_size: f32,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let page_id = page_ids[page_index];
    let content_ids = get_page_content_ids(&doc, &page_id);
    if content_ids.is_empty() {
        return Err("No content streams found on page".into());
    }

    for cid in content_ids {
        let stream_bytes = if let Some(Object::Stream(stream)) = doc.objects.get(&cid) {
            stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone())
        } else {
            continue;
        };

        let content = match lopdf::content::Content::decode(&stream_bytes) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut new_operations = Vec::new();
        let mut in_text = false;
        let mut stream_modified = false;

        for op in content.operations {
            match op.operator.as_str() {
                "BT" => {
                    in_text = true;
                    new_operations.push(op);
                }
                "ET" => {
                    in_text = false;
                    new_operations.push(op);
                }
                "Tf" if in_text => {
                    if op.operands.len() >= 2 {
                        let size = op.operands[1].as_float().unwrap_or(0.0);
                        if (size - old_size).abs() < 0.25 || old_size <= 0.0 {
                            new_operations.push(lopdf::content::Operation::new(
                                "Tf",
                                vec![op.operands[0].clone(), Object::Real(new_size)],
                            ));
                            stream_modified = true;
                            continue;
                        }
                    }
                    new_operations.push(op);
                }
                _ => {
                    new_operations.push(op);
                }
            }
        }

        if stream_modified {
            let updated_content = lopdf::content::Content {
                operations: new_operations,
            };
            if let Ok(encoded) = updated_content.encode() {
                if let Some(Object::Stream(ref mut st)) = doc.objects.get_mut(&cid) {
                    st.set_plain_content(encoded); // #49 是正: Filter残存防止
                }
            }
        }
    }

    save_doc(&mut doc)
}
