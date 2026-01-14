# Saga-Scribe

A desktop application for scholarly manuscript transcription, producing TEI-XML with MENOTA (Medieval Nordic Text Archive) extensions.

**Version:** 0.4.0 (alpha)

## Features

- **Custom DSL**: Concise notation for transcription that compiles to TEI-XML
- **Multi-level transcription**: MENOTA-compliant facsimile, diplomatic, and normalized levels
- **Entity browser**: ~1,980 MENOTA/MUFI characters with customizable diplomatic mappings
- **Lemmatization**: ONP dictionary integration with morphological analysis (me:msa)
- **Project archives**: Bundle source, compiled XML, and lemma confirmations in .teis files
- **Real-time preview**: See rendered text, XSLT-transformed output, or raw XML as you type
- **Import support**: Import from TEI-XML or plain text files into DSL format
- **XSLT rendering**: Apply custom XSLT stylesheets to preview XML transformations
- **Page navigation**: Browse large manuscripts by page with instant navigation
- **Collapsible sections**: Fold/unfold pages in the editor at page breaks (`///`)
- **Search & replace**: Find and replace text in the editor (Ctrl+H)
- **Settings menu**: Configure theme, font size, auto-preview, and default template

## Tech Stack

- **Frontend:** Svelte 5, TypeScript, CodeMirror 6
- **Backend:** Rust + Tauri 2.9
- **Package Manager:** bun

## DSL Syntax Reference

| DSL Syntax               | TEI-XML Output                                               | Description                           |
| ------------------------ | ------------------------------------------------------------ | ------------------------------------- |
| `//`                     | `<lb/>` or `<lb n="auto"/>`                                  | Line break (auto-numbered if enabled) |
| `//n`                    | `<lb n="n"/>`                                                | Line break with explicit number       |
| `///n`                   | `<pb n="n"/>`                                                | Page break (n = page number)          |
| `.abbr[abbr]{expansion}` | `<choice><abbr>abbr</abbr><expan>expansion</expan></choice>` | Abbreviation                          |
| `[...]` or `[...n]`      | `<gap reason="illegible" quantity="n" unit="chars"/>`        | Gap/lacuna                            |
| `[...<text>]`            | `<gap/><supplied>text</supplied>`                            | Gap with supplied reading             |
| `[...n<text>]`           | `<gap quantity="n"/><supplied>text</supplied>`               | Gap with quantity and supplied        |
| `<text>`                 | `<supplied>text</supplied>`                                  | Supplied/reconstructed text           |
| `-{text}-`               | `<del>text</del>`                                            | Deletion                              |
| `+{text}+`               | `<add>text</add>`                                            | Addition                              |
| `^{text}`                | `<note>text</note>`                                          | Note/annotation                       |
| `?{text}?`               | `<unclear>text</unclear>`                                    | Unclear reading                       |
| `:name:`                 | `&name;`                                                     | Entity reference (XML entity)         |
| `\|`                     | (word boundary)                                              | Explicit word boundary                |
| `~//`                    | (continuation + lb)                                          | Word continues across line break      |
| `~///n`                  | (continuation + pb)                                          | Word continues across page break      |

## Installation

As the project is currently in alpha, you'll need to build from source:

```bash
git clone https://github.com/motheatensoul/saga-scribe
cd saga-scribe
bun install
bun run tauri dev    # Development mode
bun run tauri build  # Production build
```

## Keyboard Shortcuts

| Shortcut     | Action                         |
| ------------ | ------------------------------ |
| Ctrl+S       | Save project (.teis archive)   |
| Ctrl+O       | Open project or DSL file       |
| Ctrl+F       | Find in editor                 |
| Ctrl+H       | Find and replace               |
| Ctrl+Z       | Undo (editor)                  |
| Ctrl+Y       | Redo (editor)                  |
| Ctrl+Shift+[ | Fold section at cursor         |
| Ctrl+Shift+] | Unfold section at cursor       |
| Ctrl+Shift+Z | Undo lemmatization             |
| Ctrl+Shift+Y | Redo lemmatization             |
| F1           | Open help dialog               |
| Ctrl+/       | Open help dialog (alternative) |

The toolbar provides access to:

- **Settings** (⚙️ icon): Appearance, editor, and template preferences
- **Help** (? icon): Keyboard shortcuts, DSL syntax reference, and about information

## Documentation

See `docs/user-guide.md` for detailed usage instructions including:

- DSL syntax reference
- Settings and preferences
- Help system and keyboard shortcuts
- Entity browser and custom mappings
- Lemmatization workflow
- Template configuration
- Import from TEI-XML and plain text
- XSLT preview mode
- Page navigation for large manuscripts

## License

This project is licensed under [GPL-v3-or-later](LICENSE).

See [LICENSES](static/fonts/LICENSES.md) for further licenses of used fonts.

## AI Assistance

This project is developed with assistance from [Claude Code](https://claude.com/claude-code). AI-assisted commits are attributed with the `Co-Authored-By: Claude` tag.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to get started, and our [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Made with love in Norway.
