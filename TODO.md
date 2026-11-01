# To do

Known problems and planned work that isn't scheduled for a version yet. Remove an entry
when it is done (and record it in CHANGELOG.md if users notice).

## Bugs

- **Ctrl+Shift+Z in the editor does two things.** CodeMirror's history keymap binds it to
  redo, and the window's shortcut handler (`handleKeydown` in `src/routes/+page.svelte`)
  binds it to "undo lemmatization" without checking whether the editor already handled the
  key (`event.defaultPrevented`). Pressed in the editor, it redoes an editor change *and*
  undoes a lemma. Decide which one owns the shortcut there (and likewise Ctrl+Shift+Y),
  then update the Help dialog and `docs/user-guide.md`.

## Development

- **Nix flake for the development environment**, so that building doesn't depend on one
  maintainer's machine. Mainly pins bun and the Rust toolchain (read from
  `rust-toolchain.toml`, e.g. with rust-overlay or fenix, so the two can't drift), plus the
  Tauri system libraries (WebKitGTK 4.1, librsvg, libappindicator) and libxml2: `nix develop`
  should give a working `bun run tauri dev` and `bun run check:all`. Mention it in the
  README's build section (currently "planned") and in CONTRIBUTING.md once it exists.

## Features

- **System notifications for long-running work**, such as a PDF or HTML export or an
  import finishing, or failing, while the window is in the background. Use each platform's
  own mechanism: the freedesktop notification service over D-Bus
  (`org.freedesktop.Notifications`) on Linux, through the notification portal inside the
  Flatpak (check whether the manifest's `finish-args` need a permission); the
  UserNotifications framework on macOS; toast notifications on Windows.
  `tauri-plugin-notification` covers all three, but weigh it against the
  keep-dependencies-minimal rule. Only notify when the window isn't focused, and keep the
  in-app message (log and progress overlay) as it is.

## Interface

- **Structure for toolbar and header actions.** The toolbar's actions (Open, Import, Save,
  Export, template, lemma undo/redo, help, settings) and the editor header's icon buttons
  are laid out one by one. Group them into containers (e.g. file actions, document
  actions, application) with defined behaviour when the window is narrow (wrapping,
  overflow menu), so that a new action has an obvious place and the layout holds up.
  Document the groups in `docs/UI_STANDARDS.md`.
- **Accessibility review** beyond Svelte's compile-time a11y checks (which pass):
  - keyboard: every action reachable and visible focus everywhere, including the word
    panel, entity browser, metadata editor, template manager and the export menu; focus
    moves into dialogs when they open and back when they close;
  - screen readers: try Orca (Linux), VoiceOver (macOS) and NVDA (Windows) on the main
    workflow (transcribe, open a word, lemmatize, export); icon-only buttons need
    accessible names, not just `title`;
  - contrast: check both themes (caramellatte, coffee) against WCAG AA, e.g. disabled
    toolbar buttons and the faint marks for suggested lemmas;
  - zoom and font size: the layout at 200% zoom and with the largest editor font.
  Record what's checked and the rules that follow from it in `docs/UI_STANDARDS.md`.

## Distribution

- **Flatpak remote**: Flathub's policy on AI-assisted submissions probably rules it out
  for now, so publishing would need our own repository. Until then the `.flatpak` bundle
  on the releases page is the way to install. Screenshots for the metadata are in
  `docs/screenshots/`; retake them when the interface changes noticeably.
