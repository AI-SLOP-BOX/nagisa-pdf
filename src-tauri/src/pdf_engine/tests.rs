#[cfg(test)]
mod tests {
    use crate::pdf_engine::*;
    use lopdf::{Dictionary, Document, Object, Stream};

    fn find_tool(name: &str) -> Option<std::path::PathBuf> {
        // 1. Check PATH env variable
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                let exe_name = if cfg!(windows) {
                    format!("{}.exe", name)
                } else {
                    name.to_string()
                };
                let full_path = dir.join(&exe_name);
                if full_path.is_file() {
                    return Some(full_path);
                }
            }
        }
        // 2. Check standard installation directories
        let common_dirs = [
            "/usr/bin",
            "/usr/local/bin",
            "/opt/homebrew/bin",
            "/bin",
            "/snap/bin",
        ];
        for dir in &common_dirs {
            let p = std::path::Path::new(dir).join(name);
            if p.is_file() {
                return Some(p);
            }
        }
        None
    }

    const TEST_SIGNING_KEY_PEM: &str = include_str!("testdata/nagisa_signing_test.key");
    const TEST_SIGNING_CERT_PEM: &str = include_str!("testdata/nagisa_signing_test.crt");

    fn create_test_pdf(num_pages: usize) -> Vec<u8> {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));
        let mut kids = Vec::new();

        for i in 0..num_pages {
            let content = format!("BT /F1 12 Tf 50 750 Td (Page {}) Tj ET", i + 1);
            let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
                Dictionary::new(),
                content.into_bytes(),
            )));

            let mut f1_dict = Dictionary::new();
            f1_dict.set("Type", Object::Name("Font".into()));
            f1_dict.set("Subtype", Object::Name("Type1".into()));
            f1_dict.set("BaseFont", Object::Name("Helvetica".into()));
            let f1_id = doc.add_object(Object::Dictionary(f1_dict));

            let mut font_dict = Dictionary::new();
            font_dict.set("F1", Object::Reference(f1_id));

            let mut res_dict = Dictionary::new();
            res_dict.set("Font", Object::Dictionary(font_dict));

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
            page_dict.set("Resources", Object::Dictionary(res_dict));
            page_dict.set("Contents", Object::Reference(content_id));

            let page_id = doc.add_object(Object::Dictionary(page_dict));
            kids.push(Object::Reference(page_id));
        }

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Kids", Object::Array(kids));
        pages_dict.set("Count", Object::Integer(num_pages as i64));
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let mut root_dict = Dictionary::new();
        root_dict.set("Type", Object::Name("Catalog".into()));
        root_dict.set("Pages", Object::Reference(pages_id));
        let root_id = doc.add_object(Object::Dictionary(root_dict));
        doc.trailer.set("Root", Object::Reference(root_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn test_page_count_and_inspection() {
        let pdf = create_test_pdf(3);
        let count = get_page_count_from_data(&pdf).expect("Page count should succeed");
        assert_eq!(count, 3);
    }

    #[test]
    fn test_page_deletion_and_rotation() {
        let pdf = create_test_pdf(3);
        let rotated = rotate_page(&pdf, 0, 90).expect("Rotate should succeed");
        assert!(!rotated.is_empty());

        let deleted = delete_page(&pdf, 1).expect("Delete should succeed");
        let new_count = get_page_count_from_data(&deleted).expect("Get new count");
        assert_eq!(new_count, 2);
    }

    #[test]
    fn test_empty_or_corrupt_recovery() {
        let corrupt_data = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Count 0 /Kids [] >>\nendobj\nxref\n0 3\n0000000000 65535 f \n0000009999 00000 n \n0000009999 00000 n \ntrailer\n<< /Size 3 /Root 1 0 R >>\nstartxref\n999999\n%%EOF";
        let repaired =
            repair_corrupt_pdf(corrupt_data).expect("Repair should salvage catalog/pages");
        assert!(!repaired.is_empty());
    }

    #[test]
    fn test_document_comparison() {
        let doc1 = create_test_pdf(1);
        let doc2 = create_test_pdf(1);
        let report = compare_pdf_documents(&doc1, &doc2).expect("Compare should succeed");
        assert_eq!(report.total_changes, 0);
    }

    #[test]
    fn test_pdf_x_conversion_and_validation() {
        let pdf = create_test_pdf(2);
        let pdfx = convert_to_pdfx_standard(&pdf, "PDF/X-1a", "Japan Color 2001 Coated")
            .expect("Convert to PDF/X-1a should succeed");
        let validation =
            validate_pdfx_compliance(&pdfx, "PDF/X-1a").expect("Validation should succeed");
        assert!(
            validation.is_compliant,
            "PDF/X-1a converted document should be compliant"
        );
    }

    #[test]
    fn test_page_numbers_injection() {
        let pdf = create_test_pdf(2);
        let numbered = add_page_numbers(&pdf, "bottom-center", 10.0, 1).expect("Add page numbers");
        assert!(!numbered.is_empty());
        let count = get_page_count_from_data(&numbered).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_redact_area_preserves_other_content() {
        let pdf = create_test_pdf(1);
        // Initial PDF contains "Page 1"
        let initial_doc = Document::load_mem(&pdf).expect("Load initial PDF");
        let initial_page_ids = get_page_ids(&initial_doc);
        let initial_contents = initial_doc
            .get_page_content(initial_page_ids[0])
            .expect("Get content");
        let initial_text = String::from_utf8_lossy(&initial_contents);
        assert!(
            initial_text.contains("Page 1"),
            "Original content must contain Page 1"
        );

        // Redact a small box at (10, 10, 50, 50), not overlapping the text at (50, 750)
        let redacted = redact_area(&pdf, 0, 10.0, 10.0, 50.0, 50.0, "#000000")
            .expect("Redact area should succeed");

        let redacted_doc = Document::load_mem(&redacted).expect("Load redacted PDF");
        let redacted_page_ids = get_page_ids(&redacted_doc);
        let redacted_contents = redacted_doc
            .get_page_content(redacted_page_ids[0])
            .expect("Get redacted content");
        let redacted_text = String::from_utf8_lossy(&redacted_contents);

        // Crucial check: the existing page content must be PRESERVED, not replaced by just a black box!
        assert!(
            redacted_text.contains("Page 1"),
            "Existing page content must be preserved after redact_area"
        );
        assert!(
            redacted_text.contains("re"),
            "Redaction rectangle must be appended"
        );
    }

    #[test]
    fn test_redact_purges_string_completely() {
        let secret = "SECRET-123456";
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));

        let content = format!("BT /F1 12 Tf 50 750 Td ({secret}) Tj ET");
        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            Dictionary::new(),
            content.into_bytes(),
        )));

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
        page_dict.set("Contents", Object::Reference(content_id));

        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages_dict.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let mut root_dict = Dictionary::new();
        root_dict.set("Type", Object::Name("Catalog".into()));
        root_dict.set("Pages", Object::Reference(pages_id));
        let root_id = doc.add_object(Object::Dictionary(root_dict));
        doc.trailer.set("Root", Object::Reference(root_id));

        let mut initial_pdf = Vec::new();
        doc.save_to(&mut initial_pdf).unwrap();

        // 1. Verify secret is present before redaction
        assert!(
            String::from_utf8_lossy(&initial_pdf).contains(secret),
            "Secret must exist in initial raw PDF bytes"
        );

        // 2. Perform deep text redaction
        let purged_pdf =
            redact_text_deep(&initial_pdf, secret, "#000000").expect("Deep redaction must succeed");

        // 3. Byte-level verification: raw byte search
        assert!(
            !String::from_utf8_lossy(&purged_pdf).contains(secret),
            "Physical raw PDF bytes must NOT contain secret after deep redaction"
        );

        // 4. Object stream level verification: lopdf document decode
        let purged_doc = Document::load_mem(&purged_pdf).expect("Load purged PDF");
        for (_, object) in purged_doc.objects.iter() {
            if let Object::Stream(ref stream) = object {
                if let Ok(decoded) = stream.decompressed_content() {
                    assert!(
                        !String::from_utf8_lossy(&decoded).contains(secret),
                        "Decompressed stream must NOT contain secret after deep redaction"
                    );
                }
            }
        }
    }

    #[test]
    fn test_searchable_pdf_structure_and_fallback() {
        let temp_output =
            std::env::temp_dir().join(format!("searchable_test_{}.pdf", std::process::id()));
        let output_path = temp_output.to_string_lossy().to_string();

        let ocr_text = "Line 1: Scanned invoice text\nLine 2: Total amount 50000 JPY";
        crate::ocr_engine::create_searchable_pdf(&[], ocr_text, &output_path)
            .expect("Searchable PDF creation should succeed");

        let data = std::fs::read(&output_path).expect("Read output PDF");
        let _ = std::fs::remove_file(&output_path);

        let doc = Document::load_mem(&data).expect("Must parse as valid PDF");

        // Check Catalog Root exists
        let root_ref = doc.trailer.get(b"Root").expect("Must have Root in trailer");
        let root_id = root_ref.as_reference().expect("Root must be reference");
        let root_dict = doc
            .objects
            .get(&root_id)
            .and_then(|o| o.as_dict().ok())
            .expect("Root must be dict");
        assert_eq!(
            root_dict.get(b"Type").unwrap().as_name().unwrap(),
            b"Catalog"
        );

        // Check Pages dictionary
        let pages_ref = root_dict.get(b"Pages").expect("Catalog must have Pages");
        let pages_id = pages_ref.as_reference().expect("Pages must be reference");
        let pages_dict = doc
            .objects
            .get(&pages_id)
            .and_then(|o| o.as_dict().ok())
            .expect("Pages must be dict");
        assert_eq!(
            pages_dict.get(b"Type").unwrap().as_name().unwrap(),
            b"Pages"
        );

        let page_ids = get_page_ids(&doc);
        assert_eq!(page_ids.len(), 1);

        // Verify page has Parent reference and Resources with Font
        let page_dict = doc
            .objects
            .get(&page_ids[0])
            .and_then(|o| o.as_dict().ok())
            .expect("Page must be dict");
        assert_eq!(
            page_dict.get(b"Parent").unwrap().as_reference().unwrap(),
            pages_id
        );
        assert!(
            page_dict.get(b"Resources").is_ok(),
            "Page must have Resources dict"
        );
    }

    #[test]
    fn test_signature_inspection_honesty() {
        let pdf = create_test_pdf(1);
        let signed = add_digital_signature(
            &pdf,
            0,
            100.0,
            100.0,
            200.0,
            50.0,
            "Alice Developer",
            "Code Review Approval",
            None,
        )
        .expect("Add digital signature structure");

        let doc = Document::load_mem(&signed).expect("Load signed PDF");
        let result = verify_signature_in_doc(&doc).expect("Verify signature structure");

        let sigs = result["signatures"]
            .as_array()
            .expect("Must have signatures array");
        assert_eq!(sigs.len(), 1);
        let sig0 = &sigs[0];

        // Verify that fields extracted match what was inserted
        assert_eq!(sig0["signer"], "Alice Developer");
        assert_eq!(sig0["reason"], "Code Review Approval");
        // Verify honesty: Lopdf must NOT claim cryptographic validity or fake issuer
        assert_eq!(sig0["status"], "unverified_structure_only");
        assert_eq!(sig0["integrity_verified"], false);
        assert_eq!(sig0["aatl_verified"], false);
    }

    #[test]
    fn test_unlock_encrypted_pdf_rejection() {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));
        let mut root_dict = Dictionary::new();
        root_dict.set("Type", Object::Name("Catalog".into()));
        root_dict.set("Pages", Object::Reference(pages_id));
        let root_id = doc.add_object(Object::Dictionary(root_dict));
        doc.trailer.set("Root", Object::Reference(root_id));

        // Insert fake /Encrypt dictionary in trailer
        let mut encrypt_dict = Dictionary::new();
        encrypt_dict.set("Filter", Object::Name("Standard".into()));
        encrypt_dict.set("V", Object::Integer(2));
        encrypt_dict.set("R", Object::Integer(3));
        let encrypt_id = doc.add_object(Object::Dictionary(encrypt_dict));
        doc.trailer.set("Encrypt", Object::Reference(encrypt_id));

        let mut encrypted_pdf_bytes = Vec::new();
        doc.save_to(&mut encrypted_pdf_bytes).unwrap();

        // Calling unlock_pdf MUST return an Err refusing to corrupt the PDF
        let unlock_result = unlock_pdf(&encrypted_pdf_bytes, "secret");
        assert!(
            unlock_result.is_err(),
            "unlock_pdf must reject blind trailer stripping when Encrypt dictionary is present"
        );
    }

    #[test]
    fn test_redact_text_replaces_stream_content() {
        let text_to_replace = "CONFIDENTIAL-DATA";
        let replacement = "REDACTED";

        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));
        let content = format!("BT /F1 12 Tf 50 750 Td ({text_to_replace}) Tj ET");
        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            Dictionary::new(),
            content.into_bytes(),
        )));

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
        page_dict.set("Contents", Object::Reference(content_id));
        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages_dict.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let mut root_dict = Dictionary::new();
        root_dict.set("Type", Object::Name("Catalog".into()));
        root_dict.set("Pages", Object::Reference(pages_id));
        let root_id = doc.add_object(Object::Dictionary(root_dict));
        doc.trailer.set("Root", Object::Reference(root_id));

        let mut pdf_data = Vec::new();
        doc.save_to(&mut pdf_data).unwrap();

        let redacted = redact_text(&pdf_data, text_to_replace, replacement)
            .expect("redact_text should replace content in body stream");

        assert!(
            !String::from_utf8_lossy(&redacted).contains(text_to_replace),
            "Original text must no longer be present"
        );
        assert!(
            String::from_utf8_lossy(&redacted).contains(replacement),
            "Replacement text must be present"
        );
    }

    #[test]
    fn test_redact_text_compressed_stream() {
        let text_to_replace = "TOP-SECRET-DEFLATE";
        let replacement = "PURGED";

        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));
        let content = format!("BT /F1 12 Tf 50 750 Td ({text_to_replace}) Tj ET");

        // Compress content stream with FlateDecode
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(content.as_bytes()).unwrap();
        let compressed_bytes = encoder.finish().unwrap();

        let mut stream_dict = Dictionary::new();
        stream_dict.set("Filter", Object::Name("FlateDecode".into()));
        let stream = lopdf::Stream::new(stream_dict, compressed_bytes);
        assert_eq!(
            stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"FlateDecode"
        );
        let content_id = doc.add_object(Object::Stream(stream));

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
        page_dict.set("Contents", Object::Reference(content_id));
        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages_dict.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let mut root_dict = Dictionary::new();
        root_dict.set("Type", Object::Name("Catalog".into()));
        root_dict.set("Pages", Object::Reference(pages_id));
        let root_id = doc.add_object(Object::Dictionary(root_dict));
        doc.trailer.set("Root", Object::Reference(root_id));

        let mut pdf_data = Vec::new();
        doc.save_to(&mut pdf_data).unwrap();

        // Perform redact_text on FlateDecode-compressed stream
        let redacted = redact_text(&pdf_data, text_to_replace, replacement)
            .expect("redact_text must succeed even on compressed stream");

        let res_doc = Document::load_mem(&redacted).expect("Parse output PDF");
        let pids = get_page_ids(&res_doc);
        let content_bytes = res_doc.get_page_content(pids[0]).expect("Get page content");
        let decompressed = String::from_utf8_lossy(&content_bytes);

        assert!(
            !decompressed.contains(text_to_replace),
            "Decompressed stream must not contain target text"
        );
        assert!(
            decompressed.contains(replacement),
            "Decompressed stream must contain replacement text"
        );
    }

    #[test]
    fn test_empty_mock_security_responses() {
        // Confirm no fake digital IDs or fake certificates are returned
        let ids = list_digital_ids().expect("list_digital_ids must succeed");
        assert!(
            ids.is_empty(),
            "list_digital_ids must be empty when no certificates enrolled"
        );

        let certs = list_certificates().expect("list_certificates must succeed");
        assert!(certs.is_empty(), "list_certificates must be empty");

        let ts_res = add_timestamp(&[], "http://tsa.example.com");
        assert!(
            ts_res.is_err(),
            "add_timestamp must reject without real TSA connection"
        );
    }

    #[test]
    fn test_ink_coverage_300_percent_threshold() {
        // Create a PDF with CMYK paint operators
        // Case 1: C=0.5, M=0.5, Y=0.5, K=0.5 -> Total 2.0 (200%), must NOT warn
        let mut doc1 = Document::with_version("1.7");
        let content1 = b"0.5 0.5 0.5 0.5 k 0 0 100 100 re f";
        let stream1 = lopdf::Stream::new(Dictionary::new(), content1.to_vec());
        let s_id1 = doc1.add_object(Object::Stream(stream1));

        let mut page1 = Dictionary::new();
        page1.set("Type", Object::Name("Page".into()));
        page1.set("Contents", Object::Reference(s_id1));
        page1.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(595.0),
                Object::Real(842.0),
            ]),
        );
        let p_id1 = doc1.add_object(Object::Dictionary(page1));

        let mut pages1 = Dictionary::new();
        pages1.set("Type", Object::Name("Pages".into()));
        pages1.set("Kids", Object::Array(vec![Object::Reference(p_id1)]));
        pages1.set("Count", Object::Integer(1));
        let pages_id1 = doc1.add_object(Object::Dictionary(pages1));

        let mut cat1 = Dictionary::new();
        cat1.set("Type", Object::Name("Catalog".into()));
        cat1.set("Pages", Object::Reference(pages_id1));
        let cat_id1 = doc1.add_object(Object::Dictionary(cat1));
        doc1.trailer.set("Root", Object::Reference(cat_id1));

        let mut pdf_bytes1 = Vec::new();
        doc1.save_to(&mut pdf_bytes1).unwrap();

        let res1 = check_ink_coverage(&pdf_bytes1, 0).expect("check_ink_coverage page 0");
        assert_eq!(
            res1["warning"], false,
            "200% ink coverage must not trigger >300% warning: {:?}",
            res1
        );
        assert!((res1["max_coverage"].as_f64().unwrap() - 200.0).abs() < 1e-2);

        // Case 2: C=0.9, M=0.9, Y=0.8, K=0.6 -> Total 3.2 (320%), MUST warn
        let mut doc2 = Document::with_version("1.7");
        let content2 = b"0.9 0.9 0.8 0.6 k 0 0 100 100 re f";
        let stream2 = lopdf::Stream::new(Dictionary::new(), content2.to_vec());
        let s_id2 = doc2.add_object(Object::Stream(stream2));

        let mut page2 = Dictionary::new();
        page2.set("Type", Object::Name("Page".into()));
        page2.set("Contents", Object::Reference(s_id2));
        page2.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(595.0),
                Object::Real(842.0),
            ]),
        );
        let p_id2 = doc2.add_object(Object::Dictionary(page2));

        let mut pages2 = Dictionary::new();
        pages2.set("Type", Object::Name("Pages".into()));
        pages2.set("Kids", Object::Array(vec![Object::Reference(p_id2)]));
        pages2.set("Count", Object::Integer(1));
        let pages_id2 = doc2.add_object(Object::Dictionary(pages2));

        let mut cat2 = Dictionary::new();
        cat2.set("Type", Object::Name("Catalog".into()));
        cat2.set("Pages", Object::Reference(pages_id2));
        let cat_id2 = doc2.add_object(Object::Dictionary(cat2));
        doc2.trailer.set("Root", Object::Reference(cat_id2));

        let mut pdf_bytes2 = Vec::new();
        doc2.save_to(&mut pdf_bytes2).unwrap();

        let res2 = check_ink_coverage(&pdf_bytes2, 0).expect("check_ink_coverage page 0");
        assert_eq!(
            res2["warning"], true,
            "320% ink coverage must trigger >300% warning: {:?}",
            res2
        );
    }

    #[test]
    fn test_add_page_numbers_on_flatedecode_stream() {
        // Construct a PDF with a compressed FlateDecode content stream
        let mut doc = Document::with_version("1.7");
        let raw_stream = b"BT /F1 12 Tf 50 700 Td (Initial Text) Tj ET";
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, raw_stream).unwrap();
        let compressed_bytes = encoder.finish().unwrap();

        let mut stream = lopdf::Stream::new(Dictionary::new(), compressed_bytes);
        stream
            .dict
            .set("Filter", Object::Name("FlateDecode".into()));
        let s_id = doc.add_object(Object::Stream(stream));

        let mut page = Dictionary::new();
        page.set("Type", Object::Name("Page".into()));
        page.set("Contents", Object::Reference(s_id));
        page.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(595.0),
                Object::Real(842.0),
            ]),
        );
        let p_id = doc.add_object(Object::Dictionary(page));

        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name("Pages".into()));
        pages.set("Kids", Object::Array(vec![Object::Reference(p_id)]));
        pages.set("Count", Object::Integer(1));
        let pages_id = doc.add_object(Object::Dictionary(pages));

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut pdf_data = Vec::new();
        doc.save_to(&mut pdf_data).unwrap();

        // Add page numbers
        let numbered = add_page_numbers(&pdf_data, "bottom-center", 12.0, 1)
            .expect("add_page_numbers must succeed");

        // Inspect output PDF: Contents should now be an array and original stream intact
        let out_doc = Document::load_mem(&numbered).expect("Load numbered PDF");
        let out_page = out_doc.get_dictionary(p_id).expect("Get page dict");

        // Contents must be Array or separate stream, never raw append to Flate stream
        let contents_obj = out_page.get(b"Contents").expect("Contents entry exists");
        match contents_obj {
            Object::Array(arr) => {
                assert_eq!(
                    arr.len(),
                    2,
                    "Contents should have 2 streams: original and new page number stream"
                );
            }
            _ => panic!("Expected Contents to be an Array of streams"),
        }

        // Font resource for Helvetica must be present in Resources
        let resources = out_page.get(b"Resources").expect("Resources exist");
        let res_dict = match resources {
            Object::Dictionary(d) => d.clone(),
            Object::Reference(r) => out_doc.get_dictionary(*r).unwrap().clone(),
            _ => panic!("Expected dict or reference for Resources"),
        };
        assert!(
            res_dict.get(b"Font").is_ok(),
            "Font dict must be present in Resources"
        );
    }

    #[test]
    fn test_repair_pdf_creates_valid_catalog() {
        let corrupt_data = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Count 0 /Kids [] >>\nendobj\nxref\n0 3\n0000000000 65535 f \n0000009999 00000 n \n0000009999 00000 n \ntrailer\n<< /Size 3 /Root 1 0 R >>\nstartxref\n999999\n%%EOF";
        let repaired = repair_pdf(corrupt_data).expect("repair_pdf must succeed");
        let repaired_doc = Document::load_mem(&repaired).expect("Repaired doc must parse");

        let root_ref = repaired_doc.trailer.get(b"Root").expect("Root in trailer");
        let root_id = root_ref.as_reference().expect("Root is reference");
        let root_dict = repaired_doc
            .get_dictionary(root_id)
            .expect("Catalog dictionary");
        assert_eq!(
            root_dict.get(b"Type").unwrap().as_name().unwrap(),
            b"Catalog",
            "Root must point to a Catalog dictionary, not a Page!"
        );
    }

    #[test]
    fn test_xfdf_xml_escaping_and_unescaping() {
        let pdf = create_test_pdf(1);
        let comment_text = "Review & approval <urgent> \"2026\" 'important'";

        // Add sticky note annotation with special XML characters
        let with_annot = add_sticky_note(&pdf, 0, 100.0, 100.0, comment_text, "#FF0000")
            .expect("add_sticky_note");

        let exported_xfdf = export_xfdf(&with_annot).expect("export_xfdf");
        assert!(
            exported_xfdf.contains("&amp;"),
            "XML must escape '&' to '&amp;':\n{}",
            exported_xfdf
        );
        assert!(
            exported_xfdf.contains("&lt;urgent&gt;"),
            "XML must escape '<' and '>':\n{}",
            exported_xfdf
        );
        assert!(
            exported_xfdf.contains("&quot;"),
            "XML must escape quotes:\n{}",
            exported_xfdf
        );

        // Import back and verify unescaping and position preservation
        let imported_pdf = import_xfdf(&pdf, &exported_xfdf).expect("import_xfdf");
        let annots = get_annotations(&imported_pdf).expect("get_annotations");
        assert!(!annots.is_empty());
        let imported_c = annots[0]["contents"].as_str().unwrap();
        assert_eq!(
            imported_c, comment_text,
            "Imported contents must match unescaped text"
        );
        let imported_x = annots[0]["x"].as_f64().unwrap();
        let imported_y = annots[0]["y"].as_f64().unwrap();
        assert_eq!(
            imported_x, 100.0,
            "Imported annotation X must match exported left"
        );
        assert_eq!(
            imported_y, 100.0,
            "Imported annotation Y must match exported top"
        );
    }

    #[test]
    fn test_add_page_numbers_preserves_indirect_and_inherited_resources() {
        let mut doc = Document::with_version("1.7");

        // Create an indirect Resources dictionary containing an existing font /F1 and XObject
        let mut font_f1 = Dictionary::new();
        font_f1.set("Type", Object::Name(b"Font".to_vec()));
        font_f1.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_f1.set("BaseFont", Object::Name(b"Times-Roman".to_vec()));
        let f1_id = doc.add_object(Object::Dictionary(font_f1));

        let mut indirect_res = Dictionary::new();
        let mut f_dict = Dictionary::new();
        f_dict.set("F1", Object::Reference(f1_id));
        indirect_res.set("Font", Object::Dictionary(f_dict));
        let res_id = doc.add_object(Object::Dictionary(indirect_res));

        // Create a page referencing Resources indirectly via Reference
        let mut page = Dictionary::new();
        page.set("Type", Object::Name("Page".into()));
        page.set("Resources", Object::Reference(res_id));
        page.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(595.0),
                Object::Real(842.0),
            ]),
        );
        let p_id = doc.add_object(Object::Dictionary(page));

        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name("Pages".into()));
        pages.set("Kids", Object::Array(vec![Object::Reference(p_id)]));
        pages.set("Count", Object::Integer(1));
        let pages_id = doc.add_object(Object::Dictionary(pages));

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut pdf_data = Vec::new();
        doc.save_to(&mut pdf_data).unwrap();

        let numbered =
            add_page_numbers(&pdf_data, "bottom-center", 10.0, 1).expect("add_page_numbers");
        let out_doc = Document::load_mem(&numbered).expect("load numbered doc");
        let page_dict = out_doc.get_dictionary(p_id).expect("get page");
        let res = page_dict.get(b"Resources").expect("Resources entry");
        let res_dict = match res {
            Object::Dictionary(d) => d,
            Object::Reference(id) => out_doc.get_dictionary(*id).expect("get indirect dict"),
            _ => panic!("Expected dictionary or reference"),
        };
        let font_dict = res_dict
            .get(b"Font")
            .expect("Font subdict")
            .as_dict()
            .expect("Font dict");
        assert!(
            font_dict.get(b"F1").is_ok(),
            "Pre-existing F1 font must NOT be wiped out!"
        );
        assert!(
            font_dict.get(b"NagisaHelv").is_ok(),
            "NagisaHelv font must be added!"
        );
    }

    #[test]
    fn test_sanitize_document_purges_names_tree() {
        let mut doc = Document::with_version("1.7");

        // Build /Names -> /JavaScript and /EmbeddedFiles
        let mut js_dict = Dictionary::new();
        js_dict.set(
            "Names",
            Object::Array(vec![Object::String(
                b"TestJS".to_vec(),
                lopdf::StringFormat::Literal,
            )]),
        );
        let js_id = doc.add_object(Object::Dictionary(js_dict));

        let mut ef_dict = Dictionary::new();
        ef_dict.set(
            "Names",
            Object::Array(vec![Object::String(
                b"Malware.exe".to_vec(),
                lopdf::StringFormat::Literal,
            )]),
        );
        let ef_id = doc.add_object(Object::Dictionary(ef_dict));

        let mut names_dict = Dictionary::new();
        names_dict.set("JavaScript", Object::Reference(js_id));
        names_dict.set("EmbeddedFiles", Object::Reference(ef_id));
        let names_id = doc.add_object(Object::Dictionary(names_dict));

        let mut page = Dictionary::new();
        page.set("Type", Object::Name("Page".into()));
        page.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(595.0),
                Object::Real(842.0),
            ]),
        );
        let p_id = doc.add_object(Object::Dictionary(page));

        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name("Pages".into()));
        pages.set("Kids", Object::Array(vec![Object::Reference(p_id)]));
        pages.set("Count", Object::Integer(1));
        let pages_id = doc.add_object(Object::Dictionary(pages));

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        cat.set("Names", Object::Reference(names_id));
        let cat_id = doc.add_object(Object::Dictionary(cat));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut pdf_data = Vec::new();
        doc.save_to(&mut pdf_data).unwrap();

        let (sanitized_bytes, summary) = sanitize_document(&pdf_data).expect("sanitize_document");
        assert!(
            summary.javascript_removed,
            "Must report javascript removed from Names tree"
        );
        assert!(
            summary.attachments_removed > 0,
            "Must report attachments removed from Names tree"
        );

        let clean_doc = Document::load_mem(&sanitized_bytes).expect("load clean doc");
        let root = clean_doc
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        let root_dict = clean_doc.get_dictionary(root).unwrap();

        if let Ok(n_ref) = root_dict.get(b"Names").and_then(|o| o.as_reference()) {
            let n_dict = clean_doc.get_dictionary(n_ref).unwrap();
            assert!(
                !n_dict.has(b"JavaScript"),
                "Names.JavaScript must be removed!"
            );
            assert!(
                !n_dict.has(b"EmbeddedFiles"),
                "Names.EmbeddedFiles must be removed!"
            );
        }
    }

    #[test]
    fn test_deep_redact_physical_image_raster_eradication() {
        // Create an image with known pixels (e.g. 100x100 all white 255)
        let mut img = image::RgbImage::new(100, 100);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgb([255, 255, 255]);
        }
        let temp_img_path =
            std::env::temp_dir().join(format!("redact_test_img_{}.png", std::process::id()));
        img.save(&temp_img_path).expect("Save test image");

        let temp_pdf_path =
            std::env::temp_dir().join(format!("redact_test_pdf_{}.pdf", std::process::id()));
        crate::pdf_engine::convert::images_to_pdf(
            &[temp_img_path.to_string_lossy().to_string()],
            &temp_pdf_path.to_string_lossy().to_string(),
        )
        .expect("Create PDF with image");

        let initial_pdf = std::fs::read(&temp_pdf_path).expect("Read test PDF");
        let _ = std::fs::remove_file(&temp_img_path);
        let _ = std::fs::remove_file(&temp_pdf_path);

        let _initial_doc = Document::load_mem(&initial_pdf).expect("Load initial PDF");

        // Perform deep redaction on page 0 overlapping part of the image
        // Placed width = 100 * 72 / 96 = 75 pt, height = 75 pt
        // Redact rectangle [10, 10, 30, 30] in black #000000
        let redacted_pdf = deep_redact(&initial_pdf, 0, 10.0, 10.0, 30.0, 30.0, "#000000")
            .expect("Deep redaction must succeed");

        let redacted_doc = Document::load_mem(&redacted_pdf).expect("Load redacted PDF");
        // Inspect the image stream in the redacted doc
        let mut found_modified_image = false;
        for (_id, obj) in redacted_doc.objects.iter() {
            if let Object::Stream(stream) = obj {
                let subtype = stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|s| s.as_name().ok());
                if subtype == Some(b"Image") {
                    let decoded_bytes = stream
                        .decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone());
                    let load_res = image::load_from_memory(&decoded_bytes)
                        .or_else(|_| image::load_from_memory(&stream.content));
                    if let Ok(dyn_img) = load_res {
                        let rgb = dyn_img.to_rgb8();
                        let has_black = rgb.pixels().any(|p| p[0] == 0 && p[1] == 0 && p[2] == 0);
                        assert!(
                            has_black,
                            "Image raster must physically contain erased solid black pixels!"
                        );
                        found_modified_image = true;
                    }
                }
            }
        }
        assert!(
            found_modified_image,
            "Must find and verify the redacted image XObject"
        );
    }

    #[test]
    fn test_pdf_x_embeds_real_icc_profile_stream() {
        let mut doc = Document::with_version("1.6");
        let mut page = Dictionary::new();
        page.set("Type", Object::Name("Page".into()));
        page.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(595.0),
                Object::Real(842.0),
            ]),
        );
        let p_id = doc.add_object(Object::Dictionary(page));

        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name("Pages".into()));
        pages.set("Kids", Object::Array(vec![Object::Reference(p_id)]));
        pages.set("Count", Object::Integer(1));
        let pages_id = doc.add_object(Object::Dictionary(pages));

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut pdf_data = Vec::new();
        doc.save_to(&mut pdf_data).unwrap();

        // Convert to PDF/X-4
        let pdfx_bytes = crate::pdf_engine::pdf_x::convert_to_pdfx_standard(
            &pdf_data,
            "PDF/X-4",
            "Japan Color 2001 Coated",
        )
        .expect("convert_to_pdfx_standard");

        let pdfx_doc = Document::load_mem(&pdfx_bytes).expect("Load converted PDF/X");
        let root = pdfx_doc
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        let root_dict = pdfx_doc.get_dictionary(root).unwrap();

        let intents = root_dict.get(b"OutputIntents").unwrap().as_array().unwrap();
        assert!(!intents.is_empty(), "Must have OutputIntents");
        let intent_ref = intents[0].as_reference().unwrap();
        let intent_dict = pdfx_doc.get_dictionary(intent_ref).unwrap();

        assert_eq!(
            intent_dict.get(b"S").unwrap().as_name().unwrap(),
            b"GTS_PDFX"
        );
        let dest_prof_ref = intent_dict
            .get(b"DestOutputProfile")
            .unwrap()
            .as_reference()
            .unwrap();

        // Verify DestOutputProfile is an actual Stream with /N 4
        let stream = pdfx_doc
            .get_object(dest_prof_ref)
            .unwrap()
            .as_stream()
            .unwrap();
        assert_eq!(stream.dict.get(b"N").unwrap().as_i64().unwrap(), 4);
        assert!(
            !stream.content.is_empty(),
            "ICC profile stream content must not be empty"
        );

        // Validate via validate_pdfx_compliance
        let report = crate::pdf_engine::pdf_x::validate_pdfx_compliance(&pdfx_bytes, "PDF/X-4")
            .expect("validate_pdfx_compliance");
        assert!(
            report.is_compliant,
            "PDF/X-4 must pass preflight compliance check"
        );
        assert!(
            report
                .passed_checks
                .iter()
                .any(|c| c.contains("DestOutputProfile")),
            "Passed checks must report embedded DestOutputProfile ICC stream"
        );
    }

    #[test]
    fn test_tsv_geometry_parsing() {
        let sample_tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t500\t800\t-1\t\n\
5\t1\t1\t1\t1\t1\t50\t100\t80\t20\t95\tNagisa\n\
5\t1\t1\t1\t1\t2\t140\t100\t60\t20\t92\tSuite";

        let (text, avg_conf, suspects, words) = crate::ocr_engine::parse_tsv_words(sample_tsv);
        assert_eq!(text.trim(), "Nagisa Suite");
        assert!(avg_conf > 90.0);
        assert_eq!(suspects.len(), 0);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Nagisa");
        assert_eq!(words[0].left, 50.0);
        assert_eq!(words[0].top, 100.0);
        assert_eq!(words[0].width, 80.0);
        assert_eq!(words[0].height, 20.0);
        assert_eq!(words[1].text, "Suite");
    }

    #[test]
    fn test_phase1_contents_preservation_across_all_ops() {
        // Build test PDF with initial text and vector shapes
        let initial_pdf = create_test_pdf(1);

        // 1. add_text should preserve existing page contents
        let with_text = add_text(&initial_pdf, 0, "Added Label", 50.0, 500.0, 14.0, "#FF0000")
            .expect("add_text must succeed");
        let doc1 = Document::load_mem(&with_text).expect("load with_text");
        let pids1 = get_page_ids(&doc1);
        let pdict1 = doc1.get_dictionary(pids1[0]).expect("page dict 1");
        let contents1 = pdict1.get(b"Contents").expect("contents 1");
        // Must be Array with 2 elements (original + added)
        assert!(matches!(contents1, Object::Array(arr) if arr.len() == 2));

        // 2. add_watermark should preserve existing page contents
        let with_wm = add_watermark(
            &with_text,
            "CONFIDENTIAL",
            0.5,
            45.0,
            30.0,
            "#888888",
            true,
            &[],
        )
        .expect("add_watermark must succeed");
        let doc2 = Document::load_mem(&with_wm).expect("load with_wm");
        let pdict2 = doc2.get_dictionary(pids1[0]).expect("page dict 2");
        let contents2 = pdict2.get(b"Contents").expect("contents 2");
        assert!(matches!(contents2, Object::Array(arr) if arr.len() == 3));

        // 3. add_header_footer should preserve existing contents
        let with_hf = add_header_footer(&with_wm, "Header {page}/{total}", "Footer", 10.0, 20.0)
            .expect("add_header_footer must succeed");
        let doc3 = Document::load_mem(&with_hf).expect("load with_hf");
        let pdict3 = doc3.get_dictionary(pids1[0]).expect("page dict 3");
        let contents3 = pdict3.get(b"Contents").expect("contents 3");
        assert!(matches!(contents3, Object::Array(arr) if arr.len() == 4));

        // 4. add_bates_number should preserve existing contents
        let with_bates = add_bates_number(&with_hf, "BATES-", 100, 10.0, 20.0)
            .expect("add_bates_number must succeed");
        let doc4 = Document::load_mem(&with_bates).expect("load with_bates");
        let pdict4 = doc4.get_dictionary(pids1[0]).expect("page dict 4");
        let contents4 = pdict4.get(b"Contents").expect("contents 4");
        assert!(matches!(contents4, Object::Array(arr) if arr.len() == 5));
    }

    #[test]
    fn test_phase1_protect_pdf_safety_rejection() {
        let initial_pdf = create_test_pdf(1);
        let res = protect_pdf(&initial_pdf, "secret_password");
        assert!(
            res.is_err(),
            "protect_pdf must refuse to emit broken pseudo-encrypted PDF"
        );
        let err_msg = res.unwrap_err();
        assert!(err_msg.contains("Standard Security Handler") || err_msg.contains("暗号化"));
    }
    #[test]
    fn test_phase1_preflight_score_no_overflow_on_many_warnings() {
        // 回帰テスト: preflight のスコア計算が u32 減算オーバーフローで
        // パニックしていた問題（デバッグビルドで「attempt to subtract with overflow」）。
        // 100x100pt の極小ページ25枚で「small page」警告が25件(125点分)発生し、
        // 100 - 25*5 が負に転じるケースを再現する。
        let data = create_test_pdf(25);
        let mut doc = Document::load_mem(&data).expect("load test pdf");
        let page_ids: Vec<_> = doc.get_pages().values().copied().collect();
        for page_id in page_ids {
            if let Ok(Object::Dictionary(page_dict)) = doc.get_object_mut(page_id) {
                page_dict.set(
                    "MediaBox",
                    Object::Array(vec![
                        Object::Real(0.0),
                        Object::Real(0.0),
                        Object::Real(100.0),
                        Object::Real(100.0),
                    ]),
                );
            }
        }
        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("save modified pdf");

        let result = crate::pdf_engine::preflight::preflight_check(&buf)
            .expect("preflight_check must not panic or error on many warnings");
        assert_eq!(
            result.score, 0,
            "警告が100点分を超えてもスコアは0にクランプされるべき"
        );
    }



    #[test]
    fn test_phase2_portfolio_valid_catalog_pages_and_names() {
        let tmp_file1 = std::env::temp_dir().join("portfolio_item1.txt");
        let tmp_file2 = std::env::temp_dir().join("portfolio_item2.txt");
        std::fs::write(&tmp_file1, b"Hello file 1").unwrap();
        std::fs::write(&tmp_file2, b"Hello file 2").unwrap();

        let out_path = std::env::temp_dir().join("portfolio_test_out.pdf");
        let paths = vec![
            tmp_file1.to_string_lossy().to_string(),
            tmp_file2.to_string_lossy().to_string(),
        ];

        create_pdf_portfolio(&paths, &out_path.to_string_lossy()).expect("create_pdf_portfolio");
        let pdf_bytes = std::fs::read(&out_path).expect("read portfolio");
        let _ = std::fs::remove_file(&out_path);
        let _ = std::fs::remove_file(&tmp_file1);
        let _ = std::fs::remove_file(&tmp_file2);

        let doc = Document::load_mem(&pdf_bytes).expect("Load portfolio PDF");
        let root_ref = doc
            .trailer
            .get(b"Root")
            .expect("Root must exist")
            .as_reference()
            .unwrap();
        let root_dict = doc.get_dictionary(root_ref).expect("Catalog dictionary");

        // 1. /Type /Catalog
        assert_eq!(
            root_dict.get(b"Type").unwrap().as_name().unwrap(),
            b"Catalog"
        );

        // 2. /Pages must exist
        let pages_ref = root_dict
            .get(b"Pages")
            .expect("Pages must exist")
            .as_reference()
            .unwrap();
        let pages_dict = doc.get_dictionary(pages_ref).expect("Pages dictionary");
        assert_eq!(
            pages_dict.get(b"Type").unwrap().as_name().unwrap(),
            b"Pages"
        );

        // 3. /Collection must exist
        let coll_ref = root_dict
            .get(b"Collection")
            .expect("Collection must exist")
            .as_reference()
            .unwrap();
        let coll_dict = doc.get_dictionary(coll_ref).expect("Collection dictionary");
        assert_eq!(
            coll_dict.get(b"Type").unwrap().as_name().unwrap(),
            b"Collection"
        );

        // 4. /Names -> /EmbeddedFiles name tree
        let names_ref = root_dict
            .get(b"Names")
            .expect("Names must exist")
            .as_reference()
            .unwrap();
        let names_dict = doc.get_dictionary(names_ref).expect("Names dictionary");
        let ef_tree_ref = names_dict
            .get(b"EmbeddedFiles")
            .expect("EmbeddedFiles tree")
            .as_reference()
            .unwrap();
        let ef_tree = doc.get_dictionary(ef_tree_ref).expect("EF tree dictionary");
        let ef_names = ef_tree
            .get(b"Names")
            .expect("Names array")
            .as_array()
            .unwrap();
        assert_eq!(
            ef_names.len(),
            4,
            "2 files = 4 array items (name + filespec ref)"
        );
    }

    #[test]
    fn test_phase2_bookmark_tree_outlines_hierarchy() {
        let initial_pdf = create_test_pdf(3);

        // Add 2 bookmarks
        let bm1 = add_bookmark(&initial_pdf, "Chapter 1", 0).expect("add bookmark 1");
        let bm2 = add_bookmark(&bm1, "Chapter 2", 1).expect("add bookmark 2");

        let doc = Document::load_mem(&bm2).expect("Load bookmarked PDF");
        let root_ref = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let root_dict = doc.get_dictionary(root_ref).unwrap();

        let outlines_ref = root_dict
            .get(b"Outlines")
            .expect("Outlines must exist in Catalog")
            .as_reference()
            .unwrap();
        let outlines = doc
            .get_dictionary(outlines_ref)
            .expect("Outlines dictionary");
        assert_eq!(
            outlines.get(b"Type").unwrap().as_name().unwrap(),
            b"Outlines"
        );
        assert_eq!(outlines.get(b"Count").unwrap().as_i64().unwrap(), 2);

        let first_ref = outlines
            .get(b"First")
            .expect("First item")
            .as_reference()
            .unwrap();
        let last_ref = outlines
            .get(b"Last")
            .expect("Last item")
            .as_reference()
            .unwrap();
        assert_ne!(
            first_ref, last_ref,
            "Two bookmarks must have distinct First and Last"
        );

        let first_dict = doc.get_dictionary(first_ref).unwrap();
        let last_dict = doc.get_dictionary(last_ref).unwrap();

        assert_eq!(
            first_dict.get(b"Title").unwrap().as_str().unwrap(),
            b"Chapter 1"
        );
        assert_eq!(
            last_dict.get(b"Title").unwrap().as_str().unwrap(),
            b"Chapter 2"
        );

        assert_eq!(
            first_dict.get(b"Parent").unwrap().as_reference().unwrap(),
            outlines_ref
        );
        assert_eq!(
            last_dict.get(b"Parent").unwrap().as_reference().unwrap(),
            outlines_ref
        );

        assert_eq!(
            first_dict.get(b"Next").unwrap().as_reference().unwrap(),
            last_ref
        );
        assert_eq!(
            last_dict.get(b"Prev").unwrap().as_reference().unwrap(),
            first_ref
        );
    }

    #[test]
    fn test_phase2_annotation_quadpoints_generation() {
        let initial_pdf = create_test_pdf(1);

        // Add Highlight
        let highlighted = add_highlight(&initial_pdf, 0, 72.0, 700.0, 150.0, 18.0, "#FFFF00")
            .expect("add_highlight");
        let doc1 = Document::load_mem(&highlighted).expect("load highlighted");
        let pids1 = get_page_ids(&doc1);
        let pdict1 = doc1.get_dictionary(pids1[0]).unwrap();
        let annots1 = pdict1.get(b"Annots").unwrap().as_array().unwrap();
        let annot1_ref = annots1[0].as_reference().unwrap();
        let annot1_dict = doc1.get_dictionary(annot1_ref).unwrap();

        assert_eq!(
            annot1_dict.get(b"Subtype").unwrap().as_name().unwrap(),
            b"Highlight"
        );
        let qp = annot1_dict
            .get(b"QuadPoints")
            .expect("QuadPoints must exist")
            .as_array()
            .unwrap();
        assert_eq!(qp.len(), 8, "QuadPoints must be 8 numbers");

        // Add Underline
        let underlined =
            add_underline(&initial_pdf, 0, 72.0, 700.0, 150.0, "#FF0000").expect("add_underline");
        let doc2 = Document::load_mem(&underlined).expect("load underlined");
        let pdict2 = doc2.get_dictionary(pids1[0]).unwrap();
        let annots2 = pdict2.get(b"Annots").unwrap().as_array().unwrap();
        let annot2_ref = annots2[0].as_reference().unwrap();
        let annot2_dict = doc2.get_dictionary(annot2_ref).unwrap();

        assert_eq!(
            annot2_dict.get(b"Subtype").unwrap().as_name().unwrap(),
            b"Underline"
        );
        let qp2 = annot2_dict
            .get(b"QuadPoints")
            .expect("QuadPoints must exist")
            .as_array()
            .unwrap();
        assert_eq!(qp2.len(), 8, "QuadPoints must be 8 numbers");
    }

    #[test]
    fn test_phase3_unicode_font_pipeline_cjk_extraction_and_pdftotext() {
        let initial_pdf = create_test_pdf(1);

        // Mandatory regression strings:
        // 1. "これはうんちです"
        // 2. "文書作成テスト"
        // 3. "こんにちは世界"
        // 4. "PDFテスト 123 ABC"
        let strings_to_test = vec![
            "これはうんちです",
            "文書作成テスト",
            "こんにちは世界",
            "PDFテスト 123 ABC",
        ];

        let mut current_pdf = initial_pdf;
        let mut y = 650.0;
        for s in &strings_to_test {
            current_pdf = add_text(&current_pdf, 0, s, 50.0, y, 14.0, "#000000")
                .expect("add_text with Unicode string must succeed");
            y -= 40.0;
        }

        // Verify Type0 and ToUnicode CMap in PDF object graph
        let doc = Document::load_mem(&current_pdf).expect("Load Unicode PDF");
        let mut found_type0 = false;
        let mut found_tounicode_cmap = false;

        for (_, obj) in &doc.objects {
            if let Object::Dictionary(dict) = obj {
                if dict.get(b"Type").ok().and_then(|o| o.as_name().ok()) == Some(b"Font")
                    && dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(b"Type0")
                {
                    found_type0 = true;
                    if dict.get(b"ToUnicode").is_ok() {
                        found_tounicode_cmap = true;
                    }
                }
            }
        }

        assert!(found_type0, "Must have Type0 Font in document");
        assert!(
            found_tounicode_cmap,
            "Must have ToUnicode CMap attached to Type0 Font"
        );

        // External verification via pdftotext CLI
        let tmp_pdf_path =
            std::env::temp_dir().join(format!("cjk_test_{}.pdf", std::process::id()));
        std::fs::write(&tmp_pdf_path, &current_pdf).unwrap();

        if let Some(tool) = find_tool("pdftotext") {
            let pdftotext_res = std::process::Command::new(tool)
                .arg(&tmp_pdf_path)
                .arg("-")
                .output();

            if let Ok(output) = pdftotext_res {
                if output.status.success() {
                    let extracted_text = String::from_utf8_lossy(&output.stdout);
                    for target_str in &strings_to_test {
                        assert!(
                            extracted_text.contains(target_str),
                            "pdftotext output must contain exact Unicode string '{target_str}'. Got: {extracted_text}"
                        );
                    }
                }
            }
        } else {
            eprintln!("pdftotext not found on system; skipped external text extraction assertion");
        }

        let _ = std::fs::remove_file(&tmp_pdf_path);
    }

    #[test]
    fn test_unchi_survives_save_reload_without_mutation() {
        let initial_pdf = create_test_pdf(1);
        let unchi_fixture = "これはうんちです";

        // 1. Render/add text
        let pdf_with_unchi = add_text(
            &initial_pdf,
            0,
            unchi_fixture,
            100.0,
            500.0,
            16.0,
            "#000000",
        )
        .expect("add_text with unchi fixture must succeed");

        // 2. Save to disk and reload
        let tmp_pdf_path =
            std::env::temp_dir().join(format!("unchi_fixture_{}.pdf", std::process::id()));
        std::fs::write(&tmp_pdf_path, &pdf_with_unchi).expect("Write to disk");

        let reloaded_bytes = std::fs::read(&tmp_pdf_path).expect("Reload from disk");
        let reloaded_doc = Document::load_mem(&reloaded_bytes).expect("Parse reloaded PDF");

        // Verify structure survives reload
        let pids = get_page_ids(&reloaded_doc);
        assert!(!pids.is_empty(), "Reloaded PDF must have pages");

        // 3. Extract text via external pdftotext
        if let Some(tool) = find_tool("pdftotext") {
            let pdftotext_res = std::process::Command::new(tool)
                .arg(&tmp_pdf_path)
                .arg("-")
                .output()
                .expect("pdftotext execution");

            let _ = std::fs::remove_file(&tmp_pdf_path);

            assert!(pdftotext_res.status.success(), "pdftotext must succeed");
            let extracted_text = String::from_utf8_lossy(&pdftotext_res.stdout);

            // 4. Strict assertions: must be exact fixture, not mutated
            assert!(
                extracted_text.contains(unchi_fixture),
                "Saved and reloaded PDF must contain unmutated '{unchi_fixture}'. Got: '{extracted_text}'"
            );
            assert!(
                !extracted_text.contains("ウンチ"),
                "Forbidden mutation: Katakana ウンチ detected!"
            );
            assert!(
                !extracted_text.contains("うんち "),
                "Forbidden mutation: Trailing space in うんち detected!"
            );
            assert!(
                !extracted_text.contains(" うんち"),
                "Forbidden mutation: Leading space in うんち detected!"
            );
        } else {
            let _ = std::fs::remove_file(&tmp_pdf_path);
            eprintln!("pdftotext not found on system; skipped external text extraction assertion");
        }
    }

    #[test]
    fn test_hostile_nested_page_tree_ops() {
        use crate::pdf_engine::page_tree::*;
        use lopdf::Stream;

        // Construct a hostile 2-level nested page tree with inherited attributes:
        // Root Pages (MediaBox = [0, 0, 600, 800])
        //   ├─ Intermediate Pages A (Rotate = 90, MediaBox = [0, 0, 500, 700])
        //   │    ├─ Page 1 (has Image XObject and Flate stream)
        //   │    └─ Page 2 (Contents Array of 2 streams)
        //   └─ Intermediate Pages B (Rotate = 180)
        //        ├─ Page 3 (Annotation)
        //        └─ Page 4 (plain)
        let mut doc = Document::with_version("1.7");

        // MediaBox inherited at root Pages
        let root_pages_id = doc.new_object_id();

        // Intermediate Pages A
        let pages_a_id = doc.new_object_id();
        // Intermediate Pages B
        let pages_b_id = doc.new_object_id();

        // 1. Page 1: with FlateDecode Content and Form/Image XObject
        let img_stream_bytes = vec![0xFF; 64];
        let mut img_dict = Dictionary::new();
        img_dict.set("Type", Object::Name("XObject".into()));
        img_dict.set("Subtype", Object::Name("Image".into()));
        img_dict.set("Width", Object::Integer(8));
        img_dict.set("Height", Object::Integer(8));
        img_dict.set("ColorSpace", Object::Name("DeviceRGB".into()));
        img_dict.set("BitsPerComponent", Object::Integer(8));
        let img_id = doc.add_object(Stream::new(img_dict, img_stream_bytes));

        let mut xobj_dict = Dictionary::new();
        xobj_dict.set("Im1", Object::Reference(img_id));
        let mut res1_dict = Dictionary::new();
        res1_dict.set("XObject", Object::Dictionary(xobj_dict));

        let c1 = lopdf::content::Content {
            operations: vec![
                lopdf::content::Operation::new("q", vec![]),
                lopdf::content::Operation::new("Do", vec![Object::Name("Im1".into())]),
                lopdf::content::Operation::new("Q", vec![]),
            ],
        };
        let c1_bytes = c1.encode().unwrap();
        let c1_stream = Stream::new(Dictionary::new(), c1_bytes);
        let c1_id = doc.add_object(c1_stream);

        let mut p1_dict = Dictionary::new();
        p1_dict.set("Type", Object::Name("Page".into()));
        p1_dict.set("Parent", Object::Reference(pages_a_id));
        p1_dict.set("Resources", Object::Dictionary(res1_dict));
        p1_dict.set("Contents", Object::Reference(c1_id));
        let p1_id = doc.add_object(Object::Dictionary(p1_dict));

        // 2. Page 2: Contents Array of 2 streams
        let sa = doc.add_object(Stream::new(
            Dictionary::new(),
            b"q 1 0 0 1 10 10 cm Q\n".to_vec(),
        ));
        let sb = doc.add_object(Stream::new(
            Dictionary::new(),
            b"q 1 0 0 1 20 20 cm Q\n".to_vec(),
        ));
        let mut p2_dict = Dictionary::new();
        p2_dict.set("Type", Object::Name("Page".into()));
        p2_dict.set("Parent", Object::Reference(pages_a_id));
        p2_dict.set(
            "Contents",
            Object::Array(vec![Object::Reference(sa), Object::Reference(sb)]),
        );
        let p2_id = doc.add_object(Object::Dictionary(p2_dict));

        // 3. Page 3: with Annotation
        let mut annot_dict = Dictionary::new();
        annot_dict.set("Type", Object::Name("Annot".into()));
        annot_dict.set("Subtype", Object::Name("Text".into()));
        annot_dict.set(
            "Rect",
            Object::Array(vec![
                Object::Integer(10),
                Object::Integer(10),
                Object::Integer(50),
                Object::Integer(50),
            ]),
        );
        annot_dict.set(
            "Contents",
            Object::String(b"Hostile Annotation".to_vec(), lopdf::StringFormat::Literal),
        );
        let annot_id = doc.add_object(Object::Dictionary(annot_dict));

        let mut p3_dict = Dictionary::new();
        p3_dict.set("Type", Object::Name("Page".into()));
        p3_dict.set("Parent", Object::Reference(pages_b_id));
        p3_dict.set("Annots", Object::Array(vec![Object::Reference(annot_id)]));
        let p3_id = doc.add_object(Object::Dictionary(p3_dict));

        // 4. Page 4: plain
        let mut p4_dict = Dictionary::new();
        p4_dict.set("Type", Object::Name("Page".into()));
        p4_dict.set("Parent", Object::Reference(pages_b_id));
        let p4_id = doc.add_object(Object::Dictionary(p4_dict));

        // Pages A
        let mut pa_dict = Dictionary::new();
        pa_dict.set("Type", Object::Name("Pages".into()));
        pa_dict.set("Parent", Object::Reference(root_pages_id));
        pa_dict.set(
            "Kids",
            Object::Array(vec![Object::Reference(p1_id), Object::Reference(p2_id)]),
        );
        pa_dict.set("Count", Object::Integer(2));
        pa_dict.set("Rotate", Object::Integer(90));
        pa_dict.set(
            "MediaBox",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(500),
                Object::Integer(700),
            ]),
        );
        doc.objects.insert(pages_a_id, Object::Dictionary(pa_dict));

        // Pages B
        let mut pb_dict = Dictionary::new();
        pb_dict.set("Type", Object::Name("Pages".into()));
        pb_dict.set("Parent", Object::Reference(root_pages_id));
        pb_dict.set(
            "Kids",
            Object::Array(vec![Object::Reference(p3_id), Object::Reference(p4_id)]),
        );
        pb_dict.set("Count", Object::Integer(2));
        pb_dict.set("Rotate", Object::Integer(180));
        doc.objects.insert(pages_b_id, Object::Dictionary(pb_dict));

        // Root Pages
        let mut rpages_dict = Dictionary::new();
        rpages_dict.set("Type", Object::Name("Pages".into()));
        rpages_dict.set(
            "Kids",
            Object::Array(vec![
                Object::Reference(pages_a_id),
                Object::Reference(pages_b_id),
            ]),
        );
        rpages_dict.set("Count", Object::Integer(4));
        rpages_dict.set(
            "MediaBox",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(600),
                Object::Integer(800),
            ]),
        );
        doc.objects
            .insert(root_pages_id, Object::Dictionary(rpages_dict));

        // Catalog
        let mut cat_dict = Dictionary::new();
        cat_dict.set("Type", Object::Name("Catalog".into()));
        cat_dict.set("Pages", Object::Reference(root_pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat_dict));

        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut hostile_pdf_bytes = Vec::new();
        doc.save_to(&mut hostile_pdf_bytes)
            .expect("Save hostile PDF");

        // Verify logical page resolution across 2-level nested tree
        let parsed_doc = Document::load_mem(&hostile_pdf_bytes).expect("Load hostile PDF");
        let logical_ids = get_logical_page_ids(&parsed_doc);
        assert_eq!(logical_ids.len(), 4, "Must resolve 4 logical pages");
        assert_eq!(logical_ids, vec![p1_id, p2_id, p3_id, p4_id]);

        // TEST 1: Delete logical page 2 (which is p3_id in nested Pages B!)
        let after_del_bytes = delete_page(&hostile_pdf_bytes, 2).expect("Delete logical page 2");
        let del_doc = Document::load_mem(&after_del_bytes).expect("Load after delete");
        let del_page_ids = get_logical_page_ids(&del_doc);
        assert_eq!(del_page_ids.len(), 3, "Page count must now be 3");

        // TEST 2: Extract pages [0, 2] from hostile PDF (p1 with image, and p3 with annot/rotate 180)
        let extracted_bytes = extract_pages(&hostile_pdf_bytes, &[0, 2]).expect("Extract pages");
        let ext_doc = Document::load_mem(&extracted_bytes).expect("Load extracted PDF");
        let ext_page_ids = get_logical_page_ids(&ext_doc);
        assert_eq!(ext_page_ids.len(), 2, "Extracted doc must have 2 pages");

        // Verify inherited MediaBox and Rotate were materialized on extracted page 0
        let ext_p0 = ext_doc
            .objects
            .get(&ext_page_ids[0])
            .unwrap()
            .as_dict()
            .unwrap();
        assert!(
            ext_p0.get(b"MediaBox").is_ok(),
            "Extracted page 0 must have materialized MediaBox"
        );
        assert_eq!(
            ext_p0.get(b"Rotate").unwrap().as_i64().unwrap(),
            90,
            "Extracted page 0 must have materialized Rotate = 90"
        );
        // Verify XObject resource was copied over
        assert!(ext_p0.get(b"Resources").is_ok(), "Resources must be copied");

        // Verify extracted page 1 (was logical page 2 in Pages B)
        let ext_p1 = ext_doc
            .objects
            .get(&ext_page_ids[1])
            .unwrap()
            .as_dict()
            .unwrap();
        assert_eq!(
            ext_p1.get(b"Rotate").unwrap().as_i64().unwrap(),
            180,
            "Extracted page 1 must have materialized Rotate = 180"
        );
        assert!(
            ext_p1.get(b"Annots").is_ok(),
            "Annots must be preserved on extracted page"
        );

        // TEST 3: Merge hostile PDF with extracted PDF
        let merged_bytes =
            merge_pdf_buffers_robust(&[&hostile_pdf_bytes, &extracted_bytes]).expect("Merge PDFs");
        let merged_doc = Document::load_mem(&merged_bytes).expect("Load merged PDF");
        let merged_ids = get_logical_page_ids(&merged_doc);
        assert_eq!(merged_ids.len(), 6, "Merged PDF must have 4 + 2 = 6 pages");

        // Verify every page in merged document has /Parent pointing to the canonical /Pages
        let cat_ref = merged_doc
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        let cat = merged_doc.objects.get(&cat_ref).unwrap().as_dict().unwrap();
        assert_eq!(cat.get(b"Type").unwrap().as_name().unwrap(), b"Catalog");
        let pages_ref = cat.get(b"Pages").unwrap().as_reference().unwrap();
        for &pid in &merged_ids {
            let pdict = merged_doc.objects.get(&pid).unwrap().as_dict().unwrap();
            assert_eq!(
                pdict.get(b"Parent").unwrap().as_reference().unwrap(),
                pages_ref,
                "Every page's /Parent must point to canonical /Pages"
            );
        }

        // TEST 4: Reorder pages on hostile PDF
        let reordered_bytes = reorder_pages(&hostile_pdf_bytes, 3, 0).expect("Reorder pages");
        let reord_doc = Document::load_mem(&reordered_bytes).expect("Load reordered PDF");
        let reord_ids = get_logical_page_ids(&reord_doc);
        assert_eq!(reord_ids.len(), 4, "Reordered PDF must have 4 pages");

        // External tool verification via qpdf --check and pdfinfo
        let tmp_merged =
            std::env::temp_dir().join(format!("hostile_merged_{}.pdf", std::process::id()));
        std::fs::write(&tmp_merged, &merged_bytes).unwrap();

        if let Some(tool) = find_tool("qpdf") {
            let qpdf_status = std::process::Command::new(tool)
                .arg("--check")
                .arg(&tmp_merged)
                .status();
            if let Ok(st) = qpdf_status {
                assert!(st.success(), "qpdf --check must succeed on merged hostile PDF");
            }
        } else {
            eprintln!("qpdf not found on system; skipped hostile PDF qpdf check");
        }

        let _ = std::fs::remove_file(&tmp_merged);
    }

    #[test]
    fn test_unicode_render_regression_full() {
        let initial_pdf = create_test_pdf(1);
        let mandatory_lines = vec![
            "これはうんちです",
            "こんにちは世界",
            "文書作成テスト",
            "PDFテスト 123 ABC",
            "漢字・ひらがな・カタカナ",
        ];

        let mut current_pdf = initial_pdf;
        let mut y = 700.0;
        for line in &mandatory_lines {
            current_pdf = add_text(&current_pdf, 0, line, 50.0, y, 16.0, "#1A202C")
                .expect("add_text must succeed for all mandatory lines");
            y -= 45.0;
        }

        let tmp_pdf =
            std::env::temp_dir().join(format!("unicode_render_test_{}.pdf", std::process::id()));
        std::fs::write(&tmp_pdf, &current_pdf).unwrap();

        // 1. Check with qpdf
        if let Some(tool) = find_tool("qpdf") {
            let qpdf_res = std::process::Command::new(tool)
                .arg("--check")
                .arg(&tmp_pdf)
                .output();
            if let Ok(res) = qpdf_res {
                assert!(
                    res.status.success(),
                    "qpdf --check must pass for Unicode PDF"
                );
            }
        } else {
            eprintln!("qpdf not found on system; skipped unicode qpdf check");
        }

        // 2. Extract with pdftotext
        if let Some(tool) = find_tool("pdftotext") {
            let pdftotext_res = std::process::Command::new(tool)
                .arg(&tmp_pdf)
                .arg("-")
                .output();
            if let Ok(res) = pdftotext_res {
                assert!(res.status.success(), "pdftotext must succeed");
                let extracted = String::from_utf8_lossy(&res.stdout);
                for line in &mandatory_lines {
                    assert!(
                        extracted.contains(line),
                        "Extracted text must contain '{line}'. Got:\n{extracted}"
                    );
                }
            }
        } else {
            eprintln!("pdftotext not found on system; skipped unicode pdftotext check");
        }

        // 3. Render with pdftoppm to PNG and verify rendering success
        let tmp_png_prefix =
            std::env::temp_dir().join(format!("rendered_page_{}", std::process::id()));
        if let Some(tool) = find_tool("pdftoppm") {
            let pdftoppm_res = std::process::Command::new(tool)
                .arg("-png")
                .arg("-r")
                .arg("150")
                .arg(&tmp_pdf)
                .arg(&tmp_png_prefix)
                .status();
            if let Ok(st) = pdftoppm_res {
                assert!(st.success(), "pdftoppm must succeed");
                let expected_png = format!("{}-1.png", tmp_png_prefix.display());
                assert!(
                    std::path::Path::new(&expected_png).exists(),
                    "Rendered PNG must be produced by pdftoppm"
                );
                let metadata = std::fs::metadata(&expected_png).unwrap();
                assert!(
                    metadata.len() > 1000,
                    "Rendered PNG must not be empty or blank"
                );
                let _ = std::fs::remove_file(&expected_png);
            }
        } else {
            eprintln!("pdftoppm not found on system; skipped unicode pdftoppm check");
        }

        let _ = std::fs::remove_file(&tmp_pdf);
    }

    #[test]
    fn test_delete_page_retains_inherited_attributes() {
        use crate::pdf_engine::page_tree::*;

        // Create a tree where an intermediate Pages node sets Rotate = 90 and MediaBox = [0, 0, 400, 600]
        let mut doc = Document::with_version("1.7");
        let (_root_id, pages_id) = ensure_catalog_and_pages_root(&mut doc);

        let p1_id = doc.add_object(Object::Dictionary(Dictionary::new()));
        let p2_id = doc.add_object(Object::Dictionary(Dictionary::new()));

        let mut inter_dict = Dictionary::new();
        inter_dict.set("Type", Object::Name(b"Pages".to_vec()));
        inter_dict.set("Parent", Object::Reference(pages_id));
        inter_dict.set("Rotate", Object::Integer(90));
        inter_dict.set(
            "MediaBox",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(400),
                Object::Integer(600),
            ]),
        );
        inter_dict.set(
            "Kids",
            Object::Array(vec![Object::Reference(p1_id), Object::Reference(p2_id)]),
        );
        inter_dict.set("Count", Object::Integer(2));
        let inter_id = doc.add_object(Object::Dictionary(inter_dict));

        // Connect p1 and p2 to inter_id
        if let Some(Object::Dictionary(ref mut p1)) = doc.objects.get_mut(&p1_id) {
            p1.set("Type", Object::Name(b"Page".to_vec()));
            p1.set("Parent", Object::Reference(inter_id));
        }
        if let Some(Object::Dictionary(ref mut p2)) = doc.objects.get_mut(&p2_id) {
            p2.set("Type", Object::Name(b"Page".to_vec()));
            p2.set("Parent", Object::Reference(inter_id));
        }

        // Connect root pages to intermediate
        if let Some(Object::Dictionary(ref mut root_pages)) = doc.objects.get_mut(&pages_id) {
            root_pages.set("Kids", Object::Array(vec![Object::Reference(inter_id)]));
            root_pages.set("Count", Object::Integer(2));
        }

        let mut raw_bytes = Vec::new();
        doc.save_to(&mut raw_bytes).expect("Save raw hostile doc");

        // Now delete page index 1 (p2). Page 0 (p1) must retain Rotate = 90 and MediaBox!
        let modified_bytes = delete_page(&raw_bytes, 1).expect("delete_page must succeed");
        let modified_doc = Document::load_mem(&modified_bytes).expect("Reload after delete");

        let logical_pages = get_logical_page_ids(&modified_doc);
        assert_eq!(logical_pages.len(), 1, "Must have exactly 1 page remaining");

        let remaining_pdict = modified_doc
            .objects
            .get(&logical_pages[0])
            .and_then(|o| o.as_dict().ok())
            .expect("Remaining page dictionary");

        let rot = remaining_pdict
            .get(b"Rotate")
            .ok()
            .and_then(|r| r.as_i64().ok())
            .unwrap_or(0);
        assert_eq!(
            rot, 90,
            "Remaining page must retain inherited Rotate = 90 after sibling deletion"
        );

        let mbox = remaining_pdict
            .get(b"MediaBox")
            .ok()
            .and_then(|m| m.as_array().ok());
        assert!(
            mbox.is_some(),
            "Remaining page must retain inherited MediaBox after sibling deletion"
        );
    }

    #[test]
    fn test_copy_object_graph_preserves_widget_parent_and_skips_annot_p() {
        use crate::pdf_engine::page_tree::copy_object_graph;
        use std::collections::HashMap;

        let mut src_doc = Document::with_version("1.7");
        let mut dest_doc = Document::with_version("1.7");
        // Add dummy objects in dest_doc so its OIDs don't coincidentally match src_doc
        dest_doc.add_object(Object::Integer(42));
        dest_doc.add_object(Object::Integer(43));

        // Create a Source Page
        let src_page_id = src_doc.add_object(Object::Dictionary(Dictionary::new()));

        // Create an AcroForm Field and a Widget with /Parent pointing to Field
        let mut field_dict = Dictionary::new();
        field_dict.set("Type", Object::Name(b"Field".to_vec()));
        field_dict.set("T", Object::string_literal("UserName"));
        let field_id = src_doc.add_object(Object::Dictionary(field_dict));

        let mut widget_dict = Dictionary::new();
        widget_dict.set("Type", Object::Name(b"Annot".to_vec()));
        widget_dict.set("Subtype", Object::Name(b"Widget".to_vec()));
        widget_dict.set("Parent", Object::Reference(field_id));
        widget_dict.set("P", Object::Reference(src_page_id)); // Back-reference to Page
        let widget_id = src_doc.add_object(Object::Dictionary(widget_dict));

        let mut id_map = HashMap::new();
        let dest_widget_id = copy_object_graph(&src_doc, &mut dest_doc, widget_id, &mut id_map);

        let copied_widget = dest_doc
            .objects
            .get(&dest_widget_id)
            .and_then(|o| o.as_dict().ok())
            .expect("Copied widget dict");

        // 1. /Parent MUST NOT have been skipped because it's a form widget!
        assert!(
            copied_widget.get(b"Parent").is_ok(),
            "Widget /Parent must NOT be stripped by copy_object_graph"
        );
        let new_parent_ref = copied_widget.get(b"Parent").unwrap().as_reference().unwrap();
        assert_ne!(
            new_parent_ref, field_id,
            "Parent field reference must be remapped to new object in dest_doc"
        );
        assert!(
            dest_doc.objects.contains_key(&new_parent_ref),
            "dest_doc must contain the copied parent Field object"
        );

        // 2. /P on Annot MUST have been skipped to prevent source page inclusion!
        assert!(
            copied_widget.get(b"P").is_err(),
            "Annot /P back-reference to Page must be stripped to prevent recursive source page inclusion"
        );
    }

    #[test]
    fn test_ocr_fallback_encodes_cids_correctly() {
        let temp_output =
            std::env::temp_dir().join(format!("searchable_fallback_cjk_{}.pdf", std::process::id()));
        let output_path = temp_output.to_string_lossy().to_string();

        let unchi_text = "これはうんちです\nこんにちは世界";
        crate::ocr_engine::create_searchable_pdf(&[], unchi_text, &output_path)
            .expect("create_searchable_pdf fallback should succeed");

        let pdf_bytes = std::fs::read(&output_path).expect("Read output PDF");
        let _ = std::fs::remove_file(&output_path);

        let doc = Document::load_mem(&pdf_bytes).expect("Load OCR fallback PDF");
        let page_ids = get_page_ids(&doc);
        assert!(!page_ids.is_empty());

        // Extract with pdftotext
        let tmp_pdf_path =
            std::env::temp_dir().join(format!("ocr_fb_pdftotext_{}.pdf", std::process::id()));
        std::fs::write(&tmp_pdf_path, &pdf_bytes).unwrap();

        if let Some(tool) = find_tool("pdftotext") {
            let res = std::process::Command::new(tool)
                .arg(&tmp_pdf_path)
                .arg("-")
                .output()
                .expect("pdftotext execution");
            assert!(res.status.success());
            let extracted = String::from_utf8_lossy(&res.stdout);
            assert!(
                extracted.contains("これはうんちです"),
                "OCR fallback must produce valid CID mapped text extractable as exact Unicode. Got: {extracted}"
            );
            assert!(
                extracted.contains("こんにちは世界"),
                "OCR fallback must produce valid CID mapped text extractable as exact Unicode. Got: {extracted}"
            );
        }
        let _ = std::fs::remove_file(&tmp_pdf_path);
    }

    #[test]
    fn test_pdf_x_validation_with_inherited_mediabox_and_resources() {
        // Construct a PDF where MediaBox and Resources are inherited from /Pages
        let mut doc = Document::with_version("1.7");
        let root_pages_id = doc.new_object_id();
        let page_id = doc.new_object_id();

        let mut res_dict = Dictionary::new();
        res_dict.set("Font", Object::Dictionary(Dictionary::new()));
        let res_id = doc.add_object(Object::Dictionary(res_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages_dict.set("Count", Object::Integer(1));
        pages_dict.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(842.0),
                Object::Real(1191.0), // A3 dimensions
            ]),
        );
        pages_dict.set("Resources", Object::Reference(res_id));
        doc.objects.insert(root_pages_id, Object::Dictionary(pages_dict));

        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name(b"Page".to_vec()));
        page_dict.set("Parent", Object::Reference(root_pages_id));
        // Intentionally no direct MediaBox, Resources, TrimBox on page_dict
        doc.objects.insert(page_id, Object::Dictionary(page_dict));

        let mut cat_dict = Dictionary::new();
        cat_dict.set("Type", Object::Name(b"Catalog".to_vec()));
        cat_dict.set("Pages", Object::Reference(root_pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat_dict));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut raw_bytes = Vec::new();
        doc.save_to(&mut raw_bytes).unwrap();

        // Convert to PDF/X-4
        let pdfx_bytes = crate::pdf_engine::pdf_x::convert_to_pdfx_standard(
            &raw_bytes,
            "PDF/X-4",
            "Japan Color 2001 Coated",
        )
        .expect("convert_to_pdfx_standard with inherited MediaBox");

        let converted_doc = Document::load_mem(&pdfx_bytes).expect("Load converted PDF/X");
        let pdict = converted_doc.get_dictionary(page_id).unwrap();

        // TrimBox must inherit A3 dimensions (842x1191), NOT 595x842!
        let trim_box = pdict.get(b"TrimBox").unwrap().as_array().unwrap();
        let trim_w = trim_box[2].as_float().unwrap();
        let trim_h = trim_box[3].as_float().unwrap();
        assert_eq!(trim_w, 842.0, "TrimBox width must preserve inherited MediaBox A3 width");
        assert_eq!(trim_h, 1191.0, "TrimBox height must preserve inherited MediaBox A3 height");

        // Validate
        let report = crate::pdf_engine::pdf_x::validate_pdfx_compliance(&pdfx_bytes, "PDF/X-4")
            .expect("validate_pdfx_compliance");
        assert!(
            report.is_compliant,
            "PDF/X-4 validation must succeed with inherited MediaBox and Resources"
        );
    }

    #[test]
    fn test_deep_redact_cid_font_in_form_xobject() {
        // Construct the ultimate hostile fixture:
        // Type0 CID font + Form XObject containing CID-encoded "これはうんちです" + inherited Resources + Rotate 90
        let initial_pdf = create_test_pdf(1);
        let unchi_str = "これはうんちです";

        // 1. First embed a Type0 Unicode font in the PDF
        let mut doc = Document::load_mem(&initial_pdf).expect("Load initial");
        let encoder = crate::pdf_engine::font_unicode::create_unicode_font_encoder(&mut doc, unchi_str)
            .expect("Create unicode font encoder");
        let type0_font_id = encoder.font_id;
        let encoded_unchi_cids = encoder.encode_text(unchi_str);

        // 2. Build Form XObject content stream using the Type0 font
        let form_content = lopdf::content::Content {
            operations: vec![
                lopdf::content::Operation::new("BT", vec![]),
                lopdf::content::Operation::new(
                    "Tf",
                    vec![Object::Name(b"UniF".to_vec()), Object::Real(16.0)],
                ),
                lopdf::content::Operation::new(
                    "Td",
                    vec![Object::Real(100.0), Object::Real(500.0)],
                ),
                lopdf::content::Operation::new(
                    "Tj",
                    vec![Object::String(
                        encoded_unchi_cids.clone(),
                        lopdf::StringFormat::Hexadecimal,
                    )],
                ),
                lopdf::content::Operation::new("ET", vec![]),
            ],
        };
        let form_bytes = form_content.encode().unwrap();

        let mut form_fonts = Dictionary::new();
        form_fonts.set("UniF", Object::Reference(type0_font_id));
        let mut form_res = Dictionary::new();
        form_res.set("Font", Object::Dictionary(form_fonts));

        let mut form_dict = Dictionary::new();
        form_dict.set("Type", Object::Name(b"XObject".to_vec()));
        form_dict.set("Subtype", Object::Name(b"Form".to_vec()));
        form_dict.set(
            "BBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(500.0),
                Object::Real(500.0),
            ]),
        );
        form_dict.set("Resources", Object::Dictionary(form_res));

        let form_xobj_id = doc.add_object(Stream::new(form_dict, form_bytes));

        // 3. Put Form XObject into page's Resources and invoke it via /Do
        let page_ids = get_page_ids(&doc);
        let pid = page_ids[0];
        if let Some(Object::Dictionary(ref mut pdict)) = doc.objects.get_mut(&pid) {
            pdict.set("Rotate", Object::Integer(90));
            let mut xobjs = Dictionary::new();
            xobjs.set("Fm1", Object::Reference(form_xobj_id));
            let mut res = Dictionary::new();
            res.set("XObject", Object::Dictionary(xobjs));
            pdict.set("Resources", Object::Dictionary(res));
        }

        // Draw the form on the page
        let page_invoke = lopdf::content::Content {
            operations: vec![
                lopdf::content::Operation::new("q", vec![]),
                lopdf::content::Operation::new("Do", vec![Object::Name(b"Fm1".to_vec())]),
                lopdf::content::Operation::new("Q", vec![]),
            ],
        };
        let page_invoke_bytes = page_invoke.encode().unwrap();
        let page_stream_id = doc.add_object(Stream::new(Dictionary::new(), page_invoke_bytes));
        if let Some(Object::Dictionary(ref mut pdict)) = doc.objects.get_mut(&pid) {
            pdict.set("Contents", Object::Reference(page_stream_id));
        }

        let mut hostile_bytes = Vec::new();
        doc.save_to(&mut hostile_bytes).unwrap();

        // 4. Verify that the hostile fixture initially contains "これはうんちです" via pdftotext
        let tmp_hostile =
            std::env::temp_dir().join(format!("hostile_form_before_{}.pdf", std::process::id()));
        std::fs::write(&tmp_hostile, &hostile_bytes).unwrap();

        if let Some(tool) = find_tool("pdftotext") {
            let res = std::process::Command::new(tool)
                .arg(&tmp_hostile)
                .arg("-")
                .output()
                .expect("pdftotext execution");
            let extracted = String::from_utf8_lossy(&res.stdout);
            assert!(
                extracted.contains(unchi_str),
                "Initial hostile PDF must contain unchi inside Form XObject. Got: {extracted}"
            );
        }
        let _ = std::fs::remove_file(&tmp_hostile);

        // 5. DEEP REDACT "これはうんちです"
        let redacted_bytes = crate::pdf_engine::redact::redact_text_deep(&hostile_bytes, unchi_str, "#000000")
            .expect("redact_text_deep must successfully redact CID text inside Form XObject");

        // 6. Strict assertions on redacted document:
        // A. Form XObject stream MUST NOT contain the CID text or operator anymore
        let redacted_doc = Document::load_mem(&redacted_bytes).expect("Load redacted doc");
        let redacted_form_st = redacted_doc.objects.get(&form_xobj_id).unwrap().as_stream().unwrap();
        let decompressed_form = redacted_form_st
            .decompressed_content()
            .unwrap_or_else(|_| redacted_form_st.content.clone());
        let form_content_after = lopdf::content::Content::decode(&decompressed_form).unwrap();
        let has_text_op = form_content_after.operations.iter().any(|op| op.operator == "Tj" || op.operator == "TJ");
        assert!(!has_text_op, "Text operation inside Form XObject must be completely purged by deep redact");

        // B. pdftotext on redacted PDF must NOT contain "これはうんちです"
        let tmp_redacted =
            std::env::temp_dir().join(format!("hostile_form_after_{}.pdf", std::process::id()));
        std::fs::write(&tmp_redacted, &redacted_bytes).unwrap();

        if let Some(tool) = find_tool("pdftotext") {
            let res = std::process::Command::new(tool)
                .arg(&tmp_redacted)
                .arg("-")
                .output()
                .expect("pdftotext execution");
            let extracted = String::from_utf8_lossy(&res.stdout);
            assert!(
                !extracted.contains(unchi_str),
                "Redacted hostile PDF must NOT contain unchi! Got: {extracted}"
            );
        }
        let _ = std::fs::remove_file(&tmp_redacted);
    }

    #[test]
    fn test_office_exports_produce_valid_openxml_zips() {
        let pdf_data = create_test_pdf(2);
        let tmp_dir = std::env::temp_dir().join(format!("test_openxml_{}", std::process::id()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        // 1. Word (.docx)
        let docx_path = tmp_dir.join("test.docx");
        crate::pdf_engine::export_office::pdf_to_word(&pdf_data, docx_path.to_str().unwrap())
            .expect("pdf_to_word failed");
        let docx_file = std::fs::File::open(&docx_path).unwrap();
        let mut docx_zip = zip::ZipArchive::new(docx_file).expect("Word output must be valid ZIP");
        assert!(docx_zip.by_name("[Content_Types].xml").is_ok());
        assert!(docx_zip.by_name("_rels/.rels").is_ok());
        assert!(docx_zip.by_name("word/document.xml").is_ok());

        // 2. Excel (.xlsx)
        let xlsx_path = tmp_dir.join("test.xlsx");
        crate::pdf_engine::export_office::pdf_to_excel(&pdf_data, xlsx_path.to_str().unwrap())
            .expect("pdf_to_excel failed");
        let xlsx_file = std::fs::File::open(&xlsx_path).unwrap();
        let mut xlsx_zip = zip::ZipArchive::new(xlsx_file).expect("Excel output must be valid ZIP");
        assert!(xlsx_zip.by_name("[Content_Types].xml").is_ok());
        assert!(xlsx_zip.by_name("_rels/.rels").is_ok());
        assert!(xlsx_zip.by_name("xl/workbook.xml").is_ok());
        assert!(xlsx_zip.by_name("xl/worksheets/sheet1.xml").is_ok());

        // 3. PowerPoint (.pptx)
        let pptx_path = tmp_dir.join("test.pptx");
        crate::pdf_engine::export_office::pdf_to_powerpoint(&pdf_data, pptx_path.to_str().unwrap())
            .expect("pdf_to_powerpoint failed");
        let pptx_file = std::fs::File::open(&pptx_path).unwrap();
        let mut pptx_zip = zip::ZipArchive::new(pptx_file).expect("PowerPoint output must be valid ZIP");
        assert!(pptx_zip.by_name("[Content_Types].xml").is_ok());
        assert!(pptx_zip.by_name("_rels/.rels").is_ok());
        assert!(pptx_zip.by_name("ppt/presentation.xml").is_ok());
        assert!(pptx_zip.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(pptx_zip.by_name("ppt/slides/slide2.xml").is_ok());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_optimize_pdf_preserves_object_references() {
        let mut doc = Document::with_version("1.7");
        let content_data = b"0.5 0.5 0.5 rg 10 10 100 100 re f";
        let stream = lopdf::Stream::new(Dictionary::new(), content_data.to_vec());
        let stream_id = doc.add_object(Object::Stream(stream));

        // Two pages referencing the EXACT SAME stream content object or empty dictionaries
        let mut p1 = Dictionary::new();
        p1.set("Type", Object::Name("Page".into()));
        p1.set("Contents", Object::Reference(stream_id));
        p1.set("MediaBox", Object::Array(vec![Object::Real(0.0), Object::Real(0.0), Object::Real(595.0), Object::Real(842.0)]));
        let p1_id = doc.add_object(Object::Dictionary(p1));

        let mut p2 = Dictionary::new();
        p2.set("Type", Object::Name("Page".into()));
        p2.set("Contents", Object::Reference(stream_id));
        p2.set("MediaBox", Object::Array(vec![Object::Real(0.0), Object::Real(0.0), Object::Real(595.0), Object::Real(842.0)]));
        let p2_id = doc.add_object(Object::Dictionary(p2));

        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name("Pages".into()));
        pages.set("Kids", Object::Array(vec![Object::Reference(p1_id), Object::Reference(p2_id)]));
        pages.set("Count", Object::Integer(2));
        let pages_id = doc.add_object(Object::Dictionary(pages));

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut initial_pdf = Vec::new();
        doc.save_to(&mut initial_pdf).unwrap();

        // Optimize PDF must NOT delete referenced objects or produce dangling references
        let optimized = crate::pdf_engine::inspect::optimize_pdf(&initial_pdf).expect("Optimize must succeed");
        let loaded_doc = Document::load_mem(&optimized).expect("Optimized PDF must be valid and readable");

        let p_ids = get_page_ids(&loaded_doc);
        assert_eq!(p_ids.len(), 2, "Optimized PDF must preserve all pages");

        for pid in p_ids {
            let page_dict = loaded_doc.objects.get(&pid).unwrap().as_dict().unwrap();
            let c_ref = page_dict.get(b"Contents").unwrap().as_reference().unwrap();
            assert!(loaded_doc.objects.get(&c_ref).is_some(), "Contents reference must exist and not be a dangling dead pointer!");
        }
    }

    #[test]
    fn test_header_footer_and_bates_cjk_embedding() {
        let mut doc = Document::with_version("1.7");
        let mut p = Dictionary::new();
        p.set("Type", Object::Name("Page".into()));
        p.set("MediaBox", Object::Array(vec![Object::Real(0.0), Object::Real(0.0), Object::Real(595.0), Object::Real(842.0)]));
        let pid = doc.add_object(Object::Dictionary(p));

        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name("Pages".into()));
        pages.set("Kids", Object::Array(vec![Object::Reference(pid)]));
        pages.set("Count", Object::Integer(1));
        let pages_id = doc.add_object(Object::Dictionary(pages));

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut base_pdf = Vec::new();
        doc.save_to(&mut base_pdf).unwrap();

        // 1. Test header footer with Japanese text
        let with_hf = crate::pdf_engine::batch_ops::add_header_footer(
            &base_pdf,
            "【社外秘】渚文書第 {page} 頁",
            "株式会社ナギサ 全 {total} 頁",
            10.0,
            20.0,
        ).expect("add_header_footer with CJK must succeed");

        let hf_doc = Document::load_mem(&with_hf).expect("Must load valid PDF");
        let hf_page_id = get_page_ids(&hf_doc)[0];
        let hf_res = crate::pdf_engine::common::resolve_page_resources(&hf_doc, hf_page_id);
        let hf_fonts = hf_res.get(b"Font").unwrap().as_dict().unwrap();
        let font_ref = hf_fonts.get(b"HeaderFooterFont").unwrap().as_reference().unwrap();
        let font_obj = hf_doc.objects.get(&font_ref).unwrap().as_dict().unwrap();
        assert_eq!(font_obj.get(b"Subtype").unwrap().as_name().unwrap(), b"Type0", "Must embed Type0 Unicode font for CJK header!");

        // 2. Test Bates numbering with Japanese prefix
        let with_bates = crate::pdf_engine::batch_ops::add_bates_number(
            &base_pdf,
            "証拠甲-",
            1,
            12.0,
            25.0,
        ).expect("add_bates_number with CJK must succeed");

        let bates_doc = Document::load_mem(&with_bates).expect("Must load valid PDF");
        let bates_page_id = get_page_ids(&bates_doc)[0];
        let bates_res = crate::pdf_engine::common::resolve_page_resources(&bates_doc, bates_page_id);
        let bates_fonts = bates_res.get(b"Font").unwrap().as_dict().unwrap();
        let bates_font_ref = bates_fonts.get(b"BatesFont").unwrap().as_reference().unwrap();
        let bates_font_obj = bates_doc.objects.get(&bates_font_ref).unwrap().as_dict().unwrap();
        assert_eq!(bates_font_obj.get(b"Subtype").unwrap().as_name().unwrap(), b"Type0", "Must embed Type0 Unicode font for CJK Bates prefix!");
    }

    #[test]
    fn test_xfdf_robust_xml_and_hierarchical_replies() {
        let base_pdf = create_test_pdf(2);

        // Acrobat-style XFDF with multiline attributes, random attribute order, self-closing tag, and inreplyto
        let sample_xfdf = r#"<?xml version="1.0" encoding="UTF-8"?>
<xfdf xmlns="http://ns.adobe.com/xfdf/" xml:space="preserve">
  <annotations>
    <highlight
        title="山田太郎"
        page="1"
        name="annot_100"
        left="50.5"
        top="100.2"
        width="200.0"
        height="30.0">
      <contents>重要箇所のハイライト</contents>
    </highlight>
    <text
        name="annot_200"
        inreplyto="annot_100"
        page="1"
        title="佐藤花子"
        left="60.0"
        top="110.0"
        width="50.0"
        height="50.0">
      <contents>了解しました、修正します。</contents>
    </text>
  </annotations>
</xfdf>"#;

        let imported_pdf = crate::pdf_engine::forms::import_xfdf(&base_pdf, sample_xfdf)
            .expect("Robust XML parser must parse Acrobat-style multiline XFDF");

        let doc = Document::load_mem(&imported_pdf).expect("Must load valid PDF");
        let page_ids = get_page_ids(&doc);
        assert!(!page_ids.is_empty());

        let page_dict = doc.objects.get(&page_ids[0]).unwrap().as_dict().unwrap();
        let annots_arr = page_dict.get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(annots_arr.len(), 2, "Both parent annot and reply must be in page Annots");

        // Find parent and reply
        let mut parent_id = None;
        let mut reply_annot = None;

        for a_ref in annots_arr {
            let id = a_ref.as_reference().unwrap();
            let annot_dict = doc.objects.get(&id).unwrap().as_dict().unwrap();
            if annot_dict.get(b"Subtype").unwrap().as_name().unwrap() == b"Highlight" {
                parent_id = Some(id);
                let contents = annot_dict.get(b"Contents").unwrap().as_str().unwrap();
                assert_eq!(crate::pdf_engine::common::decode_pdf_text_string(contents), "重要箇所のハイライト");
            } else if annot_dict.get(b"Subtype").unwrap().as_name().unwrap() == b"Text" {
                reply_annot = Some(annot_dict.clone());
            }
        }

        let pid = parent_id.expect("Must find parent highlight annotation");
        let reply = reply_annot.expect("Must find reply annotation");

        // Verify /IRT points to parent_id
        let irt = reply.get(b"IRT").expect("Reply must have /IRT entry").as_reference().unwrap();
        assert_eq!(irt, pid, "Reply's /IRT must point directly to parent's ObjectId!");

        // Now test export_xfdf roundtrip to ensure inreplyto="annot_..." is exported!
        let exported_xfdf = crate::pdf_engine::forms::export_xfdf(&imported_pdf)
            .expect("export_xfdf must succeed");
        assert!(exported_xfdf.contains(&format!("inreplyto=\"annot_{}\"", pid.0)), "Exported XFDF must preserve inreplyto attribute!");
    }

    #[test]
    fn test_delete_annotation_indirect_annots() {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));

        // Create an annotation object
        let mut a_dict = Dictionary::new();
        a_dict.set("Type", Object::Name("Annot".into()));
        a_dict.set("Subtype", Object::Name("Highlight".into()));
        let annot_id = doc.add_object(Object::Dictionary(a_dict));

        // Create an INDIRECT Annots array (typical of InDesign / Acrobat Pro)
        let indirect_annots_id = doc.add_object(Object::Array(vec![Object::Reference(annot_id)]));

        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name("Page".into()));
        page_dict.set("Parent", Object::Reference(pages_id));
        page_dict.set("Annots", Object::Reference(indirect_annots_id)); // Indirect reference!
        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Count", Object::Integer(1));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        if let Some(Object::Dictionary(ref mut p)) = doc.objects.get_mut(&pages_id) {
            *p = pages_dict;
        }

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut pdf_data = Vec::new();
        doc.save_to(&mut pdf_data).unwrap();

        // Call delete_annotation
        let modified_pdf = crate::pdf_engine::annot_manage::delete_annotation(&pdf_data, annot_id)
            .expect("delete_annotation must succeed on indirect Annots");

        let res_doc = Document::load_mem(&modified_pdf).expect("Must load valid PDF");
        // Annotation object itself must be deleted
        assert!(!res_doc.objects.contains_key(&annot_id), "Annotation object must be removed");

        // Indirect Annots array must be updated and empty
        let indir_annots = res_doc.objects.get(&indirect_annots_id).unwrap().as_array().unwrap();
        assert!(indir_annots.is_empty(), "Indirect Annots array must no longer contain deleted annot reference!");
    }

    #[test]
    fn test_scanned_images_homography_and_dpi_sanity() {
        use image::{Rgb, RgbImage};

        // Create a temporary test image with a white quadrilateral document on a dark background
        let w = 400u32;
        let h = 300u32;
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgb([20, 20, 20]));
            }
        }
        // Draw document shape (approx 40,30 to 360,270)
        for y in 40..260 {
            for x in 50..350 {
                img.put_pixel(x, y, Rgb([230, 230, 230]));
            }
        }

        let temp_dir = std::env::temp_dir();
        let test_path = temp_dir.join("test_scan_sample.png");
        let path_str = test_path.to_str().unwrap().to_string();
        img.save(&test_path).expect("save test image");

        let pdf_data = crate::image_engine::process_scanned_images(
            &[path_str],
            true,
            true,
            300,
        ).expect("process_scanned_images must succeed");

        let _ = std::fs::remove_file(&test_path);

        assert!(!pdf_data.is_empty());
        let doc = Document::load_mem(&pdf_data).expect("Scanned output must be valid PDF");
        assert_eq!(doc.get_pages().len(), 1, "Must generate exactly 1 page");
    }

    #[test]
    fn test_html_to_pdf_multi_engine_fallback() {
        let html_content = "<html><body><h1>Nagisa PDF Report</h1><p>Testing robust HTML to PDF generation.</p></body></html>";
        let out_path = std::env::temp_dir().join("test_html_output.pdf");
        let out_str = out_path.to_str().unwrap();

        let res = crate::pdf_engine::convert::html_to_pdf(html_content, out_str);
        assert!(res.is_ok(), "html_to_pdf must succeed via headless browser or pure-Rust fallback: {:?}", res.err());

        let pdf_bytes = std::fs::read(&out_path).expect("read generated pdf");
        let _ = std::fs::remove_file(&out_path);

        assert!(!pdf_bytes.is_empty());
        let doc = Document::load_mem(&pdf_bytes).expect("Output must be valid PDF");
        assert!(doc.get_pages().len() >= 1);
    }

    #[test]
    fn test_add_and_verify_doctimestamp() {
        let pdf = create_test_pdf(1);
        let stamped = crate::pdf_engine::security::add_timestamp(&pdf, "Nagisa DigiCert TSA")
            .expect("add_timestamp must succeed");

        let res = crate::pdf_engine::security::verify_timestamp(&stamped)
            .expect("verify_timestamp must succeed");
        assert!(res.valid, "DocTimeStamp must be recognized as valid");
        assert_eq!(res.authority, "Nagisa DigiCert TSA");
        assert!(res.timestamp.starts_with("D:"));
    }

    #[test]
    fn test_images_to_pdf_high_dpi_does_not_blow_up_page_size() {
        // Create a 2480x3508 high-resolution image (typical 300 DPI A4 scan)
        use image::{ImageBuffer, Rgb};
        let w = 2480u32;
        let h = 3508u32;
        let img = ImageBuffer::from_pixel(w, h, Rgb([240u8, 240u8, 240u8]));
        let tmp_img_path = std::env::temp_dir().join(format!("nagisa_hires_scan_{}.jpg", std::process::id()));
        img.save_with_format(&tmp_img_path, image::ImageFormat::Jpeg).expect("save hires jpeg");

        let out_pdf_path = std::env::temp_dir().join(format!("nagisa_hires_scan_{}.pdf", std::process::id()));
        let res = images_to_pdf(&[tmp_img_path.to_str().unwrap().to_string()], out_pdf_path.to_str().unwrap());
        assert!(res.is_ok(), "images_to_pdf must succeed");

        let pdf_bytes = std::fs::read(&out_pdf_path).expect("read output PDF");
        let _ = std::fs::remove_file(&tmp_img_path);
        let _ = std::fs::remove_file(&out_pdf_path);

        let doc = Document::load_mem(&pdf_bytes).expect("load output PDF");
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 1);

        let page_id = pages.values().next().unwrap();
        let page_obj = doc.objects.get(page_id).unwrap().as_dict().unwrap();
        let mediabox = page_obj.get(b"MediaBox").unwrap().as_array().unwrap();
        let pt_w = mediabox[2].as_float().unwrap();
        let pt_h = mediabox[3].as_float().unwrap();

        // Standard A4 is 595.28 x 841.89 pt. It must NOT be scaled up to 1860+ pt (96 DPI blowup)!
        assert!(
            pt_w <= 596.0 && pt_h <= 842.0,
            "High-res scan must fit within standard A4 bounds! Got: {} x {}",
            pt_w, pt_h
        );
    }

    #[test]
    fn test_flatten_transparency_neutralizes_extgstate_and_group() {
        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();

        // Add ExtGState with transparency: CA 0.5, ca 0.5, BM /Multiply, and SMask
        let mut gs_dict = Dictionary::new();
        gs_dict.set("Type", Object::Name(b"ExtGState".to_vec()));
        gs_dict.set("CA", Object::Real(0.5));
        gs_dict.set("ca", Object::Real(0.5));
        gs_dict.set("BM", Object::Name(b"Multiply".to_vec()));
        gs_dict.set("SMask", Object::Name(b"None".to_vec()));
        let gs_id = doc.add_object(Object::Dictionary(gs_dict));

        let mut res_dict = Dictionary::new();
        let mut extg_dict = Dictionary::new();
        extg_dict.set("GS1", Object::Reference(gs_id));
        res_dict.set("ExtGState", Object::Dictionary(extg_dict));
        let res_id = doc.add_object(Object::Dictionary(res_dict));

        // Create transparency group
        let mut grp_dict = Dictionary::new();
        grp_dict.set("Type", Object::Name(b"Group".to_vec()));
        grp_dict.set("S", Object::Name(b"Transparency".to_vec()));

        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name(b"Page".to_vec()));
        page_dict.set("Parent", Object::Reference(pages_id));
        page_dict.set("MediaBox", Object::Array(vec![Object::Integer(0), Object::Integer(0), Object::Integer(612), Object::Integer(792)]));
        page_dict.set("Resources", Object::Reference(res_id));
        page_dict.set("Group", Object::Dictionary(grp_dict));

        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages_dict.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let mut catalog_dict = Dictionary::new();
        catalog_dict.set("Type", Object::Name(b"Catalog".to_vec()));
        catalog_dict.set("Pages", Object::Reference(pages_id));
        let catalog_id = doc.add_object(Object::Dictionary(catalog_dict));
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let raw_pdf = save_doc(&mut doc).expect("save initial PDF");

        // Flatten transparency
        let flattened_pdf = flatten_transparency(&raw_pdf).expect("flatten_transparency must succeed");
        let flat_doc = Document::load_mem(&flattened_pdf).expect("load flattened PDF");

        // Verify ExtGState is now completely opaque and blend mode is Normal
        let updated_gs = flat_doc.objects.get(&gs_id).expect("gs_id must exist").as_dict().unwrap();
        assert_eq!(updated_gs.get(b"CA").unwrap().as_float().unwrap(), 1.0);
        assert_eq!(updated_gs.get(b"ca").unwrap().as_float().unwrap(), 1.0);
        assert_eq!(updated_gs.get(b"BM").unwrap().as_name().unwrap(), b"Normal");
        assert!(updated_gs.get(b"SMask").is_err(), "SMask must be removed");

        // Verify Page transparency group has been stripped
        let updated_page = flat_doc.objects.get(&page_id).expect("page_id must exist").as_dict().unwrap();
        assert!(updated_page.get(b"Group").is_err(), "Transparency group must be removed");
    }

    #[test]
    fn test_execute_action_wizard_multi_step() {
        let initial_pdf = create_test_pdf(2);
        let wizard_json = r#"{
            "name": "QuickSanitizeAndStamp",
            "steps": [
                {
                    "action_type": "remove_metadata",
                    "params": {}
                },
                {
                    "action_type": "add_page_numbers",
                    "params": {
                        "position": "bottom-center",
                        "font_size": 10.0,
                        "start_number": 1
                    }
                },
                {
                    "action_type": "optimize",
                    "params": {}
                }
            ]
        }"#;

        let result_pdf = execute_action_wizard(&initial_pdf, wizard_json).expect("wizard execution must succeed");
        assert!(!result_pdf.is_empty());
        let doc = Document::load_mem(&result_pdf).expect("result must be a valid PDF");
        assert_eq!(doc.get_pages().len(), 2);
    }

    #[test]
    fn test_change_text_color_protects_non_text_graphics_and_handles_arrays() {
        // Build a PDF with both text block (BT..ET) and vector graphic shape (re, f)
        // using an array of content streams for /Contents
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();

        // Stream 1: Background vector rectangle in blue (#0000FF = 0 0 1 rg)
        let stream1_content = "0 0 1 rg 50 50 200 100 re f".as_bytes().to_vec();
        let stream1_id = doc.add_object(Stream::new(Dictionary::new(), stream1_content));

        // Stream 2: Text block in black (#000000 = 0 0 0 rg)
        let stream2_content = "BT /Helvetica 12 Tf 0 0 0 rg (Hello World) Tj ET".as_bytes().to_vec();
        let stream2_id = doc.add_object(Stream::new(Dictionary::new(), stream2_content));

        // Page with /Contents as an Array [stream1_id, stream2_id]
        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name(b"Page".to_vec()));
        page_dict.set("Parent", Object::Reference(pages_id));
        page_dict.set("MediaBox", Object::Array(vec![Object::Integer(0), Object::Integer(0), Object::Integer(612), Object::Integer(792)]));
        page_dict.set("Contents", Object::Array(vec![Object::Reference(stream1_id), Object::Reference(stream2_id)]));
        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages_dict.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let mut catalog_dict = Dictionary::new();
        catalog_dict.set("Type", Object::Name(b"Catalog".to_vec()));
        catalog_dict.set("Pages", Object::Reference(pages_id));
        let catalog_id = doc.add_object(Object::Dictionary(catalog_dict));
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let initial_pdf = save_doc(&mut doc).expect("save doc");

        // Change text color to Red (#FF0000)
        let modified_pdf = crate::pdf_engine::font_style::change_text_color(
            &initial_pdf,
            0,
            "#000000",
            "#FF0000",
        ).expect("change_text_color must succeed on array contents");

        let mod_doc = Document::load_mem(&modified_pdf).expect("load modified PDF");

        // Check Stream 1: Background vector rectangle MUST STILL BE BLUE! Not turned into Red!
        let s1 = mod_doc.objects.get(&stream1_id).unwrap().as_stream().unwrap();
        let s1_decomp = s1.decompressed_content().unwrap_or_else(|_| s1.content.clone());
        let s1_str = String::from_utf8_lossy(&s1_decomp);
        assert!(s1_str.contains("0 0 1 rg"), "Vector graphic background must NOT be modified: {}", s1_str);

        // Check Stream 2: Text block MUST BE RED (#FF0000 -> 1 0 0 rg)
        let s2 = mod_doc.objects.get(&stream2_id).unwrap().as_stream().unwrap();
        let s2_decomp = s2.decompressed_content().unwrap_or_else(|_| s2.content.clone());
        let s2_str = String::from_utf8_lossy(&s2_decomp);
        assert!(s2_str.contains("1 0 0 rg"), "Text color must be updated to red: {}", s2_str);
    }

    #[test]
    fn test_interactive_form_fields_iso_compliance() {
        let base_pdf = create_test_pdf(1);

        // 1. Create Dropdown: must have /Ff with Combo flag (1 << 17 = 131072)
        let dropdown_pdf = form_creator::create_dropdown(
            &base_pdf,
            0,
            "Fruits",
            &["Apple".into(), "Banana".into(), "Cherry".into()],
            50.0,
            600.0,
            120.0,
            24.0,
        ).expect("create dropdown");

        let doc1 = Document::load_mem(&dropdown_pdf).expect("load dropdown pdf");
        // Find field
        let mut found_combo = false;
        for (_, obj) in doc1.objects.iter() {
            if let Object::Dictionary(dict) = obj {
                if let Ok(Object::String(name, _)) = dict.get(b"T") {
                    if name == b"Fruits" {
                        if let Ok(Object::Integer(ff)) = dict.get(b"Ff") {
                            assert_eq!(ff & (1 << 17), 1 << 17, "Combo box MUST have Combo bit (1<<17) set in Ff");
                            found_combo = true;
                        }
                    }
                }
            }
        }
        assert!(found_combo, "Dropdown field 'Fruits' with Combo flag must exist");

        // 2. Create Checkbox: must have /V and /AS Name objects and dual /AP /N states (/Yes and /Off)
        let checkbox_pdf = form_creator::create_checkbox(
            &base_pdf,
            0,
            "Agreement",
            50.0,
            550.0,
            true,
        ).expect("create checkbox");

        let doc2 = Document::load_mem(&checkbox_pdf).expect("load checkbox pdf");
        let mut found_cb = false;
        for (_, obj) in doc2.objects.iter() {
            if let Object::Dictionary(dict) = obj {
                if let Ok(Object::String(name, _)) = dict.get(b"T") {
                    if name == b"Agreement" {
                        assert_eq!(dict.get(b"V").unwrap(), &Object::Name(b"Yes".to_vec()));
                        assert_eq!(dict.get(b"AS").unwrap(), &Object::Name(b"Yes".to_vec()));
                        // Check AP /N contains both /Yes and /Off
                        let ap_dict = dict.get(b"AP").unwrap().as_dict().unwrap();
                        let n_dict = ap_dict.get(b"N").unwrap().as_dict().unwrap();
                        assert!(n_dict.get(b"Yes").is_ok(), "Normal appearance must have /Yes state");
                        assert!(n_dict.get(b"Off").is_ok(), "Normal appearance must have /Off state");
                        found_cb = true;
                    }
                }
            }
        }
        assert!(found_cb, "Checkbox field 'Agreement' must have compliant /V, /AS and dual /AP /N");

        // 3. Create Radio Button Group: must have single parent field with Radio flag & Kids widget annotations
        let radio_pdf = form_creator::create_radio_button(
            &base_pdf,
            0,
            "ShippingMethod",
            &["Standard".into(), "Express".into(), "Overnight".into()],
            50.0,
            500.0,
        ).expect("create radio button group");

        let doc3 = Document::load_mem(&radio_pdf).expect("load radio pdf");
        let mut found_parent = false;
        for (_, obj) in doc3.objects.iter() {
            if let Object::Dictionary(dict) = obj {
                if let Ok(Object::String(name, _)) = dict.get(b"T") {
                    if name == b"ShippingMethod" {
                        // Check Ff has Radio flag (1 << 15)
                        let ff = dict.get(b"Ff").unwrap().as_i64().unwrap();
                        assert_ne!(ff & (1 << 15), 0, "Radio group parent must have Radio bit set in Ff");
                        // Check Kids has 3 annotations
                        let kids = dict.get(b"Kids").unwrap().as_array().unwrap();
                        assert_eq!(kids.len(), 3, "Radio group must have 3 kid widgets");
                        found_parent = true;
                    }
                }
            }
        }
        assert!(found_parent, "Radio button parent field must exist with ISO 32000 compliant hierarchy");
    }

    #[test]
    fn test_add_text_multiline_support() {
        let base_pdf = create_test_pdf(1);
        let multiline_japanese = "一行目のテキスト\n二行目のテキスト\n三行目のテキスト";

        let modified_pdf = common::add_text(
            &base_pdf,
            0,
            multiline_japanese,
            50.0,
            700.0,
            16.0,
            "#000000",
        ).expect("add_text multiline must succeed");

        let doc = Document::load_mem(&modified_pdf).expect("load modified doc");
        let page_ids = get_page_ids(&doc);
        let page_dict = doc.get_dictionary(page_ids[0]).expect("page dict");
        let contents = page_dict.get(b"Contents").expect("contents");

        // The added content stream should contain multiple Tm and Tj operations
        let added_stream_id = match contents {
            Object::Array(arr) => arr.last().unwrap().as_reference().unwrap(),
            Object::Reference(r) => *r,
            _ => panic!("Expected array or reference contents"),
        };
        let stream = doc.objects.get(&added_stream_id).unwrap().as_stream().unwrap();
        let decomp = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
        let content = lopdf::content::Content::decode(&decomp).expect("decode content operations");

        let tj_count = content.operations.iter().filter(|op| op.operator == "Tj").count();
        let tm_count = content.operations.iter().filter(|op| op.operator == "Tm").count();
        assert_eq!(tj_count, 3, "Must render exactly 3 lines via Tj operators");
        assert_eq!(tm_count, 3, "Must position each line via Tm matrix operators");
    }

    #[test]
    fn test_add_calculated_field_iso32000_compliance() {
        let base_pdf = create_test_pdf(1);
        let updated = crate::pdf_engine::forms::add_calculated_field(
            &base_pdf,
            0,
            "TotalSum",
            "Quantity * Price",
            100.0,
            500.0,
            120.0,
            30.0,
        )
        .expect("add_calculated_field should succeed");

        let doc = Document::load_mem(&updated).expect("load updated doc");

        // 1. Verify AcroForm in Catalog
        let catalog = doc.catalog().expect("Catalog must exist");
        let acroform_ref = catalog.get(b"AcroForm").expect("AcroForm must be present in Catalog");
        let acroform = match acroform_ref {
            Object::Reference(r) => doc.objects.get(r).unwrap().as_dict().unwrap(),
            Object::Dictionary(d) => d,
            _ => panic!("Expected AcroForm dict or ref"),
        };
        let fields = acroform.get(b"Fields").expect("Fields array in AcroForm").as_array().unwrap();
        assert!(!fields.is_empty(), "AcroForm /Fields must not be empty");

        // 2. Find the TotalSum field dictionary
        let mut found_field = false;
        for field_ref in fields {
            let f_id = field_ref.as_reference().unwrap();
            let f_dict = doc.objects.get(&f_id).unwrap().as_dict().unwrap();
            if let Ok(Object::String(name, _)) = f_dict.get(b"T") {
                if name == b"TotalSum" {
                    found_field = true;

                    // 3. Verify Appearance Stream (/AP /N)
                    let ap = f_dict.get(b"AP").expect("/AP dict must be present").as_dict().unwrap();
                    assert!(ap.get(b"N").is_ok(), "/AP /N normal appearance stream must exist");

                    // 4. Verify ISO 32000-1 §12.6.3 Additional-actions dictionary (/AA)
                    let aa = f_dict.get(b"AA").expect("/AA dict must be present").as_dict().unwrap();
                    let c_action_ref = aa.get(b"C").expect("/AA must contain /C (Calculate) event dictionary");
                    let c_action = match c_action_ref {
                        Object::Reference(r) => doc.objects.get(r).unwrap().as_dict().unwrap(),
                        Object::Dictionary(d) => d,
                        _ => panic!("Expected /C action dict"),
                    };

                    let s = c_action.get(b"S").unwrap().as_name().unwrap();
                    assert_eq!(s, b"JavaScript", "Action /S must be /JavaScript");

                    let js_bytes = c_action.get(b"JS").unwrap().as_str().unwrap();
                    let js_str = String::from_utf8_lossy(js_bytes);
                    assert!(
                        js_str.contains("Quantity * Price") && js_str.contains("event.value"),
                        "Calculation JS script must set event.value: {}",
                        js_str
                    );
                }
            }
        }
        assert!(found_field, "TotalSum field must be registered in AcroForm Fields");
    }

    #[test]
    fn test_embed_javascript_iso32000_compliance() {
        let base_pdf = create_test_pdf(1);
        let js_code = "console.println('Nagisa Engine Init');";

        let embedded = crate::pdf_engine::convert::embed_javascript(&base_pdf, js_code)
            .expect("embed_javascript should succeed");

        let doc = Document::load_mem(&embedded).expect("load doc");
        let catalog = doc.catalog().expect("Catalog must exist");

        // Verify /Root /Names /JavaScript exists per ISO 32000-1 §12.6.4.4
        let names_ref = catalog.get(b"Names").expect("/Names dictionary must exist on Root");
        let names_dict = match names_ref {
            Object::Reference(r) => doc.objects.get(r).unwrap().as_dict().unwrap(),
            Object::Dictionary(d) => d,
            _ => panic!("Expected /Names dict"),
        };

        let js_names_ref = names_dict.get(b"JavaScript").expect("/Names /JavaScript must exist");
        let js_names_dict = match js_names_ref {
            Object::Reference(r) => doc.objects.get(r).unwrap().as_dict().unwrap(),
            Object::Dictionary(d) => d,
            _ => panic!("Expected /JavaScript dict"),
        };

        let names_arr = js_names_dict.get(b"Names").expect("/Names array must exist in JS name tree").as_array().unwrap();
        assert!(names_arr.len() >= 2, "Name tree must have key-value pairs");

        let action_ref = names_arr[1].as_reference().unwrap();
        let action_dict = doc.objects.get(&action_ref).unwrap().as_dict().unwrap();
        assert_eq!(action_dict.get(b"S").unwrap().as_name().unwrap(), b"JavaScript");
        assert_eq!(action_dict.get(b"JS").unwrap().as_str().unwrap(), js_code.as_bytes());
    }

    #[test]
    fn test_html_to_pdf_fallback_japanese_preservation() {
        let temp_dir = std::env::temp_dir().join(format!("nagisa_test_html_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let output_path = temp_dir.join("fallback_japanese.pdf");

        // Fallback plain-text/HTML rendering with Japanese content
        let html = "<html><body><h1>渚エンジン</h1><p>日本語の文書です。消滅しません。</p></body></html>";
        let res = crate::pdf_engine::convert::generate_pdf_from_plain_text(
            &crate::pdf_engine::convert::extract_text_from_html(html),
            output_path.to_str().unwrap(),
        );

        assert!(res.is_ok(), "Fallback PDF generation must succeed: {:?}", res);

        let pdf_bytes = std::fs::read(&output_path).expect("read generated fallback PDF");
        let doc = Document::load_mem(&pdf_bytes).expect("load fallback PDF");

        // Verify that font is NagisaCJK with TrueType/Type0 composite Unicode font
        let page_ids = get_page_ids(&doc);
        assert!(!page_ids.is_empty(), "Page must be created");

        let page_dict = doc.get_dictionary(page_ids[0]).expect("page dict");
        let res_ref = page_dict.get(b"Resources").expect("Resources");
        let res_dict = match res_ref {
            Object::Reference(r) => doc.objects.get(r).unwrap().as_dict().unwrap(),
            Object::Dictionary(d) => d,
            _ => panic!("Resources"),
        };
        let fonts = res_dict.get(b"Font").expect("Font dict").as_dict().unwrap();
        assert!(fonts.get(b"NagisaCJK").is_ok(), "NagisaCJK Unicode font must be embedded for Japanese fallback");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_add_page_numbers_graphics_state_isolation() {
        let base_pdf = create_test_pdf(1);
        let numbered = crate::pdf_engine::convert::add_page_numbers(&base_pdf, "bottom-center", 10.0, 1)
            .expect("add_page_numbers must succeed");

        let doc = Document::load_mem(&numbered).expect("load numbered PDF");
        let page_ids = get_page_ids(&doc);
        let page_dict = doc.get_dictionary(page_ids[0]).expect("page dict");

        // Verify ExtGState /NagisaGS exists in Resources
        let res_dict = crate::pdf_engine::common::resolve_page_resources(&doc, page_ids[0]);
        let ext_gstate = res_dict.get(b"ExtGState").expect("ExtGState must be present").as_dict().unwrap();
        assert!(ext_gstate.get(b"NagisaGS").is_ok(), "NagisaGS state must be registered");

        // Verify page content stream contains graphics reset
        let contents = page_dict.get(b"Contents").expect("contents");
        let added_stream_id = match contents {
            Object::Array(arr) => arr.last().unwrap().as_reference().unwrap(),
            Object::Reference(r) => *r,
            _ => panic!("Expected array or reference contents"),
        };
        let stream = doc.objects.get(&added_stream_id).unwrap().as_stream().unwrap();
        let decomp = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
        let content_str = String::from_utf8_lossy(&decomp);

        assert!(content_str.contains("/NagisaGS gs"), "Must activate isolated ExtGState");
        assert!(content_str.contains("0 0 0 rg"), "Must reset RGB fill color to black");
        assert!(content_str.contains("/NagisaHelv"), "Must apply Helvetica font");
    }

    #[test]
    fn test_bookmark_tree_japanese_utf16be_and_hierarchy() {
        let base_pdf = create_test_pdf(3);

        let bookmarks = serde_json::json!([
            {
                "title": "第1章 日本語タイトル",
                "page": 0,
                "children": [
                    {
                        "title": "第1節 詳細概要",
                        "page": 1,
                        "children": []
                    }
                ]
            },
            {
                "title": "第2章 結び",
                "page": 2
            }
        ]);

        let bookmarked = crate::pdf_engine::convert::add_bookmark_tree(
            &base_pdf,
            bookmarks.as_array().unwrap(),
        )
        .expect("add_bookmark_tree should succeed");

        let doc = Document::load_mem(&bookmarked).expect("load bookmarked doc");
        let catalog = doc.catalog().expect("catalog");
        let outlines_ref = catalog.get(b"Outlines").unwrap().as_reference().unwrap();
        let outlines = doc.get_dictionary(outlines_ref).unwrap();

        // Total count should be 3 (Chapter 1, Section 1, Chapter 2)
        assert_eq!(outlines.get(b"Count").unwrap().as_i64().unwrap(), 3);

        let first_id = outlines.get(b"First").unwrap().as_reference().unwrap();
        let ch1 = doc.get_dictionary(first_id).unwrap();

        // Check UTF-16BE BOM on Japanese title
        let title_bytes = match ch1.get(b"Title").unwrap() {
            Object::String(bytes, _) => bytes.clone(),
            _ => panic!("Expected String for Title"),
        };
        assert!(
            title_bytes.starts_with(&[0xFE, 0xFF]),
            "Japanese bookmark title must be encoded in UTF-16BE with BOM [0xFE, 0xFF]"
        );

        // Verify child node (First/Count)
        let ch1_count = ch1.get(b"Count").unwrap().as_i64().unwrap();
        assert_eq!(ch1_count, 1, "Chapter 1 has 1 child");
        let sub1_id = ch1.get(b"First").unwrap().as_reference().unwrap();
        let sub1 = doc.get_dictionary(sub1_id).unwrap();
        assert_eq!(sub1.get(b"Parent").unwrap().as_reference().unwrap(), first_id);

        let sub1_title_bytes = match sub1.get(b"Title").unwrap() {
            Object::String(bytes, _) => bytes.clone(),
            _ => panic!("Expected String for Title"),
        };
        assert!(
            sub1_title_bytes.starts_with(&[0xFE, 0xFF]),
            "Child bookmark title must also be encoded in UTF-16BE"
        );
    }

    #[test]
    fn test_annotation_status_uses_state_and_statemodel() {
        let base_pdf = create_test_pdf(1);

        let annot_bytes = crate::pdf_engine::annotations::add_sticky_note(
            &base_pdf,
            0,
            100.0,
            100.0,
            "レビュアーのコメント",
            "#FFCC00",
        )
        .expect("add sticky note");

        let annots = crate::pdf_engine::annot_manage::get_annotations(&annot_bytes).unwrap();
        assert_eq!(annots.len(), 1);
        let annot_id_str = annots[0]["id"].as_str().unwrap();

        // Parse (u32, u16)
        let parts: Vec<&str> = annot_id_str
            .trim_matches(|c| c == '(' || c == ')')
            .split(|c| c == ',' || c == '_')
            .collect();
        let oid = (parts[0].trim().parse::<u32>().unwrap(), parts[1].trim().parse::<u16>().unwrap());

        // Set status to "Accepted"
        let updated = crate::pdf_engine::annot_manage::set_annotation_status(&annot_bytes, oid, "Accepted").unwrap();
        let updated_doc = Document::load_mem(&updated).unwrap();
        let annot_dict = updated_doc.get_dictionary(oid).unwrap();

        // ISO 32000-1 §12.5.6.3 check
        let state = annot_dict.get(b"State").expect("/State must be set").as_str().unwrap();
        assert_eq!(state, b"Accepted", "/State must equal Accepted");

        let state_model = annot_dict.get(b"StateModel").expect("/StateModel must be set").as_str().unwrap();
        assert_eq!(state_model, b"Review", "/StateModel must equal Review");

        // get_annotations should read status as "Accepted"
        let fetched = crate::pdf_engine::annot_manage::get_annotations(&updated).unwrap();
        assert_eq!(fetched[0]["status"].as_str().unwrap(), "Accepted");
    }

    #[test]
    fn test_action_wizard_extended_steps() {
        let base_pdf = create_test_pdf(1);
        let wizard_json = serde_json::json!({
            "name": "FullWorkflow",
            "steps": [
                {
                    "action_type": "rotate_pages",
                    "params": {
                        "rotation": 90
                    }
                },
                {
                    "action_type": "sanitize_document",
                    "params": {}
                }
            ]
        }).to_string();

        let executed = crate::pdf_engine::convert::execute_action_wizard(&base_pdf, &wizard_json);
        assert!(executed.is_ok(), "Action wizard with extended actions must succeed: {:?}", executed);
    }

    #[test]
    fn test_compare_pdf_documents_multi_stream_and_tj() {
        // Construct PDF 1 with multi-stream Contents and TJ operator
        let mut doc1 = Document::with_version("1.7");
        let stream1 = Stream::new(Dictionary::new(), b"BT /F1 12 Tf 50 700 Td [(Hello) -10 (World)] TJ ET".to_vec());
        let s1_id = doc1.add_object(Object::Stream(stream1));

        let stream2 = Stream::new(Dictionary::new(), b"BT /F1 16 Tf 50 600 Td (Second Stream Content) Tj ET".to_vec());
        let s2_id = doc1.add_object(Object::Stream(stream2));

        let mut page1 = Dictionary::new();
        page1.set("Type", Object::Name("Page".into()));
        page1.set("Contents", Object::Array(vec![Object::Reference(s1_id), Object::Reference(s2_id)]));
        page1.set("MediaBox", Object::Array(vec![Object::Real(0.0), Object::Real(0.0), Object::Real(595.0), Object::Real(842.0)]));
        let p1_id = doc1.add_object(Object::Dictionary(page1));

        let mut pages1 = Dictionary::new();
        pages1.set("Type", Object::Name("Pages".into()));
        pages1.set("Kids", Object::Array(vec![Object::Reference(p1_id)]));
        pages1.set("Count", Object::Integer(1));
        let pages1_id = doc1.add_object(Object::Dictionary(pages1));

        let mut cat1 = Dictionary::new();
        cat1.set("Type", Object::Name("Catalog".into()));
        cat1.set("Pages", Object::Reference(pages1_id));
        let cat1_id = doc1.add_object(Object::Dictionary(cat1));
        doc1.trailer.set("Root", Object::Reference(cat1_id));

        let mut pdf1_data = Vec::new();
        doc1.save_to(&mut pdf1_data).unwrap();

        // Construct PDF 2 with revised text in the TJ operator
        let mut doc2 = Document::with_version("1.7");
        let stream1_rev = Stream::new(Dictionary::new(), b"BT /F1 12 Tf 50 700 Td [(Hello) -10 (Nagisa)] TJ ET".to_vec());
        let s1_rev_id = doc2.add_object(Object::Stream(stream1_rev));
        let s2_rev_id = doc2.add_object(Object::Stream(Stream::new(Dictionary::new(), b"BT /F1 16 Tf 50 600 Td (Second Stream Content) Tj ET".to_vec())));

        let mut page2 = Dictionary::new();
        page2.set("Type", Object::Name("Page".into()));
        page2.set("Contents", Object::Array(vec![Object::Reference(s1_rev_id), Object::Reference(s2_rev_id)]));
        page2.set("MediaBox", Object::Array(vec![Object::Real(0.0), Object::Real(0.0), Object::Real(595.0), Object::Real(842.0)]));
        let p2_id = doc2.add_object(Object::Dictionary(page2));

        let mut pages2 = Dictionary::new();
        pages2.set("Type", Object::Name("Pages".into()));
        pages2.set("Kids", Object::Array(vec![Object::Reference(p2_id)]));
        pages2.set("Count", Object::Integer(1));
        let pages2_id = doc2.add_object(Object::Dictionary(pages2));

        let mut cat2 = Dictionary::new();
        cat2.set("Type", Object::Name("Catalog".into()));
        cat2.set("Pages", Object::Reference(pages2_id));
        let cat2_id = doc2.add_object(Object::Dictionary(cat2));
        doc2.trailer.set("Root", Object::Reference(cat2_id));

        let mut pdf2_data = Vec::new();
        doc2.save_to(&mut pdf2_data).unwrap();

        // Compare documents
        let report = crate::pdf_engine::compare::compare_pdf_documents(&pdf1_data, &pdf2_data)
            .expect("compare must succeed");

        assert!(
            report.total_changes > 0,
            "Must detect difference between 'HelloWorld' and 'HelloNagisa' across multi-stream TJ arrays"
        );
    }

    #[test]
    fn test_inspect_bookmarks_and_form_fields_unicode() {
        let base_pdf = create_test_pdf(2);

        // 1. Add Japanese bookmark tree
        let bookmarks = serde_json::json!([
            {
                "title": "第1章 日本語概要",
                "page": 0,
                "children": [
                    {
                        "title": "第1節 詳細",
                        "page": 1,
                        "children": []
                    }
                ]
            }
        ]);
        let bookmarked_pdf = crate::pdf_engine::convert::add_bookmark_tree(&base_pdf, bookmarks.as_array().unwrap())
            .expect("add_bookmark_tree");

        // 2. Add Japanese Form Field
        let field_config = crate::pdf_engine::form_creator::FormFieldConfig {
            field_type: "Text".into(),
            name: "氏名_フィールド".into(),
            x: 50.0,
            y: 700.0,
            width: 200.0,
            height: 25.0,
            value: Some("山田 太郎".into()),
            options: None,
            required: false,
            read_only: false,
            max_length: None,
        };
        let form_pdf = crate::pdf_engine::form_creator::create_form_field(&bookmarked_pdf, 0, &field_config)
            .expect("create_form_field");

        // 3. Inspect doc with inspect functions
        let doc = Document::load_mem(&form_pdf).expect("load mem");
        let extracted_bookmarks = crate::pdf_engine::inspect::get_bookmarks_from_doc(&doc).expect("bookmarks");
        assert_eq!(extracted_bookmarks.len(), 2);
        assert_eq!(extracted_bookmarks[0]["title"].as_str().unwrap(), "第1章 日本語概要");
        assert_eq!(extracted_bookmarks[1]["title"].as_str().unwrap(), "第1節 詳細");

        let extracted_fields = crate::pdf_engine::inspect::get_form_fields_from_doc(&doc).expect("form fields");
        assert_eq!(extracted_fields.len(), 1);
        assert_eq!(extracted_fields[0]["name"].as_str().unwrap(), "氏名_フィールド");
        assert_eq!(extracted_fields[0]["value"].as_str().unwrap(), "山田 太郎");
    }

    #[test]
    fn test_watermark_removal_and_cmyk_flatedecode() {
        let base_pdf = create_test_pdf(1);

        // 1. Add watermark
        let watermarked = crate::pdf_engine::annotations::add_watermark(
            &base_pdf,
            "社外秘 CONFIDENTIAL",
            0.3,
            45.0,
            48.0,
            "#FF0000",
            true,
            &[],
        ).expect("add_watermark");

        let doc_wm = Document::load_mem(&watermarked).unwrap();
        let page_ids = get_page_ids(&doc_wm);
        let content_ids = resolve_page_content_stream_ids(&doc_wm, page_ids[0]);
        assert_eq!(content_ids.len(), 2, "Base content + watermark stream");

        // 2. Remove watermark
        let cleaned = crate::pdf_engine::annotations::remove_watermarks(&watermarked)
            .expect("remove_watermarks");

        let doc_cleaned = Document::load_mem(&cleaned).unwrap();
        let cleaned_cids = resolve_page_content_stream_ids(&doc_cleaned, page_ids[0]);
        assert_eq!(cleaned_cids.len(), 1, "Watermark stream must be completely removed");

        // Verify that GSWatermark is gone from page content streams
        if let Some(Object::Stream(s)) = doc_cleaned.objects.get(&cleaned_cids[0]) {
            let decomp = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
            let content_str = String::from_utf8_lossy(&decomp);
            assert!(!content_str.contains("GSWatermark"), "Cleaned stream must not contain watermark resources");
        }

        // 3. Test convert_to_cmyk FlateDecode compression
        // Create a test PDF with an RGB image
        let mut img_doc = Document::with_version("1.7");
        let rgb_raw: Vec<u8> = vec![128u8; 100 * 100 * 3]; // 100x100 RGB
        let mut img_dict = Dictionary::new();
        img_dict.set("Type", Object::Name("XObject".into()));
        img_dict.set("Subtype", Object::Name("Image".into()));
        img_dict.set("Width", Object::Integer(100));
        img_dict.set("Height", Object::Integer(100));
        img_dict.set("ColorSpace", Object::Name("DeviceRGB".into()));
        img_dict.set("BitsPerComponent", Object::Integer(8));
        let img_stream = Stream::new(img_dict, rgb_raw);
        let img_id = img_doc.add_object(Object::Stream(img_stream));

        let mut p_dict = Dictionary::new();
        p_dict.set("Type", Object::Name("Page".into()));
        let mut res = Dictionary::new();
        let mut xobj = Dictionary::new();
        xobj.set("Im0", Object::Reference(img_id));
        res.set("XObject", Object::Dictionary(xobj));
        p_dict.set("Resources", Object::Dictionary(res));
        let pid = img_doc.add_object(Object::Dictionary(p_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(pid)]));
        pages_dict.set("Count", Object::Integer(1));
        let pages_id = img_doc.add_object(Object::Dictionary(pages_dict));

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        let cat_id = img_doc.add_object(Object::Dictionary(cat));
        img_doc.trailer.set("Root", Object::Reference(cat_id));

        let mut initial_pdf = Vec::new();
        img_doc.save_to(&mut initial_pdf).unwrap();

        let cmyk_pdf = crate::pdf_engine::print_prod::convert_to_cmyk(&initial_pdf)
            .expect("convert_to_cmyk");
        let cmyk_doc = Document::load_mem(&cmyk_pdf).unwrap();
        let cmyk_stream = cmyk_doc.objects.get(&img_id).unwrap().as_stream().unwrap();

        assert_eq!(
            cmyk_stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"FlateDecode",
            "CMYK converted image stream must be compressed with FlateDecode!"
        );
    }
    #[test]
    fn test_annotation_reply_and_deletion_and_shapes() {
        use crate::pdf_engine::annot_manage::{add_annotation_reply, delete_annotation};
        use crate::pdf_engine::annotations::{add_line, add_rectangle};

        // 1. Create a minimal valid PDF with 1 page
        let mut doc = Document::with_version("1.7");
        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name("Page".into()));
        page_dict.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(612.0),
                Object::Real(792.0),
            ]),
        );

        // Add parent annotation
        let mut parent_annot = Dictionary::new();
        parent_annot.set("Type", Object::Name("Annot".into()));
        parent_annot.set("Subtype", Object::Name("Text".into()));
        parent_annot.set(
            "Rect",
            Object::Array(vec![
                Object::Real(100.0),
                Object::Real(100.0),
                Object::Real(120.0),
                Object::Real(120.0),
            ]),
        );
        let parent_id = doc.add_object(Object::Dictionary(parent_annot));

        page_dict.set(
            "Annots",
            Object::Array(vec![Object::Reference(parent_id)]),
        );
        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages_dict.set("Count", Object::Integer(1));
        let pages_id = doc.add_object(Object::Dictionary(pages_dict));

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut base_pdf = Vec::new();
        doc.save_to(&mut base_pdf).unwrap();

        // 2. Add reply to parent annotation
        let reply_pdf = add_annotation_reply(&base_pdf, parent_id, "Alice", "This is a reply")
            .expect("add_annotation_reply should succeed");
        let reply_doc = Document::load_mem(&reply_pdf).expect("Load reply PDF");

        // Verify reply annotation dictionary attributes
        let mut reply_id_opt = None;
        for (id, obj) in reply_doc.objects.iter() {
            if let Object::Dictionary(d) = obj {
                if let Ok(Object::Reference(irt)) = d.get(b"IRT") {
                    if *irt == parent_id {
                        reply_id_opt = Some(*id);
                        // Must have RT == /R
                        assert_eq!(d.get(b"RT").unwrap().as_name().unwrap(), b"R");
                        // Must have non-zero Rect
                        let r = d.get(b"Rect").unwrap().as_array().unwrap();
                        assert_eq!(r.len(), 4);
                        assert_ne!(r[2].as_float().unwrap(), 0.0);
                        // Must have F flags set
                        assert_eq!(d.get(b"F").unwrap().as_i64().unwrap(), 28);
                    }
                }
            }
        }
        let reply_id = reply_id_opt.expect("Reply annotation object must be present in doc");

        // Verify page Annots contains both parent and reply
        let page_annots = reply_doc
            .objects
            .get(&page_id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(page_annots.contains(&Object::Reference(parent_id)));
        assert!(page_annots.contains(&Object::Reference(reply_id)));

        // 3. Delete parent annotation -> must delete reply AND remove both from page Annots with 0 dangling references
        let deleted_pdf = delete_annotation(&reply_pdf, parent_id).expect("delete_annotation should succeed");
        let del_doc = Document::load_mem(&deleted_pdf).expect("Load deleted PDF");

        assert!(!del_doc.objects.contains_key(&parent_id), "Parent must be removed from doc");
        assert!(!del_doc.objects.contains_key(&reply_id), "Reply must be removed from doc");

        let remaining_annots = del_doc
            .objects
            .get(&page_id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(!remaining_annots.contains(&Object::Reference(parent_id)), "Parent reference must not dangle in Annots");
        assert!(!remaining_annots.contains(&Object::Reference(reply_id)), "Reply reference must not dangle in Annots");
        assert!(remaining_annots.is_empty());

        // 4. Test add_rectangle creates an actual /Subtype /Square annotation with /AP /N stream instead of modifying page contents
        let rect_pdf = add_rectangle(&base_pdf, 0, 50.0, 50.0, 100.0, 100.0, "#FF0000", "#00FF00", 2.0)
            .expect("add_rectangle should succeed");
        let rect_doc = Document::load_mem(&rect_pdf).expect("Load rect PDF");
        let page_annots_rect = rect_doc
            .objects
            .get(&page_id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap();
        // Page annots now has 2 annotations: initial parent + new square
        assert_eq!(page_annots_rect.len(), 2);
        let square_id = match page_annots_rect[1] {
            Object::Reference(id) => id,
            _ => panic!("Expected reference"),
        };
        let square_dict = rect_doc.objects.get(&square_id).unwrap().as_dict().unwrap();
        assert_eq!(square_dict.get(b"Subtype").unwrap().as_name().unwrap(), b"Square");
        assert!(square_dict.get(b"AP").is_ok(), "Must have /AP appearance dictionary");

        // 5. Test add_line creates an actual /Subtype /Line annotation with /AP /N stream
        let line_pdf = add_line(&base_pdf, 0, 10.0, 10.0, 200.0, 200.0, "#0000FF", 1.5)
            .expect("add_line should succeed");
        let line_doc = Document::load_mem(&line_pdf).expect("Load line PDF");
        let page_annots_line = line_doc
            .objects
            .get(&page_id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(page_annots_line.len(), 2);
        let line_id = match page_annots_line[1] {
            Object::Reference(id) => id,
            _ => panic!("Expected reference"),
        };
        let line_dict = line_doc.objects.get(&line_id).unwrap().as_dict().unwrap();
        assert_eq!(line_dict.get(b"Subtype").unwrap().as_name().unwrap(), b"Line");
        assert!(line_dict.get(b"AP").is_ok(), "Must have /AP appearance dictionary");
    }

    #[test]
    fn test_compress_pdf_quality_images_and_visual_diff() {
        use crate::pdf_engine::convert::compress_pdf_quality;

        // 1. Create a synthetic PDF containing a raw uncompressed RGB image
        let mut doc = Document::with_version("1.7");
        let w = 80u32;
        let h = 80u32;
        let mut rgb_raw = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                rgb_raw.push(((x * 3) % 256) as u8);
                rgb_raw.push(((y * 3) % 256) as u8);
                rgb_raw.push(128);
            }
        }

        let mut img_dict = Dictionary::new();
        img_dict.set("Type", Object::Name("XObject".into()));
        img_dict.set("Subtype", Object::Name("Image".into()));
        img_dict.set("Width", Object::Integer(w as i64));
        img_dict.set("Height", Object::Integer(h as i64));
        img_dict.set("ColorSpace", Object::Name("DeviceRGB".into()));
        img_dict.set("BitsPerComponent", Object::Integer(8));
        let img_stream = Stream::new(img_dict, rgb_raw);
        let img_id = doc.add_object(Object::Stream(img_stream));

        let mut p_dict = Dictionary::new();
        p_dict.set("Type", Object::Name("Page".into()));
        let mut res = Dictionary::new();
        let mut xobj = Dictionary::new();
        xobj.set("Im0", Object::Reference(img_id));
        res.set("XObject", Object::Dictionary(xobj));
        p_dict.set("Resources", Object::Dictionary(res));
        let pid = doc.add_object(Object::Dictionary(p_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(pid)]));
        pages_dict.set("Count", Object::Integer(1));
        let pages_id = doc.add_object(Object::Dictionary(pages_dict));

        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name("Catalog".into()));
        cat.set("Pages", Object::Reference(pages_id));
        let cat_id = doc.add_object(Object::Dictionary(cat));
        doc.trailer.set("Root", Object::Reference(cat_id));

        let mut initial_pdf = Vec::new();
        doc.save_to(&mut initial_pdf).unwrap();

        // Compress at quality 40 -> should re-encode image with DCTDecode (JPEG)
        let compressed_pdf = compress_pdf_quality(&initial_pdf, 40)
            .expect("compress_pdf_quality should succeed");

        let c_doc = Document::load_mem(&compressed_pdf).unwrap();
        let c_stream = c_doc.objects.get(&img_id).unwrap().as_stream().unwrap();
        let filter_name = c_stream.dict.get(b"Filter").unwrap().as_name().unwrap();
        assert_eq!(filter_name, b"DCTDecode", "Image must be re-encoded as DCTDecode JPEG!");

        // Output size should be significantly smaller than raw uncompressed 19.2KB
        assert!(c_stream.content.len() < (w * h * 3) as usize);
    }

    #[test]
    fn test_sticky_note_appearance_stream_generation() {
        let pdf = create_test_pdf(1);
        let updated = crate::pdf_engine::annotations::add_sticky_note(
            &pdf,
            0,
            100.0,
            200.0,
            "Important note for browser viewers",
            "#ffea00",
        ).expect("add_sticky_note should succeed");

        let doc = Document::load_mem(&updated).expect("Must load valid PDF");
        let page_ids = get_page_ids(&doc);
        let page_dict = doc.objects.get(&page_ids[0]).unwrap().as_dict().unwrap();
        let annots = page_dict.get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(annots.len(), 1);

        let annot_id = annots[0].as_reference().unwrap();
        let annot_dict = doc.objects.get(&annot_id).unwrap().as_dict().unwrap();

        // Must have Appearance dictionary /AP with normal appearance /N
        let ap_obj = annot_dict.get(b"AP").expect("Sticky note must contain /AP dictionary");
        let ap_dict = ap_obj.as_dict().expect("/AP must be dictionary");
        let n_ref = ap_dict.get(b"N").expect("/AP must have /N entry").as_reference().unwrap();

        // Must point to a Form XObject stream
        let stream = doc.objects.get(&n_ref).unwrap().as_stream().unwrap();
        assert_eq!(stream.dict.get(b"Type").unwrap().as_name().unwrap(), b"XObject");
        assert_eq!(stream.dict.get(b"Subtype").unwrap().as_name().unwrap(), b"Form");
        assert!(!stream.content.is_empty(), "Appearance stream content must not be empty");
    }

    #[test]
    fn test_verify_signature_index_filtering() {
        let pdf = create_test_pdf(1);
        let signed = add_digital_signature(
            &pdf,
            0,
            100.0,
            100.0,
            200.0,
            50.0,
            "Alice",
            "Approval",
            None,
        ).expect("Add signature");

        // Verify index 0 succeeds and returns selected signature
        let res0 = crate::pdf_engine::security::verify_signature(&signed, 0).expect("verify index 0");
        assert_eq!(res0["count"], 1);
        assert_eq!(res0["selected_index"], 0);
        assert_eq!(res0["signature"]["signer"], "Alice");

        // Verify out-of-bounds index returns an error
        let res_err = crate::pdf_engine::security::verify_signature(&signed, 5);
        assert!(res_err.is_err(), "Out of bounds signature index must return Err");
    }

    #[test]
    fn test_add_circle_annotation() {
        let pdf = create_test_pdf(1);
        let circled = crate::pdf_engine::annotations::add_circle(
            &pdf,
            0,
            50.0,
            100.0,
            120.0,
            80.0,
            "#FF0000",
            "#00FF00",
            2.0,
        ).expect("add_circle should succeed");

        let doc = Document::load_mem(&circled).expect("load doc");
        let page_id = get_page_ids(&doc)[0];
        let page = doc.objects.get(&page_id).unwrap().as_dict().unwrap();
        let annots = page.get(b"Annots").unwrap().as_array().unwrap();
        assert!(!annots.is_empty());

        let annot_ref = annots.last().unwrap().as_reference().unwrap();
        let annot_dict = doc.objects.get(&annot_ref).unwrap().as_dict().unwrap();
        assert_eq!(annot_dict.get(b"Subtype").unwrap().as_name().unwrap(), b"Circle");
        assert!(annot_dict.has(b"AP"), "Circle annotation must have appearance stream /AP");
    }

    #[test]
    fn test_cms_signature_round_trip_incremental() {
        let pdf = create_test_pdf(1);
        let request = crate::pdf_engine::cms_sign::CmsSignRequest {
            seed: crate::pdf_engine::cms_sign::SignatureFieldSeed {
                page_index: 0,
                rect: [50.0, 50.0, 250.0, 100.0],
                field_name: "Signature1".to_string(),
                signer_name: "Nagisa Test".to_string(),
                reason: "test".to_string(),
                location: "test".to_string(),
                contact_info: "test".to_string(),
            },
            private_key_pem: TEST_SIGNING_KEY_PEM.as_bytes().to_vec(),
            certificate_pem: TEST_SIGNING_CERT_PEM.as_bytes().to_vec(),
            chain_pem: Vec::new(),
            p12_der: None,
            p12_password: None,
            tsa_url: None,
        };
        let signed = crate::pdf_engine::cms_sign::sign_pdf_cms(&request, &pdf).expect("CMS signing must succeed");
        assert!(signed.len() > pdf.len());
        // Incremental updates keep the original %%EOF and append a new one.
        let eof_count = signed.windows(5).filter(|window| *window == b"%%EOF".as_slice()).count();
        assert!(eof_count >= 2, "signed PDF must contain the original and the incremental %%EOF, found {eof_count}");
        // Incremental updates preserve every original byte as a prefix.
        assert_eq!(&signed[..pdf.len()], &pdf[..]);
        let report = crate::pdf_engine::cms_sign::verify_pdf_cms(&signed, 0).expect("CMS verification must succeed");
        assert_eq!(report.signatures_found, 1);
        assert!(report.digest_matches, "ByteRange digest must match CMS signature");
        assert!(report.cms_signature_valid, "detached CMS token must verify");
        assert_eq!(report.digest_algorithm, "SHA-256");
        let mut tampered = signed.clone();
        let flip = pdf.len().saturating_sub(20);
        tampered[flip] ^= 0x01;
        let tampered_report = crate::pdf_engine::cms_sign::verify_pdf_cms(&tampered, 0).expect("tampered verification parses");
        assert!(!tampered_report.digest_matches, "tampering with signed bytes must fail digest");
        assert!(!tampered_report.cms_signature_valid, "tampering with signed bytes must fail CMS validity");
    }


    fn run_openssl_test(args: &[&str]) -> Result<(), String> {
        let bin = find_tool("openssl").ok_or_else(|| "opensslが見つかりません".to_string())?;
        let output = std::process::Command::new(&bin).args(args).output().map_err(|e| format!("openssl起動失敗: {e}"))?;
        if !output.status.success() {
            return Err(format!("openssl失敗: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }

    #[test]
    fn test_cms_chain_validation() {
        if find_tool("openssl").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("nagisa_chain_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (_ca_key, ca_crt, leaf_key, leaf_crt) = {
            let ca_key = dir.join("ca.key");
            let ca_crt = dir.join("ca.crt");
            let leaf_key = dir.join("leaf.key");
            let leaf_csr = dir.join("leaf.csr");
            let leaf_crt = dir.join("leaf.crt");
            run_openssl_test(&["req","-x509","-newkey","rsa:2048","-nodes","-subj","/CN=Nagisa Test CA","-keyout",ca_key.to_str().unwrap(),"-out",ca_crt.to_str().unwrap(),"-days","365","-sha256"]).unwrap();
            run_openssl_test(&["req","-newkey","rsa:2048","-nodes","-subj","/CN=Nagisa Leaf","-keyout",leaf_key.to_str().unwrap(),"-out",leaf_csr.to_str().unwrap(),"-sha256"]).unwrap();
            run_openssl_test(&["x509","-req","-in",leaf_csr.to_str().unwrap(),"-CA",ca_crt.to_str().unwrap(),"-CAkey",ca_key.to_str().unwrap(),"-CAcreateserial","-out",leaf_crt.to_str().unwrap(),"-days","365","-sha256"]).unwrap();
            (String::new(), std::fs::read_to_string(&ca_crt).unwrap(), std::fs::read_to_string(&leaf_key).unwrap(), std::fs::read_to_string(&leaf_crt).unwrap())
        };
        let pdf = create_test_pdf(1);
        let request = crate::pdf_engine::cms_sign::CmsSignRequest {
            seed: crate::pdf_engine::cms_sign::SignatureFieldSeed {
                page_index: 0,
                rect: [50.0, 50.0, 250.0, 100.0],
                field_name: "Signature1".to_string(),
                signer_name: "Nagisa Test".to_string(),
                reason: "test".to_string(),
                location: "test".to_string(),
                contact_info: "test".to_string(),
            },
            private_key_pem: leaf_key.into_bytes(),
            certificate_pem: leaf_crt.clone().into_bytes(),
            chain_pem: vec![ca_crt.clone().into_bytes()],
            p12_der: None,
            p12_password: None,
            tsa_url: None,
        };
        let signed = crate::pdf_engine::cms_sign::sign_pdf_cms(&request, &pdf).expect("chain signing must succeed");
        let trusted = crate::pdf_engine::cms_sign::verify_pdf_cms_with_trust(&signed, 0, Some(ca_crt.as_bytes())).expect("trusted verify");
        assert_eq!(trusted.chain_valid, Some(true), "trusted root must validate chain: {}", trusted.chain_details);
        let bogus_crt = dir.join("bogus.crt");
        run_openssl_test(&["req","-x509","-newkey","rsa:2048","-nodes","-subj","/CN=Bogus Root","-keyout",dir.join("bogus.key").to_str().unwrap(),"-out",bogus_crt.to_str().unwrap(),"-days","365","-sha256"]).unwrap();
        let bogus = std::fs::read_to_string(&bogus_crt).unwrap();
        let untrusted = crate::pdf_engine::cms_sign::verify_pdf_cms_with_trust(&signed, 0, Some(bogus.as_bytes())).expect("bogus verify");
        assert_eq!(untrusted.chain_valid, Some(false), "bogus root must fail chain validation");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cms_sign_with_p12() {
        if find_tool("openssl").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("nagisa_p12_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p12_path = dir.join("bundle.p12");
        run_openssl_test(&["pkcs12","-export","-inkey","src/pdf_engine/testdata/nagisa_signing_test.key","-in","src/pdf_engine/testdata/nagisa_signing_test.crt","-out",p12_path.to_str().unwrap(),"-passout","pass:secret"]).expect("p12 export");
        let p12_der = std::fs::read(&p12_path).expect("read p12");
        let pdf = create_test_pdf(1);
        let request = crate::pdf_engine::cms_sign::CmsSignRequest {
            seed: crate::pdf_engine::cms_sign::SignatureFieldSeed {
                page_index: 0,
                rect: [50.0, 50.0, 250.0, 100.0],
                field_name: "Signature1".to_string(),
                signer_name: "Nagisa Test".to_string(),
                reason: "test".to_string(),
                location: "test".to_string(),
                contact_info: "test".to_string(),
            },
            private_key_pem: Vec::new(),
            certificate_pem: Vec::new(),
            chain_pem: Vec::new(),
            p12_der: Some(p12_der),
            p12_password: Some("secret".to_string()),
            tsa_url: None,
        };
        let signed = crate::pdf_engine::cms_sign::sign_pdf_cms(&request, &pdf).expect("p12 signing must succeed");
        let report = crate::pdf_engine::cms_sign::verify_pdf_cms(&signed, 0).expect("p12 verify");
        assert!(report.digest_matches && report.cms_signature_valid, "p12 signature must verify");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_append_dss_update_structure() {
        let pdf = create_test_pdf(1);
        let request = crate::pdf_engine::cms_sign::CmsSignRequest {
            seed: crate::pdf_engine::cms_sign::SignatureFieldSeed {
                page_index: 0,
                rect: [50.0, 50.0, 250.0, 100.0],
                field_name: "Signature1".to_string(),
                signer_name: "Nagisa Test".to_string(),
                reason: "test".to_string(),
                location: "test".to_string(),
                contact_info: "test".to_string(),
            },
            private_key_pem: TEST_SIGNING_KEY_PEM.as_bytes().to_vec(),
            certificate_pem: TEST_SIGNING_CERT_PEM.as_bytes().to_vec(),
            chain_pem: Vec::new(),
            p12_der: None,
            p12_password: None,
            tsa_url: None,
        };
        let signed = crate::pdf_engine::cms_sign::sign_pdf_cms(&request, &pdf).expect("sign for DSS");
        let material = crate::pdf_engine::cms_sign::LtvMaterial {
            certificates_pem: vec![TEST_SIGNING_CERT_PEM.as_bytes().to_vec()],
            ocsps_der: vec![b"\x00\x00".to_vec()],
            crls_der: vec![b"\x00\x00".to_vec()],
        };
        let with_dss = crate::pdf_engine::cms_sign::append_dss_update(&signed, &material).expect("append DSS");
        let doc = Document::load_mem(&with_dss).expect("load dss doc");
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = doc.objects.get(&root).unwrap().as_dict().unwrap();
        let dss_ref = catalog.get(b"DSS").unwrap().as_reference().unwrap();
        let dss = doc.objects.get(&dss_ref).unwrap().as_dict().unwrap();
        assert_eq!(dss.get(b"Type").unwrap().as_name().unwrap(), b"DSS");
        assert_eq!(dss.get(b"Certs").unwrap().as_array().unwrap().len(), 1);
        let vri_ref = dss.get(b"VRI").unwrap().as_reference().unwrap();
        let vri = doc.objects.get(&vri_ref).unwrap().as_dict().unwrap();
        assert_eq!(vri.len(), 1, "VRI must contain one entry per certificate");
        let report = crate::pdf_engine::cms_sign::verify_pdf_cms(&with_dss, 0).expect("verify after DSS");
        assert!(report.digest_matches && report.cms_signature_valid, "signature must still verify after DSS append");
        assert_eq!(&with_dss[..signed.len()], &signed[..], "DSS must be an incremental update");
        assert!(report.has_verification_dss, "DSS must be visible to the verifier");
    }

    #[test]
    fn test_stamp_ltv_dss_offline_chain_only() {
        let pdf = create_test_pdf(1);
        let request = crate::pdf_engine::cms_sign::CmsSignRequest {
            seed: crate::pdf_engine::cms_sign::SignatureFieldSeed {
                page_index: 0,
                rect: [50.0, 50.0, 250.0, 100.0],
                field_name: "Signature1".to_string(),
                signer_name: "Nagisa Test".to_string(),
                reason: "test".to_string(),
                location: "test".to_string(),
                contact_info: "test".to_string(),
            },
            private_key_pem: TEST_SIGNING_KEY_PEM.as_bytes().to_vec(),
            certificate_pem: TEST_SIGNING_CERT_PEM.as_bytes().to_vec(),
            chain_pem: Vec::new(),
            p12_der: None,
            p12_password: None,
            tsa_url: None,
        };
        let signed = crate::pdf_engine::cms_sign::sign_pdf_cms(&request, &pdf).expect("sign for LTV");
        assert!(!crate::pdf_engine::cms_sign::has_verification_dss(&signed));
        let stamped = crate::pdf_engine::cms_sign::stamp_ltv_dss(&signed).expect("stamp LTV");
        assert!(stamped.certificates_embedded >= 1, "signer cert must be embedded");
        assert!(
            stamped.warnings.iter().any(|w| w.contains("失効情報")),
            "self-signed chain must warn about missing revocation info: {:?}",
            stamped.warnings
        );
        assert!(crate::pdf_engine::cms_sign::has_verification_dss(&stamped.data));
        let report = crate::pdf_engine::cms_sign::verify_pdf_cms(&stamped.data, 0).expect("verify after stamp");
        assert!(report.has_verification_dss);
        assert!(report.digest_matches && report.cms_signature_valid, "signature must survive DSS stamp");
        assert_eq!(&stamped.data[..signed.len()], &signed[..], "stamp must stay incremental");
    }

    #[test]
    fn test_cms_signature_no_timestamp_by_default() {
        let pdf = create_test_pdf(1);
        let request = crate::pdf_engine::cms_sign::CmsSignRequest {
            seed: crate::pdf_engine::cms_sign::SignatureFieldSeed {
                page_index: 0,
                rect: [50.0, 50.0, 250.0, 100.0],
                field_name: "Signature1".to_string(),
                signer_name: "Nagisa Test".to_string(),
                reason: "test".to_string(),
                location: "test".to_string(),
                contact_info: "test".to_string(),
            },
            private_key_pem: TEST_SIGNING_KEY_PEM.as_bytes().to_vec(),
            certificate_pem: TEST_SIGNING_CERT_PEM.as_bytes().to_vec(),
            chain_pem: Vec::new(),
            p12_der: None,
            p12_password: None,
            tsa_url: None,
        };
        let signed = crate::pdf_engine::cms_sign::sign_pdf_cms(&request, &pdf).expect("sign for timestamp test");
        let report = crate::pdf_engine::cms_sign::verify_pdf_cms(&signed, 0).expect("verify base");
        assert!(report.timestamp.is_none(), "no timestamp expected on base signature");
    }
    #[test]
    fn test_add_digital_signature_with_certificate() {
        let pdf = create_test_pdf(1);
        let mock_cert = b"-----BEGIN CERTIFICATE-----\nMOCK\n-----END CERTIFICATE-----";
        let signed = add_digital_signature(
            &pdf,
            0,
            50.0,
            50.0,
            200.0,
            50.0,
            "Bob",
            "Signed with Cert",
            Some(mock_cert),
        ).expect("add_digital_signature with cert");

        let doc = Document::load_mem(&signed).expect("load doc");
        // Verify /Cert entry is present in signature dictionary
        let mut found_cert = false;
        for (_, obj) in doc.objects.iter() {
            if let Object::Dictionary(dict) = obj {
                if let Ok(Object::Name(ft)) = dict.get(b"FT") {
                    if ft == b"Sig" {
                        if let Ok(v_ref) = dict.get(b"V").and_then(|o| o.as_reference()) {
                            if let Some(Object::Dictionary(sig_dict)) = doc.objects.get(&v_ref) {
                                if sig_dict.has(b"Cert") {
                                    found_cert = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(found_cert, "Signature dictionary must contain /Cert when certificate_data is provided");
    }

    #[test]
    fn test_plain_text_ascii_generates_valid_pdf() {
        let text = "Hello World!\nThis is standard ASCII text without CJK characters.";
        let tmp_path = std::env::temp_dir().join(format!("test_ascii_{}.pdf", std::process::id()));
        let path = tmp_path.to_str().unwrap();
        crate::pdf_engine::convert::generate_pdf_from_plain_text(text, path).expect("ASCII PDF generation");

        let data = std::fs::read(path).unwrap();
        let _ = std::fs::remove_file(path);
        let doc = Document::load_mem(&data).expect("Must load valid PDF");
        let page_id = get_page_ids(&doc)[0];
        let page = doc.objects.get(&page_id).unwrap().as_dict().unwrap();
        assert!(page.has(b"Resources"), "Page must contain /Resources");
    }

    #[test]
    fn test_import_xfdf_creates_appearance_stream() {
        let base_pdf = create_test_pdf(1);
        let xfdf = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n",
            "  <annotations>\n",
            "    <square page=\"1\" name=\"rect_1\" title=\"Tester\" color=\"0,255,0\" left=\"50\" top=\"50\" width=\"100\" height=\"80\">\n",
            "      <contents>Test Square</contents>\n",
            "    </square>\n",
            "    <circle page=\"1\" name=\"circ_1\" title=\"Tester\" color=\"#0000FF\" left=\"150\" top=\"50\" width=\"60\" height=\"60\">\n",
            "      <contents>Test Circle</contents>\n",
            "    </circle>\n",
            "  </annotations>\n",
            "</xfdf>"
        );

        let result = crate::pdf_engine::forms::import_xfdf(&base_pdf, xfdf).expect("import xfdf");
        let doc = Document::load_mem(&result).expect("load result PDF");
        let page_id = get_page_ids(&doc)[0];
        let page = doc.objects.get(&page_id).unwrap().as_dict().unwrap();
        let annots = page.get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(annots.len(), 2);

        for annot_ref in annots {
            let annot_id = annot_ref.as_reference().unwrap();
            let annot = doc.objects.get(&annot_id).unwrap().as_dict().unwrap();
            // Verify /AP /N Form XObject exists
            let ap = annot.get(b"AP").expect("Annot must have /AP dictionary").as_dict().unwrap();
            let n_ref = ap.get(b"N").expect("/AP must contain /N").as_reference().unwrap();
            let stream_obj = doc.objects.get(&n_ref).unwrap().as_stream().unwrap();
            assert_eq!(stream_obj.dict.get(b"Subtype").unwrap().as_name().unwrap(), b"Form");
        }
    }

    #[test]
    fn test_add_digital_signature_blocks_on_existing_signature() {
        let base_pdf = create_test_pdf(1);
        let mut doc = Document::load_mem(&base_pdf).unwrap();

        // Simulate a signed field with ByteRange
        let mut sig_val = Dictionary::new();
        sig_val.set("Type", Object::Name(b"Sig".to_vec()));
        sig_val.set("Filter", Object::Name(b"Adobe.PPKLite".to_vec()));
        sig_val.set("SubFilter", Object::Name(b"adbe.pkcs7.detached".to_vec()));
        sig_val.set(
            "ByteRange",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(100),
                Object::Integer(200),
                Object::Integer(500),
            ]),
        );
        let sig_val_id = doc.add_object(Object::Dictionary(sig_val));

        let mut sig_field = Dictionary::new();
        sig_field.set("FT", Object::Name(b"Sig".to_vec()));
        sig_field.set("T", Object::String(b"ExistingSig".to_vec(), lopdf::StringFormat::Literal));
        sig_field.set("V", Object::Reference(sig_val_id));
        doc.add_object(Object::Dictionary(sig_field));

        let signed_bytes = save_doc(&mut doc).unwrap();

        // Attempting to add a new signature on already cryptographically signed PDF without incremental update must return Err
        let err_result = crate::pdf_engine::security::add_digital_signature(
            &signed_bytes,
            0,
            50.0,
            50.0,
            120.0,
            40.0,
            "Second Signer",
            "Approval",
            None,
        );
        assert!(err_result.is_err(), "Must reject modifying signed PDF to prevent ByteRange invalidation");
        let err_msg = err_result.unwrap_err();
        assert!(err_msg.contains("already contains cryptographically signed fields"));
    }

    #[test]
    fn test_edit_text_color_injection_and_replacement() {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));

        let content = "BT /F1 12 Tf 50 750 Td (Hello World) Tj ET";
        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            Dictionary::new(),
            content.as_bytes().to_vec(),
        )));

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
        page_dict.set("Contents", Object::Reference(content_id));

        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages_dict.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let mut root_dict = Dictionary::new();
        root_dict.set("Type", Object::Name("Catalog".into()));
        root_dict.set("Pages", Object::Reference(pages_id));
        let root_id = doc.add_object(Object::Dictionary(root_dict));
        doc.trailer.set("Root", Object::Reference(root_id));

        let mut initial_pdf = Vec::new();
        doc.save_to(&mut initial_pdf).unwrap();

        // 1. In-place text replace with red color (#FF0000)
        let edited = crate::pdf_engine::text_edit::edit_text(
            &initial_pdf,
            0,
            "World",
            "Universe",
            "Helvetica",
            14.0,
            "#FF0000",
        )
        .expect("edit_text should succeed");

        let edited_doc = Document::load_mem(&edited).unwrap();
        // #49 テスト修正: save_to→load_mem 後は page_id が変わる場合があるため
        // ページリストの先頭から取得する
        let edited_page_ids = get_page_ids(&edited_doc);
        let edited_page_id = edited_page_ids[0];
        let content_bytes = edited_doc
            .get_page_content(edited_page_id)
            .expect("Page content must exist");
        let decoded_str = String::from_utf8_lossy(&content_bytes);

        assert!(
            decoded_str.contains("Universe"),
            "Replacement text 'Universe' must appear in stream"
        );
        assert!(
            decoded_str.contains("rg"),
            "Color operator 'rg' must be injected when color is specified"
        );
    }

    #[test]
    fn test_annot_reply_rejects_missing_parent() {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));

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
        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Object::Name("Pages".into()));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages_dict.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let mut root_dict = Dictionary::new();
        root_dict.set("Type", Object::Name("Catalog".into()));
        root_dict.set("Pages", Object::Reference(pages_id));
        let root_id = doc.add_object(Object::Dictionary(root_dict));
        doc.trailer.set("Root", Object::Reference(root_id));

        let mut initial_pdf = Vec::new();
        doc.save_to(&mut initial_pdf).unwrap();

        // Non-existent parent annotation ID (999, 0)
        let reply_res = crate::pdf_engine::annot_manage::add_annotation_reply(
            &initial_pdf,
            (999, 0),
            "Ghost Reply",
            "Tester",
        );
        assert!(
            reply_res.is_err(),
            "Must reject adding reply to non-existent parent annotation to prevent zombie objects"
        );
    }
    #[test]
    fn test_inspect_pdf_synthetic_doc() {
        let mut doc = Document::with_version("1.7");
        let pages = doc.add_object(lopdf::Dictionary::new());
        let page = doc.add_object({
            let mut dict = lopdf::Dictionary::new();
            dict.set("Type", Object::Name(b"Page".into()));
            dict.set("Parent", Object::Reference(pages));
            dict.set("MediaBox", Object::Array(vec![Object::Integer(0), Object::Integer(0), Object::Integer(612), Object::Integer(792)]));
            dict
        });
        let mut kids = lopdf::Dictionary::new();
        kids.set("Type", Object::Name(b"Pages".into()));
        kids.set("Count", Object::Integer(1));
        kids.set("Kids", Object::Array(vec![Object::Reference(page)]));
        doc.objects.insert(pages, Object::Dictionary(kids));
        let mut catalog = lopdf::Dictionary::new();
        catalog.set("Type", Object::Name(b"Catalog".into()));
        catalog.set("Pages", Object::Reference(pages));
        let catalog_id = doc.add_object(Object::Dictionary(catalog));
        doc.trailer.set("Root", Object::Reference(catalog_id));
        let bytes = crate::pdf_engine::common::save_doc(&mut doc).expect("save synthetic");
        let report = crate::pdf_engine::compatibility::inspect_pdf(&bytes).expect("inspect synthetic");
        assert!(report.parseable);
        assert_eq!(report.pdf_version, "1.7");
        assert_eq!(report.page_count, 1);
        assert!(!report.encrypted);
        assert_eq!(report.signed_signature_count, 0);
    }

    #[test]
    fn test_inspect_pdf_malformed_does_not_panic() {
        for bad in [b"".as_ref(), b"%PDF-1.7".as_ref(), b"%PDF-1.7\n%binary garbage not a pdf".as_ref(), b"not a pdf at all".as_ref()] {
            let result = crate::pdf_engine::compatibility::inspect_pdf(bad);
            assert!(result.is_err(), "malformed PDF must return Err, not panic: {:?}", std::str::from_utf8(bad));
        }
    }

    #[test]
    fn test_inspect_pdf_signed_doc_reports_signature() {
        let pdf = create_test_pdf(1);
        let request = crate::pdf_engine::cms_sign::CmsSignRequest {
            seed: crate::pdf_engine::cms_sign::SignatureFieldSeed {
                page_index: 0,
                rect: [50.0, 50.0, 250.0, 100.0],
                field_name: "Signature1".to_string(),
                signer_name: "Nagisa Test".to_string(),
                reason: "test".to_string(),
                location: "test".to_string(),
                contact_info: "test".to_string(),
            },
            private_key_pem: TEST_SIGNING_KEY_PEM.as_bytes().to_vec(),
            certificate_pem: TEST_SIGNING_CERT_PEM.as_bytes().to_vec(),
            chain_pem: Vec::new(),
            p12_der: None,
            p12_password: None,
            tsa_url: None,
        };
        let signed = crate::pdf_engine::cms_sign::sign_pdf_cms(&request, &pdf).expect("sign for inspect");
        let report = crate::pdf_engine::compatibility::inspect_pdf(&signed).expect("inspect signed");
        assert!(report.parseable);
        assert!(report.signed_signature_count >= 1, "signed doc must report >=1 signature field");
        let material = crate::pdf_engine::cms_sign::LtvMaterial {
            certificates_pem: vec![TEST_SIGNING_CERT_PEM.as_bytes().to_vec()],
            ocsps_der: Vec::new(),
            crls_der: Vec::new(),
        };
        let with_dss = crate::pdf_engine::cms_sign::append_dss_update(&signed, &material).expect("append DSS");
        let report2 = crate::pdf_engine::compatibility::inspect_pdf(&with_dss).expect("inspect dss");
        assert_eq!(report2.page_count, report.page_count, "DSS append must not change page count");
        assert_eq!(report2.signed_signature_count, report.signed_signature_count, "DSS append must not change signature count");
    }

    #[test]
    fn test_engine_health_release_readiness() {
        let health = crate::pdf_engine::inspect::engine_health();
        let version = health["version"].as_str().expect("version string");
        assert!(!version.is_empty(), "engine version must be reported");
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
        assert_eq!(health["features"]["cms_sign"], true);
        assert_eq!(health["features"]["compatibility"], true);
        assert_eq!(health["features"]["dss_ltv"], true);
        // openssl availability in the health matrix must match the test finder.
        let openssl_available = health["binaries"]["openssl"].as_bool().expect("openssl bool");
        assert_eq!(openssl_available, find_tool("openssl").is_some(), "openssl availability mismatch");
    }

    #[test]
    fn test_inspect_pdf_invalid_returns_err_not_panic() {
        // A truncated / corrupted PDF body must yield Err, not abort the process.
        for bad in [b"not a pdf".as_ref(), b"%PDF-1.7\n%\xff\xff\xfe".as_ref(), b"%PDF-1.4\n%%EOF only header".as_ref()] {
            let res = std::panic::catch_unwind(|| crate::pdf_engine::compatibility::inspect_pdf(bad));
            assert!(res.is_ok(), "inspect_pdf must not panic on malformed input");
            assert!(res.unwrap().is_err(), "malformed PDF must return Err");
        }
    }

    #[test]
    fn test_validate_pdfa_compliance_reports_violations_on_plain_pdf() {
        let pdf = create_test_pdf(1);
        let report = crate::pdf_engine::validate_pdfa_compliance(&pdf, "B")
            .expect("validate_pdfa_compliance must not error on a valid PDF");

        // A plain test PDF has none of the PDF/A apparatus.
        assert!(!report.is_compliant, "plain PDF must not be PDF/A compliant");
        assert_eq!(report.standard, "PDF/A-1B (ISO 19005-1)");
        assert!(
            report.violations.iter().any(|v| v.contains("GTS_PDFA1")),
            "must report the missing PDF/A OutputIntent, got: {:?}",
            report.violations
        );
        assert!(
            report.violations.iter().any(|v| v.contains("XMP metadata")),
            "must report missing XMP metadata, got: {:?}",
            report.violations
        );
        assert_eq!(report.details["has_output_intent"], false);
        assert_eq!(report.details["has_icc_profile"], false);
        assert_eq!(report.details["marked"], false);
    }

    #[test]
    fn test_validate_pdfa_compliance_passes_after_convert_to_pdfa() {
        // convert_to_pdfa only accepts documents whose fonts are already embedded
        // (ISO 19005-1 hard requirement). We therefore inject a minimal embedded
        // FontFile2 into the test PDF's font descriptor, then verify the full
        // round-trip: embedded -> convert_to_pdfa -> validate as compliant.
        let mut doc = Document::load_mem(&create_test_pdf(1)).expect("load test pdf");
        let font_id = doc
            .objects
            .iter()
            .find_map(|(id, obj)| match obj {
                Object::Dictionary(d) if d.get(b"Type").ok().and_then(|t| t.as_name().ok()) == Some(b"Font") => {
                    Some(*id)
                }
                _ => None,
            })
            .expect("test PDF must contain a font object");

        // Minimal valid TrueType stream (sfnt header) — the validator only checks
        // structural presence, matching how the engine treats embedding.
        let mut font_stream = lopdf::Stream::new(Dictionary::new(), vec![0u8; 16]);
        font_stream.dict.set("Length1", Object::Integer(16));
        let font_stream_id = doc.add_object(Object::Stream(font_stream));

        let mut descriptor = Dictionary::new();
        descriptor.set("Type", Object::Name(b"FontDescriptor".into()));
        descriptor.set("FontName", Object::Name(b"Helvetica".into()));
        descriptor.set("FontFile2", Object::Reference(font_stream_id));
        let descriptor_id = doc.add_object(Object::Dictionary(descriptor));

        if let Some(Object::Dictionary(font)) = doc.objects.get_mut(&font_id) {
            font.set("FontDescriptor", Object::Reference(descriptor_id));
        }

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("serialize embedded-font PDF");

        let archival =
            crate::pdf_engine::convert_to_pdfa(&buf).expect("convert_to_pdfa must succeed once fonts are embedded");
        let report = crate::pdf_engine::validate_pdfa_compliance(&archival, "B")
            .expect("validate_pdfa_compliance must parse the archival PDF");

        // OutputIntent + XMP + MarkInfo must all be present after conversion.
        assert!(
            report.is_compliant,
            "converted PDF must be PDF/A compliant, violations: {:?}",
            report.violations
        );
        assert_eq!(report.details["has_output_intent"], true);
        assert_eq!(report.details["has_icc_profile"], true);
        assert_eq!(report.details["has_xmp"], true);
        assert_eq!(report.details["marked"], true);
        assert!(report.passed_checks.iter().any(|c| c.contains("GTS_PDFA1")));
    }

    #[test]
    fn test_validate_pdfa_compliance_flags_non_embedded_fonts() {
        // create_test_pdf references /Helvetica without embedding it, which is the
        // single most common PDF/A violation.
        let pdf = create_test_pdf(1);
        let report = crate::pdf_engine::validate_pdfa_compliance(&pdf, "B").unwrap();
        assert!(
            report.violations.iter().any(|v| v.contains("not fully embedded")),
            "must flag the non-embedded Helvetica font, got: {:?}",
            report.violations
        );
        assert_eq!(report.details["has_output_intent"], false);
    }

    #[test]
    fn test_validate_pdfa_compliance_selects_conformance_level() {
        let pdf = create_test_pdf(1);
        let b = crate::pdf_engine::validate_pdfa_compliance(&pdf, "B").unwrap();
        let a = crate::pdf_engine::validate_pdfa_compliance(&pdf, "A").unwrap();
        assert_eq!(b.standard, "PDF/A-1B (ISO 19005-1)");
        assert_eq!(a.standard, "PDF/A-1A (ISO 19005-1)");
    }

    #[test]
    fn test_validate_pdfa_compliance_errors_on_malformed_pdf() {
        let res = std::panic::catch_unwind(|| {
            crate::pdf_engine::validate_pdfa_compliance(b"not a pdf at all", "B")
        });
        assert!(res.is_ok(), "validate_pdfa_compliance must not panic on malformed input");
        assert!(res.unwrap().is_err(), "malformed PDF must return Err");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_keychain_identity_listing_parses_real_keychain() {
        // Exercises the real Security framework; it must never panic and every
        // returned entry must carry a usable selector for `security cms -N`.
        let identities = crate::pdf_engine::cms_sign::list_keychain_identities()
            .expect("listing keychain identities must not error");
        for id in &identities {
            assert!(
                id.sha1_fingerprint.len() >= 40,
                "fingerprint should be a SHA-1 hex string, got {}",
                id.sha1_fingerprint
            );
            assert!(!id.common_name.is_empty());
            assert!(!id.nickname.is_empty());
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_sign_pdf_cms_with_keychain_round_trip() {
        let identities = crate::pdf_engine::cms_sign::list_keychain_identities()
            .expect("listing keychain identities must not error");
        let Some(identity) = identities.first() else {
            // No code-signing identity on this machine (CI): the parsing path is
            // still covered by the listing test above.
            return;
        };

        let pdf = create_test_pdf(1);
        let seed = crate::pdf_engine::cms_sign::SignatureFieldSeed {
            page_index: 0,
            rect: [50.0, 50.0, 250.0, 100.0],
            field_name: "KeychainSignature".to_string(),
            signer_name: "Nagisa Keychain Test".to_string(),
            reason: "keychain round trip".to_string(),
            location: String::new(),
            contact_info: String::new(),
        };

        let signed =
            crate::pdf_engine::cms_sign::sign_pdf_cms_with_keychain(&pdf, seed, identity, None)
                .expect("keychain signing must succeed for a valid identity");

        // Incremental update must preserve the original bytes as a prefix.
        assert_eq!(&signed[..pdf.len()], &pdf[..]);

        // The result must pass the same cryptographic verification used for the
        // file-based paths: ByteRange digest binding + detached CMS validation.
        let report = crate::pdf_engine::cms_sign::verify_pdf_cms(&signed, 0)
            .expect("verify_pdf_cms must parse the keychain-signed PDF");
        assert!(report.digest_matches, "ByteRange digest must match");
        assert!(report.cms_signature_valid, "detached CMS must verify");
        assert_eq!(report.digest_algorithm, "SHA-256");
    }
    /// Regression tests for the expanded ISO 19005-1 rule set. Each case injects
    /// exactly one prohibited construct so a failure points at a specific rule.
    #[cfg(test)]
    mod pdfa_rules {
        use super::*;
        use lopdf::Document;

        /// Load the base PDF, mutate it, serialise, and validate.
        fn validate(mutate: impl FnOnce(&mut Document)) -> crate::pdf_engine::PdfaValidationReport {
            let mut doc = Document::load_mem(&create_test_pdf(1)).expect("load base pdf");
            mutate(&mut doc);
            let mut buf = Vec::new();
            doc.save_to(&mut buf).expect("serialize mutated pdf");
            crate::pdf_engine::validate_pdfa_compliance(&buf, "B")
                .expect("validate_pdfa_compliance must parse the mutated pdf")
        }

        #[test]
        fn detects_encryption_as_violation() {
            let report = validate(|doc| {
                doc.trailer
                    .set("Encrypt", Object::Integer(42));
            });
            assert!(!report.is_compliant);
            assert_eq!(report.details["encrypted"], true);
            assert!(
                report.violations.iter().any(|v| v.contains("encrypted")),
                "must flag encryption, got {:?}",
                report.violations
            );
        }

        #[test]
        fn detects_javascript_as_violation() {
            let report = validate(|doc| {
                let mut action = Dictionary::new();
                action.set("S", Object::Name(b"JavaScript".into()));
                action.set("JS", Object::String(b"app.alert(1)".to_vec(), lopdf::StringFormat::Literal));
                let id = doc.add_object(Object::Dictionary(action));
                doc.get_object_mut(id).unwrap();
            });
            assert_eq!(report.details["has_javascript"], true);
            assert!(report.violations.iter().any(|v| v.contains("JavaScript")));
        }

        #[test]
        fn detects_lzw_compression_as_violation() {
            let report = validate(|doc| {
                let mut stream_dict = Dictionary::new();
                stream_dict.set("Filter", Object::Name(b"LZWDecode".into()));
                let id = doc.add_object(Object::Stream(lopdf::Stream::new(stream_dict, vec![0u8; 8])));
                doc.get_object_mut(id).unwrap();
            });
            assert_eq!(report.details["has_lzw"], true);
            assert!(report.violations.iter().any(|v| v.contains("LZW")));
        }

        #[test]
        fn detects_transparency_as_violation() {
            let report = validate(|doc| {
                // /Group belongs on a page (or catalog), not a floating object,
                // so attach it to a real page dictionary to mirror a genuine
                // transparency group.
                let page_id = *crate::pdf_engine::get_page_ids(doc).first().expect("page");
                if let Some(Object::Dictionary(page)) = doc.objects.get_mut(&page_id) {
                    let mut group = Dictionary::new();
                    group.set("S", Object::Name(b"Transparency".into()));
                    group.set("CS", Object::Name(b"DeviceRGB".into()));
                    page.set("Group", Object::Dictionary(group));
                }
            });
            assert_eq!(report.details["has_transparency"], true);
            assert!(report.violations.iter().any(|v| v.contains("transparency")));
        }

        #[test]
        fn detects_postscript_xobject_as_violation() {
            let report = validate(|doc| {
                let mut xobject = Dictionary::new();
                xobject.set("Subtype", Object::Name(b"PS".into()));
                xobject.set("Type", Object::Name(b"XObject".into()));
                let id = doc.add_object(Object::Dictionary(xobject));
                doc.get_object_mut(id).unwrap();
            });
            assert_eq!(report.details["has_postscript_xobject"], true);
            assert!(report.violations.iter().any(|v| v.contains("PostScript")));
        }

        #[test]
        fn missing_marked_is_fine_for_level_b_but_violation_for_level_a() {
            // Level B does not mandate a tagged document, so this must NOT be a
            // violation for PDF/A-1b (it previously was a false positive).
            let mut doc = Document::load_mem(&create_test_pdf(1)).expect("load");
            let mut buf = Vec::new();
            doc.save_to(&mut buf).expect("serialize");
            let level_b = crate::pdf_engine::validate_pdfa_compliance(&buf, "B").unwrap();
            assert!(
                !level_b.violations.iter().any(|v| v.contains("MarkInfo")),
                "Marked must not be a level-B violation, got {:?}",
                level_b.violations
            );
            let level_a = crate::pdf_engine::validate_pdfa_compliance(&buf, "A").unwrap();
            assert!(
                level_a.violations.iter().any(|v| v.contains("MarkInfo")),
                "Marked must be a level-A violation when absent"
            );
        }

        #[test]
        fn reports_rule_count_and_trailer_id_state() {
            let report = validate(|_| {});
            assert_eq!(report.details["checked_rule_count"], 15);
            // The synthetic base PDF has no /ID, which the validator must say.
            assert_eq!(report.details["has_trailer_id"], false);
            assert!(report.violations.iter().any(|v| v.contains("/ID")));
        }

        #[test]
        fn clean_document_reports_many_passing_checks() {
            let report = validate(|_| {});
            // A plain PDF fails several rules, but the passing side must still be
            // populated (encryption, JS, XFA, LZW, transparency, OPI, ...).
            assert!(
                report.passed_checks.len() >= 8,
                "expected many passing checks, got {:?}",
                report.passed_checks
            );
        }
    }

}
