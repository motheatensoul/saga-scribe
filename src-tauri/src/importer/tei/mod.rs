//! TEI-XML importer modules for segment-based preservation.

pub mod helpers;
pub mod segments;
pub mod extraction;
pub mod patching;
mod importer;

// Re-export everything from the main importer for backward compatibility
pub use importer::*;
