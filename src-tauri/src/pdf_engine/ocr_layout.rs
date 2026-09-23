use super::common::*;
use super::compare::compare_pdf_documents;
use super::convert::pdf_to_images;
use super::inspect::get_page_text;
use super::text_block_ops::get_text_blocks_from_doc;
use lopdf::Document;
fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        result.push(CHARSET[(b0 >> 2) as usize] as char);
        result.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARSET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARSET[(b2 & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ===== CONTENT COMPARISON (Visual & Semantic Diff) =====

pub fn visual_diff(data1: &[u8], data2: &[u8], output_path: &str) -> Result<(), String> {
    // 1. Run semantic block-level comparison
    let report = compare_pdf_documents(data1, data2)?;

    // 2. Render side-by-side raster images if pdftoppm is available
    let tmp_dir = std::env::temp_dir().join(format!("nagisa_vdiff_{}", std::process::id()));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    let imgs1 = pdf_to_images(data1, &tmp_dir.join("orig").to_string_lossy(), "png", 100).unwrap_or_default();
    let imgs2 = pdf_to_images(data2, &tmp_dir.join("rev").to_string_lossy(), "png", 100).unwrap_or_default();

    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>Nagisa PDF Professional Visual Diff</title>
<style>
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 0; background: #0f172a; color: #f8fafc; padding: 24px; }
.header { display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid #334155; padding-bottom: 16px; margin-bottom: 24px; }
.stats { display: flex; gap: 16px; }
.badge { padding: 6px 12px; border-radius: 6px; font-weight: 600; font-size: 13px; }
.badge-add { background: rgba(34, 197, 94, 0.2); color: #4ade80; border: 1px solid #22c55e; }
.badge-del { background: rgba(239, 68, 68, 0.2); color: #f87171; border: 1px solid #ef4444; }
.badge-mod { background: rgba(234, 179, 8, 0.2); color: #facc15; border: 1px solid #eab308; }
.diff-container { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; margin-bottom: 32px; background: #1e293b; padding: 16px; border-radius: 8px; }
.panel h3 { margin-top: 0; font-size: 14px; color: #94a3b8; }
.panel img { max-width: 100%; height: auto; border: 1px solid #475569; border-radius: 4px; }
.diff-list { margin-top: 16px; font-size: 12px; }
.diff-row { padding: 4px 8px; margin: 4px 0; border-radius: 4px; }
.diff-row.added { background: rgba(34, 197, 94, 0.15); border-left: 3px solid #22c55e; }
.diff-row.deleted { background: rgba(239, 68, 68, 0.15); border-left: 3px solid #ef4444; }
.diff-row.modified { background: rgba(234, 179, 8, 0.15); border-left: 3px solid #eab308; }
</style>
</head>
<body>
<div class="header">
  <h2>Nagisa PDF Visual & Semantic Comparison</h2>
  <div class="stats">
"#,
    );

    html.push_str(&format!(
        r#"    <span class="badge badge-add">+ {} 箇所追加</span>
    <span class="badge badge-del">- {} 箇所削除</span>
    <span class="badge badge-mod">~ {} 箇所変更</span>
  </div>
</div>
"#,
        report.changes_added, report.changes_deleted, report.changes_modified
    ));

    let max_pages = report.total_pages_original.max(report.total_pages_revised);

    for p in 0..max_pages {
        let page_num = p + 1;
        html.push_str(&format!(
            "<h3>ページ {}</h3><div class='diff-container'>",
            page_num
        ));

        // Left Panel (Original)
        html.push_str("<div class='panel'><h3>元ドキュメント</h3>");
        if let Some(img_path) = imgs1.get(p) {
            if let Ok(bytes) = std::fs::read(img_path) {
                let b64 = base64_encode(&bytes);
                html.push_str(&format!("<img src='data:image/png;base64,{}'>", b64));
            }
        } else if p < report.total_pages_original {
            html.push_str("<p style='color:#64748b;'>ページ存在</p>");
        } else {
            html.push_str("<p style='color:#ef4444;'>ページなし（削除）</p>");
        }
        html.push_str("</div>");

        // Right Panel (Revised)
        html.push_str("<div class='panel'><h3>改訂ドキュメント</h3>");
        if let Some(img_path) = imgs2.get(p) {
            if let Ok(bytes) = std::fs::read(img_path) {
                let b64 = base64_encode(&bytes);
                html.push_str(&format!("<img src='data:image/png;base64,{}'>", b64));
            }
        } else if p < report.total_pages_revised {
            html.push_str("<p style='color:#64748b;'>ページ存在</p>");
        } else {
            html.push_str("<p style='color:#ef4444;'>ページなし</p>");
        }
        html.push_str("</div>");

        // Diff summary for this page
        let page_diffs: Vec<&crate::pdf_engine::compare::DiffItem> = report.diffs.iter().filter(|d| d.page == p).collect();
        if !page_diffs.is_empty() {
            html.push_str("<div class='diff-list' style='grid-column: span 2;'><b>差分一覧:</b>");
            for d in page_diffs {
                let escaped_orig = html_escape(&d.original_text);
                let escaped_rev = html_escape(&d.revised_text);
                if d.kind == "modified" {
                    html.push_str(&format!(
                        "<div class='diff-row modified'>変更: <del>{}</del> &rarr; <ins>{}</ins></div>",
                        escaped_orig, escaped_rev
                    ));
                } else if d.kind == "added" {
                    html.push_str(&format!(
                        "<div class='diff-row added'>追加: <ins>{}</ins></div>",
                        escaped_rev
                    ));
                } else {
                    html.push_str(&format!(
                        "<div class='diff-row deleted'>削除: <del>{}</del></div>",
                        escaped_orig
                    ));
                }
            }
            html.push_str("</div>");
        }

        html.push_str("</div>");
    }

    html.push_str("</body></html>");

    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::write(output_path, html).map_err(|e| e.to_string())?;
    Ok(())
}

// ===== ADVANCED OCR WITH TRUE LAYOUT EXTRACTION =====

pub fn ocr_with_layout(
    data: &[u8],
    language: &str,
    preserve_layout: bool,
) -> Result<serde_json::Value, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    let lang = if language.is_empty() { "jpn+eng" } else { language };
    let mut pages = Vec::new();

    // Prepare temporary image cache if any scanned pages need OCR fallback
    let mut scanned_images: Option<Vec<String>> = None;
    let mut tmp_ocr_dir: Option<std::path::PathBuf> = None;

    for (page_idx, &page_id) in page_ids.iter().enumerate() {
        let (w, h) = get_page_dimensions(&doc, page_id);

        let mut extracted_blocks = if let Ok(blocks) = get_text_blocks_from_doc(&doc, page_idx) {
            blocks
        } else {
            Vec::new()
        };

        let mut text = get_page_text(data, page_idx).unwrap_or_default();

        // If page has no stream text (e.g. scanned image / fax), run true Tesseract OCR on rendered page image
        if extracted_blocks.is_empty() && text.trim().is_empty() {
            if scanned_images.is_none() {
                let unique = format!("nagisa_layout_ocr_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
                let dir = std::env::temp_dir().join(unique);
                if let Ok(imgs) = pdf_to_images(data, &dir.to_string_lossy(), "png", 150) {
                    scanned_images = Some(imgs);
                    tmp_ocr_dir = Some(dir);
                }
            }

            if let Some(ref imgs) = scanned_images {
                if page_idx < imgs.len() {
                    let img_path = &imgs[page_idx];
                    if let Ok((ocr_str, _, _, words)) = crate::ocr_engine::run_tesseract(img_path, lang) {
                        text = ocr_str;
                        // Convert OCR word bounding boxes to TextBlock structure
                        if let Ok(img) = image::open(img_path) {
                            let (img_w, img_h) = (img.width() as f32, img.height() as f32);
                            let scale_x = w / img_w.max(1.0);
                            let scale_y = h / img_h.max(1.0);

                            for (w_idx, wb) in words.into_iter().enumerate() {
                                if wb.text.trim().is_empty() {
                                    continue;
                                }
                                let block_x = wb.left * scale_x;
                                let block_w = wb.width * scale_x;
                                let block_h = wb.height * scale_y;
                                let block_y = h - (wb.top + wb.height) * scale_y;

                                extracted_blocks.push(super::text_block_ops::TextBlock {
                                    id: w_idx,
                                    text: wb.text,
                                    x: block_x,
                                    y: block_y,
                                    width: block_w,
                                    height: block_h,
                                    font_size: block_h.max(8.0),
                                    font_name: "OCR".to_string(),
                                    color: "#000000".to_string(),
                                    page_index: page_idx,
                                });
                            }
                        }
                    }
                }
            }
        }

        let layout = if preserve_layout && !extracted_blocks.is_empty() {
            let json_blocks: Vec<serde_json::Value> = extracted_blocks
                .into_iter()
                .map(|b| {
                    serde_json::json!({
                        "text": b.text,
                        "x": b.x,
                        "y": b.y,
                        "width": b.width,
                        "height": b.height,
                        "font_size": b.font_size,
                        "font_name": b.font_name,
                        "color": b.color,
                    })
                })
                .collect();

            Some(serde_json::json!({
                "blocks": json_blocks,
                "count": json_blocks.len(),
            }))
        } else {
            None
        };

        pages.push(serde_json::json!({
            "page": page_idx + 1,
            "width": w,
            "height": h,
            "text": text,
            "layout": layout,
        }));
    }

    if let Some(dir) = tmp_ocr_dir {
        let _ = std::fs::remove_dir_all(&dir);
    }

    Ok(serde_json::json!({
        "pages": pages,
        "total_pages": page_ids.len(),
        "language": lang,
    }))
}

// ===== SEARCHABLE PDF GENERATOR =====

pub fn create_searchable_pdf_from_scanned(data: &[u8], language: &str) -> Result<Vec<u8>, String> {
    let tmp_dir = std::env::temp_dir().join(format!("searchable_pdf_{}", std::process::id()));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    // Render scanned pages to images
    let images = pdf_to_images(data, &tmp_dir.to_string_lossy(), "png", 200)?;
    if images.is_empty() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err("No scanned pages could be rendered for OCR".into());
    }

    // Extract text per page, separated by FormFeed (\x0C) to preserve page boundaries
    let mut page_texts = Vec::new();
    let lang = if language.is_empty() { "jpn+eng" } else { language };

    for img_path in &images {
        let text = match crate::ocr_engine::run_tesseract(img_path, lang) {
            Ok((t, _, _, _)) => t,
            Err(_) => String::new(),
        };
        page_texts.push(text);
    }
    let combined_ocr_text = page_texts.join("\x0C");

    let output_pdf = tmp_dir.join("output_searchable.pdf");
    crate::ocr_engine::create_searchable_pdf(
        &images,
        &combined_ocr_text,
        &output_pdf.to_string_lossy(),
    )?;

    let result_bytes = std::fs::read(&output_pdf).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(result_bytes)
}

