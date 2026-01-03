use super::ast::{Document, Node};

/// Tokenizes and parses the DSL input into an AST
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Document, String> {
        let mut doc = Document::new();
        let mut text_buf = String::new();

        while self.pos < self.input.len() {
            let remaining = &self.input[self.pos..];

            // Check for page break first (/// before //)
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
                let line_num = self.consume_until_whitespace();
                let line_num = if line_num.is_empty() {
                    None
                } else {
                    Some(line_num)
                };
                doc.push(Node::LineBreak(line_num));
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

            // Gap: [...] or [...n]
            if remaining.starts_with("[...") {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 4;
                let quantity = self.parse_optional_number();
                self.expect(']')?;
                doc.push(Node::Gap { quantity });
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

            // Explicit word boundary: |
            if remaining.starts_with('|') {
                self.flush_text(&mut doc, &mut text_buf);
                self.pos += 1;
                doc.push(Node::WordBoundary);
                continue;
            }

            // Regular character
            text_buf.push(self.current_char().unwrap());
            self.pos += 1;
        }

        self.flush_text(&mut doc, &mut text_buf);
        Ok(doc)
    }

    fn flush_text(&self, doc: &mut Document, buf: &mut String) {
        if !buf.is_empty() {
            doc.push(Node::Text(std::mem::take(buf)));
        }
    }

    fn current_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn consume_until_whitespace(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.input.len() {
            if self.current_char().map_or(true, |c| c.is_whitespace()) {
                break;
            }
            self.pos += 1;
        }
        self.input[start..self.pos].to_string()
    }

    fn consume_bracketed(&mut self, end: char) -> Result<String, String> {
        let start = self.pos;
        let mut depth = 1;
        while self.pos < self.input.len() {
            let c = self.current_char().unwrap();
            if c == end {
                depth -= 1;
                if depth == 0 {
                    let result = self.input[start..self.pos].to_string();
                    self.pos += 1;
                    return Ok(result);
                }
            } else if c == '{' || c == '[' || c == '<' {
                depth += 1;
            }
            self.pos += 1;
        }
        Err(format!("Unclosed bracket, expected '{}'", end))
    }

    fn consume_until(&mut self, end: char) -> Result<String, String> {
        let start = self.pos;
        while self.pos < self.input.len() {
            if self.current_char() == Some(end) {
                let result = self.input[start..self.pos].to_string();
                self.pos += 1;
                return Ok(result);
            }
            self.pos += 1;
        }
        Err(format!("Expected '{}'", end))
    }

    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.current_char() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "Expected '{}', found {:?}",
                c,
                self.current_char()
            ))
        }
    }

    fn parse_optional_number(&mut self) -> Option<u32> {
        let start = self.pos;
        while self.pos < self.input.len() {
            if !self.current_char().map_or(false, |c| c.is_ascii_digit()) {
                break;
            }
            self.pos += 1;
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
        self.pos += 1;

        // Collect entity name (alphanumeric + underscore)
        let name_start = self.pos;
        while self.pos < self.input.len() {
            match self.current_char() {
                Some(c) if c.is_alphanumeric() || c == '_' => {
                    self.pos += 1;
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
        self.pos += 1; // consume closing colon

        Some(name)
    }
}
