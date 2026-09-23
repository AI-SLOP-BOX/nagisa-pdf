use super::common::*;
use lopdf::{Dictionary, Document, Object};

// ===== ACCESSIBILITY CHECK =====

pub fn check_accessibility(data: &[u8]) -> Result<serde_json::Value, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let page_ids = get_page_ids(&doc);
    let mut issues = Vec::new();

    // Check for tagged PDF (both MarkInfo and StructTreeRoot are required by ISO 14289 / PDF/UA)
    let has_tags = if let Ok(root_id) = doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        if let Some(Object::Dictionary(ref root_dict)) = doc.objects.get(&root_id) {
            let marked = root_dict
                .get(b"MarkInfo")
                .and_then(|m| match m {
                    Object::Dictionary(d) => {
                        Ok(d.get(b"Marked").ok() == Some(&Object::Boolean(true)))
                    }
                    Object::Reference(id) => {
                        let is_m = doc
                            .objects
                            .get(id)
                            .and_then(|o| o.as_dict().ok())
                            .map(|d| d.get(b"Marked").ok() == Some(&Object::Boolean(true)))
                            .unwrap_or(false);
                        Ok(is_m)
                    }
                    _ => Ok(false),
                })
                .unwrap_or(false);
            let has_struct_tree = root_dict.get(b"StructTreeRoot").is_ok();
            marked && has_struct_tree
        } else {
            false
        }
    } else {
        false
    };

    if !has_tags {
        issues.push(serde_json::json!({
            "severity": "error",
            "message": "PDF is not tagged (論理構造ツリー StructTreeRoot / MarkInfo が未定義です)"
        }));
    }

    // Check for document title
    let has_title = if let Ok(info_id) = doc.trailer.get(b"Info").and_then(|o| o.as_reference()) {
        if let Some(Object::Dictionary(ref info_dict)) = doc.objects.get(&info_id) {
            info_dict.get(b"Title").is_ok()
        } else {
            false
        }
    } else {
        false
    };

    if !has_title {
        issues.push(serde_json::json!({
            "severity": "warning",
            "message": "Document title is not set"
        }));
    }

    // Check for language
    let has_lang = if let Ok(root_id) = doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        if let Some(Object::Dictionary(ref root_dict)) = doc.objects.get(&root_id) {
            root_dict.get(b"Lang").is_ok()
        } else {
            false
        }
    } else {
        false
    };

    if !has_lang {
        issues.push(serde_json::json!({
            "severity": "warning",
            "message": "Document language is not set"
        }));
    }

    // Deep PDF/UA structural validation: traverse StructTreeRoot if present
    if let Ok(root_id) = doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        if let Some(Object::Dictionary(ref root_dict)) = doc.objects.get(&root_id) {
            if let Ok(struct_tree_ref) = root_dict
                .get(b"StructTreeRoot")
                .and_then(|o| o.as_reference())
            {
                let mut struct_queue = vec![struct_tree_ref];
                let mut figures_without_alt = 0;
                let mut table_element_count = 0;
                let mut table_rows_found = 0;

                while let Some(elem_id) = struct_queue.pop() {
                    if let Some(Object::Dictionary(ref elem_dict)) = doc.objects.get(&elem_id) {
                        let struct_type = elem_dict
                            .get(b"S")
                            .ok()
                            .and_then(|s| s.as_name().ok())
                            .unwrap_or(b"");

                        if struct_type == b"Figure" {
                            if elem_dict.get(b"Alt").is_err() {
                                figures_without_alt += 1;
                            }
                        } else if struct_type == b"Table" {
                            table_element_count += 1;
                        } else if struct_type == b"TR" {
                            table_rows_found += 1;
                        }

                        // Traverse children in /K
                        if let Ok(k_obj) = elem_dict.get(b"K") {
                            match k_obj {
                                Object::Reference(child_id) => struct_queue.push(*child_id),
                                Object::Array(arr) => {
                                    for item in arr {
                                        if let Ok(child_id) = item.as_reference() {
                                            struct_queue.push(child_id);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if figures_without_alt > 0 {
                    issues.push(serde_json::json!({
                        "severity": "error",
                        "message": format!("PDF/UA violation: {figures_without_alt} Figure structure element(s) missing required /Alt text")
                    }));
                }

                if table_element_count > 0 && table_rows_found == 0 {
                    issues.push(serde_json::json!({
                        "severity": "warning",
                        "message": "PDF/UA warning: Table element detected without standard /TR row structure hierarchy"
                    }));
                }
            }
        }
    }

    // Check each page for form fields without tooltips
    for &page_id in &page_ids {
        if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
            if let Ok(Object::Array(annots)) = dict.get(b"Annots") {
                for annot_ref in annots {
                    if let Object::Reference(ref_id) = annot_ref {
                        if let Some(Object::Dictionary(annot_dict)) = doc.objects.get(ref_id) {
                            if let Ok(Object::Name(subtype)) = annot_dict.get(b"Subtype") {
                                if subtype == b"Widget" {
                                    // Form field - check for tooltip
                                    if annot_dict.get(b"TU").is_err() {
                                        issues.push(serde_json::json!({
                                            "severity": "warning",
                                            "message": "Form field missing tooltip (TU entry)"
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let score = if issues.is_empty() {
        100
    } else {
        100 - (issues.iter().filter(|i| i["severity"] == "error").count() * 20)
            - (issues.iter().filter(|i| i["severity"] == "warning").count() * 10)
    };

    Ok(serde_json::json!({
        "score": score.max(0),
        "issues": issues,
        "page_count": page_ids.len(),
        "has_tags": has_tags,
        "has_title": has_title,
        "has_language": has_lang,
    }))
}

/// Automatically repair accessibility issues (inject MarkInfo/Marked, default Lang, Title, and field Tooltips)
pub fn fix_accessibility_issues(
    data: &[u8],
    default_title: &str,
    default_lang: &str,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    let root_id = match doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        Ok(id) => id,
        Err(_) => return Err("No Root catalog found in PDF".into()),
    };

    let title_str = if default_title.trim().is_empty() {
        "Accessible Document"
    } else {
        default_title
    };
    let lang_str = if default_lang.trim().is_empty() {
        "ja-JP"
    } else {
        default_lang
    };

    // 1. Ensure StructTreeRoot exists, or construct a conforming logical structure tree
    let struct_tree_root_id = if let Some(Object::Dictionary(ref root_dict)) = doc.objects.get(&root_id) {
        root_dict.get(b"StructTreeRoot").ok().and_then(|o| o.as_reference().ok())
    } else {
        None
    };

    let struct_tree_id = match struct_tree_root_id {
        Some(id) => id,
        None => {
            // Build logical structure tree hierarchy for Tagged PDF (PDF/UA conformance)
            let page_ids = get_page_ids(&doc);
            let mut elem_refs = Vec::new();

            // Create structure elements for each page (/Document -> /Part -> /P)
            // ISO 14289-1 (PDF/UA) & ISO 32000-1 §14.7: Structure elements must correspond to Marked Content
            // in page content streams via /MCID (Marked Content Identifier) tags.
            for &pid in &page_ids {
                let mut p_dict = Dictionary::new();
                p_dict.set("Type", Object::Name(b"StructElem".to_vec()));
                p_dict.set("S", Object::Name(b"P".to_vec()));
                p_dict.set("Pg", Object::Reference(pid));
                // Associate MCID 0 with this paragraph structure element
                p_dict.set("K", Object::Integer(0));
                let p_id = doc.add_object(Object::Dictionary(p_dict));
                elem_refs.push(Object::Reference(p_id));

                // Tag the page contents with BDC /Span << /MCID 0 >> and EMC so screen readers and PDF/UA validators recognize content
                let content_ids = super::common::resolve_page_content_stream_ids(&doc, pid);
                for cid in content_ids {
                    if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&cid) {
                        let orig_bytes = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
                        if let Ok(mut content) = lopdf::content::Content::decode(&orig_bytes) {
                            let mut mc_dict = Dictionary::new();
                            mc_dict.set("MCID", Object::Integer(0));
                            let bdc_op = lopdf::content::Operation::new(
                                "BDC",
                                vec![Object::Name(b"Span".to_vec()), Object::Dictionary(mc_dict)],
                            );
                            let emc_op = lopdf::content::Operation::new("EMC", vec![]);
                            content.operations.insert(0, bdc_op);
                            content.operations.push(emc_op);
                            if let Ok(tagged_bytes) = content.encode() {
                                stream.set_content(tagged_bytes);
                                stream.dict.remove(b"Filter");
                            }
                        }
                    }
                }
            }

            let mut struct_elem_dict = Dictionary::new();
            struct_elem_dict.set("Type", Object::Name(b"StructElem".to_vec()));
            struct_elem_dict.set("S", Object::Name(b"Document".to_vec()));
            struct_elem_dict.set("K", Object::Array(elem_refs));
            let doc_elem_id = doc.add_object(Object::Dictionary(struct_elem_dict));

            let mut st_dict = Dictionary::new();
            st_dict.set("Type", Object::Name(b"StructTreeRoot".to_vec()));
            st_dict.set("K", Object::Reference(doc_elem_id));
            let st_id = doc.add_object(Object::Dictionary(st_dict));

            // Link parent in Document element
            if let Some(Object::Dictionary(ref mut elem)) = doc.objects.get_mut(&doc_elem_id) {
                elem.set("P", Object::Reference(st_id));
            }

            st_id
        }
    };

    // Mark Root with Lang, Title preference, MarkInfo, and StructTreeRoot
    if let Some(Object::Dictionary(ref mut root_dict)) = doc.objects.get_mut(&root_id) {
        root_dict.set("StructTreeRoot", Object::Reference(struct_tree_id));

        let mut mark_info = Dictionary::new();
        mark_info.set("Marked", Object::Boolean(true));
        root_dict.set("MarkInfo", Object::Dictionary(mark_info));

        root_dict.set(
            "Lang",
            Object::String(lang_str.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        );

        // Set ViewerPreferences to display DocTitle
        let mut vp = Dictionary::new();
        vp.set("DisplayDocTitle", Object::Boolean(true));
        root_dict.set("ViewerPreferences", Object::Dictionary(vp));
    }

    // Ensure all Figure structure elements have default Alt text if missing
    let mut to_fix_alts = Vec::new();
    for (&oid, obj) in doc.objects.iter() {
        if let Ok(dict) = obj.as_dict() {
            if dict.get(b"Type").ok().and_then(|t| t.as_name().ok()) == Some(b"StructElem") {
                if dict.get(b"S").ok().and_then(|s| s.as_name().ok()) == Some(b"Figure") {
                    if dict.get(b"Alt").is_err() {
                        to_fix_alts.push(oid);
                    }
                }
            }
        }
    }
    for fid in to_fix_alts {
        if let Some(Object::Dictionary(ref mut fdict)) = doc.objects.get_mut(&fid) {
            fdict.set("Alt", Object::String(b"Image illustration".to_vec(), lopdf::StringFormat::Literal));
        }
    }

    // 2. Set Info Title
    let info_id = if let Ok(id) = doc.trailer.get(b"Info").and_then(|o| o.as_reference()) {
        id
    } else {
        let info_dict = Dictionary::new();
        doc.add_object(Object::Dictionary(info_dict))
    };

    if let Some(Object::Dictionary(ref mut info_dict)) = doc.objects.get_mut(&info_id) {
        if info_dict.get(b"Title").is_err() {
            info_dict.set(
                "Title",
                Object::String(
                    encode_pdf_text_string(title_str),
                    lopdf::StringFormat::Literal,
                ),
            );
        }
    }
    doc.trailer.set("Info", Object::Reference(info_id));

    // 3. Add Tooltips (TU) to Widget form fields if missing
    let page_ids = get_page_ids(&doc);
    for &page_id in &page_ids {
        let annot_refs: Vec<OID> =
            if let Some(Object::Dictionary(ref pdict)) = doc.objects.get(&page_id) {
                pdict
                    .get(b"Annots")
                    .ok()
                    .and_then(|a| a.as_array().ok())
                    .map(|arr| arr.iter().filter_map(|o| o.as_reference().ok()).collect())
                    .unwrap_or_default()
            } else {
                vec![]
            };

        for aref in annot_refs {
            if let Some(Object::Dictionary(ref mut adict)) = doc.objects.get_mut(&aref) {
                if let Ok(Object::Name(sub)) = adict.get(b"Subtype") {
                    if sub == b"Widget" && adict.get(b"TU").is_err() {
                        let name_desc = adict
                            .get(b"T")
                            .ok()
                            .and_then(|o| match o {
                                Object::String(b, _) => {
                                    Some(decode_pdf_text_string(b))
                                }
                                _ => None,
                            })
                            .unwrap_or_else(|| "Form Input Field".to_string());
                        adict.set(
                            "TU",
                            Object::String(
                                encode_pdf_text_string(&name_desc),
                                lopdf::StringFormat::Literal,
                            ),
                        );
                    }
                }
            }
        }
    }

    save_doc(&mut doc)
}
