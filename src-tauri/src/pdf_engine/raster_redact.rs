//! Pixel-level (raster) redaction for image XObjects.
//!
//! Overwrites the actual pixels covered by a redaction rectangle so the
//! original content cannot be recovered from the file — the crucial case
//! being scanned documents whose page *is* one large raster image.
//!
//! What this module does correctly (unlike the naive "black rectangle on
//! top" approach, which leaves the pixels intact underneath):
//!   * resolves the image placement from the real graphics state: `q`/`Q`
//!     stack, `cm` concatenation (matrix composition, not "last cm wins"),
//!     Form XObjects (incl. their /Matrix) and inherited /Resources
//!   * maps the PDF-space rectangle through the inverse CTM, so scaled,
//!     translated, rotated and flipped placements hit the right pixels
//!   * decodes DeviceRGB / DeviceGray / Indexed 8-bit raster data, raw or
//!     FlateDecode, plus DCTDecode (JPEG) via the `image` crate
//!   * also paints the matching region of /SMask and /Mask companions so a
//!     transparency layer cannot re-reveal the erased area
//!   * re-encodes the modified image **losslessly** (FlateDecode) instead of
//!     an irreversible JPEG round trip that can leave ghosting
//!   * refuses to redact (hard error) when an intersecting image cannot be
//!     decoded — a silent "looks redacted but is not" result is the one
//!     failure mode that must never happen
//!
//! `purge_unreachable_objects` additionally drops objects that are no longer
//! reachable from the trailer (e.g. an unreferenced copy of the original
//! scan), which would otherwise remain recoverable in the file.
//!
//! Note: `deep_redact` already runs lopdf's `prune_objects` pass, so the
//! pruning concern is covered end-to-end; the helper is exercised in tests.

use std::collections::HashSet;

use lopdf::{Document, Object, ObjectId};

use crate::pdf_engine::page_tree::materialize_inherited_page_attrs;

// ===== affine matrices (PDF convention: [a b c d e f], row vectors) =====

pub type Mat = [f64; 6];

pub const IDENTITY: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m1` applied first, then `m2` (i.e. CTM' = cm × CTM).
pub fn mat_mul(m1: Mat, m2: Mat) -> Mat {
    let [a1, b1, c1, d1, e1, f1] = m1;
    let [a2, b2, c2, d2, e2, f2] = m2;
    [
        a1 * a2 + b1 * c2,
        a1 * b2 + b1 * d2,
        c1 * a2 + d1 * c2,
        c1 * b2 + d1 * d2,
        e1 * a2 + f1 * c2 + e2,
        e1 * b2 + f1 * d2 + f2,
    ]
}

pub fn mat_apply(m: Mat, x: f64, y: f64) -> (f64, f64) {
    let [a, b, c, d, e, f] = m;
    (a * x + c * y + e, b * x + d * y + f)
}

pub fn mat_invert(m: Mat) -> Option<Mat> {
    let [a, b, c, d, e, f] = m;
    let det = a * d - b * c;
    if det.abs() < 1e-12 {
        return None;
    }
    Some([
        d / det,
        -b / det,
        -c / det,
        a / det,
        (c * f - d * e) / det,
        (b * e - a * f) / det,
    ])
}

/// An image XObject drawn on the page; `ctm` maps image unit space (0..1
/// square, origin bottom-left) → PDF user space.
#[derive(Debug, Clone)]
pub struct ImagePlacement {
    pub name: Vec<u8>,
    pub oid: ObjectId,
    pub ctm: Mat,
}

fn as_num(obj: &Object) -> Option<f64> {
    match obj {
        Object::Real(f) => Some(*f as f64),
        Object::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

// ===== image decoding (always materialized as RGB8) =====

pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

fn color_space_name(dict: &lopdf::Dictionary) -> Option<Vec<u8>> {
    match dict.get(b"ColorSpace").ok() {
        Some(Object::Name(n)) => Some(n.clone()),
        Some(Object::Array(arr)) => arr
            .first()
            .and_then(|o| o.as_name().ok())
            .map(|n| n.to_vec()),
        _ => None,
    }
}

fn gray_to_rgb(gray: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(gray.len() * 3);
    for g in gray {
        out.extend_from_slice(&[*g, *g, *g]);
    }
    out
}

/// Decode an image XObject stream into RGB8. Returns an honest error for
/// formats that cannot be safely rewritten (JPX, CCITT, JBIG2, sub-8-bit
/// components, exotic colour spaces).
pub fn decode_image_rgb(stream: &lopdf::Stream) -> Result<DecodedImage, String> {
    let width = stream
        .dict
        .get(b"Width")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0) as u32;
    let height = stream
        .dict
        .get(b"Height")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err("画像のWidth/Heightが不正です".into());
    }
    let bpc = stream
        .dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(8);
    if bpc != 8 {
        return Err(format!("BitsPerComponent={bpc} は未対応です（8のみ対応）"));
    }
    if stream.dict.get(b"Decode").is_ok() {
        return Err("独自/Decode配列付きの画像は未対応です".into());
    }

    let mut filters: Vec<Vec<u8>> = Vec::new();
    match stream.dict.get(b"Filter").ok() {
        Some(Object::Name(n)) => filters.push(n.clone()),
        Some(Object::Array(arr)) => {
            for o in arr {
                if let Ok(n) = o.as_name() {
                    filters.push(n.to_vec());
                }
            }
        }
        _ => {}
    }
    let has = |f: &[u8]| filters.iter().any(|x| x.as_slice() == f);

    if has(b"DCTDecode") {
        let img = image::load_from_memory(&stream.content)
            .or_else(|_| {
                image::load_from_memory(&stream.decompressed_content().unwrap_or_default())
            })
            .map_err(|e| format!("JPEG画像のデコードに失敗しました: {e}"))?;
        let rgb = img.to_rgb8();
        return Ok(DecodedImage {
            width: rgb.width(),
            height: rgb.height(),
            rgb: rgb.into_raw(),
        });
    }
    if has(b"JPXDecode") || has(b"CCITTFaxDecode") || has(b"JBIG2Decode") {
        let f = filters.first().cloned().unwrap_or_default();
        return Err(format!(
            "フィルタ /{} の画像は画素消去に未対応です（先に画像を平坦化してください）",
            String::from_utf8_lossy(&f)
        ));
    }
    if !filters.is_empty() && !has(b"FlateDecode") {
        let f = filters.first().cloned().unwrap_or_default();
        return Err(format!(
            "フィルタ /{} は未対応です",
            String::from_utf8_lossy(&f)
        ));
    }

    let raw = stream
        .decompressed_content()
        .map_err(|e| format!("画像ストリームの展開に失敗しました: {e}"))?;

    match color_space_name(&stream.dict).as_deref() {
        Some(b"DeviceRGB") | Some(b"RGB") => {
            let need = (width as usize) * (height as usize) * 3;
            if raw.len() < need {
                return Err("RGB画像のデータ長が不足しています".into());
            }
            Ok(DecodedImage {
                width,
                height,
                rgb: raw[..need].to_vec(),
            })
        }
        Some(b"DeviceGray") | Some(b"G") => {
            let need = (width as usize) * (height as usize);
            if raw.len() < need {
                return Err("グレースケール画像のデータ長が不足しています".into());
            }
            Ok(DecodedImage {
                width,
                height,
                rgb: gray_to_rgb(&raw[..need]),
            })
        }
        Some(b"Indexed") | Some(b"I") => decode_indexed(&stream.dict, width, height, &raw),
        Some(other) => Err(format!(
            "カラースペース /{} の画像は画素消去に未対応です",
            String::from_utf8_lossy(other)
        )),
        None => Err("ColorSpaceが指定されていない画像は未対応です".into()),
    }
}

fn decode_indexed(
    dict: &lopdf::Dictionary,
    width: u32,
    height: u32,
    raw: &[u8],
) -> Result<DecodedImage, String> {
    let arr = match dict.get(b"ColorSpace").ok() {
        Some(Object::Array(arr)) => arr.clone(),
        _ => return Err("Indexedカラースペースの配列が不正です".into()),
    };
    let hival = arr.get(2).and_then(|o| o.as_i64().ok()).unwrap_or(255) as usize;
    let lookup: Vec<u8> = match arr.get(3) {
        Some(Object::String(bytes, _)) => bytes.clone(),
        Some(Object::Stream(s)) => s
            .decompressed_content()
            .unwrap_or_else(|_| s.content.clone()),
        _ => return Err("Indexedカラースペースの参照テーブルが不正です".into()),
    };
    let base = arr
        .first()
        .and_then(|o| o.as_name().ok())
        .unwrap_or(b"DeviceRGB");
    let comps = if base == b"DeviceGray" { 1 } else { 3 };
    let need = (width as usize) * (height as usize);
    if raw.len() < need || lookup.len() < (hival + 1) * comps {
        return Err("Indexed画像のデータ長が不足しています".into());
    }
    let mut rgb = Vec::with_capacity(need * 3);
    for &idx in &raw[..need] {
        let i = (idx as usize).min(hival) * comps;
        if comps == 1 {
            let g = lookup[i];
            rgb.extend_from_slice(&[g, g, g]);
        } else {
            rgb.extend_from_slice(&lookup[i..i + 3]);
        }
    }
    Ok(DecodedImage { width, height, rgb })
}

fn resolve_dict(doc: &Document, obj: Option<&Object>) -> Option<lopdf::Dictionary> {
    match obj {
        Some(Object::Dictionary(d)) => Some(d.clone()),
        Some(Object::Reference(id)) => doc.objects.get(id).and_then(|o| o.as_dict().ok()).cloned(),
        _ => None,
    }
}

fn stream_bytes(stream: &lopdf::Stream) -> Vec<u8> {
    stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone())
}

fn mat_from_ops(op: &lopdf::content::Operation) -> Option<Mat> {
    if op.operands.len() < 6 {
        return None;
    }
    let vals: Option<Vec<f64>> = op.operands.iter().map(as_num).collect();
    let v = vals?;
    Some([v[0], v[1], v[2], v[3], v[4], v[5]])
}

fn matrix_from_array(obj: Option<&Object>) -> Mat {
    if let Some(Object::Array(arr)) = obj {
        let vals: Option<Vec<f64>> = arr.iter().map(as_num).collect();
        if let Some(v) = vals {
            if v.len() >= 6 {
                return [v[0], v[1], v[2], v[3], v[4], v[5]];
            }
        }
    }
    IDENTITY
}

/// Collect every image drawn on the page (through Form XObjects too) with the
/// exact CTM that places it, honouring `q`/`Q`, `cm` composition and
/// inherited resources. Annotation appearance streams are intentionally out
/// of scope (they are not page content).
pub fn collect_page_image_placements(doc: &Document, page_id: ObjectId) -> Vec<ImagePlacement> {
    let page_dict = materialize_inherited_page_attrs(doc, page_id);
    let resources = resolve_dict(doc, page_dict.get(b"Resources").ok());

    let mut bytes: Vec<u8> = Vec::new();
    match page_dict.get(b"Contents").ok() {
        Some(Object::Reference(id)) => {
            if let Some(Object::Stream(s)) = doc.objects.get(id) {
                bytes.extend_from_slice(&stream_bytes(s));
                bytes.push(b'\n');
            }
        }
        Some(Object::Array(arr)) => {
            for o in arr {
                if let Ok(id) = o.as_reference() {
                    if let Some(Object::Stream(s)) = doc.objects.get(&id) {
                        bytes.extend_from_slice(&stream_bytes(s));
                        bytes.push(b'\n');
                    }
                }
            }
        }
        _ => {}
    }

    let mut out = Vec::new();
    let mut path: HashSet<ObjectId> = HashSet::new();
    walk_content(
        doc,
        &bytes,
        resources.as_ref(),
        IDENTITY,
        &mut out,
        &mut path,
        0,
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn walk_content(
    doc: &Document,
    bytes: &[u8],
    resources: Option<&lopdf::Dictionary>,
    base_ctm: Mat,
    out: &mut Vec<ImagePlacement>,
    path: &mut HashSet<ObjectId>,
    depth: usize,
) {
    if depth > 8 {
        return;
    }
    let Ok(content) = lopdf::content::Content::decode(bytes) else {
        return;
    };

    let xobjects = resources
        .and_then(|r| r.get(b"XObject").ok())
        .and_then(|xo| match xo {
            Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()),
            Object::Dictionary(d) => Some(d),
            _ => None,
        });

    let mut stack: Vec<Mat> = Vec::new();
    let mut ctm = base_ctm;

    for op in &content.operations {
        match op.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => {
                if let Some(m) = stack.pop() {
                    ctm = m;
                }
            }
            "cm" => {
                if let Some(m) = mat_from_ops(op) {
                    ctm = mat_mul(m, ctm);
                }
            }
            "Do" => {
                let Some(Object::Name(name)) = op.operands.first() else {
                    continue;
                };
                let Some(xobjects) = xobjects else { continue };
                let entry = xobjects.get(name.as_slice()).ok();
                let (oid, stream) = match entry {
                    Some(Object::Reference(id)) => match doc.objects.get(id) {
                        Some(Object::Stream(s)) => (Some(*id), s.clone()),
                        _ => continue,
                    },
                    Some(Object::Stream(s)) => (None, s.clone()),
                    _ => continue,
                };
                let subtype = stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|o| o.as_name().ok());
                match subtype {
                    Some(b"Image") => {
                        if let Some(oid) = oid {
                            out.push(ImagePlacement {
                                name: name.clone(),
                                oid,
                                ctm,
                            });
                        }
                    }
                    Some(b"Form") => {
                        let Some(oid) = oid else { continue };
                        if path.contains(&oid) {
                            continue; // cycle guard
                        }
                        path.insert(oid);
                        let form_matrix = matrix_from_array(stream.dict.get(b"Matrix").ok());
                        let nested_ctm = mat_mul(form_matrix, ctm);
                        // Forms fall back to the calling context's resources.
                        let form_res = resolve_dict(doc, stream.dict.get(b"Resources").ok())
                            .or_else(|| resources.cloned());
                        let form_bytes = stream_bytes(&stream);
                        walk_content(
                            doc,
                            &form_bytes,
                            form_res.as_ref(),
                            nested_ctm,
                            out,
                            path,
                            depth + 1,
                        );
                        path.remove(&oid);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// Map a PDF-space rectangle through the inverse CTM into pixel bounds
/// (start inclusive, end exclusive). `None` when the rectangle does not
/// cover the image at all.
fn pixel_bounds(
    ctm: Mat,
    rect: (f64, f64, f64, f64),
    w: u32,
    h: u32,
    margin: u32,
) -> Option<(u32, u32, u32, u32)> {
    let inv = mat_invert(ctm)?;
    let (x1, y1, x2, y2) = rect;
    let mut min_u = f64::MAX;
    let mut max_u = f64::MIN;
    let mut min_v = f64::MAX;
    let mut max_v = f64::MIN;
    for (px, py) in [(x1, y1), (x2, y1), (x1, y2), (x2, y2)] {
        let (u, v) = mat_apply(inv, px, py);
        min_u = min_u.min(u);
        max_u = max_u.max(u);
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }
    if max_u <= 0.0 || min_u >= 1.0 || max_v <= 0.0 || min_v >= 1.0 {
        return None;
    }
    let min_u = min_u.clamp(0.0, 1.0);
    let max_u = max_u.clamp(0.0, 1.0);
    let min_v = min_v.clamp(0.0, 1.0);
    let max_v = max_v.clamp(0.0, 1.0);

    let mut px1 = ((min_u * w as f64).floor() as i64) - margin as i64;
    let mut px2 = ((max_u * w as f64).ceil() as i64) + margin as i64;
    // Image row 0 is the top edge; v grows upwards.
    let mut py1 = (((1.0 - max_v) * h as f64).floor() as i64) - margin as i64;
    let mut py2 = (((1.0 - min_v) * h as f64).ceil() as i64) + margin as i64;

    px1 = px1.clamp(0, w as i64);
    px2 = px2.clamp(0, w as i64);
    py1 = py1.clamp(0, h as i64);
    py2 = py2.clamp(0, h as i64);
    if px2 <= px1 || py2 <= py1 {
        return None;
    }
    Some((px1 as u32, py1 as u32, px2 as u32, py2 as u32))
}

/// Overwrite `rgb` pixels inside the bounds with one colour.
fn fill_rgb_region(
    rgb: &mut [u8],
    w: u32,
    h: u32,
    bounds: (u32, u32, u32, u32),
    fill: (u8, u8, u8),
) -> usize {
    let (x1, y1, x2, y2) = bounds;
    let mut touched = 0usize;
    for y in y1..y2.min(h) {
        for x in x1..x2.min(w) {
            let i = ((y as usize) * (w as usize) + (x as usize)) * 3;
            if i + 2 < rgb.len() {
                rgb[i] = fill.0;
                rgb[i + 1] = fill.1;
                rgb[i + 2] = fill.2;
                touched += 1;
            }
        }
    }
    touched
}

/// Rewrite a stream with the region filled, encoded losslessly as
/// FlateDecode (DeviceRGB or DeviceGray) — never a lossy JPEG round trip.
fn rewrite_image_stream(
    stream: &mut lopdf::Stream,
    (w, h, mut data): (u32, u32, Vec<u8>),
    bounds: (u32, u32, u32, u32),
    fill: (u8, u8, u8),
    keep_gray: bool,
) -> Result<(), String> {
    if fill_rgb_region(&mut data, w, h, bounds, fill) == 0 {
        return Ok(());
    }
    let payload: Vec<u8> = if keep_gray {
        data.as_chunks::<3>()
            .0
            .iter()
            .map(|c| ((c[0] as u32 * 299 + c[1] as u32 * 587 + c[2] as u32 * 114) / 1000) as u8)
            .collect()
    } else {
        data
    };
    stream.dict.set("BitsPerComponent", Object::Integer(8));
    stream.dict.set(
        "ColorSpace",
        Object::Name(if keep_gray {
            b"DeviceGray".to_vec()
        } else {
            b"DeviceRGB".to_vec()
        }),
    );
    // lopdf's compress() is a no-op while /Filter is present, so clear any
    // stale filter first and let it decide (Flate when it actually shrinks,
    // otherwise a raw unfiltered payload — both lossless and spec-valid).
    stream.dict.remove(b"Filter");
    stream.dict.remove(b"DecodeParms");
    stream.set_content(payload);
    stream
        .compress()
        .map_err(|e| format!("画像の再圧縮に失敗しました: {e}"))?;
    let len = stream.content.len() as i64;
    stream.dict.set("Length", Object::Integer(len));
    Ok(())
}

/// Decode a mask stream that may omit /ColorSpace (implicit DeviceGray).
fn decode_mask_rgb(stream: &lopdf::Stream) -> Result<DecodedImage, String> {
    if stream.dict.get(b"ColorSpace").is_err() {
        let w = stream
            .dict
            .get(b"Width")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0) as u32;
        let h = stream
            .dict
            .get(b"Height")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0) as u32;
        let bpc = stream
            .dict
            .get(b"BitsPerComponent")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(8);
        let raw = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        let need = (w as usize) * (h as usize);
        if w > 0 && h > 0 && bpc == 8 && raw.len() >= need {
            return Ok(DecodedImage {
                width: w,
                height: h,
                rgb: gray_to_rgb(&raw[..need]),
            });
        }
    }
    decode_image_rgb(stream)
}

/// Physically erase pixels inside `rect` for one placed image XObject,
/// including its /SMask and /Mask companions. Returns the number of streams
/// rewritten.
pub fn redact_placement(
    doc: &mut Document,
    placement: &ImagePlacement,
    rect: (f64, f64, f64, f64),
    fill: (u8, u8, u8),
    margin: u32,
) -> Result<usize, String> {
    let (snapshot, smask_id, mask_id) = {
        let Some(Object::Stream(s)) = doc.objects.get(&placement.oid) else {
            return Ok(0);
        };
        (
            s.clone(),
            s.dict
                .get(b"SMask")
                .ok()
                .and_then(|o| o.as_reference().ok()),
            s.dict.get(b"Mask").ok().and_then(|o| o.as_reference().ok()),
        )
    };

    let decoded = decode_image_rgb(&snapshot).map_err(|e| {
        format!(
            "画像XObjectの画素を安全に消去できないため処理を中止しました（{e}）。該当ページを画像として平坦化してから再実行してください"
        )
    })?;
    let (w, h) = (decoded.width, decoded.height);
    let Some(bounds) = pixel_bounds(placement.ctm, rect, w, h, margin) else {
        return Ok(0); // no overlap with this image
    };

    let mut rewritten = 0usize;
    if let Some(Object::Stream(s)) = doc.objects.get_mut(&placement.oid) {
        rewrite_image_stream(s, (w, h, decoded.rgb), bounds, fill, false)?;
        rewritten += 1;
    }

    // Masks: paint the same region fully opaque/visible so a transparency
    // layer can never re-expose the erased area.
    for mask_oid in [smask_id, mask_id].into_iter().flatten() {
        let Ok(mask_decoded) = (match doc.objects.get(&mask_oid) {
            Some(Object::Stream(s)) => decode_mask_rgb(s),
            _ => continue,
        }) else {
            continue;
        };
        let (mw, mh) = (mask_decoded.width, mask_decoded.height);
        if mw == 0 || mh == 0 {
            continue;
        }
        let scaled = (
            ((bounds.0 as f64) * mw as f64 / w as f64).floor().max(0.0) as u32,
            ((bounds.1 as f64) * mh as f64 / h as f64).floor().max(0.0) as u32,
            ((bounds.2 as f64) * mw as f64 / w as f64)
                .ceil()
                .min(mw as f64) as u32,
            ((bounds.3 as f64) * mh as f64 / h as f64)
                .ceil()
                .min(mh as f64) as u32,
        );
        if let Some(Object::Stream(s)) = doc.objects.get_mut(&mask_oid) {
            rewrite_image_stream(s, (mw, mh, mask_decoded.rgb), scaled, (255, 255, 255), true)?;
            rewritten += 1;
        }
    }
    Ok(rewritten)
}

/// Report from a page-level raster redaction pass.
#[derive(Debug, Clone, Default)]
pub struct RasterRedactReport {
    pub placements: usize,
    pub streams_rewritten: usize,
}

/// Fill every image pixel covered by `rect` on `page_id` (PDF points, y-up).
///
/// Returns an error when an intersecting image cannot be decoded: silently
/// leaving recoverable pixels behind would be worse than failing.
pub fn pixel_redact_page(
    doc: &mut Document,
    page_id: ObjectId,
    rect: (f64, f64, f64, f64),
    fill: (u8, u8, u8),
    margin: u32,
) -> Result<RasterRedactReport, String> {
    let placements = collect_page_image_placements(doc, page_id);
    let mut report = RasterRedactReport {
        placements: placements.len(),
        streams_rewritten: 0,
    };
    for placement in &placements {
        report.streams_rewritten += redact_placement(doc, placement, rect, fill, margin)?;
    }
    Ok(report)
}

/// Remove every object that is no longer reachable from the trailer. Leftover
/// copies of a redacted scan would otherwise stay recoverable in the file.
pub fn purge_unreachable_objects(doc: &mut Document) -> usize {
    let mut reachable: HashSet<ObjectId> = HashSet::new();
    let mut stack: Vec<ObjectId> = Vec::new();

    let collect = |obj: &Object, stack: &mut Vec<ObjectId>| collect_refs(obj, stack);
    for (_key, value) in doc.trailer.iter() {
        collect(value, &mut stack);
    }
    if let Some(id) = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|o| o.as_reference().ok())
    {
        stack.push(id);
    }

    while let Some(id) = stack.pop() {
        if !reachable.insert(id) {
            continue;
        }
        if let Some(obj) = doc.objects.get(&id) {
            let obj = obj.clone();
            collect_refs(&obj, &mut stack);
        }
    }

    let before = doc.objects.len();
    doc.objects.retain(|id, _| reachable.contains(id));
    before - doc.objects.len()
}

fn collect_refs(obj: &Object, out: &mut Vec<ObjectId>) {
    match obj {
        Object::Reference(id) => out.push(*id),
        Object::Array(items) => {
            for item in items {
                collect_refs(item, out);
            }
        }
        Object::Dictionary(dict) => {
            for (_k, v) in dict.iter() {
                collect_refs(v, out);
            }
        }
        Object::Stream(stream) => {
            for (_k, v) in stream.dict.iter() {
                collect_refs(v, out);
            }
        }
        _ => {}
    }
}
