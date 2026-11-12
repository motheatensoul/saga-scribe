//! # Word Tokenizer
//!
//! This module implements word boundary detection for grouping AST nodes into
//! TEI `<w>` (word) and `<pc>` (punctuation) elements.
//!
//! ## State Machine Design
//!
//! The tokenizer uses a simple two-state machine:
//!
//! ```text
//! [BetweenWords] ──text/entity──▶ [InWord]
//!       ▲                            │
//!       └──whitespace/punctuation────┘
//! ```
//!
//! ## Word Boundary Rules
//!
//! Words are split by:
//! - Whitespace characters
//! - Punctuation marks (configurable set)
//! - Explicit `|` word boundary markers
//! - Block elements (`.head{}`, `.supplied{}`) and numerals (`#.x.#`)
//!
//! A character written with `\` before it is part of the word, even
//! punctuation (`ꝼæ\;ðꝛ`); right after punctuation it continues the
//! punctuation (`,\,` is one `<pc>`).
//!
//! Words continue across:
//! - Line/page breaks when preceded by `~` (explicit continuation)
//! - Line/page breaks when the word ends with a letter (heuristic continuation)
//!
//! ## Compound Word Handling
//!
//! The `~` marker between word parts (e.g., `upp~haf`) keeps them as a single
//! word unit. The compiler handles the space insertion at facsimile/diplomatic
//! levels and joining at the normalized level.

use super::ast::Node;
use std::ops::Range;

/// Collects tokens with their source ranges while a word is being built.
#[derive(Default)]
struct Builder {
    result: Vec<(Node, Range<usize>)>,
    /// The word being built
    word: Vec<Node>,
    /// The source range of `word` so far
    span: Option<Range<usize>>,
}

impl Builder {
    /// Adds a node to the word being built.
    fn add(&mut self, node: Node, span: Range<usize>) {
        self.word.push(node);
        self.span = Some(match self.span.take() {
            Some(word) => word.start..span.end.max(word.end),
            None => span,
        });
    }

    /// Ends the word being built, if any. A `~` with nothing to join on one
    /// side (`upp~ haf`) is dropped, and repeated ones (`upp~~haf`) count
    /// once, rather than compiling to stray spaces.
    fn end_word(&mut self) {
        self.word
            .dedup_by(|a, b| matches!((a, b), (Node::CompoundJoin, Node::CompoundJoin)));
        while matches!(self.word.last(), Some(Node::CompoundJoin)) {
            self.word.pop();
        }
        let leading = self.word.iter().take_while(|n| matches!(n, Node::CompoundJoin)).count();
        self.word.drain(..leading);
        if !self.word.is_empty() {
            let span = self.span.take().unwrap_or(0..0);
            self.result.push((Node::Word(std::mem::take(&mut self.word)), span));
        }
        self.span = None;
    }

    /// Emits a token that is not part of a word.
    fn emit(&mut self, node: Node, span: Range<usize>) {
        self.result.push((node, span));
    }

    /// Adds a node to the punctuation token just emitted.
    fn continue_punctuation(&mut self, node: Node, span: Range<usize>) {
        if let Some((Node::Punctuation(children), token)) = self.result.last_mut() {
            children.push(node);
            token.end = token.end.max(span.end);
        }
    }
}

/// State machine states for word tokenization.
#[derive(Debug, PartialEq)]
enum State {
    /// Not currently inside a word (initial state, after whitespace/punctuation)
    BetweenWords,
    /// Currently accumulating content for a word
    InWord,
}

/// Tokenizes a flat node stream into words wrapped in Word/Punctuation nodes.
///
/// The tokenizer transforms a flat sequence of AST nodes into a sequence where
/// text content is grouped into `Word` nodes and punctuation into `Punctuation` nodes.
///
/// # Example
///
/// Input nodes: `[Text("Hello"), Text(" "), Text("world"), Text(".")]`
/// Output nodes: `[Word([Text("Hello")]), Word([Text("world")]), Punctuation([Text(".")])]`
pub struct WordTokenizer {
    /// Characters that trigger word boundaries (in addition to whitespace)
    punctuation: Vec<char>,
}

impl WordTokenizer {
    pub fn new() -> Self {
        Self {
            punctuation: vec!['.', ',', ';', ':', '!', '?', '(', ')', '[', ']'],
        }
    }

    /// Tokenizes a flat node stream into word-wrapped nodes.
    ///
    /// Processes each node and groups consecutive non-boundary nodes into
    /// `Word` containers. Handles special cases:
    ///
    /// - **Continuation (`~`)**: Marks that the word continues across the next break
    /// - **Line/page breaks**: Either split words or are included based on continuation
    /// - **Block elements**: Always force word boundaries
    /// - **Punctuation**: Wrapped in `Punctuation` nodes, not `Word`
    pub fn tokenize(&self, nodes: Vec<Node>) -> Vec<Node> {
        let spanned = nodes.into_iter().map(|node| (node, 0..0)).collect();
        self.tokenize_spanned(spanned).into_iter().map(|(node, _)| node).collect()
    }

    /// Like [`tokenize`](Self::tokenize), for nodes with their source byte
    /// ranges (as lowered from the syntax tree); returns each token with the
    /// range it covers, from its first character to its last.
    pub fn tokenize_spanned(&self, nodes: Vec<(Node, Range<usize>)>) -> Vec<(Node, Range<usize>)> {
        let mut out = Builder::default();
        let mut state = State::BetweenWords;
        // Flag set by WordContinuation node to indicate next break should be included
        let mut continuation_active = false;
        // Set after an explicit `~//`: whitespace right after the break (as in
        // `hel~//5 lo`) doesn't end the word
        let mut skip_leading_whitespace = false;
        // Punctuation was the last thing emitted, with nothing after it yet
        let mut glued_punctuation = false;

        for (node, span) in nodes {
            let after_punctuation = std::mem::take(&mut glued_punctuation);
            match &node {
                // Word boundary marker always splits
                Node::WordBoundary => {
                    out.end_word();
                    state = State::BetweenWords;
                    continuation_active = false;
                }

                // Word continuation marker - set flag but don't add to output
                Node::WordContinuation => {
                    continuation_active = true;
                }

                // Line/page breaks - determine if word continues across the break
                //
                // Three cases:
                // 1. Explicit continuation (`~//`): always continue
                // 2. Heuristic continuation: if word ends with a letter, likely continues
                // 3. Otherwise: end the word before the break
                Node::LineBreak(_) | Node::PageBreak(_) => {
                    if continuation_active {
                        // Case 1: Explicit continuation via ~ marker
                        out.add(node, span);
                        continuation_active = false;
                        state = State::InWord;
                        skip_leading_whitespace = true;
                        continue;
                    } else {
                        // Case 2: Check heuristic - if we're in a word and last char is a letter,
                        // include the break in the word (manuscript words often span lines)
                        let should_continue =
                            state == State::InWord && self.last_char_is_letter(&out.word);

                        if should_continue {
                            out.add(node, span);
                        } else {
                            // End current word, output break separately
                            out.end_word();
                            out.emit(node, span);
                            state = State::BetweenWords;
                        }
                    }
                }

                Node::Head(_) | Node::SuppliedBlock(_) | Node::Norm(_) | Node::ParagraphBreak => {
                    out.end_word();
                    out.emit(node, span);
                    state = State::BetweenWords;
                    continuation_active = false;
                }

                // Text nodes - split on whitespace and punctuation
                Node::Text(text) => {
                    let skipped = if skip_leading_whitespace {
                        text.len() - text.trim_start().len()
                    } else {
                        0
                    };
                    glued_punctuation =
                        self.process_text(&text[skipped..], span.start + skipped, &mut out, &mut state);
                    continuation_active = false;
                }

                // An escaped character: right after punctuation it continues
                // it (`,\,`), otherwise it is part of the word
                Node::Literal(c) => {
                    if after_punctuation && !c.chars().all(char::is_alphanumeric) {
                        out.continue_punctuation(node, span);
                        glued_punctuation = true;
                    } else {
                        state = State::InWord;
                        out.add(node, span);
                    }
                    continuation_active = false;
                }

                Node::Numeral(_) => {
                    out.end_word();
                    out.emit(node, span);
                    state = State::BetweenWords;
                    continuation_active = false;
                }

                // Compound join - keeps word parts together, will be handled by compiler
                // e.g., "upp~haf" stays as one word unit
                Node::CompoundJoin => {
                    if state == State::BetweenWords {
                        state = State::InWord;
                    }
                    out.add(node, span);
                    continuation_active = false;
                }

                // Other inline elements stay within the current word
                Node::Abbreviation { .. }
                | Node::Gap { .. }
                | Node::Supplied(_)
                | Node::Deletion(_)
                | Node::Addition(_)
                | Node::Note(_)
                | Node::Unclear(_)
                | Node::Entity(_) => {
                    if state == State::BetweenWords {
                        state = State::InWord;
                    }
                    out.add(node, span);
                    continuation_active = false;
                }

                // Nested words shouldn't happen, but handle gracefully
                Node::Word(_) => {
                    out.end_word();
                    out.emit(node, span);
                    state = State::BetweenWords;
                    continuation_active = false;
                }

                // Punctuation nodes pass through (already wrapped)
                Node::Punctuation(_) => {
                    out.end_word();
                    out.emit(node, span);
                    state = State::BetweenWords;
                    continuation_active = false;
                    glued_punctuation = true;
                }
            }
            skip_leading_whitespace = false;
        }

        // Flush remaining word
        out.end_word();

        out.result
            .into_iter()
            .map(|(node, span)| (Self::entity_punctuation(node), span))
            .collect()
    }

    /// A word made only of punctuation entities (`:middot:` between spaces)
    /// and escaped punctuation (`\(`) is punctuation. Glued to letters
    /// (`þa:middot:`, `:middot:Q:middot:`) an entity stays part of the word.
    fn entity_punctuation(node: Node) -> Node {
        match node {
            Node::Word(children)
                if !children.is_empty()
                    && children.iter().all(|c| match c {
                        Node::Entity(name) => crate::entities::is_punctuation_entity(name),
                        Node::Literal(c) => !c.chars().any(char::is_alphanumeric),
                        _ => false,
                    }) =>
            {
                Node::Punctuation(children)
            }
            other => other,
        }
    }

    /// Process a text node starting at source offset `at`, splitting on
    /// whitespace and punctuation. Returns whether it ended with punctuation.
    fn process_text(&self, text: &str, at: usize, out: &mut Builder, state: &mut State) -> bool {
        let mut ends_with_punctuation = false;
        // The run of regular characters not yet added to the word
        let mut buffer = String::new();
        let mut buffer_start = at;

        for (i, c) in text.char_indices() {
            let pos = at + i;
            if c.is_whitespace() {
                // Flush buffer to current word, then end the word
                if !buffer.is_empty() {
                    out.add(Node::Text(std::mem::take(&mut buffer)), buffer_start..pos);
                }
                out.end_word();
                *state = State::BetweenWords;
                ends_with_punctuation = false;
            } else if self.punctuation.contains(&c) {
                // Punctuation: flush buffer, end word, output punctuation as <pc>
                if !buffer.is_empty() {
                    out.add(Node::Text(std::mem::take(&mut buffer)), buffer_start..pos);
                }
                out.end_word();
                out.emit(Node::Punctuation(vec![Node::Text(c.to_string())]), pos..pos + c.len_utf8());
                *state = State::BetweenWords;
                ends_with_punctuation = true;
            } else {
                ends_with_punctuation = false;
                // Regular character
                if *state == State::BetweenWords {
                    *state = State::InWord;
                }
                if buffer.is_empty() {
                    buffer_start = pos;
                }
                buffer.push(c);
            }
        }

        // Flush remaining buffer
        if !buffer.is_empty() {
            out.add(Node::Text(buffer), buffer_start..at + text.len());
        }
        ends_with_punctuation
    }

    /// Check if the last character in the word is a letter (for heuristic continuation)
    fn last_char_is_letter(&self, word: &[Node]) -> bool {
        for node in word.iter().rev() {
            match node {
                Node::Text(text) => {
                    if let Some(c) = text.chars().last() {
                        return c.is_alphabetic();
                    }
                }
                Node::Entity(_) => return true, // Entities are typically letters
                Node::Literal(c) => return c.chars().all(char::is_alphabetic),
                _ => continue,
            }
        }
        false
    }
}

impl Default for WordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}
