//! # Compiler Nodes
//!
//! The flat node sequence consumed by the word tokenizer and the compiler.
//!
//! The DSL itself is parsed into a hierarchical, span-annotated tree by
//! [`super::syntax`]; this flat form is that tree lowered for the existing
//! pipeline. Container nodes like `Word` and `Punctuation` are introduced by
//! the word tokenizer stage. Block constructs still carry their content as DSL
//! text here; the tree has it parsed.

use serde::{Deserialize, Serialize};

/// A node in the DSL abstract syntax tree.
///
/// The AST starts as a flat sequence from the lexer, then the word tokenizer
/// groups content into `Word` and `Punctuation` container nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Node {
    /// Plain text content
    Text(String),
    /// Line break: // or //n where n is optional line number
    LineBreak(Option<String>),
    /// Page break: ///n where n is the page number
    PageBreak(String),
    /// Abbreviation with expansion: .abbr[abbr]{expansion}
    Abbreviation { abbr: String, expansion: String },
    /// Gap/lacuna: [...] or [...n] or [...<text>] or [...n<text>]
    Gap {
        /// Extent in characters; compiled as `quantity` with `unit="chars"`.
        /// (`unit` is optional in MENOTA's schema and only meaningful with a quantity.)
        quantity: Option<u32>,
        supplied: Option<String>,
    },
    /// Supplied text (standalone): <text>
    Supplied(String),
    /// Supplied block wrapper: .supplied{text}
    SuppliedBlock(String),
    /// Deletion: -{text}-
    Deletion(String),
    /// Addition: +{text}+
    Addition(String),
    /// Note: ^{text}
    Note(String),
    /// Heading: .head{text}
    Head(String),
    /// Normalized-only wrapper: .norm{text}
    Norm(String),
    /// Unclear reading: ?{text}?
    Unclear(String),
    /// Custom entity: :name:
    Entity(String),
    /// Word continuation marker: ~ (used before line/page breaks)
    WordContinuation,
    /// Compound word join: ~ between words (upp~haf → upphaf in norm)
    /// Outputs space in facs/dipl but joins in norm
    CompoundJoin,
    /// Explicit word boundary marker: |
    WordBoundary,
    /// Word container (groups nodes into a single word)
    Word(Vec<Node>),
    /// Punctuation container (groups punctuation nodes)
    Punctuation(Vec<Node>),
}

/// A document is a sequence of nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub nodes: Vec<Node>,
}

impl Document {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
