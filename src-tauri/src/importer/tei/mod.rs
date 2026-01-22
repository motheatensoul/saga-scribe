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
//! TEI-XML → [Extractor] → Segments → [segments_to_dsl] → DSL
//!                            ↓
//!                     (preserved for export)
//!                            ↓
//! Edited DSL → [compute_patches] → Patches → [apply_patches] → TEI-XML
//! ```
//!
//! ## Key Components
//!
//! - **[`extraction`]**: Extracts segments from TEI-XML, converting content to DSL
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
//! The [`menota_abbr_expansion`](extraction::Extractor::menota_abbr_expansion) function
//! detects abbreviation patterns (am/ex markers) and converts them to `.abbr[]{}` syntax.

pub mod helpers;
pub mod segments;
pub mod extraction;
pub mod patching;
mod importer;

// Re-export everything from the main importer for backward compatibility
pub use importer::*;
