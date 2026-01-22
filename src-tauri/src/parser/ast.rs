//! # Abstract Syntax Tree
//!
//! This module defines the AST nodes produced by the lexer and consumed by the compiler.
//!
//! The AST is a flat sequence of nodes (not a tree in the traditional sense),
//! where container nodes like `Word` and `Punctuation` are introduced by the
//! word tokenizer stage.

//TODO We have to check whether a flat sequence still makes sense, since we do have hierarchical structures within the DSL now, even if largely simple.

use serde::{Deserialize, Serialize};

/// A node in the DSL abstract syntax tree.
///
/// The AST starts as a flat sequence from the lexer, then the word tokenizer
/// groups content into `Word` and `Punctuation` container nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        quantity: Option<u32>, //can theoretically sized down or set to usize? if a gap would reach anywhere close to the limit of u16 for any given unit, the unit should be scaled up instead. 
        supplied: Option<String>,
        //TODO per MENOTA spec unit field is mandatory, how to handle? 
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

    pub fn push(&mut self, node: Node) {
        self.nodes.push(node);
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
