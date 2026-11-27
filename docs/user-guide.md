# Saga-Scribe User Guide

This guide describes how to transcribe, annotate and export a manuscript with Saga-Scribe.
For installation, see the [README](../README.md#getting-started).

The document is its TEI-XML file: that is the edition, and it holds everything you record.
The shorthand, the word panel, the metadata editor and the previews are views of it, each
showing part of it; a change made in any of them is written into the XML, and what a view
doesn't show stays in the XML as it was.

## Contents

- [The window](#the-window)
- [Projects and files](#projects-and-files)
- [Transcription shorthand](#transcription-shorthand)
- [Templates](#templates)
- [Manuscript metadata](#manuscript-metadata)
- [MENOTA transcription levels](#menota-transcription-levels)
- [Special characters](#special-characters)
- [Preview](#preview)
- [Lemmatization](#lemmatization)
- [Annotations](#annotations)
- [Validation](#validation)
- [Importing existing files](#importing-existing-files)
- [Exporting](#exporting)
- [Settings](#settings)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Logs and reporting problems](#logs-and-reporting-problems)

## The window

The window is divided into three parts:

- **Toolbar** (top), in groups from left to right: the file (**Open**, **Import**, **Save**,
  **Export**); the document (the template in use, the template manager, **Metadata**,
  **Abbreviations** and **Validate**); undo and redo of lemmatization; and, at the right, the application log,
  help and settings, followed by the file's name. In a narrow window the buttons show only
  their icons (hover over one for its name), and groups that don't fit move to a second
  row.
- **Editor** (left): the transcription, written in the shorthand described below. Its header
  has the buttons that act on the text: undo and redo, **Show entities as characters**
  (**þ**), search and replace, and the entity browser (**ꝥ**).
- **Preview** (right): the compiled document, shown as reading text, through an XSLT
  stylesheet, or as XML.

A typical session:

1. Choose a template in the toolbar (see [Templates](#templates)).
2. Transcribe in the editor. The preview is updated after each pause in typing.
3. Click words in the preview to lemmatize or annotate them.
4. Save the project (**Ctrl+S**).
5. Export TEI-XML, HTML or PDF.

## Projects and files

Work is saved as a project file with the extension `.teis`. It is a ZIP archive containing:

- the transcription in shorthand (`source.dsl`);
- the compiled TEI-XML (`output.xml`), or for an imported TEI file, the file as it now is
  (`document.xml`), with how it encodes its transcription (`profile.json`) and the
  replaced words still to be decided (`records.json`);
- confirmed lemmas and analyses, word by word;
- the abbreviation store (`abbreviations.json`, see [Abbreviations](#abbreviations));
- annotations, with a fingerprint of each annotated word (`anchors.json`), so that if the
  file was changed outside Saga-Scribe they are found on their words again, and those whose
  words are gone are left out with a warning in the log;
- manuscript metadata;
- for a new document, the words whose XML was set in the word panel's **XML** tab
  (`overrides.json`).

A new document's XML is compiled from the shorthand, with lemmas, notes and other
annotations written into it, so the exported file is a complete edition. A word whose XML
you set in the word panel's **XML** tab keeps it: later edits of the word in the shorthand
are merged into it, and lemmas and annotations changed later are written into it too.

For an imported file, `document.xml` is the file with your edits: what you didn't edit is
in it exactly as in the file you imported. When the project is opened, the shorthand is
compared with that file, and any place where it doesn't read as the file is rewritten from
it (the log says how many). Projects saved with Saga-Scribe 0.8 can't be opened with
earlier versions.

**Open** (**Ctrl+O**) opens a project, or a plain shorthand file (`.dsl` or `.txt`).
**Save** (**Ctrl+S**) saves the project; the first save asks for a location. Projects saved
with older versions of Saga-Scribe can still be opened. If such a project was imported
from a TEI file, the words you didn't edit are rewritten in the shorthand as this version
imports the file, and the log says how many places changed; what you typed stays as it was. If the compiled TEI-XML isn't
well-formed, the project is saved all the same and the log shows a warning (this is a bug
in Saga-Scribe; please report it).

## Transcription shorthand

The shorthand is plain text with inline markers. Each marker corresponds to a TEI element;
the compiler adds the surrounding structure, including the division into words.

### Line and page breaks

```
//           Line break (numbered automatically if the template says so)
//5          Line break with an explicit number
///1v        Page break, with the folio or page number
//{56r}      Line break whose number is not just digits
///{fol. 1r} Page break whose number contains a space
///c=B       Column break (<cb n="B"/>)
.p           New paragraph
```

A line break's number is digits only, so that `//2Text` is line 2 followed by "Text"; a
page break's number ends at the next space. Any other number goes in braces, as does a
page number starting with `c=`. A word continues across a column break as across a page
break (`hel~///c=Blo`).

`.p` has to stand on its own between spaces (`word.p` is ordinary text). A paragraph can
begin anywhere, also mid-line: `//4 ſina . .p þer vitið …`. The pilcrow `¶` is a character
written by the scribe (`:para:`), not a paragraph break.

### Editorial marks

```
[...]        Gap (illegible or lost text)
[...3]       Gap of about three characters
[...3<ab>]   Gap with supplied text
<abc>        Supplied text (it can contain shorthand: kæn<n~//14:thorn:>a)
-{abc}-      Deletion
+{abc}+      Addition
?{abc}?      Unclear reading
^{note}      Note
```

Supplied text, deletions, additions and unclear readings can contain shorthand: entities,
gaps, line breaks and abbreviations (`þ?{:er:}?`, `-{ny[...]}-`). A note is plain text.
To write a character that would be read as shorthand there, put a backslash before it
(`?{a\:b}?`).

### Other markup

Elements the shorthand has no sign of their own for are written with their name, a dot
before it and their content in braces: `.q{Ok sagði hann}` is `<q>…</q>`. The content is
shorthand. Some have short forms:

```
*{A}*        Highlighting (<hi>)
&{abc}       Segment (<seg>)
${abc}       Surplus text (<surplus>)
.c{S}        A character (<c>)
.s{…}        A sentence (<s>)
.q{…}        Quotation (<q>); also .quote{…}
.name{…}     A name (<name>); also .title{…}, .gloss{…}, .damage{…}, .subst{…}
.psname{…}   Person's name (<persName>), with .fname{…} for the forename (<forename>)
.plname{…}   Place name (<placeName>), with .country{…}, .region{…}, .settlement{…}
.corr{ſagt=sagt}   An error and its correction (<choice><sic>…</sic><corr>…</corr></choice>)
.trans{a b=b a}    Transposed words and their order (<choice><orig>…</orig><reg>…</reg></choice>)
.rdg[AM70]{…}      A witness's reading (<rdg wit="AM70">)
.space[3]    Space left by the scribe (<space quantity="3"/>)
.hs{hand2}   Change of hand (<handShift new="hand2"/>)
```

Where the markup stands on its own between words, it goes around the words it holds,
each still a word of its own (`.psname{.fname{Ari} fróði}` is a `<persName>` around two
`<w>`). Attached to letters, it is inside the word (`*{A}*nnat` is `<w><hi>A</hi>nnat</w>`,
on each level of a MENOTA word). Markup around words can't contain a paragraph break
(`.p`). For imported files, see [TEI-XML](#tei-xml); column breaks and changes of hand
can't be added to them yet.

### Abbreviations

```
{mark=expansion}
```

The braces hold only what is abbreviated, so they stand inside the word: the abbreviation
mark as written before `=`, and what it stands for after it. As in the MENOTA handbook, the
mark is `<am>` on the facsimile level, the expansion `<ex>` on the diplomatic level, and the
normalized level has the word as read:

- `k{:bar:=onun}gs` (a bar over the k) is `k<am>&bar;</am>gs`, `k<ex>onun</ex>gs`,
  `konungs`;
- `{:kbar:=konun}gs` (a k with a stroke through it, one character) is
  `<am>&kbar;</am>gs`, `k<ex>onun</ex>gs`, `konungs`: the letter's base letter (from
  MUFI's character list) was written, so it stays outside `<ex>`;
- `hæn{=n}aR` is an expansion without a mark (`hæn<ex>n</ex>aR`), as diplomatic
  transcriptions have them;
- `{:et:=ok}` is a sign that stands for a whole word.

In a transcription without levels, mark and expansion stand side by side
(`<am>&kbar;</am><ex>konun</ex>gs`). A brace as text is written `\{`.

#### The abbreviation store

Most marks stand for the same letters nearly every time, so the expansion can be left out:
`{:kbar:}gs` takes the expansion from the project's abbreviation store, where each mark
has its expansions, the default first. An expansion written out always wins over the store.

- When a TEI file is imported, the store is filled from its `<am>` and `<ex>`: each mark
  gets the expansions it has in the file, the most frequent first, and wherever a mark has
  that expansion it is written `{mark}`. (A mark found with its expansion only once is
  written out and not put in the store.)
- A `{mark}` whose mark isn't in the store is underlined with a warning: it is compiled as
  the mark alone. Write it once as `{mark=expansion}`; that form, for a mark not yet in the
  store, offers **Add "…" for … to the store** (rest the pointer on its underline), after
  which every `{mark}` takes it.
- Resting the pointer on a `{mark}` shows its expansion.
- **Abbreviations** in the toolbar lists the whole store: choose which expansion of a mark
  is the default, remove an expansion (a mark goes with its last), or add a mark and an
  expansion. A change applies to every `{mark}` at once; words with their expansion written
  out don't change.

The store is saved with the project (`abbreviations.json`).

The older form `.abbr[kgs]{konungs}`, for a whole word, still works and is encoded as
`<choice><abbr>kgs</abbr><expan>konungs</expan></choice>`. When a TEI file is imported, its
`<am>` and `<ex>` are written as `{mark=expansion}` where they stand in the word, in the
word's facsimile text; where the diplomatic level has other letterforms (`ꝼ` and `f`), it
keeps them. Only a word whose marks and expansions don't pair up letter for letter is
written whole, as `.abbr[facsimile]{diplomatic}`.

### Word division

Words are separated by spaces. The following markers override that:

```
|            Word boundary where there is no space
upp~haf      One word written as two (one <w>; "upp haf" in facs/dipl, "upphaf" in norm)
hel~//5lo    Word continuing across a line break (here, to line 5)
hel~///1vlo  Word continuing across a page break
```

Punctuation written as an entity is a separate `<pc>` element when it stands alone between
spaces (`þa :middot: þo`). Attached to letters, it is part of the word (`:middot:Q:middot:`
is the numeral ·Q·). A single letter entity such as `:inodot:` (the word í) is a word.

### Numerals

```
#.x.#        Numeral (<num>), here .x.
```

A numeral is a token of its own, like a word, and its dots and other punctuation are part
of it. It is not numbered or lemmatized as a word.

### Characters that are otherwise shorthand

A backslash before a character makes it plain text, part of the word it stands in:

```
ꝼæ\;ðꝛ       One word containing a semicolon (a lone ";" would be punctuation)
\(anno\)     One word in brackets
,\,          One punctuation mark made of two commas
\#  \\  \|   The characters #, \ and | themselves
```

Right after punctuation, an escaped character continues the punctuation (`,\,`); `|`
keeps them apart. A word made only of escaped punctuation (`\(`) is punctuation.

### Special characters

Characters outside the ordinary alphabet are written as MENOTA/MUFI entities between colons:

```
:rrot:       r rotunda
:thorn:      þ
:eth:        ð
:aelig:      æ
```

The [entity browser](#special-characters) lists all available entities and inserts them at
the cursor.

## Templates

A template supplies the parts of the TEI document around the transcription (the header and
the closing tags) and decides how the transcription is encoded. The template in use is
chosen in the toolbar. Templates are managed with the template button next to it.

Two templates are built in:

- **TEI P5**: a TEI P5 document with a minimal header.
- **Menota**: a document following the MENOTA handbook, with the MENOTA namespace and
  entity declarations, and the three transcription levels enabled.

Each template has three options:

| Option            | Effect                                                                      |
| ----------------- | --------------------------------------------------------------------------- |
| Word wrapping     | Each word is wrapped in `<w>`, punctuation in `<pc>`. Required for lemmatization and annotations |
| Auto line numbers | Line breaks without a number (`//`) are numbered consecutively              |
| Multi-level (MENOTA) | Each word contains `<me:facs>`, `<me:dipl>` and `<me:norm>` (see [below](#menota-transcription-levels)) |

To create a template, click **+ New Template**, enter a name and description, the XML before
the transcription (up to and including `<body>`) and after it, and choose the options.
Built-in templates cannot be changed; their edit button creates a copy that can be. Any
template can be duplicated, and your own templates can be edited and deleted.

The default template for new projects is set in [Settings](#settings).

## Manuscript metadata

The metadata editor (**Metadata** in the toolbar) records information about the edition
and the manuscript, in collapsible sections:

- **Title & Responsibility**: title, subtitle, author, editors (with ORCID) and other
  contributors with their roles.
- **Publication Information**: publisher, place and date, identifiers (DOI, URN, URL) and
  licence.
- **Manuscript Identifier**: country, city, repository, collection, shelfmark and other
  identifiers.
- **Manuscript Contents**: summary and main language.
- **Physical Description**: material, extent, foliation, layout, condition and scribal hands.
- **History**: date and place of origin, provenance and acquisition.
- **Languages Used**: all languages occurring in the transcription.

The metadata is written into the document's `<teiHeader>` and saved with the project. When a
TEI file is imported, its header is read into the metadata editor.

## MENOTA transcription levels

With the **Multi-level** option, each word is given in three forms, as the MENOTA handbook
describes:

- **Facsimile** (`me:facs`): close to the manuscript, with special characters as entities.
- **Diplomatic** (`me:dipl`): special characters resolved to their usual forms.
- **Normalized** (`me:norm`): the standardized form, set during
  [lemmatization](#lemmatization).

For example, `:rrot:egn` becomes

```xml
<w>
  <me:facs>&rrot;egn</me:facs>
  <me:dipl>regn</me:dipl>
  <me:norm>regn</me:norm>
</w>
```

How an entity appears on the diplomatic level is set in the entity browser (see
[Diplomatic forms](#diplomatic-forms)).

## Special characters

The entity browser (**ꝥ** in the editor header) lists the roughly 2,000 MENOTA and MUFI
characters, displayed in the Junicode font.

- Search by name or description, or choose a category.
- Click **Insert entity** to insert `:name:` at the cursor.

### Entities shown as characters

With **Show entities as characters** (**þ** in the editor header) switched on, the editor
shows each known entity as its character, in Junicode on a faint background, so that `:thorn:`
reads as þ. Where the cursor touches or enters one, its `:name:` is shown again for editing.
**Backspace** and **Delete** next to a character shown this way delete the whole entity.
The text itself still holds `:name:`: search and replace, copying and the compiled XML all
work with the names. Unknown entities are always shown as written, and marked with a
warning. Switch the button off to see every entity as `:name:`. The setting is kept
between sessions; it is on by default.

### Diplomatic forms

Selecting an entity shows its **Translation**: the form it takes on the diplomatic and
normalized levels. Change it and press Enter to save; **Reset to default** restores the
original. Changed entities are marked "custom".

### Your own entities

Characters that are not in the MENOTA/MUFI set can be added with the **+** button in the
browser's header: give the name used in the shorthand (without colons), the character, and
optionally its Unicode code point, a category and a description. Your own entities can be
edited and deleted from the list.

Your own entities and diplomatic forms are stored with the application, not in projects:
they apply to all your projects, and are not included when a project is given to someone
else. Changing an entity affects every document compiled afterwards; deleting one leaves
`:name:` in the transcription without a definition.

## Preview

The buttons in the preview header switch between three views.

### Text

The reading text, with clickable words for [lemmatization](#lemmatization) and
[annotations](#annotations). Manuscripts with page breaks are shown one page at a time;
use the navigation bar (first, previous, next, last, or the page list) to move between
pages. The navigation bar also shows the number of words and tokens.

### XSLT

The compiled document transformed by an XSLT stylesheet and shown as HTML. The stylesheet
is chosen in [Settings](#settings), where your own stylesheets can also be imported; the
same stylesheet is used for HTML and PDF export. Stylesheets are XSLT 1.0, as supported by
the system's web engine.

The XSLT view shows the document as the stylesheet will produce it, so its words can be
clicked but not reached with the keyboard. To open the word panel from the keyboard, use
the Text view, where Tab moves from word to word and Enter opens the word panel.

#### Faster previews with your own stylesheets

Applying a stylesheet to a whole manuscript can take seconds, which delays the preview
after each edit. If your stylesheet's output for an element depends only on that element
and its ancestors (no `preceding::` or `following::` axes, keys, or counting across the
document), add this line inside `<xsl:stylesheet>`:

```xml
<?saga-scribe render="page"?>
```

The preview then applies the stylesheet one page at a time and, after an edit, only to the
pages that changed. Each page is passed as a small TEI document of its own: elements that
continue from the previous page are repeated at its start with `saga-continued="true"`
(so that, for example, a paragraph mark can be skipped there), and the parameter
`lineBased` is `"1"` when the text has line breaks. Exports always apply the stylesheet to
the whole document. The built-in stylesheet uses this declaration.

### XML

The compiled TEI-XML.

## Lemmatization

Lemmatization requires a template with word wrapping. Click a word in the Text or XSLT
preview to open the word panel; its **Lemmatize** tab shows:

- **Known analyses** of the same word form, from earlier work. Click one to apply it to
  this word. The ✕ next to an analysis removes it from the list of known analyses (for all
  words); lemmas already given to words are kept.
- **Search ONP dictionary**: search the
  [Dictionary of Old Norse Prose](https://onp.ku.dk/) by headword and choose the entry.
  The link next to an entry opens it on the ONP website. If ONP cannot be reached, the
  panel says so; the analysis can still be entered.
- **Word class** and the morphological categories that apply to it (case, number, gender,
  definiteness, grade, person, tense, mood, voice, finiteness).
- **Normalized form** (`me:norm`), on multi-level transcriptions. It starts from the word's
  current form and can be saved with or without a lemma. In an imported file, **From the
  shorthand** puts in the form Saga-Scribe gives the word's shorthand, for example to
  replace a curated form that an edit has made out of date; save to keep it.

**Save** confirms the lemma and analysis for this occurrence of the word. **Remove lemma**
removes them again; a normalized form entered by hand is kept.

In the Text view, words with a confirmed lemma have a green outline and background; words
whose form has a known analysis that has not been confirmed for this occurrence have a
dotted line underneath. Hovering over a word shows its lemma and analysis.

Lemmatization has its own undo and redo (toolbar, or **Ctrl+Shift+Z** / **Ctrl+Shift+Y**),
separate from the editor's. In the editor, **Ctrl+Shift+Z** redoes a text change instead, so
undo a lemma there with the toolbar's **Lemma Undo**.

Lemmas are stored by word, and follow their words when text is inserted or deleted before
them. If a word is edited slightly (for example to correct a typo, changing at most half
its letters), it keeps its lemma; a different word typed in its place does not.

### Morphological analysis (me:msa)

Analyses use MENOTA's `me:msa` codes, separated by spaces. The order of the codes does not
matter.

#### Word classes

| Code | Meaning                   |
| ---- | ------------------------- |
| xNC  | Common noun               |
| xNP  | Proper noun               |
| xAJ  | Adjective                 |
| xVB  | Verb                      |
| xPE  | Personal pronoun          |
| xPR  | Reflexive pronoun         |
| xPQ  | Interrogative pronoun     |
| xPI  | Indefinite pronoun        |
| xDD  | Demonstrative determiner  |
| xDQ  | Quantifying determiner    |
| xDP  | Possessive determiner     |
| xAV  | Adverb                    |
| xAQ  | Interrogative adverb      |
| xAP  | Preposition               |
| xCC  | Coordinating conjunction  |
| xCS  | Subordinating conjunction |
| xIT  | Interjection              |
| xIM  | Infinitive marker         |

#### Morphological categories

| Category     | Values                                                    |
| ------------ | --------------------------------------------------------- |
| Case         | cN (nominative), cG (genitive), cD (dative), cA (accusative) |
| Number       | nS (singular), nD (dual), nP (plural)                     |
| Gender       | gM (masculine), gF (feminine), gN (neuter)                |
| Definiteness | sI (indefinite), sD (definite)                            |
| Grade        | rP (positive), rC (comparative), rS (superlative)         |
| Person       | p1, p2, p3                                                |
| Tense        | tPS (present), tPT (preterite)                            |
| Mood         | mIN (indicative), mSU (subjunctive), mIP (imperative)     |
| Voice        | vA (active), vR (reflexive/middle)                        |
| Finiteness   | fF (finite), fP (participle), fS (supine), fI (infinitive) |

Examples:

- `xNC cN nS gF`: common noun, nominative singular feminine
- `xVB fF p3 nS tPT mIN vA`: verb, finite, 3rd person singular preterite indicative active
- `xAJ rC cA nP gM`: adjective, comparative, accusative plural masculine

### In the TEI output

Confirmed lemmas appear as `lemma` and `me:msa` attributes on the word:

```xml
<w lemma="kona" me:msa="xNC cN nP gF">
  <me:facs>konur</me:facs>
  <me:dipl>konur</me:dipl>
  <me:norm>konur</me:norm>
</w>
```

## Annotations

The word panel's **Annotate** tab lists a word's annotations and adds new ones. There are
three types:

- **Note**: free text, optionally with a category. Written as a `<note>` after the word.
- **Semantic category**: a category and subcategory (for example a personal name), with an
  optional label. Written into the word's `ana` attribute.
- **Paleographic**: an observation about the writing (unclear reading, addition, deletion,
  supplied text, or a feature of the letters), with details such as reason, placement and
  hand. It can apply to the whole word or to selected characters. Written into the word's
  `ana` attribute.

An annotation can cover several words: with the panel open, Shift-click another word to
extend the span. A single-word annotation can be applied to every occurrence of the same
form at once ("Apply to all").

Annotations are saved in the project and written into the XML, in new documents and
imported TEI files alike, so the exported file (the same as the one in the project) holds
everything: notes as `<note>` in their word, categories and observations as attributes
(`ana`, `cert`), character annotations as `<c>`. Annotations on a span of several words
stay in the project only.

## Validation

**Validate** in the toolbar checks the compiled document against a schema:
**TEI P5** (`tei_all`) or **MENOTA P5** (RelaxNG). Errors are listed with their line
numbers where available. In an imported document, the replaced words still to be decided
(see [Replaced words](#replaced-words)) are listed above the results, each with an **Open**
button that opens the word panel on it.

## Importing existing files

**Import** in the toolbar reads a TEI-XML file (`.xml`, `.tei`) or a plain text file
(`.txt`) and replaces the current document with it.

### TEI-XML

The body of the document is converted to shorthand, which you then edit as usual:
line breaks become `//`, page breaks `///`, each paragraph starts with `.p`, and so on.
Lemmas, analyses and normalized forms in the file are taken over, and its header is read
into the [metadata editor](#manuscript-metadata).

After a TEI file is imported, **Transcription of the imported file** shows how the file
encodes its transcription, as read from it: which levels its words have (facsimile,
diplomatic, normalized) and how many of them have each, whether the levels are in a
`<choice>`, how abbreviations and punctuation are encoded, the file's lineations if it has
several, and the markup the shorthand doesn't show (such as `<l>`, or `<hi>` inside a
word), with counts. That markup is kept as it is, also in words you edit. Words you add in the editor
are written with the levels ticked under **Levels**: change them if the file's reading is
wrong, then **Confirm**. Words already in the file are not changed. The choice is saved
with the project.

Some files number their lines in more than one way, such as a charter with the
manuscript's lines (`<lb ed="ms">`) and those of a printed edition (`<lb ed="DN">`). The
shorthand then shows the line breaks of one of them as `//n`, the manuscript's if there is
one; the others are kept in the file as they are. **Lineations** in the same dialog
chooses another one, or all of them; changing it imports the file again.

A file that isn't well-formed XML (an unclosed element, say) is still imported, as the XML
parser repairs it, but the [log](#logs-and-reporting-problems) shows a warning with the parser's first error and
its line. Check the text around that line: the repair may have moved or dropped markup
there.

Each word, numeral and punctuation mark of the file is one word of the shorthand, so that
Saga-Scribe knows which element an edit belongs to. Where the file has two of them with
nothing between them, the shorthand sets them apart: a note by a space (`skrims ^{sic}`,
where `skrims^{sic}` would put the note inside the word), others by `|` (`þꝩi|at`, two words
written together). Characters that would otherwise split a word are escaped (`iðran\;`), and
a `<num>` is written `#…#`.

Markup around words that the shorthand has a sign for is shown around them
([Other markup](#other-markup)): `<s>` as `.s{…}`, `<q>` as `.q{…}`, `<persName>` as
`.psname{…}`, `<hi>` as `*{…}*`, `<seg>` as `&{…}`, `<sic>` and `<corr>` in a `<choice>` as
`.corr{…=…}`, and so on, unless it holds a paragraph. Its attributes are kept (a reading's
witness is shown, `.rdg[AM70]{…}`). Typing such markup around words puts them in a new
element; deleting its start and end takes the element away and leaves its words; changing
its name (`.q{` to `.seg{`) puts the words in an element of the new name, with the old one's
attributes. Markup has to start and end in the same element of the file: around words of
one sentence, say, not from the middle of one sentence into the next. Moving where
existing markup ends, and changing a pair (`.corr{…=…}`), isn't possible in the shorthand
yet. Projects imported before have the markup put into their shorthand when opened.
Inside a word, `<hi>`, `<c>`, `<seg>`, `<surplus>`, `<damage>` and a `<sic>`/`<corr>` pair
are shown where they stand (`*{A}*nnat`, `.c{S}tephan`, `mikill.corr{mikit=mikit}`) when they
cover part of the word; markup over the whole word stays hidden there (it would read as
markup around the word), as does markup on the other levels, which an edit keeps.

An imported document is marked **Imported** in the editor header. In this mode, the
original file is kept and only edited passages are rewritten when the document is saved or
exported:

- Everything you have not edited is written out exactly as it was, byte for byte,
  including the header, comments, namespaces and markup the shorthand cannot express.
- Changes that do not affect the encoding (whitespace in the shorthand, for instance) do
  not change the file.
- An edited word is changed in place, not written anew: markup the shorthand does not show
  (expansions `<ex>`, abbreviation marks `<am>`, editors' attributes, comments) stays with
  its text, a transcription level the edit cannot be placed in (such as a curated
  normalized form) is kept as it was, and no level the word did not have is added.
- A word with a lemma or analysis that you replace with another word is marked until you
  say in the word panel whether it is the same word (see [below](#replaced-words)).
- A word cut and pasted elsewhere is moved as the file has it, with its markup, lemma,
  analysis and annotations. If the same word occurs several times among the words cut
  and pasted in one go, and it isn't clear which went where, the pasted ones are new words.
- Lemmas confirmed in Saga-Scribe are written into the words' `lemma` and `me:msa` (or
  `msa`) attributes; a changed normalized form is written into the word's `<me:norm>`.
  Removing a lemma removes these attributes.
- Notes and other annotations are written into their words, as in a new document (see
  [Annotations](#annotations)); a word without annotations stays as the file has it.
- The word panel's **XML** tab shows a word as the file has it: its levels, attributes and
  the markup in it (such as `<ex>` or `<hi>`), and its XML, which can be edited there for
  anything the shorthand and the other tabs don't cover. Under **Attributes** each attribute
  can be changed, removed or added, and under **Levels** each level's content (as XML); both
  change the XML below. **Apply** sets the word in the
  file and puts its shorthand in the editor; the XML must be a single, well-formed element
  of the same kind (a `<w>` stays a `<w>`). A word added in the shorthand can be edited
  this way once the project has been saved and opened again. (In a new document, any word
  can be: see [Projects and files](#projects-and-files).)
- The word panel's **Markup** tab lists the elements of the file around the word (its
  sentence, quotation, name, paragraph, division), in it (`<hi>`, `<c>`, `<ex>`) and right
  before it (the line or page break that starts its line, a note), each with its
  attributes: change, remove or add them, then **Apply** sets them in the file. Clicking a
  line number or page number in the rendered text opens this tab for the word after the
  break. Attributes the shorthand writes, such as a break's number (`n`), are set in the
  shorthand and shown read-only. Markup added in the shorthand gets its attributes once
  the project has been saved and opened again. New documents don't have the tab's controls
  yet.
- A paragraph can be split by adding `.p` and joined by removing one. A split has to fall
  between elements such as sentences (`<s>`), not inside one.
- If the shorthand contains an error, the document is not saved or exported, rather than
  written incomplete.

### Replaced words

How a word was changed in the editor says little about what was meant: a misread word is
often fixed by selecting it and typing it again, which looks the same as putting another
word in its place. So when a word of the imported file that has a lemma or an analysis is
replaced in the editor, Saga-Scribe asks whether it is still the same word. The word gets
a wavy underline in the editor and in the Text and XSLT views. Click it in a view to open
the word panel, or press **F8**: it opens the panel on the next word marked for review
(**Shift+F8**: the previous one) and shows that word in the view. The **Lemmatize** tab names the word it replaced and that word's lemma;
**The replaced word's XML** shows that word as the file has it, with everything that
belongs to it. Two buttons answer the question:

- **Keep the old lemma**: the word was corrected. It keeps its lemma, analysis and
  normalized form.
- **Set a new lemma**: another word stands in its place. Its lemma, analysis and normalized
  form are removed, and a new lemma can be chosen below.

Choosing a lemma for the word in the panel answers the question as well. Saga-Scribe
suggests an answer, and until you choose, the file follows it: the old word's lemma is kept
if the old and new forms are alike, and removed otherwise. In both cases the transcription
markup the shorthand does not show (such as expansions) stays with the text. Once
answered, the wavy line goes away, and the panel still offers the other answer. The
answers are saved with the project. Words not yet decided stay open after saving and
reopening; **Validate** lists them, and saving or exporting says how many there are.

A replaced word that has no lemma but has annotations of its own (a note or category on
the word, made in the **Annotate** tab) is asked about too, with the buttons **Keep it as
the same word** and **Treat it as a different word**; the answer decides whether it keeps
the old word's normalized form. Annotations are never removed by the answer, as they may
be about the place in the text rather than the word: the panel says the word has some, so
you can check that they still fit. Annotations on a span of several words don't count.

A word's normalized form follows an edit of the word when it is the form Saga-Scribe
would give the word's shorthand. A normalized form that differs (one an editor chose, such
as `maðr` for `maþr`) is kept as the file has it. If the edit changes the word so that the
kept form may no longer fit (`maþr` edited to `maþrr`), the word is marked and asked about
in the same way: the panel says which form was kept, and either button, or a normalized
form entered below, answers it. The same holds for a diplomatic form the edit cannot be
carried into.

A deleted word is not asked about: it is removed with its lemma, and **Undo** brings it
back.

### Plain text

The text is placed in the editor as it is. Add line and page breaks and other markers by
hand.

## Exporting

The **Export** menu in the toolbar offers:

- **TEI-XML**: the complete document, including the header, lemmas, annotations, entity
  references and the MENOTA levels if enabled.
- **HTML**: the document transformed by the stylesheet chosen in [Settings](#settings), as
  a single file with its styles and the Junicode font embedded. It needs no network
  connection or other files.
- **PDF**: the same output, set in A4 pages with margins and page numbers. On Linux and
  Windows it is saved directly to the file you choose; on macOS the system print dialog
  opens, where **Save as PDF** creates the file.
- **Dictionary JSON**: your confirmed word forms with their lemmas, analyses and normalized
  forms, for backup or analysis.

Exports are refused while the shorthand contains errors.

HTML and PDF exports show their progress and can be cancelled. If you switch to another
window while an HTML or PDF export or an import is running, a system notification tells
you when it has finished or failed. On macOS, the system asks once whether Saga-Scribe may
show notifications; this and the notifications themselves can be changed in the system
settings.

## Settings

The **Settings** dialog (gear icon in the toolbar) has these sections:

- **Appearance**: the colour theme (light, dark, or following the system) and the editor's
  font size.
- **Editor**: whether the preview is updated automatically, and how long it waits after
  typing stops.
- **Stylesheets**: the XSLT stylesheet used for the XSLT preview and for HTML and PDF export.
  **Import Stylesheet** adds your own; **Remove Selected** removes it again (the built-in
  stylesheet cannot be removed).
- **Default Template**: the template used for new projects.

## Keyboard shortcuts

On macOS, use **Cmd** instead of **Ctrl**. **F1** opens the help dialog, which lists these
shortcuts and the shorthand.

### Application

| Shortcut         | Action                        |
| ---------------- | ----------------------------- |
| Ctrl+O           | Open project or shorthand file |
| Ctrl+S           | Save project                  |
| Ctrl+Shift+Z     | Undo lemmatization (outside the editor) |
| Ctrl+Shift+Y     | Redo lemmatization            |
| F8 / Shift+F8    | Word panel on the next / previous word marked for review |
| F1 or Ctrl+/     | Help                          |
| Escape           | Close dialog or panel         |
| ← / →            | Previous / next tab (in the preview, the word panel and the help) |

When a dialog or panel opens, the keyboard focus moves into it (to the search field in the
word panel and the entity browser); Tab and Shift+Tab stay within it until it closes, and
focus then returns to where it was, such as the word that opened the word panel.

### Editor

| Shortcut         | Action                                      |
| ---------------- | ------------------------------------------- |
| Ctrl+Z           | Undo                                        |
| Ctrl+Y or Ctrl+Shift+Z | Redo                                  |
| Ctrl+F           | Search and replace                          |
| F3 or Ctrl+G     | Next match                                  |
| Shift+F3 or Ctrl+Shift+G | Previous match                      |
| Ctrl+D           | Select next occurrence of the selection     |
| Alt+↑ / Alt+↓    | Move line up / down                         |

### Folding

Pages can be folded at their page breaks (`///`), using the markers in the editor's
margin or the keyboard:

| Shortcut         | Action                       |
| ---------------- | ---------------------------- |
| Ctrl+Shift+[     | Fold the page at the cursor  |
| Ctrl+Shift+]     | Unfold the page at the cursor |
| Ctrl+Alt+[       | Fold all                     |
| Ctrl+Alt+]       | Unfold all                   |

## Logs and reporting problems

The log button (at the right of the toolbar, before help and settings) shows the application log: loading of
dictionaries and entities, compilation errors, and the results of imports and exports. Its icon
changes to a warning sign when there are errors. Errors in the shorthand are also underlined in the editor.

When reporting a problem, please include the relevant log entries. If Saga-Scribe becomes
slow, start it with the option `--perf-log`; it then records how long each step takes in a
file `perf-<time>.jsonl` in its log directory, which helps locate the cause.

Problems can be reported on
[GitHub](https://github.com/motheatensoul/saga-scribe/issues).
