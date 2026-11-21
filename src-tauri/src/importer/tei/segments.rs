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
//! - **Paragraph segments**: `<p>` start tags, editable as `.p` in the DSL
//! - **Structural segments**: Other elements' tags (div, s, …), preserved verbatim
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

    /// The start tag of a paragraph (`<p>`), shown as `.p` in the DSL so
    /// paragraphs can be split and merged. Its end tag is a `Structural`
    /// segment. Projects imported before 0.6.0 store `<p>` as `Structural`.
    #[serde(rename = "paragraph")]
    Paragraph {
        id: usize,
        xml: String, // Exact start tag: "<p rend=\"indent\">"
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
            Segment::Paragraph { id, .. } => *id,
            Segment::Word { id, .. } => *id,
            Segment::Punctuation { id, .. } => *id,
            Segment::LineBreak { id, .. } => *id,
            Segment::PageBreak { id, .. } => *id,
            Segment::HandShift { id, .. } => *id,
            Segment::Whitespace { id, .. } => *id,
        }
    }
}

/// Byte ranges of segments within the original `<body>` source, indexed by segment ID.
///
/// Editable segments (words, punctuation, breaks) and the tags of structural
/// elements carry a span. The whole
/// map is absent when the source could not be aligned with the parsed DOM.
pub type SourceSpans = Vec<Option<(usize, usize)>>;

/// Complete manifest for an imported TEI document.
///
/// This structure preserves all information needed for round-trip fidelity:
/// the segment list tracks structural and content elements, while `is_menota`
/// indicates whether the document uses MENOTA multi-level transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedDocument {
    /// Ordered list of document segments (structural, words, breaks, etc.)
    pub segments: Vec<Segment>,
    /// Whether the document has MENOTA three-level structure (me:facs/dipl/norm)
    pub is_menota: bool,
    /// Source spans for splicing edits into the original bytes (absent in
    /// projects saved before spans existed; recomputed on demand).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<SourceSpans>,
    /// How the file encodes its transcription (step 4 of
    /// `docs/DOCUMENT_MODEL.md`); inferred from the file when it is read,
    /// absent in projects saved before 0.8.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<crate::document::profile::Profile>,
    /// The DSL rules the document's DSL was generated with ([`DSL_RULES`]);
    /// 0 for documents imported before rules were versioned.
    #[serde(default)]
    pub dsl_rules: u32,
    /// The editor's answers for words replaced in the shorthand, by the
    /// segment ID of the original word (see `patching::WordRecord`). Kept by
    /// the frontend, and saved with the manifest (`.teis` 1.8).
    #[serde(default, skip_serializing_if = "HashMap::is_empty", deserialize_with = "known_decisions")]
    pub decisions: HashMap<usize, Decision>,
}

/// The editor's answer for a word of the original that was replaced in the
/// shorthand (`docs/DOCUMENT_MODEL.md`, "Replacing and deleting words").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// The same word, corrected: it keeps its lemma, analysis and normalized form
    Same,
    /// Another word in its place: those are removed (its markup still
    /// follows the text)
    Different,
}

/// The decisions of a manifest, without answers this version doesn't know
/// (early 0.8.0 builds also saved "removed" for deleted words).
fn known_decisions<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<HashMap<usize, Decision>, D::Error> {
    let all = HashMap::<usize, serde_json::Value>::deserialize(deserializer)?;
    Ok(all
        .into_iter()
        .filter_map(|(key, value)| Some((key, serde_json::from_value(value).ok()?)))
        .collect())
}

/// Version of the rules for writing imported XML as DSL. Documents keep the
/// rules they were imported with, so their stored DSL still matches.
///
/// - 0: before 0.6.0
/// - 1: entity punctuation (`:middot:`) is set off by spaces
/// - 2: a word or punctuation right after a page break starts a new line
///   (before, `///109r^{note}` read the note as part of the page number)
/// - 3: every word, numeral and punctuation reads as one token of the DSL:
///   characters that would split or end it are escaped (`ꝼæ\;ðꝛ`), a
///   `<num>` is `#…#`, and neighbours with nothing between them in the XML
///   are set apart (`skrims ^{sic}`, `þꝩi|at`). The escaping is in the
///   segments' `dsl_content`; projects from before it are escaped when
///   opened (`.teis` 1.7).
/// - 4: a break number the plain form can't hold is in braces (`//{56r}`,
///   `///{fol. 1r}`), and supplied text with markup inside a word is `<…>`
///   with shorthand content (`.supplied{…}` would end the word).
/// - 5: `-{…}-`, `+{…}+` and `?{…}?` have shorthand content too (entities,
///   gaps and breaks in them used to compile as text): characters that would
///   read as shorthand in their text are escaped, as in `<…>`.
pub const DSL_RULES: u32 = 5;

impl ImportedDocument {
    /// Drops the copies of the file's XML that words and punctuation hold
    /// (their XML, a word's attributes) where they are what the file's tree
    /// has: `xml` and `attributes` read it there. A session reads them in the
    /// tree (`patching::Originals`), and a manuscript's copies are tens of
    /// megabytes. Returns the IDs of the words whose attributes were dropped
    /// (some elements stored as words, such as notes, are saved without
    /// theirs), for [`restore_copies`](Self::restore_copies).
    pub fn drop_copies<'t>(
        &mut self,
        xml: impl Fn(usize) -> Option<&'t str>,
        attributes: impl Fn(usize) -> Option<HashMap<String, String>>,
    ) -> std::collections::HashSet<usize> {
        let mut dropped_attributes = std::collections::HashSet::new();
        for segment in &mut self.segments {
            match segment {
                Segment::Word { id, original_xml, attributes: attrs, .. } => {
                    if xml(*id) == Some(original_xml.as_str()) {
                        *original_xml = String::new();
                    }
                    if !attrs.is_empty() && attributes(*id).as_ref() == Some(attrs) {
                        *attrs = HashMap::new();
                        dropped_attributes.insert(*id);
                    }
                }
                Segment::Punctuation { id, original_xml, .. } if xml(*id) == Some(original_xml.as_str()) => {
                    *original_xml = String::new();
                }
                _ => {}
            }
        }
        dropped_attributes
    }

    /// Puts back the copies [`drop_copies`](Self::drop_copies) dropped: the
    /// XML (dropped where it is empty) and the attributes of `dropped_attributes`.
    pub fn restore_copies<'t>(
        &mut self,
        xml: impl Fn(usize) -> Option<&'t str>,
        attributes: impl Fn(usize) -> Option<HashMap<String, String>>,
        dropped_attributes: &std::collections::HashSet<usize>,
    ) {
        for segment in &mut self.segments {
            match segment {
                Segment::Word { id, original_xml, attributes: attrs, .. } => {
                    if original_xml.is_empty() {
                        *original_xml = xml(*id).unwrap_or_default().to_string();
                    }
                    if dropped_attributes.contains(id) {
                        *attrs = attributes(*id).unwrap_or_default();
                    }
                }
                Segment::Punctuation { id, original_xml, .. } if original_xml.is_empty() => {
                    *original_xml = xml(*id).unwrap_or_default().to_string();
                }
                _ => {}
            }
        }
    }

    /// Parses a manifest as stored in segments.json.
    ///
    /// Since project format 1.5 this is the full manifest. Versions up to 1.4
    /// wrote a bare segment array, which lost `is_menota`; MENOTA structure is
    /// then re-detected from the segments themselves.
    pub fn from_json(json: &str) -> Result<Self, String> {
        if let Ok(doc) = serde_json::from_str::<ImportedDocument>(json) {
            return Ok(doc);
        }
        let segments: Vec<Segment> = serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse segments.json: {}", e))?;
        let is_menota = segments.iter().any(|seg| match seg {
            Segment::Word { original_xml, .. } | Segment::Punctuation { original_xml, .. } => {
                original_xml.contains(":facs")
            }
            _ => false,
        });
        Ok(ImportedDocument {
            segments,
            is_menota,
            spans: None,
            profile: None,
            dsl_rules: 0,
            decisions: HashMap::new(),
        })
    }
}
