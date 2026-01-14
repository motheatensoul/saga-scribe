# Contributing to Saga-Scribe

Thank you for your interest in contributing to Saga-Scribe! This document provides guidelines and instructions for setting up your development environment and submitting contributions.

## Getting Started

### Prerequisites

- **Package Manager:** [bun](https://bun.sh/) (Required)
- **Rust Toolchain:** Stable release
- **Tauri CLI:** v2.9+

### Build & Run

1.  **Clone the repository:**

    ```bash
    git clone https://github.com/motheatensoul/saga-scribe
    cd saga-scribe
    ```

2.  **Install dependencies:**

    ```bash
    bun install
    ```

3.  **Run Development Server:**

    ```bash
    bun run tauri dev
    ```

4.  **Build for Production:**
    ```bash
    bun run tauri build
    ```

## Project Structure

```
saga-scribe/
├── src/                          # Frontend (Svelte 5 + TypeScript)
│   ├── lib/
│   │   ├── components/           # UI Components
│   │   ├── parser/               # DSL syntax highlighting (Lezer)
│   │   ├── stores/               # Svelte stores (Runes)
│   │   ├── tauri.ts              # IPC Bridge
│   │   └── types/                # TypeScript definitions
│   └── routes/+page.svelte       # Main Entry
├── src-tauri/                    # Backend (Rust)
│   ├── src/
│   │   ├── commands/             # Tauri Commands
│   │   ├── dictionary/           # Dictionary modules (ONP, etc.)
│   │   ├── entities/             # Entity registry
│   │   ├── importer/             # Import logic (TEI, text)
│   │   ├── normalizer/           # MENOTA Normalization
│   │   ├── parser/               # DSL Compiler (Lexer, AST)
│   │   ├── validator/            # Validation logic
│   │   └── lib.rs                # Lib entry point
│   └── tauri.conf.json
└── static/                       # Assets (Fonts, JSON dictionaries)
```

## Development Workflow

### Frontend (Svelte/TypeScript)

- **Install:** `bun install`
- **Lint/Check:** `bun run check` (Runs `svelte-check`; must show 0 warnings)
- **Format:** 4 spaces indentation

### Backend (Rust)

_Run these commands inside `src-tauri/` or use the provided wrapper scripts._

- **Lint:** `cd src-tauri; cargo clippy -- -D warnings` (or `bun run check:rust`)
- **Test:** `cd src-tauri; cargo test` (or `bun run test:rust`)

### Pull Requests

Before submitting a PR, you **MUST** verify everything passes:

```bash
bun run check:all
```

This ensures both Svelte type checks and Rust clippy lints pass.

## Code Style Guidelines

### General

- **Conventions:** Adhere strictly to existing conventions. Analyze surrounding code first.
- **Dependencies:** Keep external dependencies to an absolute minimum.

### Frontend (Svelte 5 + TypeScript)

- **Framework:** Svelte 5 with Runes (`$props`, `$state`, `$derived`).
- **Components:** Use `lang="ts"`. Define props with `let { prop }: { prop: Type } = $props();`.
- **Formatting:** 4 spaces indentation.
- **Naming:** PascalCase for components (`Editor.svelte`).
- **Accessibility:** All components MUST pass a11y checks. Use semantic HTML.

### Backend (Rust)

- **Style:** Standard Rust idioms (`rustfmt`). 4 spaces indentation.
- **Error Handling:** Propagate errors using `Result`. Avoid `unwrap()`/`expect()` in production.
- **Async:** Use `#[tauri::command(async)]` and `spawn_blocking()` for heavy CPU operations.

## Architecture Notes

### Async Processing

Large manuscripts can have 50k+ tokens.

- **Frontend:** Use chunked async processing with `yieldToMain` to keep UI responsive.
- **Backend:** Offload heavy work to `spawn_blocking`.

### DSL Syntax

See `README.md` or `docs/user-guide.md` for DSL syntax details.

## License

By contributing, you agree that your contributions will be licensed under the [GPL-v3-or-later](LICENSE).
