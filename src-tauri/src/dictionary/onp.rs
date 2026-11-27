use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A headword entry from ONP (as returned by /list/onp)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnpEntry {
    pub release: String,
    pub lemma: String,
    #[serde(rename = "x-lemma_mod")]
    pub lemma_mod: String,
    pub language: String,
    pub id: String,
    #[serde(rename = "partOfSpeech")]
    pub part_of_speech: Vec<String>,
    pub formats: Vec<String>,
    #[serde(rename = "x-citations")]
    pub citations: u32,
}

/// Full entry data from ONP (as returned by /json/onp/{id}). The API changes
/// without notice (by November 2026 senses had lost `onp-key` and
/// `onp-def`), so everything but the definitions is optional: an entry must
/// never fail to load over a field the app doesn't show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnpFullEntry {
    #[serde(rename = "@context")]
    pub context: Option<String>,
    #[serde(rename = "@type")]
    pub entry_type: Option<String>,
    #[serde(default)]
    pub language: String,
    #[serde(rename = "partOfSpeech", default)]
    pub part_of_speech: String,
    #[serde(rename = "canonicalForm", default)]
    pub canonical_form: CanonicalForm,
    #[serde(rename = "onp-citations", default)]
    pub citations: String,
    #[serde(rename = "onp-canonicalurl", default)]
    pub canonical_url: String,
    #[serde(default)]
    pub senses: Vec<Sense>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanonicalForm {
    #[serde(rename = "writtenRep", default)]
    pub written_rep: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sense {
    #[serde(default)]
    pub definition: String,
}

/// Registry for ONP headwords with efficient lookup
#[derive(Debug, Clone, Default)]
pub struct OnpRegistry {
    /// Map from entry ID to entry data
    entries_by_id: HashMap<String, OnpEntry>,
    /// Map from lemma (lowercase) to list of entry IDs (handles homographs)
    entries_by_lemma: HashMap<String, Vec<String>>,
}

impl OnpRegistry {
    pub fn new() -> Self {
        Self {
            entries_by_id: HashMap::new(),
            entries_by_lemma: HashMap::new(),
        }
    }

    /// Load headwords from the ONP list JSON file
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let content = fs::read_to_string(path.as_ref()).map_err(|e| e.to_string())?;
        self.load_from_str(&content)
    }

    /// Load headwords from a JSON string
    pub fn load_from_str(&mut self, json: &str) -> Result<(), String> {
        let entries: Vec<OnpEntry> = serde_json::from_str(json).map_err(|e| e.to_string())?;

        for entry in entries {
            let lemma_lower = entry.lemma.to_lowercase();
            let id = entry.id.clone();

            self.entries_by_lemma
                .entry(lemma_lower)
                .or_default()
                .push(id.clone());

            self.entries_by_id.insert(id, entry);
        }

        Ok(())
    }

    /// Get entry by ID
    pub fn get_by_id(&self, id: &str) -> Option<&OnpEntry> {
        self.entries_by_id.get(id)
    }

    /// Look up entries by lemma (case-insensitive, returns all homographs)
    pub fn lookup_lemma(&self, lemma: &str) -> Vec<&OnpEntry> {
        let lemma_lower = lemma.to_lowercase();
        self.entries_by_lemma
            .get(&lemma_lower)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.entries_by_id.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Search for lemmas that start with a prefix (for autocomplete)
    pub fn search_prefix(&self, prefix: &str, limit: usize) -> Vec<&OnpEntry> {
        let prefix_lower = prefix.to_lowercase();
        self.entries_by_lemma
            .iter()
            .filter(|(lemma, _)| lemma.starts_with(&prefix_lower))
            .flat_map(|(_, ids)| ids.iter().filter_map(|id| self.entries_by_id.get(id)))
            .take(limit)
            .collect()
    }

    /// Get total number of entries
    pub fn len(&self) -> usize {
        self.entries_by_id.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_entry_in_the_current_api_shape_loads() {
        // As /json/onp/o5 answered in November 2026 (shortened): senses
        // without onp-key and onp-def, and PHP warnings in "errors"
        let json = r#"{"@context":"http:\/\/lexinfo.net\/jsonld\/3.0\/content.json","@type":"Word","language":"is",
            "partOfSpeech":"commonNoun","canonicalForm":{"writtenRep":"\u00e1"},"onp-citations":"85",
            "onp-canonicalurl":"https:\/\/onp.ku.dk\/o5","errors":["WARNING [2] Undefined array key 0"],
            "senses":[{"definition":"1) flod, \u00e5, elv \/\/ river","@id":"onp:d1739",
            "ontolex:example":[{"@id":"onp:c573","rdf:value":"x","source":"Eluc674(1989) 023.02"}]}]}"#;
        let entry: OnpFullEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.canonical_form.written_rep, "á");
        assert_eq!(entry.senses[0].definition, "1) flod, å, elv // river");
        // An entry without senses or citations still loads
        let bare: OnpFullEntry = serde_json::from_str(r#"{"canonicalForm":{"writtenRep":"a"}}"#).unwrap();
        assert!(bare.senses.is_empty());
    }

    #[test]
    fn test_load_and_lookup() {
        let json = r#"[
            {"release":"PUBLIC","lemma":"kona","x-lemma_mod":"kona","language":"is","id":"o123","partOfSpeech":["commonNoun"],"formats":["json"],"x-citations":50},
            {"release":"PUBLIC","lemma":"konr","x-lemma_mod":"konr","language":"is","id":"o124","partOfSpeech":["commonNoun"],"formats":["json"],"x-citations":163}
        ]"#;

        let mut registry = OnpRegistry::new();
        registry.load_from_str(json).unwrap();

        assert_eq!(registry.len(), 2);

        let results = registry.lookup_lemma("kona");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "o123");

        let results = registry.lookup_lemma("KONA"); // case insensitive
        assert_eq!(results.len(), 1);

        let results = registry.lookup_lemma("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_homographs() {
        let json = r#"[
            {"release":"PUBLIC","lemma":"á","x-lemma_mod":"á","language":"is","id":"o5","partOfSpeech":["commonNoun"],"formats":["json"],"x-citations":85},
            {"release":"PUBLIC","lemma":"á","x-lemma_mod":"á","language":"is","id":"o9","partOfSpeech":["adposition"],"formats":["json"],"x-citations":1012}
        ]"#;

        let mut registry = OnpRegistry::new();
        registry.load_from_str(json).unwrap();

        let results = registry.lookup_lemma("á");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_prefix_search() {
        let json = r#"[
            {"release":"PUBLIC","lemma":"kona","x-lemma_mod":"kona","language":"is","id":"o1","partOfSpeech":["commonNoun"],"formats":["json"],"x-citations":50},
            {"release":"PUBLIC","lemma":"konr","x-lemma_mod":"konr","language":"is","id":"o2","partOfSpeech":["commonNoun"],"formats":["json"],"x-citations":163},
            {"release":"PUBLIC","lemma":"karl","x-lemma_mod":"karl","language":"is","id":"o3","partOfSpeech":["commonNoun"],"formats":["json"],"x-citations":100}
        ]"#;

        let mut registry = OnpRegistry::new();
        registry.load_from_str(json).unwrap();

        // kona and konr match "kon", but not karl
        let results = registry.search_prefix("kon", 10);
        assert_eq!(results.len(), 2);

        // kona, konr, and karl all match "k"
        let results = registry.search_prefix("k", 10);
        assert_eq!(results.len(), 3);

        let results = registry.search_prefix("k", 2);
        assert_eq!(results.len(), 2);
    }
}
