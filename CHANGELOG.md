# Changelog

All notable changes to Saga-Scribe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - Unreleased

### Added

- **Lemmas are written into imported files**: confirming a lemma sets `lemma` and `me:msa` (or `msa`) on that word in the imported XML. Lemmas that came from the file leave it byte-identical. Notes and other annotations stay in the project, as they would add markup the file doesn't have

### Fixed

- **Annotations moved to the wrong words** when words were inserted or deleted before them: lemmas and annotations are stored by word number, which shifted. The backend now keeps track of which word is which across edits (as imported mode did since 0.6), reports how word numbers moved after each compile, and the app moves lemmas, annotations, their undo history and the selection along
- **Another word typed in place of a deleted one inherited the old word's lemma and normalized form** (in imported files, the normalized form stuck even when the old word was typed back). Only an edit of a word (differing in at most half its letters, like a typo fix) keeps its lemma, analysis and normalized form; another word in its place is a new word, written in the same place
- **Removing a lemma**: the word panel has "Remove lemma from this word"; its ✕ on known analyses removes the analysis from the dictionary (now labelled so), which left the word's lemma in place. On imported files, removing a lemma removes `lemma` and `me:msa` from the word in the file (and is kept in the project, undoable)
- **Undo and redo** of lemmas were never enabled: the toolbar followed a history nothing was recorded in
- **Analyses with their tags in another order** (`xNC gM nS` and `gM xNC nS`) counted as different analyses; they are the same, in the dictionary and when writing lemmas into imported files
- **ONP unreachable**: when an entry's details can't be fetched (offline, or onp.ku.dk down), the word panel says so instead of waiting indefinitely; the analysis can still be entered
- **Lemmas after a heading were off by the heading's words**: words inside `.head{}`, `.supplied{}` and `.norm{}` weren't counted when lemmas and annotations were applied, though the previews count them

## [0.6.1] - 2026-10-29

### Fixed

- **XSLT preview lag**: editing a large manuscript with the XSLT preview open re-ran the stylesheet on the whole document after every pause in typing (over 2 s for HolmPerg, blocking the editor). Stylesheets that declare `<?saga-scribe render="page"?>`, like the built-in one, are now applied one page at a time, and after an edit only to the pages that changed (about 70 ms); other stylesheets render as before

## [0.6.0] - 2026-10-29

### Added

- **Direct PDF export** (Linux, Windows): choose where to save, and the PDF is written without the print dialog, from a hidden window so the editor stays responsive. macOS keeps the print dialog ("Save as PDF")
- **Paragraphs in the DSL**: `.p` starts a new paragraph, also mid-line. Imported files show each `<p>` as `.p`, so paragraphs can be split and joined; only the paragraph tags change in the file, and a split that would break the XML (e.g. inside a sentence `<s>`) is reported instead. (The pilcrow ¶ stays the MENOTA character `:para:`.)
- **Typeset PDFs**: pages are laid out with [Vivliostyle](https://vivliostyle.org/) (CSS Paged Media), giving fixed page margins, page numbers, and no headings or single lines stranded at a page edge
- **Bundled Junicode in PDFs**, so MUFI characters print even where the font isn't installed
- **Syntax errors in the editor**: every DSL syntax error is underlined where it occurs, not just the first; compile errors name the line and column
- **Export progress**: HTML and PDF export run behind a progress overlay showing the current step, with a Cancel button
- **Incremental previews**: Rendered Text re-parses only the pages an edit touches; the XML view applies only the changed range
- **Security audit for JS dependencies** in CI, also run on dependency PRs
- **Frontend unit tests** (Vitest) run in CI: rendered-text parsing, incremental rendering equivalence, XSLT sanitizing

### Changed

- **Imported files are never modified by reading them**: unedited content is written back byte-for-byte, and edits only replace the bytes of what was edited
- **Project file format 1.6**: `segments.json` always holds the full manifest (with MENOTA flag and source spans for every tag); 1.4 and 1.5 projects still open. Files imported before this version don't show `.p` until re-imported
- **Imported XML and its DSL read as the same words**: a `<w>` written as several words (`þo at`, 1,447 in HolmPerg) is written `þo~at`, and entity punctuation that is its own `<pc>` (`:middot:`, 483) is set off by spaces. Documents imported earlier keep their DSL as it was
- **Entity punctuation alone between spaces** (`þa :middot: þo`) compiles to `<pc>`; glued to letters (`:middot:Q:middot:`) it stays part of the word
- **Imported mode knows which word you edited**: the editor tells the backend exactly what changed, so deleting or editing one of two identical neighbouring words (`ok ok` with different lemmas) changes that word, not whichever one a comparison guesses. Each compile is also faster, as the original document is analysed once instead of on every keystroke
- **Editing an imported word** keeps its lemma, analysis and hand-curated normalized form (before, the normalized form was regenerated from the dictionary); the diplomatic form follows the edit. Such words are underlined in the previews and flagged "edited" in the word panel until re-imported, as a reminder to check them
- **Non-MENOTA imports** keep their own encoding style (plain `<w>` or plain text) instead of gaining MENOTA markup on edited words
- **DSL parser**: rebuilt on a hierarchical syntax tree with source positions and error recovery (internal foundation for the round-trip work); output is unchanged
- **Syntax errors inside `.head{}`, `.supplied{}` and `.norm{}`** are reported, instead of compiling to an XML comment
- **Validation** runs after a short pause in typing instead of on every compile, and parsed schemas are cached
- **Only the visible preview tab updates**; hidden tabs catch up when shown
- **Content Security Policy** enabled; XSLT output is sanitized before it is rendered, exported or printed
- **SvelteKit 3 / Vite 8**, Tauri 2.12, zip 9 and other dependency upgrades; minimum Rust version 1.90, toolchain pinned
- **Window title and dialogs** use the Saga Scribe name
- **Flatpak runtime**: GNOME 51 (was 49)
- **Third-party licences** are listed in `THIRD_PARTY_LICENSES.md` (dependency tables regenerated with `bun run licenses`)
- **Smaller app**: an 8 MB test manuscript was being bundled into the app; it now lives in `tests/fixtures/`
- **Line numbers after `~//` are digits only**, as after `//`: `hel~//5lo` is one word broken across lines 4 and 5 (the importer writes breaks this way); whitespace right after `~//5` no longer splits the word

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
- **Mid-word line breaks** (`~//`): the compiled XML kept the break (single-level) or dropped it entirely (MENOTA), and read the rest of the word as the line number; the default XSLT dropped breaks inside words, merging lines and losing their numbers. It now shows a broken word as one box per line, the first ending in a hyphen
- **Default XSLT, line-by-line transcriptions**: a paragraph starting mid-line pushed the rest of that line onto a row of its own; paragraphs now run inline there, marked with a pilcrow (¶). Prose without line breaks (e.g. front matter) keeps block paragraphs
- **Word spaces between entities** inside a level (`<me:facs>&et; &thorn;a</me:facs>`) were dropped on import, joining two written words
- **A dangling or doubled `~`** (`upp~ haf`, `upp~~haf`) no longer compiles to stray spaces inside a word
- **Abbreviations in MENOTA output**: entities inside `.abbr[…]` were written as literal text (`k:bar:` instead of `k&bar;`), and reading such a file back lost the expansion
- **PDF margins**: page margins were ignored, so text ran to the edge of the page
- **Characters lost on import with older libxml2** (Ubuntu 24.04, i.e. the .deb): undeclared entities such as `&thorn;` were dropped, from text and from attributes like `lemma`. Every entity a document uses is now declared before parsing, so import and validation behave the same on every libxml2 version, and never access the network
- **MENOTA exports declare their entities**: the Menota template and the generated header carry Menota's DOCTYPE (the external entity list, as Menota files have), so exported files using `&thorn;` and the like are well-formed XML
- **Flatpak couldn't start**: it packaged a binary built on Ubuntu, linked against `libxml2.so.2`, which the GNOME runtime doesn't have (it ships libxml2 2.14+). The Flatpak is now compiled inside the GNOME SDK against the runtime's own libraries, which also get the runtime's security updates
- **Export menu** stopped responding after cancelling a file dialog
- **Save and export dialogs** add the file extension (`.teis`, `.xml`, `.html`, `.pdf`, `.json`) when a name is typed without one
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
