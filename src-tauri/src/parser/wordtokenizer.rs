use super::ast::Node;

/// State machine for word tokenization
#[derive(Debug, PartialEq)]
enum State {
    BetweenWords,
    InWord,
}

/// Tokenizes a flat node stream into words wrapped in Word nodes
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

    /// Tokenize a flat node stream into words
    pub fn tokenize(&self, nodes: Vec<Node>) -> Vec<Node> {
        let mut result: Vec<Node> = Vec::new();
        let mut current_word: Vec<Node> = Vec::new();
        let mut state = State::BetweenWords;
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

                // Line/page breaks - check if continuation is active
                Node::LineBreak(_) | Node::PageBreak(_) => {
                    if continuation_active {
                        // Word continues across the break
                        current_word.push(node);
                        continuation_active = false;
                    } else {
                        // Check heuristic: if we're in a word and last char is a letter,
                        // include the break in the word (implicit continuation)
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
