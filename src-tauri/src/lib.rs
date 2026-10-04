// Clippy policy: public Tauri command fns intentionally take many args;
// complex PDF code intentionally uses manual patterns for auditability.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::type_complexity)]
#![allow(clippy::vec_init_then_push)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::unnecessary_get_then_check)]
#![allow(clippy::format_in_format_args)]
#![allow(clippy::manual_checked_ops)]
#![allow(clippy::module_inception)]

pub mod commands_advanced;
pub mod commands_annotation;
pub mod commands_core;
pub mod commands_io;
pub mod commands_ocr;
pub mod commands_prod;
pub mod commands_session;
pub mod commands_text;
pub mod error;
pub mod image_engine;
pub mod ocr_engine;
pub mod pdf_engine;
pub mod session;

pub use commands_advanced::*;
pub use commands_annotation::*;
pub use commands_core::*;
pub use commands_io::*;
pub use commands_ocr::*;
pub use commands_prod::*;
pub use commands_session::*;
pub use commands_text::*;
pub use error::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            // Core Page & Document operations
            merge_pdfs,
            delete_page,
            rotate_page,
            reorder_pages,
            extract_pages,
            duplicate_page,
            create_blank_pdf,
            crop_page,
            get_page_count,
            get_page_dimensions,
            render_page_to_png,
            get_page_text,
            search_text,
            get_bookmarks,
            add_bookmark_to_pdf,
            get_form_fields,
            set_form_field,
            flatten_form,
            add_stamp,
            print_pdf,
            get_pdf_metadata,
            optimize_pdf,
            protect_pdf,
            compare_pdfs,
            convert_to_pdfa,
            // Text & Font Editing
            add_text,
            add_image_to_page,
            get_text_blocks,
            edit_text_block,
            move_text_block,
            delete_text_block,
            get_fonts,
            replace_font,
            change_text_color,
            change_font_size,
            edit_text,
            get_text_positions,
            reflow_text,
            // Annotations & Markup
            add_highlight,
            add_underline,
            add_sticky_note,
            add_rectangle,
            add_circle,
            add_line,
            add_watermark,
            remove_watermarks,
            get_annotations,
            add_annotation_reply,
            set_annotation_status,
            delete_annotation,
            // File I/O, Batch & Conversion
            read_file_bytes,
            get_pdf_file_info,
            write_file_bytes,
            write_text_file,
            batch_merge_pdfs,
            batch_add_watermark,
            batch_protect,
            batch_optimize,
            pdf_to_images,
            images_to_pdf,
            html_to_pdf,
            pdf_to_word,
            pdf_to_excel,
            pdf_to_powerpoint,
            create_pdf_portfolio,
            add_header_footer,
            add_bookmark,
            add_bates_number,
            // OCR, Scanner & Redaction
            process_scanned_images,
            ocr_files,
            ocr_image_blocks,
            create_epub,
            create_searchable_pdf,
            // 注意: 視覚専用の redact_area（黒四角の上描き・下層データ残存）は
            // 誤用防止のため IPC 公開しない。破壊的フローは deep_redact 系を使用。
            redact_text,
            deep_redact,
            deep_redact_scanned_pdf,
            redact_text_deep,
            sanitize_document,
            convert_fonts_to_outlines,
            // Production, Color & Standards
            convert_to_pdfx,
            convert_to_pdfx_standard,
            validate_pdfx_compliance,
            validate_pdfa_compliance,
            check_accessibility,
            run_preflight,
            check_ink_coverage,
            fix_accessibility_issues,
            preview_color_separations,
            render_color_separation,
            convert_to_cmyk,
            embed_icc_profile,
            rgb_to_cmyk,
            cmyk_to_rgb,
            flatten_transparency,
            flatten_content,
            downsample_images,
            remove_metadata,
            repair_corrupt_pdf,
            enhance_scanned_pdf,
            compare_pdf_documents,
            // Advanced Signature, Form, & Wizards
            add_digital_signature,
            verify_signature,
            sign_pdf_cms,
            verify_pdf_cms,
            stamp_pdf_ltv,
            add_document_timestamp,
            verify_document_timestamp,
            decrypt_pdf,
            is_pdf_encrypted,
            inspect_compatibility,
            get_engine_health,
            list_keychain_identities,
            sign_pdf_with_keychain,
            list_pkcs11_slots,
            sign_pdf_with_pkcs11,
            embed_font,
            add_form_field,
            add_calculated_field,
            export_xfdf,
            import_xfdf,
            repair_pdf,
            unlock_pdf,
            compress_pdf_quality,
            add_page_numbers,
            create_action_wizard,
            execute_action_wizard,
            aggregate_form_data,
            embed_javascript,
            add_bookmark_tree,
            visual_diff,
            list_digital_ids,
            // Session Management
            session_open_pdf,
            session_close,
            session_get_bytes,
            session_rotate_page,
            session_delete_page,
            session_undo,
            session_redo,
            session_update_bytes,
            session_exec,
            session_get_history_status,
            session_get_page_count,
            session_get_page_dimensions,
            session_get_text_blocks,
            session_get_metadata,
            session_get_bookmarks,
            session_get_form_fields,
            session_search_text,
            session_render_page_to_png,
            session_render_color_separation,
            session_verify_signature,
            session_print_pdf,
        ])
        .manage(session::SessionManager::new())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
