---
Hybrid Overlay Implementation Plan
Overview
Goal: Import TEI-XML files and allow DSL-based editing while preserving 100% of the original XML structure for round-trip fidelity.
Key Decisions:
- Edit facs only, regenerate dipl/norm on compilation
- Inline line breaks shown as frosto~//8þingsbókar 
- Subtle "Imported" badge in UI
- XML stored as source of truth with segment manifest
---
Architecture Diagram
┌─────────────────────────────────────────────────────────────────┐
│                       IMPORT (TEI-XML → Segments)                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Original XML ──► Segment Parser ──► Segment Manifest           │
│       │                                    │                     │
│       │                                    ├── Structural[0]: "<div type="text">"
│       │                                    ├── Word[1]: {attrs, dsl: ".abbr[...]"}:
│       │                                    ├── LineBreak[2]: {n: "2"}
│       │                                    ├── Structural[3]: "</p><p>"
│       │                                    └── ...               │
│       │                                    │                     │
│       └─────────────────────────────┐      │                     │
│                                     ▼      ▼                     │
│                              ┌──────────────────┐                │
│                              │  Project File    │                │
│                              │  (.saga-scribe)  │                │
│                              ├──────────────────┤                │
│                              │ original.xml     │ ◄── Verbatim   │
│                              │ segments.json    │ ◄── Manifest   │
│                              │ source.dsl       │ ◄── Extracted  │
│                              │ metadata.json    │                │
│                              │ annotations.json │                │
│                              └──────────────────┘                │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│                       EDIT (DSL View)                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Editor displays extracted DSL:                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ ///8r                                                        ││
│  │ //1 .abbr[kn:bar:gr]{konongr} ok                            ││
│  │ //2 frosto~//8þingsbókar                                    ││
│  │ //3 ...                                                      ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
│  User edits like normal DSL. Structural elements invisible.     │
│  [Imported] badge shown in status bar.                          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│                       COMPILE (Segments → XML)                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Parse edited DSL                                             │
│  2. Diff DSL tokens against segment manifest                    │
│  3. For each segment:                                            │
│     - Structural → output verbatim (no change)                  │
│     - Word/Punct unchanged → output original XML element        │
│     - Word/Punct modified → compile DSL → new <w>/<pc> element  │
│     - Word/Punct deleted → skip                                  │
│     - New token (no segment) → compile DSL → new element        │
│  4. Reconstruct full XML with preserved structure               │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
---
Phase 1: Segment Data Model (Backend)
New file: src-tauri/src/importer/tei/segments.rs
/// Represents a segment of the document - either structural XML or editable content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Segment {
    /// Structural XML preserved verbatim (div, p, s, head, supplied, comments, etc.)
    Structural {
        id: usize,
        xml: String,  // Exact XML fragment: "<div type=\"chapter\" n=\"1\">"
    },
    
    /// A word element with editable content
    Word {
        id: usize,
        original_xml: String,           // Full <w>...</w> for reference
        attributes: HashMap<String, String>,  // lemma, me:msa, etc.
        dsl_content: String,            // DSL representation of facs content
        has_inline_lb: bool,            // Contains line break inside
    },
    
    /// Punctuation element
    Punctuation {
        id: usize,
        original_xml: String,
        dsl_content: String,
    },
    
    /// Line break (can be standalone or mid-word, tracked in Word if inline)
    LineBreak {
        id: usize,
        attributes: HashMap<String, String>,  // ed, n, rend
    },
    
    /// Page break
    PageBreak {
        id: usize,
        attributes: HashMap<String, String>,
    },
    
    /// Hand shift marker
    HandShift {
        id: usize,
        attributes: HashMap<String, String>,
    },
    
    /// Whitespace between elements (for formatting preservation)
    Whitespace {
        id: usize,
        content: String,
    },
}
/// The complete imported document manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedDocument {
    pub segments: Vec<Segment>,
    pub is_menota: bool,  // Has me:facs/dipl/norm structure
}
---
Phase 2: Segment Extraction (Backend)
New file: src-tauri/src/importer/tei/extraction.rs
Key functions:
- extract_segments(body_node: &Node) -> Vec<Segment>: Walk body, create segments
- word_to_dsl(word_node: &Node) -> String: Extract facs-level content as DSL
- punctuation_to_dsl(pc_node: &Node) -> String: Extract punctuation as DSL
- extract_attributes(node: &Node) -> HashMap<String, String>: Get all attributes
Extraction logic:
For each node in body (depth-first):
  - If structural element (div, p, s, head, supplied, etc.):
    → Emit Structural segment with opening tag
    → Recurse into children
    → Emit Structural segment with closing tag
  
  - If <w> element:
    → Extract attributes (lemma, me:msa, etc.)
    → Find <me:facs> child (or direct content if single-level)
    → Convert facs content to DSL notation
    → Check for inline <lb> elements
    → Emit Word segment
  
  - If <pc> element:
    → Extract facs-level punctuation
    → Emit Punctuation segment
  
  - If <lb>, <pb>, <handShift>:
    → Emit corresponding milestone segment
  
  - If text node (whitespace between elements):
    → Emit Whitespace segment
  
  - If comment:
    → Emit Structural segment with "<!--...-->"
DSL conversion for facs content:
| TEI Element | DSL Output |
|-------------|------------|
| Plain text | text |
| <am>&bar;</am> | :bar: |
| <add>x</add> | +{x}+ |
| <del>x</del> | -{x}- |
| <unclear>x</unclear> | ?{x}? |
| <c type="initial">X</c> | X (preserve as text, log attribute for later) |
| <lb n="8"/> inside word | ~//8 (continuation notation) |
---
Phase 3: DSL Assembly
New function in extraction.rs:
pub fn segments_to_dsl(segments: &[Segment]) -> String {
    let mut dsl = String::new();
    
    for segment in segments {
        match segment {
            Segment::Word { dsl_content, .. } => {
                dsl.push_str(dsl_content);
                dsl.push(' ');
            },
            Segment::Punctuation { dsl_content, .. } => {
                dsl.push_str(dsl_content);
                dsl.push(' ');
            },
            Segment::LineBreak { attributes, .. } => {
                if let Some(n) = attributes.get("n") {
                    dsl.push_str(&format!("//{} ", n));
                } else {
                    dsl.push_str("// ");
                }
            },
            Segment::PageBreak { attributes, .. } => {
                if let Some(n) = attributes.get("n") {
                    dsl.push_str(&format!("///{}\n", n));
                } else {
                    dsl.push_str("///\n");
                }
            },
            Segment::HandShift { attributes, .. } => {
                // Preserve as structural (not shown in DSL)
                // Or add DSL notation if desired
            },
            Segment::Structural { .. } | Segment::Whitespace { .. } => {
                // Skip - not shown in DSL
            },
        }
    }
    
    dsl
}
---
Phase 4: Patching System (Backend)
New file: src-tauri/src/importer/tei/patching.rs
Key challenge: Matching edited DSL tokens back to original segments.
Approach: Token-by-token comparison using a variant of the Patience Diff algorithm:
1. Parse edited DSL → Get list of tokens with positions
2. Build token list from segments → Original tokens with segment IDs
3. Find longest common subsequences → Unchanged tokens
4. Identify modifications:
   - Token at same position but different content → Modify segment
   - Token in edited but not original → Insert new segment
   - Token in original but not edited → Mark segment as deleted
pub enum PatchOperation {
    Keep { segment_id: usize },
    Modify { segment_id: usize, new_dsl: String },
    Insert { position: usize, dsl: String },
    Delete { segment_id: usize },
}
pub fn compute_patches(
    segments: &[Segment],
    edited_dsl: &str
) -> Vec<PatchOperation> {
    // 1. Extract tokens from segments (with IDs)
    // 2. Parse edited DSL into tokens
    // 3. Diff and generate operations
}
---
Phase 5: XML Reconstruction (Backend)
New function in patching.rs:
pub fn apply_patches_and_reconstruct(
    segments: &[Segment],
    patches: &[PatchOperation],
    compiler: &Compiler,
) -> String {
    let mut xml = String::new();
    
    for segment in segments.iter() {
        match find_patch_for_segment(segment.id(), patches) {
            Some(PatchOperation::Delete { .. }) => {
                // Skip this segment
            },
            Some(PatchOperation::Modify { new_dsl, .. }) => {
                // Compile new DSL and output
                match segment {
                    Segment::Word { attributes, .. } => {
                        let compiled = compiler.compile_word_from_dsl(&new_dsl, attributes);
                        xml.push_str(&compiled);
                    },
                    // Similar for Punctuation
                }
            },
            None | Some(PatchOperation::Keep { .. }) => {
                // Output original
                match segment {
                    Segment::Structural { xml: s, .. } => xml.push_str(s),
                    Segment::Word { original_xml, .. } => xml.push_str(original_xml),
                    Segment::Punctuation { original_xml, .. } => xml.push_str(original_xml),
                    Segment::LineBreak { attributes, .. } => {
                        xml.push_str(&format_lb(attributes));
                    },
                    // etc.
                }
            },
        }
    }
    
    // Handle insertions at appropriate positions
    
    xml
}
---
Phase 6: Project File Updates (Backend)
Modify: src-tauri/src/commands/file.rs
Update ProjectData:
pub struct ProjectData {
    // ... existing fields ...
    
    /// Imported document manifest (if this was an imported file)
    pub imported_document: Option<ImportedDocument>,
    
    /// Original body XML (for imported files only)
    pub original_body_xml: Option<String>,
}
Update ZIP format (version 1.4):
- original_body.xml - Verbatim body XML for imported files
- segments.json - Segment manifest
---
Phase 7: Tauri Command Updates (Backend)
Modify: src-tauri/src/commands/import.rs
#[derive(Serialize)]
pub struct ImportResult {
    pub dsl: String,
    pub metadata: Option<Metadata>,
    pub segments: Option<Vec<Segment>>,        // NEW
    pub original_body_xml: Option<String>,     // NEW
    pub is_imported_mode: bool,                // NEW
    // ... existing preservation fields ...
}
New command in src-tauri/src/commands/compile.rs:
#[tauri::command(async)]
pub async fn compile_imported(
    edited_dsl: String,
    segments_json: String,
    original_body_xml: String,
    preamble: Option<String>,
    postamble: Option<String>,
) -> Result<String, String> {
    // 1. Deserialize segments
    // 2. Compute patches
    // 3. Apply patches to reconstruct body
    // 4. Combine with preamble/postamble
}
---
Phase 8: Frontend Updates
Modify: src/lib/tauri.ts
export interface ImportResult {
    dsl: string;
    metadata: Metadata | null;
    segments: Segment[] | null;          // NEW
    originalBodyXml: string | null;      // NEW
    isImportedMode: boolean;             // NEW
    // ... existing ...
}
export interface ProjectData {
    // ... existing ...
    importedDocument: ImportedDocument | null;
    originalBodyXml: string | null;
}
New store: src/lib/stores/imported.svelte.ts
interface ImportedDocumentStore {
    segments: Segment[];
    originalBodyXml: string;
    isActive: boolean;  // Is this an imported project?
}
// Functions:
// - setImported(doc): Set from import
// - clear(): Reset for new/native projects  
// - getSegments(): Get segment list
Modify: src/routes/+page.svelte
1. In compileOnly():
      if (importedStore.isActive) {
       // Use compile_imported instead of compile_dsl
       output = await compileImported(
           editor.content,
           JSON.stringify(importedStore.getSegments()),
           importedStore.originalBodyXml,
           preservationStore.preamble,
           preservationStore.postamble
       );
   } else {
       // Existing compile_dsl flow
   }
   
2. In status bar area, add subtle badge:
      {#if importedStore.isActive}
       <span class="imported-badge">Imported</span>
   {/if}
   
---
Phase 9: Testing
New test file: src-tauri/src/importer/tei/tests.rs
Test cases:
1. test_segment_extraction_basic - Extract segments from simple body
2. test_segment_extraction_nested_divs - Handle deeply nested structure
3. test_segment_extraction_inline_lb - Line breaks inside words
4. test_segment_extraction_comments - XML comments preserved
5. test_dsl_extraction_abbreviations - Convert <am> to :entity:
6. test_dsl_extraction_add_del - Convert additions/deletions
7. test_patching_no_changes - Round-trip with no edits
8. test_patching_word_modification - Edit a word, verify diff
9. test_patching_word_insertion - Insert new word between existing
10. test_patching_word_deletion - Delete word, verify removal
11. test_round_trip_holmperg - Full round-trip with test file
---
File Summary
New Rust files:
- src-tauri/src/importer/tei/segments.rs - Data model
- src-tauri/src/importer/tei/extraction.rs - Segment extraction
- src-tauri/src/importer/tei/patching.rs - Patch computation & application
- src-tauri/src/importer/tei/tests.rs - Test suite
Modified Rust files:
- src-tauri/src/importer/tei/mod.rs - Wire up new modules
- src-tauri/src/commands/file.rs - Update ProjectData, save/load segments
- src-tauri/src/commands/import.rs - Return segments from import
- src-tauri/src/commands/parse.rs - Add compile_imported command
- src-tauri/src/parser/compiler/mod.rs - Add compile_word_from_dsl helper
New TypeScript files:
- src/lib/stores/imported.svelte.ts - Imported document state
Modified TypeScript files:
- src/lib/tauri.ts - Update interfaces, add compileImported()
- src/lib/composables/useFileOperations.svelte.ts - Handle imported state
- src/routes/+page.svelte - Compile routing, UI badge
---
Estimated Effort
| Phase | Description | Effort |
|-------|-------------|--------|
| 1 | Data model | Small |
| 2 | Segment extraction | Large |
| 3 | DSL assembly | Small |
| 4 | Patching system | Large |
| 5 | XML reconstruction | Medium |
| 6 | Project file updates | Small |
| 7 | Tauri commands | Medium |
| 8 | Frontend updates | Medium |
| 9 | Testing | Medium |
Total: Significant undertaking (~3-5 days of focused work)
---
Open Questions / Risks
1. Whitespace preservation: XML parsers may normalize whitespace. Need to handle carefully to avoid spurious diffs.
2. Entity references: Need to preserve &bar; as literal, not resolve to Unicode.
3. Namespace handling: me:facs etc. need proper namespace handling in reconstruction.
4. Inline line breaks mid-word: The frosto~//8þingsbókar notation works for display, but on reconstruction we need to reinsert the <lb> at the right position within the <me:facs>.
5. Block-level <add>/<del>: When <add> wraps entire <w> elements, treated as structural (preserved).
---
