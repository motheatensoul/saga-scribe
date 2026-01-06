# TEI Scribe

## Overview

TEI-Scribe is a Tauri desktop application for manuscript transcription using a custom DSL that compiles to TEI-XML, with specific support for MENOTA (Medieval Nordic Text Archive) extensions.


## Tech Stack

- **Frontend:** Svelte 5 + SvelteKit, TypeScript, CodeMirror 6, svelte-splitpanes
- **Backend:** Rust + Tauri 2.9

The project uses a custom domain-specific language as shorthand, which is compiled into TEI-compliant XML.

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

## Building

As the project is currently in development, the main way of running the app is building the app yourself:

`$ git clone https://github.com/motheatensoul/tei-scribe`

and

`$ bun run tauri:dev`

## License

This project is licensed under [GPL-v3-or-later](LICENSE).

See [LICENSES](static/fonts/LICENSES.md) for further licenses of used fonts.

## AI Assistance

This project is developed with assistance from [Claude Code](https://claude.com/claude-code). AI-assisted commits are attributed with the `Co-Authored-By: Claude` tag.

Made with love in Norway.
