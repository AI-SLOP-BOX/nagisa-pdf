use super::common::*;
use lopdf::{Dictionary, Document, Object, Stream};

// ===== COLOR MANAGEMENT (CMYK) =====

pub fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let k = 1.0 - r.max(g).max(b);
    if k >= 1.0 {
        return (0, 0, 0, 255);
    }

    let c = ((1.0 - r - k) / (1.0 - k) * 100.0) as u8;
    let m = ((1.0 - g - k) / (1.0 - k) * 100.0) as u8;
    let y = ((1.0 - b - k) / (1.0 - k) * 100.0) as u8;
    let k = (k * 100.0) as u8;

    (c, m, y, k)
}

pub fn cmyk_to_rgb(c: u8, m: u8, y: u8, k: u8) -> (u8, u8, u8) {
    let c = c as f32 / 100.0;
    let m = m as f32 / 100.0;
    let y = y as f32 / 100.0;
    let k = k as f32 / 100.0;

    let r = 255.0 * (1.0 - c) * (1.0 - k);
    let g = 255.0 * (1.0 - m) * (1.0 - k);
    let b = 255.0 * (1.0 - y) * (1.0 - k);

    (r as u8, g as u8, b as u8)
}

pub fn convert_to_cmyk(data: &[u8]) -> Result<Vec<u8>, String> {
    // If input is a PDF document, convert embedded RGB images to CMYK and attach CMYK output intent
    if let Ok(mut doc) = Document::load_mem(data) {
        let mut images_to_convert: Vec<OID> = Vec::new();

        for (&id, obj) in doc.objects.iter() {
            if let Object::Stream(ref stream) = obj {
                if let Ok(Object::Name(ref subtype)) = stream.dict.get(b"Subtype") {
                    if subtype == b"Image" {
                        if let Ok(Object::Name(ref cs)) = stream.dict.get(b"ColorSpace") {
                            if cs == b"DeviceRGB" {
                                images_to_convert.push(id);
                            }
                        }
                    }
                }
            }
        }

        for img_id in images_to_convert {
            if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&img_id) {
                let width = stream.dict.get(b"Width").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0) as u32;
                let height = stream.dict.get(b"Height").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0) as u32;

                if width > 0 && height > 0 {
                    let decoded_bytes = stream.decompressed_content()
                        .or_else(|_| image::load_from_memory(&stream.content).map(|img| img.to_rgb8().into_raw()).map_err(|e| e.to_string()))
                        .unwrap_or_else(|_| stream.content.clone());

                    if decoded_bytes.len() >= (width * height * 3) as usize {
                        let bytes = decoded_bytes;
                        let mut cmyk = Vec::with_capacity((width * height * 4) as usize);
                            for chunk in bytes.chunks_exact(3) {
                                let (c, m, y, k) = rgb_to_cmyk(chunk[0], chunk[1], chunk[2]);
                                cmyk.push(c);
                                cmyk.push(m);
                                cmyk.push(y);
                                cmyk.push(k);
                            }
                            use std::io::Write;
                            use flate2::write::ZlibEncoder;
                            use flate2::Compression;

                            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                            let _ = encoder.write_all(&cmyk);
                            let compressed = encoder.finish().unwrap_or(cmyk);

                            stream.set_content(compressed);
                            stream.dict.set("ColorSpace", Object::Name(b"DeviceCMYK".to_vec()));
                            stream.dict.set("BitsPerComponent", Object::Integer(8));
                            stream.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
                        }
                    }
                }
            }

        let saved = save_doc(&mut doc)?;

        // Also convert RGB operators in content streams (text/vector) to CMYK
        let with_vectors_converted = convert_rgb_operators_in_streams(&saved)?;
        return set_cmyk_output_intent(&with_vectors_converted, "Japan Color 2001 Coated");
    }

    // Fallback if data is a standalone image (PNG/JPEG)
    let img = image::load_from_memory(data).map_err(|e| format!("Failed to parse as PDF or image: {e}"))?;
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();

    let mut cmyk_data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let pixel = rgb.get_pixel(x, y);
            let (c, m, y_val, k) = rgb_to_cmyk(pixel[0], pixel[1], pixel[2]);
            cmyk_data.push(c);
            cmyk_data.push(m);
            cmyk_data.push(y_val);
            cmyk_data.push(k);
        }
    }

    Ok(cmyk_data)
}

/// Convert `rg`/`RG` (RGB fill/stroke) operators in all page content streams to `k`/`K` (CMYK).
/// This ensures that text and vector graphics are also converted to CMYK,
/// not just embedded raster images.
fn convert_rgb_operators_in_streams(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);

    for &page_id in &page_ids {
        let content_ids = resolve_page_content_stream_ids(&doc, page_id);
        if content_ids.is_empty() {
            continue;
        }

        // Convert RGB operators in-place within each individual content stream.
        // This preserves multiple stream separation, graphics state (q/Q) isolation,
        // and avoids leaving orphaned zombie streams in the PDF object table.
        for cid in content_ids {
            if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&cid) {
                let bytes = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
                if let Ok(content) = lopdf::content::Content::decode(&bytes) {
                    let new_ops: Vec<lopdf::content::Operation> = content.operations.into_iter().map(|mut op| {
                        match op.operator.as_str() {
                            // rg: fill color (RGB) -> k: fill color (CMYK)
                            "rg" if op.operands.len() >= 3 => {
                                if let (Some(r), Some(g), Some(b)) = (
                                    op.operands[0].as_float().ok(),
                                    op.operands[1].as_float().ok(),
                                    op.operands[2].as_float().ok(),
                                ) {
                                    let ri = (r * 255.0).clamp(0.0, 255.0) as u8;
                                    let gi = (g * 255.0).clamp(0.0, 255.0) as u8;
                                    let bi = (b * 255.0).clamp(0.0, 255.0) as u8;
                                    let (c, m, y, k) = rgb_to_cmyk(ri, gi, bi);
                                    op.operator = "k".to_string();
                                    op.operands = vec![
                                        Object::Real(c as f32 / 100.0),
                                        Object::Real(m as f32 / 100.0),
                                        Object::Real(y as f32 / 100.0),
                                        Object::Real(k as f32 / 100.0),
                                    ];
                                }
                                op
                            }
                            // RG: stroke color (RGB) -> K: stroke color (CMYK)
                            "RG" if op.operands.len() >= 3 => {
                                if let (Some(r), Some(g), Some(b)) = (
                                    op.operands[0].as_float().ok(),
                                    op.operands[1].as_float().ok(),
                                    op.operands[2].as_float().ok(),
                                ) {
                                    let ri = (r * 255.0).clamp(0.0, 255.0) as u8;
                                    let gi = (g * 255.0).clamp(0.0, 255.0) as u8;
                                    let bi = (b * 255.0).clamp(0.0, 255.0) as u8;
                                    let (c, m, y, k) = rgb_to_cmyk(ri, gi, bi);
                                    op.operator = "K".to_string();
                                    op.operands = vec![
                                        Object::Real(c as f32 / 100.0),
                                        Object::Real(m as f32 / 100.0),
                                        Object::Real(y as f32 / 100.0),
                                        Object::Real(k as f32 / 100.0),
                                    ];
                                }
                                op
                            }
                            _ => op,
                        }
                    }).collect();

                    let new_content = lopdf::content::Content { operations: new_ops };
                    if let Ok(encoded) = new_content.encode() {
                        stream.set_content(encoded);
                        stream.dict.remove(b"Filter"); // Update to raw encoded bytes
                    }
                }
            }
        }
    }

    save_doc(&mut doc)
}

// Set CMYK output intent with standard ICC Profile stream
pub fn set_cmyk_output_intent(data: &[u8], profile_name: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let root_id = doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference())
        .map_err(|_| "No root catalog in PDF".to_string())?;

    let icc_bytes = super::pdf_x::generate_valid_cmyk_icc(profile_name);
    let mut icc_dict = Dictionary::new();
    icc_dict.set("N", Object::Integer(4));
    let profile_stream = Stream::new(icc_dict, icc_bytes);
    let profile_id = doc.add_object(Object::Stream(profile_stream));

    let mut intent_dict = Dictionary::new();
    intent_dict.set("Type", Object::Name("OutputIntent".into()));
    intent_dict.set("S", Object::Name("GTS_PDFX".into()));
    intent_dict.set(
        "OutputConditionIdentifier",
        Object::String(profile_name.as_bytes().to_vec(), lopdf::StringFormat::Literal),
    );
    intent_dict.set(
        "Info",
        Object::String(format!("Output Profile: {profile_name}").into_bytes(), lopdf::StringFormat::Literal),
    );
    intent_dict.set("DestOutputProfile", Object::Reference(profile_id));
    let intent_id = doc.add_object(Object::Dictionary(intent_dict));

    // Extract existing intents array or reference first to avoid borrow conflicts
    let mut intents = Vec::new();
    if let Some(Object::Dictionary(root_dict)) = doc.objects.get(&root_id) {
        if let Ok(existing) = root_dict.get(b"OutputIntents") {
            match existing {
                Object::Array(arr) => intents = arr.clone(),
                Object::Reference(ref_id) => {
                    if let Some(arr) = doc.objects.get(ref_id).and_then(|o| o.as_array().ok()) {
                        intents = arr.clone();
                    }
                }
                _ => {}
            }
        }
    }
    intents.push(Object::Reference(intent_id));

    if let Some(Object::Dictionary(ref mut root_dict)) = doc.objects.get_mut(&root_id) {
        root_dict.set("OutputIntents", Object::Array(intents));
    }

    save_doc(&mut doc)
}

// Alias for embed_icc_profile for backward compatibility
pub fn embed_icc_profile(data: &[u8], profile_name: &str) -> Result<Vec<u8>, String> {
    set_cmyk_output_intent(data, profile_name)
}

// ===== ADVANCED PDF OPTIMIZATION =====

pub fn downsample_images(data: &[u8], target_dpi: u32, quality: u8) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let mut images_to_update: Vec<OID> = Vec::new();

    // Find all image XObjects
    for (&id, obj) in doc.objects.iter() {
        if let Object::Stream(ref stream) = obj {
            if let Some(subtype) = stream.dict.get(b"Subtype").ok() {
                if let Object::Name(name) = subtype {
                    if name == b"Image" {
                        images_to_update.push(id);
                    }
                }
            }
        }
    }

    // Process each image
    for img_id in images_to_update {
        if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&img_id) {
            // Get image dimensions
            let width = stream
                .dict
                .get(b"Width")
                .ok()
                .and_then(|o| match o {
                    Object::Integer(v) => Some(*v as u32),
                    _ => None,
                })
                .unwrap_or(100);

            let height = stream
                .dict
                .get(b"Height")
                .ok()
                .and_then(|o| match o {
                    Object::Integer(v) => Some(*v as u32),
                    _ => None,
                })
                .unwrap_or(100);

            // Calculate actual DPI by looking up the image placement size from page content streams.
            // We need the XObject resource name for this image to find placement dims.
            // Build name->OID mapping from page resources for DPI calculation.
            let page_ids_for_dpi = get_page_ids(&doc);
            let mut image_dpi = 72.0f32; // Conservative fallback: assume 72 DPI if not placed
            'dpi_outer: for &pid in &page_ids_for_dpi {
                let resources = resolve_page_resources(&doc, pid);
                if let Ok(Object::Dictionary(xobj_dict)) = resources.get(b"XObject") {
                    for (res_name, val) in xobj_dict.iter() {
                        if let Ok(oid) = val.as_reference() {
                            if oid == img_id {
                                // Found the resource name; now scan content streams for Do + cm
                                let content_ids = resolve_page_content_stream_ids(&doc, pid);
                                for cid in content_ids {
                                    if let Some(Object::Stream(cs)) = doc.objects.get(&cid) {
                                        let cbytes = cs.decompressed_content().unwrap_or_else(|_| cs.content.clone());
                                        if let Ok(c_content) = lopdf::content::Content::decode(&cbytes) {
                                            let mut cm = (1.0f32, 0.0f32, 0.0f32, 1.0f32);
                                            for op in &c_content.operations {
                                                if op.operator == "cm" && op.operands.len() >= 4 {
                                                    if let (Some(a), Some(b), Some(c), Some(d)) = (
                                                        op.operands[0].as_float().ok(),
                                                        op.operands[1].as_float().ok(),
                                                        op.operands[2].as_float().ok(),
                                                        op.operands[3].as_float().ok(),
                                                    ) { cm = (a, b, c, d); }
                                                } else if op.operator == "Do" {
                                                    if let Some(do_name) = op.operands.first().and_then(|o| o.as_name().ok()) {
                                                        if do_name == res_name.as_slice() {
                                                            let placed_w_pt = cm.0.hypot(cm.1).abs().max(1.0);
                                                            let placed_w_inch = placed_w_pt / 72.0;
                                                            if placed_w_inch > 0.0 {
                                                                image_dpi = width as f32 / placed_w_inch;
                                                            }
                                                            break 'dpi_outer;
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
                }
            }

            // Calculate new dimensions based on actual image DPI -> target DPI
            // Only downsample if actual DPI exceeds the target
            if image_dpi <= target_dpi as f32 {
                continue; // Already at or below target DPI, skip
            }
            let scale = target_dpi as f32 / image_dpi;
            let new_width = ((width as f32 * scale) as u32).max(1);
            let new_height = ((height as f32 * scale) as u32).max(1);

            // Try to decode and re-encode with lower quality
            if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&img_id) {
                if let Ok(decoded) = image::load_from_memory(&stream.content) {
                    let resized = decoded.resize(
                        new_width,
                        new_height,
                        image::imageops::FilterType::Lanczos3,
                    );

                    // Encode as JPEG with specified quality
                    let mut jpg_buf = std::io::Cursor::new(Vec::new());
                    let encoder =
                        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpg_buf, quality);
                    if resized.write_with_encoder(encoder).is_ok() {
                        let new_data = jpg_buf.into_inner();
                        stream.content = new_data;
                        stream.dict.set("Width", Object::Integer(new_width as i64));
                        stream.dict.set("Height", Object::Integer(new_height as i64));
                        stream.dict.set("Filter", Object::Name("DCTDecode".into()));
                        stream.dict.remove(b"BitsPerComponent");
                    }
                }
            }
        }
    }

    save_doc(&mut doc)
}

pub fn remove_metadata(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    // Remove document info
    let info_id = doc.trailer.get(b"Info").and_then(|o| o.as_reference()).ok();

    if let Some(id) = info_id {
        doc.objects.remove(&id);
        doc.trailer.remove(b"Info");
    }

    // Remove XMP metadata
    let root_id = doc.trailer.get(b"Root").and_then(|o| o.as_reference()).ok();

    if let Some(id) = root_id {
        if let Some(root) = doc.objects.get_mut(&id) {
            if let Ok(dict) = root.as_dict_mut() {
                dict.remove(b"Metadata");
            }
        }
    }

    // Remove any embedded files
    for (_, obj) in doc.objects.iter_mut() {
        if let Object::Dictionary(ref mut dict) = obj {
            dict.remove(b"Names");
            dict.remove(b"EmbeddedFiles");
        }
    }

    save_doc(&mut doc)
}

pub fn flatten_content(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let page_ids = get_page_ids(&doc).clone();

    for &page_id in &page_ids {
        // Merge all content streams into one
        let mut all_operations = Vec::new();

        let content_ids = resolve_page_content_stream_ids(&doc, page_id);
        for contents_id in content_ids {
            if let Some(Object::Stream(stream)) = doc.objects.get(&contents_id) {
                let bytes = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                if let Ok(content) = lopdf::content::Content::decode(&bytes) {
                    all_operations.extend(content.operations);
                }
            }
        }

        if !all_operations.is_empty() {
            let content = lopdf::content::Content {
                operations: all_operations,
            };
            let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

            let mut stream = Stream::new(Dictionary::new(), content_bytes);
            stream.dict.set("Type", Object::Name("Content".into()));
            let content_id = doc.add_object(stream);

            if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
                dict.set("Contents", Object::Reference(content_id));
            }
        }
    }

    save_doc(&mut doc)
}

// ===== TRANSPARENCY FLATTENING & SANITIZATION =====

/// PDF/X-1a および PostScript RIP 向けにドキュメント内のライブ透明度（Live Transparency）を無力化・除去します。
/// ※ 重要: 本機能は ExtGState（アルファ値 CA/ca、ブレンドモード BM、ソフトマスク SMask）および
///    ページレベルの透明度グループ（/Group << /S /Transparency >>）を無力化・除去し、
///    PostScript Level 3 / PDF 1.3 互換の非透明仕様へ適合させるサニタイズ処理を行います。
///    幾何学的ベクターパス交差計算による色ブレンド（Adobe InDesign等の高負荷平面分割）は行いません。
pub fn flatten_transparency(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let page_ids = get_page_ids(&doc).clone();

    // 1. Traverse and neutralize live transparency in ExtGState objects
    // In PDF (and specifically PDF/X-1a ISO 15930-1), live transparency is introduced via:
    // - ExtGState with CA (stroke alpha) < 1.0 or ca (fill alpha) < 1.0
    // - Blend modes other than /Normal or /Compatible (e.g. /Multiply, /Screen, /Overlay)
    // - Soft masks (/SMask)
    // Flattening requires making these opaque (/CA 1.0, /ca 1.0), resetting blend modes to /Normal,
    // and removing soft masks so output devices & PostScript RIPs do not choke.
    let mut extgstate_ids = Vec::new();
    for (&id, obj) in doc.objects.iter() {
        if let Object::Dictionary(ref dict) = obj {
            if let Ok(Object::Name(ref subtype)) = dict.get(b"Type") {
                if subtype == b"ExtGState" {
                    extgstate_ids.push(id);
                    continue;
                }
            }
            // Check if dict looks like an ExtGState (contains CA, ca, BM, or SMask)
            if dict.has(b"CA") || dict.has(b"ca") || dict.has(b"BM") || dict.has(b"SMask") {
                extgstate_ids.push(id);
            }
        }
    }

    for id in extgstate_ids {
        if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&id) {
            dict.set("CA", Object::Real(1.0));
            dict.set("ca", Object::Real(1.0));
            dict.set("BM", Object::Name(b"Normal".to_vec()));
            dict.remove(b"SMask");
        }
    }

    // 2. Process each page: remove Page-level Transparency Group and sanitize resources
    for &page_id in &page_ids {
        // Remove /Group if it is a Transparency group
        let group_opt = if let Some(Object::Dictionary(ref pdict)) = doc.objects.get(&page_id) {
            pdict.get(b"Group").ok().cloned()
        } else {
            None
        };

        let mut remove_group = false;
        if let Some(group_obj) = group_opt {
            match group_obj {
                Object::Dictionary(ref gdict) => {
                    if let Ok(Object::Name(ref s)) = gdict.get(b"S") {
                        if s == b"Transparency" {
                            remove_group = true;
                        }
                    }
                }
                Object::Reference(gid) => {
                    if let Some(Object::Dictionary(ref gdict)) = doc.objects.get(&gid) {
                        if let Ok(Object::Name(ref s)) = gdict.get(b"S") {
                            if s == b"Transparency" {
                                remove_group = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if remove_group {
            if let Some(Object::Dictionary(ref mut pdict)) = doc.objects.get_mut(&page_id) {
                pdict.remove(b"Group");
            }
        }

        // Also check inline ExtGState dictionaries inside page Resources
        let resources_opt = if let Some(Object::Dictionary(ref pdict)) = doc.objects.get(&page_id) {
            pdict.get(b"Resources").ok().cloned()
        } else {
            None
        };

        if let Some(res_obj) = resources_opt {
            let mut target_res_id = None;
            let mut inline_res = match res_obj {
                Object::Reference(rid) => {
                    target_res_id = Some(rid);
                    doc.objects.get(&rid).and_then(|o| o.as_dict().ok()).cloned()
                }
                Object::Dictionary(d) => Some(d),
                _ => None,
            };

            if let Some(ref mut rdict) = inline_res {
                if let Ok(egs_obj) = rdict.get(b"ExtGState") {
                    let mut egs_target_id = None;
                    let mut inline_egs = match egs_obj {
                        Object::Reference(eid) => {
                            egs_target_id = Some(*eid);
                            doc.objects.get(eid).and_then(|o| o.as_dict().ok()).cloned()
                        }
                        Object::Dictionary(d) => Some(d.clone()),
                        _ => None,
                    };

                    if let Some(ref mut states) = inline_egs {
                        for (_, state_obj) in states.iter_mut() {
                            match state_obj {
                                Object::Dictionary(ref mut sd) => {
                                    sd.set("CA", Object::Real(1.0));
                                    sd.set("ca", Object::Real(1.0));
                                    sd.set("BM", Object::Name(b"Normal".to_vec()));
                                    sd.remove(b"SMask");
                                }
                                Object::Reference(s_id) => {
                                    if let Some(Object::Dictionary(ref mut sd)) = doc.objects.get_mut(s_id) {
                                        sd.set("CA", Object::Real(1.0));
                                        sd.set("ca", Object::Real(1.0));
                                        sd.set("BM", Object::Name(b"Normal".to_vec()));
                                        sd.remove(b"SMask");
                                    }
                                }
                                _ => {}
                            }
                        }

                        if let Some(eid) = egs_target_id {
                            doc.objects.insert(eid, Object::Dictionary(states.clone()));
                        } else {
                            rdict.set("ExtGState", Object::Dictionary(states.clone()));
                        }
                    }
                }

                if let Some(rid) = target_res_id {
                    doc.objects.insert(rid, Object::Dictionary(rdict.clone()));
                } else if let Some(Object::Dictionary(ref mut pdict)) = doc.objects.get_mut(&page_id) {
                    pdict.set("Resources", Object::Dictionary(rdict.clone()));
                }
            }
        }
    }

    save_doc(&mut doc)
}

// ===== PDF/X (ISO 15930 Print Production Standard) =====
pub use super::pdf_x::*;

// ===== COLOR SEPARATION PREVIEW & TOTAL AREA COVERAGE (TAC) =====

pub fn preview_color_separations(data: &[u8]) -> Result<serde_json::Value, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let page_ids = get_page_ids(&doc);
    let mut separations = Vec::new();

    // Analyze color usage in each page
    for (page_idx, &page_id) in page_ids.iter().enumerate() {
        let mut uses_rgb = false;
        let mut uses_cmyk = false;
        let mut uses_gray = false;

        // 1. Inspect Content Stream operations across all content streams
        let content_ids = resolve_page_content_stream_ids(&doc, page_id);
        for content_id in content_ids {
            if let Some(Object::Stream(stream)) = doc.objects.get(&content_id) {
                let bytes = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                if let Ok(content) = lopdf::content::Content::decode(&bytes) {
                    for op in &content.operations {
                        match op.operator.as_str() {
                            "rg" | "RG" => uses_rgb = true,
                            "k" | "K" => uses_cmyk = true,
                            "g" | "G" => uses_gray = true,
                            "cs" | "CS" => {
                                if let Some(name) = op.operands.first().and_then(|o| o.as_name().ok()) {
                                    if name == b"DeviceRGB" { uses_rgb = true; }
                                    else if name == b"DeviceCMYK" { uses_cmyk = true; }
                                    else if name == b"DeviceGray" { uses_gray = true; }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
            // 2. Inspect embedded XObject Images in Page Resources
            let res_dict = dict.get(b"Resources").ok().and_then(|r| match r {
                Object::Dictionary(d) => Some(d.clone()),
                Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()).cloned(),
                _ => None,
            });

            if let Some(res) = res_dict {
                if let Ok(xobjs) = res.get(b"XObject") {
                    let xobj_dict = match xobjs {
                        Object::Dictionary(d) => Some(d.clone()),
                        Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()).cloned(),
                        _ => None,
                    };

                    if let Some(xd) = xobj_dict {
                        for (_, val) in xd.iter() {
                            if let Ok(xid) = val.as_reference() {
                                if let Some(Object::Stream(stream)) = doc.objects.get(&xid) {
                                    if let Ok(Object::Name(ref subtype)) = stream.dict.get(b"Subtype") {
                                        if subtype == b"Image" {
                                            if let Ok(Object::Name(ref cs)) = stream.dict.get(b"ColorSpace") {
                                                if cs == b"DeviceRGB" {
                                                    uses_rgb = true;
                                                } else if cs == b"DeviceCMYK" {
                                                    uses_cmyk = true;
                                                } else if cs == b"DeviceGray" {
                                                    uses_gray = true;
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
        }
        separations.push(serde_json::json!({
            "page": page_idx + 1,
            "rgb": uses_rgb,
            "cmyk": uses_cmyk,
            "gray": uses_gray,
        }));
    }

    // Determine if conversion is needed
    let needs_cmyk_conversion = separations.iter().any(|s| s["rgb"] == true);

    Ok(serde_json::json!({
        "separations": separations,
        "needs_cmyk_conversion": needs_cmyk_conversion,
        "max_tac_limit": 300,
        "recommendation": if needs_cmyk_conversion {
            "CMYK conversion recommended for print production (RGB elements detected)"
        } else {
            "Color separations verified (CMYK / Monochrome only)"
        },
    }))
}

/// Render a specific color separation plate (Cyan, Magenta, Yellow, Key/Black, or Total Area Coverage highlight)
pub fn render_color_separation(
    data: &[u8],
    page_index: usize,
    dpi: u32,
    show_c: bool,
    show_m: bool,
    show_y: bool,
    show_k: bool,
    highlight_tac: bool,
    tac_limit: u32,
) -> Result<Vec<u8>, String> {
    // 1. Render base page image using pdftoppm
    let base_png = crate::pdf_engine::inspect::render_page_to_png(data, page_index, dpi)?;
    let img = image::load_from_memory(&base_png)
        .map_err(|e| format!("Failed to decode rendered page: {e}"))?;
    let mut rgba = img.to_rgba8();

    let limit = if tac_limit == 0 { 300 } else { tac_limit };

    // 2. Process each pixel into CMYK separation or TAC highlight
    for pixel in rgba.pixels_mut() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;
        let alpha = pixel[3];

        // Standard RGB to CMYK formula
        let k = 1.0 - r.max(g).max(b);
        let (c, m, y) = if k >= 0.9999 {
            (0.0f32, 0.0f32, 0.0f32)
        } else {
            let inv_k = 1.0 - k;
            (
                (1.0 - r - k) / inv_k,
                (1.0 - g - k) / inv_k,
                (1.0 - b - k) / inv_k,
            )
        };

        // Total Area Coverage (TAC) in % (0 - 400%)
        let total_ink_percent = ((c + m + y + k) * 100.0) as u32;

        if highlight_tac && total_ink_percent > limit {
            // Highlight exceeding ink coverage in vivid neon magenta/red
            pixel[0] = 255;
            pixel[1] = 0;
            pixel[2] = 80;
            pixel[3] = alpha;
        } else {
            // Combine enabled separation plates
            let active_c = if show_c { c } else { 0.0 };
            let active_m = if show_m { m } else { 0.0 };
            let active_y = if show_y { y } else { 0.0 };
            let active_k = if show_k { k } else { 0.0 };

            // Reconstruct RGB from active CMYK channels
            let inv_active_k = 1.0 - active_k;
            let out_r = ((1.0 - active_c) * inv_active_k * 255.0).clamp(0.0, 255.0) as u8;
            let out_g = ((1.0 - active_m) * inv_active_k * 255.0).clamp(0.0, 255.0) as u8;
            let out_b = ((1.0 - active_y) * inv_active_k * 255.0).clamp(0.0, 255.0) as u8;

            pixel[0] = out_r;
            pixel[1] = out_g;
            pixel[2] = out_b;
            pixel[3] = alpha;
        }
    }

    let mut out_buf = std::io::Cursor::new(Vec::new());
    rgba.write_to(&mut out_buf, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode separation PNG: {e}"))?;
    Ok(out_buf.into_inner())
}

// ===== PREFLIGHT & PRINT PRODUCTION CHECK (Separated to preflight.rs) =====
pub use super::preflight::*;
