# Agent Instructions for Saga-Scribe

## 1. Build, Lint, and Test Commands

**Package Manager:** Always use `bun`.

### Frontend (Svelte/TypeScript)
- **Install:** `bun install`
- **Dev Server:** `bun run dev`
- **Build:** `bun run build`
- **Type Check:** `bun run check` (Svelte-check)
- **Lint:** `bun run check` covers most linting needs.

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
| `.abbr[a]{b}` | `<choice><abbr>a</abbr><expan>b</expan></choice>` | Abbreviation |
| `[...]` | `<gap/>` | Gap |
| `[...<txt>]` | `<gap/><supplied>txt</supplied>` | Gap + Supplied |
| `<txt>` | `<supplied>txt</supplied>` | Supplied |
| `-{txt}-` | `<del>txt</del>` | Deletion |
| `+{txt}+` | `<add>txt</add>` | Addition |
| `:name:` | `&name;` | Entity |

## Testing
Run Rust tests: `cd src-tauri; cargo test`
~200 tests cover parser, compiler, importer round-trip, normalizer, and dictionary.

## Fonts
Uses **Junicode** (in `static/fonts/`) for MUFI characters.

## Imported Mode (Round-Trip) Guarantees
The user's file is privileged. Reading it must never change it:
- Unedited content is emitted byte-for-byte: the importer records source spans (`src-tauri/src/importer/tei/source_map.rs`) and the patcher splices edits into the original bytes (`patching.rs`).
- Whitespace-only or otherwise non-semantic DSL changes must produce no XML change.
- Invalid DSL is an error, never an empty document; save/export refuse to write a failed compile.
- Regression tests: `test_menota_file_unchanged_is_byte_identical` and friends in `src-tauri/src/importer/tests.rs`.

## Development Notes
- **Toolchain:** Rust is pinned in `rust-toolchain.toml`; the crate MSRV is `rust-version` in `src-tauri/Cargo.toml`.
- **SvelteKit 3:** configuration lives in `vite.config.ts` (`sveltekit({...})`); there is no `svelte.config.js`. The `$lib` alias is configured there.
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

