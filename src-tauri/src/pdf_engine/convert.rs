use super::common::*;
use super::*;
use lopdf::{Dictionary, Document, Object, Stream};
// #42: タイムアウト付き外部コマンド実行
use crate::pdf_engine::common::{run_command_with_timeout, EXTERNAL_CMD_TIMEOUT_SECS};

/// #46 是正: 一時ディレクトリの RAII ガード。
///
/// `convert.rs` の `pdf_to_images` / `html_to_pdf` では関数末尾の
/// `remove_dir_all` に依存していたため、`?` 演算子による早期リターン時に
/// 一時ディレクトリが残存してディスクを圧迫する問題があった。
/// `Drop` トレイトで `remove_dir_all` を自動実行することで
/// どのリターンパスでもクリーンアップを保証する。
struct TempDirGuard(std::path::PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ===== PDF→IMAGE CONVERSION =====

pub fn pdf_to_images(
    data: &[u8],
    output_dir: &str,
    format: &str,
    dpi: u32,
) -> Result<Vec<String>, String> {
    let unique = format!(
        "nagisa_pdf2img_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let tmp = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    // #46 是正: RAII ガードで早期リターン時のディレクトリ残存を防止
    let _tmp_guard = TempDirGuard(tmp.clone());
    let input = tmp.join("input.pdf");
    std::fs::write(&input, data).map_err(|e| e.to_string())?;

    let mut cmd = find_tool_command("pdftoppm");
    cmd.arg("-r").arg(dpi.to_string());
    if format == "jpg" {
        cmd.arg("-jpeg");
    } else {
        cmd.arg("-png");
    }
    cmd.arg(&input).arg(tmp.join("page"));

    // #42 是正: pdftoppm をタイムアウト付き実行に変更（細工ファイルによるDoS防止）
    let pdftoppm_res = run_command_with_timeout(cmd, EXTERNAL_CMD_TIMEOUT_SECS);
    let pdftoppm_success = match &pdftoppm_res {
        Ok(output) => output.status.success(),
        Err(_) => false,
    };

    if !pdftoppm_success {
        // Fallback: Pure-Rust extraction of embedded page images (useful for scanned PDFs and embedded visuals
        // when poppler pdftoppm is not installed on the system)
        if let Ok(doc) = Document::load_mem(data) {
            let mut extracted_count = 0;
            let mut page_idx = 1;
            for page_id in get_page_ids(&doc) {
                let resources = resolve_page_resources(&doc, page_id);
                if let Ok(Object::Dictionary(xobj_dict)) = resources.get(b"XObject") {
                    for (_, obj_ref) in xobj_dict.iter() {
                        let stream_opt = match obj_ref {
                            Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_stream().ok()),
                            _ => None,
                        };
                        if let Some(stream) = stream_opt {
                            if let Ok(sub) = stream.dict.get(b"Subtype").and_then(|o| o.as_name()) {
                                if sub == b"Image" {
                                    let img_bytes = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
                                    let dyn_res = image::load_from_memory(&img_bytes).or_else(|_| {
                                        let w = stream.dict.get(b"Width").and_then(|w| w.as_i64()).unwrap_or(0) as u32;
                                        let h = stream.dict.get(b"Height").and_then(|h| h.as_i64()).unwrap_or(0) as u32;
                                        if w > 0 && h > 0 && img_bytes.len() == (w * h * 3) as usize {
                                            image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(w, h, img_bytes).map(image::DynamicImage::ImageRgb8).ok_or(())
                                        } else {
                                            Err(())
                                        }
                                    });
                                    if let Ok(dyn_img) = dyn_res {
                                        let out_filename = format!("page-{:03}.{}", page_idx, if format == "jpg" { "jpg" } else { "png" });
                                        let out_filepath = tmp.join(out_filename);
                                        let save_res = if format == "jpg" {
                                            dyn_img.save_with_format(&out_filepath, image::ImageFormat::Jpeg)
                                        } else {
                                            dyn_img.save_with_format(&out_filepath, image::ImageFormat::Png)
                                        };
                                        if save_res.is_ok() {
                                            extracted_count += 1;
                                            page_idx += 1;
                                            break; // Found primary page image
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if extracted_count == 0 {
                let _ = std::fs::remove_dir_all(&tmp);
                let err_details = match pdftoppm_res {
                    Ok(output) => String::from_utf8_lossy(&output.stderr).to_string(),
                    Err(e) => format!("pdftoppm コマンドが見つからないか実行できませんでした ({e})。システムに poppler (brew install poppler 等) をインストールしてください。"),
                };
                return Err(err_details);
            }
        } else {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err("pdftoppm が見つからず、PDFの直接解析にも失敗しました。".into());
        }
    }

    // Move files to output_dir
    std::fs::create_dir_all(output_dir).map_err(|e| {
        let _ = std::fs::remove_dir_all(&tmp);
        e.to_string()
    })?;
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&tmp) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("page") && (name.ends_with(".jpg") || name.ends_with(".png")) {
                let dest = std::path::Path::new(output_dir).join(&name);
                if std::fs::copy(entry.path(), &dest).is_ok() {
                    result.push(dest.to_string_lossy().to_string());
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    result.sort();
    Ok(result)
}

// ===== IMAGE→PDF CONVERSION =====

pub fn images_to_pdf(image_paths: &[String], output_path: &str) -> Result<(), String> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.add_object(Object::Dictionary(Dictionary::new())); // placeholder

    let mut kids = Vec::new();

    for path in image_paths {
        let img_data = std::fs::read(path).map_err(|e| format!("Failed to read {path}: {e}"))?;
        let img = image::load_from_memory(&img_data).map_err(|e| e.to_string())?;
        let rgb = img.to_rgb8();
        let (width, height) = rgb.dimensions();

        let mut jpeg_buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut jpeg_buf, image::ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to encode image to JPEG: {e}"))?;
        let jpeg_bytes = jpeg_buf.into_inner();

        // Create image XObject
        let mut img_dict = Dictionary::new();
        img_dict.set("Type", Object::Name("XObject".into()));
        img_dict.set("Subtype", Object::Name("Image".into()));
        img_dict.set("Width", Object::Integer(width as i64));
        img_dict.set("Height", Object::Integer(height as i64));
        img_dict.set("ColorSpace", Object::Name("DeviceRGB".into()));
        img_dict.set("BitsPerComponent", Object::Integer(8));
        img_dict.set("Filter", Object::Name("DCTDecode".into()));

        let img_stream = Stream::new(img_dict, jpeg_bytes);
        let img_id = doc.add_object(img_stream);

        // Calculate page size:
        // Scanned documents are typically 200-600 DPI. Using 96 DPI stretches a 2480x3508 A4 scan into an A1 poster!
        // We fit onto standard A4 (595.28 x 841.89 pt) if the image is large, preserving aspect ratio,
        // or calculate dimensions using 150-300 DPI if smaller.
        let a4_w = 595.28f32;
        let a4_h = 841.89f32;

        let (pt_w, pt_h) = if width > 1200 || height > 1200 {
            // High-resolution scan / photo: scale to fit within standard A4 bounds while strictly preserving aspect ratio
            let scale = (a4_w / width as f32).min(a4_h / height as f32);
            ((width as f32 * scale).max(10.0), (height as f32 * scale).max(10.0))
        } else {
            // Screen or web image: calculate at 150 DPI, but scale uniformly if exceeding A4 to prevent aspect distortion
            let base_dpi = 150.0f32;
            let raw_w = width as f32 * 72.0 / base_dpi;
            let raw_h = height as f32 * 72.0 / base_dpi;
            let scale = (a4_w / raw_w).min(a4_h / raw_h).min(1.0f32);
            ((raw_w * scale).max(10.0), (raw_h * scale).max(10.0))
        };

        let mut xobj_dict = Dictionary::new();
        xobj_dict.set("Im1", Object::Reference(img_id));
        let mut res_dict = Dictionary::new();
        let xobj_id = doc.add_object(Object::Dictionary(xobj_dict));
        res_dict.set("XObject", Object::Reference(xobj_id));
        let res_id = doc.add_object(Object::Dictionary(res_dict));

        let content_stream = format!("q {pt_w:.2} 0 0 {pt_h:.2} 0 0 cm /Im1 Do Q");
        let content_id =
            doc.add_object(Stream::new(Dictionary::new(), content_stream.into_bytes()));

        // Create page
        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name("Page".into()));
        page_dict.set("Parent", Object::Reference(pages_id));
        page_dict.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(pt_w),
                Object::Real(pt_h),
            ]),
        );
        page_dict.set("Resources", Object::Reference(res_id));
        page_dict.set("Contents", Object::Reference(content_id));

        let page_id = doc.add_object(Object::Dictionary(page_dict));
        kids.push(Object::Reference(page_id));
    }

    // Update Pages dict
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name("Pages".into()));
    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.set("Count", Object::Integer(image_paths.len() as i64));
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let mut catalog_dict = Dictionary::new();
    catalog_dict.set("Type", Object::Name("Catalog".into()));
    catalog_dict.set("Pages", Object::Reference(pages_id));
    let catalog_id = doc.add_object(Object::Dictionary(catalog_dict));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).map_err(|e| e.to_string())?;
    std::fs::write(output_path, buf).map_err(|e| e.to_string())?;
    Ok(())
}

// ===== HTML→PDF CONVERSION =====

pub fn html_to_pdf(html_content: &str, output_path: &str) -> Result<(), String> {
    let unique = format!(
        "nagisa_html2pdf_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let tmp = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    // #46 是正: RAII ガードで早期リターン時の一時ディレクトリ残存を防止
    let _tmp_guard = TempDirGuard(tmp.clone());
    let html_file = tmp.join("input.html");
    let pdf_file = tmp.join("output.pdf");

    std::fs::write(&html_file, html_content).map_err(|e| e.to_string())?;

    // 1. Try Headless Chromium / Chrome / Edge browsers if installed on system
    let browser_candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        "google-chrome",
        "chromium",
        "chrome",
        "msedge",
    ];

    let mut browser_converted = false;
    for candidate in browser_candidates {
        if candidate.starts_with('/') && !std::path::Path::new(candidate).exists() {
            continue;
        }

        // #42 是正: ブラウザ変換もタイムアウト付き実行（Chromeはフリーズしやすい）
        let cmd_res = {
            let mut c = std::process::Command::new(candidate);
            c.arg("--headless=new")
             .arg("--disable-gpu")
             .arg("--no-pdf-header-footer")
             .arg(format!("--print-to-pdf={}", pdf_file.display()))
             .arg(&html_file);
            run_command_with_timeout(c, EXTERNAL_CMD_TIMEOUT_SECS)
        };

        if let Ok(out) = cmd_res {
            if out.status.success() && pdf_file.exists() {
                if let Ok(pdf_bytes) = std::fs::read(&pdf_file) {
                    if !pdf_bytes.is_empty() {
                        let _ = std::fs::write(output_path, pdf_bytes);
                        browser_converted = true;
                        break;
                    }
                }
            }
        }
    }

    if browser_converted {
        let _ = std::fs::remove_dir_all(&tmp);
        return Ok(());
    }

    // 2. Try wkhtmltopdf if available
    let wk_cmd = {
        let mut c = crate::pdf_engine::common::find_tool_command("wkhtmltopdf");
        c.arg("--disable-local-file-access")
         .arg(&html_file)
         .arg(output_path);
        run_command_with_timeout(c, EXTERNAL_CMD_TIMEOUT_SECS)
    };

    if let Ok(out) = wk_cmd {
        if out.status.success() && std::path::Path::new(output_path).exists() {
            let _ = std::fs::remove_dir_all(&tmp);
            return Ok(());
        }
    }

    // 3. Try weasyprint if available
    let weasy_cmd = {
        let mut c = crate::pdf_engine::common::find_tool_command("weasyprint");
        c.arg(&html_file).arg(output_path);
        run_command_with_timeout(c, EXTERNAL_CMD_TIMEOUT_SECS)
    };

    if let Ok(out) = weasy_cmd {
        if out.status.success() && std::path::Path::new(output_path).exists() {
            let _ = std::fs::remove_dir_all(&tmp);
            return Ok(());
        }
    }

    let _ = std::fs::remove_dir_all(&tmp);

    // 4. Built-in Pure-Rust HTML text renderer fallback:
    // Strips HTML markup cleanly and generates standard PDF with text stream and page layout
    let clean_text = extract_text_from_html(html_content);
    generate_pdf_from_plain_text(&clean_text, output_path)
}

/// Helper: Extract clean readable text from HTML markup with paragraphs and headings preserved
pub(crate) fn extract_text_from_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut current_tag = String::new();
    let mut in_script_or_style = false;

    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '<' {
            in_tag = true;
            current_tag.clear();
        } else if c == '>' {
            in_tag = false;
            let tag_lower = current_tag.to_lowercase();
            let tag_name = tag_lower.split_whitespace().next().unwrap_or("");
            if tag_name == "script" || tag_name == "style" {
                in_script_or_style = true;
            } else if tag_name == "/script" || tag_name == "/style" {
                in_script_or_style = false;
            } else if tag_name == "p" || tag_name == "br" || tag_name == "/p" || tag_name == "div" || tag_name == "/div" || tag_name.starts_with("h1") || tag_name.starts_with("h2") || tag_name.starts_with("h3") || tag_name.starts_with("h4") || tag_name.starts_with("h5") || tag_name.starts_with("h6") {
                out.push('\n');
            } else if tag_name == "li" {
                out.push_str("\n• ");
            } else if tag_name == "tr" || tag_name == "/tr" {
                out.push('\n');
            } else if tag_name == "td" || tag_name == "th" {
                out.push_str("  |  ");
            } else if tag_name == "hr" {
                out.push_str("\n----------------------------------------\n");
            } else if tag_name == "blockquote" {
                out.push_str("\n> ");
            }
        } else if in_tag {
            current_tag.push(c);
        } else if !in_script_or_style {
            out.push(c);
        }
        i += 1;
    }

    // Decode standard HTML entities
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'");

    // Clean up excessive blank lines
    let mut final_text = String::new();
    let mut prev_blank = false;
    for line in decoded.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_blank {
                final_text.push('\n');
                prev_blank = true;
            }
        } else {
            final_text.push_str(trimmed);
            final_text.push('\n');
            prev_blank = false;
        }
    }

    if final_text.trim().is_empty() {
        "HTML Document".to_string()
    } else {
        final_text
    }
}

/// Helper: Generate a valid multi-page PDF document from plain text
pub(crate) fn generate_pdf_from_plain_text(text: &str, output_path: &str) -> Result<(), String> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));

    let lines: Vec<&str> = text.lines().collect();
    let lines_per_page = 45;
    let mut kids = Vec::new();

    let page_chunks: Vec<&[&str]> = if lines.is_empty() {
        vec![&[][..]]
    } else {
        lines.chunks(lines_per_page).collect()
    };

    let page_w = 595.0f32; // A4 pt
    let page_h = 842.0f32;

    let has_non_ascii = text.chars().any(|c| !c.is_ascii());

    if has_non_ascii {
        // True CJK Unicode embedding pipeline using bundled IPAexGothic font
        let encoder = super::font_unicode::create_unicode_font_encoder(&mut doc, text)?;
        let font_id = encoder.font_id;

        for chunk in &page_chunks {
            let mut operations = vec![
                lopdf::content::Operation::new("q", vec![]),
                lopdf::content::Operation::new("BT", vec![]),
                lopdf::content::Operation::new(
                    "Tf",
                    vec![Object::Name("NagisaCJK".into()), Object::Real(11.0)],
                ),
                lopdf::content::Operation::new(
                    "rg",
                    vec![Object::Real(0.1), Object::Real(0.1), Object::Real(0.1)],
                ),
            ];

            for (line_idx, line) in chunk.iter().enumerate() {
                let line_y = 790.0f32 - (line_idx as f32 * 16.0f32);
                operations.push(lopdf::content::Operation::new(
                    "Tm",
                    vec![
                        Object::Real(1.0),
                        Object::Real(0.0),
                        Object::Real(0.0),
                        Object::Real(1.0),
                        Object::Real(50.0),
                        Object::Real(line_y),
                    ],
                ));
                let encoded_cids = encoder.encode_text(line);
                operations.push(lopdf::content::Operation::new(
                    "Tj",
                    vec![Object::String(encoded_cids, lopdf::StringFormat::Hexadecimal)],
                ));
            }

            operations.push(lopdf::content::Operation::new("ET", vec![]));
            operations.push(lopdf::content::Operation::new("Q", vec![]));

            let content = lopdf::content::Content { operations };
            let content_bytes = content.encode().map_err(|e| format!("Content encode: {e}"))?;

            let mut res_dict = Dictionary::new();
            let mut fonts = Dictionary::new();
            fonts.set("NagisaCJK", Object::Reference(font_id));
            res_dict.set("Font", Object::Dictionary(fonts));
            let res_id = doc.add_object(Object::Dictionary(res_dict));

            let stream = Stream::new(Dictionary::new(), content_bytes);
            let stream_id = doc.add_object(stream);

            let mut page_dict = Dictionary::new();
            page_dict.set("Type", Object::Name("Page".into()));
            page_dict.set("Parent", Object::Reference(pages_id));
            page_dict.set(
                "MediaBox",
                Object::Array(vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(page_w),
                    Object::Real(page_h),
                ]),
            );
            page_dict.set("Resources", Object::Reference(res_id));
            page_dict.set("Contents", Object::Reference(stream_id));

            let page_id = doc.add_object(Object::Dictionary(page_dict));
            kids.push(Object::Reference(page_id));
        }
    } else {
        // Standard ASCII Helvetica path: create shared Helvetica font resource with WinAnsiEncoding
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name("Font".into()));
        font_dict.set("Subtype", Object::Name("Type1".into()));
        font_dict.set("BaseFont", Object::Name("Helvetica".into()));
        font_dict.set("Encoding", Object::Name("WinAnsiEncoding".into()));
        let font_id = doc.add_object(Object::Dictionary(font_dict));

        for chunk in &page_chunks {
            let mut content = String::from("BT\n/F1 11 Tf\n14 TL\n50 790 Td\n");
            for line in *chunk {
                let escaped = line
                    .replace('\\', "\\\\")
                    .replace('(', "\\(")
                    .replace(')', "\\)");
                let ascii_safe: String = escaped
                    .chars()
                    .map(|c| if c.is_ascii() && !c.is_control() { c } else { ' ' })
                    .collect();
                content.push_str(&format!("({}) ' \n", ascii_safe));
            }
            content.push_str("ET\n");

            let mut fonts = Dictionary::new();
            fonts.set("F1", Object::Reference(font_id));

            let mut res_dict = Dictionary::new();
            res_dict.set("Font", Object::Dictionary(fonts));
            let res_id = doc.add_object(Object::Dictionary(res_dict));

            let stream = Stream::new(Dictionary::new(), content.into_bytes());
            let stream_id = doc.add_object(stream);

            let mut page_dict = Dictionary::new();
            page_dict.set("Type", Object::Name("Page".into()));
            page_dict.set("Parent", Object::Reference(pages_id));
            page_dict.set(
                "MediaBox",
                Object::Array(vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(page_w),
                    Object::Real(page_h),
                ]),
            );
            page_dict.set("Resources", Object::Reference(res_id));
            page_dict.set("Contents", Object::Reference(stream_id));

            let page_id = doc.add_object(Object::Dictionary(page_dict));
            kids.push(Object::Reference(page_id));
        }
    }

    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name("Pages".into()));
    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.set("Count", Object::Integer(page_chunks.len() as i64));
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let mut catalog_dict = Dictionary::new();
    catalog_dict.set("Type", Object::Name("Catalog".into()));
    catalog_dict.set("Pages", Object::Reference(pages_id));
    let catalog_id = doc.add_object(Object::Dictionary(catalog_dict));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).map_err(|e| e.to_string())?;
    std::fs::write(output_path, buf).map_err(|e| e.to_string())?;
    Ok(())
}

// ===== PDF REPAIR =====

pub fn repair_pdf(data: &[u8]) -> Result<Vec<u8>, String> {
    // Delegate directly to repair_corrupt_pdf which salvages surviving objects
    // and reconstructs a proper /Type /Catalog and /Type /Pages tree instead of
    // erroneously pointing Root to a Page object.
    super::repair::repair_corrupt_pdf(data)
}

// ===== QUALITY-BASED COMPRESSION =====

pub fn compress_pdf_quality(data: &[u8], quality: u8) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| e.to_string())?;

    // Remove metadata if quality is low
    if quality < 50 {
        doc.trailer.remove(b"Info");
    }

    let comp_level = match quality {
        0..=30 => 9,
        31..=60 => 6,
        61..=85 => 4,
        _ => 1,
    };

    // Rebuild streams:
    // 1. Re-encode and compress DCTDecode (JPEG) and uncompressed image streams according to target quality
    // 2. FlateDecode text/vector streams with target compression level
    for (_, obj) in doc.objects.iter_mut() {
        if let Object::Stream(ref mut stream) = obj {
            let is_image = stream
                .dict
                .get(b"Subtype")
                .map(|s| s == &Object::Name(b"Image".to_vec()))
                .unwrap_or(false);

            if is_image {
                // Image compression handling
                let filter_name = stream
                    .dict
                    .get(b"Filter")
                    .ok()
                    .and_then(|f| f.as_name().ok());
                // Respect ColorSpace: Do not compress or forcibly convert DeviceCMYK / Separation images
                // to RGB as this destroys print prepress color accuracy and corrupts PDF/X compliance.
                let is_cmyk = stream
                    .dict
                    .get(b"ColorSpace")
                    .ok()
                    .and_then(|cs| cs.as_name().ok())
                    .map(|name| name == b"DeviceCMYK" || name == b"Separation")
                    .unwrap_or(false);

                if is_cmyk {
                    continue;
                }

                if filter_name.as_deref() == Some(b"DCTDecode") || filter_name.is_none() {
                    let w = stream
                        .dict
                        .get(b"Width")
                        .and_then(|w| w.as_i64())
                        .unwrap_or(0) as u32;
                    let h = stream
                        .dict
                        .get(b"Height")
                        .and_then(|h| h.as_i64())
                        .unwrap_or(0) as u32;

                    let img_bytes = stream
                        .decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone());

                    let dyn_img_res = if let Ok(img) = image::load_from_memory(&img_bytes) {
                        Ok(img)
                    } else if w > 0 && h > 0 {
                        // Handle raw uncompressed RGB or Grayscale bitmap buffer
                        if img_bytes.len() == (w * h * 3) as usize {
                            image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(w, h, img_bytes)
                                .map(image::DynamicImage::ImageRgb8)
                                .ok_or(())
                        } else if img_bytes.len() == (w * h) as usize {
                            image::ImageBuffer::<image::Luma<u8>, _>::from_raw(w, h, img_bytes)
                                .map(image::DynamicImage::ImageLuma8)
                                .ok_or(())
                        } else {
                            Err(())
                        }
                    } else {
                        Err(())
                    };

                    if let Ok(dyn_img) = dyn_img_res {
                        let (_curr_w, _curr_h) = (dyn_img.width(), dyn_img.height());
                        // Downscale high-resolution images if quality is aggressively low
                        let target_img = if quality <= 30 && (w > 1600 || h > 1600) {
                            dyn_img.resize(w / 2, h / 2, image::imageops::FilterType::Triangle)
                        } else if quality <= 60 && (w > 2400 || h > 2400) {
                            dyn_img.resize((w * 3) / 4, (h * 3) / 4, image::imageops::FilterType::Triangle)
                        } else {
                            dyn_img
                        };

                        let (new_w, new_h) = (target_img.width(), target_img.height());
                        let rgb = target_img.to_rgb8();
                        let mut jpeg_buf = std::io::Cursor::new(Vec::new());
                        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, quality.clamp(10, 95));
                        if encoder.encode(rgb.as_raw(), new_w, new_h, image::ExtendedColorType::Rgb8).is_ok() {
                            let new_jpeg = jpeg_buf.into_inner();
                            if new_jpeg.len() < stream.content.len() || stream.dict.get(b"Filter").is_err() {
                                stream.set_content(new_jpeg);
                                stream.dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
                                stream.dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
                                stream.dict.set("BitsPerComponent", Object::Integer(8));
                                stream.dict.set("Width", Object::Integer(new_w as i64));
                                stream.dict.set("Height", Object::Integer(new_h as i64));
                            }
                        }
                    }
                    continue;
                }
            }

            // FlateDecode streams (text, graphics, metadata)
            let raw_data = if let Ok(filter) = stream.dict.get(b"Filter") {
                if let Ok(filter_name) = filter.as_name() {
                    if filter_name == b"FlateDecode" {
                        if stream.decompress().is_err() {
                            // If decompression fails (corrupted or unsupported predictor), do NOT double-compress
                            continue;
                        }
                        stream.content.clone()
                    } else {
                        // Other filters - keep as is
                        continue;
                    }
                } else {
                    continue;
                }
            } else {
                stream.content.clone()
            };

            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(comp_level));
            if std::io::Write::write_all(&mut encoder, &raw_data).is_ok() {
                if let Ok(compressed) = encoder.finish() {
                    if compressed.len() < stream.content.len()
                        || stream.dict.get(b"Filter").is_err()
                    {
                        stream.set_content(compressed);
                        stream
                            .dict
                            .set("Filter", Object::Name(b"FlateDecode".to_vec()));
                    }
                }
            }
        }
    }

    doc.prune_objects();
    save_doc(&mut doc)
}

// ===== PAGE NUMBERS =====

pub fn add_page_numbers(
    data: &[u8],
    position: &str, // "bottom-center", "top-right", etc.
    font_size: f32,
    start_number: usize,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| e.to_string())?;
    let page_ids = get_page_ids(&doc).clone();

    // Ensure /Helvetica font resource is created and referenced
    let mut font_dict = Dictionary::new();
    font_dict.set("Type", Object::Name(b"Font".to_vec()));
    font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
    font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
    font_dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
    let font_id = doc.add_object(Object::Dictionary(font_dict));

    for (i, &page_id) in page_ids.iter().enumerate() {
        let page_num = start_number + i;
        let (pw, ph) = get_page_dimensions(&doc, page_id);
        let text = format!("{page_num}");

        // Estimate text width for accurate centering and right alignment using character metrics
        let text_width = text.chars().fold(0.0f32, |acc, c| {
            acc + crate::pdf_engine::reflow::get_char_metric_width(c, font_size)
        });

        // Calculate position
        let (x, y) = match position {
            "top-left" => (50.0, ph - 30.0),
            "top-center" => (((pw - text_width) / 2.0).max(10.0), ph - 30.0),
            "top-right" => ((pw - 50.0 - text_width).max(10.0), ph - 30.0),
            "bottom-left" => (50.0, 30.0),
            "bottom-center" => (((pw - text_width) / 2.0).max(10.0), 30.0),
            "bottom-right" => ((pw - 50.0 - text_width).max(10.0), 30.0),
            _ => (((pw - text_width) / 2.0).max(10.0), 30.0),
        };

        // Create standard normal ExtGState to reset any transparency or blend mode from prior content
        let mut normal_gs = Dictionary::new();
        normal_gs.set("Type", Object::Name(b"ExtGState".to_vec()));
        normal_gs.set("CA", Object::Real(1.0)); // stroke alpha = 1.0
        normal_gs.set("ca", Object::Real(1.0)); // fill alpha = 1.0
        normal_gs.set("BM", Object::Name(b"Normal".to_vec())); // Normal blend mode
        let gs_id = doc.add_object(Object::Dictionary(normal_gs));

        // Format content stream:
        // Explicitly isolate graphics state with 'q ... Q', reset color space to DeviceRGB,
        // force black fill (0 0 0 rg), reset line width (1 w), reset ExtGState (/NagisaGS gs),
        // and position text cleanly.
        let new_content = format!(
            " q /NagisaGS gs 0 0 0 rg 0 0 0 RG 1 w [] 0 d BT /NagisaHelv {} Tf {} {} Td ({}) Tj ET Q ",
            font_size, x, y, text
        );

        // Create independent new content stream object
        let mut new_stream = Stream::new(Dictionary::new(), new_content.into_bytes());
        new_stream.dict.set("Type", Object::Name("Content".into()));
        let new_cid = doc.add_object(new_stream);

        // Safely update page resources:
        // If page has its own /Resources (direct or indirect reference), update it in place.
        // If it inherits /Resources from an ancestor (/Pages), create a new local Resources dict
        // cloned from the inherited resources, or update the existing referenced dictionary directly.
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
        fonts_dict.set("NagisaHelv", Object::Reference(font_id));
        resources_dict.set("Font", Object::Dictionary(fonts_dict));

        let mut ext_gstate_dict = match resources_dict.get(b"ExtGState") {
            Ok(Object::Dictionary(gd)) => gd.clone(),
            Ok(Object::Reference(g_ref)) => doc
                .objects
                .get(g_ref)
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        ext_gstate_dict.set("NagisaGS", Object::Reference(gs_id));
        resources_dict.set("ExtGState", Object::Dictionary(ext_gstate_dict));

        // If the page already had an indirect /Resources reference, check if it's unique to this page.
        // Otherwise assign a fresh indirect Resources object to this page to avoid mutating shared ancestor resources.
        let page_has_shared_or_no_resources = match doc.objects.get(&page_id) {
            Some(Object::Dictionary(pd)) => pd.get(b"Resources").is_err(),
            _ => true,
        };

        if page_has_shared_or_no_resources {
            // Inherited from /Pages: create a local indirect Resources object for this page
            let new_res_id = doc.add_object(Object::Dictionary(resources_dict));
            if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
                page_dict.set("Resources", Object::Reference(new_res_id));
            }
        } else {
            // Page has direct or indirect Resources. If indirect, update the existing object
            let res_ref = doc
                .objects
                .get(&page_id)
                .and_then(|o| o.as_dict().ok())
                .and_then(|pd| pd.get(b"Resources").ok())
                .and_then(|r| r.as_reference().ok());

            if let Some(existing_res_id) = res_ref {
                doc.objects.insert(existing_res_id, Object::Dictionary(resources_dict));
            } else if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
                page_dict.set("Resources", Object::Dictionary(resources_dict));
            }
        }

        append_page_content(&mut doc, page_id, new_cid)?;
    }

    save_doc(&mut doc)
}

// ===== EXPORT TO OFFICE & PORTFOLIO (Separated to export_office.rs) =====
pub use super::export_office::*;

// ===== ACTION WIZARD (Record & Replay) =====

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ActionStep {
    pub action_type: String,
    pub params: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ActionWizard {
    pub name: String,
    pub steps: Vec<ActionStep>,
}

pub fn create_action_wizard(name: &str, steps: &[ActionStep]) -> Result<String, String> {
    let wizard = ActionWizard {
        name: name.to_string(),
        steps: steps.to_vec(),
    };
    serde_json::to_string_pretty(&wizard).map_err(|e| e.to_string())
}

pub fn execute_action_wizard(data: &[u8], wizard_json: &str) -> Result<Vec<u8>, String> {
    let wizard: ActionWizard =
        serde_json::from_str(wizard_json).map_err(|e| format!("Invalid wizard JSON: {e}"))?;

    let mut current_data = data.to_vec();

    for step in &wizard.steps {
        match step.action_type.as_str() {
            "add_watermark" => {
                let text = step
                    .params
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let font_size = step
                    .params
                    .get("font_size")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(48.0) as f32;
                let color = step
                    .params
                    .get("color")
                    .and_then(|v| v.as_str())
                    .unwrap_or("#FF0000");
                let opacity = step
                    .params
                    .get("opacity")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.3) as f32;
                let rotation = step
                    .params
                    .get("rotation")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(45.0) as f32;
                current_data = add_watermark(
                    &current_data,
                    text,
                    opacity,
                    rotation,
                    font_size,
                    color,
                    true,
                    &[],
                )?;
            }
            "add_page_numbers" => {
                let position = step
                    .params
                    .get("position")
                    .and_then(|v| v.as_str())
                    .unwrap_or("bottom-center");
                let font_size = step
                    .params
                    .get("font_size")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(12.0) as f32;
                let start = step
                    .params
                    .get("start_number")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as usize;
                current_data = add_page_numbers(&current_data, position, font_size, start)?;
            }
            "optimize" => {
                current_data = optimize_pdf(&current_data)?;
            }
            "flatten_form" => {
                current_data = flatten_form(&current_data)?;
            }
            "remove_metadata" => {
                current_data = remove_metadata(&current_data)?;
            }
            "convert_to_pdfa" => {
                current_data = convert_to_pdfa(&current_data)?;
            }
            "redact_text" => {
                let search = step
                    .params
                    .get("search_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let replacement = step
                    .params
                    .get("replacement")
                    .and_then(|v| v.as_str())
                    .unwrap_or("[REDACTED]");
                if !search.is_empty() {
                    current_data = super::redact::redact_text(&current_data, search, replacement)?;
                }
            }
            "sanitize_document" | "sanitize" => {
                let (sanitized, _) = super::security::sanitize_document(&current_data)?;
                current_data = sanitized;
            }
            "rotate_pages" => {
                let rotation = step
                    .params
                    .get("rotation")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(90) as i32;
                let mut temp_doc = Document::load_mem(&current_data)
                    .map_err(|e| format!("Failed to parse PDF for rotation: {e}"))?;
                let page_count = get_page_count(&temp_doc);
                for p in 0..page_count {
                    super::common::rotate_page_in_doc(&mut temp_doc, p, rotation)?;
                }
                current_data = save_doc(&mut temp_doc)?;
            }
            "binarize" | "deskew" | "enhance_scan" => {
                let options = super::scan_enhance::ScanEnhanceOptions {
                    deskew: true,
                    remove_bleedthrough: true,
                    binarize_text: true,
                    contrast_boost: 1.2,
                };
                current_data = super::scan_enhance::enhance_scanned_pdf(&current_data, &options)?;
            }
            _ => {
                return Err(format!("Unknown action: {}", step.action_type));
            }
        }
    }

    Ok(current_data)
}

// ===== ACCESSIBILITY (Separated to accessibility.rs) =====
pub use super::accessibility::*;

// ===== JAVASCRIPT EMBEDDING (ISO 32000-1 §12.6.4.16 & §7.7.4) =====

pub fn embed_javascript(data: &[u8], script: &str) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    // Create JavaScript action dictionary
    let mut js_action = Dictionary::new();
    js_action.set("S", Object::Name("JavaScript".into()));
    js_action.set(
        "JS",
        Object::String(script.as_bytes().to_vec(), lopdf::StringFormat::Literal),
    );
    let js_action_id = doc.add_object(Object::Dictionary(js_action));

    // Ensure valid Catalog root
    let (root_id, _) = super::page_tree::ensure_catalog_and_pages_root(&mut doc);

    // 1. Standard-compliant Document-Level JavaScript: /Root /Names /JavaScript name tree
    // Create a JavaScript Name Tree leaf node
    let mut js_name_tree = Dictionary::new();
    js_name_tree.set(
        "Names",
        Object::Array(vec![
            Object::String(b"DocLevelJS".to_vec(), lopdf::StringFormat::Literal),
            Object::Reference(js_action_id),
        ]),
    );
    let js_name_tree_id = doc.add_object(Object::Dictionary(js_name_tree));

    // Resolve or create /Names dictionary on Catalog
    let names_id = {
        let root_dict = doc.objects.get(&root_id).and_then(|o| o.as_dict().ok());
        match root_dict.and_then(|d| d.get(b"Names").ok()) {
            Some(Object::Reference(r)) => Some(*r),
            _ => None,
        }
    };

    let target_names_id = if let Some(nid) = names_id {
        if let Some(Object::Dictionary(ref mut names_dict)) = doc.objects.get_mut(&nid) {
            names_dict.set("JavaScript", Object::Reference(js_name_tree_id));
        }
        nid
    } else {
        let mut names_dict = Dictionary::new();
        names_dict.set("JavaScript", Object::Reference(js_name_tree_id));
        doc.add_object(Object::Dictionary(names_dict))
    };

    if let Some(Object::Dictionary(ref mut root_dict)) = doc.objects.get_mut(&root_id) {
        root_dict.set("Names", Object::Reference(target_names_id));

        // 2. If OpenAction does not exist, also set OpenAction for immediate execution upon document open
        // without overriding any existing navigation/zoom OpenAction
        if root_dict.get(b"OpenAction").is_err() {
            root_dict.set("OpenAction", Object::Reference(js_action_id));
        }
    }

    save_doc(&mut doc)
}

// ===== BOOKMARK TREE =====

fn build_outline_nodes(
    doc: &mut Document,
    page_ids: &[OID],
    parent_id: OID,
    items: &[serde_json::Value],
) -> (Vec<OID>, i64) {
    let mut node_ids = Vec::new();
    let mut total_count = 0i64;

    for item in items {
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");
        let page_idx = item.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        if page_idx < page_ids.len() {
            let page_id = page_ids[page_idx];

            let mut item_dict = Dictionary::new();
            item_dict.set(
                "Title",
                Object::String(
                    encode_pdf_text_string(title),
                    lopdf::StringFormat::Literal,
                ),
            );
            item_dict.set("Parent", Object::Reference(parent_id));
            item_dict.set(
                "Dest",
                Object::Array(vec![
                    Object::Reference(page_id),
                    Object::Name("FitH".into()),
                    Object::Real(0.0),
                ]),
            );

            let item_id = doc.add_object(Object::Dictionary(item_dict));
            node_ids.push(item_id);
            total_count += 1;

            // Recursively process children if present
            let children = item
                .get("children")
                .or_else(|| item.get("kids"))
                .and_then(|v| v.as_array());

            if let Some(child_items) = children {
                if !child_items.is_empty() {
                    let (child_ids, child_count) =
                        build_outline_nodes(doc, page_ids, item_id, child_items);
                    total_count += child_count;

                    if let Some(Object::Dictionary(ref mut cur_dict)) = doc.objects.get_mut(&item_id) {
                        cur_dict.set("Count", Object::Integer(child_count));
                        if let Some(first_child) = child_ids.first() {
                            cur_dict.set("First", Object::Reference(*first_child));
                        }
                        if let Some(last_child) = child_ids.last() {
                            cur_dict.set("Last", Object::Reference(*last_child));
                        }
                    }

                    // Link sibling children
                    for i in 0..child_ids.len() {
                        let c_id = child_ids[i];
                        if let Some(Object::Dictionary(ref mut c_dict)) = doc.objects.get_mut(&c_id) {
                            if i > 0 {
                                c_dict.set("Prev", Object::Reference(child_ids[i - 1]));
                            }
                            if i + 1 < child_ids.len() {
                                c_dict.set("Next", Object::Reference(child_ids[i + 1]));
                            }
                        }
                    }
                }
            }
        }
    }

    (node_ids, total_count)
}

pub fn add_bookmark_tree(data: &[u8], bookmarks: &[serde_json::Value]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);

    // Ensure valid Catalog root
    let (root_id, _) = super::page_tree::ensure_catalog_and_pages_root(&mut doc);

    // Allocate outline root dictionary
    let mut outline_dict = Dictionary::new();
    outline_dict.set("Type", Object::Name("Outlines".into()));
    let outline_id = doc.add_object(Object::Dictionary(outline_dict));

    // Recursively build hierarchical outline nodes
    let (top_level_ids, total_count) = build_outline_nodes(&mut doc, &page_ids, outline_id, bookmarks);

    // Link top-level siblings
    for i in 0..top_level_ids.len() {
        let node_id = top_level_ids[i];
        if let Some(Object::Dictionary(ref mut node_dict)) = doc.objects.get_mut(&node_id) {
            if i > 0 {
                node_dict.set("Prev", Object::Reference(top_level_ids[i - 1]));
            }
            if i + 1 < top_level_ids.len() {
                node_dict.set("Next", Object::Reference(top_level_ids[i + 1]));
            }
        }
    }

    // Update Outlines root dictionary
    if let Some(Object::Dictionary(ref mut out_dict)) = doc.objects.get_mut(&outline_id) {
        out_dict.set("Count", Object::Integer(total_count));
        if let Some(first) = top_level_ids.first() {
            out_dict.set("First", Object::Reference(*first));
        }
        if let Some(last) = top_level_ids.last() {
            out_dict.set("Last", Object::Reference(*last));
        }
    }

    if let Some(Object::Dictionary(ref mut root_dict)) = doc.objects.get_mut(&root_id) {
        root_dict.set("Outlines", Object::Reference(outline_id));
    }

    save_doc(&mut doc)
}

// ===== CONTENT COMPARISON & OCR (Separated to ocr_layout.rs) =====
pub use super::ocr_layout::*;
