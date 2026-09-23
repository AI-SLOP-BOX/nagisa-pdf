use super::common::*;
use super::*;
use lopdf::{Dictionary, Document, Object};

// ===== ADVANCED FORM =====

pub fn add_form_field(
    data: &[u8],
    page_index: usize,
    field_name: &str,
    field_type: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    default_value: &str,
) -> Result<Vec<u8>, String> {
    let config = FormFieldConfig {
        field_type: field_type.to_string(),
        name: field_name.to_string(),
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        value: if default_value.is_empty() {
            None
        } else {
            Some(default_value.to_string())
        },
        options: None,
        required: false,
        read_only: false,
        max_length: None,
    };
    create_form_field(data, page_index, &config)
}

pub fn add_calculated_field(
    data: &[u8],
    page_index: usize,
    field_name: &str,
    formula: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    // Create field with JavaScript calculation
    let mut field_dict = Dictionary::new();
    field_dict.set("Type", Object::Name("Annot".into()));
    field_dict.set("Subtype", Object::Name("Widget".into()));
    field_dict.set("FT", Object::Name(b"Tx".to_vec()));
    field_dict.set(
        "T",
        Object::String(
            encode_pdf_text_string(field_name),
            lopdf::StringFormat::Literal,
        ),
    );
    field_dict.set(
        "V",
        Object::String(b"0".to_vec(), lopdf::StringFormat::Literal),
    );
    field_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(x as f32),
            Object::Real(y as f32),
            Object::Real((x + width) as f32),
            Object::Real((y + height) as f32),
        ]),
    );
    field_dict.set("F", Object::Integer(4)); // Print flag
    field_dict.set(
        "DA",
        Object::String(
            b"/Helv 12 Tf 0 0 0 rg".to_vec(),
            lopdf::StringFormat::Literal,
        ),
    );

    // 1. ISO 32000-1 §12.6.3 compliant /AA (Additional Actions) with /C (Calculate event)
    let js_code = format!("event.value = {};", formula);
    let mut action_dict = Dictionary::new();
    action_dict.set("S", Object::Name("JavaScript".into()));
    action_dict.set(
        "JS",
        Object::String(js_code.as_bytes().to_vec(), lopdf::StringFormat::Literal),
    );
    let action_id = doc.add_object(Object::Dictionary(action_dict));

    let mut aa_dict = Dictionary::new();
    aa_dict.set("C", Object::Reference(action_id)); // /C triggers calculation
    field_dict.set("AA", Object::Dictionary(aa_dict));

    // 2. Visible Normal Appearance (/AP /N)
    let ap_stream = super::form_creator::create_appearance_stream(
        width as f32,
        height as f32,
        "Tx",
        Some("0"),
    );
    let ap_id = doc.add_object(Object::Stream(ap_stream));
    let mut ap_dict = Dictionary::new();
    ap_dict.set("N", Object::Reference(ap_id));
    field_dict.set("AP", Object::Dictionary(ap_dict));

    let field_id = doc.add_object(Object::Dictionary(field_dict));

    // 3. Add to page annotations
    let page_id = page_ids[page_index];
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        let mut annots = match dict.get(b"Annots") {
            Ok(Object::Array(a)) => a.clone(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(field_id));
        dict.set("Annots", Object::Array(annots));
    }

    // 4. Register in AcroForm
    super::form_creator::ensure_acroform_with_resources(&mut doc, field_id)?;

    // 5. Also ensure the page's own /Resources contains /Helv font reference
    // so viewers that look up appearance fonts in Page Resources instead of AcroForm DR will render correctly
    let mut page_resources = resolve_page_resources(&doc, page_id);
    let mut font_dict = match page_resources.get(b"Font") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(f_ref)) => doc.objects.get(f_ref).and_then(|o| o.as_dict().ok()).cloned().unwrap_or_default(),
        _ => Dictionary::new(),
    };
    if !font_dict.has(b"Helv") {
        // Find or create Helvetica font object
        let helv_font_id = doc.objects.iter().find_map(|(id, obj)| {
            if let Object::Dictionary(d) = obj {
                if d.get(b"Type").ok().and_then(|t| t.as_name().ok()) == Some(b"Font")
                    && d.get(b"BaseFont").ok().and_then(|b| b.as_name().ok()) == Some(b"Helvetica")
                {
                    return Some(*id);
                }
            }
            None
        }).unwrap_or_else(|| {
            let mut f_dict = Dictionary::new();
            f_dict.set("Type", Object::Name(b"Font".to_vec()));
            f_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
            f_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
            f_dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
            doc.add_object(Object::Dictionary(f_dict))
        });
        font_dict.set("Helv", Object::Reference(helv_font_id));
        page_resources.set("Font", Object::Dictionary(font_dict));
        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.set("Resources", Object::Dictionary(page_resources));
        }
    }

    save_doc(&mut doc)
}

// ===== XFDF/FDF IMPORT/EXPORT =====

fn xml_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn export_xfdf(data: &[u8]) -> Result<String, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);

    let mut xfdf = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xfdf.push_str("<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n");
    xfdf.push_str("  <annotations>\n");

    for (page_idx, &page_id) in page_ids.iter().enumerate() {
        if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
            if let Ok(Object::Array(annots)) = dict.get(b"Annots") {
                for annot_ref in annots {
                    if let Object::Reference(ref_id) = annot_ref {
                        if let Some(Object::Dictionary(annot_dict)) = doc.objects.get(ref_id) {
                            let annot_type = annot_dict
                                .get(b"Subtype")
                                .ok()
                                .and_then(|o| match o {
                                    Object::Name(bytes) => {
                                        Some(String::from_utf8_lossy(bytes).to_string())
                                    }
                                    _ => None,
                                })
                                .unwrap_or_default();

                            let contents = annot_dict
                                .get(b"Contents")
                                .ok()
                                .and_then(|o| match o {
                                    Object::String(bytes, _) => {
                                        Some(decode_pdf_text_string(bytes))
                                    }
                                    _ => None,
                                })
                                .unwrap_or_default();

                            let author = annot_dict
                                .get(b"T")
                                .ok()
                                .and_then(|o| match o {
                                    Object::String(bytes, _) => {
                                        Some(decode_pdf_text_string(bytes))
                                    }
                                    _ => None,
                                })
                                .unwrap_or_default();

                            let (x, y, w, h) = match annot_dict.get(b"Rect") {
                                Ok(Object::Array(arr)) if arr.len() >= 4 => {
                                    let x = match &arr[0] {
                                        Object::Real(v) => *v,
                                        Object::Integer(v) => *v as f32,
                                        _ => 0.0,
                                    };
                                    let y = match &arr[1] {
                                        Object::Real(v) => *v,
                                        Object::Integer(v) => *v as f32,
                                        _ => 0.0,
                                    };
                                    let w = match &arr[2] {
                                        Object::Real(v) => *v,
                                        Object::Integer(v) => *v as f32,
                                        _ => 0.0,
                                    };
                                    let h = match &arr[3] {
                                        Object::Real(v) => *v,
                                        Object::Integer(v) => *v as f32,
                                        _ => 0.0,
                                    };
                                    (x, y, w - x, h - y)
                                }
                                _ => (0.0, 0.0, 0.0, 0.0),
                            };

                            let color = match annot_dict.get(b"C") {
                                Ok(Object::Array(arr)) if arr.len() >= 3 => {
                                    let r = match &arr[0] {
                                        Object::Real(v) => (v * 255.0) as u8,
                                        _ => 0,
                                    };
                                    let g = match &arr[1] {
                                        Object::Real(v) => (v * 255.0) as u8,
                                        _ => 0,
                                    };
                                    let b = match &arr[2] {
                                        Object::Real(v) => (v * 255.0) as u8,
                                        _ => 0,
                                    };
                                    format!("{},{},{}", r, g, b)
                                }
                                _ => "255,0,0".to_string(),
                            };

                            let escaped_author = xml_escape(&author);
                            let escaped_contents = xml_escape(&contents);

                            // Check /IRT (In-Reply-To) for hierarchical replies
                            let irt_attr = match annot_dict.get(b"IRT") {
                                Ok(Object::Reference(parent_id)) => {
                                    format!(" inreplyto=\"annot_{}\"", parent_id.0)
                                }
                                _ => String::new(),
                            };

                            xfdf.push_str(&format!(
                                "    <{} page=\"{}\" name=\"{}\" title=\"{}\" color=\"{}\" left=\"{}\" top=\"{}\" width=\"{}\" height=\"{}\"{}>\n",
                                annot_type, page_idx + 1, format!("annot_{}", ref_id.0), escaped_author, color, x, y, w, h, irt_attr
                            ));
                            if !escaped_contents.is_empty() {
                                xfdf.push_str(&format!(
                                    "      <contents>{}</contents>\n",
                                    escaped_contents
                                ));
                            }
                            xfdf.push_str(&format!("    </{}>\n", annot_type));
                        }
                    }
                }
            }
        }
    }

    xfdf.push_str("  </annotations>\n");
    xfdf.push_str("</xfdf>\n");

    Ok(xfdf)
}

pub fn import_xfdf(data: &[u8], xfdf_content: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);

    // Standard-compliant XML streaming parser using quick-xml
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;
    use std::collections::HashMap;

    let mut reader = Reader::from_str(xfdf_content);

    // Track parsed annotation data
    struct PendingAnnot {
        annot_type: String,
        page: usize,
        name: String,
        author: String,
        rect: (f32, f32, f32, f32),
        color: (f32, f32, f32),
        contents: String,
        in_reply_to: Option<String>,
    }

    fn parse_xfdf_color(color_str: &str) -> (f32, f32, f32) {
        let s = color_str.trim();
        if s.is_empty() {
            return (1.0, 0.0, 0.0); // Default red
        }
        if s.starts_with('#') {
            let hex = s.trim_start_matches('#');
            if hex.len() >= 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255) as f32 / 255.0;
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
                return (r, g, b);
            }
        }
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() >= 3 {
            let r = parts[0].trim().parse::<f32>().unwrap_or(255.0);
            let g = parts[1].trim().parse::<f32>().unwrap_or(0.0);
            let b = parts[2].trim().parse::<f32>().unwrap_or(0.0);
            // If values are in [0..255] range, normalize
            let max_val = r.max(g).max(b);
            if max_val > 1.0 {
                (r / 255.0, g / 255.0, b / 255.0)
            } else {
                (r, g, b)
            }
        } else {
            (1.0, 0.0, 0.0)
        }
    }

    let mut pending_annots: Vec<PendingAnnot> = Vec::new();
    let mut current_annot: Option<PendingAnnot> = None;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = e.name().into_inner();
                let tag_lower = tag_name.to_lowercase();

                if tag_lower == "contents" {
                    if let Some(ref mut annot) = current_annot {
                        match reader.read_text(e.to_end().name()) {
                            Ok(text) => {
                                let unescaped = quick_xml::escape::unescape(&text)
                                    .map(|c| c.into_owned())
                                    .unwrap_or_else(|_| text.to_string());
                                // Also ensure &apos; and &quot; are handled if unescape was minimal
                                let resolved = unescaped
                                    .replace("&quot;", "\"")
                                    .replace("&apos;", "'");
                                annot.contents = resolved;
                            }
                            Err(_) => {}
                        }
                    }
                } else if tag_lower == "highlight"
                    || tag_lower == "text"
                    || tag_lower == "underline"
                    || tag_lower == "strikeout"
                    || tag_lower == "freetext"
                    || tag_lower == "square"
                    || tag_lower == "circle"
                    || tag_lower == "line"
                    || tag_lower == "ink"
                {
                    // Map common XFDF tags to PDF Subtypes
                    let subtype = match tag_lower.as_str() {
                        "highlight" => "Highlight",
                        "text" => "Text",
                        "underline" => "Underline",
                        "strikeout" => "StrikeOut",
                        "freetext" => "FreeText",
                        "square" => "Square",
                        "circle" => "Circle",
                        "line" => "Line",
                        "ink" => "Ink",
                        _ => "Text",
                    };

                    let mut page = 0usize;
                    let mut name = String::new();
                    let mut author = String::new();
                    let mut in_reply_to = None;
                    let mut left = 0.0f32;
                    let mut top = 0.0f32;
                    let mut width = 100.0f32;
                    let mut height = 100.0f32;
                    let mut color_str = String::new();

                    for attr in e.attributes().flatten() {
                        let key = attr.key.into_inner().to_lowercase();
                        let val = attr.value.as_ref();
                        match key.as_str() {
                            "page" => {
                                if let Ok(p) = val.parse::<usize>() {
                                    page = p.saturating_sub(1);
                                }
                            }
                            "name" => name = val.to_string(),
                            "title" => author = val.to_string(),
                            "inreplyto" => in_reply_to = Some(val.to_string()),
                            "color" => color_str = val.to_string(),
                            "left" => {
                                if let Ok(v) = val.parse::<f32>() {
                                    left = v;
                                }
                            }
                            "top" => {
                                if let Ok(v) = val.parse::<f32>() {
                                    top = v;
                                }
                            }
                            "width" => {
                                if let Ok(v) = val.parse::<f32>() {
                                    width = v;
                                }
                            }
                            "height" => {
                                if let Ok(v) = val.parse::<f32>() {
                                    height = v;
                                }
                            }
                            _ => {}
                        }
                    }

                    current_annot = Some(PendingAnnot {
                        annot_type: subtype.to_string(),
                        page,
                        name,
                        author,
                        rect: (left, top, left + width, top + height),
                        color: parse_xfdf_color(&color_str),
                        contents: String::new(),
                        in_reply_to,
                    });
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag_name = e.name().into_inner();
                let tag_lower = tag_name.to_lowercase();

                if tag_lower == "highlight"
                    || tag_lower == "text"
                    || tag_lower == "underline"
                    || tag_lower == "strikeout"
                    || tag_lower == "freetext"
                    || tag_lower == "square"
                    || tag_lower == "circle"
                    || tag_lower == "line"
                    || tag_lower == "ink"
                {
                    let subtype = match tag_lower.as_str() {
                        "highlight" => "Highlight",
                        "text" => "Text",
                        "underline" => "Underline",
                        "strikeout" => "StrikeOut",
                        "freetext" => "FreeText",
                        "square" => "Square",
                        "circle" => "Circle",
                        "line" => "Line",
                        "ink" => "Ink",
                        _ => "Text",
                    };

                    let mut page = 0usize;
                    let mut name = String::new();
                    let mut author = String::new();
                    let mut in_reply_to = None;
                    let mut left = 0.0f32;
                    let mut top = 0.0f32;
                    let mut width = 100.0f32;
                    let mut height = 100.0f32;
                    let mut color_str = String::new();

                    for attr in e.attributes().flatten() {
                        let key = attr.key.into_inner().to_lowercase();
                        let val = attr.value.as_ref();
                        match key.as_str() {
                            "page" => {
                                if let Ok(p) = val.parse::<usize>() {
                                    page = p.saturating_sub(1);
                                }
                            }
                            "name" => name = val.to_string(),
                            "title" => author = val.to_string(),
                            "inreplyto" => in_reply_to = Some(val.to_string()),
                            "color" => color_str = val.to_string(),
                            "left" => {
                                if let Ok(v) = val.parse::<f32>() {
                                    left = v;
                                }
                            }
                            "top" => {
                                if let Ok(v) = val.parse::<f32>() {
                                    top = v;
                                }
                            }
                            "width" => {
                                if let Ok(v) = val.parse::<f32>() {
                                    width = v;
                                }
                            }
                            "height" => {
                                if let Ok(v) = val.parse::<f32>() {
                                    height = v;
                                }
                            }
                            _ => {}
                        }
                    }

                    pending_annots.push(PendingAnnot {
                        annot_type: subtype.to_string(),
                        page,
                        name,
                        author,
                        rect: (left, top, left + width, top + height),
                        color: parse_xfdf_color(&color_str),
                        contents: String::new(),
                        in_reply_to,
                    });
                }
            }
            Ok(Event::End(ref _e)) => {
                if let Some(annot) = current_annot.take() {
                    pending_annots.push(annot);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XFDF XML parsing error at position {}: {:?}", reader.buffer_position(), e)),
            _ => {}
        }
        buf.clear();
    }

    // Map from annotation name (e.g. "annot_12") to created PDF ObjectId
    let mut name_to_id: HashMap<String, (u32, u16)> = HashMap::new();
    let mut created_annots: Vec<((u32, u16), Option<String>)> = Vec::new();

    for pa in pending_annots {
        if pa.page >= page_ids.len() {
            continue;
        }

        let mut annot_dict = Dictionary::new();
        annot_dict.set("Type", Object::Name("Annot".into()));
        annot_dict.set("Subtype", Object::Name(pa.annot_type.as_bytes().to_vec()));
        annot_dict.set(
            "Rect",
            Object::Array(vec![
                Object::Real(pa.rect.0),
                Object::Real(pa.rect.1),
                Object::Real(pa.rect.2),
                Object::Real(pa.rect.3),
            ]),
        );
        annot_dict.set(
            "C",
            Object::Array(vec![
                Object::Real(pa.color.0),
                Object::Real(pa.color.1),
                Object::Real(pa.color.2),
            ]),
        );
        if !pa.contents.is_empty() {
            annot_dict.set(
                "Contents",
                Object::String(
                    encode_pdf_text_string(&pa.contents),
                    lopdf::StringFormat::Literal,
                ),
            );
        }
        if !pa.author.is_empty() {
            annot_dict.set(
                "T",
                Object::String(
                    encode_pdf_text_string(&pa.author),
                    lopdf::StringFormat::Literal,
                ),
            );
        }

        // Generate /AP /N Form XObject Appearance Stream for standard PDF viewers (ISO 32000-1 §12.5.5)
        let (x1, y1, x2, y2) = pa.rect;
        let w = (x2 - x1).abs().max(1.0);
        let h = (y2 - y1).abs().max(1.0);
        let stroke_w = 2.0f32;
        let half_s = stroke_w / 2.0;

        let ap_stream_opt = match pa.annot_type.as_str() {
            "Square" => {
                let ops = vec![
                    lopdf::content::Operation::new("q", vec![]),
                    lopdf::content::Operation::new("w", vec![Object::Real(stroke_w)]),
                    lopdf::content::Operation::new(
                        "RG",
                        vec![
                            Object::Real(pa.color.0),
                            Object::Real(pa.color.1),
                            Object::Real(pa.color.2),
                        ],
                    ),
                    lopdf::content::Operation::new(
                        "re",
                        vec![
                            Object::Real(half_s),
                            Object::Real(half_s),
                            Object::Real((w - stroke_w).max(0.1)),
                            Object::Real((h - stroke_w).max(0.1)),
                        ],
                    ),
                    lopdf::content::Operation::new("S", vec![]),
                    lopdf::content::Operation::new("Q", vec![]),
                ];
                let content = lopdf::content::Content { operations: ops };
                content.encode().ok().map(|bytes| {
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
                    lopdf::Stream::new(ap_dict, bytes)
                })
            }
            "Circle" => {
                let rx = ((w - stroke_w) / 2.0).max(0.1);
                let ry = ((h - stroke_w) / 2.0).max(0.1);
                let cx = half_s + rx;
                let cy = half_s + ry;
                let kx = rx * 0.55228475;
                let ky = ry * 0.55228475;
                let ops = vec![
                    lopdf::content::Operation::new("q", vec![]),
                    lopdf::content::Operation::new("w", vec![Object::Real(stroke_w)]),
                    lopdf::content::Operation::new(
                        "RG",
                        vec![
                            Object::Real(pa.color.0),
                            Object::Real(pa.color.1),
                            Object::Real(pa.color.2),
                        ],
                    ),
                    lopdf::content::Operation::new("m", vec![Object::Real(cx + rx), Object::Real(cy)]),
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
                    lopdf::content::Operation::new("S", vec![]),
                    lopdf::content::Operation::new("Q", vec![]),
                ];
                let content = lopdf::content::Content { operations: ops };
                content.encode().ok().map(|bytes| {
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
                    lopdf::Stream::new(ap_dict, bytes)
                })
            }
            _ => None,
        };

        if let Some(stream) = ap_stream_opt {
            let ap_id = doc.add_object(Object::Stream(stream));
            let mut ap_wrapper = Dictionary::new();
            ap_wrapper.set("N", Object::Reference(ap_id));
            annot_dict.set("AP", Object::Dictionary(ap_wrapper));
        }

        let annot_id = doc.add_object(Object::Dictionary(annot_dict));
        if !pa.name.is_empty() {
            name_to_id.insert(pa.name.clone(), annot_id);
        }
        created_annots.push((annot_id, pa.in_reply_to));

        // Add annotation reference to page Annots (handling both direct and indirect Annots)
        let page_id = page_ids[pa.page];
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
                arr.push(Object::Reference(annot_id));
            }
        } else if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
            let mut annots = match dict.get(b"Annots") {
                Ok(Object::Array(a)) => a.clone(),
                _ => Vec::new(),
            };
            annots.push(Object::Reference(annot_id));
            dict.set("Annots", Object::Array(annots));
        }
    }

    // Resolve /IRT (In-Reply-To) references across all created annotations
    for (annot_id, in_reply_to) in created_annots {
        if let Some(ref parent_name) = in_reply_to {
            if let Some(&parent_id) = name_to_id.get(parent_name) {
                if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&annot_id) {
                    dict.set("IRT", Object::Reference(parent_id));
                }
            }
        }
    }

    save_doc(&mut doc)
}

// ===== FORM DATA AGGREGATION =====

pub fn aggregate_form_data(pdf_paths: &[String]) -> Result<serde_json::Value, String> {
    let mut all_data = Vec::new();

    for path in pdf_paths {
        let data = std::fs::read(path).map_err(|e| format!("Failed to read {path}: {e}"))?;
        let fields = get_form_fields(&data)?;

        let mut file_data = serde_json::Map::new();
        file_data.insert("file".into(), serde_json::Value::String(path.clone()));
        file_data.insert("fields".into(), serde_json::Value::Array(fields));

        all_data.push(serde_json::Value::Object(file_data));
    }

    // Create summary
    let mut summary = serde_json::Map::new();
    summary.insert(
        "total_files".into(),
        serde_json::Value::Number(all_data.len().into()),
    );
    summary.insert("files".into(), serde_json::Value::Array(all_data));

    Ok(serde_json::Value::Object(summary))
}

// ===== INTERACTIVE FORM CREATION (Separated to form_creator.rs) =====
pub use super::form_creator::*;
