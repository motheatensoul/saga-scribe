//! # Shorthand from segments
//!
//! [`segments_to_dsl`] writes the shorthand of an imported document from its
//! segments, which the document tree's projection makes
//! ([`crate::document::project_body`]); [`join_written_words`] is the rule for
//! the written words within one `<w>`.

use crate::importer::tei::segments::Segment;

/// Joins the written words of one `<w>` with `~` (compound join), so the DSL
/// reads them as one word, as the XML does: `þo at` → `þo~at`. Whitespace
/// inside constructs (`.abbr[…]{…}`, `^{…}`, `<…>`) is left alone, and
/// whitespace after a continued break (`to~//5 þings`) is dropped.
pub fn join_written_words(dsl: &str) -> String {
    let mut out = String::with_capacity(dsl.len());
    let mut depth = 0usize;
    let mut chars = dsl.trim().chars().peekable();
    // Whether the last character written was escaped (`\~` is no join)
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(c);
                out.push(next);
                escaped = true;
                continue;
            }
        }
        // A note is free prose, where only braces nest (`^{u < ?}`)
        if c == '^' && depth == 0 && chars.peek() == Some(&'{') {
            out.push(c);
            let mut braces = 0usize;
            while let Some(c) = chars.next() {
                out.push(c);
                match c {
                    '\\' => out.extend(chars.next()),
                    '{' => braces += 1,
                    '}' => {
                        braces -= 1;
                        if braces == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            escaped = false;
            continue;
        }
        match c {
            '[' | '{' | '<' => depth += 1,
            ']' | '}' | '>' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && c.is_whitespace() {
            while chars.next_if(|c| c.is_whitespace()).is_some() {}
            let after_break = out.trim_end_matches(|c: char| c.is_ascii_digit()).ends_with("~//");
            if !after_break && (escaped || !out.ends_with('~')) {
                out.push('~');
            }
            escaped = false;
            continue;
        }
        out.push(c);
        escaped = false;
    }
    out
}

/// Whether two tokens' shorthand, written without whitespace between them,
/// would read as other tokens (`skrims^{sic}` is one word with a note in
/// it, `þꝩiat` one word, `,\,` one punctuation).
fn glued(previous: &str, next: &str) -> bool {
    // Punctuation that starts a token splits from what is before it (but
    // `?{` is unclear text)
    let second = next.chars().nth(1);
    let construct = match next.chars().next() {
        Some('?') => second == Some('{'),
        // `.abbr[`, `.norm{`, …
        Some('.') => second.is_some_and(char::is_alphabetic),
        _ => true,
    };
    if !construct || next.starts_with([',', ';', '!', '(', ')']) {
        return false;
    }
    let tokens = |dsl: &str| -> Option<Vec<String>> {
        let nodes = crate::parser::Lexer::new(dsl).parse().ok()?.nodes;
        Some(
            crate::parser::WordTokenizer::new()
                .tokenize(nodes)
                .iter()
                .filter(|node| crate::identity::is_token(node))
                .map(crate::identity::node_to_dsl)
                .collect(),
        )
    };
    match (tokens(previous), tokens(next), tokens(&format!("{previous}{next}"))) {
        (Some(mut apart), Some(next), Some(together)) => {
            apart.extend(next);
            apart != together
        }
        _ => false,
    }
}

/// Whether a token's shorthand ends with a break it continues across
/// (`míſ~//2`, `ꝩand~///10v`): whitespace after it doesn't end the word.
fn ends_with_continued_break(dsl: &str) -> bool {
    dsl.rfind("~//").is_some_and(|at| !dsl[at..].contains(char::is_whitespace))
}

/// Whether a punctuation segment's DSL is made only of punctuation entities
/// (`:middot:`), which the DSL marks by setting them off with spaces.
fn is_entity_punctuation(dsl: &str) -> bool {
    let dsl = dsl.trim();
    dsl.len() > 2
        && dsl.starts_with(':')
        && dsl.ends_with(':')
        && dsl[1..dsl.len() - 1].split("::").all(|name| {
            !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && crate::entities::is_punctuation_entity(name)
        })
}

/// Converts a segment list back to DSL text representation.
///
/// This is used for:
/// 1. Initial DSL generation after import
/// 2. Fast-path comparison in patch computation (unchanged document detection)
///
/// # Formatting Rules
///
/// - Words are separated by spaces (from Whitespace segments)
/// - Punctuation immediately follows words (no space), except entity
///   punctuation (`:middot:`), which is set off by spaces
/// - Paragraph starts: `.p`, set off by spaces
/// - Line breaks: newline + `//n` + space
/// - Page breaks: newline + `///n`; from rules 2 on, a word or punctuation
///   right after one starts a new line, so the page number doesn't run into it
/// - Structural elements are skipped (not represented in DSL)
///
/// `dsl_rules` is the document's [`DSL_RULES`](super::segments::DSL_RULES) version.
pub fn segments_to_dsl(segments: &[Segment], dsl_rules: u32) -> String {
    let mut dsl = String::new();
    let mut last_was_linebreak = false;
    let mut pending_space = false;  // Track if we should add a space before next word
    // The last word or punctuation written, while nothing else follows it
    let mut previous: Option<&str> = None;

    // From rules 3 on, a token written right after another that it would
    // run into is set apart: a note by a space, others by `|`. After a word
    // continued across a break (`míſ~//2`) a space doesn't end the word, so
    // it is replaced by `|`.
    let separate = |dsl: &mut String, previous: Option<&str>, next: &str| {
        let Some(previous) = previous else { return };
        if dsl_rules < 3 {
            return;
        }
        let spaced = dsl.ends_with(char::is_whitespace);
        if spaced && !ends_with_continued_break(previous) {
            return;
        }
        let space = if spaced { " " } else { "" };
        if !glued(&format!("{previous}{space}"), next) {
            return;
        }
        if next.starts_with("^{") && !ends_with_continued_break(previous) {
            dsl.push(' ');
        } else {
            dsl.truncate(dsl.trim_end().len());
            dsl.push('|');
        }
    };

    for segment in segments {
        match segment {
            Segment::Word { dsl_content, .. } => {
                if last_was_linebreak && dsl_rules >= 2 {
                    dsl.push('\n');
                }
                // Only add space if there was whitespace in the source XML
                if pending_space && !last_was_linebreak {
                    dsl.push(' ');
                }
                separate(&mut dsl, previous, dsl_content);
                dsl.push_str(dsl_content);
                last_was_linebreak = false;
                pending_space = false;
                // A word without shorthand (an empty `<w>`) isn't a token
                if !dsl_content.trim().is_empty() {
                    previous = Some(dsl_content);
                }
            }
            Segment::Punctuation { dsl_content, .. }
                if dsl_rules >= 1 && is_entity_punctuation(dsl_content) =>
            {
                // Set off by spaces: glued to a word it would be part of the word
                if !dsl.is_empty() && !dsl.ends_with(char::is_whitespace) {
                    dsl.push(' ');
                }
                dsl.push_str(dsl_content);
                last_was_linebreak = false;
                pending_space = true;
                previous = Some(dsl_content);
            }
            Segment::Punctuation { dsl_content, .. } => {
                if last_was_linebreak && dsl_rules >= 2 {
                    dsl.push('\n');
                }
                // Punctuation typically follows immediately after word (no space)
                separate(&mut dsl, previous, dsl_content);
                dsl.push_str(dsl_content);
                last_was_linebreak = false;
                pending_space = false;
                previous = Some(dsl_content);
            }
            Segment::LineBreak { attributes, .. } => {
                // After a word continued across a break, a break of its own
                // is set apart (the word would run on across it)
                if dsl_rules >= 3 && previous.is_some_and(ends_with_continued_break) {
                    dsl.push('|');
                }
                // Line break on new line, then continue on same line
                dsl.push('\n');
                if let Some(n) = attributes.get("n") {
                    dsl.push_str(&format!("//{}", n));
                } else {
                    dsl.push_str("//");
                }
                dsl.push(' ');
                last_was_linebreak = false;
                pending_space = false;
                previous = None;
            }
            Segment::PageBreak { attributes, .. } => {
                if dsl_rules >= 3 && previous.is_some_and(ends_with_continued_break) {
                    dsl.push('|');
                }
                // Page break: put on new line, no trailing newline (content follows)
                dsl.push('\n');
                if let Some(n) = attributes.get("n") {
                    dsl.push_str(&format!("///{}", n));
                } else {
                    dsl.push_str("///");
                }
                last_was_linebreak = true;
                pending_space = false;
                previous = None;
            }
            Segment::HandShift { .. } => {
                // HandShift is preserved structurally, not shown in DSL
            }
            Segment::Paragraph { .. } => {
                // Standalone token: always set off by spaces
                if !dsl.is_empty() && !dsl.ends_with(char::is_whitespace) {
                    dsl.push(' ');
                }
                dsl.push_str(".p ");
                last_was_linebreak = false;
                pending_space = false;
                previous = None;
            }
            Segment::Structural { .. } => {
                // Skip structural elements in DSL output
            }
            Segment::Whitespace { content, .. } => {
                // Whitespace signals that next word should have a leading space
                if content.contains(' ') || content.contains('\t') || content.contains('\n') {
                    pending_space = true;
                }
            }
        }
    }

    dsl.trim().to_string()
}
