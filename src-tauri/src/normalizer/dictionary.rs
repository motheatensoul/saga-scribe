use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
struct DiplomaticConfig {
    combining_marks: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct NormalizedConfig {
    character_mappings: HashMap<String, String>,
    ligature_expansions: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct DictionaryJson {
    diplomatic: DiplomaticConfig,
    normalized: NormalizedConfig,
}

/// Entity base letter mappings JSON structure
#[derive(Debug, Deserialize)]
struct EntityMappingsJson {
    mappings: HashMap<String, String>,
}

/// Dictionary for deriving diplomatic and normalized levels from facsimile
#[derive(Default, Debug, Clone)]
pub struct LevelDictionary {
    /// Entity names that are combining marks (to be removed at diplomatic level)
    combining_marks: HashSet<String>,
    /// Character → normalized form mappings
    char_mappings: HashMap<char, String>,
    /// Ligature → expansion mappings
    ligature_mappings: HashMap<char, String>,
    /// Entity name → diplomatic base letter mappings
    entity_mappings: HashMap<String, String>,
}

impl LevelDictionary {
    /// Load dictionary from JSON string
    pub fn load(json: &str) -> Result<Self, String> {
        let parsed: DictionaryJson =
            serde_json::from_str(json).map_err(|e| format!("Failed to parse dictionary: {}", e))?;

        let combining_marks: HashSet<String> =
            parsed.diplomatic.combining_marks.into_iter().collect();

        // Convert string keys to char keys for efficient lookup
        let mut char_mappings = HashMap::new();
        for (k, v) in parsed.normalized.character_mappings {
            if let Some(c) = k.chars().next() {
                char_mappings.insert(c, v);
            }
        }

        let mut ligature_mappings = HashMap::new();
        for (k, v) in parsed.normalized.ligature_expansions {
            if let Some(c) = k.chars().next() {
                ligature_mappings.insert(c, v);
            }
        }

        Ok(Self {
            combining_marks,
            char_mappings,
            ligature_mappings,
            entity_mappings: HashMap::new(),
        })
    }

    /// Load entity base letter mappings from JSON string
    pub fn load_entity_mappings(&mut self, json: &str) -> Result<(), String> {
        let parsed: EntityMappingsJson =
            serde_json::from_str(json).map_err(|e| format!("Failed to parse entity mappings: {}", e))?;
        self.entity_mappings = parsed.mappings;
        Ok(())
    }

    /// Add custom entity mappings (overrides base mappings)
    pub fn add_entity_mappings(&mut self, mappings: HashMap<String, String>) {
        for (k, v) in mappings {
            self.entity_mappings.insert(k, v);
        }
    }

    /// Get the diplomatic base letter for an entity name
    pub fn get_entity_diplomatic(&self, entity_name: &str) -> Option<&str> {
        self.entity_mappings.get(entity_name).map(|s| s.as_str())
    }

    /// Check if an entity name is a combining mark (should be removed at diplomatic level)
    pub fn is_combining_mark(&self, entity_name: &str) -> bool {
        self.combining_marks.contains(entity_name)
    }

    /// Get the normalized form of a character, or None if unchanged
    pub fn normalize_char(&self, c: char) -> Option<&str> {
        self.char_mappings
            .get(&c)
            .or_else(|| self.ligature_mappings.get(&c))
            .map(|s| s.as_str())
    }

    /// Normalize a string by applying character mappings.
    /// Uses pre-allocated buffer to avoid per-character allocations.
    pub fn normalize_text(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        
        for c in text.chars() {
            if let Some(replacement) = self.normalize_char(c) {
                result.push_str(replacement);
            } else {
                result.push(c);
            }
        }
        
        result
    }
}

// impl Default for LevelDictionary {
//     fn default() -> Self {
//         Self {
//             combining_marks: HashSet::new(),
//             char_mappings: HashMap::new(),
//             ligature_mappings: HashMap::new(),
//             entity_mappings: HashMap::new(),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_dictionary() {
        let json = r#"{
            "version": "1.0",
            "description": "Test dictionary",
            "diplomatic": {
                "description": "Test",
                "rules": {},
                "combining_marks": ["combacute", "combgrave"]
            },
            "normalized": {
                "description": "Test",
                "rules": {},
                "character_mappings": {"ſ": "s"},
                "ligature_expansions": {"ﬀ": "ff"}
            },
            "preserved_characters": {"list": []}
        }"#;

        let dict = LevelDictionary::load(json).unwrap();

        assert!(dict.is_combining_mark("combacute"));
        assert!(dict.is_combining_mark("combgrave"));
        assert!(!dict.is_combining_mark("eth"));

        assert_eq!(dict.normalize_char('ſ'), Some("s"));
        assert_eq!(dict.normalize_char('ﬀ'), Some("ff"));
        assert_eq!(dict.normalize_char('a'), None);
    }

    #[test]
    fn test_normalize_text() {
        let json = r#"{
            "version": "1.0",
            "diplomatic": {
                "combining_marks": []
            },
            "normalized": {
                "character_mappings": {"ſ": "s"},
                "ligature_expansions": {}
            }
        }"#;

        let dict = LevelDictionary::load(json).unwrap();
        assert_eq!(dict.normalize_text("ſome teſt"), "some test");
    }
}
