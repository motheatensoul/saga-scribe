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
//! - **Edits change only what they touch.** The patches are applied to the
//!   file's tree (see [`apply`](super::apply)), whose writer copies every
//!   node they don't touch from the original text: attribute order, entity
//!   references and formatting stay as written. Without the tree (a file
//!   whose text and tree don't line up), the body is rebuilt from segments.
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

use crate::parser::syntax::break_number_dsl;
use crate::importer::tei::extraction::segments_to_dsl;
use crate::importer::tei::apply::{apply_to_tree, TreeSource};
use crate::importer::tei::helpers;
use crate::importer::tei::segments::{Decision, ImportedDocument, Segment, DSL_RULES};
use crate::importer::tei::source_map::{set_tag_attribute, tag_attribute, tag_length};
use crate::identity::{
    carry, diff_range, is_token, matched_tokens, node_to_dsl, spanned_dsl_tokens, Edit, Guidance,
    Tracked,
    PARAGRAPH,
};
use crate::parser::{Compiler, CompilerConfig, LemmaMapping, Lexer, Node, WordKey, WordTokenizer};
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
        /// An original word moved here (deleted where it was, the same text
        /// inserted here, as by cut and paste): written as the file has it,
        /// with its markup, lemma and analysis
        #[serde(default)]
        moved: Option<usize>,
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
    /// Words that are one segment and one token, which can be recognised
    /// as moved: their original token, by segment ID
    movable: HashMap<usize, usize>,
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
        let words: std::collections::HashSet<usize> =
            segments.iter().filter(|s| matches!(s, Segment::Word { .. })).map(Segment::id).collect();
        let movable = original
            .units
            .iter()
            .filter(|unit| unit.tokens.len() == 1)
            .filter_map(|unit| match unit.segment_ids.as_slice() {
                [id] if words.contains(id) => Some((*id, unit.tokens.start)),
                _ => None,
            })
            .collect();
        Self { original_dsl, original, token_of_segment, movable, state: None }
    }

    /// The original token of segment `id` (the first of its unit).
    pub fn token_of(&self, id: usize) -> Option<usize> {
        self.token_of_segment.get(&id).copied()
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

        let mut known = known;
        let edits = edits_from_origin(&origin, &tokens, &self.original.tokens);
        let mut ops = self.original.to_segment_ops(&edits, &tokens);
        let moved = self.recognise_moves(&mut ops, &mut known, carried.as_deref());
        let patches = replace_in_place(ops, &moved);
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

    /// Turns a deleted word and an insertion of the same text elsewhere (cut
    /// and paste, or identical words the comparison paired differently) into
    /// the word moved: the insertion is the original word, with its markup
    /// and data, and keeps its identity (`known`). Texts deleted and inserted
    /// a different number of times are left alone, as which is which can't
    /// be told, unless the editor's guidance (`carried`, the IDs carried
    /// from the last compile) says which word an insertion is. The words'
    /// deletions stay; their segment IDs are returned.
    fn recognise_moves(
        &self,
        ops: &mut [PatchOperation],
        known: &mut [Option<u64>],
        carried: Option<&[Option<u64>]>,
    ) -> std::collections::HashSet<usize> {
        let mut deleted: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut inserted: HashMap<&str, Vec<usize>> = HashMap::new();
        // Deletions by original token, for the guidance
        let mut deletion_of: HashMap<usize, usize> = HashMap::new();
        for (k, op) in ops.iter().enumerate() {
            if let PatchOperation::Delete { segment_id } = op {
                if let Some(&i) = self.movable.get(segment_id) {
                    deletion_of.insert(i, k);
                }
            }
        }
        let mut moves: Vec<(usize, usize)> = Vec::new();
        let mut paired = std::collections::HashSet::new();
        // The same word, by the guidance, where its text is unchanged
        for (k, op) in ops.iter().enumerate() {
            let PatchOperation::Insert { dsl, token: Some(j), moved: None } = op else { continue };
            let carried_id = carried.and_then(|ids| ids.get(*j).copied().flatten());
            let Some(i) = carried_id.and_then(|id| usize::try_from(id).ok()) else { continue };
            if let Some(&delete) = deletion_of.get(&i) {
                if self.original.tokens.get(i) == Some(dsl) && paired.insert(delete) {
                    paired.insert(k);
                    moves.push((delete, k));
                }
            }
        }
        for (k, op) in ops.iter().enumerate() {
            if paired.contains(&k) {
                continue;
            }
            match op {
                PatchOperation::Delete { segment_id } => {
                    if let Some(&i) = self.movable.get(segment_id) {
                        deleted.entry(self.original.tokens[i].as_str()).or_default().push(k);
                    }
                }
                PatchOperation::Insert { dsl, token: Some(_), moved: None } => {
                    inserted.entry(dsl.as_str()).or_default().push(k);
                }
                _ => {}
            }
        }
        for (text, deletes) in &deleted {
            match inserted.get(text) {
                Some(inserts) if inserts.len() == deletes.len() => moves.extend(deletes.iter().copied().zip(inserts.iter().copied())),
                _ => {}
            }
        }
        let mut moved_away = std::collections::HashSet::new();
        for (delete, insert) in moves {
            let PatchOperation::Delete { segment_id } = ops[delete] else { continue };
            if let PatchOperation::Insert { token: Some(j), moved, .. } = &mut ops[insert] {
                *moved = Some(segment_id);
                if let (Some(slot), Some(&i)) = (known.get_mut(*j), self.movable.get(&segment_id)) {
                    *slot = Some(i as u64);
                }
                moved_away.insert(segment_id);
            }
        }
        if !moved_away.is_empty() {
            log::info!("Recognised {} moved word(s)", moved_away.len());
        }
        moved_away
    }

    /// Makes `state` the basis for the next compile.
    pub fn commit(&mut self, state: Tracked) {
        self.state = Some(state);
    }

    /// The words of the original that `patches` (as [`compute`](Self::compute)
    /// made them, for the DSL tracked in `state`) replace with another word
    /// and that have something to lose: a lemma or an analysis, or (if
    /// `annotations` says some words have any) annotations, which the caller
    /// checks once the words are numbered. Each has the
    /// patcher's answer as its suggestion, and the editor's from `decisions`
    /// if there is one. Their word numbers are filled in by
    /// [`WordRecord::number`] once the body is written.
    ///
    /// Deleted words aren't asked about: they have no word in the text that
    /// the word panel could be opened on, and deleting is undone in the
    /// editor.
    ///
    /// With `only`, just the records of those segments, whatever the words
    /// have to lose (words whose levels need a review).
    #[allow(clippy::too_many_arguments)]
    pub fn records(
        &self,
        segments: &[Segment],
        originals: Originals,
        patches: &[PatchOperation],
        state: &Tracked,
        decisions: &HashMap<usize, Decision>,
        annotations: bool,
        only: Option<&std::collections::HashSet<usize>>,
    ) -> Vec<WordRecord> {
        let segment = |id: usize| segments.get(id).filter(|s| s.id() == id).or_else(|| segments.iter().find(|s| s.id() == id));
        let mut records = Vec::new();
        for patch in patches {
            let PatchOperation::Modify { segment_id, new_dsl, token: Some(j), fresh } = patch else { continue };
            let Some(span) = state.spans.get(*j) else { continue };
            let Some(seg @ Segment::Word { dsl_content, .. }) = segment(*segment_id) else { continue };
            // A word replaced by something else (punctuation, a break) is deleted
            if !keeps_word_data(seg, new_dsl, originals) {
                continue;
            }
            let lemma = originals.attribute(seg, "lemma");
            let msa = originals.attribute(seg, "me:msa").or_else(|| originals.attribute(seg, "msa"));
            match only {
                Some(only) if !only.contains(segment_id) => continue,
                None if lemma.is_none() && msa.is_none() && !annotations => continue,
                _ => {}
            }
            let was = match parse_dsl_to_tokens(dsl_content).as_deref() {
                Ok([token]) => token.clone(),
                _ => dsl_content.trim().to_string(),
            };
            records.push(WordRecord {
                key: *segment_id,
                suggestion: if *fresh { Decision::Different } else { Decision::Same },
                decision: decisions.get(segment_id).copied(),
                word: None,
                token: state.ids.get(*j).copied(),
                from: span.start,
                to: span.end,
                was,
                xml: originals.xml(seg).trim().to_string(),
                lemma,
                msa,
                annotated: false,
                stale_levels: Vec::new(),
            });
        }
        // Byte offsets in the DSL to UTF-16 (the editor's)
        let mut offsets: Vec<usize> = records.iter().flat_map(|r| [r.from, r.to]).collect();
        offsets.sort_unstable();
        offsets.dedup();
        let mut utf16 = HashMap::with_capacity(offsets.len());
        let (mut byte, mut units) = (0, 0);
        for offset in offsets {
            if let Some(text) = state.dsl.get(byte..offset) {
                units += text.encode_utf16().count();
                byte = offset;
            }
            utf16.insert(offset, units);
        }
        for record in &mut records {
            record.from = utf16.get(&record.from).copied().unwrap_or(0);
            record.to = utf16.get(&record.to).copied().unwrap_or(record.from);
        }
        records
    }
}

/// `dsl`, shorthand written against `old` (the segments of a project saved
/// with older import rules, `old_rules`), with its unedited words written as
/// the current rules write `fresh` (the same file projected now); edited and
/// added words, and the layout, stay as typed (`docs/DOCUMENT_MODEL.md`,
/// "Keeping the shorthand as written"). `fresh_id` maps `old`'s segment IDs
/// to `fresh`'s (by where they are in the file). Also returns how many places
/// were rewritten. `None` if `dsl` doesn't parse.
///
/// Tokens are matched in groups: the old tokens of a group and the fresh
/// ones cover the same segments. A group is rewritten if all its old tokens
/// are unedited and next to each other in `dsl`.
pub fn rewrite_unedited(
    old: &[Segment],
    old_rules: u32,
    fresh: &[Segment],
    dsl: &str,
    fresh_id: &HashMap<usize, usize>,
) -> Option<(String, usize)> {
    use crate::importer::tei::extraction::segments_to_placed_dsl;
    let (old_dsl, old_places) = segments_to_placed_dsl(old, old_rules);
    let (fresh_dsl, fresh_places) = segments_to_placed_dsl(fresh, DSL_RULES);
    let (old_tokens, old_spans) = spanned_dsl_tokens(&old_dsl).ok()?;
    let (fresh_tokens, fresh_spans) = spanned_dsl_tokens(&fresh_dsl).ok()?;
    let (tokens, spans) = spanned_dsl_tokens(dsl).ok()?;

    // Which old token each unedited token of `dsl` is
    let a: Vec<&str> = old_tokens.iter().map(String::as_str).collect();
    let b: Vec<&str> = tokens.iter().map(String::as_str).collect();
    let mut edits = Vec::with_capacity(a.len().max(b.len()));
    diff_range(&a, &b, 0, 0, &mut edits);
    let mut kept_at: HashMap<usize, usize> = HashMap::new();
    let mut j = 0;
    for edit in &edits {
        match *edit {
            Edit::Keep(i) => {
                kept_at.insert(i, j);
                j += 1;
            }
            Edit::Insert(k) | Edit::Replace(_, k) => j = k + 1,
            Edit::Delete(_) => {}
        }
    }

    // The tokens each segment's shorthand is in
    let tokens_of = |places: &[(usize, std::ops::Range<usize>)], spans: &[std::ops::Range<usize>]| {
        let mut of: HashMap<usize, Vec<usize>> = HashMap::new();
        for (segment, range) in places {
            let first = spans.partition_point(|s| s.end <= range.start);
            for (t, span) in spans.iter().enumerate().skip(first) {
                if span.start >= range.end.max(range.start + 1) {
                    break;
                }
                of.entry(*segment).or_default().push(t);
            }
        }
        of
    };
    let old_of = tokens_of(&old_places, &old_spans);
    let fresh_of = tokens_of(&fresh_places, &fresh_spans);

    // Groups: old tokens (0..m) and fresh tokens (m..) joined by segments
    let (m, n) = (old_tokens.len(), fresh_tokens.len());
    let mut parent: Vec<usize> = (0..m + n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let union = |parent: &mut Vec<usize>, x: usize, y: usize| {
        let (a, b) = (find(parent, x), find(parent, y));
        parent[a] = b;
    };
    let mut broken = vec![false; m + n];
    let mut covered = vec![false; m + n];
    for (segment, olds) in &old_of {
        let fresh = fresh_id.get(segment);
        let freshes = fresh.and_then(|s| fresh_of.get(s));
        for &o in olds {
            covered[o] = true;
            match (fresh, freshes) {
                (_, Some(freshes)) => {
                    for &f in freshes {
                        covered[m + f] = true;
                        union(&mut parent, o, m + f);
                    }
                }
                // The segment is still there, but no longer shown (a line
                // break of another lineation): the token goes
                (Some(_), None) => {}
                (None, None) => broken[o] = true,
            }
        }
    }
    let mapped: std::collections::HashSet<usize> = fresh_id.values().copied().collect();
    for (segment, freshes) in &fresh_of {
        if !mapped.contains(segment) {
            for &f in freshes {
                broken[m + f] = true;
            }
        }
    }
    let mut groups: HashMap<usize, (Vec<usize>, Vec<usize>, bool)> = HashMap::new();
    for x in 0..m + n {
        let root = find(&mut parent, x);
        let group = groups.entry(root).or_default();
        if x < m {
            group.0.push(x);
        } else {
            group.1.push(x - m);
        }
        // A token of no segment (shouldn't be) can't be placed
        group.2 |= broken[x] || !covered[x];
    }

    let mut replacements: Vec<(std::ops::Range<usize>, &str)> = Vec::new();
    for (olds, freshes, broken) in groups.values() {
        if *broken || olds.is_empty() {
            continue;
        }
        if freshes.is_empty() {
            // No longer shown: removed with the space after it, if unedited
            let Some(&j) = (olds.len() == 1).then(|| kept_at.get(&olds[0])).flatten() else { continue };
            let after = dsl[spans[j].end..].len() - dsl[spans[j].end..].trim_start_matches([' ', '\t']).len();
            replacements.push((spans[j].start..spans[j].end + after, ""));
            continue;
        }
        // Every old token unedited, and next to each other in `dsl`
        let Some(at) = olds.iter().map(|i| kept_at.get(i).copied()).collect::<Option<Vec<usize>>>() else { continue };
        let consecutive = |v: &[usize]| v.windows(2).all(|w| w[1] == w[0] + 1);
        if !consecutive(&at) || !consecutive(freshes) {
            continue;
        }
        let (first, last) = (freshes[0], freshes[freshes.len() - 1]);
        let text = fresh_dsl.get(fresh_spans[first].start..fresh_spans[last].end)?;
        let range = spans[at[0]].start..spans[at[at.len() - 1]].end;
        if dsl.get(range.clone()) != Some(text) {
            replacements.push((range, text));
        }
    }
    replacements.sort_by_key(|(range, _)| range.start);
    let mut out = String::with_capacity(dsl.len() + 64);
    let mut from = 0;
    for (range, text) in &replacements {
        if range.start < from {
            return None;
        }
        out.push_str(&dsl[from..range.start]);
        out.push_str(text);
        from = range.end;
    }
    out.push_str(&dsl[from..]);
    Some((out, replacements.len()))
}

/// The shorthand `stored` (saved with a project) with the places where it
/// doesn't read as `projected` (the saved file projected now) rewritten from
/// the projection: the XML wins, the rest keeps its layout
/// (`docs/DOCUMENT_MODEL.md`, "Keeping the shorthand as written"). Also
/// returns how many places were rewritten. If `stored` doesn't parse, the
/// projection.
pub fn keep_as_written(stored: &str, projected: &str) -> (String, usize) {
    let (Ok((a, a_spans)), Ok((b, b_spans))) = (spanned_dsl_tokens(stored), spanned_dsl_tokens(projected)) else {
        return (projected.to_string(), 1);
    };
    if a == b {
        return (stored.to_string(), 0);
    }
    let x: Vec<&str> = a.iter().map(String::as_str).collect();
    let y: Vec<&str> = b.iter().map(String::as_str).collect();
    let mut edits = Vec::with_capacity(x.len().max(y.len()));
    diff_range(&x, &y, 0, 0, &mut edits);
    // Runs of changes: the stored tokens they take out, the projected ones they put in
    /// The stored tokens a run takes out, the projected ones it puts in, and
    /// where it goes if it takes none out
    type Run = (Option<std::ops::Range<usize>>, Option<std::ops::Range<usize>>, usize);
    let mut runs: Vec<Run> = Vec::new();
    let (mut old, mut new): (Option<std::ops::Range<usize>>, Option<std::ops::Range<usize>>) = (None, None);
    let widen = |r: &mut Option<std::ops::Range<usize>>, i: usize| {
        *r = Some(match r.take() {
            Some(r) => r.start.min(i)..r.end.max(i + 1),
            None => i..i + 1,
        });
    };
    // Where a run without stored tokens goes: after this stored token
    let mut after: usize = 0;
    let mut last_kept: Option<usize> = None;
    for edit in edits.iter().chain([&Edit::Keep(usize::MAX)]) {
        match *edit {
            Edit::Keep(i) => {
                if old.is_some() || new.is_some() {
                    runs.push((old.take(), new.take(), after));
                }
                if i != usize::MAX {
                    last_kept = Some(i);
                }
            }
            Edit::Delete(i) => widen(&mut old, i),
            Edit::Insert(j) => widen(&mut new, j),
            Edit::Replace(i, j) => {
                widen(&mut old, i);
                widen(&mut new, j);
            }
        }
        if old.is_none() && new.is_none() {
            after = last_kept.map_or(0, |i| a_spans[i].end);
        }
    }
    let mut pieces: Vec<(std::ops::Range<usize>, String)> = runs
        .into_iter()
        .map(|(old, new, after)| {
            let text = new.map_or(String::new(), |r| projected[b_spans[r.start].start..b_spans[r.end - 1].end].to_string());
            match old {
                Some(r) => (a_spans[r.start].start..a_spans[r.end - 1].end, text),
                None => (after..after, format!(" {text}")),
            }
        })
        .collect();
    pieces.sort_by_key(|(r, _)| r.start);
    let count = pieces.len();
    let mut out = String::with_capacity(stored.len());
    let mut from = 0;
    for (range, text) in pieces {
        if range.start < from {
            return (projected.to_string(), count);
        }
        out.push_str(&stored[from..range.start]);
        out.push_str(&text);
        from = range.end;
    }
    out.push_str(&stored[from..]);
    (out, count)
}

/// Applies the editor's answers to `patches`: a replaced word that is the
/// same word keeps its data, another word loses it, whatever the patcher
/// suggested.
pub fn apply_decisions(
    segments: &[Segment],
    originals: Originals,
    patches: &mut [PatchOperation],
    decisions: &HashMap<usize, Decision>,
) {
    if decisions.is_empty() {
        return;
    }
    for patch in patches {
        if let PatchOperation::Modify { segment_id, new_dsl, fresh, .. } = patch {
            let same = match decisions.get(segment_id) {
                Some(Decision::Same) => true,
                Some(Decision::Different) => false,
                None => continue,
            };
            let segment = segments.get(*segment_id).filter(|s| s.id() == *segment_id);
            if segment.is_some_and(|s| keeps_word_data(s, new_dsl, originals)) {
                *fresh = !same;
            }
        }
    }
}

/// A word of the original that the edited shorthand replaces with another
/// word, with the lemma and analysis it would lose (`docs/DOCUMENT_MODEL.md`,
/// "Replacing and deleting words"). Open until the editor decides, in the
/// word panel.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordRecord {
    /// The segment ID of the original word: what decisions are kept by
    pub key: usize,
    /// The patcher's answer, which the XML follows until the editor decides
    pub suggestion: Decision,
    pub decision: Option<Decision>,
    /// The new word's number in the compiled body (the n-th `<w>`, as the
    /// previews count), which the word panel is opened by
    pub word: Option<usize>,
    /// The new word's token ID (`Tracked::ids`)
    #[serde(skip)]
    pub token: Option<u64>,
    /// Where the new word is in the edited DSL (UTF-16)
    pub from: usize,
    pub to: usize,
    /// The old word's shorthand
    pub was: String,
    /// The old word's XML as the file has it: everything a different word
    /// would lose, shown in the word panel
    pub xml: String,
    pub lemma: Option<String>,
    pub msa: Option<String>,
    /// The word has annotations (other than its lemma) in the project. They
    /// stay with it whatever the answer: they may be about the place in the
    /// text rather than the word
    pub annotated: bool,
    /// The word's levels that couldn't follow the edit and were kept as
    /// they were (a curated normalized form): they may be out of date
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stale_levels: Vec<String>,
}

/// An open record as a project saves it (`.teis` 2.0, `records.json`): the
/// file it is saved with has the suggested answer written, and the record
/// refers to its word there by place and fingerprint
/// ([`crate::document::anchor`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedRecord {
    /// The word's number in the saved file (the n-th `<w>` of the body)
    pub word: usize,
    pub fingerprint: String,
    pub suggestion: Decision,
    /// The old word's shorthand and XML, its lemma and analysis
    pub was: String,
    pub xml: String,
    pub lemma: Option<String>,
    pub msa: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale_levels: Vec<String>,
}

impl WordRecord {
    /// What the editor's answer, or else the suggestion, makes of the word.
    pub fn outcome(&self) -> Decision {
        self.decision.unwrap_or(self.suggestion)
    }

    /// Fills in the records' word numbers from the compiled body's `words`.
    pub fn number(records: &mut [WordRecord], words: &[WordKey]) {
        if records.is_empty() {
            return;
        }
        let first: HashMap<u64, usize> = words
            .iter()
            .enumerate()
            .filter(|(_, (_, k))| *k == 0)
            .map(|(n, (id, _))| (*id, n))
            .collect();
        for record in records {
            record.word = record.token.and_then(|id| first.get(&id).copied());
        }
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
                        moved: None,
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
            Some(num) => format!("//{}", break_number_dsl(num, false)),
            None => "//".to_string(),
        }),
        Segment::PageBreak { attributes, .. } => Some(match attributes.get("n").filter(|n| !n.is_empty()) {
            Some(num) => format!("///{}", break_number_dsl(num, true)),
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
/// Words moved elsewhere (`moved`) are deleted, not paired.
fn replace_in_place(ops: Vec<PatchOperation>, moved: &std::collections::HashSet<usize>) -> Vec<PatchOperation> {
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
        result.extend(inserts.drain(..).map(|(dsl, token)| PatchOperation::Insert { dsl, token, moved: None }));
    };

    for op in ops {
        match op {
            PatchOperation::Delete { segment_id } if !moved.contains(&segment_id) => deletes.push(segment_id),
            PatchOperation::Insert { dsl, token, moved: None } => inserts.push((dsl, token)),
            other => {
                flush(&mut result, &mut deletes, &mut inserts);
                result.push(other);
            }
        }
    }
    flush(&mut result, &mut deletes, &mut inserts);
    result
}

/// The original elements behind the segments, as the patcher reads them: in
/// the file's tree when there is one, otherwise in the copies the segments
/// hold. A session with a tree drops those copies
/// ([`ImportedDocument::drop_copies`]), so with a tree everything about a
/// word's XML must be read here.
#[derive(Debug, Clone, Copy, Default)]
pub struct Originals<'a> {
    tree: Option<&'a TreeSource>,
}

impl<'a> Originals<'a> {
    pub fn new(tree: Option<&'a TreeSource>) -> Self {
        Self { tree }
    }

    /// The segment as written in the file: its copy, or where it has none
    /// (dropped), the tree's.
    pub fn xml<'s>(&self, segment: &'s Segment) -> std::borrow::Cow<'s, str>
    where
        'a: 's,
    {
        match segment {
            Segment::Word { original_xml, .. } | Segment::Punctuation { original_xml, .. } if !original_xml.is_empty() => {
                original_xml.into()
            }
            Segment::Word { .. } | Segment::Punctuation { .. } => {
                self.tree.and_then(|tree| tree.segment_xml(segment.id())).unwrap_or_default().into()
            }
            other => serialize_original_segment(other).into(),
        }
    }

    /// Whether the segment is an element named `name` (as written, with
    /// its prefix if it has one).
    fn is_element(&self, segment: &Segment, name: &str) -> bool {
        if !matches!(segment, Segment::Word { .. } | Segment::Punctuation { .. }) {
            return false;
        }
        match self.tree {
            Some(tree) if tree.segment_xml(segment.id()).is_some() => {
                tree.segment_element(segment.id()).is_some_and(|e| e.name == name)
            }
            _ => is_element(&self.xml(segment), name),
        }
    }

    /// Whether the segment is a `<w>` element (not text outside words, or
    /// another element stored as a word, such as `<num>`).
    pub fn is_word(&self, segment: &Segment) -> bool {
        matches!(segment, Segment::Word { .. }) && self.is_element(segment, "w")
    }

    /// A word's attributes (lemma, analysis, …): its copy, or where it has
    /// none (dropped), the tree's.
    fn attributes(&self, segment: &Segment) -> HashMap<String, String> {
        match segment {
            Segment::Word { attributes, .. } if !attributes.is_empty() => attributes.clone(),
            Segment::Word { .. } => self.tree.and_then(|tree| tree.segment_attributes(segment.id())).unwrap_or_default(),
            _ => HashMap::new(),
        }
    }

    pub(super) fn attribute(&self, segment: &Segment, name: &str) -> Option<String> {
        match segment {
            Segment::Word { attributes, .. } if !attributes.is_empty() => attributes.get(name).cloned(),
            Segment::Word { .. } => self.tree?.segment_element(segment.id())?.attribute(name).map(str::to_string),
            _ => None,
        }
    }
}

/// Compiler settings matching an imported document's own encoding style:
/// MENOTA three-level words, plain `<w>` words, or unwrapped text.
pub fn compiler_config_for(doc: &ImportedDocument, originals: Originals) -> CompilerConfig {
    let word_wrap = doc.is_menota || doc.segments.iter().any(|seg| originals.is_word(seg));
    let levels = doc.profile.as_ref().and_then(crate::parser::Levels::of);
    CompilerConfig {
        word_wrap,
        auto_line_numbers: false,
        // Words get the profile's levels (MENOTA's three in files without one)
        multi_level: if doc.profile.is_some() { levels.is_some() } else { doc.is_menota },
        wrap_pages: false,
        levels,
    }
}

/// Renders the body XML for an imported document after applying `patches`.
///
/// - No effective changes: returns `original_body` verbatim.
/// - With the document's `tree`: applies the edits to it (see
///   [`apply`](super::apply)), so only edited nodes are written anew.
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
    tree: Option<&TreeSource>,
    patches: &[PatchOperation],
    compiler: &mut Compiler,
) -> Result<RenderedBody, String> {
    let unchanged = patches
        .iter()
        .all(|p| matches!(p, PatchOperation::Keep { .. }));

    if let Some(body) = original_body {
        if unchanged {
            // The words are the tree's: scanning the text would also count
            // a `<w` in a comment
            if let Some(tree) = tree {
                let words = tree.word_offsets().into_iter().map(|w| (w, Origin::Source(w))).collect();
                return Ok(RenderedBody { xml: body.to_string(), edited_words: Vec::new(), words, stale: Vec::new() });
            }
            let mut out = Output::with_capacity(body.len());
            out.push(body, Origin::Source(0));
            return Ok(out.finish(Vec::new()));
        }
    }

    let patches = separate_paragraph_edits(&doc.segments, patches);
    check_paragraph_edits(&doc.segments, &patches)?;
    let originals = Originals::new(tree);

    if let Some(tree) = tree {
        if let Some(rendered) = apply_to_tree(tree, &doc.segments, &patches, compiler) {
            return Ok(rendered);
        }
        log::warn!("The edits don't fit the document's tree; falling back to reconstruction");
        // Tests that give a tree expect it to take the edits
        if cfg!(test) {
            panic!("the edits didn't fit the document's tree");
        }
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
    Ok(reconstruct(&doc.segments, originals, &patches, compiler))
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
    /// Edited words (by segment) with levels that couldn't follow the edit
    /// and were kept as they were: to be reviewed
    pub stale: Vec<(usize, Vec<String>)>,
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
        let insert = |dsl: &String| PatchOperation::Insert { dsl: dsl.clone(), token: *token, moved: None };

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
pub(super) struct ParagraphStructure<'a> {
    /// Position of each segment in document order, by ID
    position: HashMap<usize, usize>,
    /// The innermost open element after each segment (by position)
    innermost_after: Vec<Option<&'a str>>,
}

impl<'a> ParagraphStructure<'a> {
    pub(super) fn new(segments: &'a [Segment]) -> Self {
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
    pub(super) fn split_point_after(&self, segments: &[Segment], id: usize) -> Result<usize, Option<&'a str>> {
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
    pub(super) fn preceding_close<'s>(&self, segments: &'s [Segment], id: usize) -> Option<&'s Segment> {
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
        RenderedBody { xml: self.xml, edited_words, words, stale: Vec::new() }
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

/// The kind of the first token in a DSL fragment.
pub(super) enum TokenKind {
    Word,
    Punctuation,
    LineBreak(Option<String>),
    PageBreak(Option<String>),
    Other,
}

pub(super) fn token_kind(dsl: &str) -> TokenKind {
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
pub(super) fn keeps_word_data(segment: &Segment, new_dsl: &str, originals: Originals) -> bool {
    originals.is_word(segment) && matches!(token_kind(new_dsl), TokenKind::Word)
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

/// Whether `xml` starts with an element named `name` (no namespace prefix).
fn is_element(xml: &str, name: &str) -> bool {
    xml.strip_prefix('<')
        .and_then(|rest| rest.strip_prefix(name))
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c == '>' || c == '/' || c.is_whitespace())
}

/// Whether `dsl` is a single numeral, `#…#`.
fn is_numeral_dsl(dsl: &str) -> bool {
    matches!(parse_dsl_nodes(dsl).as_deref(), Ok([Node::Numeral(_)]))
}

/// Compiles newly inserted DSL in the style of the document (word-wrapped or not).
pub(super) fn render_insert(dsl: &str, compiler: &mut Compiler) -> String {
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
/// Also returns the word's levels that couldn't follow the edit and were
/// kept as they were (pinned levels, such as a curated normalized form, to
/// be reviewed: `docs/DOCUMENT_MODEL.md`, "Derived or pinned").
pub(super) fn render_modify(
    segment: &Segment,
    originals: Originals,
    source: Option<&str>,
    new_dsl: &str,
    same_word: bool,
    compiler: &mut Compiler,
) -> (String, Vec<String>) {
    let mut stale = Vec::new();
    let xml = render_modify_into(segment, originals, source, new_dsl, same_word, compiler, &mut stale);
    (xml, stale)
}

fn render_modify_into(
    segment: &Segment,
    originals: Originals,
    source: Option<&str>,
    new_dsl: &str,
    same_word: bool,
    compiler: &mut Compiler,
    stale: &mut Vec<String>,
) -> String {
    let original_xml = originals.xml(segment);
    match (segment, token_kind(new_dsl)) {
        (Segment::Word { dsl_content, .. }, TokenKind::Word) if originals.is_word(segment) => {
            // The same word: lemma, msa (attributes) and the curated norm are
            // kept; facs and dipl follow the edit. The word is reported as
            // edited for review. Another word: those are dropped
            let original = source.unwrap_or(&original_xml);
            let norm = compiler
                .config()
                .multi_level
                .then(|| level_content(original, "norm"))
                .flatten()
                .map(without_breaks)
                .filter(|_| same_word);
            let mut attributes = originals.attributes(segment);
            if !same_word {
                attributes.retain(|name, _| !matches!(name.as_str(), "lemma" | "me:msa" | "msa"));
            }
            let attributes = &attributes;
            // The old shorthand is read as a token, as `new_dsl` was, so the
            // two differ only by the edit
            let old_token = match parse_dsl_to_tokens(dsl_content).as_deref() {
                Ok([token]) => token.clone(),
                _ => dsl_content.clone(),
            };
            // A normalized form that is what the shorthand gives is derived,
            // and follows the edit. One that isn't is pinned (a reading the
            // editor chose): it's kept, and marked for review if the edit
            // changes the form the shorthand gives to something else
            let mut norm = norm;
            if let Some(curated) = norm.clone() {
                let derived = |dsl: &str, compiler: &mut Compiler| {
                    let xml = compiler.compile_word_from_dsl(dsl, attributes, None);
                    level_content(&xml, "norm").map(without_breaks)
                };
                let (was, now) = (derived(&old_token, compiler), derived(new_dsl, compiler));
                if was.as_deref() == Some(curated.as_str()) {
                    norm = None;
                } else if was != now && now.as_deref() != Some(curated.as_str()) {
                    stale.push("norm".to_string());
                }
            }
            let compiled = compiler.compile_word_from_dsl(new_dsl, attributes, norm.as_deref());
            // The edit is merged into the word as written, so markup the
            // shorthand doesn't show stays (see document/merge.rs)
            let before = compiler.compile_word_from_dsl(&old_token, attributes, norm.as_deref());
            match crate::document::merge_word_edit(original, &before, &compiled) {
                Some(merged) => {
                    if same_word {
                        stale.extend(merged.kept_levels.iter().filter(|l| !stale.contains(l)).cloned().collect::<Vec<_>>());
                    }
                    if !merged.kept_levels.is_empty() || !merged.removed.is_empty() {
                        log::info!(
                            "Edited word {:?}: levels kept as they were {:?}, removed with their text {:?}",
                            new_dsl,
                            merged.kept_levels,
                            merged.removed
                        );
                    }
                    if same_word {
                        merged.xml
                    } else {
                        crate::document::as_different_word(&merged.xml, &compiled).unwrap_or(compiled)
                    }
                }
                None => compiled,
            }
        }
        // A numeral stays a numeral: the edit is merged into it as into a
        // word, keeping its attributes and the markup the shorthand doesn't show
        (Segment::Word { dsl_content, .. }, TokenKind::Other)
            if originals.is_element(segment, "num") && is_numeral_dsl(new_dsl) && is_numeral_dsl(dsl_content) =>
        {
            let original = source.unwrap_or(&original_xml);
            let compiled = render_insert(new_dsl, compiler);
            let before = render_insert(dsl_content.trim(), compiler);
            match crate::document::merge_word_edit(original, &before, &compiled) {
                Some(merged) => merged.xml,
                None => compiled,
            }
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
    reconstruct(segments, Originals::default(), patches, compiler).xml
}

fn reconstruct(segments: &[Segment], originals: Originals, patches: &[PatchOperation], compiler: &mut Compiler) -> RenderedBody {
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
                    PatchOperation::Insert { dsl, token, moved } => {
                        out.generate(&inserted_xml(dsl, *moved, segments, originals, compiler), *token);
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
                                out.push(&originals.xml(seg), Origin::Segment(seg_id));
                            }
                            PatchOperation::Modify { new_dsl, token, fresh, .. } => {
                                let word = keeps_word_data(seg, new_dsl, originals);
                                if *fresh && !word {
                                    out.generate(&render_insert(new_dsl, compiler), *token);
                                } else {
                                    if word && !fresh {
                                        edited_words.push(out.xml.len());
                                    }
                                    out.generate(&render_modify(seg, originals, None, new_dsl, !fresh, compiler).0, *token);
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
                out.push(&originals.xml(seg), Origin::Segment(seg_id));
            }
        } else {
            // Structural or whitespace segment
            out.push(&serialize_original_segment(seg), Origin::Segment(seg.id()));
        }
    }

    // 2. Consume any trailing insertions
    for patch in &patches[current_patch_idx.min(patches.len())..] {
        if let PatchOperation::Insert { dsl, token, moved } = patch {
            if dsl == PARAGRAPH {
                out.generate("</p>\n<p>", None);
            } else {
                out.generate(&inserted_xml(dsl, *moved, segments, originals, compiler), *token);
            }
        }
    }

    out.finish(edited_words)
}

/// The XML of an insertion: a moved word as the file has it, anything else
/// compiled from its DSL.
pub(super) fn inserted_xml(
    dsl: &str,
    moved: Option<usize>,
    segments: &[Segment],
    originals: Originals,
    compiler: &mut Compiler,
) -> String {
    let segment = moved.and_then(|id| segments.get(id).filter(|s| s.id() == id).or_else(|| segments.iter().find(|s| s.id() == id)));
    match segment {
        Some(segment) => originals.xml(segment).trim().to_string(),
        None => render_insert(dsl, compiler),
    }
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
        assert!(is_element("<w>a</w>", "w"));
        assert!(is_element(r#"<w lemma="x">a</w>"#, "w"));
        assert!(!is_element("<wx>a</wx>", "w"));
        assert!(!is_element("word", "w"));
    }
}
