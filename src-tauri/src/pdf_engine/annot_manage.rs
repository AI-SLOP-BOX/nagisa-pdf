use super::common::*;
use lopdf::{Dictionary, Document, Object};
use std::collections::HashSet;

// ===== ANNOTATION MANAGEMENT =====

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Annotation {
    pub id: OID,
    pub annot_type: String,
    pub contents: String,
    pub author: String,
    pub page: usize,
    pub x: f64,
    pub y: f64,
    pub color: String,
    pub status: String,
    pub replies: Vec<AnnotationReply>,
    pub created: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AnnotationReply {
    pub id: OID,
    pub author: String,
    pub contents: String,
    pub created: String,
}

pub fn get_annotations(data: &[u8]) -> Result<Vec<serde_json::Value>, String> {
    let doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    let mut annotations = Vec::new();

    for (page_idx, &page_id) in page_ids.iter().enumerate() {
        if let Some(Object::Dictionary(ref dict)) = doc.objects.get(&page_id) {
            // Resolve /Annots: handle both direct Array and indirect Reference
            let annots_arr: Vec<Object> = match dict.get(b"Annots") {
                Ok(Object::Array(arr)) => arr.clone(),
                Ok(Object::Reference(ref_id)) => {
                    doc.objects
                        .get(ref_id)
                        .and_then(|o| o.as_array().ok())
                        .cloned()
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            };
            for annot_ref in &annots_arr {
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

                            // ISO 32000-1 §12.5.6.3: Annotation review status is stored in /State
                            // (e.g. "Accepted", "Rejected", "Completed", "Cancelled", "None")
                            let status = annot_dict
                                .get(b"State")
                                .ok()
                                .and_then(|o| match o {
                                    Object::String(bytes, _) => {
                                        Some(String::from_utf8_lossy(bytes).to_string())
                                    }
                                    Object::Name(bytes) => {
                                        Some(String::from_utf8_lossy(bytes).to_string())
                                    }
                                    _ => None,
                                })
                                .or_else(|| {
                                    // Fallback if legacy or non-standard tool wrote to /Name
                                    annot_dict.get(b"Name").ok().and_then(|o| match o {
                                        Object::Name(bytes) => {
                                            Some(String::from_utf8_lossy(bytes).to_string())
                                        }
                                        Object::String(bytes, _) => {
                                            Some(String::from_utf8_lossy(bytes).to_string())
                                        }
                                        _ => None,
                                    })
                                })
                                .unwrap_or_default();

                            // ISO 32000-1 §12.5.4: /Rect is [x1, y1, x2, y2]
                            let (x, y, width, height) = match annot_dict.get(b"Rect") {
                                Ok(Object::Array(arr)) if arr.len() >= 4 => {
                                    let x1 = match &arr[0] {
                                        Object::Real(v) => *v as f64,
                                        Object::Integer(v) => *v as f64,
                                        _ => 0.0,
                                    };
                                    let y1 = match &arr[1] {
                                        Object::Real(v) => *v as f64,
                                        Object::Integer(v) => *v as f64,
                                        _ => 0.0,
                                    };
                                    let x2 = match &arr[2] {
                                        Object::Real(v) => *v as f64,
                                        Object::Integer(v) => *v as f64,
                                        _ => x1,
                                    };
                                    let y2 = match &arr[3] {
                                        Object::Real(v) => *v as f64,
                                        Object::Integer(v) => *v as f64,
                                        _ => y1,
                                    };
                                    (x1, y1, (x2 - x1).abs(), (y2 - y1).abs())
                                }
                                Ok(Object::Array(arr)) if arr.len() >= 2 => {
                                    let x = match &arr[0] {
                                        Object::Real(v) => *v as f64,
                                        Object::Integer(v) => *v as f64,
                                        _ => 0.0,
                                    };
                                    let y = match &arr[1] {
                                        Object::Real(v) => *v as f64,
                                        Object::Integer(v) => *v as f64,
                                        _ => 0.0,
                                    };
                                    (x, y, 0.0, 0.0)
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
                                    format!("#{:02X}{:02X}{:02X}", r, g, b)
                                }
                                _ => "#FF0000".to_string(),
                            };

                            // Check for replies (IRT - In Reply To)
                            let mut replies = Vec::new();
                            for (_, reply_obj) in doc.objects.iter() {
                                if let Object::Dictionary(reply_dict) = reply_obj {
                                    if let Ok(Object::Reference(irt_ref)) = reply_dict.get(b"IRT") {
                                        if irt_ref == ref_id {
                                            let reply_contents = reply_dict
                                                .get(b"Contents")
                                                .ok()
                                                .and_then(|o| match o {
                                                    Object::String(bytes, _) => {
                                                        Some(decode_pdf_text_string(bytes))
                                                    }
                                                    _ => None,
                                                })
                                                .unwrap_or_default();

                                            let reply_author = reply_dict
                                                .get(b"T")
                                                .ok()
                                                .and_then(|o| match o {
                                                    Object::String(bytes, _) => {
                                                        Some(decode_pdf_text_string(bytes))
                                                    }
                                                    _ => None,
                                                })
                                                .unwrap_or_default();

                                            replies.push(serde_json::json!({
                                                "author": reply_author,
                                                "contents": reply_contents,
                                            }));
                                        }
                                    }
                                }
                            }

                            annotations.push(serde_json::json!({
                                "id": format!("{}_{}", ref_id.0, ref_id.1),
                                "page": page_idx + 1,
                                "type": annot_type,
                                "author": author,
                                "contents": contents,
                                "status": status,
                                "x": x,
                                "y": y,
                                "width": width,
                                "height": height,
                                "color": color,
                                "replies": replies,
                            }));
                        }
                    }
                }
            }
        }

    Ok(annotations)
}

pub fn add_annotation_reply(
    data: &[u8],
    annotation_id: (u32, u16),
    author: &str,
    contents: &str,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    // ISO 32000-1 §12.5.6.3: Inherit bounding rect from parent annotation if available
    let parent_rect = if let Some(Object::Dictionary(ref parent_dict)) = doc.objects.get(&annotation_id) {
        parent_dict.get(b"Rect").ok().cloned()
    } else {
        None
    };

    let rect = parent_rect.unwrap_or_else(|| {
        Object::Array(vec![
            Object::Real(50.0),
            Object::Real(50.0),
            Object::Real(70.0),
            Object::Real(70.0),
        ])
    });

    let mut reply_dict = Dictionary::new();
    reply_dict.set("Type", Object::Name("Annot".into()));
    reply_dict.set("Subtype", Object::Name("Text".into()));
    reply_dict.set(
        "T",
        Object::String(
            encode_pdf_text_string(author),
            lopdf::StringFormat::Literal,
        ),
    );
    reply_dict.set(
        "Contents",
        Object::String(
            encode_pdf_text_string(contents),
            lopdf::StringFormat::Literal,
        ),
    );
    // ISO 32000-1 §12.5.6.3:
    // /IRT specifies the parent annotation object
    // /RT specifies the reply type: /R (Reply) or /Group
    reply_dict.set("IRT", Object::Reference(annotation_id));
    reply_dict.set("RT", Object::Name("R".into()));
    reply_dict.set("Rect", rect);
    // Annotation flags: Print (4) + NoZoom (8) + NoRotate (16) = 28
    reply_dict.set("F", Object::Integer(28));
    reply_dict.set("Open", Object::Boolean(false));

    let reply_id = doc.add_object(Object::Dictionary(reply_dict));

    // Find the page containing the parent annotation and add reply to its Annots (supporting indirect Annots)
    let page_ids = get_page_ids(&doc);
    let mut attached_to_page = false;
    for page_id in page_ids {
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
                let contains = arr.iter().any(|a| matches!(a, Object::Reference(id) if *id == annotation_id));
                if contains {
                    arr.push(Object::Reference(reply_id));
                    attached_to_page = true;
                    break;
                }
            }
        } else if let Some(Object::Dictionary(ref mut p_dict)) = doc.objects.get_mut(&page_id) {
            if let Ok(Object::Array(ref mut arr)) = p_dict.get_mut(b"Annots") {
                let contains = arr.iter().any(|a| matches!(a, Object::Reference(id) if *id == annotation_id));
                if contains {
                    arr.push(Object::Reference(reply_id));
                    attached_to_page = true;
                    break;
                }
            }
        }
    }

    if !attached_to_page {
        return Err(format!(
            "親注釈（ID: {}:{}）がPDF内のいずれのページにも見つかりませんでした。孤立した注釈返信の生成を防ぐため処理を中断しました。",
            annotation_id.0, annotation_id.1
        ));
    }

    save_doc(&mut doc)
}

pub fn set_annotation_status(
    data: &[u8],
    annotation_id: (u32, u16),
    status: &str,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    if let Some(Object::Dictionary(ref mut annot_dict)) = doc.objects.get_mut(&annotation_id) {
        // ISO 32000-1 §12.5.6.3:
        // /State specifies the review status (e.g., "Accepted", "Rejected", "Completed", "Cancelled", "None")
        // /StateModel specifies the state model being used, typically "Review"
        annot_dict.set("State", Object::String(status.as_bytes().to_vec(), lopdf::StringFormat::Literal));
        annot_dict.set("StateModel", Object::String(b"Review".to_vec(), lopdf::StringFormat::Literal));
        // Keep /Name for backwards-compatibility with legacy clients
        annot_dict.set("Name", Object::Name(status.as_bytes().to_vec()));
    }

    save_doc(&mut doc)
}

pub fn delete_annotation(data: &[u8], annotation_id: (u32, u16)) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;

    // 1. Identify parent annotation and all hierarchical replies (transitive IRT)
    // #41 是正: 訪問済みIDを HashSet で管理し、IRTがループしている悉意のPDFで無限ループに降るかのサイクルガード。
    let mut ids_to_remove: Vec<OID> = vec![annotation_id];
    let mut visited: HashSet<OID> = HashSet::new();
    visited.insert(annotation_id);
    let mut changed = true;
    while changed {
        changed = false;
        for (id, obj) in doc.objects.iter() {
            // 既に訪問済みの ID はスキップ（サイクルガード）
            if visited.contains(id) {
                continue;
            }
            if let Object::Dictionary(dict) = obj {
                if let Ok(Object::Reference(irt_ref)) = dict.get(b"IRT") {
                    if ids_to_remove.contains(irt_ref) {
                        ids_to_remove.push(*id);
                        visited.insert(*id);
                        changed = true;
                    }
                }
            }
        }
    }

    // 2. Remove references to annotation_id and ALL replies from page Annots (both direct and indirect arrays)
    let page_ids = get_page_ids(&doc);
    for page_id in page_ids {
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
                arr.retain(|a| !matches!(a, Object::Reference(id) if ids_to_remove.contains(id)));
            }
        } else if let Some(Object::Dictionary(ref mut p_dict)) = doc.objects.get_mut(&page_id) {
            if let Ok(Object::Array(ref mut arr)) = p_dict.get_mut(b"Annots") {
                arr.retain(|a| !matches!(a, Object::Reference(id) if ids_to_remove.contains(id)));
            }
        }
    }

    // Also scan all remaining arrays in document to guarantee zero dangling references
    for (_, obj) in doc.objects.iter_mut() {
        if let Object::Array(ref mut arr) = obj {
            arr.retain(|a| !matches!(a, Object::Reference(id) if ids_to_remove.contains(id)));
        } else if let Object::Dictionary(ref mut dict) = obj {
            if let Ok(Object::Array(ref mut arr)) = dict.get_mut(b"Annots") {
                arr.retain(|a| !matches!(a, Object::Reference(id) if ids_to_remove.contains(id)));
            }
        }
    }

    // 3. Remove all annotation and reply objects from document
    for id in ids_to_remove {
        doc.objects.remove(&id);
    }

    save_doc(&mut doc)
}
