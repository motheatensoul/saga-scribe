# Saga-Scribe User Guide

This guide explains how to use Saga-Scribe for manuscript transcription.

## Getting Started

Saga-Scribe provides a split-pane interface:
- **Left pane**: DSL Editor for entering your transcription
- **Right pane**: Preview showing rendered text or raw XML output

### Basic Workflow

1. Select a template from the toolbar dropdown
2. Type your transcription using the DSL syntax
3. Click words in the rendered view to add lemma annotations
4. Save your project with Ctrl+S
5. Export the TEI-XML when ready for publication

### Project Files

Saga-Scribe uses `.teis` project archives (ZIP format) that bundle:
- Your DSL source text
- Compiled TEI-XML output
- Per-word lemma confirmations
- Project metadata

This means you never lose your lemmatization work when saving and reopening projects.

## DSL Syntax

The Domain-Specific Language (DSL) uses short notation that compiles to TEI-XML.

### Line and Page Breaks

```
//           Line break (auto-numbered if enabled)
//5          Line break with explicit number
///1v        Page break with folio number
```

### Editorial Marks

```
[...]        Gap/lacuna (illegible text)
[...3]       Gap of approximately 3 characters
[...3<ab>]   Gap with supplied reading
<abc>        Supplied/reconstructed text
-{abc}-      Deleted text
+{abc}+      Added text
?{abc}?      Unclear reading
^{note}      Marginal note
```

### Abbreviations

```
.abbr[abbreviation]{expansion}
```

Example: `.abbr[d.]{deus}` produces an abbreviation mark for "d." expanding to "deus".

### Word Boundaries

```
|            Explicit word boundary
~//          Word continues across line break
~///1v       Word continues across page break
```

### Special Characters

Use entity references for medieval characters:

```
:rrot:       Rotunda r
:thorn:      Thorn (þ)
:eth:        Eth (ð)
:aelig:      Ligature ae (æ)
```

## Entity Browser

Click the "ꝥ" button in the editor header to open the Entity Browser.

### Searching Entities

- Type in the search box to filter by name or description
- Use the category dropdown to browse by type
- Click an entity to select it
- Click "+" to insert the entity at cursor position

### Custom Entity Mappings

When you select an entity, you can customize how it normalizes:

1. Select an entity from the list
2. Edit the "Translation" field to set the diplomatic normalization
3. Press Enter or click away to save
4. Click "Reset" to restore the default mapping

Custom mappings appear with an "custom" badge and affect how entities render in the diplomatic and normalized levels.

## Lemmatizer

Click any word in the rendered text view to open the Lemmatizer.

### Adding Lemma Mappings

1. Click a word in the rendered text
2. Search for the lemma in the ONP dictionary
3. Select the correct headword from results
4. Select the word class (noun, verb, adjective, etc.)
5. Fill in the relevant morphological fields that appear
6. Click "Save"

### Editing Existing Mappings

When you click an existing mapping from the list:
- The form fields populate with the saved values
- You can modify the word class, morphology, or normalized form
- Click "Save" to update the mapping

### Viewing Lemma Info

- **Confirmed words** (green border): This specific word instance has a confirmed lemma
- **Suggested words** (faded border): The dictionary knows this wordform but it hasn't been confirmed for this instance
- Hover over a word to see its lemma and analysis in the tooltip

### MENOTA Morphological Analysis Format (me:msa)

The Lemmatizer uses the MENOTA me:msa format for morphological tagging. Analyses are stored as space-separated name tokens.

#### Word Classes

| Code | Meaning |
|------|---------|
| xNC | Common noun |
| xNP | Proper noun |
| xAJ | Adjective |
| xVB | Verb |
| xPE | Personal pronoun |
| xPR | Reflexive pronoun |
| xPQ | Interrogative pronoun |
| xPI | Indefinite pronoun |
| xDD | Demonstrative determiner |
| xDQ | Quantifying determiner |
| xDP | Possessive determiner |
| xAV | Adverb |
| xAQ | Interrogative adverb |
| xAP | Preposition |
| xCC | Coordinating conjunction |
| xCS | Subordinating conjunction |
| xIT | Interjection |
| xIM | Infinitive marker |

#### Morphological Categories

| Category | Values |
|----------|--------|
| Case | cN (nom), cG (gen), cD (dat), cA (acc) |
| Number | nS (singular), nD (dual), nP (plural) |
| Gender | gM (masc), gF (fem), gN (neut) |
| Definiteness | sI (indefinite), sD (definite) |
| Grade | rP (positive), rC (comparative), rS (superlative) |
| Person | p1, p2, p3 |
| Tense | tPS (present), tPT (preterite) |
| Mood | mIN (indicative), mSU (subjunctive), mIP (imperative) |
| Voice | vA (active), vR (reflexive/middle) |
| Finiteness | fF (finite), fP (participle), fS (supine), fI (infinitive) |

#### Examples

- `xNC cN nS gF` = common noun, nominative, singular, feminine
- `xVB fF p3 nS tPT mIN vA` = verb, finite, 3rd person, singular, preterite, indicative, active
- `xAJ rC cA nP gM` = adjective, comparative, accusative, plural, masculine

### TEI Output

When you save lemma mappings, the TEI output will include `lemma` and `me:msa` attributes on word elements:

```xml
<w lemma="kona" me:msa="xNC cN nP gF">
  <me:facs>konur</me:facs>
  <me:dipl>konur</me:dipl>
  <me:norm>konur</me:norm>
</w>
```

## Templates

Templates define the TEI-XML header and footer that wrap your content.

### Built-in Templates

- **TEI P5**: Standard TEI with minimal header
- **MENOTA**: Full MENOTA-compliant template with multi-level support

### Template Options

- **Word wrapping**: Wrap words in `<w>` and punctuation in `<pc>` tags
- **Auto line numbers**: Automatically number line breaks
- **Multi-level output**: Generate MENOTA `<me:facs>`, `<me:dipl>`, `<me:norm>` levels

### Creating Custom Templates

1. Click the gear icon in the editor header
2. Click "+ Create Template"
3. Fill in the template fields:
   - Name and description
   - Header XML (before content)
   - Footer XML (after content)
   - Options checkboxes
4. Click "Save"

## Multi-Level Transcription

When using a MENOTA template with multi-level output enabled, each word generates three levels:

- **Facsimile (`me:facs`)**: Exact manuscript appearance with entity references
- **Diplomatic (`me:dipl`)**: Resolved entities, expanded abbreviations
- **Normalized (`me:norm`)**: Standardized spelling

### Example Output

Input:
```
:rrot:egn
```

Output:
```xml
<w>
  <me:facs>&rrot;egn</me:facs>
  <me:dipl>regn</me:dipl>
  <me:norm>regn</me:norm>
</w>
```

## Preview Modes

Toggle between views using the buttons in the preview header:

- **Text**: Rendered view with clickable words
- **XML**: Raw TEI-XML output

## Keyboard Shortcuts

### Project

| Shortcut | Action |
|----------|--------|
| Ctrl+S / Cmd+S | Save project (.teis archive) |
| Ctrl+O / Cmd+O | Open project or DSL file |

### Editor

- Standard text editing shortcuts work in the editor
- Tab indents, Shift+Tab unindents
- Ctrl+Z / Cmd+Z for undo

## Error Panel

Click the "☰" button to view the application log. This shows:
- Loading status for dictionaries and entities
- Errors during parsing or compilation
- Debug information

## Tips

1. **Start simple**: Use basic syntax first, add markup incrementally
2. **Use templates**: Let templates handle the XML boilerplate
3. **Check the preview**: Watch the XML output as you type
4. **Build your dictionary**: Lemmatize words as you encounter them
5. **Customize entities**: Set up diplomatic normalizations for your text's conventions
