# Changelog

All notable changes to Saga-Scribe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - Unreleased

### Added

- **Direct PDF export** (Linux, Windows): choose where to save, and the PDF is written without the print dialog; the document is laid out in a hidden window so the editor stays responsive. macOS keeps the print dialog ("Save as PDF")
- **Bundled Junicode in PDFs**, so MUFI characters print even where the font isn't installed
- **Export progress**: HTML and PDF export run behind a progress overlay showing the current step, with a Cancel button
- **Incremental previews**: Rendered Text re-parses only the pages an edit touches; the XML view applies only the changed range
- **Security audit for JS dependencies** in CI, also run on dependency PRs

### Changed

- **Imported files are never modified by reading them**: unedited content is written back byte-for-byte, and edits only replace the bytes of what was edited
- **Project file format 1.5**: `segments.json` always holds the full manifest (with MENOTA flag and source spans); 1.4 projects still open
- **Non-MENOTA imports** keep their own encoding style (plain `<w>` or plain text) instead of gaining MENOTA markup on edited words
- **Validation** runs after a short pause in typing instead of on every compile, and parsed schemas are cached
- **Only the visible preview tab updates**; hidden tabs catch up when shown
- **Content Security Policy** enabled; XSLT output is sanitized before it is rendered, exported or printed
- **SvelteKit 3 / Vite 8**, Tauri 2.12, zip 9 and other dependency upgrades; minimum Rust version 1.90, toolchain pinned
- **Window title and dialogs** use the Saga Scribe name

### Fixed

- **Round-trip data loss**: a DSL syntax error no longer wipes the document body; distant edits in large files no longer rewrite (and drop lemmas from) everything in between
- **Line/page break edits** in imported mode are applied; inserted words stay in the right paragraph
- **MENOTA detection** on import
- **Save/export** refuse to write a failed compile instead of writing the error message
- **Saved imported projects** reopen in imported mode (segments were saved in a shape that failed to load)
- **Saves are atomic**: a crash mid-save can no longer truncate a project or export
- **PDF/HTML export freeze** on large files (minutes spent resolving entity placeholders)
- **Rendered Text**: `<add>`/`<del>`/`<unclear>` around words showed all three transcription levels and stray line breaks; sic/corr words were missing; the view no longer jumps to page 1 or flashes on every edit
- **Word numbering** now follows document order everywhere, so lemmas loaded from a file stay on the right words
- **Security**: XSS via XSLT output, network access during XML parsing, vulnerable `zip`, Svelte, Vite and PostCSS versions

## [0.5.0] - 2026-01-22

### Added

- **Export formats**: TEI-XML, standalone HTML, and PDF export via browser print dialog
- **Dictionary export**: Export lemmatization data as JSON for backup and analysis
- **Custom entity management**: Create, edit, and delete custom entities beyond the MENOTA/MUFI set
- **Imported mode**: Preserve original XML structure during round-trip editing of imported TEI files
- **Metadata editor**: Edit TEI header metadata (title, author, source, language, etc.)
- **Word and character annotations**: MENOTA me:msa morphological annotations with visual indicators
- **XSLT preview mode**: Apply custom XSLT stylesheets to preview XML transformations
- **Page navigation**: Browse large manuscripts by page with instant navigation controls
- **Collapsible sections**: Fold/unfold pages in the editor at page break markers (`///`)
- **Search and replace**: Find and replace text in the editor (Ctrl+H)
- **Settings persistence**: Theme, font size, auto-preview, and template preferences saved between sessions

### Changed

- **Version bump**: 0.3.0 to 0.5.0 reflecting significant feature additions
- **Project file format**: Updated to v1.3 with segment storage for imported documents
- **MENOTA rendering**: Improved multi-level transcription with `<choice>` wrappers around facs/dipl/norm elements
- **Entity browser**: Enhanced with custom entity editing and deletion capabilities
- **Import flow**: Better handling of complex MENOTA structures and namespace declarations

### Fixed

- **XSLT rendering**: Resolved issues with DOCTYPE declarations and namespace handling
- **Windows build**: Properly configure libxml2 linking via vcpkg
- **Windows headers**: Add BINDGEN_EXTRA_CLANG_ARGS for SDK headers
- **Accessibility (a11y)**: All modals now support Escape key dismissal; resolved all Svelte a11y warnings
- **Import stability**: Enhanced handling of unclear tags and edge cases in MENOTA imports
- **Whitespace normalization**: Consistent DSL output from import operations

### Technical

- MENOTA morphology constants extracted to `src/lib/constants/menota.ts`
- Debug console.log statements removed from production code
- 167 passing Rust tests (162 core + 5 documented limitations as ignored)
- Zero Svelte check warnings
- Zero Clippy warnings

## [0.4.0] - 2024-12-XX (alpha)

Initial alpha release with core transcription features.

### Added

- Custom DSL syntax for manuscript transcription
- Multi-level MENOTA transcription support (facs/dipl/norm)
- Entity browser with ~1,980 MENOTA/MUFI characters
- ONP dictionary integration for lemmatization
- Project archives (.teis) bundling source, XML, and annotations
- Real-time preview with text, XSLT, and XML views
- Template system with built-in TEI P5 and MENOTA templates
- Import support for TEI-XML and plain text files

## [0.3.0] - 2024-XX-XX

Early development release.
