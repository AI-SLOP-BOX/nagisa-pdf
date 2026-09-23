---
trigger: always_on
description: Critical UI/UX protection, layout freezing, and feature preservation rules
---

# UI/UX Protection Rules (CRITICAL)

To prevent regression, layout breaking, or unintended UI re-architecting:

## 1. Strict UI Freeze (UI凍結ルール)
- **NO Unprompted Redesigns**: Never redesign, reorganize, or rewrite the DOM structure, toolbar layout, or component architecture of:
  - `src/components/EditorTopBar.tsx`
  - `src/views/HomeView.tsx`
  - `src/components/EditorRightInspector.tsx`
  - `src/views/PDFEditorView.tsx`
  unless the user explicitly commands a layout change.
- **No Arbitrary Style Refactoring**: Do not convert existing inline styles or CSS classes into ad-hoc frameworks or arbitrary rewrites under the guise of "cleaning up" or "improving aesthetics".

## 2. Feature & Shortcut Preservation (既存機能の死守)
- **Never Drop Features**: When refactoring or updating components, every single existing feature must be preserved:
  - Print button & shortcut (`⌘P`)
  - Undo/Redo dual-tier handling (`⌘Z`, `⌘⇧Z`)
  - Zoom preset dropdown & buttons (`[−]`, `[＋]`, `select`)
  - Single-key tool shortcuts (`V`, `T`, `H`, `A`, `R`, `P`)
  - Command Palette (`⌘K`)
- **No Fake Fallbacks**: Do not introduce mock web fetch fallbacks (`fetch('/')`) or artificial blacklist filters on user document names.

## 3. High-Performance Zero-IPC Standard
- Always prioritize Session-based IPC (`session_xxx` passing `doc_id`) over transferring full PDF byte arrays (`Vec<u8>`) over Tauri IPC.
