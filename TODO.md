# To do

Known problems and planned work that isn't scheduled for a version yet. Remove an entry
when it is done (and record it in CHANGELOG.md if users notice).

## Interface

- **Accessibility review** beyond Svelte's compile-time a11y checks (which pass):
  - keyboard: every action reachable and visible focus everywhere, including the word
    panel, entity browser, metadata editor, template manager and the export menu; focus
    moves into dialogs when they open and back when they close;
  - screen readers: try Orca (Linux), VoiceOver (macOS) and NVDA (Windows) on the main
    workflow (transcribe, open a word, lemmatize, export); icon-only buttons need
    accessible names, not just `title` (done for the toolbar and editor header, not yet
    elsewhere, e.g. the preview and the dialogs);
  - contrast: check both themes (caramellatte, coffee) against WCAG AA, e.g. disabled
    toolbar buttons and the faint marks for suggested lemmas;
  - zoom and font size: the layout at 200% zoom and with the largest editor font.
  Record what's checked and the rules that follow from it in `docs/UI_STANDARDS.md`.

## Distribution

- **Flatpak remote**: Flathub's policy on AI-assisted submissions probably rules it out
  for now, so publishing would need our own repository. Until then the `.flatpak` bundle
  on the releases page is the way to install. Screenshots for the metadata are in
  `docs/screenshots/`; retake them when the interface changes noticeably.
