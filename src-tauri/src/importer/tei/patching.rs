//! # Round-Trip Patching System
//!
//! This module computes the minimal set of changes (patches) between the original
//! imported document and the user's edited DSL, then applies those patches to
//! produce TEI-XML.
//!
//! ## Guarantees
//!
//! - **Unchanged input is emitted byte-for-byte.** If the edited DSL has the same
//!   tokens as the import (whitespace-only edits included), the original body is
//!   returned verbatim.
//! - **Edits are spliced into the original source.** When source spans are
//!   available (see [`source_map`](super::source_map)), only the bytes of changed
//!   segments are replaced. Everything else, including attribute order, entity
//!   references and formatting, is left untouched.
//! - **Invalid DSL never reaches the output.** A DSL parse error is returned as an
//!   error instead of being treated as an empty document.
//!
//! ## Patch Computation Algorithm
//!
//! 1. **Fast path**: If edited DSL matches original, return empty patch list
//! 2. **Tokenize**: Convert both original segments and edited DSL to token lists
//! 3. **Diff**: Common prefix/suffix are kept; the middle is diffed with an exact
//!    LCS when small, otherwise anchored on tokens that are unique on both sides
//!    (patience diff) and recursed. Without anchors, the region is replaced
//!    wholesale rather than guessed at.
//! 4. **Pair**: Runs of deletions and insertions are paired into Modify operations
//!    so that edited words keep their original attributes (lemma, me:msa, ...)
//!
//! ```text
//! Original: [A, B, C, D, E]
//! Edited:   [A, B, X, Y, E]
//!
//! Patches:  Keep(A), Keep(B), Modify(C→X), Modify(D→Y), Keep(E)
//! ```

use crate::importer::tei::extraction::segments_to_dsl;
use crate::importer::tei::helpers;
use crate::importer::tei::segments::{ImportedDocument, Segment};
use crate::importer::tei::source_map::{set_tag_attribute, tag_attribute, tag_length};
use crate::identity::{
    carry, diff_range, is_token, matched_tokens, node_to_dsl, spanned_dsl_tokens, Edit, Guidance,
    Tracked,
    PARAGRAPH,
};
use crate::parser::{Compiler, CompilerConfig, LemmaMapping, Lexer, Node, WordKey, WordTokenizer};
use std::ops::Range;
use std::collections::HashMap;

/// A patch operation describing how to transform a segment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PatchOperation {
    #[serde(rename = "keep")]
    Keep { segment_id: usize },
    #[serde(rename = "modify")]
    Modify {
        segment_id: usize,
        new_dsl: String,
        /// The edited token it now is (index into the edited DSL's tokens)
        #[serde(default)]
        token: Option<usize>,
        /// Another word typed in its place, not the word edited: compiled
        /// afresh (in the same place), without the old word's data
        #[serde(default)]
        fresh: bool,
    },
    #[serde(rename = "insert")]
    Insert {
        dsl: String,
        /// The edited token inserted (index into the edited DSL's tokens)
        #[serde(default)]
        token: Option<usize>,
    },
    #[serde(rename = "delete")]
    Delete { segment_id: usize },
}

/// Computes the patch operations needed to transform original segments into edited DSL.
///
/// This is the main entry point for the patching system. It handles:
/// - Fast-path detection (no changes → empty patch list)
/// - Tokenizing both sides through the same DSL pipeline, so that unchanged
///   text always produces identical tokens
/// - Diffing (exact LCS for small regions, patience anchoring for large ones)
/// - Mapping token edits back to segments and pairing deletions with
///   insertions into Modify operations
///
/// # Errors
///
/// Returns an error if the edited DSL does not parse.
#[cfg(test)]
pub fn compute_patches(
    segments: &[Segment],
    dsl_rules: u32,
    edited_dsl: &str,
) -> Result<Vec<PatchOperation>, String> {
    Patcher::new(segments, dsl_rules)
        .compute(edited_dsl, None)
        .map(|(patches, _)| patches)
}

// ============================================================================
// Word identity across edits
// ============================================================================

/// Turns edited DSL into patches against the original document, keeping
/// track of which word is which.
///
/// With [`Guidance`] (the editor's changes since the last compile), tokens
/// outside the changed ranges keep their identity outright, and only tokens
/// in a changed range are matched by comparison, locally. Without it (first
/// compile, or out-of-step guidance), the whole document is compared with
/// the original, which can mix up identical neighbouring words.
pub struct Patcher {
    original_dsl: String,
    original: OriginalTokens,
    /// The original token (the first of its unit) of each segment, by ID
    token_of_segment: HashMap<usize, usize>,
    /// The last committed compile. Original token `i` has ID `i`; tokens
    /// added since import have IDs from the original token count on.
    state: Option<Tracked>,
}

impl Patcher {
    pub fn new(segments: &[Segment], dsl_rules: u32) -> Self {
        let original_dsl = segments_to_dsl(segments, dsl_rules);
        let original = OriginalTokens::build(segments, &original_dsl);
        let token_of_segment = original
            .units
            .iter()
            .flat_map(|unit| unit.segment_ids.iter().map(move |&id| (id, unit.tokens.start)))
            .collect();
        Self { original_dsl, original, token_of_segment, state: None }
    }

    /// The revision of the last committed compile (0 before any).
    pub fn revision(&self) -> u64 {
        self.state.as_ref().map_or(0, |state| state.revision)
    }

    /// Computes patches for `edited_dsl`. The returned state becomes the
    /// basis for the next compile once [`commit`](Self::commit)ted (after
    /// the patches were applied successfully).
    ///
    /// # Errors
    ///
    /// Returns an error if the edited DSL does not parse.
    pub fn compute(
        &self,
        edited_dsl: &str,
        guidance: Option<&Guidance>,
    ) -> Result<(Vec<PatchOperation>, Tracked), String> {
        let (tokens, spans) = spanned_dsl_tokens(edited_dsl)?;
        let original_count = self.original.tokens.len();
        let previous = self.state.as_ref();

        // Unchanged since import (up to whitespace): every token is itself
        if self.original_dsl.trim() == edited_dsl.trim() && tokens == self.original.tokens {
            let known: Vec<Option<u64>> = (0..tokens.len() as u64).map(Some).collect();
            let state = Tracked::with_ids(previous, edited_dsl, tokens, spans, &known, original_count as u64);
            return Ok((Vec::new(), state));
        }

        let carried = carry(previous, guidance, edited_dsl, &tokens, &spans);
        let origin: Vec<Option<usize>> = match &carried {
            // Original tokens are those with an original ID
            Some(ids) => ids
                .iter()
                .map(|id| id.and_then(|id| usize::try_from(id).ok()).filter(|&i| i < original_count))
                .collect(),
            None => {
                let a: Vec<&str> = self.original.tokens.iter().map(String::as_str).collect();
                let b: Vec<&str> = tokens.iter().map(String::as_str).collect();
                let mut edits = Vec::with_capacity(a.len().max(b.len()));
                diff_range(&a, &b, 0, 0, &mut edits);
                let mut origin = vec![None; tokens.len()];
                for (j, i) in matched_tokens(&edits, 0, &a, &b) {
                    origin[j] = Some(i);
                }
                origin
            }
        };
        let origin = increasing(origin);

        // IDs: the original token's, or the carried one of content added since
        // import; anything else is new
        let known: Vec<Option<u64>> = (0..tokens.len())
            .map(|j| match origin[j] {
                Some(i) => Some(i as u64),
                None => carried
                    .as_ref()
                    .and_then(|ids| ids[j])
                    .filter(|&id| id >= original_count as u64),
            })
            .collect();

        let edits = edits_from_origin(&origin, &tokens, &self.original.tokens);
        let patches = replace_in_place(self.original.to_segment_ops(&edits, &tokens));
        let state = Tracked::with_ids(previous, edited_dsl, tokens, spans, &known, original_count as u64);
        Ok((patches, state))
    }

    /// The key of each `<w>` of `rendered` (see [`WordKey`]): the ID of the
    /// token it comes from (as tracked in `state`) and its position among
    /// that token's words. A word copied from the original belongs to the
    /// original token of the segment it is in; original token `i` has ID `i`.
    pub fn word_keys(&self, doc: &ImportedDocument, rendered: &RenderedBody, state: &Tracked) -> Vec<WordKey> {
        // Segments with a token, by where they are in the original body
        let mut located: Vec<(usize, usize, usize)> = doc
            .spans
            .iter()
            .flatten()
            .enumerate()
            .filter_map(|(id, span)| {
                let (start, end) = (*span)?;
                self.token_of_segment.contains_key(&id).then_some((start, end, id))
            })
            .collect();
        located.sort_unstable();
        let segment_at = |offset: usize| {
            let i = located.partition_point(|(start, _, _)| *start <= offset).checked_sub(1)?;
            let (_, end, id) = located[i];
            (offset < end).then_some(id)
        };

        let mut counts: HashMap<u64, u32> = HashMap::new();
        let mut unknown = 0u32;
        rendered
            .words
            .iter()
            .map(|(_, origin)| {
                let id = match *origin {
                    Origin::Source(offset) => segment_at(offset).and_then(|s| self.token_of_segment.get(&s)).map(|&t| t as u64),
                    Origin::Segment(segment) => self.token_of_segment.get(&segment).map(|&t| t as u64),
                    Origin::Token(Some(j)) => state.ids.get(j).copied(),
                    Origin::Token(None) => None,
                };
                match id {
                    Some(id) => {
                        let k = counts.entry(id).or_default();
                        *k += 1;
                        (id, *k - 1)
                    }
                    // Not from a token (e.g. a <w> in markup the DSL doesn't show)
                    None => {
                        unknown += 1;
                        (u64::MAX, unknown - 1)
                    }
                }
            })
            .collect()
    }

    /// Makes `state` the basis for the next compile.
    pub fn commit(&mut self, state: Tracked) {
        self.state = Some(state);
    }
}

/// Drops identities that would reorder the original (text moved elsewhere
/// counts as deleted and inserted), so patches stay in document order.
fn increasing(mut origin: Vec<Option<usize>>) -> Vec<Option<usize>> {
    let mut last: Option<usize> = None;
    for slot in origin.iter_mut() {
        if let Some(i) = *slot {
            if last.is_some_and(|last| i <= last) {
                *slot = None;
            } else {
                last = Some(i);
            }
        }
    }
    origin
}

/// The edits from the original tokens to `tokens`, given which original
/// token each one is (`origin`, increasing).
fn edits_from_origin(origin: &[Option<usize>], tokens: &[String], original: &[String]) -> Vec<Edit> {
    let mut edits = Vec::with_capacity(origin.len().max(original.len()));
    let mut next = 0;
    for (j, slot) in origin.iter().enumerate() {
        match *slot {
            Some(i) => {
                edits.extend((next..i).map(Edit::Delete));
                edits.push(if tokens[j] == original[i] { Edit::Keep(i) } else { Edit::Replace(i, j) });
                next = i + 1;
            }
            None => edits.push(Edit::Insert(j)),
        }
    }
    edits.extend((next..original.len()).map(Edit::Delete));
    edits
}

/// A minimal group of original segments and DSL tokens that cover exactly the
/// same text. Usually one segment ↔ one token, but a multi-word `<w>` is one
/// segment ↔ several tokens, and a word followed by entity punctuation
/// (`no:rrot:eg:slong::middot:`) is several segments ↔ one token.
#[derive(Debug)]
struct Unit {
    segment_ids: Vec<usize>,
    tokens: std::ops::Range<usize>,
}

/// The original document as DSL tokens, produced by the same pipeline as the
/// edited DSL, plus the mapping from tokens back to segments.
#[derive(Debug)]
struct OriginalTokens {
    tokens: Vec<String>,
    units: Vec<Unit>,
    unit_of_token: Vec<usize>,
}

impl OriginalTokens {
    fn build(segments: &[Segment], original_dsl: &str) -> Self {
        let per_segment: Vec<(usize, Vec<String>)> = segments
            .iter()
            .filter_map(|seg| {
                let text = segment_token_text(seg)?;
                let tokens = parse_dsl_to_tokens(&text).unwrap_or_else(|_| vec![text]);
                (!tokens.is_empty()).then(|| (seg.id(), tokens))
            })
            .collect();

        if let Ok(whole) = parse_dsl_to_tokens(original_dsl) {
            if let Some(aligned) = Self::align(&per_segment, whole) {
                return aligned;
            }
        }
        log::warn!("Could not align imported segments with DSL tokens; using per-segment tokens");
        Self::per_segment(per_segment)
    }

    /// Groups whole-document tokens and per-segment tokens into units whose
    /// concatenated text is identical and whose ends fall on segment boundaries.
    fn align(per_segment: &[(usize, Vec<String>)], whole: Vec<String>) -> Option<Self> {
        let flat: Vec<(usize, &str)> = per_segment
            .iter()
            .flat_map(|(id, tokens)| tokens.iter().map(move |t| (*id, t.as_str())))
            .collect();

        let mut units = Vec::new();
        let mut unit_of_token = Vec::with_capacity(whole.len());
        let (mut p, mut q) = (0, 0);

        while p < flat.len() || q < whole.len() {
            let unit_start = q;
            let mut segment_ids: Vec<usize> = Vec::new();
            let (mut acc_seg, mut acc_doc) = (String::new(), String::new());

            loop {
                let continues_segment = p < flat.len()
                    && segment_ids.last() == Some(&flat[p].0)
                    && acc_seg.len() >= acc_doc.len();
                let take_segment = p < flat.len()
                    && (acc_seg.len() < acc_doc.len() || acc_seg.is_empty() || continues_segment);

                if take_segment {
                    let (id, token) = flat[p];
                    if segment_ids.last() != Some(&id) {
                        segment_ids.push(id);
                    }
                    acc_seg.push_str(token);
                    p += 1;
                } else if q < whole.len() {
                    acc_doc.push_str(&whole[q]);
                    q += 1;
                } else {
                    return None;
                }

                if !(acc_doc.starts_with(&acc_seg) || acc_seg.starts_with(&acc_doc)) {
                    return None;
                }
                let at_segment_boundary = p >= flat.len() || segment_ids.last() != Some(&flat[p].0);
                if !acc_seg.is_empty() && acc_seg == acc_doc && at_segment_boundary {
                    break;
                }
            }

            unit_of_token.extend(std::iter::repeat_n(units.len(), q - unit_start));
            units.push(Unit {
                segment_ids,
                tokens: unit_start..q,
            });
        }

        Some(Self {
            tokens: whole,
            units,
            unit_of_token,
        })
    }

    /// One unit per segment: used when the whole-document tokens cannot be aligned.
    fn per_segment(per_segment: Vec<(usize, Vec<String>)>) -> Self {
        let mut tokens = Vec::new();
        let mut units = Vec::new();
        let mut unit_of_token = Vec::new();
        for (id, seg_tokens) in per_segment {
            let start = tokens.len();
            unit_of_token.extend(std::iter::repeat_n(units.len(), seg_tokens.len()));
            tokens.extend(seg_tokens);
            units.push(Unit {
                segment_ids: vec![id],
                tokens: start..tokens.len(),
            });
        }
        Self {
            tokens,
            units,
            unit_of_token,
        }
    }

    /// Converts token-level edits into segment-level patch operations.
    ///
    /// A unit whose tokens are all kept (with no insertions between them) keeps
    /// all its segments. A changed unit's first segment is modified to the new
    /// token text; any further segments in the unit are deleted.
    fn to_segment_ops(&self, edits: &[Edit], edited: &[String]) -> Vec<PatchOperation> {
        let mut ops = Vec::new();
        let mut k = 0;

        while k < edits.len() {
            let first_token = match edits[k] {
                Edit::Insert(j) => {
                    ops.push(PatchOperation::Insert {
                        dsl: edited[j].clone(),
                        token: Some(j),
                    });
                    k += 1;
                    continue;
                }
                Edit::Keep(i) | Edit::Delete(i) | Edit::Replace(i, _) => i,
            };

            let unit = &self.units[self.unit_of_token[first_token]];
            let unit_len = unit.tokens.len();

            // Collect this unit's edits: its tokens plus insertions strictly between them.
            let mut end = k;
            let mut seen = 0;
            while end < edits.len() && seen < unit_len {
                if matches!(edits[end], Edit::Keep(_) | Edit::Delete(_) | Edit::Replace(..)) {
                    seen += 1;
                }
                end += 1;
            }
            let unit_edits = &edits[k..end];
            k = end;

            if unit_edits.iter().all(|e| matches!(e, Edit::Keep(_))) {
                ops.extend(
                    unit.segment_ids
                        .iter()
                        .map(|&segment_id| PatchOperation::Keep { segment_id }),
                );
                continue;
            }

            let new_tokens: Vec<&str> = unit_edits
                .iter()
                .filter_map(|e| match *e {
                    Edit::Keep(i) => Some(self.tokens[i].as_str()),
                    Edit::Insert(j) | Edit::Replace(_, j) => Some(edited[j].as_str()),
                    Edit::Delete(_) => None,
                })
                .collect();

            let token = unit_edits.iter().find_map(|e| match *e {
                Edit::Insert(j) | Edit::Replace(_, j) => Some(j),
                _ => None,
            });
            let mut segment_ids = unit.segment_ids.iter().copied();
            if !new_tokens.is_empty() {
                if let Some(segment_id) = segment_ids.next() {
                    ops.push(PatchOperation::Modify {
                        segment_id,
                        new_dsl: new_tokens.join(" "),
                        token,
                        fresh: false,
                    });
                }
            }
            ops.extend(segment_ids.map(|segment_id| PatchOperation::Delete { segment_id }));
        }

        ops
    }
}

/// The one DSL token an imported element reads as (words, punctuation,
/// breaks, paragraph starts), for checking both readings agree.
#[cfg(test)]
pub fn segment_dsl_token(seg: &Segment) -> Option<String> {
    let text = segment_token_text(seg)?;
    let tokens = parse_dsl_to_tokens(&text).ok()?;
    match tokens.as_slice() {
        [token] => Some(token.clone()),
        _ => Some(format!("<{} tokens: {}>", tokens.len(), text)),
    }
}

/// The tokens the patcher diffs, read from DSL.
#[cfg(test)]
pub fn dsl_tokens(dsl: &str) -> Result<Vec<String>, String> {
    parse_dsl_to_tokens(dsl)
}

/// The DSL text a segment contributes to the token stream, if any.
fn segment_token_text(seg: &Segment) -> Option<String> {
    match seg {
        Segment::Word { dsl_content, .. } | Segment::Punctuation { dsl_content, .. } => {
            (!dsl_content.trim().is_empty()).then(|| dsl_content.clone())
        }
        Segment::LineBreak { attributes, .. } => Some(match attributes.get("n") {
            Some(num) => format!("//{}", num),
            None => "//".to_string(),
        }),
        Segment::PageBreak { attributes, .. } => Some(match attributes.get("n") {
            Some(num) => format!("///{}", num),
            None => "///".to_string(),
        }),
        Segment::Paragraph { .. } => Some(PARAGRAPH.to_string()),
        _ => None,
    }
}

fn parse_dsl_to_tokens(dsl: &str) -> Result<Vec<String>, String> {
    let nodes = parse_dsl_nodes(dsl)?;
    Ok(nodes.iter().filter(|node| is_token(node)).map(node_to_dsl).collect())
}

fn parse_dsl_nodes(dsl: &str) -> Result<Vec<Node>, String> {
    let doc = Lexer::new(dsl).parse()?;
    Ok(WordTokenizer::new().tokenize(doc.nodes))
}

// ============================================================================
// Rendering
// ============================================================================

/// Turns a deleted segment and a word inserted in its place into one
/// replacement in the same place (instead of deleting it and inserting the
/// new word next to it). These are different words, so the replacement is
/// fresh: it doesn't inherit the old word's lemma or normalized form.
fn replace_in_place(ops: Vec<PatchOperation>) -> Vec<PatchOperation> {
    let mut result = Vec::with_capacity(ops.len());
    let mut deletes = Vec::new();
    let mut inserts = Vec::new();

    let flush = |result: &mut Vec<PatchOperation>,
                 deletes: &mut Vec<usize>,
                 inserts: &mut Vec<(String, Option<usize>)>| {
        let paired = deletes.len().min(inserts.len());
        for (segment_id, (new_dsl, token)) in deletes.drain(..paired).zip(inserts.drain(..paired)) {
            result.push(PatchOperation::Modify { segment_id, new_dsl, token, fresh: true });
        }
        result.extend(deletes.drain(..).map(|segment_id| PatchOperation::Delete { segment_id }));
        result.extend(inserts.drain(..).map(|(dsl, token)| PatchOperation::Insert { dsl, token }));
    };

    for op in ops {
        match op {
            PatchOperation::Delete { segment_id } => deletes.push(segment_id),
            PatchOperation::Insert { dsl, token } => inserts.push((dsl, token)),
            other => {
                flush(&mut result, &mut deletes, &mut inserts);
                result.push(other);
            }
        }
    }
    flush(&mut result, &mut deletes, &mut inserts);
    result
}

/// Compiler settings matching an imported document's own encoding style:
/// MENOTA three-level words, plain `<w>` words, or unwrapped text.
pub fn compiler_config_for(doc: &ImportedDocument) -> CompilerConfig {
    let word_wrap = doc.is_menota
        || doc.segments.iter().any(|seg| {
            matches!(seg, Segment::Word { original_xml, .. } if is_w_element(original_xml))
        });
    CompilerConfig {
        word_wrap,
        auto_line_numbers: false,
        multi_level: doc.is_menota,
        wrap_pages: false,
    }
}

/// Renders the body XML for an imported document after applying `patches`.
///
/// - No effective changes: returns `original_body` verbatim.
/// - Source spans available: splices changes into `original_body`.
/// - Otherwise: falls back to rebuilding the body from segments, which preserves
///   structure but not the exact original formatting.
///
/// # Errors
///
/// Returns an error if a `.p` edit can't be applied without breaking the XML
/// (see [`check_paragraph_edits`]).
pub fn render_body(
    doc: &ImportedDocument,
    original_body: Option<&str>,
    patches: &[PatchOperation],
    compiler: &mut Compiler,
) -> Result<RenderedBody, String> {
    let unchanged = patches
        .iter()
        .all(|p| matches!(p, PatchOperation::Keep { .. }));

    if let Some(body) = original_body {
        if unchanged {
            let mut out = Output::with_capacity(body.len());
            out.push(body, Origin::Source(0));
            return Ok(out.finish(Vec::new()));
        }
    }

    let patches = separate_paragraph_edits(&doc.segments, patches);
    check_paragraph_edits(&doc.segments, &patches)?;

    if let (Some(body), Some(spans)) = (original_body, &doc.spans) {
        if let Some(rendered) = splice_into_source(body, &doc.segments, spans, &patches, compiler) {
            return Ok(rendered);
        }
        log::warn!("Source spans inconsistent with patches; falling back to reconstruction");
    }

    // Rebuilding inserts at segment boundaries only, which may not be where
    // the split belongs (after closing tags); merges are still fine
    if patches
        .iter()
        .any(|p| matches!(p, PatchOperation::Insert { dsl, .. } if dsl == PARAGRAPH))
    {
        return Err(
            "Paragraphs can't be split in this file: its layout couldn't be mapped to the original source"
                .to_string(),
        );
    }
    Ok(reconstruct(&doc.segments, &patches, compiler))
}

/// The rewritten `<body>` of an imported document.
#[derive(Debug)]
pub struct RenderedBody {
    pub xml: String,
    /// Byte offsets (in `xml`) of edited `<w>` elements whose lemma, msa and
    /// normalized form were kept from the original and need a review.
    pub edited_words: Vec<usize>,
    /// Every `<w>` element: its offset in `xml` and where it came from
    pub words: Vec<(usize, Origin)>,
}

impl RenderedBody {
    /// The edited words as word numbers: the count of `<w>` elements before
    /// each, the numbering the previews and annotations use.
    pub fn edited_word_indices(&self) -> Vec<u32> {
        let starts: Vec<usize> = self.words.iter().map(|(at, _)| *at).collect();
        self.edited_words
            .iter()
            .map(|&offset| starts.partition_point(|&start| start < offset) as u32)
            .collect()
    }
}

/// Rewrites patches so that paragraph breaks are only ever kept, deleted or
/// inserted on their own: a paragraph start tag is never "modified" into a
/// word, and a word is never "modified" into a paragraph break.
fn separate_paragraph_edits(segments: &[Segment], patches: &[PatchOperation]) -> Vec<PatchOperation> {
    let paragraphs: std::collections::HashSet<usize> = segments
        .iter()
        .filter(|s| matches!(s, Segment::Paragraph { .. }))
        .map(Segment::id)
        .collect();

    let mut out = Vec::with_capacity(patches.len());
    for patch in patches {
        let PatchOperation::Modify { segment_id, new_dsl, token, fresh } = patch else {
            out.push(patch.clone());
            continue;
        };
        let is_paragraph = paragraphs.contains(segment_id);
        let tokens = parse_dsl_to_tokens(new_dsl).unwrap_or_else(|_| vec![new_dsl.clone()]);
        if !is_paragraph && !tokens.iter().any(|t| t == PARAGRAPH) {
            out.push(patch.clone());
            continue;
        }
        let insert = |dsl: &String| PatchOperation::Insert { dsl: dsl.clone(), token: *token };

        if is_paragraph {
            // The first `.p` keeps the original tag; everything else is new
            match tokens.iter().position(|t| t == PARAGRAPH) {
                Some(k) => {
                    out.extend(tokens[..k].iter().map(insert));
                    out.push(PatchOperation::Keep { segment_id: *segment_id });
                    out.extend(tokens[k + 1..].iter().map(insert));
                }
                None => {
                    out.push(PatchOperation::Delete { segment_id: *segment_id });
                    out.extend(tokens.iter().map(insert));
                }
            }
        } else {
            // Breaks before the first other token go before the segment, its
            // first run of other tokens modifies it, the rest follows
            let lead = tokens.iter().take_while(|t| *t == PARAGRAPH).count();
            let run = tokens[lead..].iter().take_while(|t| *t != PARAGRAPH).count();
            out.extend(tokens[..lead].iter().map(insert));
            if run == 0 {
                out.push(PatchOperation::Delete { segment_id: *segment_id });
            } else {
                out.push(PatchOperation::Modify {
                    segment_id: *segment_id,
                    new_dsl: tokens[lead..lead + run].join(" "),
                    token: *token,
                    fresh: *fresh,
                });
            }
            out.extend(tokens[lead + run..].iter().map(insert));
        }
    }
    out
}

/// Where paragraph breaks can be inserted or removed, from the document's
/// element nesting.
struct ParagraphStructure<'a> {
    /// Position of each segment in document order, by ID
    position: HashMap<usize, usize>,
    /// The innermost open element after each segment (by position)
    innermost_after: Vec<Option<&'a str>>,
}

impl<'a> ParagraphStructure<'a> {
    fn new(segments: &'a [Segment]) -> Self {
        let mut stack: Vec<&str> = Vec::new();
        let mut innermost_after = Vec::with_capacity(segments.len());
        for segment in segments {
            match segment {
                Segment::Paragraph { .. } => stack.push("p"),
                Segment::Structural { xml, .. } => {
                    if xml.starts_with("</") {
                        stack.pop();
                    } else if !xml.starts_with("<!") && !xml.starts_with("<?") && !xml.ends_with("/>") {
                        let name = xml[1..]
                            .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                            .next()
                            .unwrap_or_default();
                        stack.push(name.rsplit(':').next().unwrap_or(name));
                    }
                }
                _ => {}
            }
            innermost_after.push(stack.last().copied());
        }
        let position = segments.iter().enumerate().map(|(i, s)| (s.id(), i)).collect();
        Self { position, innermost_after }
    }

    /// Where a paragraph break after segment `id` goes: right after it if that
    /// is directly inside a paragraph, else after the end tags that follow it
    /// (`maþr</s>` splits after `</s>`). Returns the segment whose end is the
    /// split point, or the innermost element if there is none.
    fn split_point_after(&self, segments: &[Segment], id: usize) -> Result<usize, Option<&'a str>> {
        let Some(&at) = self.position.get(&id) else { return Err(None) };
        if self.innermost_after[at] == Some("p") {
            return Ok(id);
        }
        for (j, segment) in segments.iter().enumerate().skip(at + 1) {
            match segment {
                Segment::Whitespace { .. } => {}
                Segment::Structural { xml, .. } if xml.starts_with("</") => {
                    if self.innermost_after[j] == Some("p") {
                        return Ok(segment.id());
                    }
                }
                _ => break,
            }
        }
        Err(self.innermost_after[at])
    }

    /// The `</p>` segment that closes the paragraph before paragraph `id`, if
    /// only whitespace lies between them.
    fn preceding_close<'s>(&self, segments: &'s [Segment], id: usize) -> Option<&'s Segment> {
        let at = *self.position.get(&id)?;
        segments[..at]
            .iter()
            .rev()
            .find(|s| !matches!(s, Segment::Whitespace { .. }))
            .filter(|s| matches!(s, Segment::Structural { xml, .. } if xml == "</p>" || xml.ends_with(":p>")))
    }
}

/// What stays between two merged paragraphs: the whitespace that separated
/// their tags, or a line break if there was none (words must stay apart).
fn separator(between: &str) -> &str {
    if between.is_empty() { "\n" } else { between }
}

/// The DSL of a segment, for naming it in error messages.
fn describe_segment(segment: Option<&Segment>) -> String {
    match segment.and_then(segment_token_text) {
        Some(text) => format!("\"{}\"", text.trim()),
        None => "the start".to_string(),
    }
}

/// Checks that every inserted `.p` falls directly inside a paragraph (so it
/// can split it) and that every removed `.p` directly follows another
/// paragraph (so the two can be merged).
///
/// # Errors
///
/// Describes the first `.p` edit that can't be applied.
fn check_paragraph_edits(segments: &[Segment], patches: &[PatchOperation]) -> Result<(), String> {
    let touches_paragraphs = patches.iter().any(|p| match p {
        PatchOperation::Insert { dsl, .. } => dsl == PARAGRAPH,
        PatchOperation::Delete { segment_id } => segments
            .iter()
            .any(|s| s.id() == *segment_id && matches!(s, Segment::Paragraph { .. })),
        _ => false,
    });
    if !touches_paragraphs {
        return Ok(());
    }

    let structure = ParagraphStructure::new(segments);
    let by_id: HashMap<usize, &Segment> = segments.iter().map(|s| (s.id(), s)).collect();
    let mut anchor: Option<usize> = None;
    for patch in patches {
        match patch {
            PatchOperation::Insert { dsl, .. } if dsl == PARAGRAPH => {
                let Some(after) = anchor else {
                    return Err("A paragraph break (.p) can't come before the first imported paragraph".to_string());
                };
                if let Err(inside) = structure.split_point_after(segments, after) {
                    let place = match inside {
                        Some(name) => format!("inside <{}>", name),
                        None => "outside any paragraph".to_string(),
                    };
                    return Err(format!(
                        "The paragraph break (.p) after {} can't be placed: that point is {}, not directly in a paragraph",
                        describe_segment(by_id.get(&after).copied()),
                        place
                    ));
                }
            }
            PatchOperation::Insert { .. } => {}
            PatchOperation::Keep { segment_id }
            | PatchOperation::Modify { segment_id, .. }
            | PatchOperation::Delete { segment_id } => {
                if matches!(patch, PatchOperation::Delete { .. })
                    && matches!(by_id.get(segment_id), Some(Segment::Paragraph { .. }))
                    && structure.preceding_close(segments, *segment_id).is_none()
                {
                    return Err(format!(
                        "The paragraph break (.p) after {} can't be removed: it starts the first paragraph of its section",
                        describe_segment(anchor.and_then(|id| by_id.get(&id).copied()))
                    ));
                }
                anchor = Some(*segment_id);
            }
        }
    }
    Ok(())
}

/// Where a stretch of the rewritten body came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Origin {
    /// Copied from the original body, starting at this offset
    Source(usize),
    /// Serialized from this original segment (rebuilding without spans)
    Segment(usize),
    /// Compiled from this edited token (index into the edited DSL's tokens)
    Token(Option<usize>),
}

/// The rewritten body as it is built, with where each stretch came from.
struct Output {
    xml: String,
    /// (offset in `xml`, origin) for each stretch, in order
    origins: Vec<(usize, Origin)>,
}

impl Output {
    fn with_capacity(capacity: usize) -> Self {
        Self { xml: String::with_capacity(capacity), origins: Vec::new() }
    }

    /// Copies `body[range]`; `None` if it isn't a valid range of `body`.
    fn copy(&mut self, body: &str, range: Range<usize>) -> Option<()> {
        let text = body.get(range.clone())?;
        self.push(text, Origin::Source(range.start));
        Some(())
    }

    fn generate(&mut self, text: &str, token: Option<usize>) {
        self.push(text, Origin::Token(token));
    }

    /// Drops everything from `len` on (and the origins of what's dropped).
    fn truncate(&mut self, len: usize) {
        self.xml.truncate(len);
        self.origins.retain(|(start, _)| *start < len);
    }

    fn push(&mut self, text: &str, origin: Origin) {
        if !text.is_empty() {
            self.origins.push((self.xml.len(), origin));
            self.xml.push_str(text);
        }
    }

    /// The finished body, with the origin of each of its `<w>` elements.
    fn finish(self, edited_words: Vec<usize>) -> RenderedBody {
        let words = word_starts(&self.xml)
            .into_iter()
            .map(|at| {
                let region = self.origins.partition_point(|(start, _)| *start <= at).saturating_sub(1);
                let origin = match self.origins.get(region) {
                    // A copied word: where its start tag is in the original
                    Some((start, Origin::Source(source))) => Origin::Source(source + (at - start)),
                    Some((_, origin)) => *origin,
                    None => Origin::Token(None),
                };
                (at, origin)
            })
            .collect();
        RenderedBody { xml: self.xml, edited_words, words }
    }
}

/// `rendered`'s XML with confirmed lemmas (by word number) written into the
/// words' start tags, as `lemma` and the analysis as `me:msa` (or `msa`,
/// whichever the word or document uses). Only values that differ from what
/// the tag already says (references resolved) are written, so lemmas read
/// from the file leave it byte-identical. An empty lemma is a lemma the user
/// removed: the word's lemma and analysis attributes are removed. A
/// normalized form, if given and different from the word's `<me:norm>` text,
/// replaces that text.
pub fn write_lemmas(rendered: &RenderedBody, lemmas: &HashMap<u32, LemmaMapping>, menota: bool) -> String {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (&number, mapping) in lemmas {
        let Some(&(at, _)) = rendered.words.get(number as usize) else { continue };
        let Some(len) = tag_length(&rendered.xml[at..]) else { continue };
        let tag = &rendered.xml[at..at + len];
        if let Some(norm) = mapping.normalized.as_deref() {
            edits.extend(norm_edit(&rendered.xml, at + len, norm));
        }

        let mut new_tag = tag.to_string();
        if mapping.lemma.is_empty() {
            for name in ["lemma", "me:msa", "msa"] {
                new_tag = set_tag_attribute(&new_tag, name, None);
            }
            if new_tag != tag {
                edits.push((at, len, new_tag));
            }
            continue;
        }
        if tag_attribute(tag, "lemma").as_deref() != Some(mapping.lemma.as_str()) {
            new_tag = set_tag_attribute(&new_tag, "lemma", Some(&mapping.lemma));
        }
        let msa_name = if tag_attribute(tag, "me:msa").is_some() {
            "me:msa"
        } else if tag_attribute(tag, "msa").is_some() || !menota {
            "msa"
        } else {
            "me:msa"
        };
        // The same tags in another order are the same analysis: left as written
        let same_msa = tag_attribute(tag, msa_name)
            .is_some_and(|written| crate::dictionary::same_analysis(&written, &mapping.msa));
        if !mapping.msa.is_empty() && !same_msa {
            new_tag = set_tag_attribute(&new_tag, msa_name, Some(&mapping.msa));
        }
        if new_tag != tag {
            edits.push((at, len, new_tag));
        }
    }

    // Back to front, so earlier offsets stay valid
    edits.sort_unstable_by_key(|(at, _, _)| std::cmp::Reverse(*at));
    let mut xml = rendered.xml.clone();
    for (at, len, tag) in edits {
        xml.replace_range(at..at + len, &tag);
    }
    xml
}

/// The edit setting the `<me:norm>` text of the word whose start tag ends at
/// `from` to `norm`, if it says something else. Its text is compared as the
/// app reads it on import: tags removed, whitespace collapsed, entity
/// references as written. Line, page and column breaks inside it are kept.
fn norm_edit(xml: &str, from: usize, norm: &str) -> Option<(usize, usize, String)> {
    let norm = norm.split_whitespace().collect::<Vec<_>>().join(" ");
    let word = &xml[from..from + xml[from..].find("</w>")?];
    let open = word.find("<me:norm")?;
    let open_len = tag_length(&word[open..])?;
    if word[open..open + open_len].ends_with("/>") {
        return None;
    }
    let start = open + open_len;
    let end = start + word[start..].find("</me:norm>")?;
    let inner = &word[start..end];

    let mut text = String::new();
    let mut breaks = String::new();
    let mut rest = inner;
    while let Some(lt) = rest.find('<') {
        text.push_str(&rest[..lt]);
        let len = tag_length(&rest[lt..])?;
        let tag = &rest[lt..lt + len];
        if ["<lb", "<pb", "<cb"].iter().any(|name| {
            tag.starts_with(name) && tag[name.len()..].starts_with([' ', '/', '>'])
        }) {
            breaks.push_str(tag);
        }
        rest = &rest[lt + len..];
    }
    text.push_str(rest);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if norm.is_empty() || text == norm || helpers::escape_xml_text(&norm) == text {
        return None;
    }
    Some((from + start, inner.len(), helpers::escape_xml_text(&norm) + &breaks))
}

/// Offsets of the `<w>` start tags in `xml`.
fn word_starts(xml: &str) -> Vec<usize> {
    xml.match_indices("<w")
        .map(|(i, _)| i)
        .filter(|&i| {
            xml[i + 2..]
                .chars()
                .next()
                .is_some_and(|c| c == '>' || c == '/' || c.is_whitespace())
        })
        .collect()
}

/// Applies patches by replacing only the source bytes of affected segments.
///
/// Returns `None` if a referenced segment has no span or spans are not in
/// document order; the caller then falls back to reconstruction.
fn splice_into_source(
    body: &str,
    segments: &[Segment],
    spans: &[Option<(usize, usize)>],
    patches: &[PatchOperation],
    compiler: &mut Compiler,
) -> Option<RenderedBody> {
    let by_id: HashMap<usize, &Segment> = segments.iter().map(|s| (s.id(), s)).collect();
    let mut edited_words = Vec::new();
    let structure = ParagraphStructure::new(segments);

    let mut out = Output::with_capacity(body.len() + 1024);
    let mut copied_to = 0;
    // End of the most recent segment touched by a patch; insertions go here.
    let mut anchor: Option<usize> = None;
    let mut anchor_segment: Option<usize> = None;
    // Insertions before the first segment, waiting for a position.
    let mut pending: Vec<(String, Option<usize>)> = Vec::new();

    for patch in patches {
        match patch {
            PatchOperation::Insert { dsl, token } => {
                // Splits the paragraph (checked by `check_paragraph_edits`)
                let split = dsl == PARAGRAPH;
                let fragment = if split {
                    "</p>\n<p>".to_string()
                } else {
                    render_insert(dsl, compiler)
                };
                if split {
                    let after = anchor_segment
                        .and_then(|id| structure.split_point_after(segments, id).ok())?;
                    // Later insertions follow the split, inside the new paragraph
                    anchor = Some((*spans.get(after)?)?.1);
                }
                match anchor {
                    Some(at) => {
                        out.copy(body, copied_to..at)?;
                        copied_to = at;
                        if !split {
                            out.generate("\n", None);
                        }
                        out.generate(fragment.trim_end(), *token);
                    }
                    None => pending.push((fragment, *token)),
                }
            }
            PatchOperation::Keep { segment_id }
            | PatchOperation::Modify { segment_id, .. }
            | PatchOperation::Delete { segment_id } => {
                let (start, end) = (*spans.get(*segment_id)?)?;
                if start < copied_to || end > body.len() {
                    return None;
                }

                if !pending.is_empty() {
                    out.copy(body, copied_to..start)?;
                    copied_to = start;
                    for (fragment, token) in pending.drain(..) {
                        out.generate(fragment.trim_end(), token);
                        out.generate("\n", None);
                    }
                }

                match patch {
                    PatchOperation::Modify { new_dsl, token, fresh, .. } => {
                        let segment = by_id.get(segment_id)?;
                        let source = body.get(start..end)?;
                        let replacement = if *fresh {
                            render_insert(new_dsl, compiler)
                        } else {
                            render_modify(segment, Some(source), new_dsl, compiler)
                        };
                        out.copy(body, copied_to..start)?;
                        if !fresh && keeps_word_data(segment, new_dsl) {
                            edited_words.push(out.xml.len());
                        }
                        out.generate(replacement.trim_end(), *token);
                        copied_to = end;
                    }
                    PatchOperation::Delete { .. } => {
                        if let Some(Segment::Paragraph { .. }) = by_id.get(segment_id) {
                            // Merge with the previous paragraph: drop its `</p>`
                            // and this start tag, keeping what separates them
                            let close = structure.preceding_close(segments, *segment_id)?;
                            let (close_start, close_end) = (*spans.get(close.id())?)?;
                            if close_start < copied_to || close_end > start {
                                return None;
                            }
                            out.copy(body, copied_to..close_start)?;
                            // What separated them (words must stay apart)
                            if close_end < start {
                                out.copy(body, close_end..start)?;
                            } else {
                                out.generate("\n", None);
                            }
                        } else {
                            out.copy(body, copied_to..start)?;
                        }
                        copied_to = end;
                    }
                    _ => {}
                }
                anchor = Some(end);
                anchor_segment = Some(*segment_id);
            }
        }
    }

    if !pending.is_empty() {
        // Nothing to anchor to (empty body): insert before the closing body tag.
        let close = body.rfind("</").filter(|&i| i >= copied_to)?;
        out.copy(body, copied_to..close)?;
        copied_to = close;
        for (fragment, token) in pending.drain(..) {
            out.generate("\n", None);
            out.generate(fragment.trim_end(), token);
        }
        out.generate("\n", None);
    }

    out.copy(body, copied_to..body.len())?;
    Some(out.finish(edited_words))
}

/// The kind of the first token in a DSL fragment.
enum TokenKind {
    Word,
    Punctuation,
    LineBreak(Option<String>),
    PageBreak(Option<String>),
    Other,
}

fn token_kind(dsl: &str) -> TokenKind {
    let nodes = match parse_dsl_nodes(dsl) {
        Ok(nodes) => nodes,
        Err(_) => return TokenKind::Other,
    };
    match nodes.into_iter().next() {
        Some(Node::Word(_)) => TokenKind::Word,
        Some(Node::Punctuation(_)) => TokenKind::Punctuation,
        Some(Node::LineBreak(n)) => TokenKind::LineBreak(n),
        Some(Node::PageBreak(n)) => TokenKind::PageBreak(Some(n).filter(|n| !n.is_empty())),
        _ => TokenKind::Other,
    }
}

/// Whether a modification keeps an imported word's lemma, msa and norm (an
/// edited `<w>` that is still a word); such words are reported for review.
fn keeps_word_data(segment: &Segment, new_dsl: &str) -> bool {
    matches!(segment, Segment::Word { original_xml, .. } if is_w_element(original_xml))
        && matches!(token_kind(new_dsl), TokenKind::Word)
}

/// The content of the first element named `level` (any namespace prefix),
/// e.g. the `<me:norm>` of a MENOTA word; empty for `<me:norm/>`.
fn level_content<'a>(xml: &'a str, level: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(lt) = xml[from..].find('<').map(|i| from + i) {
        from = lt + 1;
        let tag = &xml[lt + 1..];
        let name_len = tag
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(tag.len());
        let qname = &tag[..name_len];
        if qname.rsplit(':').next() != Some(level) {
            continue;
        }
        let tag_end = lt + 1 + tag.find('>')?;
        if xml[..tag_end].ends_with('/') {
            return Some("");
        }
        let close = format!("</{}>", qname);
        let end = xml[tag_end + 1..].find(&close)? + tag_end + 1;
        return Some(&xml[tag_end + 1..end]);
    }
    None
}

/// `xml` without empty `<lb/>` and `<pb/>` elements (any prefix or attributes).
fn without_breaks(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let tag = &rest[lt..];
        let end = tag.find('>').map_or(tag.len(), |i| i + 1);
        let name = tag[1..]
            .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .next()
            .unwrap_or_default();
        let local = name.rsplit(':').next().unwrap_or(name);
        let is_break = (local == "lb" || local == "pb") && tag[..end].ends_with("/>");
        if !is_break {
            out.push_str(&tag[..end]);
        }
        rest = &tag[end..];
    }
    out.push_str(rest);
    out
}

/// Whether a segment's original XML is a `<w>` element (as opposed to bare text
/// or another inline element stored as a word segment).
fn is_w_element(original_xml: &str) -> bool {
    original_xml
        .strip_prefix("<w")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c == '>' || c == '/' || c.is_whitespace())
}

/// Compiles newly inserted DSL in the style of the document (word-wrapped or not).
fn render_insert(dsl: &str, compiler: &mut Compiler) -> String {
    if compiler.config().word_wrap {
        compiler.compile_fragment_from_dsl(dsl)
    } else {
        compiler.compile_inline_from_dsl(dsl)
    }
}

/// Compiles a modified segment. Same-kind edits keep the original element's
/// attributes; a change of kind (e.g. word → line break) is compiled as new content.
///
/// `source` is the segment's original XML text when known, letting break edits
/// rewrite only the `n` attribute in place.
fn render_modify(
    segment: &Segment,
    source: Option<&str>,
    new_dsl: &str,
    compiler: &mut Compiler,
) -> String {
    match (segment, token_kind(new_dsl)) {
        (
            Segment::Word {
                attributes,
                original_xml,
                ..
            },
            TokenKind::Word,
        ) if is_w_element(original_xml) => {
            // Lemma, msa (attributes) and the curated norm are kept; facs and
            // dipl follow the edit. The word is reported as edited for review.
            let original = source.unwrap_or(original_xml);
            let norm = compiler
                .config()
                .multi_level
                .then(|| level_content(original, "norm"))
                .flatten()
                .map(without_breaks);
            compiler.compile_word_from_dsl(new_dsl, attributes, norm.as_deref())
        }
        (Segment::Punctuation { .. }, TokenKind::Punctuation) => {
            compiler.compile_punctuation_from_dsl(new_dsl)
        }
        (Segment::LineBreak { attributes, .. }, TokenKind::LineBreak(n)) => {
            rewrite_break("lb", source, attributes, n.as_deref())
        }
        (Segment::PageBreak { attributes, .. }, TokenKind::PageBreak(n)) => {
            rewrite_break("pb", source, attributes, n.as_deref())
        }
        _ => render_insert(new_dsl, compiler),
    }
}

fn rewrite_break(
    name: &str,
    source: Option<&str>,
    attributes: &HashMap<String, String>,
    n: Option<&str>,
) -> String {
    match source {
        Some(tag) => set_tag_attribute(tag, "n", n),
        None => {
            let mut attrs = attributes.clone();
            match n {
                Some(value) => attrs.insert("n".to_string(), value.to_string()),
                None => attrs.remove("n"),
            };
            format_tag(name, &attrs)
        }
    }
}

/// Applies patch operations by rebuilding TEI-XML from segments.
///
/// This is the fallback used when source spans are unavailable. It preserves
/// document structure but re-serializes kept segments, so formatting details
/// (attribute order, empty-element style) may differ from the original file.
///
/// # Reconstruction Strategy
///
/// - **Keep**: Emit the segment's stored XML
/// - **Modify**: Compile new DSL to XML, preserving original word attributes
/// - **Delete**: Skip the segment
/// - **Insert**: Compile new DSL to XML (no original attributes available)
/// - **Structural**: Always emit verbatim (divs, paragraphs, etc.)
/// - **Whitespace**: Always emit verbatim (formatting preservation)
#[cfg(test)]
pub fn apply_patches_and_reconstruct(
    segments: &[Segment],
    patches: &[PatchOperation],
    compiler: &mut Compiler,
) -> String {
    reconstruct(segments, patches, compiler).xml
}

fn reconstruct(segments: &[Segment], patches: &[PatchOperation], compiler: &mut Compiler) -> RenderedBody {
    let mut out = Output::with_capacity(0);
    let mut edited_words = Vec::new();
    let mut current_patch_idx = 0;

    // 1. Process segments in order
    for seg in segments {
        if matches!(
            seg,
            Segment::Word { .. }
                | Segment::Punctuation { .. }
                | Segment::LineBreak { .. }
                | Segment::PageBreak { .. }
                | Segment::Paragraph { .. }
        ) {
            let seg_id = seg.id();
            let mut handled = false;

            // Interleave leading insertions
            while let Some(patch) = patches.get(current_patch_idx) {
                match patch {
                    PatchOperation::Insert { dsl, .. } if dsl == PARAGRAPH => {
                        out.generate("</p>\n<p>", None);
                        current_patch_idx += 1;
                    }
                    PatchOperation::Insert { dsl, token } => {
                        out.generate(&render_insert(dsl, compiler), *token);
                        current_patch_idx += 1;
                    }
                    PatchOperation::Keep { segment_id }
                    | PatchOperation::Modify { segment_id, .. }
                    | PatchOperation::Delete { segment_id } => {
                        if *segment_id != seg_id {
                            // This patch belongs to a later segment.
                            break;
                        }
                        match patch {
                            PatchOperation::Keep { .. } => {
                                out.push(&serialize_original_segment(seg), Origin::Segment(seg_id));
                            }
                            PatchOperation::Modify { new_dsl, token, fresh, .. } => {
                                if *fresh {
                                    out.generate(&render_insert(new_dsl, compiler), *token);
                                } else {
                                    if keeps_word_data(seg, new_dsl) {
                                        edited_words.push(out.xml.len());
                                    }
                                    out.generate(&render_modify(seg, None, new_dsl, compiler), *token);
                                }
                            }
                            PatchOperation::Delete { .. } if matches!(seg, Segment::Paragraph { .. }) => {
                                // Merge with the previous paragraph: drop its `</p>`
                                if let Some(close) = out.xml.rfind("</p>") {
                                    if out.xml[close + 4..].trim().is_empty() {
                                        let between = out.xml[close + 4..].to_string();
                                        out.truncate(close);
                                        out.generate(separator(&between), None);
                                    }
                                }
                            }
                            _ => {}
                        }
                        current_patch_idx += 1;
                        handled = true;
                        break; // Finished this segment
                    }
                }
            }

            if !handled {
                out.push(&serialize_original_segment(seg), Origin::Segment(seg_id));
            }
        } else {
            // Structural or whitespace segment
            out.push(&serialize_original_segment(seg), Origin::Segment(seg.id()));
        }
    }

    // 2. Consume any trailing insertions
    for patch in &patches[current_patch_idx.min(patches.len())..] {
        if let PatchOperation::Insert { dsl, token } = patch {
            if dsl == PARAGRAPH {
                out.generate("</p>\n<p>", None);
            } else {
                out.generate(&render_insert(dsl, compiler), *token);
            }
        }
    }

    out.finish(edited_words)
}

fn serialize_original_segment(seg: &Segment) -> String {
    match seg {
        Segment::Structural { xml, .. } | Segment::Paragraph { xml, .. } => xml.clone(),
        Segment::Word { original_xml, .. } => original_xml.clone(),
        Segment::Punctuation { original_xml, .. } => original_xml.clone(),
        Segment::LineBreak { attributes, .. } => format_tag("lb", attributes),
        Segment::PageBreak { attributes, .. } => format_tag("pb", attributes),
        Segment::HandShift { attributes, .. } => format_tag("handShift", attributes),
        Segment::Whitespace { content, .. } => content.clone(),
    }
}

fn format_tag(name: &str, attrs: &HashMap<String, String>) -> String {
    let mut s = format!("<{}", name);
    let mut sorted: Vec<_> = attrs.iter().collect();
    sorted.sort();
    for (k, v) in sorted {
        s.push_str(&format!(" {}=\"{}\"", k, helpers::escape_xml_attr(v)));
    }
    s.push_str("/>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(s: &str) -> Vec<&str> {
        s.split_whitespace().collect()
    }

    fn run(a: &str, b: &str) -> Vec<Edit> {
        let (a, b) = (tokens(a), tokens(b));
        let mut out = Vec::new();
        diff_range(&a, &b, 0, 0, &mut out);
        out
    }

    #[test]
    fn test_diff_lcs_small() {
        assert_eq!(
            run("a b c d e", "a b x y e"),
            vec![
                Edit::Keep(0),
                Edit::Keep(1),
                Edit::Delete(2),
                Edit::Delete(3),
                Edit::Insert(2),
                Edit::Insert(3),
                Edit::Keep(4),
            ]
        );
    }

    #[test]
    fn test_diff_large_region_uses_anchors() {
        // Both edits far apart with a large unchanged middle: everything in the
        // middle must be kept, not modified positionally.
        let middle: Vec<String> = (0..5000).map(|i| format!("w{}", i)).collect();
        let a = format!("first {} last", middle.join(" "));
        let b = format!("FIRST {} last extra", middle.join(" "));
        let edits = run(&a, &b);
        let changed = edits.iter().filter(|e| !matches!(e, Edit::Keep(_))).count();
        assert_eq!(changed, 3, "expected delete+insert of first word and one insert");
    }

    #[test]
    fn test_is_w_element() {
        assert!(is_w_element("<w>a</w>"));
        assert!(is_w_element(r#"<w lemma="x">a</w>"#));
        assert!(!is_w_element("<wx>a</wx>"));
        assert!(!is_w_element("word"));
    }
}
