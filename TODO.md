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

- **Edits lose markup the shorthand can't show** (`<ex>` in dipl-only files, `<supplied>`
  inside a level, gap attributes, comments, levels the file doesn't have; numbers in the
  survey below). The plan is a new document model with the XML as the only truth, views
  as lenses, and a project profile for transcription levels: `docs/DOCUMENT_MODEL.md`
  (design agreed 2026-11-11). Done: steps 1, 2 and 3a (an edited word keeps the markup
  the shorthand doesn't show), and the shorthand of all 288 Menota files reads as their
  words token for token (DSL rules 3 and 4, 2026-11-12), which 3b needs. The 3b gates
  (GetPut, PutGet, Locality: `corpus_edits_are_put_back_locally`) pass, and edits are
  applied to the file's tree instead of spliced into its text (2026-11-12). Replaced and deleted
  words are asked about in the editor (first version, 2026-11-12). **Next in 3b:** the
  patcher pairing tokens with tree nodes instead of segments; the rest of the records
  (listed in the design under 3b: word panel, previews, Validate, count on saving,
  annotations, moved words). The look of the gutter mark is to be checked with testers. It comes before the
  shorthand redesign. Corpus gates: `scripts/fetch-menota-corpus.sh`, then
  `cd src-tauri; MENOTA_CORPUS=../tests/corpus/menota cargo test --release corpus_ -- --test-threads=1`
  (run them one at a time; together they can run out of memory).
- **Text outside words in documents without `<w>`** is still split at punctuation by the
  shorthand (`Hello,` is one segment, two tokens); in documents with `<w>` such loose text
  is escaped like a word.
- **Markup inside `-{…}-`, `+{…}+` and `?{…}?` is written as text**: their content isn't
  read as shorthand, so `<del>ny<gap/></del>` is `-{ny[...]}-`, compiled as the text
  `[...]`; editing such a word moves the gap out of the `<del>` (Holm D 4). Like `<…>`, their
  content should be shorthand.
- **An edited `<num>` is compiled anew**, not merged like a `<w>` (`document/merge.rs`),
  so its attributes and markup the shorthand doesn't show are lost.
- **Without the editor's guidance, the patcher can move words.** Comparing whole
  documents, it may pair a run of identical tokens (e.g. `[...5]`) differently, delete
  words and insert them again as new ones, which loses their markup. The app always sends
  guidance, but the patcher falls back to comparing when guidance doesn't fit. Step 3b's
  removed and replaced records cover this.
- **An imported session holds the file twice**: as segments (each word's XML as a
  string) and as the tree edits are applied to. For AM 28 8vo (7.5 MB, 427k nodes) the
  tree alone is well over 100 MB, and the memory slows the patcher down: an edit there
  compiles in about 29 ms instead of 20 before the tree. Goes away when the patcher works
  on the tree (3b); the nodes (200 bytes each) could also be made smaller.
- **Output isn't checked by libxml2 before saving**: `Tree::write_checked` exists, but a
  compile writes with `write`, since parsing the whole file again on every edit is too slow.
  The check should run when saving or exporting, or only on the nodes an edit wrote anew.
- **The compiler writes entities inside `?{…}?` within `.abbr[…]` literally**:
  `.abbr[?{þ:er:}?]{…}` gives `<unclear>þ:er:</unclear>`, not `&er;`.
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
  - **Supplied text with markup.** `<…>` and `[...3<ab>]` take shorthand content (done,
    DSL rules 4). Left: `.supplied{…}` is still how supplied text spanning several words
    is written (`<a b>` is one token); retiring it needs `<…>` to hold several words.
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

- **Flatpak remote**: As mentioned in the documentation, flathub distribution, if desired, would require the flatpak manifest and all related files to be untouched by agents. Given the rather specialized use-case for the app (almost 0 usage to someone not working with manuscripts, so basically worthless to genpop), setting up a flatpak remote on the server would probably be best for ensuring updates.
