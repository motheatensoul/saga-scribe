//! # TEI-XML Importer Module
//!
//! This module provides import and round-trip editing capabilities for TEI-XML documents,
//! with special support for MENOTA multi-level transcriptions.
//!
//! ## Architecture Overview
//!
//! The import pipeline transforms TEI-XML into editable DSL while preserving structural
//! fidelity for round-trip export:
//!
//! ```text
//! TEI-XML → document tree → [project_body] → Segments → [segments_to_dsl] → DSL
//!                            ↓
//!                     (preserved for export)
//!                            ↓
//! Edited DSL → [compute_patches] → Patches → [render_body] → TEI-XML
//!                                        (spliced into the original bytes)
//! ```
//!
//! ## Key Components
//!
//! - **[`crate::document`]**: The document as a tree, and its projection to
//!   segments ([`project_body`](crate::document::project_body))
//! - **[`extraction`]**: Writes segments as DSL ([`segments_to_dsl`](extraction::segments_to_dsl));
//!   its extractor is kept for tests, as the reference the projection matches
//! - **[`segments`]**: Defines the [`Segment`](segments::Segment) manifest structure
//! - **[`patching`]**: Computes and applies diffs for round-trip export
//! - **[`helpers`]**: XML serialization and attribute handling utilities
//! - **[`importer`]**: Main entry point ([`parse`](importer::parse)) and metadata extraction
//!
//! ## Round-Trip Fidelity
//!
//! The system preserves:
//! - Original XML structure (div, p, s elements)
//! - Word attributes (lemma, me:msa, etc.)
//! - Line/page break attributes (n, ed, rend)
//! - MENOTA multi-level structure (me:facs, me:dipl, me:norm)
//! - XML preamble and postamble (DOCTYPE, comments, etc.)
//!
//! ## MENOTA Support
//!
//! For MENOTA documents with three-level transcription, the importer extracts the
//! facsimile level for editing and preserves the full multi-level structure for export.
//! The projection detects abbreviation patterns (`<am>`/`<ex>` markers) and converts
//! them to `.abbr[]{}` syntax.

pub mod helpers;
pub mod segments;
pub mod extraction;
pub mod patching;
pub mod source_map;
mod importer;

// Re-export everything from the main importer for backward compatibility
pub use importer::*;
