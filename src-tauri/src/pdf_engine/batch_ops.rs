use super::common::*;
use super::*;
use lopdf::{Dictionary, Document, Object, Stream};

// ===== BATCH PROCESSING =====

pub fn batch_merge_pdfs(paths: &[String], output_path: &str) -> Result<(), String> {
    let merged = merge_pdfs(paths)?;
    std::fs::write(output_path, merged).map_err(|e| format!("Failed to write output: {e}"))
}

pub fn batch_add_watermark(
    paths: &[String],
    text: &str,
    opacity: f32,
    rotation: f32,
    font_size: f32,
    color: &str,
) -> Result<Vec<Vec<u8>>, String> {
    let mut results = Vec::new();
    for path in paths {
        let data = std::fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path))?;
        let watermarked =
            add_watermark(&data, text, opacity, rotation, font_size, color, true, &[])?;
        results.push(watermarked);
    }
    Ok(results)
}

pub fn batch_protect(_paths: &[String], _password: &str) -> Result<Vec<Vec<u8>>, String> {
    // Honest: Standard Security Handler (AES-128/256) is under implementation.
    // Early return honest error to prevent wasted I/O and prevent generation of corrupt PDFs.
    Err("PDF暗号化（AES-128/256 Standard Security Handler）によるストリーム暗号化は現在実装準備中です。破損した暗号化PDFの出力を防止するためバッチ処理を安全に中断しました。".into())
}

pub fn batch_optimize(paths: &[String]) -> Result<Vec<Vec<u8>>, String> {
    let mut results = Vec::new();
    for path in paths {
        let data = std::fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path))?;
        let optimized = optimize_pdf(&data)?;
        results.push(optimized);
    }
    Ok(results)
}

// ===== PDF/A COMPLIANCE =====

pub fn convert_to_pdfa(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    // PDF/A-1 requirement: All fonts MUST be embedded
    let mut uncompressed_non_embedded = Vec::new();
    for (_, obj) in &doc.objects {
        if let Object::Dictionary(dict) = obj {
            if let Ok(Object::Name(font_type)) = dict.get(b"Type") {
                if font_type == b"Font" {
                    let font_name = dict
                        .get(b"BaseFont")
                        .ok()
                        .and_then(|o| match o {
                            Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
                            _ => None,
                        })
                        .unwrap_or_else(|| "Unknown".into());

                    let has_font_file = if let Ok(desc_ref) =
                        dict.get(b"FontDescriptor").and_then(|o| o.as_reference())
                    {
                        if let Some(Object::Dictionary(desc)) = doc.objects.get(&desc_ref) {
                            desc.get(b"FontFile").is_ok()
                                || desc.get(b"FontFile2").is_ok()
                                || desc.get(b"FontFile3").is_ok()
                        } else {
                            false
                        }
                    } else {
                        dict.get(b"FontFile").is_ok()
                            || dict.get(b"FontFile2").is_ok()
                            || dict.get(b"FontFile3").is_ok()
                    };

                    if !has_font_file && !uncompressed_non_embedded.contains(&font_name) {
                        uncompressed_non_embedded.push(font_name);
                    }
                }
            }
        }
    }

    if !uncompressed_non_embedded.is_empty() {
        return Err(format!(
            "PDF/A 規格への変換エラー: 以下のフォントが完全に埋め込まれていません (ISO 19005-1 適合性要件): {}",
            uncompressed_non_embedded.join(", ")
        ));
    }

    // ISO 19005-1 requirement: OutputIntent GTS_PDFA1 MUST have an embedded DestOutputProfile ICC stream
    let icc_bytes = crate::pdf_engine::pdf_x::generate_valid_icc("sRGB IEC61966-2.1", true);
    let mut icc_dict = Dictionary::new();
    icc_dict.set("N", Object::Integer(3)); // RGB profile (3 channels)
    let icc_stream = Stream::new(icc_dict, icc_bytes);
    let icc_id = doc.add_object(Object::Stream(icc_stream));

    let mut identification = Dictionary::new();
    identification.set("Type", Object::Name("OutputIntent".into()));
    identification.set("S", Object::Name("GTS_PDFA1".into()));
    identification.set(
        "OutputConditionIdentifier",
        Object::String(b"sRGB IEC61966-2.1".to_vec(), lopdf::StringFormat::Literal),
    );
    identification.set(
        "RegistryName",
        Object::String(
            b"http://www.color.org".to_vec(),
            lopdf::StringFormat::Literal,
        ),
    );
    identification.set("DestOutputProfile", Object::Reference(icc_id));

    let identification_id = doc.add_object(Object::Dictionary(identification));

    let root_id = doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference())
        .ok()
        .ok_or("No root catalog found in PDF")?;

        // Valid ISO 19005-1 XMP Metadata packet
        let xmp_metadata = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
    <pdfaid:part>1</pdfaid:part>
    <pdfaid:conformance>B</pdfaid:conformance>
  </rdf:Description>
  <rdf:Description rdf:about="" xmlns:pdf="http://ns.adobe.com/pdf/1.3/">
    <pdf:Producer>Nagisa PDF</pdf:Producer>
  </rdf:Description>
</rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

    let mut meta_dict = Dictionary::new();
    meta_dict.set("Type", Object::Name("Metadata".into()));
    meta_dict.set("Subtype", Object::Name("XML".into()));
    let meta_stream = Stream::new(meta_dict, xmp_metadata.as_bytes().to_vec());
    let meta_id = doc.add_object(Object::Stream(meta_stream));

    if let Some(Object::Dictionary(ref mut root)) = doc.objects.get_mut(&root_id) {
        root.set(
            "OutputIntents",
            Object::Array(vec![Object::Reference(identification_id)]),
        );
        let mut mark_info = Dictionary::new();
        mark_info.set("Marked", Object::Boolean(true));
        root.set("MarkInfo", Object::Dictionary(mark_info));
        root.set("Metadata", Object::Reference(meta_id));
    }

    doc.version = "1.4".to_string();
    save_doc(&mut doc)
}

// ===== HEADERS & FOOTERS =====

pub fn add_header_footer(
    data: &[u8],
    header_text: &str,
    footer_text: &str,
    font_size: f32,
    margin: f32,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc).clone();

    // Check if CJK characters are present
    let has_cjk = header_text.chars().any(|c| !c.is_ascii())
        || footer_text.chars().any(|c| !c.is_ascii());

    let (font_id, cjk_font_opt) = if has_cjk {
        // Embed unified Type0 TrueType CJK font for Header/Footer
        let combined_text = format!("{} {} 0123456789/", header_text, footer_text);
        let encoder = crate::pdf_engine::font_unicode::create_unicode_font_encoder(&mut doc, &combined_text)?;
        (encoder.font_id, Some(encoder))
    } else {
        // Standard /Helvetica font for pure ASCII
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name(b"Font".to_vec()));
        font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
        font_dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        let fid = doc.add_object(Object::Dictionary(font_dict));
        (fid, None)
    };

    for (i, &page_id) in page_ids.iter().enumerate() {
        let (_pw, ph) = get_page_dimensions(&doc, page_id);

        let header = header_text
            .replace("{page}", &(i + 1).to_string())
            .replace("{total}", &page_ids.len().to_string());
        let footer = footer_text
            .replace("{page}", &(i + 1).to_string())
            .replace("{total}", &page_ids.len().to_string());

        let header_tj = if let Some(ref encoder) = cjk_font_opt {
            let cids = encoder.encode_text(&header);
            lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(cids, lopdf::StringFormat::Hexadecimal)],
            )
        } else {
            lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(
                    header.as_bytes().to_vec(),
                    lopdf::StringFormat::Literal,
                )],
            )
        };

        let footer_tj = if let Some(ref encoder) = cjk_font_opt {
            let cids = encoder.encode_text(&footer);
            lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(cids, lopdf::StringFormat::Hexadecimal)],
            )
        } else {
            lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(
                    footer.as_bytes().to_vec(),
                    lopdf::StringFormat::Literal,
                )],
            )
        };

        let operations = vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new("BT", vec![]),
            lopdf::content::Operation::new(
                "Tf",
                vec![
                    Object::Name("HeaderFooterFont".into()),
                    Object::Real(font_size),
                ],
            ),
            lopdf::content::Operation::new(
                "rg",
                vec![Object::Real(0.3), Object::Real(0.3), Object::Real(0.3)],
            ),
            lopdf::content::Operation::new(
                "Td",
                vec![Object::Real(margin), Object::Real(ph - margin)],
            ),
            header_tj,
            lopdf::content::Operation::new("ET", vec![]),
            lopdf::content::Operation::new("BT", vec![]),
            lopdf::content::Operation::new(
                "Tf",
                vec![
                    Object::Name("HeaderFooterFont".into()),
                    Object::Real(font_size),
                ],
            ),
            lopdf::content::Operation::new(
                "Td",
                vec![Object::Real(margin), Object::Real(margin)],
            ),
            footer_tj,
            lopdf::content::Operation::new("ET", vec![]),
            lopdf::content::Operation::new("Q", vec![]),
        ];

        let content = lopdf::content::Content { operations };
        let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

        let mut stream = Stream::new(Dictionary::new(), content_bytes);
        stream.dict.set("Type", Object::Name("Content".into()));
        let content_id = doc.add_object(stream);

        // Register font in page resources safely
        let mut resources = resolve_page_resources(&doc, page_id);
        let mut fonts = match resources.get(b"Font") {
            Ok(Object::Dictionary(f)) => f.clone(),
            Ok(Object::Reference(f_ref)) => doc
                .objects
                .get(f_ref)
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        fonts.set("HeaderFooterFont", Object::Reference(font_id));
        resources.set("Font", Object::Dictionary(fonts));

        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.set("Resources", Object::Dictionary(resources));
        }

        append_page_content(&mut doc, page_id, content_id)?;
    }

    save_doc(&mut doc)
}

// ===== BOOKMARKS =====

pub fn add_bookmark(data: &[u8], title: &str, page_index: usize) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let page_id = page_ids[page_index];

    // Ensure Catalog exists
    let (root_id, _) = super::page_tree::ensure_catalog_and_pages_root(&mut doc);

    // Check if Catalog already has Outlines
    let outline_id = if let Some(Object::Dictionary(ref root_dict)) = doc.objects.get(&root_id) {
        root_dict
            .get(b"Outlines")
            .ok()
            .and_then(|o| o.as_reference().ok())
    } else {
        None
    };

    let outline_id = match outline_id {
        Some(oid) => oid,
        None => {
            let mut outlines = Dictionary::new();
            outlines.set("Type", Object::Name("Outlines".into()));
            outlines.set("Count", Object::Integer(0));
            let oid = doc.add_object(Object::Dictionary(outlines));
            if let Some(Object::Dictionary(ref mut root_dict)) = doc.objects.get_mut(&root_id) {
                root_dict.set("Outlines", Object::Reference(oid));
            }
            oid
        }
    };

    // Create bookmark item dictionary
    let mut item_dict = Dictionary::new();
    item_dict.set(
        "Title",
        Object::String(encode_pdf_text_string(title), lopdf::StringFormat::Literal),
    );
    item_dict.set("Parent", Object::Reference(outline_id));
    item_dict.set(
        "Dest",
        Object::Array(vec![
            Object::Reference(page_id),
            Object::Name("FitH".into()),
            Object::Real(0.0),
        ]),
    );

    // Read existing Outlines dictionary to find First, Last, and Count
    let (first_ref, last_ref, current_count) =
        if let Some(Object::Dictionary(ref out_dict)) = doc.objects.get(&outline_id) {
            let first = out_dict
                .get(b"First")
                .ok()
                .and_then(|o| o.as_reference().ok());
            let last = out_dict
                .get(b"Last")
                .ok()
                .and_then(|o| o.as_reference().ok());
            let count = out_dict
                .get(b"Count")
                .ok()
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0);
            (first, last, count)
        } else {
            (None, None, 0)
        };

    // If there is an existing last item, link its Next to this new item, and new item's Prev to last
    if let Some(prev_last_id) = last_ref {
        item_dict.set("Prev", Object::Reference(prev_last_id));
    }

    let new_item_id = doc.add_object(Object::Dictionary(item_dict));

    if let Some(prev_last_id) = last_ref {
        if let Some(Object::Dictionary(ref mut prev_dict)) = doc.objects.get_mut(&prev_last_id) {
            prev_dict.set("Next", Object::Reference(new_item_id));
        }
    }

    // Update Outlines dictionary First, Last, and Count
    if let Some(Object::Dictionary(ref mut out_dict)) = doc.objects.get_mut(&outline_id) {
        if first_ref.is_none() {
            out_dict.set("First", Object::Reference(new_item_id));
        }
        out_dict.set("Last", Object::Reference(new_item_id));
        out_dict.set("Count", Object::Integer(current_count + 1));
    }

    save_doc(&mut doc)
}

// ===== BATES NUMBERING =====

pub fn add_bates_number(
    data: &[u8],
    prefix: &str,
    start_number: usize,
    font_size: f32,
    margin: f32,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc).clone();

    let has_cjk = prefix.chars().any(|c| !c.is_ascii());

    let (font_id, cjk_font_opt) = if has_cjk {
        let combined_text = format!("{} 0123456789", prefix);
        let encoder = crate::pdf_engine::font_unicode::create_unicode_font_encoder(&mut doc, &combined_text)?;
        (encoder.font_id, Some(encoder))
    } else {
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name(b"Font".to_vec()));
        font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
        font_dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        let fid = doc.add_object(Object::Dictionary(font_dict));
        (fid, None)
    };

    for (i, &page_id) in page_ids.iter().enumerate() {
        let (pw, _ph) = get_page_dimensions(&doc, page_id);

        let bates_text = format!("{}{:06}", prefix, start_number + i);

        let text_tj = if let Some(ref encoder) = cjk_font_opt {
            let cids = encoder.encode_text(&bates_text);
            lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(cids, lopdf::StringFormat::Hexadecimal)],
            )
        } else {
            lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(
                    bates_text.as_bytes().to_vec(),
                    lopdf::StringFormat::Literal,
                )],
            )
        };

        let operations = vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new("BT", vec![]),
            lopdf::content::Operation::new(
                "Tf",
                vec![Object::Name("BatesFont".into()), Object::Real(font_size)],
            ),
            lopdf::content::Operation::new(
                "rg",
                vec![Object::Real(0.0), Object::Real(0.0), Object::Real(0.0)],
            ),
            lopdf::content::Operation::new(
                "Td",
                vec![Object::Real(pw - margin - 60.0), Object::Real(margin)],
            ),
            text_tj,
            lopdf::content::Operation::new("ET", vec![]),
            lopdf::content::Operation::new("Q", vec![]),
        ];

        let content = lopdf::content::Content { operations };
        let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

        let mut stream = Stream::new(Dictionary::new(), content_bytes);
        stream.dict.set("Type", Object::Name("Content".into()));
        let content_id = doc.add_object(stream);

        // Register font in page resources safely
        let mut resources = resolve_page_resources(&doc, page_id);
        let mut fonts = match resources.get(b"Font") {
            Ok(Object::Dictionary(f)) => f.clone(),
            Ok(Object::Reference(f_ref)) => doc
                .objects
                .get(f_ref)
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        fonts.set("BatesFont", Object::Reference(font_id));
        resources.set("Font", Object::Dictionary(fonts));

        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.set("Resources", Object::Dictionary(resources));
        }

        append_page_content(&mut doc, page_id, content_id)?;
    }

    save_doc(&mut doc)
}
