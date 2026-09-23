use super::common::*;
use lopdf::{Dictionary, Document, Object, Stream};

// ===== INTERACTIVE FORM CREATION (ISO 32000 Compliant) =====

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FormFieldConfig {
    pub field_type: String,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub value: Option<String>,
    pub options: Option<Vec<String>>,
    pub required: bool,
    pub read_only: bool,
    pub max_length: Option<u32>,
}

/// Helper to ensure /AcroForm exists on Catalog Root and has standard /DR (Default Resources)
/// with /Helv (Helvetica) font properly declared and /NeedAppearances true.
pub(crate) fn ensure_acroform_with_resources(doc: &mut Document, field_id: lopdf::ObjectId) -> Result<(), String> {
    let root_id = doc
        .trailer
        .get(b"Root")
        .and_then(|o| o.as_reference())
        .map_err(|_| "No root catalog in PDF".to_string())?;

    // #50 是正: 毎回新規 Helvetica オブジェクトを追加すると
    // フォームフィールドを10個追加すると同一定義が10個生成されてファイルが肥大化する。
    // 既存の /AcroForm /DR /Font /Helv を調べ、存在すれば OID を再利用する。
    let existing_helv_id: Option<lopdf::ObjectId> = {
        let root_dict = doc.objects.get(&root_id).and_then(|o| o.as_dict().ok());
        root_dict.and_then(|rd| {
            let af_dict: Option<&Dictionary> = match rd.get(b"AcroForm").ok() {
                Some(Object::Dictionary(d)) => Some(d),
                Some(Object::Reference(r)) => doc.objects.get(r).and_then(|o| o.as_dict().ok()),
                _ => None,
            };
            af_dict.and_then(|af| {
                let dr = match af.get(b"DR").ok() {
                    Some(Object::Dictionary(d)) => Some(d),
                    Some(Object::Reference(r)) => doc.objects.get(r).and_then(|o| o.as_dict().ok()),
                    _ => None,
                };
                dr.and_then(|d| {
                    let fonts = match d.get(b"Font").ok() {
                        Some(Object::Dictionary(fd)) => Some(fd),
                        Some(Object::Reference(r)) => doc.objects.get(r).and_then(|o| o.as_dict().ok()),
                        _ => None,
                    };
                    fonts.and_then(|fd| {
                        fd.get(b"Helv").ok().and_then(|o| match o {
                            Object::Reference(r) => Some(*r),
                            _ => None,
                        })
                    })
                })
            })
        })
    };

    // Helvetica OID を取得（既存があれば再利用、なければ新規作成）
    let font_id = existing_helv_id.unwrap_or_else(|| {
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name(b"Font".to_vec()));
        font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
        font_dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        doc.add_object(Object::Dictionary(font_dict))
    });

    // 2. Prepare Font resources dictionary
    let mut font_res = Dictionary::new();
    font_res.set("Helv", Object::Reference(font_id));
    let mut dr_dict = Dictionary::new();
    dr_dict.set("Font", Object::Dictionary(font_res));

    // 3. Inspect existing fields before mutable borrow
    let existing_fields = if let Some(Object::Dictionary(ref root_dict)) = doc.objects.get(&root_id) {
        match root_dict.get(b"AcroForm") {
            Ok(Object::Dictionary(ref form_dict)) => match form_dict.get(b"Fields") {
                Ok(Object::Array(ref arr)) => arr.clone(),
                _ => Vec::new(),
            },
            Ok(Object::Reference(af_id)) => {
                let existing_af = doc.objects.get(af_id).and_then(|o| o.as_dict().ok());
                match existing_af.and_then(|d| d.get(b"Fields").ok()) {
                    Some(Object::Array(arr)) => arr.clone(),
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let mut fields = existing_fields;
    fields.push(Object::Reference(field_id));

    // 4. Attach or update AcroForm on root
    if let Some(Object::Dictionary(ref mut root_dict)) = doc.objects.get_mut(&root_id) {
        let mut acro_form = Dictionary::new();
        acro_form.set("Fields", Object::Array(fields));
        acro_form.set("DR", Object::Dictionary(dr_dict));
        acro_form.set(
            "DA",
            Object::String(b"/Helv 12 Tf 0 0 0 rg".to_vec(), lopdf::StringFormat::Literal),
        );
        acro_form.set("NeedAppearances", Object::Boolean(true));
        root_dict.set("AcroForm", Object::Dictionary(acro_form));
    }

    Ok(())
}

/// Helper to generate checkbox / radio button appearance stream for true (checked) or false (unchecked)
pub(crate) fn create_button_appearance_stream(width: f32, height: f32, checked: bool) -> Stream {
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Form".to_vec()));
    dict.set(
        "BBox",
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(width),
            Object::Real(height),
        ]),
    );

    let mut ops = format!(
        "0.92 0.92 0.92 rg 0 0 {width:.2} {height:.2} re f \
         0.3 0.3 0.3 RG 1 w 0.5 0.5 {w:.2} {h:.2} re s ",
        w = width - 1.0,
        h = height - 1.0
    );
    if checked {
        ops.push_str(&format!(
            "0 0 0 RG 1.5 w 3.5 3.5 m {w:.2} {h:.2} l s 3.5 {h:.2} m {w:.2} 3.5 l s ",
            w = width - 3.5,
            h = height - 3.5
        ));
    }

    Stream::new(dict, ops.into_bytes())
}

/// Helper to generate a visible normal appearance (/AP /N) stream for fields
pub(crate) fn create_appearance_stream(
    width: f32,
    height: f32,
    field_type: &str,
    value: Option<&str>,
) -> Stream {
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Form".to_vec()));
    dict.set(
        "BBox",
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(width),
            Object::Real(height),
        ]),
    );

    let stream_content = match field_type {
        "Btn" => {
            // Checkbox appearance: border box with an X if checked
            let is_checked = value == Some("Yes") || value == Some("true") || value == Some("1");
            let mut ops = format!(
                "0.85 0.85 0.85 rg 0 0 {width:.2} {height:.2} re f \
                 0.3 0.3 0.3 RG 1 w 0.5 0.5 {w:.2} {h:.2} re s ",
                w = width - 1.0,
                h = height - 1.0
            );
            if is_checked {
                ops.push_str(&format!(
                    "0 0 0 RG 1.5 w 3 3 m {w:.2} {h:.2} l s 3 {h:.2} m {w:.2} 3 l s ",
                    w = width - 3.0,
                    h = height - 3.0
                ));
            }
            ops
        }
        "Sig" => {
            // Signature field appearance: subtle dashed border, signature icon line, and label
            format!(
                "0.96 0.97 0.99 rg 0 0 {width:.2} {height:.2} re f \
                 0.2 0.4 0.8 RG 1 w [3 3] 0 d 0.5 0.5 {w:.2} {h:.2} re s [] 0 d \
                 0.2 0.4 0.8 rg /Helv 9 Tf 6 {baseline:.2} Td (Digital Signature Field) Tj",
                w = width - 1.0,
                h = height - 1.0,
                baseline = (height / 2.0 - 3.0).max(4.0)
            )
        }
        _ => {
            // Text or Choice field appearance: clean light background, subtle border, and text
            let val_text = value.unwrap_or("");
            let escaped_text = val_text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
            format!(
                "0.98 0.98 0.98 rg 0 0 {width:.2} {height:.2} re f \
                 0.7 0.7 0.7 RG 1 w 0.5 0.5 {w:.2} {h:.2} re s \
                 0 0 0 rg /Helv 10 Tf 4 {baseline:.2} Td ({escaped_text}) Tj",
                w = width - 1.0,
                h = height - 1.0,
                baseline = (height / 2.0 - 4.0).max(4.0)
            )
        }
    };

    Stream::new(dict, stream_content.into_bytes())
}

// Create interactive form field
pub fn create_form_field(
    data: &[u8],
    page_index: usize,
    config: &FormFieldConfig,
) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }

    let page_id = page_ids[page_index];

    // Create field dictionary
    let mut field_dict = Dictionary::new();
    field_dict.set("Type", Object::Name("Annot".into()));
    field_dict.set("Subtype", Object::Name("Widget".into()));
    field_dict.set("FT", Object::Name(config.field_type.as_bytes().to_vec()));
    field_dict.set(
        "T",
        Object::String(
            encode_pdf_text_string(&config.name),
            lopdf::StringFormat::Literal,
        ),
    );
    field_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(config.x),
            Object::Real(config.y),
            Object::Real(config.x + config.width),
            Object::Real(config.y + config.height),
        ]),
    );
    field_dict.set("F", Object::Integer(4)); // Print flag
    field_dict.set(
        "DA",
        Object::String(
            b"/Helv 12 Tf 0 0 0 rg".to_vec(),
            lopdf::StringFormat::Literal,
        ),
    );

    // 1. Value and Digital Signature specifics
    let is_button = config.field_type == "Btn";

    if config.field_type == "Sig" {
        // Construct compliant Signature Value (/V) placeholder dictionary according to ISO 32000
        let mut sig_val = Dictionary::new();
        sig_val.set("Type", Object::Name("Sig".into()));
        sig_val.set("Filter", Object::Name("Adobe.PPKLite".into()));
        sig_val.set("SubFilter", Object::Name("adbe.pkcs7.detached".into()));
        sig_val.set(
            "Name",
            Object::String(b"Unsigned".to_vec(), lopdf::StringFormat::Literal),
        );
        sig_val.set(
            "Reason",
            Object::String(b"Sign document".to_vec(), lopdf::StringFormat::Literal),
        );
        // Standard placeholder for byte range
        sig_val.set(
            "ByteRange",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(0),
            ]),
        );
        let sig_val_id = doc.add_object(Object::Dictionary(sig_val));
        field_dict.set("V", Object::Reference(sig_val_id));
    } else if let Some(ref value) = config.value {
        if is_button {
            // For checkbox/radio button, value is a Name (e.g. /Yes or /Off)
            let on_state = if value.eq_ignore_ascii_case("off") { "Off" } else { value.as_str() };
            field_dict.set("V", Object::Name(on_state.as_bytes().to_vec()));
            field_dict.set("AS", Object::Name(on_state.as_bytes().to_vec()));
        } else {
            field_dict.set(
                "V",
                Object::String(encode_pdf_text_string(value), lopdf::StringFormat::Literal),
            );
        }
    }

    // Accumulate ISO 32000 Field Flags (Ff)
    let mut ff: i64 = 0;
    if config.read_only {
        ff |= 1; // Bit 1: ReadOnly
    }
    if config.required {
        ff |= 1 << 1; // Bit 2: Required
    }
    if config.field_type == "Ch" {
        // Bit 18: Combo box (1 << 17). Without this, viewers treat /FT /Ch as a List box.
        ff |= 1 << 17;
    }
    if ff != 0 {
        field_dict.set("Ff", Object::Integer(ff));
    }

    if let Some(max_len) = config.max_length {
        field_dict.set("MaxLen", Object::Integer(max_len as i64));
    }

    // Add options for choice fields
    if let Some(ref options) = config.options {
        let opt_array: Vec<Object> = options
            .iter()
            .map(|o| Object::String(encode_pdf_text_string(o), lopdf::StringFormat::Literal))
            .collect();
        field_dict.set("Opt", Object::Array(opt_array));
    }

    // 2. Generate and attach visible Normal Appearance (/AP /N)
    if is_button {
        // For buttons/checkboxes, PDF viewers expect /AP /N dictionary with On (/Yes) and /Off states
        let on_stream = create_button_appearance_stream(config.width, config.height, true);
        let off_stream = create_button_appearance_stream(config.width, config.height, false);
        let on_id = doc.add_object(Object::Stream(on_stream));
        let off_id = doc.add_object(Object::Stream(off_stream));

        let mut n_dict = Dictionary::new();
        let on_val = config.value.as_deref().unwrap_or("Yes");
        let on_state_name = if on_val.eq_ignore_ascii_case("off") { "Yes" } else { on_val };
        n_dict.set(on_state_name, Object::Reference(on_id));
        n_dict.set("Off", Object::Reference(off_id));

        let mut ap_dict = Dictionary::new();
        ap_dict.set("N", Object::Dictionary(n_dict));
        field_dict.set("AP", Object::Dictionary(ap_dict));
    } else {
        let ap_stream = create_appearance_stream(
            config.width,
            config.height,
            &config.field_type,
            config.value.as_deref(),
        );
        let ap_id = doc.add_object(Object::Stream(ap_stream));
        let mut ap_dict = Dictionary::new();
        ap_dict.set("N", Object::Reference(ap_id));
        field_dict.set("AP", Object::Dictionary(ap_dict));
    }

    let field_id = doc.add_object(Object::Dictionary(field_dict));

    // 3. Add field to page annotations
    if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
        let mut annots = match page_dict.get(b"Annots") {
            Ok(Object::Array(arr)) => arr.clone(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(field_id));
        page_dict.set("Annots", Object::Array(annots));
    }

    // 4. Register in AcroForm with proper /DR and /NeedAppearances
    ensure_acroform_with_resources(&mut doc, field_id)?;

    save_doc(&mut doc)
}

// Create checkbox field
pub fn create_checkbox(
    data: &[u8],
    page_index: usize,
    name: &str,
    x: f32,
    y: f32,
    checked: bool,
) -> Result<Vec<u8>, String> {
    let config = FormFieldConfig {
        field_type: "Btn".into(),
        name: name.to_string(),
        x,
        y,
        width: 20.0,
        height: 20.0,
        value: Some(if checked { "Yes".into() } else { "Off".into() }),
        options: None,
        required: false,
        read_only: false,
        max_length: None,
    };
    create_form_field(data, page_index, &config)
}

// Create radio button group compliant with ISO 32000-1 §12.7.4.2.4
// In PDF, a radio button group is a single parent field (/FT /Btn with Radio flag 1<<15)
// and multiple widget kid annotations with mutually exclusive states.
pub fn create_radio_button(
    data: &[u8],
    page_index: usize,
    group_name: &str,
    options: &[String],
    x: f32,
    y: f32,
) -> Result<Vec<u8>, String> {
    if options.is_empty() {
        return Ok(data.to_vec());
    }

    let mut doc = Document::load_mem(data).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_ids = get_page_ids(&doc);
    if page_index >= page_ids.len() {
        return Err("Page index out of range".into());
    }
    let page_id = page_ids[page_index];

    let parent_id = doc.new_object_id();
    let mut kid_refs = Vec::new();
    let mut current_y = y;

    let selected_val = options[0].clone();

    for (_i, opt_name) in options.iter().enumerate() {
        let is_selected = opt_name == &selected_val;

        // Button normal appearance streams
        let on_stream = create_button_appearance_stream(20.0, 20.0, true);
        let off_stream = create_button_appearance_stream(20.0, 20.0, false);
        let on_id = doc.add_object(Object::Stream(on_stream));
        let off_id = doc.add_object(Object::Stream(off_stream));

        let mut n_dict = Dictionary::new();
        n_dict.set(opt_name.as_bytes().to_vec(), Object::Reference(on_id));
        n_dict.set("Off", Object::Reference(off_id));

        let mut ap_dict = Dictionary::new();
        ap_dict.set("N", Object::Dictionary(n_dict));

        // Widget annotation
        let mut widget_dict = Dictionary::new();
        widget_dict.set("Type", Object::Name("Annot".into()));
        widget_dict.set("Subtype", Object::Name("Widget".into()));
        widget_dict.set("Parent", Object::Reference(parent_id));
        widget_dict.set(
            "Rect",
            Object::Array(vec![
                Object::Real(x),
                Object::Real(current_y),
                Object::Real(x + 20.0),
                Object::Real(current_y + 20.0),
            ]),
        );
        widget_dict.set("F", Object::Integer(4)); // Print flag
        widget_dict.set(
            "AS",
            Object::Name(if is_selected {
                opt_name.as_bytes().to_vec()
            } else {
                b"Off".to_vec()
            }),
        );
        widget_dict.set("AP", Object::Dictionary(ap_dict));

        let widget_id = doc.add_object(Object::Dictionary(widget_dict));
        kid_refs.push(Object::Reference(widget_id));

        current_y -= 25.0;
    }

    // Create parent field dictionary
    let mut parent_dict = Dictionary::new();
    parent_dict.set("FT", Object::Name("Btn".into()));
    parent_dict.set(
        "T",
        Object::String(
            encode_pdf_text_string(group_name),
            lopdf::StringFormat::Literal,
        ),
    );
    // Ff: Bit 16 is Radio (1 << 15 = 32768), Bit 17 is NoToggleToOff (1 << 16 = 65536)
    parent_dict.set("Ff", Object::Integer((1 << 15) | (1 << 16)));
    parent_dict.set("V", Object::Name(selected_val.as_bytes().to_vec()));
    parent_dict.set("Kids", Object::Array(kid_refs.clone()));

    doc.objects.insert(parent_id, Object::Dictionary(parent_dict));

    // Add kid widgets to page annotations
    if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
        let mut annots = match page_dict.get(b"Annots") {
            Ok(Object::Array(arr)) => arr.clone(),
            _ => Vec::new(),
        };
        for kid in kid_refs {
            annots.push(kid);
        }
        page_dict.set("Annots", Object::Array(annots));
    }

    // Register parent field in AcroForm
    ensure_acroform_with_resources(&mut doc, parent_id)?;

    save_doc(&mut doc)
}

// Create text input field
pub fn create_text_field(
    data: &[u8],
    page_index: usize,
    name: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    max_length: Option<u32>,
) -> Result<Vec<u8>, String> {
    let config = FormFieldConfig {
        field_type: "Tx".into(),
        name: name.to_string(),
        x,
        y,
        width,
        height,
        value: None,
        options: None,
        required: false,
        read_only: false,
        max_length,
    };
    create_form_field(data, page_index, &config)
}

// Create signature field
pub fn create_signature_field(
    data: &[u8],
    page_index: usize,
    name: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<Vec<u8>, String> {
    let config = FormFieldConfig {
        field_type: "Sig".into(),
        name: name.to_string(),
        x,
        y,
        width,
        height,
        value: None,
        options: None,
        required: true,
        read_only: false,
        max_length: None,
    };
    create_form_field(data, page_index, &config)
}

// Create dropdown/combo box
pub fn create_dropdown(
    data: &[u8],
    page_index: usize,
    name: &str,
    options: &[String],
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<Vec<u8>, String> {
    let config = FormFieldConfig {
        field_type: "Ch".into(),
        name: name.to_string(),
        x,
        y,
        width,
        height,
        value: options.first().cloned(),
        options: Some(options.to_vec()),
        required: false,
        read_only: false,
        max_length: None,
    };
    create_form_field(data, page_index, &config)
}
