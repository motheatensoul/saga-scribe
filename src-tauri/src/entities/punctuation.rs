//! Which entities are punctuation marks.
//!
//! Decides whether a word made only of entities (`:middot:` between spaces)
//! is punctuation (`<pc>`) rather than a word. Taken from the bundled MENOTA
//! entity list: its "punctuation" category, plus the middle dot, which MENOTA
//! files as "other" but scribes use as their most common punctuation mark.

use std::collections::HashSet;
use std::sync::OnceLock;

const MENOTA_ENTITIES: &str = include_str!("../../../static/entities/menota.json");

/// Punctuation outside MENOTA's "punctuation" category.
const EXTRA: &[&str] = &["middot"];

fn names() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names: HashSet<String> = EXTRA.iter().map(|name| name.to_string()).collect();
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(MENOTA_ENTITIES);
        if let Some(entities) = parsed.ok().as_ref().and_then(|v| v.get("entities")).and_then(|e| e.as_object()) {
            names.extend(
                entities
                    .iter()
                    .filter(|(_, entity)| entity.get("category").and_then(|c| c.as_str()) == Some("punctuation"))
                    .map(|(name, _)| name.clone()),
            );
        }
        names
    })
}

/// Whether the entity `name` is a punctuation mark.
pub fn is_punctuation_entity(name: &str) -> bool {
    names().contains(name)
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
