use super::common::*;
use super::*;
use lopdf::{Dictionary, Document, Object, Stream};
use sha2::{Digest, Sha256};

// ===== BATCH PROCESSING =====

pub fn batch_merge_pdfs(paths: &[String], output_path: &str) -> Result<(), String> {
    batch_merge_pdfs_with_options(paths, output_path, &MergeOptions::default())
}

/// Options mirroring the「出力設定」panel of the Merge view.
#[derive(Clone, Debug)]
pub struct MergeOptions {
    /// Rebuild a combined outline tree from every source document, with page
    /// destinations shifted by the running page offset (separator pages included).
    pub keep_bookmarks: bool,
    /// When true, permission-restricted PDFs (empty user password) are decrypted
    /// in memory before merging; files that really require a user password abort
    /// the batch with an error naming the offending file. When false, any
    /// encrypted input aborts up front (merging ciphered streams would silently
    /// emit a corrupt PDF — the engine must never do that).
    pub handle_password: bool,
    /// Insert a separator page between every pair of source documents.
    pub insert_separator: bool,
    /// Optional text drawn on separator pages (empty = blank page).
    pub separator_text: String,
    /// Optional password tried first when decrypting encrypted inputs
    /// (empty/None ⇒ try the empty user password).
    pub password: Option<String>,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            keep_bookmarks: true,
            handle_password: true,
            insert_separator: false,
            separator_text: String::new(),
            password: None,
        }
    }
}

/// Depth-first (preorder) walk of the outline tree preserving sibling order —
/// `inspect::get_bookmarks_from_doc` uses a LIFO queue which scrambles ordering,
/// so merge builds its own traversal. Each entry's page index is shifted by
/// `page_offset` (position of this document inside the merged output).
fn collect_outlines_with_offset(doc: &Document, page_offset: usize) -> Vec<serde_json::Value> {
    fn resolve_dest_page(dest_obj: &Object, page_ids: &[OID]) -> Option<usize> {
        let target = match dest_obj {
            Object::Array(arr) => arr.first().and_then(|o| o.as_reference().ok()),
            Object::Reference(id) => Some(*id),
            _ => None,
        }?;
        page_ids.iter().position(|&pid| pid == target)
    }

    fn walk(
        doc: &Document,
        start: OID,
        page_ids: &[OID],
        page_offset: usize,
        out: &mut Vec<serde_json::Value>,
    ) {
        let mut cur = Some(start);
        while let Some(id) = cur {
            let dict = match doc.objects.get(&id).and_then(|o| o.as_dict().ok()) {
                Some(d) => d,
                None => break,
            };
            let title = dict
                .get(b"Title")
                .ok()
                .and_then(|o| match o {
                    Object::String(bytes, _) => Some(decode_pdf_text_string(bytes)),
                    _ => None,
                })
                .unwrap_or_else(|| "Untitled".to_string());

            // Destination: explicit /Dest, or /A << /S /GoTo /D … >> action.
            let local_page = dict
                .get(b"Dest")
                .ok()
                .and_then(|o| resolve_dest_page(o, page_ids))
                .or_else(|| {
                    dict.get(b"A")
                        .ok()
                        .and_then(|a| match a {
                            Object::Reference(aid) => {
                                doc.objects.get(aid).and_then(|o| o.as_dict().ok())
                            }
                            Object::Dictionary(ad) => Some(ad),
                            _ => None,
                        })
                        .and_then(|ad| {
                            ad.get(b"D")
                                .ok()
                                .and_then(|d| resolve_dest_page(d, page_ids))
                        })
                });
            // Unresolvable destination ⇒ start of this document's range.
            let page = page_offset + local_page.unwrap_or(0);

            out.push(serde_json::json!({ "title": title, "page": page }));

            if let Ok(Object::Reference(child)) = dict.get(b"First") {
                walk(doc, *child, page_ids, page_offset, out);
            }
            cur = dict.get(b"Next").ok().and_then(|o| o.as_reference().ok());
        }
    }

    let page_ids = get_page_ids(doc);
    let mut out = Vec::new();
    if let Ok(root_ref) = doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        if let Some(root) = doc.objects.get(&root_ref).and_then(|o| o.as_dict().ok()) {
            let outlines_id = match root.get(b"Outlines").ok() {
                Some(Object::Reference(id)) => Some(*id),
                _ => None,
            };
            if let Some(oid) = outlines_id {
                if let Some(outlines) = doc.objects.get(&oid).and_then(|o| o.as_dict().ok()) {
                    if let Ok(Object::Reference(first)) = outlines.get(b"First") {
                        walk(doc, *first, &page_ids, page_offset, &mut out);
                    }
                }
            }
        }
    }
    out
}

/// Extract a plain filename for user-facing error messages.
fn path_display_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Build a single separator page (optionally labelled) used between merged files.
fn build_separator_page(width: f64, height: f64, text: &str) -> Result<Vec<u8>, String> {
    let blank = create_blank_pdf(width, height, 1)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(blank);
    }
    // Rough centring: ASCII ≈ 0.55em wide, full-width glyphs ≈ 1em.
    let size = 24.0f64;
    let est_width: f64 = trimmed
        .chars()
        .map(|c| if c.is_ascii() { size * 0.55 } else { size })
        .sum();
    let x = ((width - est_width) / 2.0).max(36.0);
    let y = height / 2.0;
    add_text(&blank, 0, trimmed, x, y, size, "#334155")
}

/// Full merge pipeline backing the Merge view options.
pub fn batch_merge_pdfs_with_options(
    paths: &[String],
    output_path: &str,
    opts: &MergeOptions,
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No files to merge".into());
    }

    let mut buffers: Vec<Vec<u8>> = Vec::new();
    let mut all_bookmarks: Vec<serde_json::Value> = Vec::new();
    let mut page_offset: usize = 0;
    let mut separator_dims: Option<(f64, f64)> = None;

    for (idx, path) in paths.iter().enumerate() {
        let name = path_display_name(path);
        let bytes =
            std::fs::read(path).map_err(|e| format!("ファイルを読み込めません: {name}: {e}"))?;
        let mut doc =
            Document::load_mem(&bytes).map_err(|e| format!("PDFを開けません: {name}: {e}"))?;

        let encrypted = doc.trailer.has(b"Encrypt");
        if encrypted {
            if !opts.handle_password {
                return Err(format!(
                    "パスワード保護されたPDFが含まれます: {name}（「パスワード付きPDFの扱い」を有効にするか、事前にロック解除してください）"
                ));
            }
            // Try the user-supplied password first, then the empty user password
            // (covers permission-restricted PDFs — owner password set, user password
            // empty — the common case). A wrong/required password fails here with an
            // honest, filename-tagged error.
            let pw = opts
                .password
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("");
            doc.decrypt(pw)
                .map_err(|e| format!("パスワード保護されたPDFを復号できませんでした: {name}: {e}"))?;
            // lopdf's decrypt() removes /Encrypt from the trailer on success.
        }

        // Separator page geometry follows the first document that established it.
        if separator_dims.is_none() {
            let pids = get_page_ids(&doc);
            if let Some(&pid) = pids.first() {
                let (w, h) = get_page_dimensions(&doc, pid);
                separator_dims = Some((w as f64, h as f64));
            }
        }

        let page_count = get_page_ids(&doc).len();
        if opts.keep_bookmarks {
            let marks = collect_outlines_with_offset(&doc, page_offset);
            all_bookmarks.extend(marks);
        }

        let buf = if encrypted {
            // Re-serialize decrypted content so streams/strings are plaintext.
            save_doc(&mut doc)?
        } else {
            bytes
        };
        buffers.push(buf);
        page_offset += page_count;

        if opts.insert_separator && idx + 1 < paths.len() {
            let (w, h) = separator_dims.unwrap_or((595.28, 841.89));
            let sep = build_separator_page(w, h, &opts.separator_text)?;
            buffers.push(sep);
            page_offset += 1;
        }
    }

    let refs: Vec<&[u8]> = buffers.iter().map(|b| b.as_slice()).collect();
    let mut merged = merge_pdf_buffers_robust(&refs)?;

    if opts.keep_bookmarks && !all_bookmarks.is_empty() {
        merged = add_bookmark_tree(&merged, &all_bookmarks)?;
    }

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

    // ISO 19005-1 requires a permanent file identifier in the trailer. Without it
    // the document can never satisfy PDF/A even though everything else is done.
    if doc.trailer.get(b"ID").is_err() {
        let id = generate_pdfa_file_id(&doc);
        doc.trailer.set("ID", Object::Array(vec![id.clone(), id]));
    }

    save_doc(&mut doc)
}

/// Build the 16-byte permanent identifier required by ISO 19005-1.
///
/// The value is derived from the document's structural content so it stays
/// stable across re-saves of the same file, and it is clearly marked as a
/// generated identifier rather than an arbitrary nonce.
fn generate_pdfa_file_id(doc: &Document) -> Object {
    let mut hasher = Sha256::new();
    hasher.update(doc.version.as_bytes());
    // Object ids are stable for a given document, so hashing them gives a
    // deterministic identifier without re-serialising the whole file.
    for id in doc.objects.keys() {
        hasher.update(id.0.to_be_bytes());
        hasher.update(id.1.to_be_bytes());
    }
    let digest = hasher.finalize();
    let mut id = vec![0u8; 16];
    id.copy_from_slice(&digest[..16]);
    // The permanent identifier is written as a hexadecimal string, which is the
    // conventional PDF representation of the 16-byte file identifier.
    Object::String(id, lopdf::StringFormat::Hexadecimal)
}

/// Result of a PDF/A (ISO 19005) conformance inspection.
/// Mirrors the shape of [`crate::pdf_engine::pdf_x::PdfxValidationReport`] so the
/// GUI can render both standards uniformly.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PdfaValidationReport {
    pub is_compliant: bool,
    pub standard: String,
    pub passed_checks: Vec<String>,
    pub violations: Vec<String>,
    pub details: serde_json::Value,
}

/// Inspect a PDF for PDF/A (ISO 19005) conformance without modifying it.
///
/// Checks the structural requirements that [`convert_to_pdfa`] enforces when it
/// writes a conforming file: every font embedded, an `OutputIntent` carrying a
/// `GTS_PDFA1` subtype with an embedded ICC `DestOutputProfile`, an XMP metadata
/// packet declaring `pdfaid:part`/`pdfaid:conformance`, a `MarkInfo /Marked`
/// entry and a 1.4-or-newer header version.
pub fn validate_pdfa_compliance(
    data: &[u8],
    target_conformance: &str,
) -> Result<PdfaValidationReport, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to parse PDF: {e}"))?;

    let conformance = match target_conformance.to_ascii_uppercase().as_str() {
        "A" | "A1" | "A2" => "A",
        _ => "B",
    };
    let standard_name = format!("PDF/A-1{conformance} (ISO 19005-1)");

    let mut passed: Vec<String> = Vec::new();
    let mut violations: Vec<String> = Vec::new();

    // 1. Header version must be 1.4 or newer for PDF/A-1.
    let major = doc
        .version
        .split('.')
        .next()
        .and_then(|m| m.parse::<u32>().ok());
    let minor = doc
        .version
        .split('.')
        .nth(1)
        .and_then(|m| m.parse::<u32>().ok());
    let version_ok = match (major, minor) {
        (Some(1), Some(min)) => min >= 4,
        (Some(maj), _) if maj >= 2 => true,
        _ => false,
    };
    if version_ok {
        passed.push(format!("Header version {} is PDF/A-1 compatible (>= 1.4)", doc.version));
    } else {
        violations.push(format!(
            "Header version {} must be 1.4 or newer for PDF/A-1",
            doc.version
        ));
    }

    // 2. All fonts must be embedded (hard ISO 19005-1 requirement).
    let mut total_fonts = 0usize;
    let mut non_embedded: Vec<String> = Vec::new();
    for (_, obj) in &doc.objects {
        if let Object::Dictionary(dict) = obj {
            let is_font = dict
                .get(b"Type")
                .ok()
                .and_then(|o| o.as_name().ok())
                .map(|name| name == b"Font")
                .unwrap_or(false);
            if !is_font {
                continue;
            }
            total_fonts += 1;

            let font_name = dict
                .get(b"BaseFont")
                .ok()
                .and_then(|o| match o {
                    Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| "Unknown".to_string());

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

            if !has_font_file && !non_embedded.contains(&font_name) {
                non_embedded.push(font_name);
            }
        }
    }
    if non_embedded.is_empty() {
        passed.push(format!(
            "All {total_fonts} font(s) are fully embedded (ISO 19005-1 requirement)"
        ));
    } else {
        violations.push(format!(
            "Fonts are not fully embedded: {} (ISO 19005-1 requires embedded fonts)",
            non_embedded.join(", ")
        ));
    }

    // 3. OutputIntent with GTS_PDFA1 subtype and an embedded ICC profile.
    let root_id = match doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference())
    {
        Ok(id) => id,
        Err(e) => return Err(format!("PDF Root Catalog not found: {}", e)),
    };

    let mut has_pdfa_intent = false;
    let mut has_icc = false;
    let mut output_condition = String::from("None");
    if let Some(Object::Dictionary(ref root)) = doc.objects.get(&root_id) {
        if let Ok(Object::Array(ref intents)) = root.get(b"OutputIntents") {
            for item in intents {
                let intent_dict = match item {
                    Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                };
                if let Some(dict) = intent_dict {
                    let subtype_ok = dict
                        .get(b"S")
                        .ok()
                        .and_then(|o| o.as_name().ok())
                        .map(|s| s == b"GTS_PDFA1")
                        .unwrap_or(false);
                    if !subtype_ok {
                        continue;
                    }
                    has_pdfa_intent = true;
                    if let Ok(ident) = dict.get(b"OutputConditionIdentifier") {
                        output_condition = match ident {
                            Object::String(bytes, _) => String::from_utf8_lossy(bytes).to_string(),
                            Object::Name(bytes) => String::from_utf8_lossy(bytes).to_string(),
                            _ => "Standard Output Condition".to_string(),
                        };
                    }
                    // DestOutputProfile must be an embedded ICC stream (N = 1/3/4).
                    if let Ok(prof_ref) =
                        dict.get(b"DestOutputProfile").and_then(|o| o.as_reference())
                    {
                        if let Some(Object::Stream(prof)) = doc.objects.get(&prof_ref) {
                            let n = prof.dict.get(b"N").and_then(|o| o.as_i64()).unwrap_or(0);
                            if n == 1 || n == 3 || n == 4 {
                                has_icc = true;
                            }
                        }
                    }
                }
            }
        }
    }
    if has_pdfa_intent && has_icc {
        passed.push(format!(
            "OutputIntent 'GTS_PDFA1' present with condition: {output_condition} and embedded ICC DestOutputProfile"
        ));
    } else if has_pdfa_intent {
        passed.push(format!(
            "OutputIntent 'GTS_PDFA1' present with condition: {output_condition}"
        ));
        violations.push(
            "OutputIntent is missing an embedded ICC DestOutputProfile (ISO 19005-1 requires an ICC profile)"
                .to_string(),
        );
    } else {
        violations.push("OutputIntents entry with S=GTS_PDFA1 is missing".to_string());
    }

    // 4. XMP metadata packet declaring pdfaid:part / pdfaid:conformance.
    let mut has_xmp = false;
    let mut xmp_declared_part = false;
    if let Some(Object::Dictionary(ref root)) = doc.objects.get(&root_id) {
        if let Ok(meta_ref) = root.get(b"Metadata").and_then(|o| o.as_reference()) {
            if let Some(Object::Stream(stream)) = doc.objects.get(&meta_ref) {
                let raw = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                if let Ok(text) = String::from_utf8(raw) {
                    has_xmp = text.contains("xpacket") || text.contains("xmpmeta");
                    xmp_declared_part = text.contains("pdfaid:part");
                }
            }
        }
    }
    if has_xmp && xmp_declared_part {
        passed
            .push("XMP metadata packet declares the PDF/A identification (pdfaid:part)".to_string());
    } else if has_xmp {
        passed.push("XMP metadata packet is present".to_string());
        violations.push(
            "XMP metadata does not declare pdfaid:part (ISO 19005-1 requires PDF/A identification)"
                .to_string(),
        );
    } else {
        violations
            .push("XMP metadata stream is missing (ISO 19005-1 requires a valid XMP packet)".to_string());
    }

    // 5. MarkInfo /Marked — mandatory for conformance level A, optional (not a
    //    violation) for level B. Flagging it for level B was a false positive.
    let marked = doc
        .objects
        .get(&root_id)
        .and_then(|o| o.as_dict().ok())
        .and_then(|root| root.get(b"MarkInfo").ok())
        .and_then(|mi| match mi {
            Object::Dictionary(d) => Some(d),
            Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()),
            _ => None,
        })
        .and_then(|d| d.get(b"Marked").ok())
        .and_then(|m| m.as_bool().ok())
        .unwrap_or(false);
    if marked {
        passed.push("Catalog declares MarkInfo /Marked = true (structured document)".to_string());
    } else if conformance == "A" {
        violations.push(
            "Catalog is missing MarkInfo /Marked = true (mandatory for PDF/A-1a level A)"
                .to_string(),
        );
    } else {
        passed
            .push("MarkInfo /Marked is not declared (optional for PDF/A-1b level B)".to_string());
    }

    // 6. Encryption is forbidden in every PDF/A conformance level.
    if doc.trailer.get(b"Encrypt").is_ok() {
        violations.push("Document is encrypted (PDF/A forbids encryption in all parts)".to_string());
    } else {
        passed.push("Document is not encrypted (PDF/A requirement)".to_string());
    }

    // 7. The trailer file identifier (/ID) is mandatory in PDF/A.
    let has_trailer_id = matches!(doc.trailer.get(b"ID"), Ok(Object::Array(_)));
    if has_trailer_id {
        passed.push("Trailer contains the mandatory /ID file identifier".to_string());
    } else {
        violations.push("Trailer is missing the mandatory /ID file identifier".to_string());
    }

    // 8-15. Construct-level prohibitions, detected in a single pass over every
    //      object; each flag records that at least one offending construct exists.
    let mut has_javascript = false;
    let mut has_xfa = false;
    let mut has_postscript_xobject = false;
    let mut has_opi = false;
    let mut has_lzw = false;
    let mut has_transparency = false;
    let mut has_embedded_files = false;
    let mut has_external_reference = false;

    for (_, obj) in &doc.objects {
        let dict = match obj {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => continue,
        };
        if dict.get(b"JS").is_ok() || dict.get(b"JavaScript").is_ok() {
            has_javascript = true;
        }
        if dict.get(b"XFA").is_ok() {
            has_xfa = true;
        }
        if dict.get(b"OPI").is_ok() {
            has_opi = true;
        }
        if dict.get(b"EmbeddedFiles").is_ok() {
            has_embedded_files = true;
        }
        // /F in a file specification points at a file outside the document.
        if matches!(dict.get(b"F"), Ok(Object::String(_, _))) {
            has_external_reference = true;
        }
        let is_lzw = dict
            .get(b"Filter")
            .ok()
            .map(|f| {
                f.as_name().map(|n| n == b"LZWDecode").unwrap_or(false)
                    || f.as_array()
                        .map(|items| {
                            items
                                .iter()
                                .any(|i| i.as_name().map(|n| n == b"LZWDecode").unwrap_or(false))
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        if is_lzw {
            has_lzw = true;
        }
        // PDF/A-1 has no transparency model at all.
        if dict.get(b"Group").is_ok() || dict.get(b"SMask").is_ok() {
            has_transparency = true;
        }
        if dict
            .get(b"Subtype")
            .and_then(|o| o.as_name())
            .map(|s| s == b"PS")
            .unwrap_or(false)
        {
            has_postscript_xobject = true;
        }
    }

    let mut forbid = |found: bool, violation: &str, passed_label: &str| {
        if found {
            violations.push(violation.to_string());
        } else {
            passed.push(passed_label.to_string());
        }
    };

    forbid(
        has_javascript,
        "Document contains JavaScript (ISO 19005-1 prohibits JavaScript actions)",
        "No JavaScript actions found",
    );
    forbid(
        has_xfa,
        "Document contains XFA form data (prohibited in PDF/A-1)",
        "No XFA dynamic form content found",
    );
    forbid(
        has_postscript_xobject,
        "Document contains a PostScript XObject (prohibited in PDF/A-1)",
        "No PostScript XObjects found",
    );
    forbid(
        has_opi,
        "Document contains OPI (Open Production Interface) references (prohibited)",
        "No OPI references found",
    );
    forbid(
        has_lzw,
        "Document uses LZW compression (ISO 19005-1 requires LZW-free streams)",
        "No LZW compression found",
    );
    forbid(
        has_transparency,
        "Document uses transparency (/Group or /SMask) which PDF/A-1 does not support",
        "No transparency constructs found",
    );
    forbid(
        has_embedded_files,
        "Document embeds files (allowed from PDF/A-3 onward, not PDF/A-1/2)",
        "No embedded file attachments found",
    );
    forbid(
        has_external_reference,
        "Document references an external file (/F) (PDF/A files must be self-contained)",
        "No external file references found",
    );

    let is_compliant = violations.is_empty();
    Ok(PdfaValidationReport {
        is_compliant,
        standard: standard_name,
        passed_checks: passed,
        violations,
        details: serde_json::json!({
            "pdf_version": doc.version,
            "conformance": conformance,
            "total_fonts": total_fonts,
            "non_embedded_fonts": non_embedded,
            "output_condition": output_condition,
            "has_output_intent": has_pdfa_intent,
            "has_icc_profile": has_icc,
            "has_xmp": has_xmp,
            "marked": marked,
            "encrypted": doc.trailer.get(b"Encrypt").is_ok(),
            "has_trailer_id": has_trailer_id,
            "has_javascript": has_javascript,
            "has_xfa": has_xfa,
            "has_postscript_xobject": has_postscript_xobject,
            "has_opi": has_opi,
            "has_lzw": has_lzw,
            "has_transparency": has_transparency,
            "has_embedded_files": has_embedded_files,
            "has_external_reference": has_external_reference,
            "checked_rule_count": 15,
        }),
    })
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
