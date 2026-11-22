//! The base letter of each MUFI letter with a fused abbreviation mark (ꝁ is
//! k with a stroke), read from MUFI's character list
//! (`static/characters/mufi-export.json`, CC BY-SA 4.0). In the shorthand
//! `{:kbar:=konun}gs`, the base letter was written, so the diplomatic level
//! has it outside the expansion: `k<ex>onun</ex>gs`.
//!
//! MUFI gives each character an alphabetical group (`alpha`: `K`, `Þ`, or
//! `_combining` and the like for marks), always in capitals; a character
//! described as SMALL has the group's small letter.

use std::collections::HashMap;
use std::sync::OnceLock;

const MUFI: &str = include_str!("../../../static/characters/mufi-export.json");

/// The base letter of the MUFI character `name` (an entity name without
/// `&` and `;`), if it is a letter other than its base letter.
pub fn base_letter(name: &str) -> Option<&'static str> {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let Ok(serde_json::Value::Array(characters)) = serde_json::from_str::<serde_json::Value>(MUFI) else {
                return HashMap::new();
            };
            characters
                .iter()
                .filter_map(|c| {
                    let field = |key: &str| c.get(key).and_then(|v| v.as_str()).unwrap_or("");
                    let name = field("ent").strip_prefix('&')?.strip_suffix(';')?;
                    let alpha = field("alpha");
                    let mut letters = alpha.chars();
                    let letter = letters.next().filter(|l| l.is_alphabetic() && letters.next().is_none())?;
                    let base = if field("description").contains("SMALL") {
                        letter.to_lowercase().collect::<String>()
                    } else {
                        letter.to_string()
                    };
                    (field("utf8") != base).then(|| (name.to_string(), base))
                })
                .collect()
        })
        .get(name)
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_with_a_fused_mark_have_their_base_letter() {
        assert_eq!(base_letter("kbar"), Some("k"));
        assert_eq!(base_letter("thornbar"), Some("þ"));
        // A mark of its own isn't a letter
        assert_eq!(base_letter("bar"), None);
        assert_eq!(base_letter("er"), None);
        // Nor is a plain letter one with a mark
        assert_eq!(base_letter("nosuchentity"), None);
    }
}
