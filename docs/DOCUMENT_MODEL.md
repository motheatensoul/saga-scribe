# Document model (design, draft)

Status: design agreed, 2026-11-11. Nothing here is implemented yet. It replaces
the segment manifest and the patcher's way of writing edited words, and prepares the
shorthand redesign (see "Shorthand (DSL)" in `TODO.md`).

## Why

The edition is the XML file: everything an editor records has to end up there. The app
lets people work on it through views that show only part of it: the shorthand, the word
panel, the previews. Each view has to write its changes back without disturbing what it
doesn't show. Today it doesn't:

- An edited word is compiled again from its shorthand (`render_modify` in
  `src-tauri/src/importer/tei/patching.rs`). Only the `<w>` attributes and the normalized
  form are carried over, so everything else in the word is lost, however small the edit:
  `<ex>` on the diplomatic level, `<supplied resp="…" reason="…">`, a gap's `extent` and
  `unit`, `<hi>`, comments. A word can also gain a level its file doesn't have.
- A `.teis` holds several partial truths side by side: `source.dsl`, `output.xml`, and
  for imported files `segments.json` and the original XML split in three, with
  `confirmations.json` and `annotations.json` next to them. New and imported documents
  go through different pipelines, and each reconciles these differently.
- The program assumes MENOTA's three levels (the `multi_level` and `is_menota` flags, read
  in about 15 files), while many files use one or two.

A survey of the whole Menota catalogue (91 texts, 196 charters, 2026-11-09; see `TODO.md`)
puts numbers on it. The texts' XML has 535k `<ex>` but their shorthand only 300k `.abbr`.
Of 11,764 `<supplied>`, about 2,200 show in the shorthand. 136 of the 196 charters are
diplomatic-only. All of that hidden markup is lost when its word is edited.

New syntax would narrow this gap, but it can never close it: editors will always use
markup the shorthand doesn't know. The fix is in how edits are written back.

## Principles

1. **The XML is the truth.** The in-memory document is the XML tree itself, not a domain
   model converted to and from it. Everything else (the shorthand, the word panel, the
   previews) is a view of it.
2. **libxml2 is the only XML parser.** We don't write our own. Everything we write is
   parsed by libxml2 again before it's saved.
3. **Views are lenses.** Each view has a `get` (tree → view) and a `put` (new view, old
   tree → new tree). `put` takes the old tree, so whatever the view doesn't show comes
   from there.
4. **Edits are local.** An edit changes only the XML nodes it overlaps.
5. **Loss is never silent.** If an edit removes something the view didn't show, the
   editor is told before it's saved.

The laws that follow from 3 and 4 are the tests:

- **GetPut:** an unchanged view gives a byte-identical file. (Today's guarantee.)
- **PutGet:** after an edit, the view shows what was typed.
- **Locality:** an edit leaves every node outside its range unchanged, attributes,
  comments and unknown elements included.

## The tree

### Parsing

`crate::xml::parse` parses the file with libxml2, as now (entities declared first, no
network). libxml2 gives the complete logical tree: elements, attributes in source order,
text, comments, processing instructions, CDATA, entity references.

What it doesn't keep is how the file is written: quote style, spacing inside tags, `<x/>`
versus `<x></x>`, character references (`&#x00E6;` becomes `æ`), `&gt;` versus `>`,
attribute value normalization, CRLF, the XML declaration's and DOCTYPE's formatting. It
also has no byte offsets for nodes. None of this is content, and none of it needs a
parser: unchanged nodes are copied from the original bytes, changed ones are written
fresh. For that, each node needs its byte span. `source_map.rs` already finds spans by
walking the original text alongside libxml2's tree. It never decides structure, only
locates what libxml2 parsed. When the text and the tree disagree it gives up, and the file
is written without spans: formatting may change, content doesn't. It will be extended
from segments to every node.

### Holding it

The binding's mutable `Document` can't be shared between threads, and our heavy work runs
on `spawn_blocking` threads. So libxml2's result is copied, one to one, into a Rust arena
tree:

- one node kind for each libxml2 node kind; elements are generic (name, namespace,
  attributes in order), so unknown elements are kept like known ones;
- each node has a stable ID (for the session), its byte span if known, and a dirty flag;
- the copy is total and mechanical. It isn't a domain model, so nothing can fall
  through it.

### Writing

A clean node with a span writes its original bytes. A dirty node is written with the
existing escaping helpers (`helpers.rs`), and its children follow the same rule. Before
anything is saved, the output is parsed by libxml2 and compared with the tree in memory.
The file is written only if they agree, so a bug in the writer can't save a broken or
different file.

## The typed layer

Code doesn't work on raw nodes. Thin handles over the tree give it a usable API without
copying anything out:

- `Word`: a `<w>` (or `<pc>`, `<me:punct>`): attributes (lemma, msa), its levels.
- `Level`: one of facs, dipl, norm in a word (or the word's own content when the file has
  no levels). Its text, plus the inline markup in it as **ranges** over that text.
- `Range`: an inline element (`<ex>`, `<am>`, `<supplied>`, `<unclear>`, `<hi>`, unknown
  ones) over a span of the level's text, with its attributes. Milestones inside a word
  (`<lb/>`, `<gap/>`) are ranges of length zero.

Handles read and edit the tree directly: `word.set_lemma(…)`, `level.edit(range, text)`,
`level.wrap(range, Supplied { … })`. What the layer doesn't understand stays in the tree
untouched, because it is never copied out.

## Levels: the project profile, and derived or pinned

### The profile

A project has a profile, stored in the `.teis`:

- which levels exist (any of facs, dipl, norm), and which one the shorthand writes (facs
  if present, otherwise dipl);
- how abbreviations are encoded (`<am>`/`<ex>` or `<choice><abbr><expan>`), which
  punctuation element is used (`<pc>` or `<me:punct>`), and which edition's line breaks
  `//n` stands for when a file has several (`<lb ed="…">`).

On import the profile is inferred from the file and shown for confirmation, with counts
(e.g. "facs in 98% of words"), together with a list of the markup the shorthand can't
show. The compiler, importer and word panel ask the profile instead of assuming MENOTA.
This replaces `multi_level` and `is_menota`.

- the **derivation rules** for each level (below). Today they are fixed for the whole
  app, but they describe Old Norse in MENOTA (e.g. which base letter a MUFI character
  has, `static/normalizer/entity-base-letters.json`). They become part of the profile:
  the current rules are the default for Old Norse, other languages and conventions get
  their own sets, and editors can adjust them.

### Derived or pinned

Each level of each word is either:

- **derived**: computed from the shorthand by the profile's rules (e.g. dipl is facs
  with the abbreviations expanded); or
- **pinned**: its content is set explicitly, by the imported file or in the word panel.

On import, a level that equals its derivation counts as derived; otherwise it's pinned.
When the shorthand changes, derived levels are computed again, and pinned levels get the
same edit where the text lines up. Where it doesn't, they're kept and marked for review.
The word panel shows which levels are derived or pinned, and can reset a level to its
derivation. A level the profile doesn't have is never generated; a level the word has but
leaves empty stays empty. (The normalized form already works this way today.)

## Edits are operations

Every change, from any view, is a small operation on the tree: insert or delete a word,
replace text in a level, wrap or unwrap a range, set or remove an attribute, pin or reset
a level. This gives:

- **one undo history** for text and lemmatization (today they are separate);
- **nothing removed unseen**: an operation that would remove something the view didn't
  show (a word's lemma, a range whose text was deleted) keeps it in a *replaced* or
  *removed* record with the word, to be confirmed or restored (see "Replacing and deleting
  words"); Validate lists the open ones;
- **incremental work**: only the words an operation touches are projected again, so typing
  stays fast in a 50,000-word manuscript.

## The shorthand lens

### `get`: tree → shorthand

The projection writes each word's shorthand from its primary level and the markup the
shorthand has syntax for. Step 2 below makes it produce exactly what today's importer
produces, so the switch is invisible; the syntax redesign later changes only the
projection and the parser.

### `put`: new shorthand, old tree → new tree

1. **Which word is which.** As now: the editor's changes since the last compile
   (`editHistory.ts`, sent as `Guidance`) say which words are untouched; only edited
   ranges are matched by comparison (`identity.rs`). Insertions and deletions become word
   operations.
2. **What changed in a word.** The old and new shorthand of the word are parsed and
   compared: a character diff of the text, and a comparison of the markup the shorthand
   shows.
3. **Applying it.** Text changes become text operations on the primary level, and
   changes to shown markup become wrap or unwrap operations. Derived levels follow, and
   pinned ones get the edit merged in (above).

**Ranges the shorthand doesn't show** move with the text, the way CodeMirror maps
positions through changes:

- text replaced entirely inside a range stays inside it, so the range grows or shrinks;
- text inserted at a range's edge goes outside it: new text doesn't inherit an editor's
  claim such as `resp`;
- a range whose text is deleted entirely is removed, and that is reported;
- zero-width ranges (milestones) stay where they are relative to the text around them.

So `hæn<ex>n</ex>aR` edited to `hænnaRR` keeps its `<ex>`.

### Replacing and deleting words

How a word was changed in the editor says little about what the editor meant. A word
misspelled by dropping to the line below is often fixed by selecting it and typing over
it, or by deleting it and typing it again, not by correcting the wrong letters. The
result looks the same as replacing it with a different word. So:

- **Keystrokes only decide which word is which, never what happens to its data.** Within
  a word, the old and the new text are compared as wholes. Typing over `kongunr` with
  `konungr` maps the word's ranges exactly as a careful two-letter fix would. Retyping
  is never worse than editing.
- **The remaining question is whether it's the same word**, corrected, or a different
  word in its place. The answer decides what belongs to the word as a word of the
  language (lemma, analysis, normalized form, `xml:id`, annotations). The transcription
  markup follows the text in both cases.
- **Nothing is removed until that is answered.** The old word's complete XML is kept with
  the new word (a *replaced* record), and the word is marked in the shorthand and in the
  previews. The program suggests an answer: same word if the texts are similar (the
  current rule) or the new form is an inflection of the old lemma in the dictionary;
  otherwise a different word. Until the editor decides, the suggestion is what goes into
  the XML.
- **Asking must not interrupt typing.** No dialog opens while someone types. A marked
  word offers both answers in the editor (as a quick fix on the word, also from the
  keyboard) and in the word panel when it's clicked in a preview:
  "Same word, corrected: keep lemma *konungr*, analysis xNC gM nS cN" or "Different word:
  remove them". Several open questions can be accepted together.
- **Deleted words are kept the same way.** Deleting a word with a lemma, annotations or
  markup the shorthand doesn't show leaves a *removed* record at its place, listed for
  review with "Restore". A word deleted in one place and the same text inserted
  elsewhere (cut and paste) is offered as a moved word.
- **Saving doesn't lose the records.** The XML is written with the current answers
  (decided or suggested); undecided records stay in the project, so the decision can
  still be made, or undone, after saving and reopening. Saving says how many are open.
- When several words are selected and replaced, old and new words are paired as for
  single words; old words left over become removed records.

### Keeping the shorthand as written

The `.teis` stores the shorthand as the editor wrote it, so their layout and spacing
survive saving and closing. On load, the XML is projected and compared with the stored
shorthand word by word (the check "XML and shorthand read as the same tokens" already
exists). Where they agree, the stored text is kept. Where they don't, the XML wins: those
words are rewritten from the projection, and the editor is told how many places changed.
Loading parses the XML anyway; the projection is cheap next to that (the survey projected
2.5M words in seconds).

## The word panel

Everything in a word must be editable from the word panel (opened by clicking a word in
the Text or XSLT view), so that markup the shorthand can't express can be checked and
corrected without the full XML view. The panel works on the typed layer directly:

- lemma and analysis;
- which levels the word has, and each level's content, pinned or derived (with reset);
- its inline markup as ranges, with their attributes (`<ex>`, `<supplied resp reason>`,
  `<gap extent unit>`, unknown elements);
- the `<w>` element's other attributes;
- annotations;
- open *replaced* or *removed* records for the word, with their answers;
- an XML tab with the word's own XML, for anything the other tabs don't cover. It's
  parsed by libxml2 and must still be a single word.

After a change the word's shorthand is projected again. The previews can mark words that
carry markup the shorthand doesn't show (a view option), so editors know which words to
check after editing them in the shorthand.

Other parts of the file (the header, the front and back matter) should become editable
the same way later.

## Words in the project data

Data kept in the project (annotations, decisions on replaced words, the stored
shorthand's word boundaries) refers to words by their node in memory, never by counting.
When saved, each reference gets an anchor: the word's `xml:id` if the file has one,
otherwise its position, together with a **fingerprint** of the word (its text on each
level and its lemma). On load, the word at the anchor must match the fingerprint. If it
doesn't (the XML was changed outside the app), the word is looked for nearby by its
fingerprint; what can't be found is listed for review, never attached to another word.
The app doesn't add `xml:id`s to files that don't have them.

## Project data and file format

A new format version of the `.teis` holds:

- `document.xml`: the current XML, the truth;
- `profile.json`: levels and encoding choices;
- `source.dsl`: the shorthand as written (a cache, checked on load as above);
- data that isn't part of the edition: annotations kept in the project only, unconfirmed
  suggestions, view settings.

`segments.json`, `output.xml` and the split original XML go away. New documents are trees
created from a template, so new and imported documents take the same path. Older projects
are converted on load: imported ones from their original XML plus the stored shorthand put
through the lens; new ones from `output.xml`.

## Steps

Each step has a gate on the full Menota corpus (opt-in, like `HOLMPERG=1`, with a script
that downloads it from `https://clarino.uib.no/korpuskel-api/download-zip?corpus=menota`
and `corpus=menota-diploma`; the corpus itself isn't committed, its licences differ per
text).

1. **Tree and typed layer.** The arena tree from libxml2, spans for every node, the
   writer, the check before saving. Gate: all files parse and write back byte for byte;
   the tree equals libxml2's for each.
2. **Projection.** `get` on the tree. Gate: the same shorthand as today's importer for
   every file. The importer then uses it.
3. **`put` through operations.** Replaces the patcher. Gates: GetPut and PutGet on every
   file; a fuzzed edit in every word keeps all markup outside the edited range (Locality);
   the existing round-trip tests.
4. **Profile and levels.** The profile with the import dialog; derived and pinned levels.
5. **Word panel** on the typed layer.
6. **New documents and the file format.** The same path for new documents; the new
   `.teis` version, converting older ones.
7. **Shorthand redesign** on top (abbreviations, supplied text with markup, short form
   for normalized punctuation): only the projection and the parser change.

Steps 1 and 2 change nothing users see. From step 3 on, edits stop losing markup.

## Decided (2026-11-11)

- Project data refers to words by node, with an anchor and fingerprint when saved (above),
  not by word number.
- Derivation rules become configurable, in the profile.
- Everything in a `<w>` becomes editable from the word panel, with an XML tab as the
  fallback. The rest of the file follows later.
- Replacing or deleting a word asks whether it's the same word before its data is
  removed (above).

- Open *replaced* and *removed* records are shown in the shorthand by a gutter mark. How
  it looks and behaves is to be settled by testing with users.

- A pinned level that can't follow an edit is kept and marked for review, with the same
  gutter mark. Example: facs `kgr` corrected to `kgs`. The dipl is derived and becomes
  `k<ex>onung</ex>s`, but the pinned norm still says `konungr`. Whether it's now wrong
  (the inflection changed) or still right (only a misread letter was fixed) can't be
  told from the letters. The mark appears only when the edit touched letters the pinned
  level can't follow, so ordinary typo fixes don't produce marks. The word panel shows
  "normalized form may be out of date" with keep, edit and, for a pinned dipl, reset to
  derived. It doesn't open by itself.

Later, once this works:

- a button that opens the word panel on the next word marked for review, so editors don't
  have to look through the whole text for marks;
- with a robust lemmatizer, the program could infer the changed lemma and analysis after
  an edit, and from them the new dipl and norm, instead of only marking the word.

## Open questions

None at the moment.
