use super::common::*;
use lopdf::{Dictionary, Document, Object, Stream};

// ===== JIS X 4051 準拠 日本語禁則判定 & プロポーショナルグリフ幅 =====

pub(crate) fn is_kinsoku_line_start(c: char) -> bool {
    matches!(
        c,
        '、' | '。'
            | '，'
            | '．'
            | '・'
            | '：'
            | '；'
            | '？'
            | '！'
            | '）'
            | '］'
            | '｝'
            | '〉'
            | '》'
            | '」'
            | '』'
            | '】'
            | '〕'
            | '〟'
            | 'ヽ'
            | 'ヾ'
            | 'ゝ'
            | 'ゞ'
            | '々'
            | 'ー'
            | 'ァ'
            | 'ィ'
            | 'ゥ'
            | 'ェ'
            | 'ォ'
            | 'ッ'
            | 'ャ'
            | 'ュ'
            | 'ョ'
            | 'ヮ'
            | 'ヵ'
            | 'ヶ'
            | 'ぁ'
            | 'ぃ'
            | 'ぅ'
            | 'ぇ'
            | 'ぉ'
            | 'っ'
            | 'ゃ'
            | 'ゅ'
            | 'ょ'
            | 'ゎ'
            | '℃'
            | '％'
            | '‰'
    )
}

pub(crate) fn is_kinsoku_line_end(c: char) -> bool {
    matches!(
        c,
        '（' | '［'
            | '｛'
            | '〈'
            | '《'
            | '「'
            | '『'
            | '【'
            | '〔'
            | '\''
            | '"'
            | '￥'
            | '＄'
            | '￡'
            | '＃'
    )
}

pub fn get_char_metric_width(c: char, font_size: f32) -> f32 {
    let scale = font_size;
    match c {
        // 半角スペース
        ' ' => scale * 0.28,
        // 全角スペース
        '\u{3000}' => scale * 1.0,
        // 欧文文字（プロポーショナル幅）
        'i' | 'l' | 'I' | 'j' | '!' | '.' | ':' | ';' | '\'' => scale * 0.28,
        'f' | 'r' | 't' | '(' | ')' | '[' | ']' => scale * 0.35,
        'm' | 'w' | 'M' | 'W' => scale * 0.85,
        c if c.is_ascii_alphanumeric() => scale * 0.55,
        c if c.is_ascii_punctuation() => scale * 0.40,
        // 句読点（全角だが約物詰めを考慮）
        '、' | '。' | '，' | '．' => scale * 0.65,
        '「' | '」' | '『' | '』' | '（' | '）' => scale * 0.60,
        // CJK全角文字（漢字・ひらがな・カタカナ等）
        _ => scale * 1.0,
    }
}

// ---------------------------------------------------------------------------
// #35 解消: reflow_text の二重印字（元テキスト残存）バグの是正
//
// 問題の実態:
//   従来実装は append_page_content で新テキストストリームを追加するだけで、
//   既存の BT〜ET テキストオペレータが背面に残り、文字が重なって真っ黒になっていた。
//
// 是正方針:
//   1. 既存コンテンツストリームを走査し、start_x/start_y/max_width/行高さで
//      定義される「リフロー対象矩形」内に位置するテキストブロック（BT〜ET）を
//      すべて除去（ストリームから削除）したうえで単一の unified ストリームに再構成する。
//   2. 除去後、新テキストを Unicode フォント（日本語）または Helvetica（ASCII）で
//      同位置に上書きする。
//   3. append_page_content を廃止し、unified 置換ストリームのみをページに設定する。
//
// リフロー矩形の判定:
//   BT 直後の Tm オペレータの (x, y) が以下の条件を満たすブロックを対象とする。
//     - x_start <= x <= x_start + max_width + margin(20pt)
//     - y_start - total_block_height - margin(20pt) <= y <= y_start + margin(20pt)
//   ただし、矩形が指定されない場合（max_width <= 0）は削除を行わない（純 Insert モード）。
// ---------------------------------------------------------------------------
pub fn reflow_text(
    data: &[u8],
    page_index: usize,
    new_text: &str,
    start_x: f64,
    start_y: f64,
    max_width: f64,
    font_size: f32,
    line_height: f32,
    color: &str,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let (r, g, b) = parse_hex_color(color, (0.0, 0.0, 0.0));

    let target_max_width = max_width as f32;
    let mut wrapped_lines: Vec<String> = Vec::new();

    // 段落ごとに分割して高度組版（JIS X 4051準拠 禁則処理 ＋ プロポーショナル幅計算）
    for raw_paragraph in new_text.split('\n') {
        if raw_paragraph.is_empty() {
            wrapped_lines.push(String::new());
            continue;
        }

        let chars: Vec<char> = raw_paragraph.chars().collect();
        let mut current_line = String::new();
        let mut current_line_width = 0.0f32;
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            let char_w = get_char_metric_width(c, font_size);

            // 欧文単語の場合は単語単位で折り返しを保護
            if c.is_ascii_alphanumeric() {
                let mut word = String::new();
                let mut word_w = 0.0f32;
                let mut j = i;
                while j < chars.len() && chars[j].is_ascii_alphanumeric() {
                    word.push(chars[j]);
                    word_w += get_char_metric_width(chars[j], font_size);
                    j += 1;
                }

                if current_line_width + word_w > target_max_width && !current_line.is_empty() {
                    wrapped_lines.push(current_line);
                    current_line = word;
                    current_line_width = word_w;
                } else {
                    current_line.push_str(&word);
                    current_line_width += word_w;
                }
                i = j;
                continue;
            }

            // 通常のCJKまたは記号文字の幅判定
            if current_line_width + char_w > target_max_width && !current_line.is_empty() {
                // 行頭禁則処理：次に来る文字が行頭禁則文字の場合、前の行末文字を次の行へ巻き込む（追い出し）
                if is_kinsoku_line_start(c) {
                    if let Some(prev_char) = current_line.pop() {
                        wrapped_lines.push(current_line);
                        current_line = String::new();
                        current_line.push(prev_char);
                        current_line.push(c);
                        current_line_width = get_char_metric_width(prev_char, font_size) + char_w;
                        i += 1;
                        continue;
                    }
                }

                wrapped_lines.push(current_line);
                current_line = String::new();
                current_line.push(c);
                current_line_width = char_w;
            } else {
                // 行末禁則処理：現在の行末に置いてはいけない文字（「、『 など）が最後の文字になる場合
                if is_kinsoku_line_end(c)
                    && (current_line_width + char_w + font_size > target_max_width)
                {
                    if !current_line.is_empty() {
                        wrapped_lines.push(current_line);
                        current_line = String::new();
                    }
                }
                current_line.push(c);
                current_line_width += char_w;
            }

            i += 1;
        }

        if !current_line.is_empty() {
            wrapped_lines.push(current_line);
        }
    }

    let num_lines = wrapped_lines.len();
    // リフロー矩形の縦範囲: start_y から下方向に num_lines * line_height まで
    let total_block_height = (num_lines.max(1) as f32) * line_height;

    // #35 是正: 既存コンテンツストリームからリフロー対象矩形内のテキストを除去する
    let page_id = page_ids[page_index];
    let content_ids = resolve_page_content_stream_ids(&doc, page_id);

    // max_width > 0 のときのみ既存テキストを除去する（置換 Replace モード）
    let should_remove_existing = max_width > 0.0;

    let mut base_operations: Vec<lopdf::content::Operation> = Vec::new();
    for cid in &content_ids {
        if let Some(Object::Stream(stream)) = doc.objects.get(cid) {
            let bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            if let Ok(c) = lopdf::content::Content::decode(&bytes) {
                base_operations.extend(c.operations);
            }
        }
    }

    let cleaned_operations = if should_remove_existing {
        remove_text_in_rect(
            base_operations,
            start_x as f32,
            start_y as f32,
            target_max_width,
            total_block_height,
        )
    } else {
        base_operations
    };

    // Check if text contains non-ASCII (e.g. Japanese Kanji/Kana/Hiragana)
    let has_non_ascii = new_text.chars().any(|c| !c.is_ascii());

    if has_non_ascii {
        // Genuine CJK TrueType Embedding Pipeline (IPAexGothic / Type0 / Identity-H / ToUnicode)
        let encoder =
            crate::pdf_engine::font_unicode::create_unicode_font_encoder(&mut doc, new_text)?;
        let font_id = encoder.font_id;

        let font_res_name = "NagisaReflowFont";
        let mut new_text_ops = vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new("BT", vec![]),
            lopdf::content::Operation::new(
                "Tf",
                vec![Object::Name(font_res_name.into()), Object::Real(font_size)],
            ),
            lopdf::content::Operation::new(
                "rg",
                vec![Object::Real(r), Object::Real(g), Object::Real(b)],
            ),
        ];

        for (i, line) in wrapped_lines.iter().enumerate() {
            let line_y = (start_y as f32) - (i as f32 * line_height);
            new_text_ops.push(lopdf::content::Operation::new(
                "Tm",
                vec![
                    Object::Real(1.0),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(1.0),
                    Object::Real(start_x as f32),
                    Object::Real(line_y),
                ],
            ));

            let line_cid_bytes = encoder.encode_text(line);

            new_text_ops.push(lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(
                    line_cid_bytes,
                    lopdf::StringFormat::Hexadecimal,
                )],
            ));
        }

        new_text_ops.push(lopdf::content::Operation::new("ET", vec![]));
        new_text_ops.push(lopdf::content::Operation::new("Q", vec![]));

        // #35 是正: cleaned_operations に新テキストを連結して単一ストリームに
        let mut all_ops = cleaned_operations;
        all_ops.extend(new_text_ops);

        let content = lopdf::content::Content { operations: all_ops };
        let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

        let mut stream = Stream::new(Dictionary::new(), content_bytes);
        stream.dict.set("Type", Object::Name("Content".into()));
        let content_id = doc.add_object(stream);

        // Register font in page resources
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
        fonts_dict.set(font_res_name, Object::Reference(font_id));
        resources_dict.set("Font", Object::Dictionary(fonts_dict));

        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.set("Resources", Object::Dictionary(resources_dict));
            // #35 是正: Contents を単一ストリーム参照に置き換え（既存ストリームはすべて統合済み）
            page_dict.set("Contents", Object::Reference(content_id));
        }
    } else {
        // Standard ASCII Latin Helvetica pipeline
        let mut new_text_ops = vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new("BT", vec![]),
            lopdf::content::Operation::new(
                "Tf",
                vec![Object::Name("Helvetica".into()), Object::Real(font_size)],
            ),
            lopdf::content::Operation::new(
                "rg",
                vec![Object::Real(r), Object::Real(g), Object::Real(b)],
            ),
        ];

        for (i, line) in wrapped_lines.iter().enumerate() {
            let line_y = (start_y as f32) - (i as f32 * line_height);
            new_text_ops.push(lopdf::content::Operation::new(
                "Tm",
                vec![
                    Object::Real(1.0),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(1.0),
                    Object::Real(start_x as f32),
                    Object::Real(line_y),
                ],
            ));
            new_text_ops.push(lopdf::content::Operation::new(
                "Tj",
                vec![Object::String(
                    line.as_bytes().to_vec(),
                    lopdf::StringFormat::Literal,
                )],
            ));
        }

        new_text_ops.push(lopdf::content::Operation::new("ET", vec![]));
        new_text_ops.push(lopdf::content::Operation::new("Q", vec![]));

        // #35 是正: cleaned_operations に新テキストを連結して単一ストリームに
        let mut all_ops = cleaned_operations;
        all_ops.extend(new_text_ops);

        let content = lopdf::content::Content { operations: all_ops };
        let content_bytes = content.encode().map_err(|e| format!("Encode error: {e}"))?;

        let mut stream = Stream::new(Dictionary::new(), content_bytes);
        stream.dict.set("Type", Object::Name("Content".into()));
        let content_id = doc.add_object(stream);

        // Register /Helvetica in page /Resources /Font before using it in Tf
        // (Standard 14 font: no embedding required per ISO 32000-1 §9.6.2.2)
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
        if fonts_dict.get(b"Helvetica").is_err() {
            let mut font_dict = Dictionary::new();
            font_dict.set("Type", Object::Name(b"Font".to_vec()));
            font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
            font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
            font_dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
            let font_id = doc.add_object(Object::Dictionary(font_dict));
            fonts_dict.set("Helvetica", Object::Reference(font_id));
        }
        resources_dict.set("Font", Object::Dictionary(fonts_dict));
        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.set("Resources", Object::Dictionary(resources_dict));
            // #35 是正: Contents を単一ストリーム参照に置き換え（既存ストリームはすべて統合済み）
            page_dict.set("Contents", Object::Reference(content_id));
        }
    }

    save_doc(&mut doc)
}

// ---------------------------------------------------------------------------
// #35 是正サブ関数: リフロー矩形内のテキストブロック(BT〜ET)を除去する
//
// 判定: BT ブロック内で最初に現れる Tm/Td の絶対座標（x, y）が
//       以下の矩形内に収まる場合そのブロック全体を除去する。
//       x_range: [sx - margin, sx + mw + margin]
//       y_range: [sy - total_h - margin, sy + margin]
//       margin = 20.0pt（スキャンずれ・浮動小数誤差の吸収）
// ---------------------------------------------------------------------------
fn remove_text_in_rect(
    operations: Vec<lopdf::content::Operation>,
    start_x: f32,
    start_y: f32,
    max_width: f32,
    total_height: f32,
) -> Vec<lopdf::content::Operation> {
    let margin = 20.0f32;
    let x_min = start_x - margin;
    let x_max = start_x + max_width + margin;
    let y_min = start_y - total_height - margin;
    let y_max = start_y + margin;

    let mut result: Vec<lopdf::content::Operation> = Vec::new();
    let mut pending_block: Vec<lopdf::content::Operation> = Vec::new();
    let mut in_text = false;
    let mut block_x = 0.0f32;
    let mut block_y = 0.0f32;
    let mut current_x = 0.0f32;
    let mut current_y = 0.0f32;
    let mut position_set = false;

    for op in operations {
        match op.operator.as_str() {
            "BT" => {
                in_text = true;
                position_set = false;
                block_x = current_x;
                block_y = current_y;
                pending_block.clear();
                pending_block.push(op);
            }
            "ET" => {
                pending_block.push(op);
                // ブロック先頭座標が矩形内にあれば除去、そうでなければ保持
                let in_rect = block_x >= x_min
                    && block_x <= x_max
                    && block_y >= y_min
                    && block_y <= y_max;
                if !in_rect {
                    result.append(&mut pending_block);
                } else {
                    pending_block.clear();
                }
                in_text = false;
                position_set = false;
            }
            "Tm" => {
                if op.operands.len() >= 6 {
                    let nx = match &op.operands[4] {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => current_x,
                    };
                    let ny = match &op.operands[5] {
                        Object::Real(v) => *v,
                        Object::Integer(v) => *v as f32,
                        _ => current_y,
                    };
                    current_x = nx;
                    current_y = ny;
                    if in_text && !position_set {
                        block_x = nx;
                        block_y = ny;
                        position_set = true;
                    }
                }
                if in_text {
                    pending_block.push(op);
                } else {
                    result.push(op);
                }
            }
            "Td" | "TD" => {
                let dx = op.operands.first().and_then(|o| match o {
                    Object::Real(v) => Some(*v),
                    Object::Integer(v) => Some(*v as f32),
                    _ => None,
                }).unwrap_or(0.0);
                let dy = op.operands.get(1).and_then(|o| match o {
                    Object::Real(v) => Some(*v),
                    Object::Integer(v) => Some(*v as f32),
                    _ => None,
                }).unwrap_or(0.0);
                current_x += dx;
                current_y += dy;
                if in_text && !position_set {
                    block_x = current_x;
                    block_y = current_y;
                    position_set = true;
                }
                if in_text {
                    pending_block.push(op);
                } else {
                    result.push(op);
                }
            }
            _ => {
                if in_text {
                    pending_block.push(op);
                } else {
                    result.push(op);
                }
            }
        }
    }

    // 閉じていない BT（ET なし）が残った場合は保持する（安全側）
    result.append(&mut pending_block);

    result
}
