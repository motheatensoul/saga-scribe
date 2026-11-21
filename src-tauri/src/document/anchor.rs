//! Finding a word again in a saved file (`docs/DOCUMENT_MODEL.md`, "Words in
//! the project data"): data kept in the project refers to a word by its
//! place (the n-th `<w>` of the body), checked by a fingerprint of the word.

use super::project::find_body;
use super::tree::{NodeId, NodeKind, Tree};

/// How far from its place a word is looked for when the word there doesn't
/// match its fingerprint (the file was changed outside the app).
const SEARCH: usize = 25;

/// The `<w>` elements of the body, in document order.
pub fn body_words(tree: &Tree) -> Vec<NodeId> {
    let Some(body) = find_body(tree) else { return Vec::new() };
    tree.descendants(body)
        .into_iter()
        .filter(|&id| tree.element(id).is_some_and(|e| e.name.rsplit(':').next() == Some("w")))
        .collect()
}

/// A word's fingerprint: its text (entity references as written) and its
/// lemma. Markup and formatting don't count, so a word reads the same
/// whether it was copied or written anew.
pub fn fingerprint(tree: &Tree, word: NodeId) -> String {
    let mut text = String::new();
    for id in tree.descendants(word) {
        match tree.kind(id) {
            NodeKind::Text(t) => text.extend(t.chars().filter(|c| !c.is_whitespace())),
            NodeKind::EntityRef(name) => {
                text.push('&');
                text.push_str(name);
                text.push(';');
            }
            _ => {}
        }
    }
    let lemma = tree.element(word).and_then(|e| e.attribute("lemma")).unwrap_or("");
    format!("{text}|{lemma}")
}

/// The word that is `n`-th among `words` with `fingerprint`, or the nearest
/// one with it within [`SEARCH`] places; `None` if there is none.
pub fn find_word(tree: &Tree, words: &[NodeId], n: usize, print: &str) -> Option<usize> {
    let matches = |i: usize| words.get(i).is_some_and(|&w| fingerprint(tree, w) == print);
    if matches(n) {
        return Some(n);
    }
    (1..=SEARCH).find_map(|d| {
        if matches(n + d) {
            Some(n + d)
        } else {
            n.checked_sub(d).filter(|&i| matches(i))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_found_by_place_and_fingerprint() {
        let tree = Tree::parse(
            r#"<TEI><text><body><p><w lemma="a">a</w> <w>b&amp;</w> <w lemma="c"><x>c</x></w></p></body></text></TEI>"#,
        )
        .unwrap();
        let words = body_words(&tree);
        assert_eq!(words.len(), 3);
        let print = fingerprint(&tree, words[2]);
        assert_eq!(print, "c|c");
        assert_eq!(find_word(&tree, &words, 2, &print), Some(2));
        // A word was inserted before it outside the app: found nearby
        assert_eq!(find_word(&tree, &words, 1, &print), Some(2));
        assert_eq!(find_word(&tree, &words, 0, "zz|"), None);
    }
}
