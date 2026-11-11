//! Writing a [`Tree`] back to text.
//!
//! A node that is as it was read is copied from the original text, so an
//! unchanged document is written byte for byte, and an edited one differs only
//! where it was edited. Changed nodes are written fresh; an element whose
//! attributes changed is patched in its original start tag, so the other
//! attributes keep their order, quotes and spelling.

use super::tree::{Attribute, Element, NodeId, NodeKind, Tree};
use crate::importer::tei::source_map::{set_tag_attribute, set_tag_attribute_escaped};

impl Tree {
    /// The document as text.
    ///
    /// # Errors
    ///
    /// If the tree holds something XML can't express (a comment containing
    /// `--`, a name that isn't one, a control character).
    pub fn write(&self) -> Result<String, String> {
        let mut out = String::with_capacity(self.source().len() + 64);
        if self.has_spans() {
            self.write_document_with_spans(&mut out)?;
        } else {
            self.write_document_without_spans(&mut out)?;
        }
        Ok(out)
    }

    /// [`write`](Self::write), checked: the output is parsed by libxml2 again
    /// and must give the same tree. Use this for anything that is saved.
    ///
    /// # Errors
    ///
    /// If writing fails, or the output doesn't read as this tree.
    pub fn write_checked(&self) -> Result<String, String> {
        let out = self.write()?;
        let reread = Tree::parse(&out).map_err(|e| format!("The written document doesn't parse: {e}"))?;
        match first_difference(self, Tree::DOCUMENT, &reread, Tree::DOCUMENT) {
            None => Ok(out),
            Some(difference) => Err(format!("The written document reads differently: {difference}")),
        }
    }

    fn write_document_with_spans(&self, out: &mut String) -> Result<(), String> {
        if self.is_unchanged(Tree::DOCUMENT) {
            out.push_str(self.source());
            return Ok(());
        }
        // What lies between top-level nodes (the XML declaration, whitespace)
        // isn't a node; it's copied as it was around the nodes still there
        let mut copied_to = 0;
        for &child in self.children(Tree::DOCUMENT) {
            match self.span(child) {
                Some(span) if span.start >= copied_to => {
                    out.push_str(&self.source()[copied_to..span.start]);
                    copied_to = span.end;
                }
                _ => {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }
            self.write_node(child, out)?;
        }
        out.push_str(&self.source()[copied_to..]);
        Ok(())
    }

    /// Without spans the prolog (XML declaration, DOCTYPE, comments before the
    /// root) is copied as written up to the root element, which is written
    /// from the tree with what follows it.
    fn write_document_without_spans(&self, out: &mut String) -> Result<(), String> {
        let root = self.root_element();
        let prolog_end = root.and(root_element_offset(self.source()));
        let mut after_root = prolog_end.is_none();
        if let Some(end) = prolog_end {
            out.push_str(&self.source()[..end]);
        }
        for &child in self.children(Tree::DOCUMENT) {
            if !after_root {
                after_root = Some(child) == root;
                if !after_root {
                    continue;
                }
            } else if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            self.write_node(child, out)?;
        }
        if self.source().ends_with('\n') && !out.ends_with('\n') {
            out.push('\n');
        }
        Ok(())
    }

    fn write_node(&self, id: NodeId, out: &mut String) -> Result<(), String> {
        if let Some(span) = self.span(id) {
            if self.is_unchanged(id) {
                out.push_str(&self.source()[span]);
                return Ok(());
            }
        }
        match self.kind(id) {
            NodeKind::Element(element) => self.write_element(id, element, out),
            NodeKind::Text(text) => write_text(text, out),
            NodeKind::EntityRef(name) => {
                check_name(name)?;
                out.push('&');
                out.push_str(name);
                out.push(';');
                Ok(())
            }
            NodeKind::Comment(text) => {
                if text.contains("--") || text.ends_with('-') {
                    return Err(format!("A comment can't contain \"--\" or end with \"-\": {text:?}"));
                }
                check_chars(text)?;
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->");
                Ok(())
            }
            NodeKind::Cdata(text) => {
                if text.contains("]]>") {
                    return Err(format!("A CDATA section can't contain \"]]>\": {text:?}"));
                }
                check_chars(text)?;
                out.push_str("<![CDATA[");
                out.push_str(text);
                out.push_str("]]>");
                Ok(())
            }
            NodeKind::ProcessingInstruction { target, data } => {
                check_name(target)?;
                if data.contains("?>") {
                    return Err(format!("A processing instruction can't contain \"?>\": {data:?}"));
                }
                check_chars(data)?;
                out.push_str("<?");
                out.push_str(target);
                if !data.is_empty() {
                    out.push(' ');
                    out.push_str(data);
                }
                out.push_str("?>");
                Ok(())
            }
            NodeKind::Doctype(text) => {
                out.push_str(text);
                Ok(())
            }
            NodeKind::Document => Err("A document can't be inside a document".to_string()),
        }
    }

    fn write_element(&self, id: NodeId, element: &Element, out: &mut String) -> Result<(), String> {
        let children = self.children(id);
        let tags = self.tags(id);
        let source = self.source();

        // Start tag: as written, patched where attributes changed, or new
        let original_start = tags.as_ref().map(|(start, _)| &source[start.clone()]);
        let mut start_tag = match original_start {
            Some(tag) if !self.own_data_changed(id) => tag.to_string(),
            Some(tag) => match self.original_attributes(id) {
                Some(before) => patch_start_tag(tag, before, &element.attributes),
                None => tag.to_string(),
            },
            None => new_start_tag(element)?,
        };
        if start_tag.ends_with("/>") && !children.is_empty() {
            // `<x/>` (as written, or new) that has content
            start_tag.truncate(start_tag.len() - 2);
            start_tag.push('>');
        }
        out.push_str(&start_tag);

        for &child in children {
            self.write_node(child, out)?;
        }

        // End tag: as written, or new where the element had none
        match tags.as_ref().map(|(_, end)| &source[end.clone()]) {
            Some(end) if !end.is_empty() => out.push_str(end),
            _ if !children.is_empty() => {
                out.push_str("</");
                out.push_str(&element.name);
                out.push('>');
            }
            _ => {}
        }
        Ok(())
    }
}

/// `<name xmlns:… attr="…"/>`: a new element's start tag, written as empty
/// (the caller opens it when it has content).
fn new_start_tag(element: &Element) -> Result<String, String> {
    check_name(&element.name)?;
    let mut tag = format!("<{}", element.name);
    for (prefix, uri) in &element.namespaces {
        let name = if prefix.is_empty() { "xmlns".to_string() } else { format!("xmlns:{prefix}") };
        push_attribute(&mut tag, &name, uri)?;
    }
    for attribute in &element.attributes {
        push_attribute(&mut tag, &attribute.name, &attribute.value)?;
    }
    tag.push_str("/>");
    Ok(tag)
}

fn push_attribute(tag: &mut String, name: &str, value: &str) -> Result<(), String> {
    check_name(name)?;
    check_chars(value)?;
    tag.push(' ');
    tag.push_str(name);
    tag.push_str("=\"");
    tag.push_str(&escape_attribute(value));
    tag.push('"');
    Ok(())
}

/// The original start tag with only the attributes that changed rewritten.
fn patch_start_tag(tag: &str, before: &[Attribute], after: &[Attribute]) -> String {
    let mut tag = tag.to_string();
    for old in before {
        if !after.iter().any(|a| a.name == old.name) {
            tag = set_tag_attribute(&tag, &old.name, None);
        }
    }
    for new in after {
        let unchanged = before.iter().any(|a| a.name == new.name && a.value == new.value);
        if !unchanged {
            tag = set_attribute_exact(&tag, &new.name, &new.value);
        }
    }
    tag
}

/// Sets an attribute in a start tag as written, with the value escaped so it
/// reads back exactly (tabs and line breaks would otherwise read as spaces).
fn set_attribute_exact(tag: &str, name: &str, value: &str) -> String {
    set_tag_attribute_escaped(tag, name, Some(&escape_attribute(value)))
}

fn write_text(text: &str, out: &mut String) -> Result<(), String> {
    check_chars(text)?;
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            // A literal CR would read as a line feed
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
    Ok(())
}

fn escape_attribute(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
    out
}

/// Characters XML 1.0 can't contain, even as references.
fn check_chars(text: &str) -> Result<(), String> {
    match text.chars().find(|&c| {
        matches!(c, '\u{0}'..='\u{8}' | '\u{B}' | '\u{C}' | '\u{E}'..='\u{1F}' | '\u{FFFE}' | '\u{FFFF}')
    }) {
        Some(c) => Err(format!("XML can't contain the character U+{:04X}", c as u32)),
        None => Ok(()),
    }
}

/// A conservative check of an XML name (with at most one prefix).
fn check_name(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name.split(':').count() <= 2
        && name.split(':').all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
                && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '\u{B7}'))
        });
    if valid { Ok(()) } else { Err(format!("Not an XML name: {name:?}")) }
}

/// Offset of the root element's start tag.
fn root_element_offset(xml: &str) -> Option<usize> {
    let mut search = 0;
    while let Some(i) = xml[search..].find('<').map(|i| i + search) {
        let rest = &xml[i..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            // From after the opener: in `<!-->x-->` the opener and `>` read `-->`
            search = i + 4 + comment.find("-->")? + 3;
        } else if rest.starts_with("<?") {
            search = i + rest.find("?>")? + 2;
        } else if rest.starts_with("<!DOCTYPE") {
            search = crate::xml::doctype_span(xml)?.end;
        } else {
            return Some(i);
        }
    }
    None
}

/// Where two trees first differ in content, if they do. Spans and formatting
/// don't count; adjacent text nodes count as one (libxml2 merges them when
/// reading), and empty ones as none.
pub(super) fn first_difference(a: &Tree, a_id: NodeId, b: &Tree, b_id: NodeId) -> Option<String> {
    if a.kind(a_id) != b.kind(b_id) {
        return Some(format!("{:?} became {:?}", a.kind(a_id), b.kind(b_id)));
    }
    let (a_children, b_children) = (merged_children(a, a_id), merged_children(b, b_id));
    if a_children.len() != b_children.len() {
        return Some(format!(
            "{:?} has {} children, then {}",
            a.kind(a_id),
            a_children.len(),
            b_children.len()
        ));
    }
    for (x, y) in a_children.iter().zip(&b_children) {
        let difference = match (x, y) {
            (Child::Text(x), Child::Text(y)) if x == y => None,
            (Child::Node(x), Child::Node(y)) => first_difference(a, *x, b, *y),
            _ => Some(format!("{x:?} became {y:?}")),
        };
        if difference.is_some() {
            return difference;
        }
    }
    None
}

#[derive(Debug)]
enum Child {
    Text(String),
    Node(NodeId),
}

fn merged_children(tree: &Tree, id: NodeId) -> Vec<Child> {
    let mut out: Vec<Child> = Vec::new();
    for &child in tree.children(id) {
        match tree.kind(child) {
            NodeKind::Text(text) => match out.last_mut() {
                Some(Child::Text(previous)) => previous.push_str(text),
                _ if text.is_empty() => {}
                _ => out.push(Child::Text(text.clone())),
            },
            _ => out.push(Child::Node(child)),
        }
    }
    out
}
