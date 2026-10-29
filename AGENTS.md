# Agent Instructions for Saga-Scribe

## 1. Build, Lint, and Test Commands

**Package Manager:** Always use `bun`.

### Frontend (Svelte/TypeScript)
- **Install:** `bun install`
- **Dev Server:** `bun run dev`
- **Build:** `bun run build`
- **Type Check:** `bun run check` (Svelte-check)
- **Lint:** `bun run check` covers most linting needs.
- **Test:** `bun run test:js` (Vitest, jsdom; `*.test.ts` next to the module under test). Components can be mounted with Svelte's `mount`/`flushSync`; name such tests `*.svelte.test.ts` so runes like `$state` work in them (see `XsltRenderer.svelte.test.ts`). Effects that write state must not read it back (`untrack`), or Svelte aborts with `effect_update_depth_exceeded`, which SvelteKit shows as a 500 error page. Shared fixtures live in `src/lib/rendered/fixtures.ts`. `HOLMPERG=1 bun run test:js` also runs the slow full-manuscript equivalence test.
- **All tests:** `bun run test` (frontend + Rust)

### Backend (Rust)
*Note: Run these commands inside `src-tauri/` or use the provided wrapper scripts.*

- **Lint (Clippy):** `cd src-tauri; cargo clippy -- -D warnings` (or `bun run check:rust`)
- **Test (All):** `cd src-tauri; cargo test` (or `bun run test:rust`)
- **Test (Single):** `cd src-tauri; cargo test -- test_name_here`
  - *Critical:* When running cargo commands via the Bash tool, avoid `cd src-tauri && cargo ...`. Use `cd src-tauri; cargo ...` or `bash -c "cd src-tauri && cargo ..."` to avoid exit code 127 errors.

### Full Check (Required Before PRs)
- **Verify everything:** `bun run check:all` (runs both `svelte-check` and `cargo clippy`)
- **Always use this command** before committing or creating PRs. Both frontend and Rust clippy checks must pass for merging.

### Versioning (SemVer)
The project follows [Semantic Versioning](https://semver.org/); while in 0.x, a **minor** bump (0.5 → 0.6) covers new features and breaking changes, a **patch** bump (0.6.0 → 0.6.1) covers fixes only.
- **One version, four places:** `package.json`, `src-tauri/Cargo.toml` (then `cargo update -p saga-scribe` for `Cargo.lock`), `src-tauri/tauri.conf.json`, and the version line in `README.md`. Keep them identical.
- **Bump on the first change after a release:** once `vX.Y.Z` is tagged, the next commit that isn't purely a fix bumps the minor version; a fixes-only follow-up bumps the patch. Check `git tag` / `gh release list` before starting work.
- **Branch names follow the target version:** work towards 0.6.0 happens on `dev/0.6.0` (or `dev/0.6.0-<topic>`).
- **Third-party licences:** after adding, removing or upgrading dependencies, run `bun run licenses` to regenerate the tables in `THIRD_PARTY_LICENSES.md`; describe new bundled fonts or data there by hand.
- **Changelog:** record user-visible changes under the unreleased version in `CHANGELOG.md` (Keep a Changelog format) as they land.
- **At release time:** set the release date in `CHANGELOG.md` and add a `<release>` entry to `flatpak/app.sagascribe.Sagascribe.metainfo.xml`.
- **Project file format** (`.teis`, see `src-tauri/src/commands/file.rs`) is versioned separately; bump it whenever the archive contents change, and keep older versions loadable.

## 2. Code Style Guidelines

### General
- **Conventions:** Adhere strictly to existing conventions. Analyze surrounding code first.
- **Dependencies:** Keep external dependencies to an absolute minimum.
- **Security:** Never roll your own auth or crypto.

### Frontend (Svelte 5 + TypeScript)
- **Framework:** Svelte 5. Use Runes syntax (`$props`, `$state`, `$derived`) extensively.
- **Components:** Use `lang="ts"`. Define props with `let { prop }: { prop: Type } = $props();`.
- **Imports:** Use `$lib/` alias for local imports.
- **Formatting:** 4 spaces indentation.
- **Naming:** PascalCase for components (`Editor.svelte`), camelCase for functions/vars.
- **Visual Style:** Refer to docs/UI_STANDARDS.md for our UI masterdoc. Style every new component as described.
- **Accessibility (a11y):** All components MUST pass Svelte's a11y checks (`bun run check` must show 0 warnings). This is not optional.
  - Use semantic HTML elements (`<button>`, `<nav>`, `<main>`, etc.)
  - All form inputs must have associated labels (use `for`/`id` or wrap in `<label>`)
  - For control groups, use `role="group"` with `aria-labelledby`
  - Interactive elements must be keyboard accessible
  - Images need `alt` attributes
  - Exception: Modal backdrop click-to-dismiss does not require keyboard equivalent

### Backend (Rust)
- **Style:** Standard Rust idioms (`rustfmt`). 4 spaces indentation.
- **Comments:** Doc comments (`///`) for public items.
- **Error Handling:** Use `Result` and `Option`. Propagate errors, avoid `unwrap()`/`expect()` in production code.
- **Modules:** Keep structure clean. New parser logic goes in `src/parser/`.

---

# Project Documentation

## Overview
Saga-Scribe is a Tauri desktop application for manuscript transcription using a custom DSL that compiles to TEI-XML, with MENOTA support.
**Stack:** Svelte 5 + SvelteKit (Frontend), Rust + Tauri 2.9 (Backend).

## Architecture
```
saga-scribe/
├── src/                          # Frontend
│   ├── lib/
│   │   ├── components/           # UI Components
│   │   ├── parser/               # DSL syntax highlighting (Lezer)
│   │   ├── stores/               # Svelte stores (Runes/Svelte 5)
│   │   └── tauri.ts              # IPC Bridge
│   └── routes/+page.svelte       # Main Entry
├── src-tauri/                    # Backend
│   ├── src/
│   │   ├── parser/               # DSL Compiler (Lexer, AST, Tokenizer)
│   │   ├── commands/             # Tauri Commands
│   │   ├── template/             # Template Manager
│   │   ├── normalizer/           # MENOTA Normalization
│   │   └── entities/             # Entity Registry
│   └── tauri.conf.json
└── static/                       # Assets (Fonts, JSON dictionaries)
```

## Key Files
- **DSL Parser:** `src-tauri/src/parser/lexer.rs`, `src-tauri/src/parser/compiler.rs`
- **Word Tokenizer:** `src-tauri/src/parser/wordtokenizer.rs`
- **Entities:** `static/entities/menota.json`, `src-tauri/src/entities/registry.rs`
- **Editor:** `src/lib/components/Editor.svelte`
- **ONP Dictionary:** `src-tauri/src/dictionary/onp.rs`

## DSL Syntax Reference (Brief)
| Syntax | Output | Description |
|--------|--------|-------------|
| `//` | `<lb/>` | Line break |
| `//n` | `<lb n="n"/>` | Numbered line break |
| `///n` | `<pb n="n"/>` | Page break |
| `.p` | `</p><p>` | New paragraph (standalone token; `¶` itself is the MENOTA character `:para:`) |
| `.abbr[a]{b}` | `<choice><abbr>a</abbr><expan>b</expan></choice>` | Abbreviation |
| `[...]` | `<gap/>` | Gap |
| `[...<txt>]` | `<gap/><supplied>txt</supplied>` | Gap + Supplied |
| `<txt>` | `<supplied>txt</supplied>` | Supplied |
| `-{txt}-` | `<del>txt</del>` | Deletion |
| `+{txt}+` | `<add>txt</add>` | Addition |
| `:name:` | `&name;` | Entity |

## Testing
- Rust: `cd src-tauri; cargo test` (~200 tests: parser incl. differential tests against the original lexer, compiler, importer round-trip, normalizer, dictionary)
- Frontend: `bun run test:js` (rendered-text parsing, incremental rendering equivalence, XSLT sanitizing, text diff)
- Note: jsdom has no `XSLTProcessor`; XSLT transforms themselves are not covered by unit tests.

## Fonts
Uses **Junicode** (in `static/fonts/`) for MUFI characters.

## Imported Mode (Round-Trip) Guarantees
The user's file is privileged. Reading it must never change it:
- Unedited content is emitted byte-for-byte: the importer records source spans (`src-tauri/src/importer/tei/source_map.rs`) and the patcher splices edits into the original bytes (`patching.rs`).
- Whitespace-only or otherwise non-semantic DSL changes must produce no XML change.
- Invalid DSL is an error, never an empty document; save/export refuse to write a failed compile.
- Confirmed lemmas are written into the words' start tags (`write_lemmas`), but only where they differ from what the tag says with references resolved: lemmas loaded from the file must leave it byte-identical (`test_menota_file_with_its_own_lemmas_stays_byte_identical`).
- The XML and the DSL generated from it read as the same tokens, one to one (`test_menota_file_xml_and_dsl_read_as_the_same_tokens`); DSL → XML → DSL likewise. A new XML construct the DSL can't express breaks this: give it DSL syntax rather than special-casing the patcher.
- Word numbers: lemmas and annotations are stored by word number (the n-th `<w>` of the body, as the previews count). The backend tracks which token is which across edits in both modes (`src-tauri/src/identity.rs`; `Tracked`, the compiler's word keys), and each compile returns `renumbering`: where the previous compile's words went. `src/lib/stores/compileSession.ts` runs compiles one at a time, applies the renumbering (`onRenumbering`) and only then lets the next compile gather its payload: annotations sent to the backend are always numbered by its previous compile. A compile without guidance means a new document (no renumbering). Anything new that stores word numbers must be renumbered in `onRenumbering`.
- Word identity: the backend `Patcher` (`patching.rs`) remembers each token of the last compiled DSL and which original word it is. The editor's changes since then (`src/lib/editor/editHistory.ts`, sent as `Guidance`) let untouched words keep their identity outright; only edited ranges are matched by comparison. Guidance only refines which word is which: without it (or out of step) the patcher compares whole documents, and the output must be correct either way (`test_guided_character_edits_always_reimport_as_edited` fuzzes this).
- Parse TEI documents with `crate::xml::parse` (`src-tauri/src/xml.rs`), never libxml2 directly: it declares the document's entities first (libxml2 2.9 drops undeclared ones, 2.14+ keeps them) and disables network access. CI tests on Ubuntu's libxml2 2.9 while local machines usually have 2.14+, so entity handling bugs may show up in only one of them.
- Changing how imported XML is written as DSL changes users' stored DSL: bump `DSL_RULES` (`src-tauri/src/importer/tei/segments.rs`) and keep older documents on their rules.
- Regression tests: `test_menota_file_unchanged_is_byte_identical` and friends in `src-tauri/src/importer/tests.rs`.

## Development Notes
- **Toolchain:** Rust is pinned in `rust-toolchain.toml` (CI reads it too, via `.github/actions/setup-rust`); the crate MSRV is `rust-version` in `src-tauri/Cargo.toml`.
- **CI:** PRs to `main` run `test.yml`; pushes to `main` run the full multi-platform `build.yml` and publish the nightly release. Dev branches don't build automatically: to check every platform before merging, run "Build" manually on the branch (`gh workflow run build.yml --ref <branch>`), which builds without publishing. Windows libxml2 setup lives in `.github/actions/setup-libxml2-windows` (shared by build and release).
- **Flatpak:** the app is compiled inside the GNOME SDK (`flatpak/app.sagascribe.Sagascribe.yml`, with the Rust and LLVM SDK extensions), so it links the runtime's libraries; only the frontend is built outside (`bun run build`). Build locally with `flatpak/build-flatpak.sh`. Don't package a binary built on the host or Ubuntu: it links that system's libraries (e.g. `libxml2.so.2`, which GNOME runtimes no longer ship). When moving to a new GNOME runtime, check its SDK's `org.freedesktop.Sdk.Extension` version (`flatpak info --show-location org.gnome.Sdk//N`, `metadata`) and the extensions available for it.
- **SvelteKit 3:** configuration lives in `vite.config.ts` (`sveltekit({...})`); there is no `svelte.config.js`. The `$lib` alias is configured there.
- **PDF export:** the main window paginates the export HTML with Vivliostyle in an off-screen iframe (`src/lib/utils/paginate.ts`; hidden windows throttle timers, which makes Vivliostyle ~20x slower) and hands the static pages to the hidden `/print` window, which prints them with zero native margins. Page layout (margins, page numbers) lives in `withPrintStyles()` (`src/lib/utils/pdfExport.ts`). Vivliostyle is AGPL-3.0, compatible with our GPL-3.0-or-later.
- **XSLT preview by page:** stylesheets declaring `<?saga-scribe render="page"?>` (like `static/xsl/simple.xsl`) are applied one page at a time and only re-run on changed pages (`src/lib/rendered/xsltPages.ts`). Such a stylesheet's output for an element may only depend on the element and its ancestors: when editing `simple.xsl`, don't use `preceding::`/`following::`, keys or document-wide counts (pass document-wide facts as parameters instead, like `lineBased`), or remove the declaration.
- **XSLT output:** always pass through `sanitizeXsltHtml()` (`src/lib/utils/xslt.ts`, which also holds the shared preview/export pipeline) before rendering or exporting; the app runs under a strict CSP (`src-tauri/tauri.conf.json`).
- **Cargo Command Quirk:** `cd src-tauri && cargo ...` fails. Use `cd src-tauri; cargo ...`.
- **Bun:** Always use `bun` instead of `npm`.

## Async Patterns (Critical for UI Responsiveness)

Large manuscripts can have 50k+ tokens. Any synchronous operation processing this much data will freeze the UI and block CSS animations. Follow these patterns:

### Frontend: Chunked Async Processing
For any operation that processes large data (parsing, rendering, transforming):
```typescript
// Helper to yield to browser for smooth animations
const yieldToMain = () => new Promise(resolve => setTimeout(resolve, 0));

// Yield every N items to keep UI responsive
async function processLargeData(items: Item[]) {
    const YIELD_EVERY = 500;
    for (let i = 0; i < items.length; i++) {
        if (i % YIELD_EVERY === 0) {
            await yieldToMain();
        }
        // Process item...
    }
}
```

### Loading Spinners
Use SVG spinners with `<animateTransform>` instead of CSS animations - they're more reliable during heavy processing:
```svelte
<svg viewBox="0 0 50 50" width="48" height="48">
    <circle cx="25" cy="25" r="20" fill="none" stroke="currentColor"
        stroke-width="4" stroke-linecap="round"
        stroke-dasharray="90, 150" stroke-dashoffset="0" class="text-primary">
        <animateTransform attributeName="transform" type="rotate"
            from="0 25 25" to="360 25 25" dur="1s" repeatCount="indefinite"/>
    </circle>
</svg>
```

### Backend: Tauri Commands
All Tauri commands that do heavy work should:
1. Use `#[tauri::command(async)]`
2. Use `tauri::async_runtime::spawn_blocking()` for CPU-bound work

### Key Files with Async Patterns
- `src/lib/components/RenderedText.svelte` - Async XML parsing with `extractTokensAsync()`
- `src-tauri/src/commands/parse.rs` - Async compile with `spawn_blocking()`
- `src-tauri/src/commands/import.rs` - Async import with `spawn_blocking()`

## Feature status:

- [x] Search/replace functionality in editor
- [x] Editor undo/redo (CodeMirror has built-in support, needs UI integration)
- [x] Import from other transcription formats (TEI-XML, text)
- [x] XSLT support for rendering from XML (browser-based XSLTProcessor)
- [x] Text rendering performance (page-based chunking, removed 5k token limit)
- [x] Clean up README, docs to include new features
- [x] Collapsible sections in editor for long manuscripts (fold at page breaks)
- [x] Manuscript metadata editor (beyond template header)
- [x] Word- and character-level annotations beyond lemmatization
- [x] Export to additional formats (HTML, PDF)
- [x] Custom entity set management (add/remove entities beyond MENOTA)
- [x] Imported mode for round-trip TEI-XML editing with structure preservation
- [ ] Performance optimization
- [ ] Code cleanup, refactoring

