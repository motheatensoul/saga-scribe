//! # Parsing TEI documents
//!
//! MENOTA documents use named entities (`&thorn;`, `&middot;`) declared in an
//! external list that is never fetched (no network access while parsing), or
//! not declared at all (Saga-Scribe's own exports before 0.6.0). libxml2
//! treats such undeclared entities differently by version: 2.14+ keeps them as
//! references, 2.9 (Ubuntu 24.04) drops them, losing characters.
//!
//! So before parsing, every entity a document uses is declared in its
//! internal DTD subset, with its character from the bundled MENOTA list.
//! libxml2 then keeps each one as an entity reference node in every version
//! (entities are never substituted), which is what the importer reads.
//!
//! The declarations go into the prologue on an existing line, so line
//! numbers (validation errors) and the body's bytes are unchanged.

use libxml::parser::{Parser, ParserOptions};
use libxml::tree::Document;
use std::borrow::Cow;
use std::collections::BTreeSet;

/// Parses a TEI document without network access, with its entities declared
/// (see the module docs).
///
/// # Errors
///
/// Returns libxml2's error if the document can't be parsed.
pub fn parse(xml: &str) -> Result<Document, String> {
    let xml = declare_entities_for_parsing(xml);
    Parser::default()
        .parse_string_with_options(xml.as_ref(), ParserOptions { no_net: true, ..Default::default() })
        .map_err(|e| format!("Failed to parse XML: {}", e))
}

/// `xml` with every named entity it uses declared in its internal DTD
/// subset (added, or created with a DOCTYPE before the root element).
pub fn declare_entities_for_parsing(xml: &str) -> Cow<'_, str> {
    let used = used_entities(xml);
    if used.is_empty() {
        return Cow::Borrowed(xml);
    }
    let bundled = crate::entities::bundled();
    let declarations: String = used
        .iter()
        .map(|name| {
            let text = bundled.get(*name).map_or("\u{FFFD}", |e| e.text.as_str());
            let value: String = text.chars().map(|c| format!("&#x{:X};", c as u32)).collect();
            format!("<!ENTITY {} \"{}\">", name, value)
        })
        .collect();

    match find_doctype(xml) {
        // Into the existing internal subset
        Some(Doctype { subset_start: Some(at), .. }) => {
            Cow::Owned(format!("{}{}{}", &xml[..at], declarations, &xml[at..]))
        }
        // As a new internal subset of the existing DOCTYPE
        Some(Doctype { end, subset_start: None }) => {
            Cow::Owned(format!("{} [{}]{}", &xml[..end], declarations, &xml[end..]))
        }
        // As a new DOCTYPE, right before the root element
        None => match root_element(xml) {
            Some((at, name)) => Cow::Owned(format!(
                "{}<!DOCTYPE {} [{}]>{}",
                &xml[..at],
                name,
                declarations,
                &xml[at..]
            )),
            None => Cow::Borrowed(xml),
        },
    }
}

/// Names of the entities referenced in `xml`, other than XML's predefined five.
fn used_entities(xml: &str) -> BTreeSet<&str> {
    const PREDEFINED: [&str; 5] = ["lt", "gt", "amp", "quot", "apos"];
    xml.match_indices('&')
        .filter_map(|(i, _)| {
            let rest = &xml[i + 1..];
            let len = rest
                .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':')))
                .unwrap_or(rest.len());
            let name = &rest[..len];
            let starts_name = name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_');
            (starts_name && rest[len..].starts_with(';') && !PREDEFINED.contains(&name)).then_some(name)
        })
        .collect()
}

struct Doctype {
    /// Offset of the DOCTYPE's closing `>`
    end: usize,
    /// Offset just inside the internal subset's `[`, if it has one
    subset_start: Option<usize>,
}

/// The document type declaration, if any (before the root element).
fn find_doctype(xml: &str) -> Option<Doctype> {
    let start = xml.find("<!DOCTYPE")?;
    if root_element(xml).is_some_and(|(root, _)| root < start) {
        return None;
    }
    // Skip quoted system/public identifiers to find `[` or the closing `>`
    let mut quote: Option<char> = None;
    for (i, c) in xml[start..].char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '[') => {
                let subset_start = start + i + 1;
                let close = xml[subset_start..].find("]")? + subset_start;
                let end = xml[close..].find('>')? + close;
                return Some(Doctype { end, subset_start: Some(subset_start) });
            }
            (None, '>') => return Some(Doctype { end: start + i, subset_start: None }),
            _ => {}
        }
    }
    None
}

/// Offset and name of the root element's start tag.
fn root_element(xml: &str) -> Option<(usize, &str)> {
    let at = xml
        .match_indices('<')
        .map(|(i, _)| i)
        .find(|&i| !xml[i..].starts_with("<?") && !xml[i..].starts_with("<!"))?;
    let tag = &xml[at + 1..];
    let len = tag.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(tag.len());
    Some((at, &tag[..len]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use libxml::tree::NodeType;

    /// The names of the entity reference nodes in `doc`, in order.
    fn entity_refs(doc: &Document) -> Vec<String> {
        fn walk(node: &libxml::tree::Node, out: &mut Vec<String>) {
            if node.get_type() == Some(NodeType::EntityRefNode) {
                out.push(node.get_name());
            }
            let mut child = node.get_first_child();
            while let Some(c) = child {
                walk(&c, out);
                child = c.get_next_sibling();
            }
        }
        let mut out = Vec::new();
        walk(&doc.get_root_element().unwrap(), &mut out);
        out
    }

    #[test]
    fn test_entities_survive_with_or_without_doctype() {
        let body = "<TEI><w>&thorn;a&middot;</w><w>a &amp; &unknownname;</w></TEI>";
        let documents = [
            body.to_string(),
            format!("<?xml version=\"1.0\"?>\n{body}"),
            format!("{}\n{body}", crate::entities::MENOTA_DOCTYPE),
            format!("<!DOCTYPE TEI SYSTEM \"tei.dtd\">{body}"),
        ];
        for xml in &documents {
            let doc = parse(xml).unwrap();
            assert_eq!(entity_refs(&doc), ["thorn", "middot", "unknownname"], "{xml}");
        }
    }

    #[test]
    fn test_declarations_keep_line_numbers_and_body() {
        let xml = "<?xml version=\"1.0\"?>\n<!DOCTYPE TEI [\n<!ENTITY % x SYSTEM \"x\">\n]>\n<TEI>&thorn;</TEI>";
        let declared = declare_entities_for_parsing(xml);
        assert_eq!(declared.lines().count(), xml.lines().count());
        assert!(declared.ends_with("<TEI>&thorn;</TEI>"));
        assert!(declared.contains("<!ENTITY thorn \"&#xFE;\">"), "{declared}");

        let plain = "<TEI>a &amp; b &#x27;</TEI>";
        assert!(matches!(declare_entities_for_parsing(plain), Cow::Borrowed(_)));
    }
}
