//! The abbreviation store (step 7 of `docs/DOCUMENT_MODEL.md`): for each
//! abbreviation mark, the expansions it has in the document, the most
//! frequent first. `{mark}` in the shorthand takes the first; the importer
//! writes `{mark}` wherever a file's expansion is that one, and
//! `{mark=expansion}` elsewhere.

use super::segments::Segment;
use crate::parser::syntax::{self, SyntaxKind, SyntaxNode};
use std::collections::{BTreeMap, HashMap};

/// Each mark's expansions (as shorthand), the default first.
pub type Store = BTreeMap<String, Vec<String>>;

/// A mark the importer puts in the store must have its expansion at least
/// this often: a single occurrence is written out.
const AT_LEAST: usize = 2;

/// The `{mark=expansion}` of `dsl`, at any depth: (span, mark, expansion).
fn expansions(dsl: &str) -> Vec<(std::ops::Range<usize>, String, String)> {
    fn walk(nodes: &[SyntaxNode], dsl: &str, out: &mut Vec<(std::ops::Range<usize>, String, String)>) {
        for node in nodes {
            match &node.kind {
                SyntaxKind::Expansion { mark, expansion: Some(expansion) } => {
                    out.push((node.span.clone(), dsl[mark.clone()].to_string(), dsl[expansion.clone()].to_string()));
                }
                SyntaxKind::Supplied { children, .. }
                | SyntaxKind::SuppliedBlock { children, .. }
                | SyntaxKind::Deletion { children, .. }
                | SyntaxKind::Addition { children, .. }
                | SyntaxKind::Head { children, .. }
                | SyntaxKind::Norm { children, .. }
                | SyntaxKind::Unclear { children, .. }
                | SyntaxKind::Numeral { children, .. }
                | SyntaxKind::Gap { children, .. } => walk(children, dsl, out),
                _ => {}
            }
        }
    }
    let tree = syntax::parse(dsl);
    let mut out = Vec::new();
    walk(&tree.nodes, dsl, &mut out);
    out
}

/// The store of a document whose words are `segments`: each mark with the
/// expansions it has, the most frequent first (alphabetically among equals).
/// Marks seen fewer than [`AT_LEAST`] times with their commonest expansion,
/// and expansions without a mark, are left out.
pub fn infer(segments: &[Segment]) -> Store {
    let mut counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for segment in segments {
        let Segment::Word { dsl_content, .. } = segment else { continue };
        if !dsl_content.contains('{') {
            continue;
        }
        for (_, mark, expansion) in expansions(dsl_content) {
            if !mark.trim().is_empty() {
                *counts.entry(mark).or_default().entry(expansion).or_default() += 1;
            }
        }
    }
    counts
        .into_iter()
        .filter_map(|(mark, expansions)| {
            let mut expansions: Vec<(String, usize)> = expansions.into_iter().collect();
            expansions.sort_by(|(a, x), (b, y)| y.cmp(x).then_with(|| a.cmp(b)));
            (expansions[0].1 >= AT_LEAST).then(|| (mark, expansions.into_iter().map(|(e, _)| e).collect()))
        })
        .collect()
}

/// Each mark's default expansion.
pub fn defaults(store: &Store) -> HashMap<String, String> {
    store.iter().filter_map(|(mark, expansions)| Some((mark.clone(), expansions.first()?.clone()))).collect()
}

/// `segments` with `{mark=expansion}` written `{mark}` wherever the
/// expansion is the store's default.
pub fn shorten(segments: &mut [Segment], store: &Store) {
    let defaults = defaults(store);
    if defaults.is_empty() {
        return;
    }
    for segment in segments {
        let Segment::Word { dsl_content, .. } = segment else { continue };
        if !dsl_content.contains('{') {
            continue;
        }
        let mut short = dsl_content.clone();
        for (span, mark, expansion) in expansions(dsl_content).into_iter().rev() {
            if defaults.get(&mark) == Some(&expansion) {
                short.replace_range(span, &format!("{{{mark}}}"));
            }
        }
        *dsl_content = short;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(dsl: &str) -> Segment {
        Segment::Word { id: 0, original_xml: String::new(), attributes: HashMap::new(), dsl_content: dsl.into(), has_inline_lb: false }
    }

    #[test]
    fn the_commonest_expansion_is_the_default_and_written_short() {
        let mut segments = vec![
            word("{:kbar:=konun}gs"),
            word("{:kbar:=konun}gi"),
            word("{:kbar:=kon}a"),
            word("<a{:et:=ok}>"),
            word("{:et:=ok}"),
            word("h{:bar:=ann}"),
            word("hæn{=n}aR"),
        ];
        let store = infer(&segments);
        assert_eq!(store.get(":kbar:"), Some(&vec!["konun".to_string(), "kon".to_string()]));
        assert_eq!(store.get(":et:"), Some(&vec!["ok".to_string()]));
        // Once only: written out
        assert_eq!(store.get(":bar:"), None);
        shorten(&mut segments, &store);
        let dsl: Vec<&str> = segments
            .iter()
            .map(|s| match s {
                Segment::Word { dsl_content, .. } => dsl_content.as_str(),
                _ => "",
            })
            .collect();
        assert_eq!(dsl, ["{:kbar:}gs", "{:kbar:}gi", "{:kbar:=kon}a", "<a{:et:}>", "{:et:}", "h{:bar:=ann}", "hæn{=n}aR"]);
    }
}
