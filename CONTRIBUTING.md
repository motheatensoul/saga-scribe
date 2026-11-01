# Contributing to Saga-Scribe

Thank you for your interest in contributing to Saga-Scribe. Bug reports, feature requests
and documentation fixes are as welcome as code: please use the
[issue templates](https://github.com/motheatensoul/saga-scribe/issues/new/choose).
Security problems are reported privately, as described in [SECURITY.md](SECURITY.md).

## Setting up

The prerequisites (bun, Rust, the system libraries Tauri and libxml2 need) and the build
commands are in the README under [Building from source](README.md#building-from-source).
In short:

```bash
git clone https://github.com/motheatensoul/saga-scribe
cd saga-scribe
bun install
bun run tauri dev
```

The Rust toolchain is pinned in `rust-toolchain.toml`; rustup installs it automatically.
With Nix, `nix develop` provides all of the prerequisites instead (`flake.nix`, which reads
the toolchain from the same file). When a build prerequisite changes, update the flake along
with CI and the README.

## Where things are

```
saga-scribe/
├── src/                      # Frontend (Svelte 5 + TypeScript)
│   ├── lib/
│   │   ├── components/       # UI components
│   │   ├── editor/           # Editor support (theme, edit history, typing)
│   │   ├── parser/           # Editor highlighting grammar (Lezer)
│   │   ├── rendered/         # Rendered-text preview
│   │   ├── stores/           # Svelte stores
│   │   ├── utils/            # XSLT, export, PDF pagination, …
│   │   └── tauri.ts          # Calls into the backend
│   └── routes/               # Main window and the print window
├── src-tauri/src/            # Backend (Rust)
│   ├── parser/               # Shorthand compiler (lexer, AST, word tokenizer)
│   ├── importer/             # TEI-XML and text import, round-trip patching
│   ├── commands/             # Tauri commands
│   ├── annotations/, metadata/, dictionary/, entities/, normalizer/,
│   │   settings/, stylesheet/, template/, validator/
│   └── identity.rs           # Which word is which across edits
├── static/                   # Fonts, entity and dictionary data, schemas, XSLT
├── docs/                     # User guide, UI standards
├── flatpak/                  # Flatpak manifest and AppStream metadata
└── tests/fixtures/           # Shared test inputs (e.g. the shorthand corpus)
```

## Making changes

[AGENTS.md](AGENTS.md) is the reference for conventions, for human contributors as much as
for AI agents: code style, accessibility, the async patterns that keep large manuscripts
responsive, the round-trip guarantees for imported files, and how releases and versions
work. [docs/UI_STANDARDS.md](docs/UI_STANDARDS.md) describes the look of the interface.

Before opening a pull request:

```bash
bun run test        # frontend (Vitest) and Rust tests
bun run check:all   # versions, svelte-check (0 warnings) and cargo clippy
```

- Record user-visible changes in [CHANGELOG.md](CHANGELOG.md), under the unreleased version.
- Update the [user guide](docs/user-guide.md) when you change what users see or do, and the
  [README](README.md) when you change a feature it describes or how the app is built.
- If you change the shorthand syntax, change the editor's grammar with it and regenerate
  the shared corpus (see "Key Files" and "Testing" in AGENTS.md).
- Branches are named after the version they work towards, e.g. `dev/0.8.0` or
  `dev/0.8.0-<topic>`.

## License

By contributing, you agree that your contributions will be licensed under the
[GPL-3.0-or-later](LICENSE).
