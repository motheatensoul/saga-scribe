# To do

Known problems and planned work that isn't scheduled for a version yet. Remove an entry
when it is done (and record it in CHANGELOG.md if users notice).

## Interface

- (PENDING USER TESTING)**Accessibility review, remaining parts.** Done so far (dialog focus, tab keys, names,
  contrast) and the rules that follow are in "Accessibility" in
  `docs/UI_STANDARDS.md`; record new findings there too. Still to do:
  - keyboard: work through the main workflow in the running app with the keyboard alone,
    to confirm what code review and unit tests found (e.g. that focus lands well in each
    dialog and returns to the opener);
  - screen readers: try Orca (Linux), VoiceOver (macOS) and NVDA (Windows) on the main
    workflow (transcribe, open a word, lemmatize, export);
  - zoom and font size: the layout at 200% zoom and with the largest editor font (24 px).

## Document model

- **Document model** (`docs/DOCUMENT_MODEL.md`, design agreed 2026-11-11): the XML is the
  only truth, views are lenses, and a project profile describes the transcription levels.
  Done: steps 1 to 3 (the tree, the projection, edits merged into words and applied to the
  tree, replaced and moved words, older projects brought onto the current rules), step 4
  (the profile, confirmed on import; pinned normalized forms; one lineation for `//n`),
  step 6 (`.teis` 2.0: imported documents saved as `document.xml` with their profile, open
  records and annotation anchors; new documents keep words set in the XML tab), and the
  word panel's XML tab of step 5 (all 2026-11-21). Left:
  - step 4: the profile's abbreviation and punctuation encoding in the compiler (with
    the abbreviation notation, step 7);
  - step 5: levels, ranges and attributes in their own controls in the word panel, and
    resetting a pinned level to its derivation (needs the normalizer outside a compile);
  - a word typed in the shorthand of an imported file can only get its XML set after
    the project is saved and opened again.
  Corpus gates: `scripts/fetch-menota-corpus.sh`, then
  `cd src-tauri; MENOTA_CORPUS=../tests/corpus/menota cargo test --release corpus_ -- --test-threads=1`
  (run them one at a time; together they can run out of memory).
- **Text outside words in documents without `<w>`** is still split at punctuation by the
  shorthand (`Hello,` is one segment, two tokens); in documents with `<w>` such loose text
  is escaped like a word.
- **Without the editor's guidance, the patcher can move tokens.** Comparing whole
  documents, it may pair a run of identical tokens (e.g. `[...5]`) differently, delete
  them and insert them again as new ones, which loses their markup. The app always sends
  guidance, but the patcher falls back to comparing when guidance doesn't fit. Words
  deleted and inserted again with the same text are recognised as moved
  (`Patcher::recognise_moves`) when it is clear which is which (as many deleted as
  inserted, or the guidance says); gaps, punctuation and breaks aren't, and neither is a
  moved word that is also edited before the next compile.
- **A session's tree is large**: about 240 MB for Njáls saga (16 MB, 1.8M nodes) at 112
  bytes a node (was 200), plus each text node's own string and each element's list of
  children. Text could be read from the file's text where it has no references.
- **Compiles don't check their output**: exporting TEI-XML refuses XML libxml2 doesn't
  read as well-formed (`export_tei`), and saving a project warns when its `output.xml`
  isn't, but a compile writes with `Tree::write`, not `write_checked` (parsing the whole
  file again on every edit is too slow), so nothing checks that the written file reads as
  the tree it came from. The tree check could run on the nodes an edit wrote anew.
- **Import of XML that isn't well-formed is only warned about**: `crate::xml::parse` uses
  libxml2's recover mode, so a broken file is imported as libxml2 repairs it, with a
  warning in the log (unedited parts are still copied from the original bytes). Whether
  such a file should be refused instead is open. All 288 Menota files are well-formed.
- **After the document model works:** inferring a changed lemma and analysis after an
  edit (and from them dipl and norm), which needs a robust lemmatizer.

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

  Planned so far:
  - **Abbreviations as `{mark=expansion}`, with a store** (decided; not implemented yet,
    reminded 2026-11-12). `.abbr[kgs]{konungs}` is too heavy for something this common.
    The braces enclose only what is abbreviated, so they can stand inside a word:
    `{:kbar:=konun}gs`, `{:etslash:=ok}`. The left of `=` is the abbreviation mark, wrapped
    in `<am>`; the right is the expansion of exactly that mark, wrapped in `<ex>` (this is
    the MENOTA encoding; documents without `<am>`/`<ex>` need a different approach). MUFI
    sometimes fuses the mark into a letter (`:hbar:` = h + bar, where the bar is the
    mark) and sometimes makes it a separate character. A fused letter's base letter was
    written and isn't part of `<ex>`, so `{:kbar:=konun}gs` gives `<am>&kbar;</am>` on the
    facsimile level and `k<ex>onun</ex>gs` on the diplomatic level. That needs a table of
    each entity's base letter: `static/normalizer/entity-base-letters.json` is an
    unchecked draft, to be replaced by the base characters in MUFI's specification once
    its website is reachable again. Check the output on each level against the MENOTA
    handbook (abbreviations chapter) before implementing.
    `{:kbar:}` takes the expansion from a store in the project file. The store maps a
    mark to one or more expansions, and the first is the default. An explicit expansion
    always overrides the store. A mark that isn't in the store gets a diagnostic with a
    quick fix "add to store", and an explicit form gets an action that adds it. The editor
    shows a short form's expansion (inline hint or hover), so the store isn't hidden state.
    JSON export/import of the store for reuse between manuscripts can come later; hands
    differ too much to make sharing a priority.
  - **Supplied text with markup.** `<…>` and `[...3<ab>]` take shorthand content (done,
    DSL rules 4). Left: `.supplied{…}` is still how supplied text spanning several words
    is written (`<a b>` is one token); retiring it needs `<…>` to hold several words.
  - **Junicode as an optional editor font** (entities are shown as characters since
    0.8.0, set in Junicode).
  - **Cost of the breaking changes:** `syntax.rs`, `tokens.ts`, the grammar, the DSL
    corpus, the help dialog and the user guide change together. The `.teis` format
    version goes up, and older DSL is rewritten when loaded. `DSL_RULES` goes up for
    imported documents. The importer fills the store from `<am>`/`<ex>` pairs and writes
    short forms where the expansion matches the default. Whole-word `<choice>`
    abbreviations without `<am>`/`<ex>` still need a DSL form, perhaps `{kgs=konungs}`
    over the whole word. XML and DSL must still read as
    the same tokens.

## Distribution

- **glib 0.18 (RUSTSEC-2024-0429)**: `cargo audit` warns about unsoundness in
  `glib::VariantStrIter` (fixed in glib 0.20). Tauri 2 (as of 2.12.7: wry, tao, muda,
  webkit2gtk 2.0) and our own Linux code use the GTK 3 bindings, whose last release is
  0.18, so it can't be updated until Tauri moves to GTK 4. Saga-Scribe doesn't use
  `VariantStrIter`.
- **Flatpak remote**: As mentioned in the documentation, flathub distribution, if desired, would require the flatpak manifest and all related files to be untouched by agents. Given the rather specialized use-case for the app (almost 0 usage to someone not working with manuscripts, so basically worthless to genpop), setting up a flatpak remote on the server would probably be best for ensuring updates.
