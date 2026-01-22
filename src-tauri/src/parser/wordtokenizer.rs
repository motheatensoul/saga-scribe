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
//! - Block elements (`.head{}`, `.supplied{}`)
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
        let mut result: Vec<Node> = Vec::new();
        let mut current_word: Vec<Node> = Vec::new();
        let mut state = State::BetweenWords;
        // Flag set by WordContinuation node to indicate next break should be included
        let mut continuation_active = false;

        for node in nodes {
            match &node {
                // Word boundary marker always splits
                Node::WordBoundary => {
                    if !current_word.is_empty() {
                        result.push(Node::Word(std::mem::take(&mut current_word)));
                    }
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
                        current_word.push(node);
                        continuation_active = false;
                    } else {
                        // Case 2: Check heuristic - if we're in a word and last char is a letter,
                        // include the break in the word (manuscript words often span lines)
                        let should_continue = state == State::InWord
                            && self.last_char_is_letter(&current_word);

                        if should_continue {
                            current_word.push(node);
                        } else {
                            // End current word, output break separately
                            if !current_word.is_empty() {
                                result.push(Node::Word(std::mem::take(&mut current_word)));
                            }
                            result.push(node);
                            state = State::BetweenWords;
                        }
                    }
                }

                Node::Head(_) | Node::SuppliedBlock(_) | Node::Norm(_) => {
                    if !current_word.is_empty() {
                        result.push(Node::Word(std::mem::take(&mut current_word)));
                    }
                    result.push(node);
                    state = State::BetweenWords;
                    continuation_active = false;
                }

                // Text nodes - split on whitespace and punctuation
                Node::Text(text) => {
                    self.process_text(
                        text,
                        &mut result,
                        &mut current_word,
                        &mut state,
                    );
                    continuation_active = false;
                }

                // Compound join - keeps word parts together, will be handled by compiler
                // e.g., "upp~haf" stays as one word unit
                Node::CompoundJoin => {
                    if state == State::BetweenWords {
                        state = State::InWord;
                    }
                    current_word.push(node);
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
                    current_word.push(node);
                    continuation_active = false;
                }

                // Nested words shouldn't happen, but handle gracefully
                Node::Word(children) => {
                    if !current_word.is_empty() {
                        result.push(Node::Word(std::mem::take(&mut current_word)));
                    }
                    result.push(Node::Word(children.clone()));
                    state = State::BetweenWords;
                    continuation_active = false;
                }

                // Punctuation nodes pass through (already wrapped)
                Node::Punctuation(children) => {
                    if !current_word.is_empty() {
                        result.push(Node::Word(std::mem::take(&mut current_word)));
                    }
                    result.push(Node::Punctuation(children.clone()));
                    state = State::BetweenWords;
                    continuation_active = false;
                }
            }
        }

        // Flush remaining word
        if !current_word.is_empty() {
            result.push(Node::Word(current_word));
        }

        result
    }

    /// Process a text node, splitting on whitespace and punctuation
    fn process_text(
        &self,
        text: &str,
        result: &mut Vec<Node>,
        current_word: &mut Vec<Node>,
        state: &mut State,
    ) {
        let mut buffer = String::new();

        for c in text.chars() {
            if c.is_whitespace() {
                // Flush buffer to current word
                if !buffer.is_empty() {
                    current_word.push(Node::Text(std::mem::take(&mut buffer)));
                }
                // End current word
                if !current_word.is_empty() {
                    result.push(Node::Word(std::mem::take(current_word)));
                }
                *state = State::BetweenWords;
            } else if self.punctuation.contains(&c) {
                // Punctuation: flush buffer, end word, output punctuation as <pc>
                if !buffer.is_empty() {
                    current_word.push(Node::Text(std::mem::take(&mut buffer)));
                }
                if !current_word.is_empty() {
                    result.push(Node::Word(std::mem::take(current_word)));
                }
                // Punctuation wrapped in <pc>
                result.push(Node::Punctuation(vec![Node::Text(c.to_string())]));
                *state = State::BetweenWords;
            } else {
                // Regular character
                if *state == State::BetweenWords {
                    *state = State::InWord;
                }
                buffer.push(c);
            }
        }

        // Flush remaining buffer
        if !buffer.is_empty() {
            current_word.push(Node::Text(buffer));
        }
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
