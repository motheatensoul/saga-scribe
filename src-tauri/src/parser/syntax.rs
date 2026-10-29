//! # DSL Syntax Tree
//!
//! A lossless, hierarchical syntax tree of DSL source, with error recovery.
//!
//! Every node records the byte span it was parsed from, so any node can be
//! traced back to the exact source text (for diagnostics, and for mapping edits
//! to the nodes they touch). Block constructs (`.head{}`, `.supplied{}`,
//! `.norm{}`) contain their parsed children instead of an unparsed string.
//!
//! The parser does not stop at the first syntax error: it records a
//! [`Diagnostic`] with its position, treats the offending opener as plain text
//! and carries on, so one unclosed bracket doesn't hide every later error.
//!
//! The rest of the pipeline (word tokenizer, compiler, patcher) still consumes
//! the flat [`Node`] sequence; [`lower`] produces it from the tree, exactly as
//! the original lexer did. [`Lexer::parse`](super::Lexer::parse) is that
//! combination.
//!
//! ## Grammar
//!
//! Constructs are recognized by prefix, longest first, as the original lexer
//! did; see [`Parser::parse_sequence`] for the order. Content of inline
//! constructs (`-{}-`, `+{}+`, `?{}?`, `^{}`, `<>`, `.abbr[]{}`) is plain text;
//! content of block constructs is DSL.

use super::ast::Node;
use std::ops::Range;

/// A byte range in the DSL source.
pub type Span = Range<usize>;

/// A node of the syntax tree. Text values are not copied: spans refer into the source.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxNode {
    pub kind: SyntaxKind,
    /// The full source range of the construct, delimiters included.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyntaxKind {
    /// Plain text; its value is the source covered by the node's span.
    Text,
    /// `//` or `//n`
    LineBreak { number: Option<Span> },
    /// `///n`
    PageBreak { number: Span },
    /// `.abbr[abbr]{expansion}`
    Abbreviation { abbr: Span, expansion: Span },
    /// `[...]`, `[...n]`, `[...<text>]`, `[...n<text>]`
    Gap { quantity: Option<u32>, supplied: Option<Span> },
    /// `<text>`
    Supplied { content: Span },
    /// `.supplied{dsl}`
    SuppliedBlock { content: Span, children: Vec<SyntaxNode> },
    /// `-{text}-`
    Deletion { content: Span },
    /// `+{text}+`
    Addition { content: Span },
    /// `^{text}`
    Note { content: Span },
    /// `.head{dsl}`
    Head { content: Span, children: Vec<SyntaxNode> },
    /// `.norm{dsl}`
    Norm { content: Span, children: Vec<SyntaxNode> },
    /// `?{text}?`
    Unclear { content: Span },
    /// `:name:`
    Entity { name: Span },
    /// `~` before a line or page break
    WordContinuation,
    /// `~` between word parts
    CompoundJoin,
    /// `|`
    WordBoundary,
}

/// A syntax error found while parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    /// From the start of the construct to where parsing it failed.
    pub span: Span,
    pub message: String,
    /// Inside the content of a block construct. The original lexer only
    /// checked block content when compiling the block (and then emitted an XML
    /// comment instead of failing), so these were previously not errors.
    pub nested: bool,
}

impl Diagnostic {
    /// The message with a 1-based line and column, e.g. `Line 3, column 7: ...`.
    pub fn describe(&self, source: &str) -> String {
        let (line, column) = line_col(source, self.span.start);
        format!("Line {}, column {}: {}", line, column, self.message)
    }
}

/// The result of parsing: the tree, plus every syntax error found.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxTree {
    pub nodes: Vec<SyntaxNode>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses DSL source into a syntax tree. Never fails; errors are reported in
/// [`SyntaxTree::diagnostics`].
pub fn parse(source: &str) -> SyntaxTree {
    let mut parser = Parser::new(source, 0..source.len(), false);
    let nodes = parser.parse_sequence();
    SyntaxTree {
        nodes,
        diagnostics: parser.diagnostics,
    }
}

/// 1-based line and column (in characters) of a byte offset.
pub fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset.min(source.len())];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, before[line_start..].chars().count() + 1)
}

/// Converts the tree to the flat node sequence the word tokenizer and
/// compiler consume. Block content is passed on as source text, as before.
pub fn lower(nodes: &[SyntaxNode], source: &str) -> Vec<Node> {
    let text = |span: &Span| source[span.clone()].to_string();
    nodes
        .iter()
        .map(|node| match &node.kind {
            SyntaxKind::Text => Node::Text(text(&node.span)),
            SyntaxKind::LineBreak { number } => Node::LineBreak(number.as_ref().map(text)),
            SyntaxKind::PageBreak { number } => Node::PageBreak(text(number)),
            SyntaxKind::Abbreviation { abbr, expansion } => Node::Abbreviation {
                abbr: text(abbr),
                expansion: text(expansion),
            },
            SyntaxKind::Gap { quantity, supplied } => Node::Gap {
                quantity: *quantity,
                supplied: supplied.as_ref().map(text),
            },
            SyntaxKind::Supplied { content } => Node::Supplied(text(content)),
            SyntaxKind::SuppliedBlock { content, .. } => Node::SuppliedBlock(text(content)),
            SyntaxKind::Deletion { content } => Node::Deletion(text(content)),
            SyntaxKind::Addition { content } => Node::Addition(text(content)),
            SyntaxKind::Note { content } => Node::Note(text(content)),
            SyntaxKind::Head { content, .. } => Node::Head(text(content)),
            SyntaxKind::Norm { content, .. } => Node::Norm(text(content)),
            SyntaxKind::Unclear { content } => Node::Unclear(text(content)),
            SyntaxKind::Entity { name } => Node::Entity(text(name)),
            SyntaxKind::WordContinuation => Node::WordContinuation,
            SyntaxKind::CompoundJoin => Node::CompoundJoin,
            SyntaxKind::WordBoundary => Node::WordBoundary,
        })
        .collect()
}

/// Why a construct could not be parsed.
type Failure = String;

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    /// Parsing stops here (the end of a block's content when parsing nested).
    end: usize,
    nested: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, range: Span, nested: bool) -> Self {
        Self {
            src,
            pos: range.start,
            end: range.end,
            nested,
            diagnostics: Vec::new(),
        }
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..self.end]
    }

    /// Parses constructs until the end of the range, checking prefixes in the
    /// same order as the original lexer (longest first where they overlap).
    fn parse_sequence(&mut self) -> Vec<SyntaxNode> {
        let mut nodes = Vec::new();
        let mut text_start: Option<usize> = None;

        // Ends the pending text run at `end` (the start of the next construct)
        macro_rules! flush_text {
            ($end:expr) => {
                if let Some(text) = text_start.take() {
                    nodes.push(SyntaxNode { kind: SyntaxKind::Text, span: text..$end });
                }
            };
        }

        while self.pos < self.end {
            let start = self.pos;
            let rest = self.rest();

            // A construct parser returns the node, or why it failed after
            // consuming `opener` bytes. On failure the opener becomes text.
            let attempt: Option<(usize, Result<Vec<SyntaxNode>, Failure>)> =
                if rest.starts_with("///") {
                    flush_text!(start);
                    self.pos += 3;
                    let number = self.consume_until_whitespace();
                    Some((3, Ok(vec![self.node(SyntaxKind::PageBreak { number }, start)])))
                } else if rest.starts_with("//") {
                    flush_text!(start);
                    self.pos += 2;
                    // Only digits, so "//2Text" doesn't eat into the text
                    let number = self.consume_while(|c| c.is_ascii_digit());
                    let number = (!number.is_empty()).then_some(number);
                    Some((2, Ok(vec![self.node(SyntaxKind::LineBreak { number }, start)])))
                } else if rest.starts_with(".supplied{") {
                    flush_text!(start);
                    Some((10, self.block(start, 10, |content, children| SyntaxKind::SuppliedBlock { content, children })))
                } else if rest.starts_with(".norm{") {
                    flush_text!(start);
                    Some((6, self.block(start, 6, |content, children| SyntaxKind::Norm { content, children })))
                } else if rest.starts_with(".head{") {
                    flush_text!(start);
                    Some((6, self.block(start, 6, |content, children| SyntaxKind::Head { content, children })))
                } else if rest.starts_with(".abbr[") {
                    flush_text!(start);
                    self.pos += 6;
                    let result = (|| {
                        let abbr = self.consume_bracketed(']')?;
                        self.expect('{')?;
                        let expansion = self.consume_bracketed('}')?;
                        Ok(vec![self.node(SyntaxKind::Abbreviation { abbr, expansion }, start)])
                    })();
                    Some((6, result))
                } else if rest.starts_with("[...") {
                    flush_text!(start);
                    self.pos += 4;
                    let result = (|| {
                        let quantity = self.parse_optional_number();
                        let supplied = if self.current_char() == Some('<') {
                            self.advance();
                            Some(self.consume_until('>')?)
                        } else {
                            None
                        };
                        self.expect(']')?;
                        Ok(vec![self.node(SyntaxKind::Gap { quantity, supplied }, start)])
                    })();
                    Some((4, result))
                } else if rest.starts_with('<') && !rest.starts_with("<<") {
                    flush_text!(start);
                    self.pos += 1;
                    let result = self
                        .consume_until('>')
                        .map(|content| vec![self.node(SyntaxKind::Supplied { content }, start)]);
                    Some((1, result))
                } else if rest.starts_with("-{") {
                    flush_text!(start);
                    Some((2, self.inline(start, Some('-'), |content| SyntaxKind::Deletion { content })))
                } else if rest.starts_with("+{") {
                    flush_text!(start);
                    Some((2, self.inline(start, Some('+'), |content| SyntaxKind::Addition { content })))
                } else if rest.starts_with("^{") {
                    flush_text!(start);
                    Some((2, self.inline(start, None, |content| SyntaxKind::Note { content })))
                } else if rest.starts_with("?{") {
                    flush_text!(start);
                    Some((2, self.inline(start, Some('?'), |content| SyntaxKind::Unclear { content })))
                } else if rest.starts_with(':') {
                    // Not an entity unless the closing colon follows; then it's text
                    match self.try_parse_entity() {
                        Some(name) => {
                            flush_text!(start);
                            Some((0, Ok(vec![self.node(SyntaxKind::Entity { name }, start)])))
                        }
                        None => None,
                    }
                } else if rest.starts_with("~///") {
                    flush_text!(start);
                    self.pos += 4;
                    let number = self.consume_until_whitespace();
                    Some((4, Ok(vec![
                        SyntaxNode { kind: SyntaxKind::WordContinuation, span: start..start + 1 },
                        self.node(SyntaxKind::PageBreak { number }, start + 1),
                    ])))
                } else if rest.starts_with("~//") {
                    flush_text!(start);
                    self.pos += 3;
                    // Unlike a plain `//`, the number runs to the next whitespace
                    let number = self.consume_until_whitespace();
                    let number = (!number.is_empty()).then_some(number);
                    Some((3, Ok(vec![
                        SyntaxNode { kind: SyntaxKind::WordContinuation, span: start..start + 1 },
                        self.node(SyntaxKind::LineBreak { number }, start + 1),
                    ])))
                } else if rest.starts_with('~') {
                    flush_text!(start);
                    self.pos += 1;
                    Some((1, Ok(vec![self.node(SyntaxKind::CompoundJoin, start)])))
                } else if rest.starts_with('|') {
                    flush_text!(start);
                    self.pos += 1;
                    Some((1, Ok(vec![self.node(SyntaxKind::WordBoundary, start)])))
                } else {
                    None
                };

            match attempt {
                Some((_, Ok(parsed))) => nodes.extend(parsed),
                Some((opener, Err(message))) => {
                    self.diagnostics.push(Diagnostic {
                        span: start..self.pos.max(start + opener),
                        message,
                        nested: self.nested,
                    });
                    // Recover: the opener is text, parsing resumes right after it
                    self.pos = start + opener;
                    text_start.get_or_insert(start);
                }
                None => {
                    text_start.get_or_insert(start);
                    self.advance();
                }
            }
        }

        flush_text!(self.pos);
        nodes
    }

    fn node(&self, kind: SyntaxKind, start: usize) -> SyntaxNode {
        SyntaxNode { kind, span: start..self.pos }
    }

    /// `opener{content}` where the content is DSL, parsed into children.
    fn block(
        &mut self,
        start: usize,
        opener: usize,
        make: impl FnOnce(Span, Vec<SyntaxNode>) -> SyntaxKind,
    ) -> Result<Vec<SyntaxNode>, Failure> {
        self.pos += opener;
        let content = self.consume_braced_block()?;
        let mut inner = Parser::new(self.src, content.clone(), true);
        let children = inner.parse_sequence();
        self.diagnostics.extend(inner.diagnostics);
        Ok(vec![self.node(make(content, children), start)])
    }

    /// `x{content}` with an optional closing marker after the brace.
    fn inline(
        &mut self,
        start: usize,
        closing: Option<char>,
        make: impl FnOnce(Span) -> SyntaxKind,
    ) -> Result<Vec<SyntaxNode>, Failure> {
        self.pos += 2;
        let content = self.consume_bracketed('}')?;
        if let Some(c) = closing {
            self.expect(c)?;
        }
        Ok(vec![self.node(make(content), start)])
    }

    fn current_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn advance(&mut self) {
        if let Some(c) = self.current_char() {
            self.pos += c.len_utf8();
        }
    }

    fn consume_while(&mut self, predicate: impl Fn(char) -> bool) -> Span {
        let start = self.pos;
        while self.current_char().is_some_and(&predicate) {
            self.advance();
        }
        start..self.pos
    }

    fn consume_until_whitespace(&mut self) -> Span {
        self.consume_while(|c| !c.is_whitespace())
    }

    /// Content up to the matching `end`. Other brackets nest, with the original
    /// lexer's rule: any of `{[<` opens, a different closer only closes nested ones.
    fn consume_bracketed(&mut self, end: char) -> Result<Span, Failure> {
        let start = self.pos;
        let mut depth = 1;
        while let Some(c) = self.current_char() {
            if c == end {
                depth -= 1;
                if depth == 0 {
                    let content = start..self.pos;
                    self.advance();
                    return Ok(content);
                }
            } else if c == '{' || c == '[' || c == '<' {
                depth += 1;
            } else if (c == '}' || c == ']' || c == '>') && depth > 1 {
                depth -= 1;
            }
            self.advance();
        }
        Err(format!("Unclosed bracket, expected '{}'", end))
    }

    /// Content up to the matching `}`, nesting on braces only.
    fn consume_braced_block(&mut self) -> Result<Span, Failure> {
        let start = self.pos;
        let mut depth = 1;
        while let Some(c) = self.current_char() {
            if c == '}' {
                depth -= 1;
                if depth == 0 {
                    let content = start..self.pos;
                    self.advance();
                    return Ok(content);
                }
            } else if c == '{' {
                depth += 1;
            }
            self.advance();
        }
        Err("Unclosed bracket, expected '}'".to_string())
    }

    /// Content up to the next `end`, without nesting.
    fn consume_until(&mut self, end: char) -> Result<Span, Failure> {
        let start = self.pos;
        while let Some(c) = self.current_char() {
            if c == end {
                let content = start..self.pos;
                self.advance();
                return Ok(content);
            }
            self.advance();
        }
        Err(format!("Expected '{}'", end))
    }

    fn expect(&mut self, c: char) -> Result<(), Failure> {
        if self.current_char() == Some(c) {
            self.advance();
            Ok(())
        } else {
            Err(format!("Expected '{}', found {:?}", c, self.current_char()))
        }
    }

    /// Digits for a gap quantity. All digits are consumed even if the number
    /// doesn't fit (it then counts as absent), as before.
    fn parse_optional_number(&mut self) -> Option<u32> {
        let digits = self.consume_while(|c| c.is_ascii_digit());
        if digits.is_empty() {
            None
        } else {
            self.src[digits].parse().ok()
        }
    }

    /// `:name:` with a name of letters, digits and underscores. On anything
    /// else the position is left unchanged.
    fn try_parse_entity(&mut self) -> Option<Span> {
        let start = self.pos;
        self.advance(); // opening colon
        let name = self.consume_while(|c| c.is_alphanumeric() || c == '_');
        if name.is_empty() || self.current_char() != Some(':') {
            self.pos = start;
            return None;
        }
        self.advance(); // closing colon
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<String> {
        parse(source)
            .nodes
            .iter()
            .map(|n| format!("{:?}@{}", std::mem::discriminant(&n.kind), &source[n.span.clone()]))
            .collect()
    }

    #[test]
    fn test_spans_cover_constructs_exactly() {
        let source = "//1 maþr .abbr[k]{konungr} -{x}- :et: ~//2";
        let tree = parse(source);
        assert!(tree.diagnostics.is_empty());
        let texts: Vec<&str> = tree.nodes.iter().map(|n| &source[n.span.clone()]).collect();
        assert_eq!(
            texts,
            vec!["//1", " maþr ", ".abbr[k]{konungr}", " ", "-{x}-", " ", ":et:", " ", "~", "//2"]
        );
    }

    #[test]
    fn test_block_content_is_parsed_into_children() {
        let source = ".head{//1 Upphaf :et: -{x}-}";
        let tree = parse(source);
        let SyntaxKind::Head { content, children } = &tree.nodes[0].kind else {
            panic!("expected a head");
        };
        assert_eq!(&source[content.clone()], "//1 Upphaf :et: -{x}-");
        let child_texts: Vec<&str> = children.iter().map(|n| &source[n.span.clone()]).collect();
        assert_eq!(child_texts, vec!["//1", " Upphaf ", ":et:", " ", "-{x}-"]);
    }

    #[test]
    fn test_recovers_and_reports_every_error_with_position() {
        let source = "ok -{never closed\nand .abbr[x] then";
        let tree = parse(source);
        assert_eq!(tree.diagnostics.len(), 2, "{:?}", tree.diagnostics);
        assert_eq!(
            tree.diagnostics[0].describe(source),
            "Line 1, column 4: Unclosed bracket, expected '}'"
        );
        assert_eq!(
            tree.diagnostics[1].describe(source),
            "Line 2, column 5: Expected '{', found Some(' ')"
        );
        // Text after the errors is still parsed
        assert!(kinds(source).last().unwrap().ends_with(" then"));
    }

    #[test]
    fn test_nested_errors_are_flagged() {
        // The head is closed; the deletion inside lacks its trailing `-`
        let tree = parse(".head{a -{b}}");
        assert_eq!(tree.diagnostics.len(), 1);
        assert!(tree.diagnostics[0].nested);
    }

    #[test]
    fn test_line_col_counts_characters() {
        assert_eq!(line_col("þþ\nþa", 0), (1, 1));
        assert_eq!(line_col("þþ\nþa", "þþ\nþ".len()), (2, 2));
    }
}
