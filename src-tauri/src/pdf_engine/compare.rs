use super::common::*;
use lopdf::{Document, Object};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DiffItem {
    pub page: usize,
    pub kind: String, // "added" | "deleted" | "modified"
    pub original_text: String,
    pub revised_text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct CompareReport {
    pub total_pages_original: usize,
    pub total_pages_revised: usize,
    pub total_changes: usize,
    pub changes_added: usize,
    pub changes_deleted: usize,
    pub changes_modified: usize,
    pub diffs: Vec<DiffItem>,
}

/// Professional semantic & graphical document comparison.
/// Compares text blocks across pages and detects changes, additions, and deletions.
pub fn compare_pdf_documents(original: &[u8], revised: &[u8]) -> Result<CompareReport, String> {
    let doc_orig =
        Document::load_mem(original).map_err(|e| format!("Failed to parse original PDF: {e}"))?;
    let doc_rev =
        Document::load_mem(revised).map_err(|e| format!("Failed to parse revised PDF: {e}"))?;

    let orig_pages = get_page_ids(&doc_orig);
    let rev_pages = get_page_ids(&doc_rev);

    let max_pages = orig_pages.len().max(rev_pages.len());
    let mut diffs = Vec::new();
    let mut added_count = 0;
    let mut deleted_count = 0;
    let mut modified_count = 0;

    for p in 0..max_pages {
        let text_orig = if p < orig_pages.len() {
            extract_page_text(&doc_orig, orig_pages[p])
        } else {
            Vec::new()
        };

        let text_rev = if p < rev_pages.len() {
            extract_page_text(&doc_rev, rev_pages[p])
        } else {
            Vec::new()
        };

        // Compute block-level differences
        let mut rev_matched = vec![false; text_rev.len()];

        for orig_item in &text_orig {
            let mut found = false;
            for (rj, rev_item) in text_rev.iter().enumerate() {
                if !rev_matched[rj] && orig_item.text.trim() == rev_item.text.trim() {
                    rev_matched[rj] = true;
                    found = true;
                    break;
                }
            }

            if !found {
                // Check if it was genuinely modified (must have close 2D position AND text similarity)
                let mut best_match: Option<(usize, f32)> = None;
                for (rj, rev_item) in text_rev.iter().enumerate() {
                    if rev_matched[rj] {
                        continue;
                    }
                    let dx = (orig_item.x - rev_item.x).abs();
                    let dy = (orig_item.y - rev_item.y).abs();

                    // Must be in close 2D proximity on the page (within 24pt X and 12pt Y)
                    if dx < 40.0 && dy < 12.0 {
                        // Calculate character-level Levenshtein similarity
                        let sim = text_similarity(&orig_item.text, &rev_item.text);
                        // At least 30% text similarity or overlapping bounding box to consider a modification
                        if sim > 0.3 || (dx < 10.0 && dy < 6.0) {
                            if best_match.as_ref().map_or(true, |m| sim > m.1) {
                                best_match = Some((rj, sim));
                            }
                        }
                    }
                }

                if let Some((rj, _)) = best_match {
                    let rev_item = &text_rev[rj];
                    diffs.push(DiffItem {
                        page: p,
                        kind: "modified".into(),
                        original_text: orig_item.text.clone(),
                        revised_text: rev_item.text.clone(),
                        x: rev_item.x,
                        y: rev_item.y,
                        width: rev_item.width.max(orig_item.width),
                        height: rev_item.height.max(orig_item.height),
                    });
                    rev_matched[rj] = true;
                    modified_count += 1;
                } else {
                    diffs.push(DiffItem {
                        page: p,
                        kind: "deleted".into(),
                        original_text: orig_item.text.clone(),
                        revised_text: String::new(),
                        x: orig_item.x,
                        y: orig_item.y,
                        width: orig_item.width,
                        height: orig_item.height,
                    });
                    deleted_count += 1;
                }
            }
        }

        // Remaining unmatched in rev are added
        for (rj, rev_item) in text_rev.iter().enumerate() {
            if !rev_matched[rj] {
                diffs.push(DiffItem {
                    page: p,
                    kind: "added".into(),
                    original_text: String::new(),
                    revised_text: rev_item.text.clone(),
                    x: rev_item.x,
                    y: rev_item.y,
                    width: rev_item.width,
                    height: rev_item.height,
                });
                added_count += 1;
            }
        }
    }

    let total = added_count + deleted_count + modified_count;

    Ok(CompareReport {
        total_pages_original: orig_pages.len(),
        total_pages_revised: rev_pages.len(),
        total_changes: total,
        changes_added: added_count,
        changes_deleted: deleted_count,
        changes_modified: modified_count,
        diffs,
    })
}

struct SimpleTextBlock {
    text: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn extract_page_text(doc: &Document, page_id: OID) -> Vec<SimpleTextBlock> {
    let mut blocks = Vec::new();

    // Collect all content stream IDs (supporting single Reference, single Stream, or Array of streams)
    let mut content_ids: Vec<OID> = Vec::new();
    if let Some(obj) = doc.objects.get(&page_id) {
        if let Ok(d) = obj.as_dict() {
            match d.get(b"Contents") {
                Ok(Object::Reference(cid)) => content_ids.push(*cid),
                Ok(Object::Array(arr)) => {
                    for item in arr {
                        if let Ok(cid) = item.as_reference() {
                            content_ids.push(cid);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut current_x = 0.0f32;
    let mut current_y = 0.0f32;
    let mut current_font_size = 12.0f32;

    for cid in content_ids {
        if let Some(obj) = doc.objects.get(&cid) {
            if let Ok(stream) = obj.as_stream() {
                let stream_bytes = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());

                if let Ok(content) = lopdf::content::Content::decode(&stream_bytes) {
                    for op in content.operations {
                        match op.operator.as_str() {
                            "Tf" => {
                                if let Some(size) = op.operands.get(1) {
                                    if let Ok(sz) = size.as_float() {
                                        if sz > 0.0 {
                                            current_font_size = sz;
                                        }
                                    }
                                }
                            }
                            "Td" | "TD" => {
                                if op.operands.len() >= 2 {
                                    if let (Ok(dx), Ok(dy)) =
                                        (op.operands[0].as_float(), op.operands[1].as_float())
                                    {
                                        current_x += dx;
                                        current_y += dy;
                                    }
                                }
                            }
                            "Tm" => {
                                if op.operands.len() >= 6 {
                                    if let (Ok(tx), Ok(ty)) =
                                        (op.operands[4].as_float(), op.operands[5].as_float())
                                    {
                                        current_x = tx;
                                        current_y = ty;
                                    }
                                }
                            }
                            "Tj" => {
                                if let Some(s) = op.operands.first().and_then(|o| match o {
                                    Object::String(bytes, _) => {
                                        Some(super::common::decode_pdf_text_string(bytes))
                                    }
                                    _ => None,
                                }) {
                                    let calc_width: f32 = s
                                        .chars()
                                        .map(|c| super::reflow::get_char_metric_width(c, current_font_size))
                                        .sum();
                                    blocks.push(SimpleTextBlock {
                                        text: s,
                                        x: current_x,
                                        y: current_y,
                                        width: calc_width.max(10.0),
                                        height: current_font_size * 1.2,
                                    });
                                }
                            }
                            "TJ" => {
                                if let Some(Object::Array(arr)) = op.operands.first() {
                                    let mut line_buf = String::new();
                                    for item in arr {
                                        if let Object::String(bytes, _) = item {
                                            line_buf.push_str(&super::common::decode_pdf_text_string(bytes));
                                        }
                                    }
                                    if !line_buf.is_empty() {
                                        let calc_width: f32 = line_buf
                                            .chars()
                                            .map(|c| super::reflow::get_char_metric_width(c, current_font_size))
                                            .sum();
                                        blocks.push(SimpleTextBlock {
                                            text: line_buf,
                                            x: current_x,
                                            y: current_y,
                                            width: calc_width.max(10.0),
                                            height: current_font_size * 1.2,
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    blocks
}

fn text_similarity(s1: &str, s2: &str) -> f32 {
    let s1 = s1.trim();
    let s2 = s2.trim();
    if s1 == s2 {
        return 1.0;
    }
    if s1.is_empty() || s2.is_empty() {
        return 0.0;
    }

    let c1: Vec<char> = s1.chars().collect();
    let c2: Vec<char> = s2.chars().collect();
    let len1 = c1.len();
    let len2 = c2.len();

    let mut dp = vec![vec![0usize; len2 + 1]; len1 + 1];
    for i in 0..=len1 {
        dp[i][0] = i;
    }
    for j in 0..=len2 {
        dp[0][j] = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if c1[i - 1] == c2[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    let dist = dp[len1][len2] as f32;
    let max_len = (len1.max(len2)) as f32;
    1.0 - (dist / max_len)
}
