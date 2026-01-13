use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// An inflected form with its morphological analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InflectedForm {
    /// The ONP entry ID this form belongs to
    pub onp_id: String,
    /// The lemma (headword)
    pub lemma: String,
    /// Morphological analysis in MENOTA me:msa format (e.g., "xNC cN nS gF")
    pub analysis: String,
    /// Part of speech / word class
    pub part_of_speech: String,
    /// Facsimile-level form (with entity references resolved to glyphs)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facsimile: Option<String>,
    /// Diplomatic-level form (base letters, abbreviations expanded)
    /// This is the primary lookup key for matching orthographic variants
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diplomatic: Option<String>,
    /// Canonical normalized form for <me:norm> level (user-provided)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
}

/// Store for user-built inflection mappings
/// Maps diplomatic-level wordforms to their possible lemmas and analyses.
/// Using diplomatic form as the key catches orthographic variants that resolve
/// to the same base letters (e.g., open o vs closed o both → ǫ).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InflectionStore {
    /// Map from diplomatic wordform (lowercase) to list of possible analyses
    /// One wordform can map to multiple lemmas (e.g., "var" could be past of "vera" or "verja")
    forms: HashMap<String, Vec<InflectedForm>>,
}

impl InflectionStore {
    pub fn new() -> Self {
        Self {
            forms: HashMap::new(),
        }
    }

    /// Get the path to the inflections file in app data
    fn get_path(app: &AppHandle) -> Result<PathBuf, String> {
        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {}", e))?;
        fs::create_dir_all(&app_data).map_err(|e| format!("Failed to create app data dir: {}", e))?;
        Ok(app_data.join("inflections.json"))
    }

    /// Load inflections from app data
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let path = Self::get_path(app)?;
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    /// Save inflections to app data
    pub fn save(&self, app: &AppHandle) -> Result<(), String> {
        let path = Self::get_path(app)?;
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, content).map_err(|e| e.to_string())
    }

    /// Normalize a wordform for lookup (lowercase, strip combining marks)
    fn normalize(form: &str) -> String {
        form.to_lowercase()
    }

    /// Add or update an inflection mapping
    pub fn add(&mut self, wordform: &str, inflection: InflectedForm) {
        let normalized = Self::normalize(wordform);
        let forms = self.forms.entry(normalized).or_default();

        // Check if this exact mapping already exists
        let exists = forms.iter().any(|f| {
            f.onp_id == inflection.onp_id && f.analysis == inflection.analysis
        });

        if !exists {
            forms.push(inflection);
        }
    }

    /// Remove a specific inflection mapping
    pub fn remove(&mut self, wordform: &str, onp_id: &str, analysis: &str) {
        let normalized = Self::normalize(wordform);
        if let Some(forms) = self.forms.get_mut(&normalized) {
            forms.retain(|f| !(f.onp_id == onp_id && f.analysis == analysis));
            if forms.is_empty() {
                self.forms.remove(&normalized);
            }
        }
    }

    /// Look up all possible analyses for a wordform
    pub fn lookup(&self, wordform: &str) -> Vec<&InflectedForm> {
        let normalized = Self::normalize(wordform);
        self.forms
            .get(&normalized)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check if a wordform has any mappings (used in tests)
    #[allow(dead_code)]
    pub fn contains(&self, wordform: &str) -> bool {
        let normalized = Self::normalize(wordform);
        self.forms.contains_key(&normalized)
    }

    /// Clear all mappings (kept for potential future use)
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.forms.clear();
    }

    /// Get the total number of wordform entries
    pub fn entry_count(&self) -> usize {
        self.forms.len()
    }

    // Get the total number of inflection mappings (across all wordforms)
    // pub fn mapping_count(&self) -> usize {
    //     self.forms.values().map(|v| v.len()).sum()
    // }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_lookup() {
        let mut store = InflectionStore::new();

        store.add(
            "konur",
            InflectedForm {
                onp_id: "o123".to_string(),
                lemma: "kona".to_string(),
                analysis: "nom.pl.f".to_string(),
                part_of_speech: "commonNoun".to_string(),
                facsimile: None,
                diplomatic: Some("konur".to_string()),
                normalized: None,
            },
        );

        let results = store.lookup("konur");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lemma, "kona");
        assert_eq!(results[0].analysis, "nom.pl.f");

        // Case insensitive lookup
        let results = store.lookup("KONUR");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_multiple_analyses() {
        let mut store = InflectionStore::new();

        // "var" can be past tense of "vera" (to be) or "verja" (to defend)
        store.add(
            "var",
            InflectedForm {
                onp_id: "o1".to_string(),
                lemma: "vera".to_string(),
                analysis: "pret.ind.1/3sg".to_string(),
                part_of_speech: "verb".to_string(),
                facsimile: None,
                diplomatic: Some("var".to_string()),
                normalized: None,
            },
        );
        store.add(
            "var",
            InflectedForm {
                onp_id: "o2".to_string(),
                lemma: "verja".to_string(),
                analysis: "pret.ind.1/3sg".to_string(),
                part_of_speech: "verb".to_string(),
                facsimile: None,
                diplomatic: Some("var".to_string()),
                normalized: None,
            },
        );

        let results = store.lookup("var");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_no_duplicates() {
        let mut store = InflectionStore::new();

        let form = InflectedForm {
            onp_id: "o123".to_string(),
            lemma: "kona".to_string(),
            analysis: "nom.pl.f".to_string(),
            part_of_speech: "commonNoun".to_string(),
            facsimile: None,
            diplomatic: Some("konur".to_string()),
            normalized: None,
        };

        store.add("konur", form.clone());
        store.add("konur", form.clone());

        let results = store.lookup("konur");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut store = InflectionStore::new();

        store.add(
            "konur",
            InflectedForm {
                onp_id: "o123".to_string(),
                lemma: "kona".to_string(),
                analysis: "nom.pl.f".to_string(),
                part_of_speech: "commonNoun".to_string(),
                facsimile: None,
                diplomatic: Some("konur".to_string()),
                normalized: None,
            },
        );

        assert!(store.contains("konur"));
        store.remove("konur", "o123", "nom.pl.f");
        assert!(!store.contains("konur"));
    }
}
