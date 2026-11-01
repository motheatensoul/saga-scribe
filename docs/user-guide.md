# Saga-Scribe User Guide

This guide describes how to transcribe, annotate and export a manuscript with Saga-Scribe.
For installation, see the [README](../README.md#getting-started).

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
  **Export**); the document (the template in use, the template manager, **Metadata** and
  **Validate**); undo and redo of lemmatization; and, at the right, the application log,
  help and settings, followed by the file's name. In a narrow window the buttons show only
  their icons (hover over one for its name), and groups that don't fit move to a second
  row.
- **Editor** (left): the transcription, written in the shorthand described below. Its header
  has the buttons that act on the text: undo and redo, search and replace, and the entity
  browser (**ꝥ**).
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
- the compiled TEI-XML (`output.xml`);
- confirmed lemmas and analyses, word by word;
- annotations;
- manuscript metadata;
- for imported TEI files, the original XML, so that it can be written back unchanged.

**Open** (**Ctrl+O**) opens a project, or a plain shorthand file (`.dsl` or `.txt`).
**Save** (**Ctrl+S**) saves the project; the first save asks for a location. Projects saved
with older versions of Saga-Scribe can still be opened.

## Transcription shorthand

The shorthand is plain text with inline markers. Each marker corresponds to a TEI element;
the compiler adds the surrounding structure, including the division into words.

### Line and page breaks

```
//           Line break (numbered automatically if the template says so)
//5          Line break with an explicit number
///1v        Page break, with the folio or page number
.p           New paragraph
```

`.p` has to stand on its own between spaces (`word.p` is ordinary text). A paragraph can
begin anywhere, also mid-line: `//4 ſina . .p þer vitið …`. The pilcrow `¶` is a character
written by the scribe (`:para:`), not a paragraph break.

### Editorial marks

```
[...]        Gap (illegible or lost text)
[...3]       Gap of about three characters
[...3<ab>]   Gap with supplied text
<abc>        Supplied text
-{abc}-      Deletion
+{abc}+      Addition
?{abc}?      Unclear reading
^{note}      Note
```

### Abbreviations

```
.abbr[abbreviation]{expansion}
```

For example, `.abbr[kgs]{konungs}` is encoded as
`<choice><abbr>kgs</abbr><expan>konungs</expan></choice>`.

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
  current form and can be saved with or without a lemma.

**Save** confirms the lemma and analysis for this occurrence of the word. **Remove lemma**
removes them again; a normalized form entered by hand is kept.

In the Text view, words with a confirmed lemma are marked; words whose form has a known
analysis that has not been confirmed for this occurrence are marked more faintly. Hovering
over a word shows its lemma and analysis.

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

Annotations are saved in the project. For imported TEI files, see
[Importing existing files](#importing-existing-files).

## Validation

**Validate** in the toolbar checks the compiled document against a schema:
**TEI P5** (`tei_all`) or **MENOTA P5** (RelaxNG). Errors are listed with their line
numbers where available.

## Importing existing files

**Import** in the toolbar reads a TEI-XML file (`.xml`, `.tei`) or a plain text file
(`.txt`) and replaces the current document with it.

### TEI-XML

The body of the document is converted to shorthand, which you then edit as usual:
line breaks become `//`, page breaks `///`, each paragraph starts with `.p`, and so on.
Lemmas, analyses and normalized forms in the file are taken over, and its header is read
into the [metadata editor](#manuscript-metadata).

An imported document is marked **Imported** in the editor header. In this mode, the
original file is kept and only edited passages are rewritten when the document is saved or
exported:

- Everything you have not edited is written out exactly as it was, byte for byte,
  including the header, comments, namespaces and markup the shorthand cannot express.
- Changes that do not affect the encoding (whitespace in the shorthand, for instance) do
  not change the file.
- Lemmas confirmed in Saga-Scribe are written into the words' `lemma` and `me:msa` (or
  `msa`) attributes; a changed normalized form is written into the word's `<me:norm>`.
  Removing a lemma removes these attributes.
- Notes and other annotations are kept in the project only, since they would add markup the
  file does not have.
- A paragraph can be split by adding `.p` and joined by removing one. A split has to fall
  between elements such as sentences (`<s>`), not inside one.
- If the shorthand contains an error, the document is not saved or exported, rather than
  written incomplete.

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
| F1 or Ctrl+/     | Help                          |
| Escape           | Close dialog or panel         |

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
dictionaries and entities, compilation errors, and the results of imports and exports. It
turns red when there are errors. Errors in the shorthand are also underlined in the editor.

When reporting a problem, please include the relevant log entries. If Saga-Scribe becomes
slow, start it with the option `--perf-log`; it then records how long each step takes in a
file `perf-<time>.jsonl` in its log directory, which helps locate the cause.

Problems can be reported on
[GitHub](https://github.com/motheatensoul/saga-scribe/issues).
