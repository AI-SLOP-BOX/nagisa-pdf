use lopdf::Document;
use super::common::get_page_ids;

fn find_subsequence(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|p| from + p)
}

/// High-level compatibility/fidelity snapshot of a PDF document.
///
/// All fields are derived from a best-effort structural inspection. When the
/// document cannot be parsed at all, [`inspect_pdf`] returns `Err` rather than a
/// degraded report so callers must explicitly opt-in to salvage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompatibilityReport {
    /// Header version string, e.g. "1.7" or "2.0".
    pub pdf_version: String,
    /// Number of leaf pages, 0 when unknown.
    pub page_count: usize,
    /// Number of visible signature fields (`/Type /Sig` widgets).
    pub signed_signature_count: usize,
    /// `true` when a DocMDP/permanent signature (`/Perms`) is present.
    pub certified: bool,
    /// The trailer uses a cross-reference stream (PDF 1.5+) instead of a table.
    pub has_xref_stream: bool,
    /// A linearization dictionary is present (web-optimized first-page open).
    pub linearized: bool,
    /// The document is encrypted (`/Encrypt` in the trailer).
    pub encrypted: bool,
    /// Object streams (`/Type /ObjStm`) are used for compression.
    pub has_object_stream: bool,
    /// A `/Collection` (portfolio) root is present.
    pub has_portfolio: bool,
    /// Embedded file attachments are referenced (`/EmbeddedFile`).
    pub has_attachments: bool,
    /// True when `Document::load_mem` succeeded.
    pub parseable: bool,
}

impl Default for CompatibilityReport {
    fn default() -> Self {
        CompatibilityReport {
            pdf_version: String::new(),
            page_count: 0,
            signed_signature_count: 0,
            certified: false,
            has_xref_stream: false,
            linearized: false,
            encrypted: false,
            has_object_stream: false,
            has_portfolio: false,
            has_attachments: false,
            parseable: false,
        }
    }
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    let mut hits = 0;
    let mut idx = 0;
    while let Some(pos) = find_subsequence(haystack, needle, idx) {
        hits += 1;
        idx = pos + needle.len();
    }
    hits
}

/// Count signature fields (`/Type /Sig` or `/FT /Sig`) by scanning object
/// dictionaries plus a raw-byte fallback for malformed-but-loadable docs.
/// Each dictionary is counted at most once even if it has both markers.
fn signature_count(doc: &Document, data: &[u8]) -> usize {
    let mut hits = 0;
    for object in doc.objects.values() {
        if let lopdf::Object::Dictionary(dict) = object {
            let is_sig_type = dict
                .get(b"Type")
                .map(|v| matches!(v, lopdf::Object::Name(n) if n == b"Sig"))
                .unwrap_or(false);
            let is_sig_ft = dict
                .get(b"FT")
                .map(|v| matches!(v, lopdf::Object::Name(n) if n == b"Sig"))
                .unwrap_or(false);
            if is_sig_type || is_sig_ft {
                hits += 1;
            }
        }
    }
    hits.max(count(data, b"/Type /Sig"))
}

fn header_version(data: &[u8]) -> String {
    if let Some(rest) = data.strip_prefix(b"%PDF-") {
        let mut end = 0;
        while end < rest.len() && rest[end] != b'\n' && rest[end] != b'\r' {
            end += 1;
        }
        return String::from_utf8_lossy(&rest[..end]).to_string();
    }
    String::new()
}

/// Inspect a PDF byte vector and return a [`CompatibilityReport`].
///
/// Returns `Err` (never panics) when the document cannot be parsed, so callers
/// can route to [`super::repair::repair_corrupt_pdf`] for salvage.
pub fn inspect_pdf(data: &[u8]) -> Result<CompatibilityReport, String> {
    let version = header_version(data);
    let mut report = CompatibilityReport {
        pdf_version: version,
        parseable: false,
        ..CompatibilityReport::default()
    };
    let doc = Document::load_mem(data).map_err(|e| format!("PDFの解析に失敗しました: {e}"))?;
    report.parseable = true;
    report.page_count = get_page_ids(&doc).len();
    report.has_xref_stream = find_subsequence(data, b"/Type /XRef", 0).is_some()
        || find_subsequence(data, b"/Type/XRef", 0).is_some();
    report.linearized = find_subsequence(data, b"Linearized", 0).is_some();
    report.encrypted = doc.trailer.get(b"Encrypt").is_ok();
    report.has_object_stream = find_subsequence(data, b"/ObjStm", 0).is_some();
    report.has_portfolio = find_subsequence(data, b"/Collection", 0).is_some();
    report.has_attachments = find_subsequence(data, b"/EmbeddedFile", 0).is_some();
    report.signed_signature_count = signature_count(&doc, data);
    report.certified = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|value| value.as_reference().ok())
        .and_then(|root_id| doc.objects.get(&root_id))
        .and_then(|object| object.as_dict().ok())
        .map_or(false, |catalog| catalog.get(b"Perms").is_ok());
    Ok(report)
}
