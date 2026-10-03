use super::common::*;
use super::reflow::get_char_metric_width;
use lopdf::{Dictionary, Document, Object, Stream};
use crate::error::NagisaError;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TextBlock {
    pub id: usize,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_name: String,
    pub font_size: f32,
    pub color: String,
    pub page_index: usize,
}

// ---------------------------------------------------------------------------
// #37 解消: ストリーム走査の共通ロジックを単一関数に集約し、
//           get / edit / delete / move が完全に同じカウント基準を使う。
//
// ブロック境界ルール: BT〜ET の間に Tj または TJ の文字列オペランドが
// 1 つ以上存在した場合のみ、ET 時に current_block をインクリメントする。
// text_buffer が空の場合はインクリメントしない（空テキストブロックを無視）。
// ---------------------------------------------------------------------------

/// 実フォント幅によるテキスト幅計測。
/// 単純フォントの /Widths + /FirstChar、CIDフォントの /DW（DescendantFonts経由）
/// を参照し、解決できない文字のみ get_char_metric_width のヒューリスティクスに
/// フォールバックする。font_dict が None の場合は全面ヒューリスティクス。
///
/// 既知の制限: Unicodeコードポイントをそのまま /Widths 添字に使うため、
/// /Encoding /Differences で再割当てされたフォントではずれる（Latin-1 範囲は
/// 偶然一致することが多い）。実測化で信頼度は上がったが完全保証ではない。
/// Differences 対応が必要になったら /Encoding 辞書の差分配列を解決すること。
pub(crate) fn measure_text_width(
    doc: &Document,
    font_dict: Option<&Dictionary>,
    text: &str,
    font_size: f32,
) -> f32 {
    let heuristic: f32 = text
        .chars()
        .map(|c| get_char_metric_width(c, font_size))
        .sum();
    let fd = match font_dict {
        Some(d) => d,
        None => return heuristic,
    };
    let num = |o: &Object| -> Option<f32> {
        match o {
            Object::Real(v) => Some(*v),
            Object::Integer(v) => Some(*v as f32),
            _ => None,
        }
    };
    // CID/Type0: Unicode→CID逆引きを持たないため /DW 一律が最善の近似
    if let Ok(Object::Name(sub)) = fd.get(b"Subtype") {
        if sub == b"Type0" {
            if let Some(dw) = descendant_default_width(doc, fd) {
                return text.chars().count() as f32 * dw / 1000.0 * font_size;
            }
            return heuristic;
        }
    }
    // 単純フォント: /Widths テーブル（間接参照の場合あり）
    let first = fd.get(b"FirstChar").ok().and_then(num).unwrap_or(32.0) as u32;
    let widths: Vec<f32> = match fd.get(b"Widths") {
        Ok(Object::Array(arr)) => arr.iter().filter_map(num).collect(),
        Ok(Object::Reference(id)) => match doc.objects.get(id) {
            Some(Object::Array(arr)) => arr.iter().filter_map(num).collect(),
            _ => return heuristic,
        },
        _ => return heuristic,
    };
    if widths.is_empty() {
        return heuristic;
    }
    let mut total = 0.0f32;
    let mut measured = false;
    for c in text.chars() {
        let code = c as u32;
        if code < 256 && code >= first {
            if let Some(w) = widths.get((code - first) as usize) {
                if *w > 0.0 {
                    total += *w / 1000.0 * font_size;
                    measured = true;
                    continue;
                }
            }
        }
        total += get_char_metric_width(c, font_size);
    }
    if measured {
        total
    } else {
        heuristic
    }
}

fn descendant_default_width(doc: &Document, font_dict: &Dictionary) -> Option<f32> {
    let arr = match font_dict.get(b"DescendantFonts").ok()? {
        Object::Array(a) => a,
        _ => return None,
    };
    let cid = match arr.first()? {
        Object::Reference(id) => doc.objects.get(id)?.as_dict().ok()?,
        _ => return None,
    };
    match cid.get(b"DW").ok()? {
        Object::Real(v) => Some(*v),
        Object::Integer(v) => Some(*v as f32),
        _ => None,
    }
}

/// ページリソース辞書からフォント辞書を解決する（間接参照対応）。
pub(crate) fn lookup_font_dict(
    doc: &Document,
    res: Option<&Dictionary>,
    key: &[u8],
) -> Option<Dictionary> {
    let res = res?;
    let font_sub = match res.get(b"Font").ok()? {
        Object::Reference(id) => doc.objects.get(id)?.as_dict().ok()?.clone(),
        Object::Dictionary(d) => d.clone(),
        _ => return None,
    };
    match font_sub.get(key).ok()? {
        Object::Reference(id) => doc.objects.get(id)?.as_dict().ok().cloned(),
        Object::Dictionary(d) => Some(d.clone()),
        _ => None,
    }
}

/// ページ上の全テキストブロックを取得する。
/// #36 解消: Tj/TJ の生バイト列を decode_pdf_text_string に通し、
///           CMap/UTF-16BE/WinAnsi を正しく解決してUIに渡す。
pub fn get_text_blocks_from_doc(
    doc: &Document,
    page_index: usize,
) -> Result<Vec<TextBlock>, NagisaError> {
    let page_ids = get_page_ids(doc);
    if page_index >= page_ids.len() {
        return Err(page_range_err(page_index, page_ids.len()));
    }

    let page_id = page_ids[page_index];
    let page_attrs = super::page_tree::materialize_inherited_page_attrs(doc, page_id);
    let res_dict: Option<Dictionary> = page_attrs.get(b"Resources").ok().and_then(|r| match r {
        Object::Reference(id) => doc.objects.get(id).and_then(|o| o.as_dict().ok()).cloned(),
        Object::Dictionary(d) => Some(d.clone()),
        _ => None,
    });
    let mut blocks = Vec::new();
    let mut block_id = 0usize;

    // 全コンテンツストリームを連結して単一のオペレーション列として処理する (#37)
    let content_ids = resolve_page_content_stream_ids(doc, page_id);
    let mut all_operations: Vec<lopdf::content::Operation> = Vec::new();
    for cid in &content_ids {
        if let Some(Object::Stream(stream)) = doc.objects.get(cid) {
            let bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            if let Ok(c) = lopdf::content::Content::decode(&bytes) {
                all_operations.extend(c.operations);
            }
        }
    }

    let mut current_x = 0.0f32;
    let mut current_y = 0.0f32;
    let mut current_font = String::new();
    let mut current_font_key: Vec<u8> = Vec::new();
    let mut current_size = 12.0f32;
    let mut current_color = "#000000".to_string();
    let mut in_text = false;
    // #37: text_buffer はブロックが "空かどうか" の判定にのみ使用
    let mut has_text_in_block = false;
    let mut text_buffer = String::new();
    let mut text_start_x = 0.0f32;
    let mut text_start_y = 0.0f32;

    for op in &all_operations {
        match op.operator.as_str() {
            "BT" => {
                in_text = true;
                has_text_in_block = false;
                text_buffer.clear();
                text_start_x = current_x;
                text_start_y = current_y;
            }
            "ET" => {
                if in_text && has_text_in_block {
                    let font_dict = lookup_font_dict(doc, res_dict.as_ref(), &current_font_key);
                    let calc_width: f32 =
                        measure_text_width(doc, font_dict.as_ref(), &text_buffer, current_size);
                    blocks.push(TextBlock {
                        id: block_id,
                        text: text_buffer.clone(),
                        x: text_start_x,
                        y: text_start_y - current_size,
                        width: calc_width,
                        height: current_size * 1.2,
                        font_name: current_font.clone(),
                        font_size: current_size,
                        color: current_color.clone(),
                        page_index,
                    });
                    block_id += 1;
                }
                in_text = false;
                has_text_in_block = false;
                text_buffer.clear();
            }
            "Tf" => {
                // フォント名はリソース名（/F1 等）。サイズのみ確実に取得する。
                if let Some(Object::Name(font)) = op.operands.first() {
                    current_font = String::from_utf8_lossy(font).to_string();
                    current_font_key = font.clone();
                }
                // サイズは Integer または Real
                let size_obj = op.operands.get(1);
                if let Some(obj) = size_obj {
                    current_size = match obj {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => current_size,
                    };
                }
            }
            "Tm" => {
                if op.operands.len() >= 6 {
                    current_x = match &op.operands[4] {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => current_x,
                    };
                    current_y = match &op.operands[5] {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => current_y,
                    };
                    if in_text {
                        text_start_x = current_x;
                        text_start_y = current_y;
                    }
                }
            }
            "Td" | "TD" => {
                if let (Some(dx_obj), Some(dy_obj)) = (op.operands.first(), op.operands.get(1)) {
                    let dx = match dx_obj {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => 0.0,
                    };
                    let dy = match dy_obj {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => 0.0,
                    };
                    current_x += dx;
                    current_y += dy;
                }
            }
            "rg" => {
                if op.operands.len() >= 3 {
                    let r = match &op.operands[0] {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => 0.0,
                    };
                    let g = match &op.operands[1] {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => 0.0,
                    };
                    let b = match &op.operands[2] {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => 0.0,
                    };
                    let ri = (r * 255.0) as u8;
                    let gi = (g * 255.0) as u8;
                    let bi = (b * 255.0) as u8;
                    current_color = format!("#{:02x}{:02x}{:02x}", ri, gi, bi);
                }
            }
            // #36 解消: decode_pdf_text_string を使用してマルチバイト文字を正しく解決
            "Tj" => {
                if in_text {
                    if let Some(Object::String(bytes, _)) = op.operands.first() {
                        let decoded = decode_pdf_text_string(bytes);
                        text_buffer.push_str(&decoded);
                        if !decoded.is_empty() {
                            has_text_in_block = true;
                        }
                    }
                }
            }
            "TJ" if in_text => {
                if let Some(Object::Array(arr)) = op.operands.first() {
                    for item in arr {
                        if let Object::String(bytes, _) = item {
                            let decoded = decode_pdf_text_string(bytes);
                            text_buffer.push_str(&decoded);
                            if !decoded.is_empty() {
                                has_text_in_block = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(blocks)
}

pub fn get_text_blocks(data: &[u8], page_index: usize) -> Result<Vec<TextBlock>, NagisaError> {
    let doc = load_pdf(data)?;
    get_text_blocks_from_doc(&doc, page_index)
}

// ---------------------------------------------------------------------------
// #34/#37 解消: edit_text_block
//   - ブロックカウントを get_text_blocks_from_doc と完全一致させる
//   - 日本語等の非ASCII文字は create_unicode_font_encoder 経由で CID 埋め込み
//   - ASCII のみの場合は既存フォントのまま WinAnsi Literal で安全に記述
// ---------------------------------------------------------------------------
pub fn edit_text_block(
    data: &[u8],
    page_index: usize,
    block_id: usize,
    new_text: &str,
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

    let page_id = page_ids[page_index];

    let content_ids = resolve_page_content_stream_ids(&doc, page_id);
    if content_ids.is_empty() {
        return Err("No content stream".into());
    }

    // 全ストリームを結合 (#37: get と同じ方式)
    let mut all_operations: Vec<lopdf::content::Operation> = Vec::new();
    for cid in &content_ids {
        if let Some(Object::Stream(stream)) = doc.objects.get(cid) {
            let bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            if let Ok(c) = lopdf::content::Content::decode(&bytes) {
                all_operations.extend(c.operations);
            }
        }
    }

    // #34 解消: 非ASCII文字を含む場合は Unicode フォントエンコーダーを用意
    let has_non_ascii = !new_text.is_ascii();

    let unicode_encoder = if has_non_ascii {
        Some(
            crate::pdf_engine::font_unicode::create_unicode_font_encoder(&mut doc, new_text)
                .map_err(|e| NagisaError::from(format!("Unicode font encoder error: {e}")))?,
        )
    } else {
        None
    };

    // フォントリソース名（日本語時のみ使用）
    let unicode_font_res = "NagisaEditFont";

    let mut new_operations: Vec<lopdf::content::Operation> = Vec::new();
    // #37: get_text_blocks_from_doc と同一カウントロジック
    let mut current_block = 0usize;
    let mut in_text = false;
    let mut has_text_in_block = false;
    let mut in_target_block = false;
    let mut block_replaced = false;
    // #40 是正: ブロックの元フォントサイズを追跡して push_replacement に伝播する
    let mut current_font_size = 12.0f32;

    for op in &all_operations {
        match op.operator.as_str() {
            "BT" => {
                in_text = true;
                has_text_in_block = false;
                in_target_block = current_block == block_id;
                new_operations.push(op.clone());
            }
            "ET" => {
                // ターゲットブロックでまだ置換が起きていない（Tj/TJ が全くなかった）場合は
                // ET 直前に挿入する
                if in_target_block && !block_replaced {
                    push_replacement(
                        &mut new_operations,
                        new_text,
                        unicode_encoder.as_ref(),
                        unicode_font_res,
                        current_font_size,
                    );
                    block_replaced = true;
                }
                // #37: get と同一カウント基準
                if in_text && has_text_in_block {
                    current_block += 1;
                }
                in_text = false;
                has_text_in_block = false;
                in_target_block = false;
                new_operations.push(op.clone());
            }
            "Tf" => {
                // #40 是正: フォントサイズを追跡
                let size_obj = op.operands.get(1);
                if let Some(obj) = size_obj {
                    current_font_size = match obj {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => current_font_size,
                    };
                }
                new_operations.push(op.clone());
            }
            "Tj" => {
                if in_text {
                    if let Some(Object::String(bytes, _)) = op.operands.first() {
                        let decoded = decode_pdf_text_string(bytes);
                        if !decoded.is_empty() {
                            has_text_in_block = true;
                        }
                    }
                }
                if in_target_block {
                    if !block_replaced {
                        push_replacement(
                            &mut new_operations,
                            new_text,
                            unicode_encoder.as_ref(),
                            unicode_font_res,
                            current_font_size,
                        );
                        block_replaced = true;
                    }
                    // 元の Tj は出力しない（置換済み）
                } else {
                    new_operations.push(op.clone());
                }
            }
            "TJ" => {
                if in_text {
                    if let Some(Object::Array(arr)) = op.operands.first() {
                        for item in arr {
                            if let Object::String(bytes, _) = item {
                                if !decode_pdf_text_string(bytes).is_empty() {
                                    has_text_in_block = true;
                                }
                            }
                        }
                    }
                }
                if in_target_block {
                    if !block_replaced {
                        push_replacement(
                            &mut new_operations,
                            new_text,
                            unicode_encoder.as_ref(),
                            unicode_font_res,
                            current_font_size,
                        );
                        block_replaced = true;
                    }
                    // 元の TJ は出力しない（置換済み）
                } else {
                    new_operations.push(op.clone());
                }
            }
            _ => {
                new_operations.push(op.clone());
            }
        }
    }

    // フォントリソースの登録（日本語埋め込み時のみ）
    if let Some(ref encoder) = unicode_encoder {
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
        fonts_dict.set(unicode_font_res, Object::Reference(encoder.font_id));
        resources_dict.set("Font", Object::Dictionary(fonts_dict));
        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.set("Resources", Object::Dictionary(resources_dict));
        }
    }

    let content = lopdf::content::Content {
        operations: new_operations,
    };
    let content_bytes = content.encode().map_err(|e| NagisaError::from(format!("Encode error: {e}")))?;

    let mut stream = Stream::new(Dictionary::new(), content_bytes);
    stream.dict.set("Type", Object::Name("Content".into()));
    let new_content_id = doc.add_object(stream);

    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        dict.set("Contents", Object::Reference(new_content_id));
    }

    save_doc(&mut doc)
}

/// 置換テキストをオペレーション列に追加するヘルパー。
/// 日本語等の非ASCII文字は CID Hex 形式、ASCII は Literal 形式で出力。
/// #40 是正: font_size に元ブロックの実際のフォントサイズを受け取り、
/// ハードコード 12.0 を排除してレイアウト崩れを防止する。
fn push_replacement(
    ops: &mut Vec<lopdf::content::Operation>,
    new_text: &str,
    unicode_encoder: Option<&crate::pdf_engine::font_unicode::UnicodeFontEncoder>,
    font_res_name: &str,
    font_size: f32,
) {
    if let Some(encoder) = unicode_encoder {
        // 日本語: Type0/Identity-H フォントに切り替えて CID Hex 文字列で挿入
        // font_size は元ブロックの Tf から伝播した値を使用 (#40 是正)
        ops.push(lopdf::content::Operation::new(
            "Tf",
            vec![Object::Name(font_res_name.into()), Object::Real(font_size)],
        ));
        let cid_bytes = encoder.encode_text(new_text);
        ops.push(lopdf::content::Operation::new(
            "Tj",
            vec![Object::String(cid_bytes, lopdf::StringFormat::Hexadecimal)],
        ));
    } else {
        // ASCII のみ: 既存フォント継承、WinAnsi Literal
        let safe_bytes: Vec<u8> = new_text
            .chars()
            .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
            .collect();
        ops.push(lopdf::content::Operation::new(
            "Tj",
            vec![Object::String(safe_bytes, lopdf::StringFormat::Literal)],
        ));
    }
}

// ---------------------------------------------------------------------------
// move_text_block (#37 整合: get と同一カウントロジック)
// ---------------------------------------------------------------------------
pub fn move_text_block(
    data: &[u8],
    page_index: usize,
    block_id: usize,
    new_x: f32,
    new_y: f32,
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

    let page_id = page_ids[page_index];

    let content_ids = resolve_page_content_stream_ids(&doc, page_id);
    if content_ids.is_empty() {
        return Err("No content stream".into());
    }

    let mut all_operations: Vec<lopdf::content::Operation> = Vec::new();
    for cid in &content_ids {
        if let Some(Object::Stream(stream)) = doc.objects.get(cid) {
            let bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            if let Ok(c) = lopdf::content::Content::decode(&bytes) {
                all_operations.extend(c.operations);
            }
        }
    }

    let num = |o: &Object| -> f32 {
        match o {
            Object::Real(v) => *v,
            Object::Integer(v) => *v as f32,
            _ => 0.0,
        }
    };

    let mut new_operations: Vec<lopdf::content::Operation> = Vec::new();
    let mut current_block = 0usize;
    let mut in_text = false;
    let mut has_text_in_block = false;
    let mut in_target_block = false;
    // ブロック内の2つ目以降の Tm/Td は相対送り（改行・字送り）のため、
    // 書き換えるのは最初の位置決めオペレータのみとする。全置換は複数行
    // ブロックを1点に潰す。Tm の a/b/c/d（回転・拡縮・傾斜）は保持し、
    // 平行移動成分 e/f のみ new_x/new_y に差し替える。
    let mut position_rewritten = false;

    for op in &all_operations {
        match op.operator.as_str() {
            "BT" => {
                in_text = true;
                has_text_in_block = false;
                in_target_block = current_block == block_id;
                position_rewritten = false;
                new_operations.push(op.clone());
            }
            "ET" => {
                if in_text && has_text_in_block {
                    current_block += 1;
                }
                in_text = false;
                has_text_in_block = false;
                in_target_block = false;
                new_operations.push(op.clone());
            }
            "Tm" => {
                if in_target_block && !position_rewritten && op.operands.len() >= 6 {
                    let (a, b, c, d) = (
                        num(&op.operands[0]),
                        num(&op.operands[1]),
                        num(&op.operands[2]),
                        num(&op.operands[3]),
                    );
                    new_operations.push(lopdf::content::Operation::new(
                        "Tm",
                        vec![
                            Object::Real(a),
                            Object::Real(b),
                            Object::Real(c),
                            Object::Real(d),
                            Object::Real(new_x),
                            Object::Real(new_y),
                        ],
                    ));
                    position_rewritten = true;
                } else {
                    new_operations.push(op.clone());
                }
            }
            "Td" | "TD" => {
                if in_target_block && !position_rewritten {
                    // Td/TD を Tm に変換して絶対座標で移動
                    new_operations.push(lopdf::content::Operation::new(
                        "Tm",
                        vec![
                            Object::Real(1.0),
                            Object::Real(0.0),
                            Object::Real(0.0),
                            Object::Real(1.0),
                            Object::Real(new_x),
                            Object::Real(new_y),
                        ],
                    ));
                    position_rewritten = true;
                } else {
                    new_operations.push(op.clone());
                }
            }
            "Tj" => {
                if in_text {
                    if let Some(Object::String(bytes, _)) = op.operands.first() {
                        if !decode_pdf_text_string(bytes).is_empty() {
                            has_text_in_block = true;
                        }
                    }
                }
                new_operations.push(op.clone());
            }
            "TJ" => {
                if in_text {
                    if let Some(Object::Array(arr)) = op.operands.first() {
                        for item in arr {
                            if let Object::String(bytes, _) = item {
                                if !decode_pdf_text_string(bytes).is_empty() {
                                    has_text_in_block = true;
                                }
                            }
                        }
                    }
                }
                new_operations.push(op.clone());
            }
            _ => {
                new_operations.push(op.clone());
            }
        }
    }

    let content = lopdf::content::Content {
        operations: new_operations,
    };
    let content_bytes = content.encode().map_err(|e| NagisaError::from(format!("Encode error: {e}")))?;

    let mut stream = Stream::new(Dictionary::new(), content_bytes);
    stream.dict.set("Type", Object::Name("Content".into()));
    let new_content_id = doc.add_object(stream);

    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        dict.set("Contents", Object::Reference(new_content_id));
    }

    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// delete_text_block (#37 整合: get と同一カウントロジック)
// ---------------------------------------------------------------------------
pub fn delete_text_block(
    data: &[u8],
    page_index: usize,
    block_id: usize,
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

    let page_id = page_ids[page_index];

    let content_ids = resolve_page_content_stream_ids(&doc, page_id);
    if content_ids.is_empty() {
        return Err("No content stream".into());
    }

    let mut all_operations: Vec<lopdf::content::Operation> = Vec::new();
    for cid in &content_ids {
        if let Some(Object::Stream(stream)) = doc.objects.get(cid) {
            let bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            if let Ok(c) = lopdf::content::Content::decode(&bytes) {
                all_operations.extend(c.operations);
            }
        }
    }

    let mut new_operations: Vec<lopdf::content::Operation> = Vec::new();
    let mut current_block = 0usize;
    let mut skip_block = false;
    let mut in_text = false;
    let mut has_text_in_block = false;

    for op in &all_operations {
        match op.operator.as_str() {
            "BT" => {
                in_text = true;
                has_text_in_block = false;
                skip_block = current_block == block_id;
                if !skip_block {
                    new_operations.push(op.clone());
                }
            }
            "ET" => {
                if !skip_block {
                    new_operations.push(op.clone());
                }
                if in_text && has_text_in_block {
                    current_block += 1;
                }
                in_text = false;
                has_text_in_block = false;
                skip_block = false;
            }
            "Tj" => {
                if in_text {
                    if let Some(Object::String(bytes, _)) = op.operands.first() {
                        if !decode_pdf_text_string(bytes).is_empty() {
                            has_text_in_block = true;
                        }
                    }
                }
                if !skip_block {
                    new_operations.push(op.clone());
                }
            }
            "TJ" => {
                if in_text {
                    if let Some(Object::Array(arr)) = op.operands.first() {
                        for item in arr {
                            if let Object::String(bytes, _) = item {
                                if !decode_pdf_text_string(bytes).is_empty() {
                                    has_text_in_block = true;
                                }
                            }
                        }
                    }
                }
                if !skip_block {
                    new_operations.push(op.clone());
                }
            }
            _ => {
                if !skip_block {
                    new_operations.push(op.clone());
                }
            }
        }
    }

    let content = lopdf::content::Content {
        operations: new_operations,
    };
    let content_bytes = content.encode().map_err(|e| NagisaError::from(format!("Encode error: {e}")))?;

    let mut stream = Stream::new(Dictionary::new(), content_bytes);
    stream.dict.set("Type", Object::Name("Content".into()));
    let new_content_id = doc.add_object(stream);

    if let Some(Object::Dictionary(ref mut dict)) = doc.objects.get_mut(&page_id) {
        dict.set("Contents", Object::Reference(new_content_id));
    }

    save_doc(&mut doc)
}
