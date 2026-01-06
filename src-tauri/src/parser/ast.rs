use serde::{Deserialize, Serialize};

/// Represents a node in the TEI-DSL abstract syntax tree
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
        quantity: Option<u32>,
        supplied: Option<String>,
    },
    /// Supplied text (standalone): <text>
    Supplied(String),
    /// Deletion: -{text}-
    Deletion(String),
    /// Addition: +{text}+
    Addition(String),
    /// Note: ^{text}
    Note(String),
    /// Unclear reading: ?{text}?
    Unclear(String),
    /// Custom entity: :name:
    Entity(String),
    /// Word continuation marker: ~ (used before line/page breaks)
    WordContinuation,
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
