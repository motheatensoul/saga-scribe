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

- **Nix flake for the development environment**: bun, the pinned Rust toolchain
  (`rust-toolchain.toml`), the Tauri system libraries (WebKitGTK 4.1, librsvg,
  libappindicator) and libxml2, so `nix develop` gives a working `bun run tauri dev`.
  The README mentions it as planned.

## Distribution

- **Flatpak remote**: Flathub's policy on AI-assisted submissions probably rules it out
  for now, so publishing would need our own repository. Until then the `.flatpak` bundle
  on the releases page is the way to install. Screenshots for the metadata are in
  `docs/screenshots/`; retake them when the interface changes noticeably.
