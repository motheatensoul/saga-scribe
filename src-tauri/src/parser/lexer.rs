//! # DSL Lexer
//!
//! This module implements the lexical analyzer (lexer/tokenizer) for the Saga-Scribe DSL.
//!
//! ## Design
//!
//! The lexer is a single-pass, greedy parser that processes input left-to-right.
//! It uses prefix matching to identify DSL constructs, with longer prefixes checked
//! first to handle overlapping syntax (e.g., `///` page break before `//` line break).
//!
//! ## Parsing Strategy
//!
//! The lexer maintains a position cursor and a text buffer:
//! - When a DSL construct is recognized, the text buffer is flushed as a `Text` node
//! - The construct is then parsed and added to the document
//! - Unknown characters are accumulated in the text buffer
//!
//! ## Entity Reference Handling
//!
//! Entity references (`:name:`) use a try-parse approach: if the pattern doesn't
//! complete with a closing colon, the position resets and the colon is treated as text.

use super::ast::{Document, Node};

/// Tokenizes and parses DSL input into an abstract syntax tree.
///
/// The lexer performs single-pass parsing with greedy prefix matching.
/// DSL constructs are recognized by their opening delimiters and parsed
/// according to their specific grammar rules.
///
/// # Example
///
/// ```rust,ignore
/// let mut lexer = Lexer::new("//1 Hello .abbr[w]{world}");
/// let doc = lexer.parse()?;
/// // doc.nodes contains: [LineBreak(Some("1")), Text("Hello "), Abbreviation{...}]
/// ```
pub struct Lexer<'a> {
    /// The input DSL string being parsed
    input: &'a str,
    /// Current byte position in the input (UTF-8 aware)
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given input string.
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Parses the input and returns a [`Document`] containing the AST.
    ///
    /// The parser recognizes DSL constructs in order of prefix length (longest first)
    /// to correctly handle overlapping syntax like `///` vs `//`.
    ///
    /// # Errors
    ///
    /// Returns an error string if:
    /// - A bracketed construct is unclosed (e.g., `.abbr[text` without `]`)
    /// - An expected delimiter is missing (e.g., `-{text}` without trailing `-`)
    pub fn parse(&mut self) -> Result<Document, String> {
        let mut doc = Document::new();
        let mut text_buf = String::new();

        // Main parsing loop: check DSL constructs in order of prefix length (longest first)
        // to correctly handle overlapping syntax patterns.
        while self.pos < self.input.len() {
            let remaining = &self.input[self.pos..];

            // Page break (///) must be checked before line break (//)
            if remaining.starts_with("///") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 3;
                let page_num = self.consume_until_whitespace();
                doc.push(Node::PageBreak(page_num));
                continue;
            }

            // Line break: // or //n where n is optional line number
            if remaining.starts_with("//") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 2;
                // Line numbers should only be alphanumeric (e.g. "1", "5a", "10v")
                // Stop consuming if we hit punctuation or other characters
                // CHANGE: Only consume digits to avoid eating into text like "//2Text"
                let line_num = self.consume_while(|c| c.is_ascii_digit());
                let line_num = if line_num.is_empty() {
                    None
                } else {
                    Some(line_num)
                };
                doc.push(Node::LineBreak(line_num));
                continue;
            }

            // Supplied block: .supplied{text}
            if remaining.starts_with(".supplied{") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 10;
                let text = self.consume_braced_block()?;
                doc.push(Node::SuppliedBlock(text));
                continue;
            }

            // Normalized-only wrapper: .norm{text}
            if remaining.starts_with(".norm{") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 6;
                let text = self.consume_braced_block()?;
                doc.push(Node::Norm(text));
                continue;
            }

            // Heading: .head{text}
            if remaining.starts_with(".head{") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 6;
                let text = self.consume_braced_block()?;
                doc.push(Node::Head(text));
                continue;
            }

            // Abbreviation: .abbr[text]{expansion}
            if remaining.starts_with(".abbr[") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 6;
                let abbr = self.consume_bracketed(']')?;
                self.expect('{')?;
                let expansion = self.consume_bracketed('}')?;
                doc.push(Node::Abbreviation { abbr, expansion });
                continue;
            }

            // Gap: [...] or [...n] or [...<text>] or [...n<text>]
            if remaining.starts_with("[...") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 4;
                let quantity = self.parse_optional_number();
                // Check for optional supplied text: <text>
                let supplied = if self.current_char() == Some('<') {
                    self.advance(); // skip '<'
                    Some(self.consume_until('>')?)
                } else {
                    None
                };
                self.expect(']')?;
                doc.push(Node::Gap { quantity, supplied });
                continue;
            }

            // Supplied: <text>
            if remaining.starts_with('<') && !remaining.starts_with("<<") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 1;
                let text = self.consume_until('>')?;
                doc.push(Node::Supplied(text));
                continue;
            }

            // Deletion: -{text}-
            if remaining.starts_with("-{") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 2;
                let text = self.consume_bracketed('}')?;
                self.expect('-')?;
                doc.push(Node::Deletion(text));
                continue;
            }

            // Addition: +{text}+
            if remaining.starts_with("+{") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 2;
                let text = self.consume_bracketed('}')?;
                self.expect('+')?;
                doc.push(Node::Addition(text));
                continue;
            }

            // Note: ^{text}
            if remaining.starts_with("^{") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 2;
                let text = self.consume_bracketed('}')?;
                doc.push(Node::Note(text));
                continue;
            }

            // Unclear: ?{text}?
            if remaining.starts_with("?{") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 2;
                let text = self.consume_bracketed('}')?;
                self.expect('?')?;
                doc.push(Node::Unclear(text));
                continue;
            }

            // Entity: :name:
            if remaining.starts_with(':') {
                if let Some(entity_name) = self.try_parse_entity() {
                    self.flush_text(&mut doc, &mut text_buf);
                    doc.push(Node::Entity(entity_name));
                    continue;
                }
            }

            // Word continuation: ~ followed by line/page break
            if remaining.starts_with("~///") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 4;
                let page_num = self.consume_until_whitespace();
                doc.push(Node::WordContinuation);
                doc.push(Node::PageBreak(page_num));
                continue;
            }

            if remaining.starts_with("~//") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 3;
                let line_num = self.consume_until_whitespace();
                let line_num = if line_num.is_empty() {
                    None
                } else {
                    Some(line_num)
                };
                doc.push(Node::WordContinuation);
                doc.push(Node::LineBreak(line_num));
                continue;
            }

            // Compound word join: ~ not followed by line/page break
            // e.g., "upp~haf" → "upp haf" in facs/dipl, "upphaf" in norm
            if remaining.starts_with('~') && !remaining.starts_with("~//") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 1;
                doc.push(Node::CompoundJoin);
                continue;
            }

            // Explicit word boundary: |
            if remaining.starts_with('|') {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 1;
                doc.push(Node::WordBoundary);
                continue;
            }

            // Regular character
            text_buf.push(self.current_char().unwrap());
            self.advance();
        }

        self.flush_text(&mut doc, &mut text_buf);
        Ok(doc)
    }

    /// Flushes accumulated text buffer as a Text node if non-empty.
    fn flush_text(&self, doc: &mut Document, buf: &mut String) {
        if !buf.is_empty() {
            doc.push(Node::Text(std::mem::take(buf)));
        }
    }

    /// Returns the current character at the cursor position.
    fn current_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    /// Advance position by one character, handling multi-byte UTF-8 correctly
    fn advance(&mut self) {
        if let Some(c) = self.current_char() {
            self.pos += c.len_utf8();
        }
    }

    /// Consumes characters while the predicate returns true.
    fn consume_while<F>(&mut self, predicate: F) -> String
    where
        F: Fn(char) -> bool,
    {
        let start = self.pos;
        while self.pos < self.input.len() {
            if let Some(c) = self.current_char() {
                if !predicate(c) {
                    break;
                }
                self.advance();
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }

    /// Consumes characters until whitespace is encountered.
    /// Used for parsing identifiers like page/line numbers.
    fn consume_until_whitespace(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.input.len() {
            if self.current_char().map_or(true, |c| c.is_whitespace()) {
                break;
            }
            self.advance();
        }
        self.input[start..self.pos].to_string()
    }

    /// Consumes content within brackets, handling nested brackets.
    ///
    /// This is used for constructs like `.abbr[text]` where the text may contain
    /// other bracketed content. Tracks nesting depth to find the matching closer.
    fn consume_bracketed(&mut self, end: char) -> Result<String, String> {
        let start = self.pos;
        let mut depth = 1;
        while self.pos < self.input.len() {
            let c = self.current_char().unwrap();
            if c == end {
                depth -= 1;
                if depth == 0 {
                    let result = self.input[start..self.pos].to_string();
                    self.advance();
                    return Ok(result);
                }
            } else if c == '{' || c == '[' || c == '<' {
                depth += 1;
            } else if c == '}' || c == ']' || c == '>' {
                // Decrement for closing brackets that aren't the end character
                // (end character is already handled above)
                if c != end && depth > 1 {
                    depth -= 1;
                }
            }
            self.advance();
        }
        Err(format!("Unclosed bracket, expected '{}'", end))
    }

    /// Consumes content within braces `{}`, tracking nesting depth.
    /// Used for constructs like `.head{content}` and `.norm{content}`.
    fn consume_braced_block(&mut self) -> Result<String, String> {
        let start = self.pos;
        let mut depth = 1;
        while self.pos < self.input.len() {
            let c = self.current_char().unwrap();
            if c == '}' {
                depth -= 1;
                if depth == 0 {
                    let result = self.input[start..self.pos].to_string();
                    self.advance();
                    return Ok(result);
                }
            } else if c == '{' {
                depth += 1;
            }
            self.advance();
        }
        Err("Unclosed bracket, expected '}'".to_string())
    }

    /// Consumes characters until the specified end character is found.
    /// Does not handle nesting - used for simple delimiters like `<text>`.
    fn consume_until(&mut self, end: char) -> Result<String, String> {
        let start = self.pos;
        while self.pos < self.input.len() {
            if self.current_char() == Some(end) {
                let result = self.input[start..self.pos].to_string();
                self.advance();
                return Ok(result);
            }
            self.advance();
        }
        Err(format!("Expected '{}'", end))
    }

    /// Expects and consumes a specific character, returning an error if not found.
    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.current_char() == Some(c) {
            self.advance();
            Ok(())
        } else {
            Err(format!(
                "Expected '{}', found {:?}",
                c,
                self.current_char()
            ))
        }
    }

    /// Parses an optional numeric value (for gap quantities like `[...3]`).
    fn parse_optional_number(&mut self) -> Option<u32> {
        let start = self.pos;
        while self.pos < self.input.len() {
            if !self.current_char().is_some_and(|c| c.is_ascii_digit()) {
                break;
            }
            self.advance();
        }
        if start == self.pos {
            None
        } else {
            self.input[start..self.pos].parse().ok()
        }
    }

    /// Try to parse an entity reference :name:
    /// Returns Some(name) if successful, None if not a valid entity pattern
    fn try_parse_entity(&mut self) -> Option<String> {
        let start = self.pos;

        // Skip opening colon
        if self.current_char() != Some(':') {
            return None;
        }
        self.advance();

        // Collect entity name (alphanumeric + underscore)
        let name_start = self.pos;
        while self.pos < self.input.len() {
            match self.current_char() {
                Some(c) if c.is_alphanumeric() || c == '_' => {
                    self.advance();
                }
                _ => break,
            }
        }

        // Must have at least one character in name
        if self.pos == name_start {
            self.pos = start;
            return None;
        }

        // Must end with closing colon
        if self.current_char() != Some(':') {
            self.pos = start;
            return None;
        }

        let name = self.input[name_start..self.pos].to_string();
        self.advance(); // consume closing colon

        Some(name)
    }
}
