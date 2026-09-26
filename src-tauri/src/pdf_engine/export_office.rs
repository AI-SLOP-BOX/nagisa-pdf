use super::common::*;
use super::*;
use lopdf::{Dictionary, Document, Object, Stream};
use std::io::Write;
use zip::write::SimpleFileOptions;

/// #45 是正: OOXML XML文字列のサニタイザー。
///
/// Office Open XML (ECMA-376) では U+0000〜U+001F の制御文字は \t \n \r を除き禁止。
/// これらが含まれると Microsoft Office / Google Docs が「ファイルが破損しています」エラーを返す。
/// また通常の XML エスケープ (&, <, >, ") も同時に行う。
#[inline]
fn xml_sanitize(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .chars()
        .filter(|&c| c == '\t' || c == '\n' || c == '\r' || c >= '\x20')
        .collect()
}

/// Resolve a caller-supplied 0-based page selection against the document,
/// returning sorted unique in-range indexes (None ⇒ all pages).
fn select_pages(total: usize, page_indexes: Option<&[usize]>) -> Result<Vec<usize>, String> {
    match page_indexes {
        None => Ok((0..total).collect()),
        Some(list) => {
            let mut sel: Vec<usize> = list.iter().copied().filter(|&i| i < total).collect();
            sel.sort_unstable();
            sel.dedup();
            if sel.is_empty() {
                return Err("指定したページ範囲に有効なページがありません".into());
            }
            Ok(sel)
        }
    }
}

/// Render `candidates` (0-based pages with no text layer) and run Tesseract on
/// each image, returning page-index → recognized text. Errors propagate: when
/// the user explicitly asked for OCR, silently emitting nothing would be a lie.
fn ocr_text_for_pages(
    data: &[u8],
    candidates: &[usize],
) -> Result<std::collections::HashMap<usize, String>, String> {
    let mut out = std::collections::HashMap::new();
    if candidates.is_empty() {
        return Ok(out);
    }
    let unique = format!(
        "nagisa_office_ocr_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let tmp = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let run = (|| -> Result<(), String> {
        // candidates must be sorted to align with pdf_to_images_ex's numeric order.
        let imgs = super::convert::pdf_to_images_ex(
            data,
            &tmp.to_string_lossy(),
            "png",
            150,
            Some(candidates),
        )?;
        if imgs.len() != candidates.len() {
            return Err(format!(
                "OCR用ページ画像の生成に失敗しました（{}/{}ページ）。pdftoppm(poppler)が必要です。",
                imgs.len(),
                candidates.len()
            ));
        }
        for (idx, &p) in candidates.iter().enumerate() {
            let (text, _, _, _) = crate::ocr_engine::run_tesseract(&imgs[idx], "jpn+eng")
                .map_err(|e| format!("OCRに失敗しました（{}ページ目）: {e}", p + 1))?;
            if !text.trim().is_empty() {
                out.insert(p, text);
            }
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    run?;
    Ok(out)
}

/// Extract a page's embedded images, re-encoded as PNG. Undecodable image
/// formats (e.g. JPEG2000) are skipped — embedding garbage would corrupt the
/// output document. At most 20 images per page are kept (zip-bomb guard).
fn collect_page_images(doc: &Document, page_id: OID) -> Vec<(Vec<u8>, u32, u32)> {
    let mut out = Vec::new();
    let resources = resolve_page_resources(doc, page_id);
    let xobjs = match resources.get(b"XObject") {
        Ok(Object::Dictionary(d)) => d.clone(),
        _ => return out,
    };
    for (_, obj_ref) in xobjs.iter() {
        if out.len() >= 20 {
            break;
        }
        let stream = match obj_ref {
            Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_stream().ok()),
            Object::Stream(s) => Some(s),
            _ => None,
        };
        let Some(stream) = stream else { continue };
        let is_image = stream
            .dict
            .get(b"Subtype")
            .and_then(|o| o.as_name())
            .map(|n| n == b"Image")
            .unwrap_or(false);
        if !is_image {
            continue;
        }
        let raw = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        let dyn_res = image::load_from_memory(&raw).or_else(|_| {
            let w = stream.dict.get(b"Width").and_then(|v| v.as_i64()).unwrap_or(0) as u32;
            let h = stream
                .dict
                .get(b"Height")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;
            if w > 0 && h > 0 && raw.len() == (w * h * 3) as usize {
                image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(w, h, raw)
                    .map(image::DynamicImage::ImageRgb8)
                    .ok_or(())
            } else {
                Err(())
            }
        });
        if let Ok(img) = dyn_res {
            let mut buf = std::io::Cursor::new(Vec::new());
            if img.write_to(&mut buf, image::ImageFormat::Png).is_ok() {
                out.push((buf.into_inner(), img.width(), img.height()));
            }
        }
    }
    out
}

/// OOXML inline-image drawing paragraph. Extents are EMU (96 DPI), capped to
/// the A4 content width declared in the document's sectPr.
fn docx_drawing_xml(r_id: &str, docpr_id: u32, px_w: u32, px_h: u32) -> String {
    const CONTENT_WIDTH_EMU: u64 = 9026 * 635; // 11906 twips − 2×1440 margins
    let natural_w = px_w as u64 * 9525;
    let natural_h = px_h as u64 * 9525;
    let cx = natural_w.min(CONTENT_WIDTH_EMU).max(9525);
    let cy = if natural_w == 0 {
        natural_h
    } else {
        natural_h * cx / natural_w
    }
    .max(9525);
    format!(
        r#"    <w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="Picture {id}"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="Picture {id}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
"#,
        cx = cx,
        cy = cy,
        id = docpr_id,
        rid = r_id
    )
}

/// Emit a real OOXML table when the page's blocks form a ≥2-column grid
/// (same row/column clustering as pdf_to_excel); None ⇒ the caller falls
/// back to plain paragraphs.
fn build_docx_table(blocks: &[crate::pdf_engine::text_block_ops::TextBlock]) -> Option<String> {
    let mut sorted: Vec<&crate::pdf_engine::text_block_ops::TextBlock> = blocks.iter().collect();
    sorted.sort_by(|a, b| {
        let dy = b.y - a.y;
        if dy.abs() > 4.0 {
            dy.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    let mut rows: Vec<Vec<&crate::pdf_engine::text_block_ops::TextBlock>> = Vec::new();
    for b in &sorted {
        if let Some(last_row) = rows.last_mut() {
            if (last_row[0].y - b.y).abs() <= 5.0 {
                last_row.push(b);
                continue;
            }
        }
        rows.push(vec![b]);
    }
    let mut x_coords: Vec<f32> = sorted.iter().map(|b| b.x).collect();
    x_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut anchors: Vec<f32> = Vec::new();
    for &x in &x_coords {
        if let Some(&last) = anchors.last() {
            if (x - last).abs() < 18.0 {
                continue;
            }
        }
        anchors.push(x);
    }
    if anchors.len() < 2 {
        return None;
    }
    let ncols = anchors.len();
    let col_twips = 9026 / ncols as i64;
    let mut xml = String::from(
        r#"<w:tbl><w:tblPr><w:tblW w:w="9026" w:type="dxa"/><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders></w:tblPr><w:tblGrid>"#,
    );
    for _ in 0..ncols {
        xml.push_str(&format!(r#"<w:gridCol w:w="{col_twips}"/>"#));
    }
    xml.push_str("</w:tblGrid>");
    for row in rows {
        xml.push_str("<w:tr>");
        for (ci, _anchor) in anchors.iter().enumerate() {
            let cell_text = row
                .iter()
                .filter(|b| {
                    let nearest = anchors
                        .iter()
                        .enumerate()
                        .min_by(|(_, &a), (_, &c)| {
                            (b.x - a)
                                .abs()
                                .partial_cmp(&(b.x - c).abs())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    nearest == ci
                })
                .map(|b| b.text.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let last = ci + 1 == ncols;
            let w = if last {
                9026 - col_twips * (ncols as i64 - 1)
            } else {
                col_twips
            };
            xml.push_str(&format!(
                r#"<w:tc><w:tcPr><w:tcW w:w="{w}" w:type="dxa"/></w:tcPr><w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p></w:tc>"#,
                w = w,
                text = xml_sanitize(&cell_text)
            ));
        }
        xml.push_str("</w:tr>");
    }
    xml.push_str("</w:tbl>");
    Some(xml)
}


pub fn pdf_to_word(data: &[u8], output_path: &str) -> Result<(), String> {
    pdf_to_word_ex(data, output_path, None, false, false, false)
}

/// Convert to .docx with optional page range, embedded images (`keep_images`),
/// grid-table reconstruction (`editable_tables`) and OCR of text-less pages.
pub fn pdf_to_word_ex(
    data: &[u8],
    output_path: &str,
    page_indexes: Option<&[usize]>,
    keep_images: bool,
    editable_tables: bool,
    run_ocr: bool,
) -> Result<(), String> {
    let doc = Document::load_mem(data).map_err(|e| e.to_string())?;
    let page_ids = get_page_ids(&doc);
    let selected = select_pages(page_ids.len(), page_indexes)?;

    // OCR pages carrying no text layer at all (only when requested). Failures
    // propagate: silently emitting nothing would misrepresent the conversion.
    let ocr_texts = if run_ocr {
        let mut candidates: Vec<usize> = Vec::new();
        for &i in &selected {
            let has_blocks = crate::pdf_engine::text_block_ops::get_text_blocks_from_doc(&doc, i)
                .map(|b| !b.is_empty())
                .unwrap_or(false);
            if has_blocks {
                continue;
            }
            let has_text = get_page_text(data, i)
                .map(|t| !t.trim().is_empty())
                .unwrap_or(false);
            if !has_text {
                candidates.push(i);
            }
        }
        ocr_text_for_pages(data, &candidates)?
    } else {
        std::collections::HashMap::new()
    };

    // Embedded images are collected during the page loop; media parts and the
    // document relationships file are written after word/document.xml.
    let mut media: Vec<(String, Vec<u8>)> = Vec::new();
    let mut image_rels: Vec<(String, String)> = Vec::new();
    let mut docpr_next: u32 = 1;

    // Build genuine Office Open XML (.docx) ZIP structure
    let file = std::fs::File::create(output_path).map_err(|e| format!("Failed to create output file: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options).map_err(|e| e.to_string())?;
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
    zip.write_all(content_types.as_bytes()).map_err(|e| e.to_string())?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options).map_err(|e| e.to_string())?;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    zip.write_all(rels.as_bytes()).map_err(|e| e.to_string())?;

    // 3. word/document.xml with intelligent paragraph flow reconstruction
    zip.start_file("word/document.xml", options).map_err(|e| e.to_string())?;
    let mut doc_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:body>
"#);

    for (out_pos, &i) in selected.iter().enumerate() {
        // Page header paragraph
        doc_xml.push_str(&format!(
            r#"    <w:p>
      <w:pPr>
        <w:pStyle w:val="Heading2"/>
      </w:pPr>
      <w:r>
        <w:rPr><w:b/></w:rPr>
        <w:t>ページ {}</w:t>
      </w:r>
    </w:p>
"#,
            i + 1
        ));

        // Attempt coordinate-based layout paragraph reconstruction
        let blocks = crate::pdf_engine::text_block_ops::get_text_blocks_from_doc(&doc, i).unwrap_or_default();
        // Editable-table mode: emit a real OOXML grid when ≥2 columns align.
        let table_xml = if editable_tables {
            build_docx_table(&blocks)
        } else {
            None
        };
        if let Some(xml) = &table_xml {
            doc_xml.push_str(xml);
        } else if !blocks.is_empty() {
            // Sort blocks top-to-bottom (PDF y is inverted, larger y is higher up), then left-to-right
            let mut sorted_blocks = blocks;
            sorted_blocks.sort_by(|a, b| {
                let dy = b.y - a.y;
                if dy.abs() > 4.0 {
                    dy.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
                }
            });

            // Smart paragraph clustering
            let mut paragraphs: Vec<Vec<crate::pdf_engine::text_block_ops::TextBlock>> = Vec::new();
            for block in sorted_blocks {
                if let Some(last_para) = paragraphs.last_mut() {
                    let prev_block = last_para.last().unwrap();
                    let line_pitch = prev_block.y - block.y; // Positive if moving downwards
                    let font_size = prev_block.font_size.max(block.font_size).max(10.0);
                    let prev_text = prev_block.text.trim();

                    // Check if previous line ends with sentence terminator
                    let ends_sentence = prev_text.ends_with('。')
                        || prev_text.ends_with('.')
                        || prev_text.ends_with('!')
                        || prev_text.ends_with('?')
                        || prev_text.ends_with(':')
                        || prev_text.ends_with('；')
                        || prev_text.ends_with(';')
                        || prev_text.starts_with('・')
                        || prev_text.starts_with('-')
                        || prev_text.starts_with('*');

                    // If within normal line leading (e.g. 1.0 to 1.7x font size) and not explicit sentence end
                    let is_continuation = line_pitch > 0.0 && line_pitch <= font_size * 1.8 && !ends_sentence;

                    if is_continuation {
                        last_para.push(block);
                        continue;
                    }
                }
                paragraphs.push(vec![block]);
            }

            for para in paragraphs {
                doc_xml.push_str(r#"    <w:p>
"#);
                for (b_idx, b) in para.iter().enumerate() {
                    let trimmed = b.text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let escaped = xml_sanitize(trimmed);

                    let prefix_space = if b_idx > 0 && !trimmed.is_empty() {
                        // Check if ASCII English text requires a joining space
                        let first_char = trimmed.chars().next().unwrap_or(' ');
                        if first_char.is_ascii_alphanumeric() {
                            " "
                        } else {
                            ""
                        }
                    } else {
                        ""
                    };

                    // Compute styling attributes for <w:rPr>
                    let mut r_pr = String::new();
                    let font_name_lower = b.font_name.to_lowercase();
                    if font_name_lower.contains("bold") {
                        r_pr.push_str("        <w:b/>\n");
                    }
                    if font_name_lower.contains("italic") || font_name_lower.contains("oblique") {
                        r_pr.push_str("        <w:i/>\n");
                    }
                    if b.font_size > 0.0 {
                        // Word sz is in half-points (e.g. 12pt = 24 half-points)
                        let half_pts = (b.font_size * 2.0).round().max(2.0) as u32;
                        r_pr.push_str(&format!("        <w:sz w:val=\"{half_pts}\"/>\n"));
                    }
                    let clean_color = b.color.trim_start_matches('#');
                    if clean_color.len() == 6 && clean_color.chars().all(|c| c.is_ascii_hexdigit()) {
                        r_pr.push_str(&format!("        <w:color w:val=\"{clean_color}\"/>\n"));
                    }

                    doc_xml.push_str("      <w:r>\n");
                    if !r_pr.is_empty() {
                        doc_xml.push_str("      <w:rPr>\n");
                        doc_xml.push_str(&r_pr);
                        doc_xml.push_str("      </w:rPr>\n");
                    }
                    doc_xml.push_str(&format!(
                        r#"        <w:t xml:space="preserve">{}{}</w:t>
      </w:r>
"#,
                        prefix_space, escaped
                    ));
                }
                doc_xml.push_str(r#"    </w:p>
"#);
            }
        } else {
            // Text-stream fallback with heuristic line unbreaking; when the page
            // has no text layer either, use the OCR result (run_ocr option).
            let mut page_text = get_page_text(data, i).unwrap_or_default();
            if page_text.trim().is_empty() {
                if let Some(ocr) = ocr_texts.get(&i) {
                    page_text = ocr.clone();
                }
            }
            if !page_text.trim().is_empty() {
            let mut current_para = String::new();
            for line in page_text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    if !current_para.is_empty() {
                        let escaped = xml_sanitize(&current_para);
                        doc_xml.push_str(&format!(
                            r#"    <w:p>
      <w:r>
        <w:t xml:space="preserve">{}</w:t>
      </w:r>
    </w:p>
"#,
                            escaped
                        ));
                        current_para.clear();
                    }
                    continue;
                }

                if current_para.is_empty() {
                    current_para.push_str(trimmed);
                } else {
                    let ends_sentence = current_para.ends_with('。')
                        || current_para.ends_with('.')
                        || current_para.ends_with('!')
                        || current_para.ends_with('?')
                        || current_para.ends_with(':');

                    if ends_sentence {
                        let escaped = xml_sanitize(&current_para);
                        doc_xml.push_str(&format!(
                            r#"    <w:p>
      <w:r>
        <w:t xml:space="preserve">{}</w:t>
      </w:r>
    </w:p>
"#,
                            escaped
                        ));
                        current_para = trimmed.to_string();
                    } else {
                        if current_para.chars().last().map(|c| c.is_ascii()).unwrap_or(false)
                            && trimmed.chars().next().map(|c| c.is_ascii()).unwrap_or(false)
                        {
                            current_para.push(' ');
                        }
                        current_para.push_str(trimmed);
                    }
                }
            }

            if !current_para.is_empty() {
                let escaped = xml_sanitize(&current_para);
                doc_xml.push_str(&format!(
                    r#"    <w:p>
      <w:r>
        <w:t xml:space="preserve">{}</w:t>
      </w:r>
    </w:p>
"#,
                    escaped
                ));
            }
            }
        }

        // keep_images: embed this page's extracted images inline.
        if keep_images {
            let page_id = page_ids[i];
            for (png, w, h) in collect_page_images(&doc, page_id) {
                let n = media.len() + 1;
                let zip_name = format!("image{n}.png");
                let r_id = format!("rIdImg{n}");
                media.push((zip_name.clone(), png));
                image_rels.push((r_id.clone(), zip_name));
                doc_xml.push_str(&docx_drawing_xml(&r_id, docpr_next, w, h));
                docpr_next += 1;
            }
        }

        // Add page break between pages
        if out_pos + 1 < selected.len() {
            doc_xml.push_str(r#"    <w:p><w:r><w:br w:type="page"/></w:r></w:p>
"#);
        }
    }

    doc_xml.push_str(r#"    <w:sectPr>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#);

    zip.write_all(doc_xml.as_bytes()).map_err(|e| e.to_string())?;

    // 4. word/_rels/document.xml.rels + word/media/* (only when images exist)
    if !image_rels.is_empty() {
        zip.start_file("word/_rels/document.xml.rels", options)
            .map_err(|e| e.to_string())?;
        let mut img_rels_xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
"#,
        );
        for (rid, target) in &image_rels {
            img_rels_xml.push_str(&format!(
                r#"  <Relationship Id="{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{target}"/>
"#
            ));
        }
        img_rels_xml.push_str("</Relationships>");
        zip.write_all(img_rels_xml.as_bytes())
            .map_err(|e| e.to_string())?;

        for (name, bytes) in &media {
            zip.start_file(format!("word/media/{name}"), options)
                .map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
        }
    }

    zip.finish().map_err(|e| format!("Failed to finalize docx zip archive: {e}"))?;
    Ok(())
}

pub fn pdf_to_excel(data: &[u8], output_path: &str) -> Result<(), String> {
    pdf_to_excel_ex(data, output_path, None, true, false)
}

/// Convert to .xlsx. `editable_tables` keeps the column-grid reconstruction;
/// when false every text line becomes a single cell in column A. `run_ocr`
/// OCRs pages that have no text layer at all.
pub fn pdf_to_excel_ex(
    data: &[u8],
    output_path: &str,
    page_indexes: Option<&[usize]>,
    editable_tables: bool,
    run_ocr: bool,
) -> Result<(), String> {
    let doc = Document::load_mem(data).map_err(|e| e.to_string())?;
    let page_ids = get_page_ids(&doc);
    let selected = select_pages(page_ids.len(), page_indexes)?;

    // OCR pages with no text layer (only when requested; failures propagate).
    let ocr_texts = if run_ocr {
        let mut candidates: Vec<usize> = Vec::new();
        for &i in &selected {
            let has_blocks = crate::pdf_engine::text_block_ops::get_text_blocks_from_doc(&doc, i)
                .map(|b| !b.is_empty())
                .unwrap_or(false);
            if has_blocks {
                continue;
            }
            let has_text = get_page_text(data, i)
                .map(|t| !t.trim().is_empty())
                .unwrap_or(false);
            if !has_text {
                candidates.push(i);
            }
        }
        ocr_text_for_pages(data, &candidates)?
    } else {
        std::collections::HashMap::new()
    };

    // Build genuine Office Open XML (.xlsx) ZIP structure
    let file = std::fs::File::create(output_path).map_err(|e| format!("Failed to create output file: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options).map_err(|e| e.to_string())?;
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#;
    zip.write_all(content_types.as_bytes()).map_err(|e| e.to_string())?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options).map_err(|e| e.to_string())?;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#;
    zip.write_all(rels.as_bytes()).map_err(|e| e.to_string())?;

    // 3. xl/_rels/workbook.xml.rels
    zip.start_file("xl/_rels/workbook.xml.rels", options).map_err(|e| e.to_string())?;
    let wb_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#;
    zip.write_all(wb_rels.as_bytes()).map_err(|e| e.to_string())?;

    // 4. xl/workbook.xml
    zip.start_file("xl/workbook.xml", options).map_err(|e| e.to_string())?;
    let workbook = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#;
    zip.write_all(workbook.as_bytes()).map_err(|e| e.to_string())?;

    // 5. xl/worksheets/sheet1.xml
    zip.start_file("xl/worksheets/sheet1.xml", options).map_err(|e| e.to_string())?;
    let mut sheet_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
"#);

    let mut current_row_idx = 1;

    for &i in &selected {
        let blocks = crate::pdf_engine::text_block_ops::get_text_blocks_from_doc(&doc, i).unwrap_or_default();
        if editable_tables && !blocks.is_empty() {
            // Cluster text blocks into rows by Y coordinate with threshold
            let mut sorted = blocks;
            sorted.sort_by(|a, b| {
                let dy = b.y - a.y;
                if dy.abs() > 4.0 {
                    dy.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
                }
            });

            // Group into visual rows
            let mut visual_rows: Vec<Vec<crate::pdf_engine::text_block_ops::TextBlock>> = Vec::new();
            for block in sorted {
                if let Some(last_row) = visual_rows.last_mut() {
                    let first_in_row = &last_row[0];
                    if (first_in_row.y - block.y).abs() <= 5.0 {
                        last_row.push(block);
                        continue;
                    }
                }
                visual_rows.push(vec![block]);
            }

            // Detect column alignment gutters across rows
            let mut x_coords: Vec<f32> = visual_rows.iter().flat_map(|r| r.iter().map(|b| b.x)).collect();
            x_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            
            // Cluster X coordinates into distinct columns
            let mut column_anchors: Vec<f32> = Vec::new();
            for &x in &x_coords {
                if let Some(&last_anchor) = column_anchors.last() {
                    if (x - last_anchor).abs() < 18.0 {
                        continue; // Same column band
                    }
                }
                column_anchors.push(x);
            }

            for row in visual_rows {
                sheet_xml.push_str(&format!(r#"    <row r="{}">"#, current_row_idx));

                // Map blocks in this row to column indices
                let mut col_map: std::collections::BTreeMap<usize, String> = std::collections::BTreeMap::new();
                for b in row {
                    let col_idx = column_anchors
                        .iter()
                        .enumerate()
                        .min_by(|(_, &a), (_, &c)| {
                            (b.x - a).abs().partial_cmp(&(b.x - c).abs()).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(idx, _)| idx)
                        .unwrap_or(0);

                    let entry = col_map.entry(col_idx).or_default();
                    if !entry.is_empty() {
                        entry.push(' ');
                    }
                    entry.push_str(b.text.trim());
                }

                for (&col_idx, text) in &col_map {
                    if text.is_empty() {
                        continue;
                    }
                    let col_letter = if col_idx < 26 {
                        ((b'A' + col_idx as u8) as char).to_string()
                    } else {
                        format!("A{}", ((b'A' + (col_idx - 26) as u8) as char))
                    };
                    let cell_ref = format!("{}{}", col_letter, current_row_idx);
                    let escaped = xml_sanitize(text);

                    sheet_xml.push_str(&format!(
                        r#"<c r="{}" t="inlineStr"><is><t>{}</t></is></c>"#,
                        cell_ref, escaped
                    ));
                }

                sheet_xml.push_str("</row>\n");
                current_row_idx += 1;
            }
        } else {
            // Tab / comma / multi-space fallback; when the page has no text layer
            // either, use the OCR result (run_ocr option). editable_tables=false
            // forces a raw one-line-per-row dump into column A.
            let mut page_text = get_page_text(data, i).unwrap_or_default();
            if page_text.trim().is_empty() {
                if let Some(ocr) = ocr_texts.get(&i) {
                    page_text = ocr.clone();
                }
            }
            if page_text.trim().is_empty() && !blocks.is_empty() {
                let mut sorted: Vec<&crate::pdf_engine::text_block_ops::TextBlock> = blocks.iter().collect();
                // No stream text at all: fall back to block texts in reading order.
                sorted.sort_by(|a, b| {
                    let dy = b.y - a.y;
                    if dy.abs() > 4.0 {
                        dy.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
                    }
                });
                page_text = sorted
                    .iter()
                    .map(|b| b.text.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            for line in page_text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // Intelligently parse multiple spaces or tabs as column delimiters
                let cols: Vec<&str> = if !editable_tables {
                    vec![trimmed]
                } else if trimmed.contains('\t') {
                    trimmed.split('\t').map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
                } else if trimmed.contains("  ") {
                    trimmed.split_whitespace().collect()
                } else if trimmed.contains(',') {
                    trimmed.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
                } else {
                    vec![trimmed]
                };

                sheet_xml.push_str(&format!(r#"    <row r="{}">"#, current_row_idx));

                for (col_idx, &val) in cols.iter().enumerate() {
                    let col_letter = if col_idx < 26 {
                        ((b'A' + col_idx as u8) as char).to_string()
                    } else {
                        format!("A{}", ((b'A' + (col_idx - 26) as u8) as char))
                    };
                    let cell_ref = format!("{}{}", col_letter, current_row_idx);
                    let escaped = xml_sanitize(val);

                    sheet_xml.push_str(&format!(
                        r#"<c r="{}" t="inlineStr"><is><t>{}</t></is></c>"#,
                        cell_ref, escaped
                    ));
                }

                sheet_xml.push_str("</row>\n");
                current_row_idx += 1;
            }
        }
    }

    sheet_xml.push_str(r#"  </sheetData>
</worksheet>"#);

    zip.write_all(sheet_xml.as_bytes()).map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| format!("Failed to finalize xlsx zip archive: {e}"))?;
    Ok(())
}

pub fn pdf_to_powerpoint(data: &[u8], output_path: &str) -> Result<(), String> {
    pdf_to_powerpoint_ex(data, output_path, None, false)
}

/// Convert to .pptx. `page_indexes` selects which pages become slides; `run_ocr`
/// OCRs text-less pages so their notes/overlay text is not silently empty.
pub fn pdf_to_powerpoint_ex(
    data: &[u8],
    output_path: &str,
    page_indexes: Option<&[usize]>,
    run_ocr: bool,
) -> Result<(), String> {
    let doc = Document::load_mem(data).map_err(|e| e.to_string())?;
    let page_ids = get_page_ids(&doc);

    if page_ids.is_empty() {
        return Err("Cannot convert empty PDF to PowerPoint".into());
    }

    // Render high-res slide backdrop images (PNG)
    let tmp = std::env::temp_dir().join(format!(
        "nagisa_pdf2ppt_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let selected = select_pages(page_ids.len(), page_indexes)?;

    // OCR text-less pages so slide notes are not silently empty (run_ocr only).
    let ocr_texts = if run_ocr {
        let mut candidates: Vec<usize> = Vec::new();
        for &i in &selected {
            let has_text = get_page_text(data, i)
                .map(|t| !t.trim().is_empty())
                .unwrap_or(false);
            if !has_text {
                candidates.push(i);
            }
        }
        ocr_text_for_pages(data, &candidates)?
    } else {
        std::collections::HashMap::new()
    };

    let rendered_images =
        super::convert::pdf_to_images_ex(data, &tmp.to_string_lossy(), "png", 150, Some(&selected))
            .unwrap_or_default();

    let file = std::fs::File::create(output_path)
        .map_err(|e| format!("Failed to create output file: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    let num_slides = selected.len();

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options).map_err(|e| e.to_string())?;
    let mut content_types = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
"#);
    for i in 1..=num_slides {
        content_types.push_str(&format!(
            r#"  <Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
"#
        ));
    }
    content_types.push_str("</Types>");
    zip.write_all(content_types.as_bytes()).map_err(|e| e.to_string())?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options).map_err(|e| e.to_string())?;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#;
    zip.write_all(rels.as_bytes()).map_err(|e| e.to_string())?;

    // 3. ppt/_rels/presentation.xml.rels
    zip.start_file("ppt/_rels/presentation.xml.rels", options).map_err(|e| e.to_string())?;
    let mut pres_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
"#);
    for i in 1..=num_slides {
        pres_rels.push_str(&format!(
            r#"  <Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>
"#
        ));
    }
    pres_rels.push_str("</Relationships>");
    zip.write_all(pres_rels.as_bytes()).map_err(|e| e.to_string())?;

    // 4. ppt/presentation.xml (Standard 16:9 widescreen 12192000 x 6858000 EMUs)
    zip.start_file("ppt/presentation.xml", options).map_err(|e| e.to_string())?;
    let mut pres_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst/>
  <p:sldIdLst>
"#);
    for i in 1..=num_slides {
        let sld_id = 255 + i;
        pres_xml.push_str(&format!(
            r#"    <p:sldId id="{sld_id}" r:id="rId{i}"/>
"#
        ));
    }
    pres_xml.push_str(r#"  </p:sldIdLst>
  <p:sldSz cx="12192000" cy="6858000" type="screen16x9"/>
  <p:notesSz cx="6858000" cy="12192000"/>
</p:presentation>"#);
    zip.write_all(pres_xml.as_bytes()).map_err(|e| e.to_string())?;

    // 5. Slides and media
    for (slide_idx, &orig_page) in selected.iter().enumerate() {
        let slide_num = slide_idx + 1;
        let img_filename = format!("image{slide_num}.png");
        let has_image = rendered_images.get(slide_idx).is_some();

        if let Some(img_path) = rendered_images.get(slide_idx) {
            if let Ok(img_bytes) = std::fs::read(img_path) {
                zip.start_file(format!("ppt/media/{img_filename}"), options).map_err(|e| e.to_string())?;
                zip.write_all(&img_bytes).map_err(|e| e.to_string())?;
            }
        }

        // slide relationships
        zip.start_file(format!("ppt/slides/_rels/slide{slide_num}.xml.rels"), options).map_err(|e| e.to_string())?;
        let mut slide_rel = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
"#);
        if has_image {
            slide_rel.push_str(&format!(
                r#"  <Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/{img_filename}"/>
"#
            ));
        }
        slide_rel.push_str("</Relationships>");
        zip.write_all(slide_rel.as_bytes()).map_err(|e| e.to_string())?;

        // Extract text for notes/searchable overlay
        let mut page_text = get_page_text(data, orig_page).unwrap_or_default();
        if page_text.trim().is_empty() {
            if let Some(ocr) = ocr_texts.get(&orig_page) {
                page_text = ocr.clone();
            }
        }
        let mut text_paragraphs = String::new();
        for line in page_text.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let escaped = xml_sanitize(trimmed);
                text_paragraphs.push_str(&format!(
                    r#"        <a:p>
          <a:r>
            <a:rPr lang="ja-JP" sz="1400"/>
            <a:t>{}</a:t>
          </a:r>
        </a:p>
"#,
                    escaped
                ));
            }
        }

        // slide{N}.xml
        zip.start_file(format!("ppt/slides/slide{slide_num}.xml"), options).map_err(|e| e.to_string())?;
        let mut slide_xml = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
"#);

        // Backdrop picture shape if rendered image is present
        if has_image {
            slide_xml.push_str(r#"      <p:pic>
        <p:nvPicPr>
          <p:cNvPr id="2" name="Page Visual"/>
          <p:cNvPicPr>
            <a:picLocks noChangeAspect="1"/>
          </p:cNvPicPr>
          <p:nvPr/>
        </p:nvPicPr>
        <p:blipFill>
          <a:blip r:embed="rIdImg"/>
          <a:stretch>
            <a:fillRect/>
          </a:stretch>
        </p:blipFill>
        <p:spPr>
          <a:xfrm>
            <a:off x="0" y="0"/>
            <a:ext cx="12192000" cy="6858000"/>
          </a:xfrm>
          <a:prstGeom prst="rect">
            <a:avLst/>
          </a:prstGeom>
        </p:spPr>
      </p:pic>
"#);
        }

        // Text shape
        if !text_paragraphs.is_empty() {
            slide_xml.push_str(&format!(
                r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Extracted Content"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="457200" y="457200"/>
            <a:ext cx="11277600" cy="5943600"/>
          </a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:solidFill>
            <a:srgbClr val="FFFFFF">
              <a:alpha val="75000"/>
            </a:srgbClr>
          </a:solidFill>
        </p:spPr>
        <p:txBody>
          <a:bodyPr wrap="square" rtlCol="0"/>
          <a:lstStyle/>
{text_paragraphs}        </p:txBody>
      </p:sp>
"#
            ));
        }

        slide_xml.push_str(r#"    </p:spTree>
  </p:cSld>
</p:sld>"#);
        zip.write_all(slide_xml.as_bytes()).map_err(|e| e.to_string())?;
    }

    let _ = std::fs::remove_dir_all(&tmp);
    zip.finish().map_err(|e| format!("Failed to finalize pptx zip archive: {e}"))?;
    Ok(())
}

pub fn create_pdf_portfolio(file_paths: &[String], output_path: &str) -> Result<(), String> {
    let mut doc = Document::with_version("1.7");
    let mut files = Vec::new();

    for path in file_paths {
        let path_obj = std::path::Path::new(path);
        let filename = path_obj
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());

        let file_data = std::fs::read(path).map_err(|e| format!("Failed to read {path}: {e}"))?;
        let file_size = file_data.len() as i64;

        let mut embed_dict = Dictionary::new();
        embed_dict.set("Type", Object::Name("EmbeddedFile".into()));
        embed_dict.set("Size", Object::Integer(file_size));

        let embed_stream = Stream::new(embed_dict, file_data);
        let embed_id = doc.add_object(Object::Stream(embed_stream));

        let mut fs_dict = Dictionary::new();
        fs_dict.set("Type", Object::Name("Filespec".into()));
        fs_dict.set(
            "F",
            Object::String(filename.clone().into_bytes(), lopdf::StringFormat::Literal),
        );
        fs_dict.set(
            "UF",
            Object::String(filename.into_bytes(), lopdf::StringFormat::Literal),
        );
        fs_dict.set(
            "EF",
            Object::Dictionary({
                let mut ef = Dictionary::new();
                ef.set("F", Object::Reference(embed_id));
                ef
            }),
        );

        let fs_id = doc.add_object(Object::Dictionary(fs_dict));
        files.push(Object::Reference(fs_id));
    }

    let mut collection_dict = Dictionary::new();
    collection_dict.set("Type", Object::Name("Collection".into()));
    collection_dict.set("View", Object::Name("Detail".into()));
    collection_dict.set("Sort", Object::Name("Name".into()));
    collection_dict.set(
        "Title",
        Object::String(
            "PDF Portfolio".as_bytes().to_vec(),
            lopdf::StringFormat::Literal,
        ),
    );

    let collection_id = doc.add_object(Object::Dictionary(collection_dict));

    // EmbeddedFiles Names tree leaf node:
    // /Names [ (filename1) ref1 (filename2) ref2 ... ]
    let mut names_array = Vec::new();
    for (i, path) in file_paths.iter().enumerate() {
        let filename = std::path::Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("file_{i}"));
        names_array.push(Object::String(
            filename.into_bytes(),
            lopdf::StringFormat::Literal,
        ));
        if let Some(Object::Reference(ref_id)) = files.get(i) {
            names_array.push(Object::Reference(*ref_id));
        }
    }

    let mut ef_tree_node = Dictionary::new();
    ef_tree_node.set("Names", Object::Array(names_array));
    let ef_tree_id = doc.add_object(Object::Dictionary(ef_tree_node));

    // Catalog Names dictionary:
    // << /EmbeddedFiles ef_tree_id >>
    let mut names_dict = Dictionary::new();
    names_dict.set("EmbeddedFiles", Object::Reference(ef_tree_id));
    let names_id = doc.add_object(Object::Dictionary(names_dict));

    // Standard cover page so the PDF has valid /Pages structure
    let pages_id = doc.new_object_id();

    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name("Page".into()));
    page_dict.set("Parent", Object::Reference(pages_id));
    page_dict.set(
        "MediaBox",
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(595.0),
            Object::Real(842.0),
        ]),
    );

    let cover_text = "q 1 0 0 1 50 750 cm BT /F1 16 Tf (Nagisa PDF Portfolio) Tj ET Q";
    let cover_content_id = doc.add_object(Stream::new(
        Dictionary::new(),
        cover_text.as_bytes().to_vec(),
    ));
    page_dict.set("Contents", Object::Reference(cover_content_id));

    let mut font_dict = Dictionary::new();
    font_dict.set("Type", Object::Name("Font".into()));
    font_dict.set("Subtype", Object::Name("Type1".into()));
    font_dict.set("BaseFont", Object::Name("Helvetica".into()));
    let font_id = doc.add_object(Object::Dictionary(font_dict));

    let mut fonts_res = Dictionary::new();
    fonts_res.set("F1", Object::Reference(font_id));
    let mut res_dict = Dictionary::new();
    res_dict.set("Font", Object::Dictionary(fonts_res));
    page_dict.set("Resources", Object::Dictionary(res_dict));

    let page_id = doc.add_object(Object::Dictionary(page_dict));

    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name("Pages".into()));
    pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
    pages_dict.set("Count", Object::Integer(1));
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Valid Document Catalog
    let mut catalog_dict = Dictionary::new();
    catalog_dict.set("Type", Object::Name("Catalog".into()));
    catalog_dict.set("Pages", Object::Reference(pages_id));
    catalog_dict.set("Names", Object::Reference(names_id));
    catalog_dict.set("Collection", Object::Reference(collection_id));

    let catalog_id = doc.add_object(Object::Dictionary(catalog_dict));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).map_err(|e| e.to_string())?;
    std::fs::write(output_path, buf).map_err(|e| e.to_string())?;
    Ok(())
}
