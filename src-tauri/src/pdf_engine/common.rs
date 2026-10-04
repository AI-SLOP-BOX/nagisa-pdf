use crate::error::NagisaError;
use lopdf::{Dictionary, Document, Object, Stream};

pub type OID = (u32, u16);

/// Find command binary checking standard paths if not directly in PATH (e.g. GUI app on macOS)
pub fn find_tool_command(name: &str) -> std::process::Command {
    let candidates = vec![
        format!("/opt/homebrew/bin/{name}"),
        format!("/usr/local/bin/{name}"),
        format!("/usr/bin/{name}"),
    ];
    for candidate in &candidates {
        if std::path::Path::new(candidate).exists() {
            return std::process::Command::new(candidate);
        }
    }
    std::process::Command::new(name)
}

/// #42 是正: タイムアウト付き外部コマンド実行ヘルパー。
///
/// `cmd.output()` はブロッキングかつタイムアウトがないため、
/// 細工されたPDFでプロセスが無限待機するDoS脆弱性がある。
/// 本関数では子プロセスを spawn し `timeout_secs` 以内に完了しなければ
/// ハンドル経由で強制 kill して Err を返す。
pub fn run_command_with_timeout(
    mut cmd: std::process::Command,
    timeout_secs: u64,
) -> Result<std::process::Output, NagisaError> {
    let child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| NagisaError::from(format!("Failed to spawn process: {e}")))?;
    wait_child_with_timeout(child, timeout_secs)
}

/// 外部コマンドのデフォルトタイムアウト（秒）。
/// 悪意のあるPDFによる外部プロセスのハング（DoS）を防ぐ。
pub const EXTERNAL_CMD_TIMEOUT_SECS: u64 = 120;

/// PDFバイト列をパースする単一入口。失敗は構造化 `PdfParse` エラーになる。
/// 各所の `Document::load_mem(..).map_err(|e| NagisaError::from(format!(..)))?`（`General` 埋没）
/// をこれに置換し、構文破損と一般失敗の判別を可能にする。
/// ページ範囲外エラーを構造化 `InvalidParameter` で生成する。
/// フロントは専用の範囲外メッセージに変換する（素朴な General 化での文言退行を防止）。
pub fn page_range_err(page_index: usize, total_pages: usize) -> crate::error::NagisaError {
    crate::error::NagisaError::InvalidParameter(format!(
        "Page index {page_index} out of range (total pages: {total_pages})"
    ))
}

pub fn load_pdf(data: &[u8]) -> Result<Document, crate::error::NagisaError> {
    Document::load_mem(data)
        .map_err(|e| crate::error::NagisaError::PdfParse(format!("Failed to load PDF: {e}")))
}

/// True when an image XObject may be destructively re-encoded to JPEG:
/// no transparency companions (/SMask, /Mask) and no print color spaces
/// (DeviceCMYK, Separation) that lossy RGB recompression would destroy.
/// Single source of truth shared by `downsample_images` (print_prod) and
/// `compress_pdf_quality` (convert); previously duplicated with the latter
/// missing the transparency guard.
pub fn image_safe_for_lossy_recompress(dict: &Dictionary) -> bool {
    if dict.has(b"SMask") || dict.has(b"Mask") {
        return false;
    }
    if let Ok(Object::Name(name)) = dict.get(b"ColorSpace") {
        if name == b"DeviceCMYK" || name == b"Separation" {
            return false;
        }
    }
    true
}

/// Composite alpha onto white for JPEG encoding (which has no alpha channel).
/// A direct `to_rgb8()` turns transparent pixels black; shared by
/// `images_to_pdf` and `compress_pdf_quality`.
pub fn flatten_alpha_to_white(img: &image::DynamicImage) -> image::RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut flat = image::RgbImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = f32::from(p[3]) / 255.0;
        flat.put_pixel(
            x,
            y,
            image::Rgb([
                (f32::from(p[0]) * a + 255.0 * (1.0 - a)) as u8,
                (f32::from(p[1]) * a + 255.0 * (1.0 - a)) as u8,
                (f32::from(p[2]) * a + 255.0 * (1.0 - a)) as u8,
            ]),
        );
    }
    flat
}

/// 既に spawn 済みの子プロセスをタイムアウト付きで待機する。
/// stdin 書き込み後に結果回収する経路（tesseract / openssl等）用。
///
/// 設計:
/// - パイプは読取スレッドで draining するため、多弁な子がパイプ満杯で
///   block して誤タイムアウトになることはない。
/// - 完了ポーリングは `try_wait` のみ。待機スレッドは作らない。
/// - kill は `Child` ハンドル経由（PID再利用競合なし・taskkill不要）。
/// - kill 後は短い猶予で reap を試み、呼出側を無期限 block させない。
///   kill が効かない病理的ケースでのみ読取スレッドが残留し得る。
pub fn wait_child_with_timeout(
    mut child: std::process::Child,
    timeout_secs: u64,
) -> Result<std::process::Output, NagisaError> {
    use std::time::{Duration, Instant};

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    // パイプ詰まり deadlock 防止の draining 読取。子終了で EOF し即時完了する。
    fn drain<T: std::io::Read + Send + 'static>(mut io: T) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = io.read_to_end(&mut buf);
            buf
        })
    }
    let out_handle = stdout.map(drain);
    let err_handle = stderr.map(drain);

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let status = loop {
        match child
            .try_wait()
            .map_err(|e| NagisaError::from(format!("Process wait error: {e}")))?
        {
            Some(status) => break status,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    // 猶予付き reap: ゾンビ化を防ぐが呼出側は止めない
                    let grace = Instant::now() + Duration::from_secs(5);
                    while child.try_wait().ok().flatten().is_none() && Instant::now() < grace {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    return Err(NagisaError::from(format!(
                        "External command timed out after {timeout_secs}s. \
                         The input may be malformed or excessively large."
                    )));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    // 子の終了でパイプは EOF するため読取は即時完了する
    let stdout = out_handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = err_handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

/// Encode text string according to ISO 32000-1 §7.9.2.2.
/// If all characters are ASCII (<= 0x7F), returns raw bytes.
/// If non-ASCII characters (e.g. Japanese/CJK/accents) are present, encodes in UTF-16BE with BOM [0xFE, 0xFF].
pub fn encode_pdf_text_string(text: &str) -> Vec<u8> {
    if text.is_ascii() {
        text.as_bytes().to_vec()
    } else {
        let mut bytes = Vec::with_capacity(2 + text.encode_utf16().count() * 2);
        bytes.push(0xFE);
        bytes.push(0xFF);
        for code_unit in text.encode_utf16() {
            bytes.extend_from_slice(&code_unit.to_be_bytes());
        }
        bytes
    }
}

/// Decode text string according to ISO 32000-1 §7.9.2.2.
/// Handles:
/// - UTF-16BE with BOM [0xFE, 0xFF]
/// - Valid UTF-8 strings
/// - PDFDocEncoding / Latin-1 fallback
pub fn decode_pdf_text_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let u16_codes: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&u16_codes)
    } else if let Ok(s) = std::str::from_utf8(bytes) {
        s.to_string()
    } else {
        String::from_utf8_lossy(bytes).to_string()
    }
}

/// Append a new content stream to a page without overwriting existing contents.
/// Handles:
/// - Page with no /Contents (sets as Reference)
/// - Page with single Direct Stream (moves existing to indirect, creates Array [orig, new])
/// - Page with single Indirect Reference (creates Array [orig, new])
/// - Page with existing Array of streams/references (appends new reference preserving order)
pub(crate) fn append_page_content(
    doc: &mut Document,
    page_id: OID,
    new_content_id: OID,
) -> Result<(), NagisaError> {
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or_else(|| NagisaError::from("Page object not found".to_string()))?;
    let page_dict = page_obj
        .as_dict_mut()
        .map_err(|_| "Page is not a dictionary".to_string())?;

    let existing_contents = page_dict.get(b"Contents").ok().cloned();

    let updated_contents = match existing_contents {
        None => Object::Reference(new_content_id),
        Some(Object::Reference(orig_id)) => Object::Array(vec![
            Object::Reference(orig_id),
            Object::Reference(new_content_id),
        ]),
        Some(Object::Array(orig_arr)) => {
            let mut arr = orig_arr;
            arr.push(Object::Reference(new_content_id));
            Object::Array(arr)
        }
        Some(Object::Stream(existing_stream)) => {
            // Direct stream inside page dictionary: allocate as new object, then form array
            let orig_id = doc.add_object(Object::Stream(existing_stream));
            Object::Array(vec![
                Object::Reference(orig_id),
                Object::Reference(new_content_id),
            ])
        }
        Some(other) => {
            // Fallback for any other object representation
            Object::Array(vec![other, Object::Reference(new_content_id)])
        }
    };

    if let Some(page_obj) = doc.objects.get_mut(&page_id) {
        if let Ok(dict) = page_obj.as_dict_mut() {
            dict.set("Contents", updated_contents);
        }
    }

    Ok(())
}

/// Collect all content stream IDs for a page, properly handling:
/// - Single indirect reference: `/Contents 12 0 R`
/// - Array of indirect references: `/Contents [12 0 R, 13 0 R, ...]`
pub(crate) fn resolve_page_content_stream_ids(doc: &Document, page_id: OID) -> Vec<OID> {
    let mut content_ids = Vec::new();
    if let Some(obj) = doc.objects.get(&page_id) {
        if let Ok(dict) = obj.as_dict() {
            match dict.get(b"Contents") {
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
    content_ids
}

/// Recursively resolve and return a copy of the page's resources dictionary,
/// walking up the /Parent tree if necessary to resolve inherited /Resources.
pub(crate) fn resolve_page_resources(doc: &Document, page_id: OID) -> Dictionary {
    let mut current_id = Some(page_id);
    while let Some(cid) = current_id {
        if let Some(Object::Dictionary(dict)) = doc.objects.get(&cid) {
            if let Ok(res_obj) = dict.get(b"Resources") {
                match res_obj {
                    Object::Dictionary(d) => return d.clone(),
                    Object::Reference(r_id) => {
                        if let Some(Object::Dictionary(d)) = doc.objects.get(r_id) {
                            return d.clone();
                        }
                    }
                    _ => {}
                }
            }
            current_id = dict.get(b"Parent").and_then(|p| p.as_reference()).ok();
        } else {
            break;
        }
    }
    Dictionary::new()
}

pub(crate) fn save_doc(doc: &mut Document) -> Result<Vec<u8>, NagisaError> {
    let mut buf = Vec::new();
    doc.save_to(&mut buf)
        .map_err(|e| NagisaError::from(format!("Failed to save: {e}")))?;
    Ok(buf)
}

pub(crate) fn get_page_ids(doc: &Document) -> Vec<OID> {
    super::page_tree::get_logical_page_ids(doc)
}

pub(crate) fn get_page_dimensions(doc: &Document, page_id: OID) -> (f32, f32) {
    if let Some(obj) = doc.objects.get(&page_id) {
        if let Ok(d) = obj.as_dict() {
            if let Ok(Object::Array(arr)) = d.get(b"MediaBox") {
                if arr.len() >= 4 {
                    let w = match &arr[2] {
                        Object::Real(r) => *r,
                        Object::Integer(i) => *i as f32,
                        _ => 595.0,
                    };
                    let h = match &arr[3] {
                        Object::Real(r) => *r,
                        Object::Integer(i) => *i as f32,
                        _ => 842.0,
                    };
                    return (w, h);
                }
            }
        }
    }
    (595.0, 842.0)
}

/// ページの /Rotate（継承解決・正規化済み、{0,90,180,270}）を返す。
/// 存在しない・不正な値は 0 とみなす（ISO 32000-1 §7.7.3.3 継承対応）。
pub fn get_page_rotation(doc: &Document, page_id: OID) -> i32 {
    let attrs = super::page_tree::materialize_inherited_page_attrs(doc, page_id);
    let raw = attrs
        .get(b"Rotate")
        .ok()
        .and_then(|o| match o {
            Object::Integer(v) => Some(*v),
            Object::Real(v) => Some(*v as i64),
            _ => None,
        })
        .unwrap_or(0);
    ((raw % 360 + 360) % 360) as i32
}

/// 表示用ビューポート（視覚寸法＋回転角）。90/270°では縦横が入れ替わる。
/// レンダラ（pdftoppm / PDF.js）は回転適用済み画素を返すため、オーバーレイの
/// 基準寸法は MediaBox ではなくこちらを使う。幾何計算用には
/// `get_page_dimensions`（非回転のまま）を使い分けること。
pub fn get_page_viewport(doc: &Document, page_id: OID) -> (f32, f32, i32) {
    let (w, h) = get_page_dimensions(doc, page_id);
    let r = get_page_rotation(doc, page_id);
    if r == 90 || r == 270 {
        (h, w, r)
    } else {
        (w, h, r)
    }
}

#[allow(dead_code)]
pub(crate) fn get_kids(doc: &Document) -> Option<Vec<Object>> {
    let root_id = doc.trailer.get(b"Root").ok()?.as_reference().ok()?;
    let root = doc.objects.get(&root_id)?;
    let pages_ref = root
        .as_dict()
        .ok()?
        .get(b"Pages")
        .ok()?
        .as_reference()
        .ok()?;
    let pages = doc.objects.get(&pages_ref)?;
    match pages.as_dict().ok()?.get(b"Kids") {
        Ok(Object::Array(kids)) => Some(kids.clone()),
        _ => None,
    }
}

#[allow(dead_code)]
pub(crate) fn set_page_info(doc: &mut Document, kids: Vec<Object>) {
    let root_id = match doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        Ok(id) => id,
        Err(_) => return,
    };
    let pages_id = match doc.objects.get(&root_id) {
        Some(root) => match root.as_dict() {
            Ok(d) => match d.get(b"Pages") {
                Ok(p) => match p.as_reference() {
                    Ok(id) => id,
                    Err(_) => return,
                },
                Err(_) => return,
            },
            Err(_) => return,
        },
        None => return,
    };
    if let Some(Object::Dictionary(ref mut pages_dict)) = doc.objects.get_mut(&pages_id) {
        pages_dict.set("Kids", Object::Array(kids.clone()));
        pages_dict.set("Count", Object::Integer(kids.len() as i64));
    }
}

#[allow(dead_code)]
pub(crate) fn get_page_count(doc: &Document) -> usize {
    let root_id = match doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        Ok(id) => id,
        Err(_) => return 0,
    };
    let pages_id = match doc.objects.get(&root_id) {
        Some(root) => match root.as_dict() {
            Ok(d) => match d.get(b"Pages") {
                Ok(p) => match p.as_reference() {
                    Ok(id) => id,
                    Err(_) => return 0,
                },
                Err(_) => return 0,
            },
            Err(_) => return 0,
        },
        None => return 0,
    };
    match doc.objects.get(&pages_id) {
        Some(pages) => match pages.as_dict() {
            Ok(d) => match d.get(b"Count") {
                Ok(Object::Integer(n)) => *n as usize,
                _ => 0,
            },
            Err(_) => 0,
        },
        None => 0,
    }
}

pub(crate) fn parse_hex_color(color: &str, default: (f32, f32, f32)) -> (f32, f32, f32) {
    let s = color.trim().trim_start_matches('#');
    if s.len() >= 6 {
        let r =
            u8::from_str_radix(&s[0..2], 16).unwrap_or((default.0 * 255.0) as u8) as f32 / 255.0;
        let g =
            u8::from_str_radix(&s[2..4], 16).unwrap_or((default.1 * 255.0) as u8) as f32 / 255.0;
        let b =
            u8::from_str_radix(&s[4..6], 16).unwrap_or((default.2 * 255.0) as u8) as f32 / 255.0;
        (r, g, b)
    } else {
        default
    }
}

#[allow(dead_code)]
pub(crate) fn ensure_page_root(doc: &mut Document) -> OID {
    let (_catalog_id, pages_id) = super::page_tree::ensure_catalog_and_pages_root(doc);
    pages_id
}

pub fn merge_pdfs(paths: &[String]) -> Result<Vec<u8>, NagisaError> {
    super::page_tree::merge_pdfs_robust(paths)
}

pub fn delete_page_in_doc(doc: &mut Document, page_index: usize) -> Result<(), NagisaError> {
    if super::security::doc_has_cryptographic_signatures(doc) {
        return Err(NagisaError::SignedPdfMutationBlocked(
            super::security::SIGNED_PDF_MUTATION_ERROR.to_string(),
        ));
    }
    let mut page_ids = super::page_tree::get_logical_page_ids(doc);
    if page_index >= page_ids.len() {
        return Err(NagisaError::from(format!(
            "Page index {page_index} out of range (total pages: {})",
            page_ids.len()
        )));
    }
    if page_ids.len() <= 1 {
        return Err(NagisaError::from(
            "Cannot delete the only remaining page in the document".to_string(),
        ));
    }
    let removed_pid = page_ids.remove(page_index);
    doc.objects.remove(&removed_pid);
    super::page_tree::rebuild_flat_page_tree(doc, &page_ids)?;
    doc.prune_objects();
    Ok(())
}

pub fn delete_page(data: &[u8], page_index: usize) -> Result<Vec<u8>, NagisaError> {
    super::page_tree::delete_page_robust(data, page_index)
}

pub fn rotate_page_in_doc(
    doc: &mut Document,
    page_index: usize,
    degrees: i32,
) -> Result<(), NagisaError> {
    if super::security::doc_has_cryptographic_signatures(doc) {
        return Err(NagisaError::SignedPdfMutationBlocked(
            super::security::SIGNED_PDF_MUTATION_ERROR.to_string(),
        ));
    }
    let page_ids = super::page_tree::get_logical_page_ids(doc);
    if page_index >= page_ids.len() {
        return Err(page_range_err(page_index, page_ids.len()));
    }
    let page_id = page_ids[page_index];
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        let current = match dict.get(b"Rotate") {
            Ok(Object::Integer(r)) => *r,
            _ => 0,
        };
        let new_rot = (current + degrees as i64).rem_euclid(360);
        dict.set("Rotate", Object::Integer(new_rot));
    }
    Ok(())
}

pub fn rotate_page(data: &[u8], page_index: usize, degrees: i32) -> Result<Vec<u8>, NagisaError> {
    let mut doc = load_pdf(data)?;
    if super::security::doc_has_cryptographic_signatures(&doc) {
        return Err(NagisaError::SignedPdfMutationBlocked(
            super::security::SIGNED_PDF_MUTATION_ERROR.to_string(),
        ));
    }
    rotate_page_in_doc(&mut doc, page_index, degrees)?;
    save_doc(&mut doc)
}

pub fn reorder_pages(
    data: &[u8],
    from_index: usize,
    to_index: usize,
) -> Result<Vec<u8>, NagisaError> {
    super::page_tree::reorder_pages_robust(data, from_index, to_index)
}

pub fn extract_pages(data: &[u8], indices: &[usize]) -> Result<Vec<u8>, NagisaError> {
    super::page_tree::extract_pages_robust(data, indices)
}

pub fn duplicate_page(data: &[u8], page_index: usize) -> Result<Vec<u8>, NagisaError> {
    super::page_tree::duplicate_page_robust(data, page_index)
}

pub fn add_text(
    data: &[u8],
    page_index: usize,
    text: &str,
    x: f64,
    y: f64,
    size: f64,
    color: &str,
) -> Result<Vec<u8>, NagisaError> {
    let mut doc = load_pdf(data)?;
    if super::security::doc_has_cryptographic_signatures(&doc) {
        return Err(NagisaError::SignedPdfMutationBlocked(
            super::security::SIGNED_PDF_MUTATION_ERROR.to_string(),
        ));
    }
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err(page_range_err(page_index, page_ids.len()));
    }

    let (r, g, b) = parse_hex_color(color, (0.0, 0.0, 0.0));
    let page_id = page_ids[page_index];

    let lines: Vec<&str> = text.split('\n').collect();
    let line_height = size * 1.25;

    let is_ascii = text.is_ascii();
    let (font_res_name, font_id, encoded_lines) = if is_ascii {
        // Standard Type1 Helvetica for ASCII
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name("Font".into()));
        font_dict.set("Subtype", Object::Name("Type1".into()));
        font_dict.set("BaseFont", Object::Name("Helvetica".into()));
        let fid = doc.add_object(Object::Dictionary(font_dict));
        let line_objs: Vec<Object> = lines
            .iter()
            .map(|l| Object::String(l.as_bytes().to_vec(), lopdf::StringFormat::Literal))
            .collect();
        (
            format!("NagisaTextHelv_{}_{}", fid.0, fid.1),
            fid,
            line_objs,
        )
    } else {
        // True embedded Type0/CIDFontType2 with real TTF cmap, dynamic widths, and ToUnicode
        let encoder = super::font_unicode::create_unicode_font_encoder(&mut doc, text)?;
        let font_id = encoder.font_id;
        let mut line_objs = Vec::new();
        for line in &lines {
            let line_cids = encoder.encode_text(line);
            line_objs.push(Object::String(line_cids, lopdf::StringFormat::Hexadecimal));
        }
        (
            format!("NagisaUniFont_{}_{}", font_id.0, font_id.1),
            font_id,
            line_objs,
        )
    };

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
    fonts_dict.set(font_res_name.clone(), Object::Reference(font_id));
    resources_dict.set("Font", Object::Dictionary(fonts_dict));
    if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
        page_dict.set("Resources", Object::Dictionary(resources_dict));
    }

    // Wrap operations with q / Q to protect graphics state
    let mut operations = vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new("BT", vec![]),
        lopdf::content::Operation::new(
            "Tf",
            vec![
                Object::Name(font_res_name.into()),
                Object::Real(size as f32),
            ],
        ),
        lopdf::content::Operation::new(
            "rg",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
    ];

    for (i, line_obj) in encoded_lines.into_iter().enumerate() {
        let line_y = (y as f32) - (i as f32 * line_height as f32);
        operations.push(lopdf::content::Operation::new(
            "Tm",
            vec![
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(x as f32),
                Object::Real(line_y),
            ],
        ));
        operations.push(lopdf::content::Operation::new("Tj", vec![line_obj]));
    }

    operations.push(lopdf::content::Operation::new("ET", vec![]));
    operations.push(lopdf::content::Operation::new("Q", vec![]));

    let content = lopdf::content::Content { operations };
    let content_bytes = content
        .encode()
        .map_err(|e| NagisaError::from(format!("Failed to encode content: {e}")))?;

    let mut stream = Stream::new(Dictionary::new(), content_bytes);
    stream.dict.set("Type", Object::Name("Content".into()));
    let content_id = doc.add_object(stream);

    append_page_content(&mut doc, page_id, content_id)?;

    save_doc(&mut doc)
}

pub fn create_blank_pdf(
    width: f64,
    height: f64,
    page_count: usize,
) -> Result<Vec<u8>, NagisaError> {
    let mut doc = Document::with_version("1.7");

    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name("Pages".into()));
    pages_dict.set("Count", Object::Integer(page_count as i64));
    let pages_id = doc.add_object(Object::Dictionary(pages_dict));

    let mut kids = Vec::new();
    for _ in 0..page_count {
        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name("Page".into()));
        page_dict.set("Parent", Object::Reference(pages_id));
        page_dict.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(width as f32),
                Object::Real(height as f32),
            ]),
        );
        let page_id = doc.add_object(Object::Dictionary(page_dict));
        kids.push(Object::Reference(page_id));
    }

    if let Some(Object::Dictionary(ref mut pages)) = doc.objects.get_mut(&pages_id) {
        pages.set("Kids", Object::Array(kids));
    }

    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name("Catalog".into()));
    catalog.set("Pages", Object::Reference(pages_id));
    let catalog_id = doc.add_object(Object::Dictionary(catalog));

    doc.trailer.set("Root", Object::Reference(catalog_id));

    save_doc(&mut doc)
}

pub fn add_image_to_page(
    data: &[u8],
    page_index: usize,
    image_data: &[u8],
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<Vec<u8>, NagisaError> {
    let mut doc = load_pdf(data)?;
    if super::security::doc_has_cryptographic_signatures(&doc) {
        return Err(NagisaError::SignedPdfMutationBlocked(
            super::security::SIGNED_PDF_MUTATION_ERROR.to_string(),
        ));
    }
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err(page_range_err(page_index, page_ids.len()));
    }

    let img = image::load_from_memory(image_data)
        .map_err(|e| NagisaError::from(format!("Failed to decode image: {e}")))?;
    let rgb = img.to_rgb8();
    let img_width = rgb.width();
    let img_height = rgb.height();

    let mut img_dict = Dictionary::new();
    img_dict.set("Type", Object::Name("XObject".into()));
    img_dict.set("Subtype", Object::Name("Image".into()));
    img_dict.set("Width", Object::Integer(img_width as i64));
    img_dict.set("Height", Object::Integer(img_height as i64));
    img_dict.set("ColorSpace", Object::Name("DeviceRGB".into()));
    img_dict.set("BitsPerComponent", Object::Integer(8));
    img_dict.set("Filter", Object::Name("DCTDecode".into()));

    let mut jpg_buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut jpg_buf, image::ImageFormat::Jpeg)
        .map_err(|e| NagisaError::from(format!("Failed to encode JPEG: {e}")))?;
    let jpg_bytes = jpg_buf.into_inner();

    let stream = Stream::new(img_dict, jpg_bytes);
    let img_id = doc.add_object(stream);

    let page_id = page_ids[page_index];

    // Unique XObject resource name based on img_id
    let img_res_name = format!("NagisaImg_{}_{}", img_id.0, img_id.1);

    // Update page resources safely
    let mut resources = resolve_page_resources(&doc, page_id);
    let mut xobjects = match resources.get(b"XObject") {
        Ok(Object::Dictionary(x)) => x.clone(),
        Ok(Object::Reference(x_ref)) => doc
            .objects
            .get(x_ref)
            .and_then(|o| o.as_dict().ok())
            .cloned()
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };
    xobjects.set(img_res_name.clone(), Object::Reference(img_id));
    resources.set("XObject", Object::Dictionary(xobjects));

    if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
        page_dict.set("Resources", Object::Dictionary(resources));
    }

    let operations = vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new(
            "cm",
            vec![
                Object::Real(width as f32),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(height as f32),
                Object::Real(x as f32),
                Object::Real(y as f32),
            ],
        ),
        lopdf::content::Operation::new("Do", vec![Object::Name(img_res_name.into())]),
        lopdf::content::Operation::new("Q", vec![]),
    ];

    let content = lopdf::content::Content { operations };
    let content_bytes = content
        .encode()
        .map_err(|e| NagisaError::from(format!("Failed to encode: {e}")))?;

    let mut content_stream = Stream::new(Dictionary::new(), content_bytes);
    content_stream
        .dict
        .set("Type", Object::Name("Content".into()));
    let content_id = doc.add_object(content_stream);

    append_page_content(&mut doc, page_id, content_id)?;

    save_doc(&mut doc)
}

pub fn crop_page(
    data: &[u8],
    page_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<Vec<u8>, NagisaError> {
    let mut doc = load_pdf(data)?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err(page_range_err(page_index, page_ids.len()));
    }
    let page_id = page_ids[page_index];
    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        dict.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(x as f32),
                Object::Real(y as f32),
                Object::Real((x + width) as f32),
                Object::Real((y + height) as f32),
            ]),
        );
    }
    save_doc(&mut doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn wait_child_returns_output_on_success() {
        let child = std::process::Command::new("echo")
            .arg("hello")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn echo");
        let out = wait_child_with_timeout(child, 10).expect("echo completes");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
    }

    #[test]
    #[cfg(unix)]
    fn wait_child_kills_on_timeout_without_hanging() {
        let child = std::process::Command::new("sleep")
            .arg("30")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn sleep");
        let start = std::time::Instant::now();
        let err = wait_child_with_timeout(child, 1).unwrap_err();
        // タイムアウトは速やかに返る（kill+5秒猶予以内）かつ分類はTimeout相当
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        assert!(err.message().contains("timed out"));
    }

    #[test]
    #[cfg(unix)]
    fn run_command_captures_large_stdout_without_deadlock() {
        // パイプ満杯（64KB超）でも draining 読取で詰まらないこと
        let output = run_command_with_timeout(
            {
                let mut c = std::process::Command::new("sh");
                c.args(["-c", "yes | head -c 200000"]);
                c
            },
            15,
        )
        .expect("large output completes");
        assert!(output.status.success());
        assert!(output.stdout.len() >= 200000);
    }
}
