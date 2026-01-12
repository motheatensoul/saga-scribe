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

### Full Check
- **Verify everything:** `bun run check:all`

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
66+ tests cover parser, compiler, normalizer, and dictionary.

## Fonts
Uses **Junicode** (in `static/fonts/`) for MUFI characters.

## Development Notes
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

## Missing features:

- [x] Search/replace functionality in editor
- [x] Editor undo/redo (CodeMirror has built-in support, needs UI integration)
- [x] Import from other transcription formats (TEI-XML, text)
- [x] XSLT support for rendering from XML (browser-based XSLTProcessor)
- [x] Text rendering performance (page-based chunking, removed 5k token limit)
- [x] Clean up README, docs to include new features
- [x] Collapsible sections in editor for long manuscripts (fold at page breaks)
- [x] Manuscript metadata editor (beyond template header)
- [x] Word- and character-level annotations beyond lemmatization
- Export to additional formats (HTML, PDF)
- Custom entity set management (add/remove entities beyond MENOTA)
- Performance optimization
- Code cleanup, refactoring

