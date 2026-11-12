//! # The document as a tree
//!
//! Step 1 of `docs/DOCUMENT_MODEL.md`: the XML file held in memory as a tree
//! that is the file, not a model derived from it.
//!
//! - libxml2 parses ([`crate::xml::parse`]); [`Tree::parse`] copies its result
//!   node for node into a Rust arena, which can be sent between threads.
//!   Every kind of node is kept, unknown elements and comments included.
//! - Each node knows its byte span in the original text, found by walking the
//!   text alongside the tree (`SourceCursor`). The walk only locates what
//!   libxml2 parsed; if the two disagree, no node gets a span, and the file is
//!   written from the tree (formatting may change, content doesn't).
//! - [`Tree::write`] copies unchanged nodes from the original text, so only
//!   what was edited is written fresh. [`Tree::write_checked`] parses the
//!   output with libxml2 again and compares it with the tree: use it for
//!   anything that is saved.

mod merge;
mod project;
mod tree;
mod write;

pub use merge::merge_word_edit;
pub use project::{find_body, project_body};
pub use tree::Tree;

#[cfg(test)]
mod tests;
