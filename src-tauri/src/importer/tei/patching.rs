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
pub fn render_body(
    doc: &ImportedDocument,
    original_body: Option<&str>,
    patches: &[PatchOperation],
    compiler: &mut Compiler,
) -> String {
    let unchanged = patches
        .iter()
        .all(|p| matches!(p, PatchOperation::Keep { .. }));

    if let Some(body) = original_body {
        if unchanged {
            return body.to_string();
        }
        if let Some(spans) = &doc.spans {
            if let Some(xml) = splice_into_source(body, &doc.segments, spans, patches, compiler) {
                return xml;
            }
            log::warn!("Source spans inconsistent with patches; falling back to reconstruction");
        }
    }

    apply_patches_and_reconstruct(&doc.segments, patches, compiler)
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
) -> Option<String> {
    let by_id: HashMap<usize, &Segment> = segments.iter().map(|s| (s.id(), s)).collect();

    let mut out = String::with_capacity(body.len() + 1024);
    let mut copied_to = 0;
    // End of the most recent segment touched by a patch; insertions go here.
    let mut anchor: Option<usize> = None;
    // Insertions before the first segment, waiting for a position.
    let mut pending: Vec<String> = Vec::new();

    for patch in patches {
        match patch {
            PatchOperation::Insert { dsl } => {
                let fragment = render_insert(dsl, compiler);
                match anchor {
                    Some(at) => {
                        out.push_str(body.get(copied_to..at)?);
                        copied_to = at;
                        out.push('\n');
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
                        out.push_str(replacement.trim_end());
                        copied_to = end;
                    }
                    PatchOperation::Delete { .. } => {
                        out.push_str(body.get(copied_to..start)?);
                        copied_to = end;
                    }
                    _ => {}
                }
                anchor = Some(end);
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
    Some(out)
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
        ) if is_w_element(original_xml) => compiler.compile_word_from_dsl(new_dsl, attributes),
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
pub fn apply_patches_and_reconstruct(
    segments: &[Segment],
    patches: &[PatchOperation],
    compiler: &mut Compiler,
) -> String {
    let mut xml = String::new();
    let mut current_patch_idx = 0;

    // 1. Process segments in order
    for seg in segments {
        if matches!(
            seg,
            Segment::Word { .. }
                | Segment::Punctuation { .. }
                | Segment::LineBreak { .. }
                | Segment::PageBreak { .. }
        ) {
            let seg_id = seg.id();
            let mut handled = false;

            // Interleave leading insertions
            while let Some(patch) = patches.get(current_patch_idx) {
                match patch {
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
                                xml.push_str(&render_modify(seg, None, new_dsl, compiler));
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
            xml.push_str(&render_insert(dsl, compiler));
        }
    }

    xml
}

fn serialize_original_segment(seg: &Segment) -> String {
    match seg {
        Segment::Structural { xml, .. } => xml.clone(),
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
