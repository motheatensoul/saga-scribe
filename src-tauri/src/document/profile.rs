//! The project profile (step 4 of `docs/DOCUMENT_MODEL.md`): how a file
//! encodes its transcription, inferred from the file and confirmed on
//! import. The compiler, the importer and the word panel ask it instead of
//! assuming MENOTA's three levels.

use super::tree::{NodeId, Tree};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A transcription level of a word (MENOTA's `me:facs`, `me:dipl`, `me:norm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Facs,
    Dipl,
    Norm,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Facs, Level::Dipl, Level::Norm];

    /// The element's local name (`facs` for `<me:facs>`).
    pub fn local_name(self) -> &'static str {
        match self {
            Level::Facs => "facs",
            Level::Dipl => "dipl",
            Level::Norm => "norm",
        }
    }
}

/// How abbreviations are encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Abbreviations {
    /// `<am>` for the mark and `<ex>` for the expansion (MENOTA)
    AmEx,
    /// `<choice><abbr>…</abbr><expan>…</expan></choice>` (TEI)
    ChoiceAbbrExpan,
}

/// The element punctuation is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Punctuation {
    /// `<pc>`
    Pc,
    /// `<me:punct>`
    MePunct,
}

/// How often the file uses each choice the profile makes, for confirming it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCounts {
    /// `<w>` elements in the body
    pub words: usize,
    /// Words, numerals and punctuation (`<w>`, `<num>`, `<pc>`), whose
    /// levels are counted
    pub tokens: usize,
    /// Tokens with each level
    pub levels: BTreeMap<Level, usize>,
    /// Tokens whose levels are in a `<choice>`
    pub words_with_choice: usize,
    pub am: usize,
    pub ex: usize,
    pub choice_abbr: usize,
    pub pc: usize,
    pub me_punct: usize,
    /// Line breaks by edition (`ed`; the empty string for none)
    pub line_editions: BTreeMap<String, usize>,
    /// Elements in the body the shorthand has no syntax for, by name: kept
    /// as they are, but only seen in the XML view or the word panel
    pub hidden: BTreeMap<String, usize>,
}

/// Elements the shorthand shows (by local name), or that hold the text it
/// shows: everything else in the body is hidden from it.
pub(crate) const SHOWN: &[&str] = &[
    "body", "div", "p", "head", "w", "pc", "punct", "num", "choice", "facs", "dipl", "norm", "abbr", "expan", "am", "ex",
    "lb", "pb", "gap", "supplied", "del", "add", "unclear", "note",
];

/// A file's transcription encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// The levels a word has, in order (none: words are plain text)
    pub levels: Vec<Level>,
    /// Whether a word's levels are wrapped in `<choice>`
    pub choice: bool,
    pub abbreviations: Abbreviations,
    pub punctuation: Punctuation,
    /// The edition whose line breaks `//n` stands for, when the file has
    /// several (`<lb ed="…">`); `None` if it has one
    pub line_edition: Option<String>,
    pub counts: ProfileCounts,
}

impl Profile {
    /// The level the shorthand writes: facs if there is one, otherwise dipl,
    /// otherwise the first.
    pub fn primary(&self) -> Option<Level> {
        [Level::Facs, Level::Dipl].into_iter().find(|l| self.levels.contains(l)).or(self.levels.first().copied())
    }

    pub fn has(&self, level: Level) -> bool {
        self.levels.contains(&level)
    }

    /// MENOTA's three levels in a `<choice>`: what new documents and files
    /// without a profile get.
    pub fn menota() -> Self {
        Self {
            levels: Level::ALL.to_vec(),
            choice: true,
            abbreviations: Abbreviations::AmEx,
            punctuation: Punctuation::Pc,
            line_edition: None,
            counts: ProfileCounts::default(),
        }
    }

    /// Words without levels.
    pub fn plain() -> Self {
        Self { levels: Vec::new(), choice: false, ..Self::menota() }
    }
}

/// A level that at least this share of the words have counts as one of the
/// file's levels (a few stray elements don't add a level).
const LEVEL_SHARE: f64 = 0.05;

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// The profile of the file whose body is `body`.
pub fn infer(tree: &Tree, body: NodeId) -> Profile {
    let mut counts = ProfileCounts::default();
    for id in tree.descendants(body) {
        let Some(element) = tree.element(id) else { continue };
        let name = element.name.as_str();
        let local_name = local(name);
        if local_name == "w" {
            counts.words += 1;
        }
        if !SHOWN.contains(&local_name) {
            *counts.hidden.entry(name.to_string()).or_default() += 1;
        }
        match local_name {
            "w" | "num" | "pc" => {
                counts.tokens += 1;
                let mut has_choice = false;
                let mut seen = [false; 3];
                let mut stack: Vec<NodeId> = tree.children(id).to_vec();
                while let Some(child) = stack.pop() {
                    let Some(e) = tree.element(child) else { continue };
                    match (e.name.split_once(':').map(|(p, _)| p), local(&e.name)) {
                        (_, "choice") => {
                            has_choice = true;
                            stack.extend_from_slice(tree.children(child));
                        }
                        (Some("me"), l) => {
                            if let Some(i) = Level::ALL.iter().position(|level| level.local_name() == l) {
                                seen[i] = true;
                            }
                        }
                        _ => {}
                    }
                }
                for (i, level) in Level::ALL.iter().enumerate() {
                    if seen[i] {
                        *counts.levels.entry(*level).or_default() += 1;
                    }
                }
                if has_choice && seen.iter().any(|&s| s) {
                    counts.words_with_choice += 1;
                }
                if local_name == "pc" {
                    counts.pc += 1;
                }
            }
            "am" => counts.am += 1,
            "ex" => counts.ex += 1,
            "abbr" => counts.choice_abbr += 1,
            "punct" if name.starts_with("me:") => counts.me_punct += 1,
            "lb" => *counts.line_editions.entry(element.attribute("ed").unwrap_or("").to_string()).or_default() += 1,
            _ => {}
        }
    }

    let levels: Vec<Level> = Level::ALL
        .into_iter()
        .filter(|level| {
            let n = counts.levels.get(level).copied().unwrap_or(0);
            n > 0 && n as f64 >= LEVEL_SHARE * counts.tokens as f64
        })
        .collect();
    let with_levels = counts.levels.values().copied().max().unwrap_or(0);
    let named: Vec<(&String, &usize)> = counts.line_editions.iter().filter(|(ed, _)| !ed.is_empty()).collect();
    // Several editions: the manuscript's, or the one with most line breaks
    let line_edition = (named.len() > 1).then(|| {
        named
            .iter()
            .find(|(ed, _)| ed.as_str() == "ms")
            .or_else(|| named.iter().max_by_key(|(_, n)| **n))
            .map(|(ed, _)| (*ed).clone())
    });
    Profile {
        choice: counts.words_with_choice * 2 > with_levels,
        abbreviations: if counts.choice_abbr > counts.am + counts.ex { Abbreviations::ChoiceAbbrExpan } else { Abbreviations::AmEx },
        punctuation: if counts.me_punct > counts.pc { Punctuation::MePunct } else { Punctuation::Pc },
        line_edition: line_edition.flatten(),
        levels,
        counts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::find_body;

    fn profile_of(body: &str) -> Profile {
        let xml = format!(r#"<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body>{body}</body></text></TEI>"#);
        let tree = Tree::parse(&xml).unwrap();
        infer(&tree, find_body(&tree).unwrap())
    }

    #[test]
    fn three_levels_in_a_choice() {
        let p = profile_of(
            "<p><w><choice><me:facs>a</me:facs><me:dipl>a</me:dipl><me:norm>a</me:norm></choice></w> <pc>.</pc><lb ed=\"ms\" n=\"2\"/></p>",
        );
        assert_eq!((p.levels.as_slice(), p.choice, p.primary()), (&Level::ALL[..], true, Some(Level::Facs)));
        assert_eq!((p.punctuation, p.line_edition.as_deref()), (Punctuation::Pc, None));
    }

    #[test]
    fn a_diplomatic_charter_with_two_lineations() {
        let p = profile_of(
            "<p><w><me:dipl>al<ex>en</ex></me:dipl></w><lb ed=\"DN\" n=\"3\"/> <w><me:dipl>b</me:dipl></w><lb ed=\"ms\" n=\"2\"/><me:punct>.</me:punct></p>",
        );
        assert_eq!((p.levels.as_slice(), p.choice, p.primary()), (&[Level::Dipl][..], false, Some(Level::Dipl)));
        assert_eq!((p.abbreviations, p.punctuation), (Abbreviations::AmEx, Punctuation::MePunct));
        assert_eq!(p.line_edition.as_deref(), Some("ms"));
        assert_eq!(p.counts.ex, 1);
    }

    #[test]
    fn markup_the_shorthand_doesnt_show() {
        let p = profile_of("<p><s><w>a<hi rend=\"red\">b</hi></w></s> <w>c<hi>d</hi></w></p>");
        assert_eq!(p.counts.hidden.into_iter().collect::<Vec<_>>(), vec![("hi".to_string(), 2), ("s".to_string(), 1)]);
    }

    #[test]
    fn plain_words() {
        let p = profile_of("<p><w>a</w> <w>b</w></p>");
        assert!(p.levels.is_empty() && p.primary().is_none());
    }
}
