//! # DSL Lexer
//!
//! Entry point for turning DSL source into the flat [`Node`] sequence the word
//! tokenizer and compiler consume.
//!
//! Parsing itself is done by [`super::syntax`], which builds a hierarchical,
//! span-annotated syntax tree and recovers from errors; this module lowers that
//! tree to nodes and turns the first syntax error into an `Err`.

use super::ast::Document;
use super::syntax;

/// Parses DSL input into a [`Document`].
///
/// # Example
///
/// ```rust,ignore
/// let mut lexer = Lexer::new("//1 Hello .abbr[w]{world}");
/// let doc = lexer.parse()?;
/// // doc.nodes contains: [LineBreak(Some("1")), Text(" Hello "), Abbreviation{...}]
/// ```
pub struct Lexer<'a> {
    /// The input DSL string being parsed
    input: &'a str,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given input string.
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    /// Parses the input and returns a [`Document`] containing the nodes.
    ///
    /// # Errors
    ///
    /// Returns the first syntax error, prefixed with its line and column, if:
    /// - A bracketed construct is unclosed (e.g., `.abbr[text` without `]`)
    /// - An expected delimiter is missing (e.g., `-{text}` without trailing `-`)
    /// - Either occurs inside a block construct's content (`.head{...}`,
    ///   `.supplied{...}`, `.norm{...}`)
    pub fn parse(&mut self) -> Result<Document, String> {
        let tree = syntax::parse(self.input);
        if let Some(error) = tree.diagnostics.first() {
            return Err(error.describe(self.input));
        }
        Ok(Document {
            nodes: syntax::lower(&tree.nodes, self.input),
        })
    }
}
