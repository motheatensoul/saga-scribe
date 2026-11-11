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

## Document model

- **Edits lose markup the shorthand can't show** (`<ex>` in dipl-only files, `<supplied>`
  inside a level, gap attributes, comments, levels the file doesn't have; numbers in the
  survey below). The plan is a new document model with the XML as the only truth, views
  as lenses, and a project profile for transcription levels: `docs/DOCUMENT_MODEL.md`
  (design agreed 2026-11-11; step 1, the tree, is done; next: step 2, projecting the tree
  to shorthand). It comes before the shorthand redesign.
- **After the document model works:** a button that opens the word panel on the next word
  marked for review; inferring a changed lemma and analysis after an edit (and from them
  dipl and norm), which needs a robust lemmatizer.

## Shorthand (DSL)

- **Design pass for legibility.** Frequent constructs should be short, and `.tag{…}`
  should remain only for rare ones. Breaking changes are acceptable. Frequency varies a lot
  between manuscripts and editors (e.g. supplied text), so counts across a corpus inform
  the design but don't rank editorial constructs by importance.

  **Survey (2026-11-09)** of the whole Menota catalogue: 91 texts (2.5M words) and 196
  charters (`https://clarino.uib.no/korpuskel-api/download-zip?corpus=menota`, and
  `corpus=menota-diploma`; the catalogue's "Download all XML files"). Counts per 1,000
  words of generated DSL, per file (median / 90th percentile / max), for the texts:
  entities 556 / 821 / 1383; `.abbr[]{}` 174 / 437 / 462; `.norm{}` 46 / 88 / 130 (almost
  all punctuation found only on the normalized level, `.norm{,}` and `.norm{.}`); `~` line-end
  continuation 17 / 29 / 63 and compound join 8 / 20 / 39; `.p` 4 / 21 / 50; gaps 2 / 91 /
  322; notes 1 / 21 / 108; unclear 2 / 24 / 139. The rest depends even more on the
  manuscript and the editor; per-file tables and the generated DSL are easy to regenerate
  from the archive. Charters are mostly dipl-only (136 of 196), with few `.abbr` but 15,760
  `<ex>`. Findings for the design:
  - Entities are most of the markup (in facs-level files, 3–7 times as many characters as
    the text itself): showing them as glyphs is the biggest gain in legibility.
  - `.norm{,}`/`.norm{.}` is frequent and has no short form in the plan yet.
  - Abbreviations come in more shapes than `.abbr` shows: no `<am>` on the facs level
    (`<me:facs>&hstrok;</me:facs>` with `<me:dipl>h<ex>ann</ex></me:dipl>`, which the
    fused-letter rule must import), `<am>` without `<ex>` (Holm A 49, facs only), and
    `<ex>` without `<am>` (dipl-only files).
  - **Markup the DSL can't show is lost when a word is edited** (not yet fixed; it needs
    the new syntax, per "give it DSL syntax rather than special-casing the patcher"):
    - `<ex>` in dipl-only files: `hæn<ex>n</ex>aR` reads `hænnaR` in the DSL, and after an
      edit the word is `<w>hænnaRR</w>`, without its `<ex>` and its `<me:dipl>`. Needs an
      expansion without a mark, e.g. `hæn{=n}aR`. 535k `<ex>` in the texts' XML against
      300k `.abbr` in their DSL.
    - `<supplied>` inside a level: of 11,764 `<supplied>` in the texts, about 2,200 appear
      in the DSL. AM 242 fol, an initial left for the rubricator: facs `<gap extent="1"
      unit="initial"/>erollꝺin`, dipl `<supplied …>V</supplied>erollꝺin`. After an edit, the
      supplied `V` is gone, the gap became `<gap reason="illegible"/>`, and the word got a
      `<me:norm>` level the file doesn't have. Restorations spread over several words and
      lines (`rendition="first|middle|last"`, NRA fragments) need the same.
  - Files with two lineations (`<lb ed="ms">` and `<lb ed="O.A.Johnsen">`) show both as
    `//n`, so the line numbers in the DSL interleave.

  Planned so far:
  - **Abbreviations as mark = expansion, with a store.** `{…}` encloses only what is
    abbreviated, so it can stand inside a word: `{:kbar:=konun}gs`, `{:etslash:=ok}`. The
    left of `=` is the abbreviation mark (`<am>`), the right is the expanded text (`<ex>`).
    This replaces `.abbr[kgs]{konungs}`, which covers whole words. MUFI sometimes fuses the
    mark into a letter (`:kbar:` = k with the bar as the mark) and sometimes makes it a
    separate character. When the expansion begins with a fused letter's base letter
    (`static/normalizer/entity-base-letters.json`, still a draft to review), that letter
    was written and isn't part of `<ex>`. So `{:kbar:=konun}gs` gives `<am>&kbar;</am>` on
    the facsimile level and `k<ex>onun</ex>gs` on the diplomatic level. Check the output on
    each level against the MENOTA handbook (abbreviations chapter) before implementing.
    `{:kbar:}` takes the expansion from a store in the project file. The store maps a
    mark to one or more expansions, and the first is the default. An explicit expansion
    always overrides the store. A mark that isn't in the store gets a diagnostic with a
    quick fix "add to store", and an explicit form gets an action that adds it. The editor
    shows a short form's expansion (inline hint or hover), so the store isn't hidden state.
    JSON export/import of the store for reuse between manuscripts can come later; hands
    differ too much to make sharing a priority.
  - **Supplied text with markup.** `<…>` takes shorthand content (entities,
    abbreviations, line breaks), which makes `.supplied{…}` unnecessary. The same applies
    to gap-with-supplied `[...3<ab>]`.
  - **Entities shown as characters.** A visible toggle in the editor header, next to
    undo/redo, chooses between two modes. In one, a well-formed, known `:name:` is shown as its glyph, and the source
    reappears when the text cursor (not the mouse) enters or touches it. In the other,
    the source is always shown. Glyphs are set in Junicode at regular weight, with
    `font-size-adjust` to match the monospace x-height, and a faint background or
    underline marks them, so they look different from characters typed directly. Unknown
    entities stay as source and get a warning. Backspace deletes a whole entity, and
    search and copy work on the source. Only the visible ranges are decorated (a
    ViewPlugin), so typing stays fast. Possibly also: Junicode as an optional editor font.
  - **Cost of the breaking changes:** `syntax.rs`, `tokens.ts`, the grammar, the DSL
    corpus, the help dialog and the user guide change together. The `.teis` format
    version goes up, and older DSL is rewritten when loaded. `DSL_RULES` goes up for
    imported documents. The importer fills the store from `<am>`/`<ex>` pairs and writes
    short forms where the expansion matches the default. Whole-word `<choice>`
    abbreviations without `<am>`/`<ex>` still need a DSL form, perhaps `{kgs=konungs}`
    over the whole word. XML and DSL must still read as
    the same tokens.

## Distribution

- **Flatpak remote**: Flathub's policy on AI-assisted submissions probably rules it out
  for now, so publishing would need our own repository. Until then the `.flatpak` bundle
  on the releases page is the way to install. Screenshots for the metadata are in
  `docs/screenshots/`; retake them when the interface changes noticeably.
