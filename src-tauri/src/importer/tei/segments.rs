use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a segment of the document - either structural XML or editable content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Segment {
    /// Structural XML preserved verbatim (div, p, s, head, supplied, comments, etc.)
    #[serde(rename = "structural")]
    Structural {
        id: usize,
        xml: String, // Exact XML fragment: "<div type=\"chapter\" n=\"1\">"
    },

    /// A word element with editable content
    #[serde(rename = "word")]
    Word {
        id: usize,
        original_xml: String,                // Full <w>...</w> for reference
        attributes: HashMap<String, String>, // lemma, me:msa, etc.
        dsl_content: String,                 // DSL representation of facs content
        has_inline_lb: bool,                 // Contains line break inside
    },

    /// Punctuation element
    #[serde(rename = "punctuation")]
    Punctuation {
        id: usize,
        original_xml: String,
        dsl_content: String,
    },

    /// Line break (can be standalone or mid-word, tracked in Word if inline)
    #[serde(rename = "line_break")]
    LineBreak {
        id: usize,
        attributes: HashMap<String, String>, // ed, n, rend
    },

    /// Page break
    #[serde(rename = "page_break")]
    PageBreak {
        id: usize,
        attributes: HashMap<String, String>,
    },

    /// Hand shift marker
    #[serde(rename = "hand_shift")]
    HandShift {
        id: usize,
        attributes: HashMap<String, String>,
    },

    /// Whitespace between elements (for formatting preservation)
    #[serde(rename = "whitespace")]
    Whitespace { id: usize, content: String },
}

impl Segment {
    pub fn id(&self) -> usize {
        match self {
            Segment::Structural { id, .. } => *id,
            Segment::Word { id, .. } => *id,
            Segment::Punctuation { id, .. } => *id,
            Segment::LineBreak { id, .. } => *id,
            Segment::PageBreak { id, .. } => *id,
            Segment::HandShift { id, .. } => *id,
            Segment::Whitespace { id, .. } => *id,
        }
    }
}

/// The complete imported document manifest
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedDocument {
    pub segments: Vec<Segment>,
    pub is_menota: bool, // Has me:facs/dipl/norm structure
}
