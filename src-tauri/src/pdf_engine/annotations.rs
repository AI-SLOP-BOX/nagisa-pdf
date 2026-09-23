use super::common::*;
use lopdf::{Dictionary, Document, Object, Stream};

// ===== WATERMARK =====

pub fn add_watermark(
    data: &[u8],
    text: &str,
    opacity: f32,
    rotation: f32,
    font_size: f32,
    color: &str,
    all_pages: bool,
    page_indices: &[usize],
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);

    let (r, g, b) = parse_hex_color(color, (0.5, 0.5, 0.5));

    let rad = rotation * std::f32::consts::PI / 180.0;
    let cos_r = rad.cos();
    let sin_r = rad.sin();

    let has_cjk = text.chars().any(|c| !c.is_ascii());

    // Font setup: If text contains Japanese / CJK characters, embed genuine Type0 CJK font
    let (font_id, cjk_font_opt) = if has_cjk {
        let (fid, _) = crate::pdf_engine::font_unicode::embed_and_encode_unicode_text(&mut doc, text)?;
        let f = crate::pdf_engine::font_unicode::load_primary_cjk_font()?;
        (fid, Some(f))
    } else {
        // Standard /Helvetica font
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name(b"Font".to_vec()));
        font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
        font_dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        let fid = doc.add_object(Object::Dictionary(font_dict));
        (fid, None)
    };

    // Create ExtGState for transparency/opacity
    let mut gs_dict = Dictionary::new();
    gs_dict.set("Type", Object::Name(b"ExtGState".to_vec()));
    gs_dict.set("ca", Object::Real(opacity.clamp(0.0, 1.0)));
    gs_dict.set("CA", Object::Real(opacity.clamp(0.0, 1.0)));
    let gs_id = doc.add_object(Object::Dictionary(gs_dict));

    for (i, &page_id) in page_ids.iter().enumerate() {
        if !all_pages && !page_indices.contains(&i) {
            continue;
        }

        let (pw, ph) = get_page_dimensions(&doc, page_id);

        let cx = pw / 2.0;
        let cy = ph / 2.0;

        let text_op = if let Some(ref f) = cjk_font_opt {
            let mut cids = Vec::new();
            for ch in text.chars() {
                let gid = f.get_gid(ch);
                cids.extend_from_slice(&gid.to_be_bytes());
            }
            lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(cids, lopdf::StringFormat::Hexadecimal)],
            )
        } else {
            lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(text.as_bytes().to_vec(), lopdf::StringFormat::Literal)],
            )
        };

        let approx_text_width = text.chars().fold(0.0f32, |acc, c| {
            acc + crate::pdf_engine::reflow::get_char_metric_width(c, font_size)
        });
        let approx_text_height = font_size * 0.7; // Cap height approx

        // Rotate offset around center: (cx, cy) is page center, offset by half-width and half-height
        let hw = approx_text_width / 2.0;
        let hh = approx_text_height / 2.0;
        let tx = cx - (hw * cos_r - hh * sin_r);
        let ty = cy - (hw * sin_r + hh * cos_r);

        let operations = vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new("cs", vec![Object::Name("DeviceRGB".into())]),
            lopdf::content::Operation::new(
                "sc",
                vec![Object::Real(r), Object::Real(g), Object::Real(b)],
            ),
            lopdf::content::Operation::new("gs", vec![Object::Name("GSWatermark".into())]),
            lopdf::content::Operation::new("BT", vec![]),
            lopdf::content::Operation::new(
                "Tf",
                vec![
                    Object::Name("WatermarkFont".into()),
                    Object::Real(font_size),
                ],
            ),
            lopdf::content::Operation::new(
                "Tm",
                vec![
                    Object::Real(cos_r),
                    Object::Real(sin_r),
                    Object::Real(-sin_r),
                    Object::Real(cos_r),
                    Object::Real(tx),
                    Object::Real(ty),
                ],
            ),
            text_op,
            lopdf::content::Operation::new("ET", vec![]),
            lopdf::content::Operation::new("Q", vec![]),
        ];

        let content = lopdf::content::Content { operations };
        let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

        let mut stream = Stream::new(Dictionary::new(), content_bytes);
        stream.dict.set("Type", Object::Name("Content".into()));
        let content_id = doc.add_object(stream);

        // Register font and ExtGState in page Resources
        let mut resources_dict = resolve_page_resources(&doc, page_id);
        let mut fonts_dict = match resources_dict.get(b"Font") {
            Ok(Object::Dictionary(fd)) => fd.clone(),
            Ok(Object::Reference(f_ref)) => doc
                .objects
                .get(f_ref)
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        fonts_dict.set("WatermarkFont", Object::Reference(font_id));
        resources_dict.set("Font", Object::Dictionary(fonts_dict));

        let mut ext_gstates_dict = match resources_dict.get(b"ExtGState") {
            Ok(Object::Dictionary(gd)) => gd.clone(),
            Ok(Object::Reference(g_ref)) => doc
                .objects
                .get(g_ref)
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        ext_gstates_dict.set("GSWatermark", Object::Reference(gs_id));
        resources_dict.set("ExtGState", Object::Dictionary(ext_gstates_dict));

        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.set("Resources", Object::Dictionary(resources_dict));
        }

        append_page_content(&mut doc, page_id, content_id)?;
    }

    save_doc(&mut doc)
}

pub fn remove_watermarks(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);

    // 1. Remove ISO 32000 /Watermark subtype Annotations from all pages
    for &page_id in &page_ids {
        let annots_list: Vec<(u32, u16)> = if let Some(Object::Dictionary(ref p_dict)) = doc.objects.get(&page_id) {
            match p_dict.get(b"Annots") {
                Ok(Object::Reference(r)) => {
                    if let Some(Object::Array(arr)) = doc.objects.get(r) {
                        arr.iter().filter_map(|a| a.as_reference().ok()).collect()
                    } else {
                        Vec::new()
                    }
                }
                Ok(Object::Array(arr)) => {
                    arr.iter().filter_map(|a| a.as_reference().ok()).collect()
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };

        let mut watermark_annot_ids = Vec::new();
        for aid in annots_list {
            if let Some(Object::Dictionary(d)) = doc.objects.get(&aid) {
                if let Ok(Object::Name(subtype)) = d.get(b"Subtype") {
                    if subtype == b"Watermark" {
                        watermark_annot_ids.push(aid);
                    }
                }
            }
        }

        if !watermark_annot_ids.is_empty() {
            let annots_ref = if let Some(Object::Dictionary(ref p_dict)) = doc.objects.get(&page_id) {
                match p_dict.get(b"Annots") {
                    Ok(Object::Reference(r)) => Some(*r),
                    _ => None,
                }
            } else {
                None
            };

            if let Some(indir_id) = annots_ref {
                if let Some(Object::Array(ref mut arr)) = doc.objects.get_mut(&indir_id) {
                    arr.retain(|a| {
                        if let Object::Reference(aid) = a {
                            !watermark_annot_ids.contains(aid)
                        } else {
                            true
                        }
                    });
                }
            } else if let Some(Object::Dictionary(ref mut p_dict)) = doc.objects.get_mut(&page_id) {
                if let Ok(Object::Array(ref mut arr)) = p_dict.get_mut(b"Annots") {
                    arr.retain(|a| {
                        if let Object::Reference(aid) = a {
                            !watermark_annot_ids.contains(aid)
                        } else {
                            true
                        }
                    });
                }
            }

            for aid in watermark_annot_ids {
                doc.objects.remove(&aid);
            }
        }
    }

    // 2. Remove watermark content streams or watermark blocks in content streams
    // Detect watermark streams created by Nagisa (GSWatermark / WatermarkFont) or standard /Artifact <</Subtype /Watermark>>
    for &page_id in &page_ids {
        let stream_ids = resolve_page_content_stream_ids(&doc, page_id);
        let mut surviving_streams = Vec::new();

        for cid in stream_ids {
            let is_watermark_stream = if let Some(Object::Stream(ref stream)) = doc.objects.get(&cid) {
                let bytes = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
                if let Ok(content) = lopdf::content::Content::decode(&bytes) {
                    // Check if entire stream is a watermark injection
                    let uses_watermark_res = content.operations.iter().any(|op| {
                        if op.operator == "gs" {
                            op.operands.iter().any(|arg| matches!(arg, Object::Name(n) if n == b"GSWatermark"))
                        } else if op.operator == "Tf" {
                            op.operands.iter().any(|arg| matches!(arg, Object::Name(n) if n == b"WatermarkFont"))
                        } else {
                            false
                        }
                    });
                    uses_watermark_res
                } else {
                    false
                }
            } else {
                false
            };

            if is_watermark_stream {
                // Delete watermark stream object from document
                doc.objects.remove(&cid);
            } else {
                // Filter out any localized watermark operations or /Artifact <</Subtype /Watermark>> blocks
                if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&cid) {
                    let decomp = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
                    if let Ok(content) = lopdf::content::Content::decode(&decomp) {
                        let mut filtered_ops = Vec::new();
                        let mut in_watermark_block = false;
                        let orig_count = content.operations.len();

                        for op in content.operations {
                            // Check for /Artifact << /Subtype /Watermark >> BDC
                            if op.operator == "BDC" {
                                let is_wm_bdc = op.operands.iter().any(|arg| match arg {
                                    Object::Name(n) => n == b"Watermark",
                                    Object::Dictionary(d) => d.get(b"Subtype").map(|s| s == &Object::Name(b"Watermark".to_vec())).unwrap_or(false),
                                    _ => false,
                                });
                                if is_wm_bdc {
                                    in_watermark_block = true;
                                    continue;
                                }
                            }

                            if in_watermark_block {
                                if op.operator == "EMC" {
                                    in_watermark_block = false;
                                }
                                continue;
                            }

                            // Check for direct watermark graphics state invocation
                            if op.operator == "gs" && op.operands.iter().any(|arg| matches!(arg, Object::Name(n) if n == b"GSWatermark")) {
                                continue;
                            }

                            filtered_ops.push(op);
                        }

                        if filtered_ops.len() != orig_count {
                            let new_content = lopdf::content::Content { operations: filtered_ops };
                            if let Ok(encoded) = new_content.encode() {
                                stream.set_plain_content(encoded);
                            }
                        }
                    }
                }
                surviving_streams.push(cid);
            }
        }

        // Update page /Contents with surviving stream references
        if let Some(Object::Dictionary(ref mut p_dict)) = doc.objects.get_mut(&page_id) {
            if surviving_streams.is_empty() {
                p_dict.remove(b"Contents");
            } else if surviving_streams.len() == 1 {
                p_dict.set("Contents", Object::Reference(surviving_streams[0]));
            } else {
                let arr: Vec<Object> = surviving_streams.into_iter().map(Object::Reference).collect();
                p_dict.set("Contents", Object::Array(arr));
            }
        }
    }

    save_doc(&mut doc)
}

// ===== ANNOTATIONS =====

pub fn add_highlight(
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

    let (r, g, b) = parse_hex_color(color, (1.0, 1.0, 0.0));

    let x1 = x as f32;
    let y1 = y as f32;
    let x2 = (x + width) as f32;
    let y2 = (y + height) as f32;

    let mut annot_dict = Dictionary::new();
    annot_dict.set("Type", Object::Name("Annot".into()));
    annot_dict.set("Subtype", Object::Name("Highlight".into()));
    annot_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(x1),
            Object::Real(y1),
            Object::Real(x2),
            Object::Real(y2),
        ]),
    );
    // PDF 32000-1 12.5.6.10: QuadPoints specifies 8 numbers [x1, y1, x2, y2, x3, y3, x4, y4]
    // representing top-left, top-right, bottom-left, bottom-right of the text selection
    annot_dict.set(
        "QuadPoints",
        Object::Array(vec![
            Object::Real(x1),
            Object::Real(y2),
            Object::Real(x2),
            Object::Real(y2),
            Object::Real(x1),
            Object::Real(y1),
            Object::Real(x2),
            Object::Real(y1),
        ]),
    );
    annot_dict.set(
        "C",
        Object::Array(vec![Object::Real(r), Object::Real(g), Object::Real(b)]),
    );
    annot_dict.set("F", Object::Integer(4));

    let annot_id = doc.add_object(Object::Dictionary(annot_dict));

    let page_id = page_ids[page_index];
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        let mut annots = match dict.get(b"Annots") {
            Ok(Object::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(annot_id));
        dict.set("Annots", Object::Array(annots));
    }

    save_doc(&mut doc)
}

pub fn add_underline(
    data: &[u8],
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    color: &str,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let (r, g, b) = parse_hex_color(color, (1.0, 0.0, 0.0));

    let x1 = x as f32;
    let y1 = y as f32;
    let x2 = (x + width) as f32;
    let y2 = (y + 2.0) as f32;

    let mut annot_dict = Dictionary::new();
    annot_dict.set("Type", Object::Name("Annot".into()));
    annot_dict.set("Subtype", Object::Name("Underline".into()));
    annot_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(x1),
            Object::Real(y1),
            Object::Real(x2),
            Object::Real(y2),
        ]),
    );
    annot_dict.set(
        "QuadPoints",
        Object::Array(vec![
            Object::Real(x1),
            Object::Real(y2),
            Object::Real(x2),
            Object::Real(y2),
            Object::Real(x1),
            Object::Real(y1),
            Object::Real(x2),
            Object::Real(y1),
        ]),
    );
    annot_dict.set(
        "C",
        Object::Array(vec![Object::Real(r), Object::Real(g), Object::Real(b)]),
    );
    annot_dict.set("F", Object::Integer(4));

    let annot_id = doc.add_object(Object::Dictionary(annot_dict));

    let page_id = page_ids[page_index];
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        let mut annots = match dict.get(b"Annots") {
            Ok(Object::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(annot_id));
        dict.set("Annots", Object::Array(annots));
    }

    save_doc(&mut doc)
}

pub fn add_sticky_note(
    data: &[u8],
    page_index: usize,
    x: f64,
    y: f64,
    text: &str,
    color: &str,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let (r, g, b) = parse_hex_color(color, (1.0, 1.0, 0.0));

    let mut annot_dict = Dictionary::new();
    annot_dict.set("Type", Object::Name("Annot".into()));
    annot_dict.set("Subtype", Object::Name("Text".into()));
    annot_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(x as f32),
            Object::Real(y as f32),
            Object::Real((x + 20.0) as f32),
            Object::Real((y + 20.0) as f32),
        ]),
    );
    annot_dict.set(
        "C",
        Object::Array(vec![Object::Real(r), Object::Real(g), Object::Real(b)]),
    );
    annot_dict.set("Contents", Object::String(encode_pdf_text_string(text), lopdf::StringFormat::Literal));
    annot_dict.set("Open", Object::Boolean(true));
    annot_dict.set("Name", Object::Name("Comment".into()));

    // Generate compliant Normal Appearance (/AP /N) Form XObject
    // Creates a clean sticky note icon (folded corner note sheet) so Chrome, Safari, Edge, and PDF.js render it
    let mut ap_dict = Dictionary::new();
    ap_dict.set("Type", Object::Name(b"XObject".to_vec()));
    ap_dict.set("Subtype", Object::Name(b"Form".to_vec()));
    ap_dict.set("BBox", Object::Array(vec![
        Object::Real(0.0),
        Object::Real(0.0),
        Object::Real(20.0),
        Object::Real(20.0),
    ]));

    // Draw folded note pad icon in selected color
    let ap_stream_content = format!(
        "q\n\
         {r:.3} {g:.3} {b:.3} rg\n\
         0.2 0.2 0.2 RG 1 w 1 j 1 J\n\
         1 1 m 1 19 l 14 19 l 19 14 l 19 1 l h B\n\
         14 19 m 14 14 l 19 14 l s\n\
         0.3 0.3 0.3 RG 1 w\n\
         4 13 m 12 13 l s\n\
         4 9 m 15 9 l s\n\
         4 5 m 11 5 l s\n\
         Q\n"
    );
    let ap_stream = Stream::new(ap_dict, ap_stream_content.into_bytes());
    let ap_stream_id = doc.add_object(Object::Stream(ap_stream));

    let mut ap_wrapper = Dictionary::new();
    ap_wrapper.set("N", Object::Reference(ap_stream_id));
    annot_dict.set("AP", Object::Dictionary(ap_wrapper));

    let annot_id = doc.add_object(Object::Dictionary(annot_dict));

    let page_id = page_ids[page_index];
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        let mut annots = match dict.get(b"Annots") {
            Ok(Object::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(annot_id));
        dict.set("Annots", Object::Array(annots));
    }

    save_doc(&mut doc)
}

pub fn add_rectangle(
    data: &[u8],
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    stroke_color: &str,
    fill_color: &str,
    stroke_width: f32,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let (sr, sg, sb) = parse_hex_color(stroke_color, (0.0, 0.0, 0.0));
    let (fr, fg, fb) = parse_hex_color(fill_color, (1.0, 1.0, 1.0));

    let x1 = x as f32;
    let y1 = y as f32;
    let w = width as f32;
    let h = height as f32;
    let x2 = x1 + w;
    let y2 = y1 + h;

    // ISO 32000-1 §12.5.6.8: Square and Circle Annotations
    // Construct appearance stream Form XObject (/AP /N) for cross-platform compatibility
    let half_stroke = stroke_width / 2.0;
    let ap_ops = vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new("w", vec![Object::Real(stroke_width)]),
        lopdf::content::Operation::new(
            "RG",
            vec![Object::Real(sr), Object::Real(sg), Object::Real(sb)],
        ),
        lopdf::content::Operation::new(
            "rg",
            vec![Object::Real(fr), Object::Real(fg), Object::Real(fb)],
        ),
        lopdf::content::Operation::new(
            "re",
            vec![
                Object::Real(half_stroke),
                Object::Real(half_stroke),
                Object::Real(w - stroke_width),
                Object::Real(h - stroke_width),
            ],
        ),
        lopdf::content::Operation::new("B", vec![]),
        lopdf::content::Operation::new("Q", vec![]),
    ];
    let ap_content = lopdf::content::Content { operations: ap_ops };
    let ap_bytes = ap_content.encode().map_err(|e| format!("Encode error: {e}"))?;

    let mut ap_dict = Dictionary::new();
    ap_dict.set("Type", Object::Name("XObject".into()));
    ap_dict.set("Subtype", Object::Name("Form".into()));
    ap_dict.set(
        "BBox",
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(w),
            Object::Real(h),
        ]),
    );
    let ap_stream = Stream::new(ap_dict, ap_bytes);
    let ap_id = doc.add_object(Object::Stream(ap_stream));

    let mut ap_wrapper = Dictionary::new();
    ap_wrapper.set("N", Object::Reference(ap_id));

    let mut annot_dict = Dictionary::new();
    annot_dict.set("Type", Object::Name("Annot".into()));
    annot_dict.set("Subtype", Object::Name("Square".into()));
    annot_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(x1),
            Object::Real(y1),
            Object::Real(x2),
            Object::Real(y2),
        ]),
    );
    annot_dict.set(
        "C",
        Object::Array(vec![Object::Real(sr), Object::Real(sg), Object::Real(sb)]),
    );
    annot_dict.set(
        "IC",
        Object::Array(vec![Object::Real(fr), Object::Real(fg), Object::Real(fb)]),
    );
    // Border style
    let mut bs = Dictionary::new();
    bs.set("W", Object::Real(stroke_width));
    annot_dict.set("BS", Object::Dictionary(bs));
    annot_dict.set("AP", Object::Dictionary(ap_wrapper));
    annot_dict.set("F", Object::Integer(4)); // Print

    let annot_id = doc.add_object(Object::Dictionary(annot_dict));

    let page_id = page_ids[page_index];
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        let mut annots = match dict.get(b"Annots") {
            Ok(Object::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(annot_id));
        dict.set("Annots", Object::Array(annots));
    }

    save_doc(&mut doc)
}

pub fn add_circle(
    data: &[u8],
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    stroke_color: &str,
    fill_color: &str,
    stroke_width: f32,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let (sr, sg, sb) = parse_hex_color(stroke_color, (0.0, 0.0, 0.0));
    let (fr, fg, fb) = parse_hex_color(fill_color, (1.0, 1.0, 1.0));

    let x1 = x as f32;
    let y1 = y as f32;
    let w = width as f32;
    let h = height as f32;
    let x2 = x1 + w;
    let y2 = y1 + h;

    // ISO 32000-1 §12.5.6.8: Square and Circle Annotations
    // Construct 4-cubic-Bézier ellipse approximation for appearance stream Form XObject (/AP /N)
    // Standard cubic Bézier constant: kappa = 4 * (sqrt(2) - 1) / 3 ≈ 0.55228475
    let half_stroke = stroke_width / 2.0;
    let rx = (w - stroke_width).max(1.0) / 2.0;
    let ry = (h - stroke_width).max(1.0) / 2.0;
    let cx = half_stroke + rx;
    let cy = half_stroke + ry;
    let kx = rx * 0.55228475;
    let ky = ry * 0.55228475;

    let ap_ops = vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new("w", vec![Object::Real(stroke_width)]),
        lopdf::content::Operation::new(
            "RG",
            vec![Object::Real(sr), Object::Real(sg), Object::Real(sb)],
        ),
        lopdf::content::Operation::new(
            "rg",
            vec![Object::Real(fr), Object::Real(fg), Object::Real(fb)],
        ),
        // Move to right edge: (cx + rx, cy)
        lopdf::content::Operation::new("m", vec![Object::Real(cx + rx), Object::Real(cy)]),
        // Top-right quadrant to (cx, cy + ry)
        lopdf::content::Operation::new(
            "c",
            vec![
                Object::Real(cx + rx),
                Object::Real(cy + ky),
                Object::Real(cx + kx),
                Object::Real(cy + ry),
                Object::Real(cx),
                Object::Real(cy + ry),
            ],
        ),
        // Top-left quadrant to (cx - rx, cy)
        lopdf::content::Operation::new(
            "c",
            vec![
                Object::Real(cx - kx),
                Object::Real(cy + ry),
                Object::Real(cx - rx),
                Object::Real(cy + ky),
                Object::Real(cx - rx),
                Object::Real(cy),
            ],
        ),
        // Bottom-left quadrant to (cx, cy - ry)
        lopdf::content::Operation::new(
            "c",
            vec![
                Object::Real(cx - rx),
                Object::Real(cy - ky),
                Object::Real(cx - kx),
                Object::Real(cy - ry),
                Object::Real(cx),
                Object::Real(cy - ry),
            ],
        ),
        // Bottom-right quadrant to (cx + rx, cy)
        lopdf::content::Operation::new(
            "c",
            vec![
                Object::Real(cx + kx),
                Object::Real(cy - ry),
                Object::Real(cx + rx),
                Object::Real(cy - ky),
                Object::Real(cx + rx),
                Object::Real(cy),
            ],
        ),
        lopdf::content::Operation::new("b", vec![]), // Close, fill, and stroke
        lopdf::content::Operation::new("Q", vec![]),
    ];
    let ap_content = lopdf::content::Content { operations: ap_ops };
    let ap_bytes = ap_content.encode().map_err(|e| format!("Encode error: {e}"))?;

    let mut ap_dict = Dictionary::new();
    ap_dict.set("Type", Object::Name("XObject".into()));
    ap_dict.set("Subtype", Object::Name("Form".into()));
    ap_dict.set(
        "BBox",
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(w),
            Object::Real(h),
        ]),
    );
    let ap_stream = Stream::new(ap_dict, ap_bytes);
    let ap_id = doc.add_object(Object::Stream(ap_stream));

    let mut ap_wrapper = Dictionary::new();
    ap_wrapper.set("N", Object::Reference(ap_id));

    let mut annot_dict = Dictionary::new();
    annot_dict.set("Type", Object::Name("Annot".into()));
    annot_dict.set("Subtype", Object::Name("Circle".into()));
    annot_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(x1),
            Object::Real(y1),
            Object::Real(x2),
            Object::Real(y2),
        ]),
    );
    annot_dict.set(
        "C",
        Object::Array(vec![Object::Real(sr), Object::Real(sg), Object::Real(sb)]),
    );
    annot_dict.set(
        "IC",
        Object::Array(vec![Object::Real(fr), Object::Real(fg), Object::Real(fb)]),
    );
    let mut bs = Dictionary::new();
    bs.set("W", Object::Real(stroke_width));
    annot_dict.set("BS", Object::Dictionary(bs));
    annot_dict.set("AP", Object::Dictionary(ap_wrapper));
    annot_dict.set("F", Object::Integer(4)); // Print

    let annot_id = doc.add_object(Object::Dictionary(annot_dict));

    let page_id = page_ids[page_index];
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        let mut annots = match dict.get(b"Annots") {
            Ok(Object::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(annot_id));
        dict.set("Annots", Object::Array(annots));
    }

    save_doc(&mut doc)
}

pub fn add_line(
    data: &[u8],
    page_index: usize,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    color: &str,
    width: f32,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let (r, g, b) = parse_hex_color(color, (0.0, 0.0, 0.0));

    let px1 = x1 as f32;
    let py1 = y1 as f32;
    let px2 = x2 as f32;
    let py2 = y2 as f32;

    let rect_x1 = px1.min(px2) - width;
    let rect_y1 = py1.min(py2) - width;
    let rect_x2 = px1.max(px2) + width;
    let rect_y2 = py1.max(py2) + width;

    // ISO 32000-1 §12.5.6.7: Line Annotations
    // Construct appearance stream Form XObject (/AP /N)
    let ap_ops = vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new("w", vec![Object::Real(width)]),
        lopdf::content::Operation::new(
            "RG",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        lopdf::content::Operation::new(
            "m",
            vec![Object::Real(px1 - rect_x1), Object::Real(py1 - rect_y1)],
        ),
        lopdf::content::Operation::new(
            "l",
            vec![Object::Real(px2 - rect_x1), Object::Real(py2 - rect_y1)],
        ),
        lopdf::content::Operation::new("S", vec![]),
        lopdf::content::Operation::new("Q", vec![]),
    ];
    let ap_content = lopdf::content::Content { operations: ap_ops };
    let ap_bytes = ap_content.encode().map_err(|e| format!("Encode error: {e}"))?;

    let mut ap_dict = Dictionary::new();
    ap_dict.set("Type", Object::Name("XObject".into()));
    ap_dict.set("Subtype", Object::Name("Form".into()));
    ap_dict.set(
        "BBox",
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(rect_x2 - rect_x1),
            Object::Real(rect_y2 - rect_y1),
        ]),
    );
    let ap_stream = Stream::new(ap_dict, ap_bytes);
    let ap_id = doc.add_object(Object::Stream(ap_stream));

    let mut ap_wrapper = Dictionary::new();
    ap_wrapper.set("N", Object::Reference(ap_id));

    let mut annot_dict = Dictionary::new();
    annot_dict.set("Type", Object::Name("Annot".into()));
    annot_dict.set("Subtype", Object::Name("Line".into()));
    annot_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(rect_x1),
            Object::Real(rect_y1),
            Object::Real(rect_x2),
            Object::Real(rect_y2),
        ]),
    );
    annot_dict.set(
        "L",
        Object::Array(vec![
            Object::Real(px1),
            Object::Real(py1),
            Object::Real(px2),
            Object::Real(py2),
        ]),
    );
    annot_dict.set(
        "C",
        Object::Array(vec![Object::Real(r), Object::Real(g), Object::Real(b)]),
    );
    let mut bs = Dictionary::new();
    bs.set("W", Object::Real(width));
    annot_dict.set("BS", Object::Dictionary(bs));
    annot_dict.set("AP", Object::Dictionary(ap_wrapper));
    annot_dict.set("F", Object::Integer(4)); // Print

    let annot_id = doc.add_object(Object::Dictionary(annot_dict));

    let page_id = page_ids[page_index];
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        let mut annots = match dict.get(b"Annots") {
            Ok(Object::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(annot_id));
        dict.set("Annots", Object::Array(annots));
    }

    save_doc(&mut doc)
}

// ===== REDACTION (Separated to redact.rs) =====
pub use super::redact::*;

// ===== ANNOTATION MANAGEMENT (Separated to annot_manage.rs) =====
pub use super::annot_manage::*;
