//! # Word identity
//!
//! Tracks which token of the DSL is which across edits, for imported mode
//! (whose tokens are matched to the original document's) and native mode
//! alike, so that annotations and the original XML stay with their words.
//!
//! Each token has an ID. The editor reports its changes since the last
//! compile ([`Guidance`]); tokens outside the changed ranges keep their IDs
//! outright, and only tokens in a changed range are matched by comparison.
//! Without guidance, callers compare whole documents ([`diff_range`]).

use crate::parser::{Lexer, Node, WordKey, WordTokenizer};
use std::collections::HashMap;
use std::ops::Range;

/// The DSL token for a paragraph break.
pub const PARAGRAPH: &str = ".p";


/// Largest middle region (original × edited tokens) diffed with an exact LCS table.
const LCS_CELL_LIMIT: usize = 4_000_000;

/// An edit made in the editor: `from_a..to_a` of the previous DSL became
/// `from_b..to_b` of the new one, in UTF-16 code units (CodeMirror's offsets).
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct TextChange {
    pub from_a: usize,
    pub to_a: usize,
    pub from_b: usize,
    pub to_b: usize,
}

/// The edits made since the DSL of revision `base_revision` was compiled.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guidance {
    pub base_revision: u64,
    pub changes: Vec<TextChange>,
}

/// The tokens of a compiled DSL and the ID of each.
#[derive(Debug, Clone)]
pub struct Tracked {
    /// Identifies this compile, for [`Guidance::base_revision`]
    pub revision: u64,
    pub dsl: String,
    pub tokens: Vec<String>,
    pub spans: Vec<Range<usize>>,
    pub ids: Vec<u64>,
    /// The ID the next new token gets
    pub next_id: u64,
}

impl Tracked {
    /// Tracks `dsl` as an edit of `previous`: tokens keep their IDs through
    /// the editor's changes (`guidance`), or, if it doesn't fit, as found by
    /// comparing the two token sequences; other tokens get new IDs, the
    /// first being `first_id` when there is no `previous`.
    ///
    /// # Errors
    ///
    /// Returns an error if `dsl` does not parse.
    pub fn track(
        previous: Option<&Tracked>,
        guidance: Option<&Guidance>,
        dsl: &str,
        first_id: u64,
    ) -> Result<Tracked, String> {
        let (tokens, spans) = spanned_dsl_tokens(dsl)?;
        let carried = carry(previous, guidance, dsl, &tokens, &spans)
            .or_else(|| previous.map(|previous| compare(previous, &tokens)))
            .unwrap_or_else(|| vec![None; tokens.len()]);
        Ok(Self::with_ids(previous, dsl, tokens, spans, &carried, first_id))
    }

    /// A tracked DSL whose tokens have the given IDs where known, and new
    /// IDs elsewhere.
    pub fn with_ids(
        previous: Option<&Tracked>,
        dsl: &str,
        tokens: Vec<String>,
        spans: Vec<Range<usize>>,
        known: &[Option<u64>],
        first_id: u64,
    ) -> Tracked {
        let mut next_id = previous.map_or(first_id, |p| p.next_id).max(first_id);
        let ids = known
            .iter()
            .map(|id| match id {
                Some(id) => *id,
                None => {
                    next_id += 1;
                    next_id - 1
                }
            })
            .collect();
        let revision = previous.map_or(0, |p| p.revision) + 1;
        Tracked { revision, dsl: dsl.to_string(), tokens, spans, ids, next_id }
    }
}

/// The IDs of `previous`'s tokens that `tokens` keeps, found by comparing
/// the two sequences (without the editor's changes to go by).
fn compare(previous: &Tracked, tokens: &[String]) -> Vec<Option<u64>> {
    let a: Vec<&str> = previous.tokens.iter().map(String::as_str).collect();
    let b: Vec<&str> = tokens.iter().map(String::as_str).collect();
    let mut edits = Vec::with_capacity(a.len().max(b.len()));
    diff_range(&a, &b, 0, 0, &mut edits);
    let mut ids = vec![None; tokens.len()];
    for (j, k) in matched_tokens(&edits, 0) {
        ids[j] = Some(previous.ids[k]);
    }
    ids
}

/// For each word number of a previous compile, its number now (`None` if
/// the word is gone).
pub type Renumbering = Vec<Option<u32>>;

/// How word numbers moved between two compiles: for each word of `previous`
/// (by number), its number in `current`, or `None` if it is gone. Returns
/// `None` if every word kept its number.
pub fn renumbering(previous: &[WordKey], current: &[WordKey]) -> Option<Renumbering> {
    if previous == current {
        return None;
    }
    let numbers: HashMap<WordKey, u32> =
        current.iter().enumerate().map(|(n, key)| (*key, n as u32)).collect();
    Some(previous.iter().map(|key| numbers.get(key).copied()).collect())
}

/// Each word's number, by key (to look up what was stored under a number).
pub fn numbers_by_key(words: &[WordKey]) -> HashMap<WordKey, u32> {
    words.iter().enumerate().map(|(n, key)| (*key, n as u32)).collect()
}

/// The IDs carried from `previous` through `guidance`, if it fits.
pub fn carry(
    previous: Option<&Tracked>,
    guidance: Option<&Guidance>,
    dsl: &str,
    tokens: &[String],
    spans: &[Range<usize>],
) -> Option<Vec<Option<u64>>> {
    let (previous, guidance) = previous.zip(guidance)?;
    if guidance.base_revision != previous.revision {
        return None;
    }
    carry_ids(previous, &guidance.changes, dsl, tokens, spans)
}

/// A DSL's tokens (as the patcher diffs them) with their source ranges.
pub fn spanned_dsl_tokens(dsl: &str) -> Result<(Vec<String>, Vec<Range<usize>>), String> {
    let nodes = Lexer::new(dsl).parse_spanned()?;
    Ok(WordTokenizer::new()
        .tokenize_spanned(nodes)
        .into_iter()
        .filter(|(node, _)| is_token(node))
        .map(|(node, span)| (node_to_dsl(&node), span))
        .unzip())
}

/// Carries identity from the previous compile through the editor's changes:
/// for each token of the new DSL, the ID of the previous token it is (edited
/// or not), if any. Returns `None` if the changes don't fit the previous DSL.
pub fn carry_ids(
    previous: &Tracked,
    changes: &[TextChange],
    edited_dsl: &str,
    tokens: &[String],
    spans: &[Range<usize>],
) -> Option<Vec<Option<u64>>> {
    // CodeMirror's UTF-16 offsets as byte offsets in the old and new DSL
    let old_at = utf16_to_bytes(&previous.dsl, changes.iter().flat_map(|c| [c.from_a, c.to_a]))?;
    let new_at = utf16_to_bytes(edited_dsl, changes.iter().flat_map(|c| [c.from_b, c.to_b]))?;
    let changes: Vec<(usize, usize, usize, usize)> = changes
        .iter()
        .map(|c| (old_at[&c.from_a], old_at[&c.to_a], new_at[&c.from_b], new_at[&c.to_b]))
        .collect();
    if changes.windows(2).any(|w| w[0].1 > w[1].0) {
        return None;
    }

    // A token no change reaches into is the same token, shifted, if the new
    // DSL has the same token at exactly the shifted place (an edit right at
    // its edge may have merged it with a neighbour, so that is checked)
    let by_start: HashMap<usize, usize> = spans.iter().enumerate().map(|(j, s)| (s.start, j)).collect();
    let mut anchors: Vec<(usize, usize)> = Vec::new();
    let mut c = 0;
    for (k, span) in previous.spans.iter().enumerate() {
        // Changes entirely before the token (insertions at its start included)
        while c < changes.len() && changes[c].1 <= span.start {
            c += 1;
        }
        let overlaps = changes.get(c).is_some_and(|&(from, _, _, _)| from < span.end);
        if overlaps {
            continue;
        }
        // Shift by the last change entirely before the token
        let shift = |p: usize| match c.checked_sub(1).map(|i| changes[i]) {
            Some((_, to_a, _, to_b)) => to_b + (p - to_a),
            None => p,
        };
        let start = shift(span.start);
        if let Some(&j) = by_start.get(&start) {
            let in_order = anchors.last().is_none_or(|&(pk, pj)| k > pk && j > pj);
            if in_order && spans[j].end == shift(span.end) && tokens[j] == previous.tokens[k] {
                anchors.push((k, j));
            }
        }
    }

    // Between anchors, match the changed tokens by comparison
    let mut ids = vec![None; tokens.len()];
    let (mut k0, mut j0) = (0, 0);
    let old: Vec<&str> = previous.tokens.iter().map(String::as_str).collect();
    let new: Vec<&str> = tokens.iter().map(String::as_str).collect();
    for &(k, j) in anchors.iter().chain(std::iter::once(&(old.len(), new.len()))) {
        let mut edits = Vec::new();
        diff_range(&old[k0..k], &new[j0..j], k0, j0, &mut edits);
        for (jj, kk) in matched_tokens(&edits, j0) {
            ids[jj] = Some(previous.ids[kk]);
        }
        if k < old.len() {
            ids[j] = Some(previous.ids[k]);
        }
        (k0, j0) = (k + 1, j + 1);
    }
    Some(ids)
}

/// Converts UTF-16 offsets into `s` to byte offsets; `None` if one is out of
/// range or inside a character.
pub(crate) fn utf16_to_bytes(s: &str, offsets: impl Iterator<Item = usize>) -> Option<HashMap<usize, usize>> {
    let mut wanted: Vec<usize> = offsets.collect();
    wanted.sort_unstable();
    wanted.dedup();
    let mut found = HashMap::with_capacity(wanted.len());
    let mut wanted = wanted.into_iter().peekable();
    let mut units = 0;
    for (byte, c) in s.char_indices().chain(std::iter::once((s.len(), '\0'))) {
        while let Some(&w) = wanted.peek() {
            if w > units {
                break;
            }
            if w < units {
                return None;
            }
            found.insert(w, byte);
            wanted.next();
        }
        units += c.len_utf16();
    }
    wanted.peek().is_none().then_some(found)
}

/// The edited tokens of `edits` that are old ones, as (edited, old) index
/// pairs: kept tokens, edits known to be the same word (`Replace`), and
/// deleted tokens paired in order with tokens inserted in their place.
/// `first_new` is the edited index the edits start at.
pub fn matched_tokens(edits: &[Edit], first_new: usize) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut next = first_new;
    let (mut deleted, mut inserted): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
    let flush = |pairs: &mut Vec<(usize, usize)>, deleted: &mut Vec<usize>, inserted: &mut Vec<usize>| {
        pairs.extend(inserted.iter().copied().zip(deleted.iter().copied()));
        deleted.clear();
        inserted.clear();
    };
    for edit in edits {
        match *edit {
            Edit::Keep(i) => {
                flush(&mut pairs, &mut deleted, &mut inserted);
                pairs.push((next, i));
                next += 1;
            }
            Edit::Delete(i) => deleted.push(i),
            Edit::Insert(j) => {
                inserted.push(j);
                next = j + 1;
            }
            Edit::Replace(i, j) => {
                flush(&mut pairs, &mut deleted, &mut inserted);
                pairs.push((j, i));
                next = j + 1;
            }
        }
    }
    flush(&mut pairs, &mut deleted, &mut inserted);
    pairs
}

/// Whether a tokenized node is one of the tokens the patcher diffs.
pub fn is_token(node: &Node) -> bool {
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
}



/// A token as DSL text, the form in which tokens are compared.
pub fn node_to_dsl(node: &Node) -> String {
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
/// edited (`Insert`) token sequence; `Replace` is an original token edited
/// into an edited one (known to be the same word, not just adjacent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edit {
    Keep(usize),
    Delete(usize),
    Insert(usize),
    Replace(usize, usize),
}

/// Diffs `a` against `b`, where `a[0]` is original token `ao` and `b[0]` is edited
/// token `bo`. Strips the common prefix and suffix before diffing the middle.
pub fn diff_range(a: &[&str], b: &[&str], ao: usize, bo: usize, out: &mut Vec<Edit>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf16_offsets_to_bytes() {
        // þ: 1 unit, 2 bytes; 𝔞: 2 units (surrogate pair), 4 bytes
        let s = "þ𝔞a";
        let at = utf16_to_bytes(s, [0, 1, 3, 4].into_iter()).unwrap();
        assert_eq!((at[&0], at[&1], at[&3], at[&4]), (0, 2, 6, 7));
        // Inside the surrogate pair, or past the end: unusable
        assert!(utf16_to_bytes(s, [2].into_iter()).is_none());
        assert!(utf16_to_bytes(s, [5].into_iter()).is_none());
    }
}
