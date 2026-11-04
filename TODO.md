# To do

Known problems and planned work that isn't scheduled for a version yet. Remove an entry
when it is done (and record it in CHANGELOG.md if users notice).

## Interface

- **Accessibility review, remaining parts.** Done so far (dialog focus, tab keys, names,
  contrast) and the rules that follow are in "Accessibility" in
  `docs/UI_STANDARDS.md`; record new findings there too. Still to do:
  - keyboard: work through the main workflow in the running app with the keyboard alone,
    to confirm what code review and unit tests found (e.g. that focus lands well in each
    dialog and returns to the opener);
  - screen readers: try Orca (Linux), VoiceOver (macOS) and NVDA (Windows) on the main
    workflow (transcribe, open a word, lemmatize, export);
  - zoom and font size: the layout at 200% zoom and with the largest editor font (24 px).

## Distribution

- **Flatpak remote**: Flathub's policy on AI-assisted submissions probably rules it out
  for now, so publishing would need our own repository. Until then the `.flatpak` bundle
  on the releases page is the way to install. Screenshots for the metadata are in
  `docs/screenshots/`; retake them when the interface changes noticeably.
