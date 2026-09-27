# Changelog

All notable changes to Nagisa PDF will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

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
