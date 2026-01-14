# Implementation Plan: MENOTA Compliance and Import Pipeline

## Current Session Summary

After investigation, I've identified two main issues and a proposed architectural approach.

---

## Issue 1: Missing `<choice>` Tags (Quick Fix)

### Problem
The MENOTA preset is missing `<choice>` tags around the three `<me:...>` elements inside `<w>` and `<pc>` tags.

**Current output:**
```xml
<w lemma="madr" me:msa="xNC cN nS sI">
  <me:facs>mapr</me:facs>
  <me:dipl>mapr</me:dipl>
  <me:norm>madr</me:norm>
</w>
```

**Expected output (MENOTA compliant):**
```xml
<w lemma="madr" me:msa="xNC cN nS sI">
  <choice>
    <me:facs>mapr</me:facs>
    <me:dipl>mapr</me:dipl>
    <me:norm>madr</me:norm>
  </choice>
</w>
```

### Files to Modify

**1. `src-tauri/src/parser/compiler/multi_level.rs`**

Line 40-43 - Change `compile_word_multi_level()`:
```rust
// FROM:
format!(
    "<w{}{}>\n  <me:facs>{}</me:facs>\n  <me:dipl>{}</me:dipl>\n  <me:norm>{}</me:norm>{}\n</w>\n",
    lemma_attrs, ann_attrs, facs_with_chars, dipl, norm, notes
)

// TO:
format!(
    "<w{}{}>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>{}\n</w>\n",
    lemma_attrs, ann_attrs, facs_with_chars, dipl, norm, notes
)
```

Line 63-66 - Change `compile_punctuation_multi_level()`:
```rust
// FROM:
format!(
    "<pc>\n  <me:facs>{}</me:facs>\n  <me:dipl>{}</me:dipl>\n  <me:norm>{}</me:norm>\n</pc>\n",
    facs, dipl, norm
)

// TO:
format!(
    "<pc>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</pc>\n",
    facs, dipl, norm
)
```

**2. `src-tauri/src/parser/tests.rs`**

Tests that check for `<me:facs>` etc. will still pass since they use `contains()`.
However, we should add a test specifically for the `<choice>` wrapper:

```rust
#[test]
fn test_multi_level_choice_wrapper() {
    let input = "test";
    let result = compile_with_options(input, CompileOptions {
        multi_level: true,
        ..Default::default()
    });
    assert!(result.contains("<choice>"));
    assert!(result.contains("</choice>"));
    // Verify structure: choice should wrap the me: elements
    assert!(result.contains("<choice>\n    <me:facs>"));
}
```

### Verification
After changes:
```bash
cd src-tauri && cargo test
cd src-tauri && cargo clippy -- -D warnings
```

---

## Issue 2: Import Pipeline Preservation (Larger Effort)

### Problem
Currently, importing a MENOTA file discards everything outside `<body>`:
- `<front>` and `<back>` matter completely lost
- `<teiHeader>` only partially extracted (specific fields)
- XML declarations, processing instructions, comments lost
- Custom attributes, namespaces lost
- `<revisionDesc>` lost

### Goal
For MENOTA-compliant imports, the output XML should be identical to the input, except for changes made by the user in the transcription body.

### Proposed Architecture: Hybrid Preservation

**Composite Source of Truth Model:**
- **DSL** = source of truth for transcription content
- **Annotation stores** = source of truth for semantic enrichment (lemmas, MSA)
- **Metadata store** = source of truth for structured metadata (editable in UI)
- **Original XML sections** = source of truth for document structure (preserved verbatim)

**New data to store on import:**
```rust
pub struct ImportResult {
    pub dsl: String,
    pub metadata: Option<Metadata>,
    // NEW: preserve original structure
    pub original_preamble: Option<String>,   // <?xml...> to <body> opening
    pub original_postamble: Option<String>,  // </body> to end
    pub front_matter: Option<String>,        // <front>...</front> verbatim
    pub back_matter: Option<String>,         // <back>...</back> verbatim
}
```

**On export:**
1. If `original_preamble` exists AND user hasn't modified metadata -> use original
2. If user modified metadata -> regenerate header, preserve rest of preamble
3. Always preserve `front_matter` and `back_matter` verbatim
4. Compile body from DSL + annotations
5. Append `original_postamble` or template footer

### Implementation Steps

1. **Modify `ImportResult` struct** (`src-tauri/src/importer/tei/mod.rs`)
   - Add `original_preamble`, `original_postamble`, `front_matter`, `back_matter` fields

2. **Update `tei::parse()`** to extract raw XML sections
   - Use string slicing on raw XML content (not DOM) to preserve exact formatting
   - Find `<body` and `</body>` positions
   - Extract front/back matter from DOM but preserve as raw strings

3. **Modify project file format** (`src-tauri/src/commands/file.rs`)
   - Add `original_preamble.xml`, `original_postamble.xml` to ZIP
   - Add `front_matter.xml`, `back_matter.xml` if present
   - Bump manifest version

4. **Update compilation/export flow** (`src-tauri/src/commands/parse.rs`)
   - Accept optional preserved sections
   - Use them instead of template header/footer when present

5. **Frontend: Track metadata modifications**
   - Flag when user edits metadata
   - Pass flag to compilation to decide header regeneration

### Files Involved
- `src-tauri/src/importer/tei/mod.rs` - ImportResult struct, parse function
- `src-tauri/src/importer/tei/dsl.rs` - DSL conversion (minor changes)
- `src-tauri/src/commands/import.rs` - Import command
- `src-tauri/src/commands/file.rs` - Project save/load
- `src-tauri/src/commands/parse.rs` - Compilation
- `src/lib/tauri.ts` - Frontend IPC types
- `src/routes/+page.svelte` - Handle preserved sections

---

## Previous Fix Applied

In this session, I fixed the XML parsing error that was breaking rendering:

**File:** `src/routes/+page.svelte` line 434

Changed error handling from setting invalid XML as `previewContent` to using `errorStore`:
```javascript
// OLD (broken):
previewContent = `Error: ${e}`;

// NEW (fixed):
errorStore.error("Compiler", `Compilation failed: ${e}`);
```

This prevents "character 1 line 1 not being '<'" errors in XML parsing.

---

## Recommended Order of Implementation

1. **Issue 1 first** - Add `<choice>` wrapper (30 min, low risk)
2. **Verify rendering works** after Issue 1 fix
3. **Issue 2** - Import preservation (2-4 hours, medium risk)

---

## Test File

Reference MENOTA file for testing: `static/tests/HolmPerg-34-4to-MLL.xml`
- Body starts at line 657
- Has proper `<choice>` wrappers
- Has `<front>` matter with background, contents, introduction
- Has rich `<teiHeader>` with revisionDesc
