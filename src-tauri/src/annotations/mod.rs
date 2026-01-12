//! Annotation system for word-, character-, and span-level annotations.
//!
//! This module provides a flexible annotation schema that extends beyond
//! simple lemmatization to support semantic categories, notes, paleographic
//! observations, syntactic structures, and cross-references.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single annotation attached to a target in the text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    /// Unique identifier for this annotation
    pub id: String,

    /// Type of annotation
    #[serde(rename = "type")]
    pub annotation_type: AnnotationType,

    /// What this annotation targets (word, character range, or span)
    pub target: AnnotationTarget,

    /// The annotation content/value
    pub value: AnnotationValue,

    /// Optional metadata about the annotation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<AnnotationMetadata>,
}

/// Types of annotations supported
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AnnotationType {
    /// Lemma with morphological analysis (existing functionality)
    Lemma,
    /// Semantic category (named entities, concepts)
    Semantic,
    /// Scholarly note or comment
    Note,
    /// Paleographic observation (glyph forms, damage, unclear readings)
    Paleographic,
    /// Syntactic structure (phrases, clauses)
    Syntax,
    /// Cross-reference to other text or external resource
    Reference,
    /// User-defined custom annotation
    Custom,
}

/// Target specification for an annotation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AnnotationTarget {
    /// Single word by index
    Word {
        /// Zero-based word index in the document
        #[serde(rename = "wordIndex")]
        word_index: u32,
    },
    /// Character range within a word
    #[serde(rename = "char")]
    Character {
        /// Word containing the characters
        #[serde(rename = "wordIndex")]
        word_index: u32,
        /// Start character offset (0-based, inclusive)
        #[serde(rename = "charStart")]
        char_start: u32,
        /// End character offset (exclusive)
        #[serde(rename = "charEnd")]
        char_end: u32,
    },
    /// Span of multiple consecutive words
    Span {
        /// First word index (inclusive)
        #[serde(rename = "startWord")]
        start_word: u32,
        /// Last word index (inclusive)
        #[serde(rename = "endWord")]
        end_word: u32,
    },
}

/// Annotation value - the actual content of the annotation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AnnotationValue {
    /// Lemma annotation (dictionary headword + morphological analysis)
    Lemma {
        /// Dictionary headword
        lemma: String,
        /// MENOTA morphological analysis (me:msa)
        msa: String,
        /// Normalized form for <me:norm>
        #[serde(default, skip_serializing_if = "Option::is_none")]
        normalized: Option<String>,
        /// ONP dictionary ID if available
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "onpId")]
        onp_id: Option<String>,
    },
    /// Semantic category annotation
    Semantic {
        /// Category (e.g., "person", "place", "organization", "concept")
        category: String,
        /// Subcategory (e.g., "masculine-name", "toponym", "divine")
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subcategory: Option<String>,
        /// Canonical identifier (e.g., Wikidata ID, authority file)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        identifier: Option<String>,
        /// Display label
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    /// Free-text note
    Note {
        /// Note content
        text: String,
        /// Note category (e.g., "editorial", "translation", "commentary")
        #[serde(default, skip_serializing_if = "Option::is_none")]
        category: Option<String>,
    },
    /// Paleographic observation
    Paleographic {
        /// Type of observation
        #[serde(rename = "observationType")]
        observation_type: PaleographicType,
        /// Description of the observation
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Certainty level (0.0 to 1.0)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        certainty: Option<f32>,
    },
    /// Syntactic structure
    Syntax {
        /// Syntactic function (e.g., "subject", "object", "clause")
        function: String,
        /// Additional grammatical information
        #[serde(default, skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
    /// Cross-reference
    Reference {
        /// Target URI or identifier
        target: String,
        /// Type of reference (e.g., "citation", "parallel", "variant")
        #[serde(rename = "refType")]
        ref_type: String,
        /// Display text
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    /// Custom user-defined annotation
    Custom {
        /// Custom type name
        #[serde(rename = "customType")]
        custom_type: String,
        /// Arbitrary key-value data
        data: HashMap<String, String>,
    },
    /// MENOTA-specific paleographic observation with detailed attributes
    #[serde(rename = "menota-paleographic")]
    MenotaPaleographic {
        /// The type of observation
        #[serde(rename = "observationType")]
        observation_type: MenotaObservationType,
        /// For unclear: the reason
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "unclearReason")]
        unclear_reason: Option<MenotaUnclearReason>,
        /// For addition: placement
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "addPlace")]
        add_place: Option<MenotaAddPlace>,
        /// For addition: type
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "addType")]
        add_type: Option<MenotaAddType>,
        /// For addition/deletion: hand identifier
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hand: Option<String>,
        /// For deletion: rendering method
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "delRend")]
        del_rend: Option<MenotaDelRend>,
        /// For supplied: reason
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "suppliedReason")]
        supplied_reason: Option<MenotaSuppliedReason>,
        /// For supplied: responsibility
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resp: Option<String>,
        /// For supplied: source
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        /// For character: type (initial, capital, etc.)
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "charType")]
        char_type: Option<MenotaCharType>,
        /// For character: size (for initials, in lines)
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "charSize")]
        char_size: Option<u32>,
        /// General description
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Certainty level (0.0 to 1.0)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        certainty: Option<f32>,
    },
}

/// Types of paleographic observations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaleographicType {
    /// Text is unclear or hard to read
    Unclear,
    /// Physical damage to the manuscript
    Damage,
    /// Erasure or scraping
    Erasure,
    /// Unusual letterform or ligature
    Letterform,
    /// Abbreviation mark or sign
    Abbreviation,
    /// Correction by scribe
    Correction,
    /// Later addition or marginalia
    Addition,
    /// Rubrication or decoration
    Decoration,
    /// Other paleographic note
    Other,
}

// ============================================================================
// MENOTA-Specific Types (based on MENOTA Handbook v3)
// ============================================================================

/// MENOTA unclear reading reasons (HB3 ch.9)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenotaUnclearReason {
    /// Cannot be read at all
    Illegible,
    /// Ink has faded
    Faded,
    /// Ink is smudged
    Smudged,
    /// Physical damage to manuscript
    Damage,
    /// Text has been erased
    Erasure,
    /// Text written over other text
    Overwriting,
    /// Hidden in binding
    Binding,
    /// Other reason
    Other,
}

/// MENOTA addition placement (HB3 ch.9)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MenotaAddPlace {
    Inline,
    Supralinear,
    Infralinear,
    MarginLeft,
    MarginRight,
    MarginTop,
    MarginBottom,
    Interlinear,
}

/// MENOTA addition type (HB3 ch.9)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenotaAddType {
    /// Scribe's own addition
    Supplement,
    /// Explanatory gloss
    Gloss,
    /// Correction by scribe
    Correction,
}

/// MENOTA deletion rendering (HB3 ch.9)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenotaDelRend {
    /// Struck through
    Overstrike,
    /// Erased
    Erasure,
    /// Dots beneath
    Subpunction,
    /// Dots above
    Expunction,
    /// Bracketed for deletion
    Bracketed,
}

/// MENOTA supplied reason (HB3 ch.9)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenotaSuppliedReason {
    /// Scribe omitted
    Omitted,
    /// Lost due to damage
    Damage,
    /// Cannot be read
    Illegible,
    /// Editorial restoration
    Restoration,
    /// Editorial emendation
    Emendation,
}

/// MENOTA character type for <c> element (HB3 ch.4)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenotaCharType {
    /// Decorated initial
    Initial,
    /// Capital letter (littera notabilior)
    Capital,
    /// Rubricated character
    Rubric,
    /// Other colored character
    Colored,
}

/// MENOTA-specific paleographic observation types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenotaObservationType {
    /// Unclear reading
    Unclear,
    /// Scribal addition
    Addition,
    /// Scribal deletion
    Deletion,
    /// Editorial supplied text
    Supplied,
    /// Character-level annotation (initials, capitals)
    Character,
}

/// Metadata about an annotation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationMetadata {
    /// Who created this annotation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// When the annotation was created (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,

    /// When the annotation was last modified
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,

    /// Confidence level (0.0 to 1.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,

    /// Source of the annotation (e.g., "auto", "manual", "imported")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Additional notes about the annotation itself
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Collection of all annotations for a document
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationSet {
    /// Schema version for forward compatibility
    pub version: String,

    /// All annotations in the document
    pub annotations: Vec<Annotation>,
}

#[allow(dead_code)]
impl AnnotationSet {
    /// Create a new empty annotation set
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            annotations: Vec::new(),
        }
    }

    /// Add an annotation
    pub fn add(&mut self, annotation: Annotation) {
        self.annotations.push(annotation);
    }

    /// Remove an annotation by ID
    pub fn remove(&mut self, id: &str) -> Option<Annotation> {
        if let Some(pos) = self.annotations.iter().position(|a| a.id == id) {
            Some(self.annotations.remove(pos))
        } else {
            None
        }
    }

    /// Get annotation by ID
    pub fn get(&self, id: &str) -> Option<&Annotation> {
        self.annotations.iter().find(|a| a.id == id)
    }

    /// Get mutable annotation by ID
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Annotation> {
        self.annotations.iter_mut().find(|a| a.id == id)
    }

    /// Get all annotations for a specific word index
    pub fn for_word(&self, word_index: u32) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.target.includes_word(word_index))
            .collect()
    }

    /// Get all annotations of a specific type
    pub fn by_type(&self, annotation_type: AnnotationType) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.annotation_type == annotation_type)
            .collect()
    }

    /// Get all lemma annotations as a map (for backward compatibility)
    pub fn lemma_map(&self) -> HashMap<u32, LemmaInfo> {
        let mut map = HashMap::new();
        for ann in &self.annotations {
            if let (
                AnnotationType::Lemma,
                AnnotationTarget::Word { word_index },
                AnnotationValue::Lemma {
                    lemma,
                    msa,
                    normalized,
                    ..
                },
            ) = (&ann.annotation_type, &ann.target, &ann.value)
            {
                map.insert(
                    *word_index,
                    LemmaInfo {
                        lemma: lemma.clone(),
                        msa: msa.clone(),
                        normalized: normalized.clone(),
                    },
                );
            }
        }
        map
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }

    /// Get the number of annotations
    pub fn len(&self) -> usize {
        self.annotations.len()
    }
}

/// Simplified lemma info for backward compatibility
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LemmaInfo {
    pub lemma: String,
    pub msa: String,
    pub normalized: Option<String>,
}

#[allow(dead_code)]
impl AnnotationTarget {
    /// Check if this target includes a specific word index
    pub fn includes_word(&self, word_index: u32) -> bool {
        match self {
            AnnotationTarget::Word { word_index: idx } => *idx == word_index,
            AnnotationTarget::Character { word_index: idx, .. } => *idx == word_index,
            AnnotationTarget::Span {
                start_word,
                end_word,
            } => word_index >= *start_word && word_index <= *end_word,
        }
    }

    /// Get the primary word index (first word for spans)
    pub fn primary_word_index(&self) -> u32 {
        match self {
            AnnotationTarget::Word { word_index } => *word_index,
            AnnotationTarget::Character { word_index, .. } => *word_index,
            AnnotationTarget::Span { start_word, .. } => *start_word,
        }
    }
}

#[allow(dead_code)]
impl Annotation {
    /// Create a new lemma annotation (convenience constructor)
    pub fn lemma(word_index: u32, lemma: String, msa: String, normalized: Option<String>) -> Self {
        Self {
            id: format!("lemma-{}", word_index),
            annotation_type: AnnotationType::Lemma,
            target: AnnotationTarget::Word { word_index },
            value: AnnotationValue::Lemma {
                lemma,
                msa,
                normalized,
                onp_id: None,
            },
            metadata: None,
        }
    }

    /// Create a new note annotation
    pub fn note(target: AnnotationTarget, text: String, category: Option<String>) -> Self {
        let id = format!("note-{}", uuid_simple());
        Self {
            id,
            annotation_type: AnnotationType::Note,
            target,
            value: AnnotationValue::Note { text, category },
            metadata: None,
        }
    }

    /// Create a new semantic annotation
    pub fn semantic(
        target: AnnotationTarget,
        category: String,
        subcategory: Option<String>,
        label: Option<String>,
    ) -> Self {
        let id = format!("sem-{}", uuid_simple());
        Self {
            id,
            annotation_type: AnnotationType::Semantic,
            target,
            value: AnnotationValue::Semantic {
                category,
                subcategory,
                identifier: None,
                label,
            },
            metadata: None,
        }
    }
}

/// Generate a simple unique ID (timestamp + random suffix)
#[allow(dead_code)]
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let millis = now.as_millis() as u64;
    let nanos = now.subsec_nanos();
    format!("{:x}{:04x}", millis, (nanos & 0xFFFF) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lemma_annotation() {
        let ann = Annotation::lemma(42, "kona".to_string(), "xNC cN nS gF".to_string(), None);

        assert_eq!(ann.annotation_type, AnnotationType::Lemma);
        assert!(matches!(ann.target, AnnotationTarget::Word { word_index: 42 }));

        if let AnnotationValue::Lemma { lemma, msa, .. } = &ann.value {
            assert_eq!(lemma, "kona");
            assert_eq!(msa, "xNC cN nS gF");
        } else {
            panic!("Expected Lemma value");
        }
    }

    #[test]
    fn test_annotation_set_operations() {
        let mut set = AnnotationSet::new();
        assert!(set.is_empty());

        set.add(Annotation::lemma(
            0,
            "maðr".to_string(),
            "xNC cN nS gM".to_string(),
            None,
        ));
        set.add(Annotation::lemma(
            1,
            "vera".to_string(),
            "xVB tPT".to_string(),
            None,
        ));

        assert_eq!(set.len(), 2);

        let for_word_0 = set.for_word(0);
        assert_eq!(for_word_0.len(), 1);

        let lemmas = set.by_type(AnnotationType::Lemma);
        assert_eq!(lemmas.len(), 2);

        let lemma_map = set.lemma_map();
        assert!(lemma_map.contains_key(&0));
        assert!(lemma_map.contains_key(&1));
    }

    #[test]
    fn test_target_includes_word() {
        let word_target = AnnotationTarget::Word { word_index: 5 };
        assert!(word_target.includes_word(5));
        assert!(!word_target.includes_word(4));

        let span_target = AnnotationTarget::Span {
            start_word: 10,
            end_word: 15,
        };
        assert!(span_target.includes_word(10));
        assert!(span_target.includes_word(12));
        assert!(span_target.includes_word(15));
        assert!(!span_target.includes_word(9));
        assert!(!span_target.includes_word(16));

        let char_target = AnnotationTarget::Character {
            word_index: 7,
            char_start: 0,
            char_end: 2,
        };
        assert!(char_target.includes_word(7));
        assert!(!char_target.includes_word(8));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut set = AnnotationSet::new();
        set.add(Annotation::lemma(
            0,
            "kona".to_string(),
            "xNC".to_string(),
            Some("kona".to_string()),
        ));
        set.add(Annotation::note(
            AnnotationTarget::Word { word_index: 1 },
            "This is a note".to_string(),
            Some("editorial".to_string()),
        ));

        let json = serde_json::to_string(&set).unwrap();
        let restored: AnnotationSet = serde_json::from_str(&json).unwrap();

        assert_eq!(set, restored);
    }

    #[test]
    fn test_semantic_annotation() {
        let ann = Annotation::semantic(
            AnnotationTarget::Span {
                start_word: 5,
                end_word: 7,
            },
            "person".to_string(),
            Some("masculine-name".to_string()),
            Some("Gunnarr".to_string()),
        );

        assert_eq!(ann.annotation_type, AnnotationType::Semantic);
        if let AnnotationValue::Semantic {
            category,
            subcategory,
            label,
            ..
        } = &ann.value
        {
            assert_eq!(category, "person");
            assert_eq!(subcategory.as_deref(), Some("masculine-name"));
            assert_eq!(label.as_deref(), Some("Gunnarr"));
        }
    }

    #[test]
    fn test_camelcase_serialization() {
        // Verify that field names serialize to camelCase for frontend compatibility
        let target_word = AnnotationTarget::Word { word_index: 42 };
        let json = serde_json::to_string(&target_word).unwrap();
        assert!(json.contains("wordIndex"), "Expected camelCase 'wordIndex' in: {}", json);
        assert!(!json.contains("word_index"), "Should not contain snake_case 'word_index' in: {}", json);

        let target_char = AnnotationTarget::Character {
            word_index: 5,
            char_start: 2,
            char_end: 4,
        };
        let json = serde_json::to_string(&target_char).unwrap();
        assert!(json.contains("charStart"), "Expected 'charStart' in: {}", json);
        assert!(json.contains("charEnd"), "Expected 'charEnd' in: {}", json);

        let target_span = AnnotationTarget::Span {
            start_word: 10,
            end_word: 15,
        };
        let json = serde_json::to_string(&target_span).unwrap();
        assert!(json.contains("startWord"), "Expected 'startWord' in: {}", json);
        assert!(json.contains("endWord"), "Expected 'endWord' in: {}", json);
    }

    #[test]
    fn test_camelcase_deserialization() {
        // Verify that camelCase JSON from frontend deserializes correctly
        let json = r#"{"type":"word","wordIndex":42}"#;
        let target: AnnotationTarget = serde_json::from_str(json).unwrap();
        assert!(matches!(target, AnnotationTarget::Word { word_index: 42 }));

        let json = r#"{"type":"char","wordIndex":5,"charStart":2,"charEnd":4}"#;
        let target: AnnotationTarget = serde_json::from_str(json).unwrap();
        assert!(matches!(target, AnnotationTarget::Character { word_index: 5, char_start: 2, char_end: 4 }));

        let json = r#"{"type":"span","startWord":10,"endWord":15}"#;
        let target: AnnotationTarget = serde_json::from_str(json).unwrap();
        assert!(matches!(target, AnnotationTarget::Span { start_word: 10, end_word: 15 }));
    }

    #[test]
    fn test_annotation_value_camelcase() {
        // Verify AnnotationValue fields also serialize to camelCase
        let paleo = AnnotationValue::Paleographic {
            observation_type: PaleographicType::Unclear,
            description: Some("hard to read".to_string()),
            certainty: Some(0.5),
        };
        let json = serde_json::to_string(&paleo).unwrap();
        assert!(json.contains("observationType"), "Expected 'observationType' in: {}", json);
        assert!(!json.contains("observation_type"), "Should not contain snake_case in: {}", json);
    }
}
