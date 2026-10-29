//! # Source Byte-Span Tracking
//!
//! libxml2 does not expose byte offsets for parsed nodes, so this module walks the
//! original body text in lockstep with the extractor's DOM traversal and records
//! where each editable segment lives in the source.
//!
//! These spans let the patcher splice edits directly into the user's original
//! bytes: anything that was not edited is emitted exactly as it was read.
//!
//! The cursor is strict. As soon as the raw text disagrees with the DOM, it gives
//! up and reports no spans at all, and the patcher falls back to reconstruction.
//! A partial span map would be worse than none.

use std::ops::Range;

/// A forward-only cursor over the raw source of a `<body>` element.
pub struct SourceCursor {
    src: String,
    pos: usize,
    /// For each open structural tag: whether it was written as `<x/>`.
    self_closing: Vec<bool>,
}

impl SourceCursor {
    pub fn new(src: &str) -> Self {
        Self {
            src: src.to_string(),
            pos: 0,
            self_closing: Vec::new(),
        }
    }

    fn rest(&self) -> &str {
        &self.src[self.pos..]
    }

    /// Consumes a complete element (start tag through matching end tag, or an
    /// empty-element tag) and returns its byte range.
    pub fn element(&mut self, qname: &str) -> Option<Range<usize>> {
        let start = self.pos;
        let (tag_end, self_closing) = self.match_start_tag(qname)?;
        self.pos = tag_end;
        if !self_closing {
            self.skip_to_matching_end(qname)?;
        }
        Some(start..self.pos)
    }

    /// Consumes the start tag of a structural element whose children the
    /// extractor will visit individually, and returns the tag's byte range.
    pub fn open_tag(&mut self, qname: &str) -> Option<Range<usize>> {
        let start = self.pos;
        let (tag_end, self_closing) = self.match_start_tag(qname)?;
        self.pos = tag_end;
        self.self_closing.push(self_closing);
        Some(start..tag_end)
    }

    /// Consumes the end tag matching the most recent [`open_tag`](Self::open_tag)
    /// and returns its byte range (empty for an empty-element tag).
    pub fn close_tag(&mut self, qname: &str) -> Option<Range<usize>> {
        let start = self.pos;
        if self.self_closing.pop()? {
            return Some(start..start);
        }
        self.pos = self.match_end_tag(self.pos, qname)?;
        Some(start..self.pos)
    }

    /// Consumes a text node and returns the source offset of every character
    /// boundary in `content` (so `offsets[i]..offsets[j]` covers chars `i..j`).
    pub fn text(&mut self, content: &str) -> Option<Vec<usize>> {
        let mut offsets = Vec::with_capacity(content.chars().count() + 1);
        let mut pos = self.pos;
        for expected in content.chars() {
            offsets.push(pos);
            pos = self.match_text_char(pos, expected)?;
        }
        offsets.push(pos);
        self.pos = pos;
        Some(offsets)
    }

    pub fn comment(&mut self) -> Option<()> {
        if !self.rest().starts_with("<!--") {
            return None;
        }
        let end = self.rest().find("-->")?;
        self.pos += end + 3;
        Some(())
    }

    pub fn cdata(&mut self) -> Option<()> {
        if !self.rest().starts_with("<![CDATA[") {
            return None;
        }
        let end = self.rest().find("]]>")?;
        self.pos += end + 3;
        Some(())
    }

    pub fn processing_instruction(&mut self) -> Option<()> {
        if !self.rest().starts_with("<?") {
            return None;
        }
        let end = self.rest().find("?>")?;
        self.pos += end + 2;
        Some(())
    }

    pub fn entity_ref(&mut self, name: &str) -> Option<()> {
        let token = format!("&{};", name);
        if !self.rest().starts_with(&token) {
            return None;
        }
        self.pos += token.len();
        Some(())
    }

    /// Matches `<qname ...>` or `<qname .../>` at the cursor.
    /// Returns the offset just past the tag and whether it was self-closing.
    fn match_start_tag(&self, qname: &str) -> Option<(usize, bool)> {
        let rest = self.rest();
        let after_name = rest.strip_prefix('<')?.strip_prefix(qname)?;
        match after_name.chars().next()? {
            c if c.is_whitespace() || c == '>' || c == '/' => {}
            _ => return None,
        }
        let tag_len = tag_length(rest)?;
        let self_closing = rest[..tag_len].ends_with("/>");
        Some((self.pos + tag_len, self_closing))
    }

    fn match_end_tag(&self, at: usize, qname: &str) -> Option<usize> {
        let rest = &self.src[at..];
        let after_name = rest.strip_prefix("</")?.strip_prefix(qname)?;
        let trimmed = after_name.trim_start();
        if !trimmed.starts_with('>') {
            return None;
        }
        Some(at + rest.len() - trimmed.len() + 1)
    }

    /// Advances past nested content until the end tag that closes the element
    /// whose start tag was just consumed.
    fn skip_to_matching_end(&mut self, qname: &str) -> Option<()> {
        let mut depth = 0usize;
        loop {
            let lt = self.rest().find('<')?;
            self.pos += lt;
            let rest = self.rest();
            if rest.starts_with("<!--") {
                self.comment()?;
            } else if rest.starts_with("<![CDATA[") {
                self.cdata()?;
            } else if rest.starts_with("<?") {
                self.processing_instruction()?;
            } else if rest.starts_with("</") {
                if depth == 0 {
                    self.pos = self.match_end_tag(self.pos, qname)?;
                    return Some(());
                }
                depth -= 1;
                self.pos += rest.find('>')? + 1;
            } else {
                let len = tag_length(rest)?;
                if !rest[..len].ends_with("/>") {
                    depth += 1;
                }
                self.pos += len;
            }
        }
    }

    /// Matches one character of DOM text content against the source, decoding
    /// predefined entities, character references and line-ending normalization.
    fn match_text_char(&self, pos: usize, expected: char) -> Option<usize> {
        let rest = &self.src[pos..];
        let actual = rest.chars().next()?;
        match actual {
            '<' => None,
            '&' => {
                let semi = rest.find(';')?;
                let reference = &rest[1..semi];
                let decoded = match reference {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    _ => {
                        let code = if let Some(hex) = reference
                            .strip_prefix("#x")
                            .or_else(|| reference.strip_prefix("#X"))
                        {
                            u32::from_str_radix(hex, 16).ok()?
                        } else {
                            reference.strip_prefix('#')?.parse().ok()?
                        };
                        char::from_u32(code)?
                    }
                };
                (decoded == expected).then_some(pos + semi + 1)
            }
            '\r' if expected == '\n' => {
                let skip = if rest[1..].starts_with('\n') { 2 } else { 1 };
                Some(pos + skip)
            }
            c => (c == expected).then_some(pos + c.len_utf8()),
        }
    }
}

/// Length of the tag starting at `s[0] == '<'`, honouring quoted attribute values.
fn tag_length(s: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices().skip(1) {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '>' => return Some(i + 1),
            None if c == '<' => return None,
            None => {}
        }
    }
    None
}

/// Replaces, inserts or removes an attribute inside a single start or empty tag,
/// leaving the rest of the tag byte-for-byte unchanged.
pub fn set_tag_attribute(tag: &str, name: &str, value: Option<&str>) -> String {
    if let Some(range) = find_attribute(tag, name) {
        let mut out = String::with_capacity(tag.len());
        match value {
            Some(v) => {
                out.push_str(&tag[..range.start]);
                out.push_str(&format!("{}=\"{}\"", name, super::helpers::escape_xml_attr(v)));
                out.push_str(&tag[range.end..]);
            }
            None => {
                // Drop the attribute together with the whitespace before it.
                let start = tag[..range.start].trim_end().len();
                out.push_str(&tag[..start]);
                out.push_str(&tag[range.end..]);
            }
        }
        return out;
    }
    match value {
        Some(v) => {
            let close = if tag.ends_with("/>") { tag.len() - 2 } else { tag.len() - 1 };
            let head = tag[..close].trim_end();
            format!(
                "{} {}=\"{}\"{}",
                head,
                name,
                super::helpers::escape_xml_attr(v),
                &tag[close..]
            )
        }
        None => tag.to_string(),
    }
}

/// Finds the byte range of `name="value"` (either quote style) inside a tag.
fn find_attribute(tag: &str, name: &str) -> Option<Range<usize>> {
    let bytes = tag.as_bytes();
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b.is_ascii_whitespace() => {
                let start = i + 1;
                if tag[start..].starts_with(name) {
                    let after = tag[start + name.len()..].trim_start();
                    if let Some(after_eq) = after.strip_prefix('=') {
                        let value = after_eq.trim_start();
                        let q = value.chars().next()?;
                        if q == '"' || q == '\'' {
                            let value_start = tag.len() - value.len();
                            let close = tag[value_start + 1..].find(q)?;
                            return Some(start..value_start + 1 + close + 1);
                        }
                    }
                }
            }
            None => {}
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_span_with_nesting_and_quotes() {
        let src = r#"<w a="x>y"><w>inner</w><!-- </w> --></w> tail"#;
        let mut cursor = SourceCursor::new(src);
        let span = cursor.element("w").unwrap();
        assert_eq!(&src[span], r#"<w a="x>y"><w>inner</w><!-- </w> --></w>"#);
    }

    #[test]
    fn test_element_rejects_prefix_name_match() {
        let mut cursor = SourceCursor::new("<wx/>");
        assert!(cursor.element("w").is_none());
    }

    #[test]
    fn test_text_decodes_references() {
        let src = "a&amp;b&#x41;\r\nc<";
        let mut cursor = SourceCursor::new(src);
        let offsets = cursor.text("a&bA\nc").unwrap();
        assert_eq!(offsets, vec![0, 1, 6, 7, 13, 15, 16]);
    }

    #[test]
    fn test_text_mismatch_fails() {
        let mut cursor = SourceCursor::new("abc");
        assert!(cursor.text("abd").is_none());
    }

    #[test]
    fn test_set_tag_attribute() {
        assert_eq!(
            set_tag_attribute(r#"<lb ed="x" n="3"/>"#, "n", Some("4")),
            r#"<lb ed="x" n="4"/>"#
        );
        assert_eq!(set_tag_attribute("<lb/>", "n", Some("4")), r#"<lb n="4"/>"#);
        assert_eq!(set_tag_attribute(r#"<lb n='3' />"#, "n", None), "<lb />");
        // An attribute whose name merely ends with "n" must not match.
        assert_eq!(
            set_tag_attribute(r#"<lb xn="1"/>"#, "n", Some("2")),
            r#"<lb xn="1" n="2"/>"#
        );
    }
}
