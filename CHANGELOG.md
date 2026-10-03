# Changelog

All notable changes to Nagisa PDF will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added
- Structured IPC errors: all `commands_*` (151 cmds), `pdf_engine`, `ocr_engine`, `session` now return `NagisaError` (`{ type, details }`); frontend already handles both shapes, no UI breakage
- `NagisaError::SessionNotFound`, `From<lopdf::Error>`/`From<PoisonError>`, `message()` accessor, `load_pdf()`/`page_range_err()` helpers
- `NagisaError` serialization/classification unit tests; structured-error frontend tests

### Fixed
- `deep_redact` now advances text position across consecutive `Tj`/`TJ` ops (font-size/`Tc`/`Tw`/`Tz` aware); previously co-located strings were misjudged
- `redact_area` no longer wipes pages with Flate-compressed content streams (decode after decompress)
- `redact_area` Tauri command removed from IPC (visual-only cover is a misuse footgun; `deep_redact` remains the supported path)
- `edit_text` TJ path preserves unmatched segments byte-identically (was lossy re-encode corrupting WinAnsi bytes)
- `move_text_block` preserves rotation/scale (`Tm` a/b/c/d) and rewrites only the first positioning op
- Text-block widths and CJK metrics use real font data (`/Widths`, CID `/DW`, embedded IPAex advances) instead of fixed coefficients
- `deep_redact_scanned_all` is fail-closed on per-page render/OCR failure (was silent partial success)
- Outline/structure/field-tree traversals guard against cyclic references (hang/stack-overflow DoS)
- `verifySignatures` propagates backend failure instead of reporting "no signatures"
- `renderPageToUrl` no longer revokes the returned blob URL (revoke-then-return broke page preview)
- `images_to_pdf` composites transparency on white and rejects empty input
- `install.sh` uses atomic temp download + `curl -fSL` (stale-file false success eliminated)
- Dormant `test_redact_area_preserves_other_content` re-activated (was missing `#[test]`)
- Dead `Sidebar.tsx` removed; `tsconfig.node.json` fixed; Cargo unix/windows dep duplication unified
- PDF.js LRU cache key upgraded to FNV-1a 64-bit (32-bit risked silent wrong-document hits)
- `deep_redact` hit test is run-overlap based (text under an opaque box can no longer survive recoverably)
- `useHistory` StrictMode-safe (ref mirror, no updater side effects) with injectable byte cap
- Rotation annotations use axis-aligned boxes (pdf-lib-verified); round-trip tests included
- Rotated pages: backend `page_viewport` (visual dims + `/Rotate`, inheritance-aware),
  frontend `utils/rotation` converters, viewer-localized visual/unrotated mapping
- Thumbnails unified via `usePageThumbnails` (lazy IntersectionObserver, no 40-cap, shared lifecycle)
- External-process wait reworked (handle kill, pipe draining, no PID race, no libc)
- Print-asset guard unified (`image_safe_for_lossy_recompress`); alpha white-matte shared
- Native-mode frontend history duplication removed (backend owns undo)
- `validate_safe_path` shared across all path-taking commands (embed_font, visual_diff, epub, OCR inputs, scans)
- Unlock flow pre-checks encryption state instead of promising impossible unlocks
- `domToPdf` keeps float precision (no per-drag quantization drift)
- Wrong-document guards: thumbnails require explicit source, latest-bytes cleared on doc open
- Session order locks fail loud instead of silent FIFO degrade
- `session_exec`: all in-place editor ops run inside the Rust session (args-only IPC, no `number[]` blowup); global 2GB history budget across sessions; `.bak` on file overwrite; unique print spool files with delayed cleanup
- `update_pdf` dead-command path removed (was false-success after signing); Security panel syncs via session with loud errors
- browser-session preview no longer hits session IPC (`getCurrentBytes`, metadata/bookmarks/search/verify/print fall back to preview bytes)
- Thumbnail sidebar shares `usePageThumbnails` with docId plumbing (no per-page full-document IPC)
- Error toasts show basenames only (no absolute-path leaks)
- IPA font bundling disclosed (`THIRD-PARTY-NOTICES.md`); offline scope clarified in READMEs
- Unsaved-changes guard (`beforeunload`) covering byte edits and overlay annotations
- Password-encrypted sessions refuse further mutation (`PasswordRequired`, prevents ciphered-stream corruption); dispatch table/match divergence test

### Security
- Batch/convert commands validate all input/output paths (`validate_safe_path`)
- External commands (`tesseract` stdin path, `openssl`, fingerprint) unified under timeout + SIGKILL
- `validate_pdfx`/`preflight` inspect decompressed streams (were blind to compressed content)

## [1.1.0] - 2026-09-28

### Added
- Real PAdES-LTV: `/DSS`+`/VRI` stamping (CRL/OCSP fetch), B-B/B-T/LTV levels
- Real RFC 3161 document timestamps + cryptographic verification + Security panel UI
- AES-256 V=5/R=6 password encryption (R6 hash, qpdf interop verified both directions)
- Encrypted-PDF open UX: password dialog, decrypt service, save-plaintext warning
- Pixel-level raster redaction (CTM-accurate, lossless Flate, SMask/Mask aware)
- OCR-powered true redaction for scanned/image PDFs (all pages, round-trip proof)
- Signature placement controls (shared signing + field-frame, clamped, aria-label)
- XFDF `<fields>` export for AcroForm values (in addition to `<annotations>`)

### Fixed
- `export_xfdf` now includes form field values (was annotations-only)
- Strict CMS parse + BER-tolerant SignerInfo fallback; hyphen-joined hex fix
- Signature self-verify targets new signature (pre-existing count)
- Redaction refuses signed PDFs (Acrobat parity); AcroForm field eradication
- Honest OCR zero-confidence (amber warning + EPUB toast)
- `cargo fmt` + `cargo clippy -D warnings` green in CI
- Duplicate Tauri command registrations removed (`sign_pdf_cms`, `validate_pdfx_compliance`)

### Platform Support
- macOS (aarch64 + x86_64 .dmg), Windows (x86_64 .exe/.msi), Linux (x86_64 .AppImage/.deb), Android (.apk)

## [1.0.0] - 2024-01-01

### Added

#### Core PDF Operations
- Merge multiple PDFs
- Split PDF by pages
- Delete, rotate, reorder pages
- Duplicate pages
- Extract pages
- Crop pages

#### Text Editing
- Direct text block editing
- Text search and replace
- Font detection and replacement
- Text color and size changes
- Paragraph reflow

#### Annotations
- Highlight, underline, strikethrough
- Sticky notes
- Shapes (rectangle, circle, line)
- Stamps
- Annotation replies and status
- XFDF import/export

#### Forms
- Create text fields, checkboxes, radio buttons
- Dropdown/combo boxes
- Signature fields
- Calculated fields
- Form data aggregation

#### Security
- Password protection
- Digital signatures (PKCS#7)
- Hardware token support (PKCS#11)
- Complete data redaction

#### Conversion
- PDF to images (JPG/PNG)
- Images to PDF
- HTML to PDF
- PDF to text/CSV
- PDF/A conversion

#### OCR
- High-accuracy text recognition (Tesseract LSTM)
- Searchable PDF creation
- EPUB conversion
- Layout-preserving OCR

#### Color Management
- RGB to CMYK conversion
- CMYK to RGB conversion
- ICC profile embedding
- Ink coverage checking

#### Print Production
- Preflight checks
- Font embedding verification
- PDF/X conversion
- Color separation preview

#### Advanced Features
- PDF portfolio creation
- Action wizard (record/replay)
- Transparency flattening
- Accessibility checking
- JavaScript embedding
- Digital ID management
- Timestamp integration
- Certificate store integration

### Platform Support
- macOS (Intel and Apple Silicon)
- Windows (x86_64)
- Linux (x86_64)

## [Unreleased]

### Planned
- 3D PDF support
- Multimedia embedding
- Advanced form design GUI
- Cloud sync integration
- Multi-language OCR
