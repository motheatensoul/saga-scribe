- **Always use bun instead of npm/node.js when relevant!**

- Keep external dependencies to an absolute minimum, random npm or cargo packages are especially risky vis-a-vis supply chain attacks.

- Do never, under any circumstance roll your authentication or cryptography.

- Write clean and concise code, avoid bloat wherever possible.

---

# TEI-Scribe Project Documentation

## Overview

TEI-Scribe is a Tauri desktop application for manuscript transcription using a custom DSL that compiles to TEI-XML, with specific support for MENOTA (Medieval Nordic Text Archive) extensions.

**Version:** 0.0.1 (early development)
**License:** MIT

## Tech Stack

- **Frontend:** Svelte 5 + SvelteKit, TypeScript, CodeMirror 6, svelte-splitpanes
- **Backend:** Rust + Tauri 2.9
- **Package Manager:** bun (not npm)

## Architecture

```
tei-scribe/
├── src/                          # Frontend (Svelte/TypeScript)
│   ├── lib/
│   │   ├── components/           # UI: Editor, Preview, Toolbar, TemplateManager, EntityBrowser
│   │   ├── parser/highlighter.ts # DSL syntax highlighting (stream-based)
│   │   ├── stores/               # Svelte stores: editor, template, entities, settings
│   │   └── tauri.ts              # Tauri IPC bridge
│   └── routes/+page.svelte       # Main application page
├── src-tauri/                    # Backend (Rust)
│   ├── src/
│   │   ├── parser/               # lexer.rs, ast.rs, compiler.rs, wordtokenizer.rs
│   │   ├── commands/             # Tauri commands: parse, template, file, entities
│   │   ├── template/manager.rs   # Template system with built-in TEI P5 & MENOTA
│   │   ├── normalizer/           # Level dictionary for multi-level transcription
│   │   └── entities/registry.rs  # Entity lookup system
│   └── tauri.conf.json
├── static/entities/menota.json   # ~1,980 MENOTA/MUFI character entities
├── static/normalizer/menota-levels.json  # Multi-level derivation mappings
└── static/normalizer/entity-base-letters.json  # Diplomatic normalization mappings
```

## DSL Syntax Reference

| DSL Syntax | TEI-XML Output | Description |
|------------|----------------|-------------|
| `//` | `<lb/>` or `<lb n="auto"/>` | Line break (auto-numbered if enabled) |
| `//n` | `<lb n="n"/>` | Line break with explicit number |
| `///n` | `<pb n="n"/>` | Page break (n = page number) |
| `.abbr[abbr]{expansion}` | `<choice><abbr>abbr</abbr><expan>expansion</expan></choice>` | Abbreviation |
| `[...]` or `[...n]` | `<gap reason="illegible" quantity="n" unit="chars"/>` | Gap/lacuna |
| `[...<text>]` | `<gap/><supplied>text</supplied>` | Gap with supplied reading |
| `[...n<text>]` | `<gap quantity="n"/><supplied>text</supplied>` | Gap with quantity and supplied |
| `<text>` | `<supplied>text</supplied>` | Supplied/reconstructed text |
| `-{text}-` | `<del>text</del>` | Deletion |
| `+{text}+` | `<add>text</add>` | Addition |
| `^{text}` | `<note>text</note>` | Note/annotation |
| `?{text}?` | `<unclear>text</unclear>` | Unclear reading |
| `:name:` | `&name;` | Entity reference (XML entity) |
| `\|` | (word boundary) | Explicit word boundary |
| `~//` | (continuation + lb) | Word continues across line break |
| `~///n` | (continuation + pb) | Word continues across page break |

**Word Wrapping (when enabled):**
- Words are wrapped in `<w>` tags
- Punctuation (`.,:;!?()[]`) is wrapped in `<pc>` tags
- Each `<w>` and `<pc>` tag is followed by a newline for readability

**Multi-Level Transcription (MENOTA, when enabled):**
- Each `<w>` and `<pc>` contains three nested levels: `<me:facs>`, `<me:dipl>`, `<me:norm>`
- **Facsimile level:** Shows entity references and abbreviated forms
- **Diplomatic level:** Resolves entities to characters, expands abbreviations, removes combining marks
- **Normalized level:** Applies character normalization (e.g., long s → s, ligatures expanded)
- Old Norse characters (ð, þ, æ) are preserved on all levels

**Diplomatic Normalization Rules:**
Entity mappings in `entity-base-letters.json` follow Old Norse diplomatic conventions:
- **Preserve:** Old Norse letters (ð, þ, æ, œ, ø), diacritics (acute, macron, umlaut, ogonek, cedilla, ring)
- **Normalize to acute:** grave → acute, double acute → single acute
- **Strip:** circumflex, tilde, breve, dot above/below, caron
- **Normalize forms:** Special letter forms (long s → s, open o → o, insular/rotunda → base letter)
- **Ligatures:** Meaningful (æ, œ) preserved; orthographic (ff, st) expanded to components

Users can override any mapping via the Entity Browser's custom mapping editor.

## Parsing Pipeline

1. **Lexer** (`lexer.rs`): Tokenizes DSL → AST nodes
2. **Word Tokenizer** (`wordtokenizer.rs`): Groups nodes into `<w>` words and `<pc>` punctuation (when enabled)
3. **Compiler** (`compiler.rs`): AST → TEI-XML with template wrapping, entity resolution, and optional auto line-numbering

## Tauri Commands

| Command | Purpose |
|---------|---------|
| `compile_dsl` | Compile DSL to TEI-XML (with multiLevel and normalizerJson options) |
| `list_templates` / `get_template` / `save_template` | Template CRUD |
| `open_file` / `save_file` / `export_tei` / `load_text_file` | File operations |
| `load_entities` / `get_entity` / `list_entity_names` | Entity system |
| `load_custom_mappings` / `save_entity_mapping` / `remove_entity_mapping` | Custom entity mappings |
| `load_settings` / `save_settings` | Settings persistence |

## Key Files for Common Tasks

- **DSL parsing logic:** `src-tauri/src/parser/lexer.rs`
- **XML generation:** `src-tauri/src/parser/compiler.rs`
- **Word tokenization:** `src-tauri/src/parser/wordtokenizer.rs`
- **AST node types:** `src-tauri/src/parser/ast.rs`
- **Entity definitions:** `static/entities/menota.json`
- **Entity base mappings:** `static/normalizer/entity-base-letters.json`
- **Custom mappings manager:** `src-tauri/src/entities/custom_mappings.rs`
- **Normalizer dictionary:** `static/normalizer/menota-levels.json`
- **Level dictionary logic:** `src-tauri/src/normalizer/dictionary.rs`
- **Mapping generator script:** `scripts/generate-entity-mappings.js`
- **Built-in templates:** `src-tauri/src/template/manager.rs`
- **Main UI:** `src/routes/+page.svelte`
- **Editor component:** `src/lib/components/Editor.svelte`
- **Syntax highlighting:** `src/lib/parser/highlighter.ts`, `src/lib/parser/tei-dsl.grammar`

## Current Implementation Status

**Complete:**
- Full DSL parsing and TEI-XML compilation
- Word tokenization with `<w>` tag wrapping
- Punctuation tokenization with `<pc>` tag wrapping
- Line numbering: manual (`//5`) and automatic (template option)
- Entity system with MENOTA/MUFI support (~1,980 chars) - outputs as `&entity;`
- Template system (TEI P5, MENOTA built-in) with wordWrap, autoLineNumbers, and multiLevel options
- Multi-level MENOTA transcription (`<me:facs>`, `<me:dipl>`, `<me:norm>`)
- Gap with supplied text syntax: `[...<text>]` and `[...n<text>]`
- File I/O (open, save, export)
- Split-pane UI with CodeMirror editor
- Entity browser with search/filter, category selection, and custom mapping editor
- Custom entity mappings with diplomatic normalization defaults (persisted to app data)
- Syntax highlighting (Lezer grammar-based)
- Auto-preview with debounce
- Settings persistence (fontSize, theme, autoPreview, previewDelay, activeTemplateId)

**Not Yet Implemented:**
- User documentation

## Testing

Run Rust tests with:
```bash
cd src-tauri && cargo test
```

51 tests cover the parser, lexer, word tokenizer, compiler, normalizer, and multi-level features.

## Debugging

- **Error Panel:** Click the ☰ button in the editor header to view logs
- **Console Logging:** All major operations log to both browser console and error panel
- **Rust Logging:** Backend uses `log` crate with tauri-plugin-log (debug builds only)

## Fonts

The app uses **Junicode** for rendering MUFI (Medieval Unicode Font Initiative) characters in the entity browser. MENOTA entities use the Unicode Private Use Area, which requires a specialized font.

- **Font:** Junicode by Peter S. Baker
- **License:** SIL Open Font License 1.1 (OFL)
- **Source:** https://github.com/psb1558/Junicode-font
- **Location:** `static/fonts/`

To set up fonts, download Junicode and place the following files in `static/fonts/`:
- `Junicode.woff2` (required - regular weight)
- `JunicodeVF-Italic.woff2` (optional - italic variant)

## Development Commands

```bash
bun install          # Install dependencies
bun run dev          # Run in development mode
bun run build        # Build for production
bun run tauri dev    # Run Tauri app in dev mode
bun run tauri build  # Build Tauri app
```
