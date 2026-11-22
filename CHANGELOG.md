# Changelog

All notable changes to Saga-Scribe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.0] - Unreleased

### Added

- **System notifications**: when an import or an HTML or PDF export finishes or fails while the window is in the background, the system shows a notification (the freedesktop notification service on Linux, including the Flatpak; toast notifications on Windows; Notification Center on macOS)
- **Shorthand for more TEI elements**: any element can be written by name, `.q{Ok sagði hann}`, its content shorthand; between words it goes around them (each still a word), attached to letters it is inside the word. Short forms: `*{…}*` (`<hi>`), `&{…}` (`<seg>`), `${…}` (`<surplus>`), `.psname{…}`, `.fname{…}`, `.plname{…}` (`<persName>`, `<forename>`, `<placeName>`), `.corr{sic=corr}` and `.trans{orig=reg}` (pairs in a `<choice>`), `.rdg[witness]{…}`, `.space[n]`, `.hs{hand}` (`<handShift>`) and `///c=B` (`<cb>`). In imported files, markup around words is shown (DSL rules 10): typed, it wraps the words in a new element; deleted, its words stay; renamed, its words and attributes go into an element of the new name. Projects imported before get it in their shorthand when opened. Markup inside words of imported files, column breaks and changes of hand aren't shown yet, and the last two can't be added to them yet. Projects saved before, where such text was punctuation or plain text (`.k{:bar:=onun}gr`, `*{`), have it escaped when opened, so they read as before (project format 2.1)
- **Nix development environment**: `nix develop` provides bun, the pinned Rust toolchain and the system libraries for building
- **Numerals and escaped characters in the shorthand**: `#.x.#` is a numeral (`<num>`), a token of its own whose dots belong to it. A backslash makes the next character plain text, part of the word (`ꝼæ\;ðꝛ`, `\(anno\)`); right after punctuation it continues the punctuation (`,\,` is one `<pc>`). Projects saved before have their `\` and `#` escaped when opened, so they read as before (project format 1.7)
- **Any line or page break number**: a number that isn't just digits (line breaks) or contains a space (page breaks) is written in braces, `//{56r}`, `///{fol. 1r}`
- **Replaced words are asked about** (imported files): when a word with a lemma or analysis is replaced by another in the editor, it gets a wavy underline in the editor and the previews, and the word panel asks whether to **Keep the old lemma** (the same word, corrected) or **Set a new lemma** (a different word: lemma, analysis and normalized form are removed). Choosing a lemma there answers it too, and the panel shows the replaced word's XML as the file has it. A replaced word without a lemma is asked about too if it has annotations of its own, which stay with it either way. **F8** / **Shift+F8** open the word panel on the next / previous word marked for review and show it in the preview. Until answered, the file follows the suggested answer (the old lemma is kept if the forms are alike). The answers are saved with the project (project format 1.8); **Validate** lists the words still to be decided, and saving or exporting says how many there are. A word typed in place of another now keeps the transcription markup that follows its text, like an edited one
- **The imported file's encoding is read and shown** (step 4 of the document model): after importing a TEI file, a dialog shows its transcription levels with how many words have each, whether they are in `<choice>`, how abbreviations and punctuation are encoded, its lineations and the markup the shorthand doesn't show. Words added in the editor get the file's levels (a diplomatic-only charter gets `<w><me:dipl>…</me:dipl></w>`, not the three MENOTA levels), which can be changed there; the choice is saved with the project
- **A curated normalized form that an edit may have outdated is asked about** (imported files): when a word is edited, its normalized form follows the edit if it is the form Saga-Scribe derives from the shorthand; one that differs (chosen by an editor) is kept, and if the edit changes the word so that it may no longer fit, the word is marked for review like a replaced word, and the word panel says which form was kept
- **One lineation in the shorthand**: in files that number their lines in more than one way (such as the Diplomatarium Norvegicum charters, with `<lb ed="ms">` and `<lb ed="DN">`), `//n` shows only one of them, the manuscript's by default, chosen under **Lineations** in the import dialog; the others are kept in the file. Their line numbers used to interleave in the shorthand. Projects imported before are converted when opened (DSL rules 6)
- **The word's XML in the word panel**: an **XML** tab shows a word as the file has it, with its levels, attributes and markup, and lets its XML be edited and applied; the shorthand in the editor follows. Markup the shorthand can't show can be corrected there without leaving the app. The tab has controls for the word's attributes and each level's content. In a new document, the word keeps that XML: later edits of it are merged in, and its lemma, analysis and notes still follow the annotations (saved with the project, `overrides.json`)
- **Annotations in imported files' XML**: notes, categories and observations, and character annotations are written into an imported file's words as in a new document (`<note>`, `ana` and `cert`, `<c>`), so the exported TEI-XML (and the project's, which is the same) is a complete edition. They used to be kept in the project only. Words without annotations stay as the file has them
- **Abbreviations as `{mark=expansion}`** (step 7 of the document model): the braces hold only what is abbreviated, inside the word, and compile as MENOTA encodes them: `k{:bar:=onun}gs` is `k<am>&bar;</am>gs` on the facsimile level, `k<ex>onun</ex>gs` on the diplomatic one and `konungs` on the normalized one; for a letter with a fused mark, `{:kbar:=konun}gs`, the base letter (from MUFI's character list, `static/characters`) stays outside `<ex>`; `hæn{=n}aR` is an expansion without a mark. A brace as text is now written `\{`. `.abbr[…]{…}` still works. Imported files have their `<am>`/`<ex>` written this way where they stand (DSL rules 7; projects imported before are converted when opened); in the facsimile text, also where the diplomatic level has other letterforms (`ꝼ`/`f`, DSL rules 9); only a word whose marks and expansions don't pair up letter for letter stays `.abbr[…]{…}`. **The abbreviation store**: `{:kbar:}gs` takes the mark's default expansion from the project's store (`abbreviations.json`). Importing fills it from the file, each mark with its expansions, the commonest first, and writes `{mark}` where a mark has that one (DSL rules 8). A `{mark}` not in the store is flagged; a written-out form for a new mark offers to add it; resting the pointer on `{mark}` shows its expansion; **Abbreviations** in the toolbar lists the store to change defaults, remove and add expansions. In files that encode abbreviations as `<choice><abbr><expan>`, words without levels get that form; in files whose punctuation is `<me:punct>`, new and edited punctuation is written in it (it used to become `<pc>`)
- **From the shorthand** next to the word panel's normalized form (imported files): puts in the form the shorthand gives the word, to replace a curated normalized form an edit has made out of date
- **Moved words** (imported files): a word cut and pasted elsewhere in the editor is moved as the file has it, with its markup, lemma, analysis and annotations. It used to be deleted and written anew from the shorthand at its new place, without them
- **Shorthand in supplied text**: `<…>` and `[...3<…>]` take shorthand, such as entities, abbreviations and line breaks (`kæn<n~//14:thorn:>a`), and stay part of the word they are in. Entities in supplied text used to be written literally
- **Shorthand in deletions, additions and unclear readings**: `-{…}-`, `+{…}+` and `?{…}?` take shorthand too (`þ?{:er:}?`, `-{ny[...]}-`). Entities, gaps and line breaks in them used to be written as text (`<unclear>þ:er:</unclear>` instead of `<unclear>þ&er;</unclear>`), and editing an imported word with a gap at the end of a deletion moved the gap out of it
- **Entities shown as characters**: the editor shows known entities as their characters (`:thorn:` as þ), in Junicode on a faint background, and shows the `:name:` again where the cursor touches or enters one; Backspace and Delete next to one delete the whole entity. Search, copying and the compiled XML still work with the names. **Show entities as characters** (þ) in the editor header switches it off and on; it is on by default and kept between sessions

### Changed

- **Project format 2.0** (step 6 of the document model): an imported document is saved as the file now is (`document.xml`), with how it encodes its transcription (`profile.json`) and the replaced words still to be decided (`records.json`), instead of the original file in three parts, a segment manifest and the compiled output. Opening a project compares the stored shorthand with the file and rewrites what doesn't read as it. Annotated words are saved with a fingerprint, so annotations follow their words if the file was changed outside Saga-Scribe, and those whose words are gone are reported. Projects from earlier versions open as before and are saved in the new format; Saga-Scribe 0.7 can't open projects saved by 0.8
- **Dependencies updated**: Tauri 2.12.7 (from 2.9), SvelteKit 3.1.1 and the other Rust and JavaScript dependencies to their latest compatible versions
- **Toolbar grouped by what the actions apply to**: file (Open, Import, Save, Export), document (template, template manager, **Metadata**, **Validate**), lemmatization (undo, redo) and application (log, help, settings). The template manager, metadata editor, validation and log moved there from the editor header, which now has only the actions on the text (undo, redo, search, entities). In a narrow window the toolbar shows icons only and wraps whole groups onto a second row
- **Large imported files take less memory and don't stall the first edit**: the file is read into its tree when it is opened rather than at the first keystroke, which took about a second for a long saga, and the tree takes about 40% less memory (240 MB instead of 390 for Njáls saga in AM 132 fol). The words' XML is no longer kept a second time while editing, and the window no longer holds a copy of the whole imported document (tens of megabytes for a long saga) or passes it back and forth when a file is imported, opened or saved; opening a project no longer blocks the window while it is read
- **Light theme without black buttons**: buttons, the active tab, focus outlines and other accents use a dark coffee brown instead of black, and turn a lighter brown when pointed at

### Fixed

- **Word numbers in unedited imported files with `<w` in a comment** (AM 619 4to): until the first edit, words were counted in the text, so a `<w` inside a comment counted as a word and lemmas or annotations could land on the wrong word. They are now counted in the file's tree, as after an edit
- **Projects imported with older versions read their shorthand with the old rules**: a project opened now works on its file as this version imports it. Unedited words of the shorthand are rewritten to the current form (such as `þꝩi|at` for two words written together, or `//{56r}`), edited words and the layout stay as typed, and the log says how many places were rewritten. Before, such projects kept the old forms, where some words read as one or ran into a page number
- **Importing XML that isn't well-formed said nothing**: such a file was imported as the XML parser repaired it, silently. The log now shows a warning with the parser's first error, so the repaired place can be checked. Likewise, saving a project warns if its compiled TEI-XML (`output.xml`) isn't well-formed
- **Export could write XML that isn't well-formed**: the TEI-XML export is now read by libxml2 first and refused, with an error, if it isn't well-formed
- **ONP definitions failed to load** ("Failed to parse response"): the ONP API no longer sends two fields of each sense that the app required, though it never showed them. Entries now load with whatever fields the API sends
- **Imported files whose shorthand didn't read as their words**: in 194 of the 288 Menota texts and charters, elements written together in the XML ran together in the shorthand (`skrims^{sic}` for a word and the note after it, `þꝩiat` for two words) and words containing punctuation fell apart (`.x.`, `iðran;`), so an edit could be paired with the wrong word and rewrite words that weren't edited. The shorthand now keeps such neighbours apart (`skrims ^{sic}`, `þꝩi|at`), escapes characters that would split a word (`iðran\;`) and writes numerals as `#.x.#`. Now all 288 do. Files imported before keep their shorthand
- **Editing a word in an imported file lost markup the shorthand doesn't show**: the word was written anew from its shorthand, which dropped expansions in diplomatic-only transcriptions (`hæn<ex>n</ex>aR`), supplied text and other elements inside a level, a gap's attributes (`extent`, `unit`), comments, and could add a normalized level the file doesn't have. The edit is now merged into the word as written: only what was edited changes, markup the shorthand doesn't show moves with the text, and a level is kept as it was where the edit can't be placed in it (such as a curated normalized form). Letters typed at the start or end of a word go next to its text, not after the line breaks and indentation some files have inside a level (`<me:dipl> <ex>ok</ex> </me:dipl>`), where the word read back as two
- **Editing a numeral in an imported file lost its markup**: an edited `#…#` was written anew, without its attributes (`value`) and markup the shorthand doesn't show. It is now merged like an edited word
- **Deleting a word in an imported file could leave an empty element**: a numeral written as a word (`<num><w>…</w></num>`) left an empty `<num>` behind, which reads as an empty numeral `##`. Elements that held only the deleted word are now removed with it (paragraphs and divisions excepted)
- **Keyboard use of dialogs and panels**: focus moves into a dialog when it opens (to the search field in the word panel and the entity browser), Tab stays within it, and focus returns to where it was when it closes. Tab bars (preview, word panel, help) switch with the arrow keys. A focused word in the preview is outlined; on lemmatized or annotated words the focus wasn't visible at all. The entity browser's edit and delete buttons show when reached with the keyboard, not only on hover
- **Screen readers**: the log, validation, entity browser and word panel are announced as dialogs with their titles; icon-only buttons (page navigation, removing entries in the metadata editor, entity and template actions) and the metadata editor's fields have names that say what they act on, e.g. "Remove editor 2"
- **Contrast**: secondary text (hints, word numbers, timestamps, the file name in the toolbar, words with a suggested lemma) was too faint to read comfortably in both themes, and red and yellow text was hard to read in the light theme. All text now meets WCAG AA (4.5:1): errors and warnings are shown in the normal text colour with a red or yellow icon beside them, the log's error and warning counts likewise, and delete buttons are no longer red but turn red-tinted when pointed at or focused. In the Text and XSLT views, a confirmed lemma has a green outline with a thicker line underneath, a suggested one a dotted line underneath (it had a faint yellow outline that could hardly be seen); the wavy line under words edited since import is grey instead of a pale yellow, and the lines marking notes and semantic annotations are lighter in the dark theme, where they were hard to see; in the XSLT view and the HTML export, paragraph marks, line-break hyphens and gaps are less faint; in the editor, commands such as `.abbr` and deletions are a darker red in the light theme, unclear readings and `|`/`~` an amber brown instead of yellow, and notes less faint; the divider between editor and preview is wider, with a visible grip
- **Imported files with a note right after a page break changed when saved unedited**: the shorthand put the note directly after the page number (`///109r^{…}`), where it read as part of the number. The text after a page break now starts on a new line. Files imported before keep their shorthand as it was
- **Notes containing `<` could not be imported**: editors' notes such as `^{ð < t}` ("ð corrected from t") were read as an unclosed bracket, so the document had errors and could not be saved. In notes, only braces now pair up, so `<` and `[` are plain text there
- **Ctrl+Shift+Z in the editor did two things**: it redid a text change and undid a lemmatization. In the editor it now only redoes (on every platform); elsewhere it still undoes a lemmatization. Likewise **Ctrl+/** in the editor no longer also wrapped the selection in a note `^{…}`: it opens the help

## [0.7.0] - 2026-10-31

### Added

- **Lemmas are written into imported files**: confirming a lemma sets `lemma` and `me:msa` (or `msa`) on that word in the imported XML. Lemmas that came from the file leave it byte-identical. Notes and other annotations stay in the project, as they would add markup the file doesn't have
- **Normalized forms without a lemma**: the word panel's normalized form field is always there (on multi-level transcriptions), starts from the word's current form, and saves with or without a lemma. Removing a lemma keeps a normalized form set by hand. On imported files, a changed normalized form is written into the word's `<me:norm>` (line breaks in it are kept)
- **Performance log**: started with `--perf-log` (or the environment variable `SAGA_SCRIBE_PERF_LOG=1`), the app writes how long each stage of compiling, previewing and validating takes, and when the window stalled, to `perf-<time>.jsonl` in its log directory

### Fixed

- **Annotations moved to the wrong words** when words were inserted or deleted before them: lemmas and annotations are stored by word number, which shifted. The backend now keeps track of which word is which across edits (as imported mode did since 0.6), reports how word numbers moved after each compile, and the app moves lemmas, annotations, their undo history and the selection along
- **Another word typed in place of a deleted one inherited the old word's lemma and normalized form** (in imported files, the normalized form stuck even when the old word was typed back). Only an edit of a word (differing in at most half its letters, like a typo fix) keeps its lemma, analysis and normalized form; another word in its place is a new word, written in the same place
- **The word panel opens on the word's lemma**, ready to change; it opened empty, so a lemma could only be removed, not edited, when the word's form wasn't in the dictionary (e.g. after a typo fix)
- **Removing a lemma**: the word panel has "Remove lemma"; its ✕ on known analyses removes the analysis from the dictionary (now labelled so), which left the word's lemma in place. On imported files, removing a lemma removes `lemma` and `me:msa` from the word in the file (and is kept in the project, undoable)
- **Undo and redo** of lemmas were never enabled: the toolbar followed a history nothing was recorded in
- **Analyses with their tags in another order** (`xNC gM nS` and `gM xNC nS`) counted as different analyses; they are the same, in the dictionary and when writing lemmas into imported files
- **ONP unreachable**: when an entry's details can't be fetched (offline, or onp.ku.dk down), the word panel says so instead of waiting indefinitely; the analysis can still be entered
- **Editor highlighting disagreed with the compiler** in places: gaps with supplied text (`[...<na>]`) and `|` or `~` right after some characters weren't highlighted, a lone `-`, `:` or `}` was marked as an error, `word.p` was shown as a paragraph break, and braces nested in `.supplied{}` ended it early. The editor now finds constructs exactly as the compiler does, checked against a shared corpus of over a thousand inputs (and HolmPerg)
- **Lemmas after a heading were off by the heading's words**: words inside `.head{}`, `.supplied{}` and `.norm{}` weren't counted when lemmas and annotations were applied, though the previews count them
- **The last lines of the editor hid under the horizontal scrollbar** when long lines made it appear: the editor's scrollbars now take up space of their own instead of overlaying the text
- **Typing in large documents fell further and further behind**: every pause in typing queued a compile, and on a manuscript like HolmPerg they were requested faster than they finished, so the preview lagged by many seconds. Now only the newest waits, and each compile sends the app only the part of the XML that changed instead of the whole document (megabytes, for a large manuscript). The rendered view ran every update it had started at once; it now finishes the one running and then renders only the newest. It also re-read the whole manuscript (seconds, for HolmPerg) for any edit near a page break (typing or deleting a word there), instead of only the edited page
- **Typing stalled for a fraction of a second after each preview update on lemmatized manuscripts**: the rendered text looked up each shown word's annotations by going through all of the document's annotations, three times per word (on HolmPerg, about 45,000 annotations for each of a thousand words shown), whenever it was redrawn. The previews now also wait for a pause in typing (200 ms) before showing new output, so redrawing them never holds up a keystroke
- **Lemma marks flickered on the next words while typing**: after a word was added, the rendered text briefly marked the words after it with their neighbours' lemmas (the marks were renumbered before the text caught up). The marks now change together with the text they belong to
- **The window was blank while the app started**: it now shows the theme's background and the logo with a loading ring until the app is drawn

## [0.6.1] - 2026-10-29

### Fixed

- **XSLT preview lag**: editing a large manuscript with the XSLT preview open re-ran the stylesheet on the whole document after every pause in typing (over 2 s for HolmPerg, blocking the editor). Stylesheets that declare `<?saga-scribe render="page"?>`, like the built-in one, are now applied one page at a time, and after an edit only to the pages that changed (about 70 ms); other stylesheets render as before

## [0.6.0] - 2026-10-29

### Added

- **Direct PDF export** (Linux, Windows): choose where to save, and the PDF is written without the print dialog, from a hidden window so the editor stays responsive. macOS keeps the print dialog ("Save as PDF")
- **Paragraphs in the DSL**: `.p` starts a new paragraph, also mid-line. Imported files show each `<p>` as `.p`, so paragraphs can be split and joined; only the paragraph tags change in the file, and a split that would break the XML (e.g. inside a sentence `<s>`) is reported instead. (The pilcrow ¶ stays the MENOTA character `:para:`.)
- **Typeset PDFs**: pages are laid out with [Vivliostyle](https://vivliostyle.org/) (CSS Paged Media), giving fixed page margins, page numbers, and no headings or single lines stranded at a page edge
- **Bundled Junicode in PDFs**, so MUFI characters print even where the font isn't installed
- **Syntax errors in the editor**: every DSL syntax error is underlined where it occurs, not just the first; compile errors name the line and column
- **Export progress**: HTML and PDF export run behind a progress overlay showing the current step, with a Cancel button
- **Incremental previews**: Rendered Text re-parses only the pages an edit touches; the XML view applies only the changed range
- **Security audit for JS dependencies** in CI, also run on dependency PRs
- **Frontend unit tests** (Vitest) run in CI: rendered-text parsing, incremental rendering equivalence, XSLT sanitizing

### Changed

- **Imported files are never modified by reading them**: unedited content is written back byte-for-byte, and edits only replace the bytes of what was edited
- **Project file format 1.6**: `segments.json` always holds the full manifest (with MENOTA flag and source spans for every tag); 1.4 and 1.5 projects still open. Files imported before this version don't show `.p` until re-imported
- **Imported XML and its DSL read as the same words**: a `<w>` written as several words (`þo at`, 1,447 in HolmPerg) is written `þo~at`, and entity punctuation that is its own `<pc>` (`:middot:`, 483) is set off by spaces. Documents imported earlier keep their DSL as it was
- **Entity punctuation alone between spaces** (`þa :middot: þo`) compiles to `<pc>`; glued to letters (`:middot:Q:middot:`) it stays part of the word
- **Imported mode knows which word you edited**: the editor tells the backend exactly what changed, so deleting or editing one of two identical neighbouring words (`ok ok` with different lemmas) changes that word, not whichever one a comparison guesses. Each compile is also faster, as the original document is analysed once instead of on every keystroke
- **Editing an imported word** keeps its lemma, analysis and hand-curated normalized form (before, the normalized form was regenerated from the dictionary); the diplomatic form follows the edit. Such words are underlined in the previews and flagged "edited" in the word panel until re-imported, as a reminder to check them
- **Non-MENOTA imports** keep their own encoding style (plain `<w>` or plain text) instead of gaining MENOTA markup on edited words
- **DSL parser**: rebuilt on a hierarchical syntax tree with source positions and error recovery (internal foundation for the round-trip work); output is unchanged
- **Syntax errors inside `.head{}`, `.supplied{}` and `.norm{}`** are reported, instead of compiling to an XML comment
- **Validation** runs after a short pause in typing instead of on every compile, and parsed schemas are cached
- **Only the visible preview tab updates**; hidden tabs catch up when shown
- **Content Security Policy** enabled; XSLT output is sanitized before it is rendered, exported or printed
- **SvelteKit 3 / Vite 8**, Tauri 2.12, zip 9 and other dependency upgrades; minimum Rust version 1.90, toolchain pinned
- **Window title and dialogs** use the Saga Scribe name
- **Flatpak runtime**: GNOME 51 (was 49)
- **Third-party licences** are listed in `THIRD_PARTY_LICENSES.md` (dependency tables regenerated with `bun run licenses`)
- **Smaller app**: an 8 MB test manuscript was being bundled into the app; it now lives in `tests/fixtures/`
- **Line numbers after `~//` are digits only**, as after `//`: `hel~//5lo` is one word broken across lines 4 and 5 (the importer writes breaks this way); whitespace right after `~//5` no longer splits the word

### Fixed

- **Round-trip data loss**: a DSL syntax error no longer wipes the document body; distant edits in large files no longer rewrite (and drop lemmas from) everything in between
- **Line/page break edits** in imported mode are applied; inserted words stay in the right paragraph
- **MENOTA detection** on import
- **Save/export** refuse to write a failed compile instead of writing the error message
- **Saved imported projects** reopen in imported mode (segments were saved in a shape that failed to load)
- **Saves are atomic**: a crash mid-save can no longer truncate a project or export
- **PDF/HTML export freeze** on large files (minutes spent resolving entity placeholders)
- **Rendered Text**: `<add>`/`<del>`/`<unclear>` around words showed all three transcription levels and stray line breaks; sic/corr words were missing; the view no longer jumps to page 1 or flashes on every edit
- **Word numbering** now follows document order everywhere, so lemmas loaded from a file stay on the right words
- **Mid-word line breaks** (`~//`): the compiled XML kept the break (single-level) or dropped it entirely (MENOTA), and read the rest of the word as the line number; the default XSLT dropped breaks inside words, merging lines and losing their numbers. It now shows a broken word as one box per line, the first ending in a hyphen
- **Default XSLT, line-by-line transcriptions**: a paragraph starting mid-line pushed the rest of that line onto a row of its own; paragraphs now run inline there, marked with a pilcrow (¶). Prose without line breaks (e.g. front matter) keeps block paragraphs
- **Word spaces between entities** inside a level (`<me:facs>&et; &thorn;a</me:facs>`) were dropped on import, joining two written words
- **A dangling or doubled `~`** (`upp~ haf`, `upp~~haf`) no longer compiles to stray spaces inside a word
- **Abbreviations in MENOTA output**: entities inside `.abbr[…]` were written as literal text (`k:bar:` instead of `k&bar;`), and reading such a file back lost the expansion
- **PDF margins**: page margins were ignored, so text ran to the edge of the page
- **Characters lost on import with older libxml2** (Ubuntu 24.04, i.e. the .deb): undeclared entities such as `&thorn;` were dropped, from text and from attributes like `lemma`. Every entity a document uses is now declared before parsing, so import and validation behave the same on every libxml2 version, and never access the network
- **MENOTA exports declare their entities**: the Menota template and the generated header carry Menota's DOCTYPE (the external entity list, as Menota files have), so exported files using `&thorn;` and the like are well-formed XML
- **Flatpak couldn't start**: it packaged a binary built on Ubuntu, linked against `libxml2.so.2`, which the GNOME runtime doesn't have (it ships libxml2 2.14+). The Flatpak is now compiled inside the GNOME SDK against the runtime's own libraries, which also get the runtime's security updates
- **Export menu** stopped responding after cancelling a file dialog
- **Save and export dialogs** add the file extension (`.teis`, `.xml`, `.html`, `.pdf`, `.json`) when a name is typed without one
- **Security**: XSS via XSLT output, network access during XML parsing, vulnerable `zip`, Svelte, Vite and PostCSS versions

## [0.5.0] - 2026-01-29

### Added

- **Export formats**: TEI-XML, standalone HTML, and PDF export via browser print dialog
- **Dictionary export**: Export lemmatization data as JSON for backup and analysis
- **Custom entity management**: Create, edit, and delete custom entities beyond the MENOTA/MUFI set
- **Imported mode**: Preserve original XML structure during round-trip editing of imported TEI files
- **Metadata editor**: Edit TEI header metadata (title, author, source, language, etc.)
- **Word and character annotations**: MENOTA me:msa morphological annotations with visual indicators
- **XSLT preview mode**: Apply custom XSLT stylesheets to preview XML transformations
- **Page navigation**: Browse large manuscripts by page with instant navigation controls
- **Collapsible sections**: Fold/unfold pages in the editor at page break markers (`///`)
- **Search and replace**: Find and replace text in the editor (Ctrl+H)
- **Settings persistence**: Theme, font size, auto-preview, and template preferences saved between sessions

### Changed

- **Version bump**: 0.3.0 to 0.5.0 reflecting significant feature additions
- **Project file format**: Updated to v1.3 with segment storage for imported documents
- **MENOTA rendering**: Improved multi-level transcription with `<choice>` wrappers around facs/dipl/norm elements
- **Entity browser**: Enhanced with custom entity editing and deletion capabilities
- **Import flow**: Better handling of complex MENOTA structures and namespace declarations

### Fixed

- **XSLT rendering**: Resolved issues with DOCTYPE declarations and namespace handling
- **Windows build**: Properly configure libxml2 linking via vcpkg
- **Windows headers**: Add BINDGEN_EXTRA_CLANG_ARGS for SDK headers
- **Accessibility (a11y)**: All modals now support Escape key dismissal; resolved all Svelte a11y warnings
- **Import stability**: Enhanced handling of unclear tags and edge cases in MENOTA imports
- **Whitespace normalization**: Consistent DSL output from import operations

### Technical

- MENOTA morphology constants extracted to `src/lib/constants/menota.ts`
- Debug console.log statements removed from production code
- 167 passing Rust tests (162 core + 5 documented limitations as ignored)
- Zero Svelte check warnings
- Zero Clippy warnings

## [0.4.0] - 2024-12-XX (alpha)

Initial alpha release with core transcription features.

### Added

- Custom DSL syntax for manuscript transcription
- Multi-level MENOTA transcription support (facs/dipl/norm)
- Entity browser with ~1,980 MENOTA/MUFI characters
- ONP dictionary integration for lemmatization
- Project archives (.teis) bundling source, XML, and annotations
- Real-time preview with text, XSLT, and XML views
- Template system with built-in TEI P5 and MENOTA templates
- Import support for TEI-XML and plain text files

## [0.3.0] - 2026-01-11

Early development release.

## [0.1.0] - 2026-01-09

Initial release.
