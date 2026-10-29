//! Which entities are punctuation marks.
//!
//! Decides whether a word made only of entities (`:middot:` between spaces)
//! is punctuation (`<pc>`) rather than a word. Taken from the bundled MENOTA
//! entity list: its "punctuation" category, plus the middle dot, which MENOTA
//! files as "other" but scribes use as their most common punctuation mark.

use super::menota::bundled;

/// Punctuation outside MENOTA's "punctuation" category.
const EXTRA: &[&str] = &["middot"];

/// Whether the entity `name` is a punctuation mark.
pub fn is_punctuation_entity(name: &str) -> bool {
    EXTRA.contains(&name) || bundled().get(name).is_some_and(|entity| entity.category == "punctuation")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_punctuation_entities() {
        assert!(is_punctuation_entity("middot"));
        assert!(is_punctuation_entity("punctelev"));
        assert!(is_punctuation_entity("period"));
        // Letters and abbreviation signs are not
        assert!(!is_punctuation_entity("inodot"));
        assert!(!is_punctuation_entity("etslash"));
        assert!(!is_punctuation_entity("et"));
    }
}
