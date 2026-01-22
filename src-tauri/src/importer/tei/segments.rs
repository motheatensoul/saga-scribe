//! # Document Segments
//!
//! This module defines the segment-based document representation used for round-trip
//! TEI-XML editing. Segments preserve the structure of the original document while
//! allowing targeted content edits.
//!
//! ## Design Philosophy
//!
//! The segment approach divides a TEI document into discrete units:
//! - **Editable segments**: Words, punctuation, line/page breaks (can be modified)
//! - **Structural segments**: Div, p, s elements (preserved verbatim)
//! - **Whitespace segments**: Formatting between elements (preserved)
//!
//! Each segment has a unique ID, allowing the patching system to track which
//! segments were modified, deleted, or have new content inserted nearby.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A segment of the TEI document, either structural or editable.
///
/// Segments form the intermediate representation between raw TEI-XML and
/// editable DSL. They preserve original XML while enabling targeted edits.
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

/// Complete manifest for an imported TEI document.
///
/// This structure preserves all information needed for round-trip fidelity:
/// the segment list tracks structural and content elements, while `is_menota`
/// indicates whether the document uses MENOTA multi-level transcription.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedDocument {
    /// Ordered list of document segments (structural, words, breaks, etc.)
    pub segments: Vec<Segment>,
    /// Whether the document has MENOTA three-level structure (me:facs/dipl/norm)
    pub is_menota: bool,
}
