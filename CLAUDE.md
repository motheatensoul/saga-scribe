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

## Missing features:

- Search/replace functionality in editor
- Editor undo/redo (CodeMirror has built-in support, needs UI integration)
- Import from other transcription formats
- XSLT support for rendering from XML
- Export to additional formats (HTML, PDF)
- Collapsible sections in editor for long documents
- Word- and character-level annotations beyond lemmatization
- Manuscript metadata editor (beyond template header)
- Custom entity set management (add/remove entities beyond MENOTA)
- Performance optimization for large manuscripts (>10k lines)


