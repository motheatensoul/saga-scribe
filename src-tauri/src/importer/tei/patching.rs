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
use crate::importer::tei::source_map::set_tag_attribute;
use crate::parser::{Compiler, CompilerConfig, Lexer, Node, WordTokenizer};
use std::collections::HashMap;

/// A patch operation describing how to transform a segment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PatchOperation {
    #[serde(rename = "keep")]
    Keep { segment_id: usize },
    #[serde(rename = "modify")]
    Modify { segment_id: usize, new_dsl: String },
    #[serde(rename = "insert")]
    Insert { dsl: String },
    #[serde(rename = "delete")]
    Delete { segment_id: usize },
}

/// The DSL token for a paragraph break.
const PARAGRAPH: &str = ".p";

/// Largest middle region (original × edited tokens) diffed with an exact LCS table.
const LCS_CELL_LIMIT: usize = 4_000_000;

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
pub fn compute_patches(
    segments: &[Segment],
    edited_dsl: &str,
) -> Result<Vec<PatchOperation>, String> {
    // Fast-path: if the edited DSL matches the original extraction, keep everything.
    let original_dsl = segments_to_dsl(segments);
    if original_dsl.trim() == edited_dsl.trim() {
        return Ok(Vec::new());
    }

    let edited = parse_dsl_to_tokens(edited_dsl)?;
    let original = OriginalTokens::build(segments, &original_dsl);

    let a: Vec<&str> = original.tokens.iter().map(String::as_str).collect();
    let b: Vec<&str> = edited.iter().map(String::as_str).collect();
    let mut edits = Vec::with_capacity(a.len().max(b.len()));
    diff_range(&a, &b, 0, 0, &mut edits);

    Ok(pair_modifications(original.to_segment_ops(&edits, &edited)))
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
                    });
                    k += 1;
                    continue;
                }
                Edit::Keep(i) | Edit::Delete(i) => i,
            };

            let unit = &self.units[self.unit_of_token[first_token]];
            let unit_len = unit.tokens.len();

            // Collect this unit's edits: its tokens plus insertions strictly between them.
            let mut end = k;
            let mut seen = 0;
            while end < edits.len() && seen < unit_len {
                if matches!(edits[end], Edit::Keep(_) | Edit::Delete(_)) {
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
                    Edit::Insert(j) => Some(edited[j].as_str()),
                    Edit::Delete(_) => None,
                })
                .collect();

            let mut segment_ids = unit.segment_ids.iter().copied();
            if !new_tokens.is_empty() {
                if let Some(segment_id) = segment_ids.next() {
                    ops.push(PatchOperation::Modify {
                        segment_id,
                        new_dsl: new_tokens.join(" "),
                    });
                }
            }
            ops.extend(segment_ids.map(|segment_id| PatchOperation::Delete { segment_id }));
        }

        ops
    }
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
    Ok(nodes
        .iter()
        .filter(|node| {
            matches!(
                node,
                Node::Word(_)
                    | Node::Punctuation(_)
                    | Node::LineBreak(_)
                    | Node::PageBreak(_)
                    | Node::Head(_)
                    | Node::SuppliedBlock(_)
                    | Node::Norm(_)
                    | Node::ParagraphBreak
            )
        })
        .map(node_to_dsl)
        .collect())
}

fn parse_dsl_nodes(dsl: &str) -> Result<Vec<Node>, String> {
    let doc = Lexer::new(dsl).parse()?;
    Ok(WordTokenizer::new().tokenize(doc.nodes))
}

fn node_to_dsl(node: &Node) -> String {
    match node {
        Node::Text(t) => t.clone(),
        Node::LineBreak(n) => {
            if let Some(num) = n {
                format!("//{}", num)
            } else {
                "//".to_string()
            }
        }
        Node::PageBreak(n) => format!("///{}", n),
        Node::Abbreviation { abbr, expansion } => format!(".abbr[{}]{{{}}}", abbr, expansion),
        Node::Gap { quantity, supplied } => {
            let mut s = "[...".to_string();
            if let Some(q) = quantity {
                s.push_str(&q.to_string());
            }
            if let Some(ref supp) = supplied {
                s.push('<');
                s.push_str(supp);
                s.push('>');
            }
            s.push(']');
            s
        }
        Node::Supplied(t) => format!("<{}>", t),
        Node::SuppliedBlock(t) => format!(".supplied{{{}}}", t),
        Node::Norm(t) => format!(".norm{{{}}}", t),
        Node::Deletion(t) => format!("-{{{}}}-", t),
        Node::Addition(t) => format!("+{{{}}}+", t),
        Node::Note(t) => format!("^{{{}}}", t),
        Node::Head(t) => format!(".head{{{}}}", t),
        Node::Unclear(t) => format!("?{{{}}}?", t),
        Node::Entity(name) => format!(":{}:", name),
        Node::WordContinuation => "~".to_string(),
        Node::CompoundJoin => "~".to_string(),
        Node::WordBoundary => "|".to_string(),
        Node::ParagraphBreak => PARAGRAPH.to_string(),
        Node::Word(nodes) | Node::Punctuation(nodes) => nodes.iter().map(node_to_dsl).collect(),
    }
}

// ============================================================================
// Diffing
// ============================================================================

/// An edit expressed as indices into the original (`Keep`, `Delete`) or
/// edited (`Insert`) token sequence.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Edit {
    Keep(usize),
    Delete(usize),
    Insert(usize),
}

/// Diffs `a` against `b`, where `a[0]` is original token `ao` and `b[0]` is edited
/// token `bo`. Strips the common prefix and suffix before diffing the middle.
fn diff_range(a: &[&str], b: &[&str], ao: usize, bo: usize, out: &mut Vec<Edit>) {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    out.extend((0..prefix).map(|i| Edit::Keep(ao + i)));
    let (a, b) = (&a[prefix..], &b[prefix..]);
    let (ao, bo) = (ao + prefix, bo + prefix);

    let suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[..a.len() - suffix], &b[..b.len() - suffix]);

    diff_middle(am, bm, ao, bo, out);
    out.extend((0..suffix).map(|i| Edit::Keep(ao + am.len() + i)));
}

fn diff_middle(a: &[&str], b: &[&str], ao: usize, bo: usize, out: &mut Vec<Edit>) {
    if a.is_empty() || b.is_empty() {
        replace_all(a.len(), b.len(), ao, bo, out);
        return;
    }
    if a.len().saturating_mul(b.len()) <= LCS_CELL_LIMIT {
        diff_lcs(a, b, ao, bo, out);
        return;
    }

    let anchors = unique_anchors(a, b);
    if anchors.is_empty() {
        // Nothing reliable to align on: replacing the region is honest, whereas
        // pairing tokens by position would attach attributes to the wrong words.
        replace_all(a.len(), b.len(), ao, bo, out);
        return;
    }

    let (mut pa, mut pb) = (0, 0);
    for (ia, ib) in anchors {
        diff_range(&a[pa..ia], &b[pb..ib], ao + pa, bo + pb, out);
        out.push(Edit::Keep(ao + ia));
        pa = ia + 1;
        pb = ib + 1;
    }
    diff_range(&a[pa..], &b[pb..], ao + pa, bo + pb, out);
}

fn replace_all(a_len: usize, b_len: usize, ao: usize, bo: usize, out: &mut Vec<Edit>) {
    out.extend((0..a_len).map(|i| Edit::Delete(ao + i)));
    out.extend((0..b_len).map(|j| Edit::Insert(bo + j)));
}

/// Exact LCS diff. Memory is `a.len() × b.len()` cells, bounded by [`LCS_CELL_LIMIT`].
fn diff_lcs(a: &[&str], b: &[&str], ao: usize, bo: usize, out: &mut Vec<Edit>) {
    let (m, n) = (a.len(), b.len());
    let width = n + 1;
    let mut dp = vec![0u32; (m + 1) * width];

    for i in 1..=m {
        for j in 1..=n {
            dp[i * width + j] = if a[i - 1] == b[j - 1] {
                dp[(i - 1) * width + (j - 1)] + 1
            } else {
                dp[(i - 1) * width + j].max(dp[i * width + (j - 1)])
            };
        }
    }

    let mut reversed = Vec::with_capacity(m + n);
    let (mut i, mut j) = (m, n);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && a[i - 1] == b[j - 1] {
            reversed.push(Edit::Keep(ao + i - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i * width + (j - 1)] >= dp[(i - 1) * width + j]) {
            reversed.push(Edit::Insert(bo + j - 1));
            j -= 1;
        } else {
            reversed.push(Edit::Delete(ao + i - 1));
            i -= 1;
        }
    }
    out.extend(reversed.into_iter().rev());
}

/// Finds tokens occurring exactly once in both sequences and returns the longest
/// chain of them that appears in the same order on both sides (patience diff).
fn unique_anchors(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    // token -> (count in a, index in a, count in b, index in b)
    let mut table: HashMap<&str, (usize, usize, usize, usize)> = HashMap::new();
    for (i, token) in a.iter().enumerate() {
        let entry = table.entry(token).or_insert((0, 0, 0, 0));
        entry.0 += 1;
        entry.1 = i;
    }
    for (j, token) in b.iter().enumerate() {
        if let Some(entry) = table.get_mut(token) {
            entry.2 += 1;
            entry.3 = j;
        }
    }

    let mut pairs: Vec<(usize, usize)> = table
        .values()
        .filter(|(ca, _, cb, _)| *ca == 1 && *cb == 1)
        .map(|(_, ia, _, ib)| (*ia, *ib))
        .collect();
    pairs.sort_unstable();

    longest_increasing_by_b(&pairs)
}

/// Longest subsequence of `pairs` (sorted by `.0`) whose `.1` values increase.
fn longest_increasing_by_b(pairs: &[(usize, usize)]) -> Vec<(usize, usize)> {
    // tails[k] = index into `pairs` of the smallest tail of an increasing run of length k+1
    let mut tails: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; pairs.len()];

    for (idx, &(_, b)) in pairs.iter().enumerate() {
        let pos = tails.partition_point(|&t| pairs[t].1 < b);
        if pos > 0 {
            prev[idx] = Some(tails[pos - 1]);
        }
        if pos == tails.len() {
            tails.push(idx);
        } else {
            tails[pos] = idx;
        }
    }

    let mut chain = Vec::with_capacity(tails.len());
    let mut cursor = tails.last().copied();
    while let Some(idx) = cursor {
        chain.push(pairs[idx]);
        cursor = prev[idx];
    }
    chain.reverse();
    chain
}

/// Pairs each run of deletions with the insertions next to it, in order, so that
/// edited tokens become Modify operations and keep their original attributes.
fn pair_modifications(ops: Vec<PatchOperation>) -> Vec<PatchOperation> {
    let mut result = Vec::with_capacity(ops.len());
    let mut deletes = Vec::new();
    let mut inserts = Vec::new();

    let flush = |result: &mut Vec<PatchOperation>,
                 deletes: &mut Vec<usize>,
                 inserts: &mut Vec<String>| {
        let paired = deletes.len().min(inserts.len());
        for (segment_id, new_dsl) in deletes.drain(..paired).zip(inserts.drain(..paired)) {
            result.push(PatchOperation::Modify { segment_id, new_dsl });
        }
        result.extend(
            deletes
                .drain(..)
                .map(|segment_id| PatchOperation::Delete { segment_id }),
        );
        result.extend(inserts.drain(..).map(|dsl| PatchOperation::Insert { dsl }));
    };

    for op in ops {
        match op {
            PatchOperation::Delete { segment_id } => deletes.push(segment_id),
            PatchOperation::Insert { dsl } => inserts.push(dsl),
            other => {
                flush(&mut result, &mut deletes, &mut inserts);
                result.push(other);
            }
        }
    }
    flush(&mut result, &mut deletes, &mut inserts);
    result
}

// ============================================================================
// Rendering
// ============================================================================

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
            return Ok(RenderedBody { xml: body.to_string(), edited_words: Vec::new() });
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
        .any(|p| matches!(p, PatchOperation::Insert { dsl } if dsl == PARAGRAPH))
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
}

impl RenderedBody {
    /// The edited words as word numbers: the count of `<w>` elements before
    /// each, the numbering the previews and annotations use.
    pub fn edited_word_indices(&self) -> Vec<u32> {
        let starts: Vec<usize> = self
            .xml
            .match_indices("<w")
            .map(|(i, _)| i)
            .filter(|&i| {
                self.xml[i + 2..]
                    .chars()
                    .next()
                    .is_some_and(|c| c == '>' || c == '/' || c.is_whitespace())
            })
            .collect();
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
        let PatchOperation::Modify { segment_id, new_dsl } = patch else {
            out.push(patch.clone());
            continue;
        };
        let is_paragraph = paragraphs.contains(segment_id);
        let tokens = parse_dsl_to_tokens(new_dsl).unwrap_or_else(|_| vec![new_dsl.clone()]);
        if !is_paragraph && !tokens.iter().any(|t| t == PARAGRAPH) {
            out.push(patch.clone());
            continue;
        }
        let insert = |dsl: &String| PatchOperation::Insert { dsl: dsl.clone() };

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
        PatchOperation::Insert { dsl } => dsl == PARAGRAPH,
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
            PatchOperation::Insert { dsl } if dsl == PARAGRAPH => {
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

    let mut out = String::with_capacity(body.len() + 1024);
    let mut copied_to = 0;
    // End of the most recent segment touched by a patch; insertions go here.
    let mut anchor: Option<usize> = None;
    let mut anchor_segment: Option<usize> = None;
    // Insertions before the first segment, waiting for a position.
    let mut pending: Vec<String> = Vec::new();

    for patch in patches {
        match patch {
            PatchOperation::Insert { dsl } => {
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
                        out.push_str(body.get(copied_to..at)?);
                        copied_to = at;
                        if !split {
                            out.push('\n');
                        }
                        out.push_str(fragment.trim_end());
                    }
                    None => pending.push(fragment),
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
                    out.push_str(body.get(copied_to..start)?);
                    copied_to = start;
                    for fragment in pending.drain(..) {
                        out.push_str(fragment.trim_end());
                        out.push('\n');
                    }
                }

                match patch {
                    PatchOperation::Modify { new_dsl, .. } => {
                        let segment = by_id.get(segment_id)?;
                        let source = body.get(start..end)?;
                        let replacement = render_modify(segment, Some(source), new_dsl, compiler);
                        out.push_str(body.get(copied_to..start)?);
                        if keeps_word_data(segment, new_dsl) {
                            edited_words.push(out.len());
                        }
                        out.push_str(replacement.trim_end());
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
                            out.push_str(body.get(copied_to..close_start)?);
                            out.push_str(separator(body.get(close_end..start)?));
                        } else {
                            out.push_str(body.get(copied_to..start)?);
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
        out.push_str(body.get(copied_to..close)?);
        copied_to = close;
        for fragment in pending.drain(..) {
            out.push('\n');
            out.push_str(fragment.trim_end());
        }
        out.push('\n');
    }

    out.push_str(body.get(copied_to..)?);
    Some(RenderedBody { xml: out, edited_words })
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
    let mut xml = String::new();
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
                    PatchOperation::Insert { dsl } if dsl == PARAGRAPH => {
                        xml.push_str("</p>\n<p>");
                        current_patch_idx += 1;
                    }
                    PatchOperation::Insert { dsl } => {
                        xml.push_str(&render_insert(dsl, compiler));
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
                                xml.push_str(&serialize_original_segment(seg));
                            }
                            PatchOperation::Modify { new_dsl, .. } => {
                                if keeps_word_data(seg, new_dsl) {
                                    edited_words.push(xml.len());
                                }
                                xml.push_str(&render_modify(seg, None, new_dsl, compiler));
                            }
                            PatchOperation::Delete { .. } if matches!(seg, Segment::Paragraph { .. }) => {
                                // Merge with the previous paragraph: drop its `</p>`
                                if let Some(close) = xml.rfind("</p>") {
                                    if xml[close + 4..].trim().is_empty() {
                                        let between = xml.split_off(close + 4);
                                        xml.truncate(close);
                                        xml.push_str(separator(&between));
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
                xml.push_str(&serialize_original_segment(seg));
            }
        } else {
            // Structural or whitespace segment
            xml.push_str(&serialize_original_segment(seg));
        }
    }

    // 2. Consume any trailing insertions
    for patch in &patches[current_patch_idx.min(patches.len())..] {
        if let PatchOperation::Insert { dsl } = patch {
            if dsl == PARAGRAPH {
                xml.push_str("</p>\n<p>");
            } else {
                xml.push_str(&render_insert(dsl, compiler));
            }
        }
    }

    RenderedBody { xml, edited_words }
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
    fn test_pair_modifications_pairs_runs_in_order() {
        let ops = vec![
            PatchOperation::Delete { segment_id: 1 },
            PatchOperation::Delete { segment_id: 2 },
            PatchOperation::Insert { dsl: "x".into() },
            PatchOperation::Insert { dsl: "y".into() },
            PatchOperation::Insert { dsl: "z".into() },
        ];
        let paired = pair_modifications(ops);
        let desc: Vec<String> = paired
            .iter()
            .map(|p| match p {
                PatchOperation::Modify { segment_id, new_dsl } => format!("M{}{}", segment_id, new_dsl),
                PatchOperation::Insert { dsl } => format!("I{}", dsl),
                PatchOperation::Delete { segment_id } => format!("D{}", segment_id),
                PatchOperation::Keep { segment_id } => format!("K{}", segment_id),
            })
            .collect();
        assert_eq!(desc, vec!["M1x", "M2y", "Iz"]);
    }

    #[test]
    fn test_is_w_element() {
        assert!(is_w_element("<w>a</w>"));
        assert!(is_w_element(r#"<w lemma="x">a</w>"#));
        assert!(!is_w_element("<wx>a</wx>"));
        assert!(!is_w_element("word"));
    }
}
