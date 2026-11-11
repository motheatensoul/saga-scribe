//! The shorthand view of a tree (step 2 of `docs/DOCUMENT_MODEL.md`).
//!
//! [`project_body`] turns the `<body>` of a [`Tree`] into the segments the
//! patcher works with, rule for rule as the importer's extractor
//! (`importer/tei/extraction.rs`) did on libxml2's tree, and records which
//! tree node each segment comes from. `segments_to_dsl` writes the shorthand
//! from the segments.
//!
//! Two differences from the extractor: a segment's `original_xml` is the
//! node's text as written in the file (the extractor wrote it again, losing
//! the attribute order), and the spans come from the tree.

use super::tree::{NodeId, NodeKind, Tree};
use crate::importer::tei::extraction::join_written_words;
use crate::importer::tei::helpers::{escape_xml_attr, escape_xml_text};
use crate::importer::tei::segments::{Segment, SourceSpans};
use crate::importer::tei::source_map::SourceCursor;
use std::collections::HashMap;

/// The segments of a body, the tree node of each (`None` for a word that is
/// part of a text node, and for whitespace), and where each editable segment
/// is in the body's text (`None` if the tree has no spans).
#[derive(Debug)]
pub struct Projection {
    pub segments: Vec<Segment>,
    pub nodes: Vec<Option<NodeId>>,
    pub spans: Option<SourceSpans>,
}

/// The `<body>` element (the first, in document order).
pub fn find_body(tree: &Tree) -> Option<NodeId> {
    tree.descendants(Tree::DOCUMENT).into_iter().find(|&id| {
        tree.element(id)
            .is_some_and(|e| e.name.split_once(':').map_or(e.name.as_str(), |(_, l)| l) == "body")
    })
}

/// Projects the element `body` (the TEI `<body>`) to segments.
pub fn project_body(tree: &Tree, body: NodeId) -> Projection {
    let mut next = vec![None; tree.capacity()];
    let mut prev = vec![None; tree.capacity()];
    for id in tree.descendants(body) {
        for pair in tree.children(id).windows(2) {
            next[pair[0].index()] = Some(pair[1]);
            prev[pair[1].index()] = Some(pair[0]);
        }
    }
    let body_start = tree.has_spans().then(|| tree.span(body).map(|s| s.start)).flatten();
    let mut projector = Projector {
        tree,
        next,
        prev,
        body_start,
        segments: Vec::new(),
        nodes: Vec::new(),
        spans: Vec::new(),
    };
    projector.process_node(body);
    let spans = projector.body_start.map(|_| projector.spans);
    Projection { segments: projector.segments, nodes: projector.nodes, spans }
}

struct Projector<'a> {
    tree: &'a Tree,
    next: Vec<Option<NodeId>>,
    prev: Vec<Option<NodeId>>,
    /// Where the body starts in the file, if spans are known
    body_start: Option<usize>,
    segments: Vec<Segment>,
    nodes: Vec<Option<NodeId>>,
    spans: Vec<Option<(usize, usize)>>,
}

impl Projector<'_> {
    // ------------------------------------------------------------------
    // The tree, as the extractor read libxml2's
    // ------------------------------------------------------------------

    /// An element's name without its prefix; "" for other nodes.
    fn local(&self, id: NodeId) -> &str {
        match self.tree.kind(id) {
            NodeKind::Element(e) => e.name.split_once(':').map_or(e.name.as_str(), |(_, local)| local),
            _ => "",
        }
    }

    fn is_element(&self, id: NodeId) -> bool {
        matches!(self.tree.kind(id), NodeKind::Element(_))
    }

    fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.tree.children(id).first().copied()
    }

    fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.next[id.index()]
    }

    fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.prev[id.index()]
    }

    /// An attribute by its name without prefix, in any namespace (as
    /// libxml2's `xmlGetProp`).
    fn property(&self, id: NodeId, name: &str) -> Option<String> {
        self.tree.element(id)?.attributes.iter().find_map(|a| {
            let local = a.name.split_once(':').map_or(a.name.as_str(), |(_, l)| l);
            (local == name).then(|| a.value.clone())
        })
    }

    fn attributes(&self, id: NodeId) -> HashMap<String, String> {
        self.tree
            .element(id)
            .map(|e| e.attributes.iter().map(|a| (a.name.clone(), a.value.clone())).collect())
            .unwrap_or_default()
    }

    /// The text of an element and everything in it (entities as their
    /// characters), as libxml2's `get_content`.
    fn text_content(&self, id: NodeId) -> String {
        let mut out = String::new();
        for node in self.tree.descendants(id) {
            match self.tree.kind(node) {
                NodeKind::Text(t) | NodeKind::Cdata(t) => out.push_str(t),
                NodeKind::EntityRef(name) => {
                    if let Some(entity) = crate::entities::bundled().get(name.as_str()) {
                        out.push_str(&entity.text);
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn original_xml(&self, id: NodeId) -> String {
        self.tree.node_xml(id).unwrap_or_default()
    }

    fn relative(&self, span: std::ops::Range<usize>) -> Option<(usize, usize)> {
        let start = self.body_start?;
        Some((span.start - start, span.end - start))
    }

    fn element_span(&self, id: NodeId) -> Option<(usize, usize)> {
        self.relative(self.tree.span(id)?)
    }

    fn push(&mut self, segment: Segment, node: Option<NodeId>, span: Option<(usize, usize)>) {
        self.segments.push(segment);
        self.nodes.push(node);
        self.spans.push(span);
    }

    fn next_id(&self) -> usize {
        self.segments.len()
    }

    // ------------------------------------------------------------------
    // Segments (the extractor's process_node)
    // ------------------------------------------------------------------

    fn process_node(&mut self, id: NodeId) {
        match self.tree.kind(id) {
            NodeKind::Element(_) => {
                let local = self.local(id).to_string();
                match local.as_str() {
                    "w" => {
                        let segment = self.extract_word(id);
                        self.push(segment, Some(id), self.element_span(id));
                    }
                    "pc" => {
                        let segment = self.extract_punctuation(id);
                        self.push(segment, Some(id), self.element_span(id));
                    }
                    "lb" | "pb" | "handShift" => {
                        let id_ = self.next_id();
                        let attributes = self.attributes(id);
                        let segment = match local.as_str() {
                            "lb" => Segment::LineBreak { id: id_, attributes },
                            "pb" => Segment::PageBreak { id: id_, attributes },
                            _ => Segment::HandShift { id: id_, attributes },
                        };
                        self.push(segment, Some(id), self.element_span(id));
                    }
                    "choice" => {
                        let segment = self.extract_choice(id);
                        self.push(segment, Some(id), self.element_span(id));
                    }
                    "head" => {
                        let content = self.dsl_of(id, true);
                        let trimmed = content.trim();
                        if !trimmed.is_empty() {
                            let dsl = format!(".head{{{}}}", trimmed);
                            self.push_inline(id, &dsl);
                        }
                    }
                    "gap" => {
                        let dsl = self.gap_dsl(id, None);
                        self.push_inline(id, &dsl);
                    }
                    "supplied" => {
                        let mut content = String::new();
                        let mut lb = false;
                        if self.has_element_children(id) {
                            self.node_to_dsl(id, &mut content, &mut lb, false);
                            let trimmed = content.trim();
                            if trimmed.is_empty() {
                                self.emit_structural(id);
                            } else {
                                let dsl = format!(".supplied{{{}}}", trimmed);
                                self.push_inline(id, &dsl);
                            }
                        } else {
                            self.node_to_dsl(id, &mut content, &mut lb, true);
                            let trimmed = content.trim();
                            if trimmed.is_empty() {
                                self.emit_structural(id);
                            } else {
                                let dsl = format!("<{}>", trimmed);
                                self.push_inline(id, &dsl);
                            }
                        }
                    }
                    "del" | "add" | "unclear" => {
                        let content = self.dsl_of(id, true);
                        let trimmed = content.trim();
                        if trimmed.is_empty() {
                            self.emit_structural(id);
                        } else {
                            let dsl = match local.as_str() {
                                "del" => format!("-{{{}}}-", trimmed),
                                "add" => format!("+{{{}}}+", trimmed),
                                _ => format!("?{{{}}}?", trimmed),
                            };
                            self.push_inline(id, &dsl);
                        }
                    }
                    "note" => {
                        let content = self.dsl_of(id, true);
                        let trimmed = content.trim();
                        if !trimmed.is_empty() {
                            let dsl = format!("^{{{}}}", trimmed);
                            self.push_inline(id, &dsl);
                        }
                    }
                    "am" => {
                        if self.has_element_children(id) {
                            self.emit_structural(id);
                        } else {
                            let content = self.dsl_of(id, true);
                            let trimmed = content.trim().to_string();
                            if !trimmed.is_empty() {
                                self.push_inline(id, &trimmed);
                            }
                        }
                    }
                    _ => self.emit_structural(id),
                }
            }
            NodeKind::Text(content) => self.process_text(id, content),
            NodeKind::Comment(text) => {
                let segment = Segment::Structural { id: self.next_id(), xml: format!("<!--{}-->", text) };
                self.push(segment, Some(id), None);
            }
            _ => {}
        }
    }

    /// A text node outside words: each run of letters is a word.
    fn process_text(&mut self, id: NodeId, content: &str) {
        let offsets: Option<Vec<usize>> = self.tree.span(id).and_then(|span| {
            self.body_start?;
            let mut cursor = SourceCursor::new(&self.tree.source()[span.clone()]);
            let relative = cursor.text(content)?;
            Some(relative.into_iter().map(|o| o + span.start).collect())
        });
        if content.trim().is_empty() {
            let segment = Segment::Whitespace { id: self.next_id(), content: content.to_string() };
            self.push(segment, None, None);
            return;
        }
        let mut word = String::new();
        let mut space = String::new();
        let mut word_start = 0;
        let chars: Vec<char> = content.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            if c.is_whitespace() {
                if !word.is_empty() {
                    let span = self.between(&offsets, word_start, i);
                    self.push_text_word(std::mem::take(&mut word), span);
                }
                space.push(c);
            } else {
                if !space.is_empty() {
                    let segment = Segment::Whitespace { id: self.next_id(), content: std::mem::take(&mut space) };
                    self.push(segment, None, None);
                }
                if word.is_empty() {
                    word_start = i;
                }
                word.push(c);
            }
        }
        if !word.is_empty() {
            let span = self.between(&offsets, word_start, chars.len());
            self.push_text_word(word, span);
        }
        if !space.is_empty() {
            let segment = Segment::Whitespace { id: self.next_id(), content: space };
            self.push(segment, None, None);
        }
    }

    /// The span from the `from`-th to the `to`-th character of a text node.
    fn between(&self, offsets: &Option<Vec<usize>>, from: usize, to: usize) -> Option<(usize, usize)> {
        let offsets = offsets.as_ref()?;
        self.relative(offsets[from]..offsets[to])
    }

    fn push_text_word(&mut self, word: String, span: Option<(usize, usize)>) {
        let segment = Segment::Word {
            id: self.next_id(),
            original_xml: escape_xml_text(&word),
            attributes: HashMap::new(),
            dsl_content: word,
            has_inline_lb: false,
        };
        self.push(segment, None, span);
    }

    /// An inline element (gap, supplied, del, …) as a word with this shorthand.
    fn push_inline(&mut self, id: NodeId, dsl: &str) {
        let segment = Segment::Word {
            id: self.next_id(),
            original_xml: self.original_xml(id),
            attributes: HashMap::new(),
            dsl_content: dsl.to_string(),
            has_inline_lb: false,
        };
        self.push(segment, Some(id), self.element_span(id));
    }

    fn open_tag(&self, id: NodeId) -> String {
        let element = self.tree.element(id).expect("an element");
        let mut tag = format!("<{}", element.name);
        for attribute in &element.attributes {
            tag.push_str(&format!(" {}=\"{}\"", attribute.name, escape_xml_attr(&attribute.value)));
        }
        tag.push('>');
        tag
    }

    fn emit_structural(&mut self, id: NodeId) {
        let tags = self.tree.tags(id);
        let open_span = tags.as_ref().and_then(|(open, _)| self.relative(open.clone()));
        let xml = self.open_tag(id);
        let open = if self.local(id) == "p" {
            Segment::Paragraph { id: self.next_id(), xml }
        } else {
            Segment::Structural { id: self.next_id(), xml }
        };
        self.push(open, Some(id), open_span);
        for child in self.tree.children(id).to_vec() {
            self.process_node(child);
        }
        let close_span = tags.and_then(|(_, close)| self.relative(close));
        let name = &self.tree.element(id).expect("an element").name;
        let close = Segment::Structural { id: self.next_id(), xml: format!("</{}>", name) };
        self.push(close, Some(id), close_span);
    }

    fn has_element_children(&self, id: NodeId) -> bool {
        self.tree.children(id).iter().any(|&c| self.is_element(c))
    }

    fn find_descendant(&self, id: NodeId, target: &str) -> Option<NodeId> {
        for &child in self.tree.children(id) {
            if self.is_element(child) {
                if self.local(child) == target {
                    return Some(child);
                }
                if let Some(found) = self.find_descendant(child, target) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn gap_dsl(&self, id: NodeId, supplied: Option<&str>) -> String {
        let digits = self
            .property(id, "quantity")
            .map(|value| value.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
            .filter(|value| !value.is_empty());
        let mut dsl = "[...]".to_string();
        if let Some(q) = digits {
            dsl.insert_str(dsl.len() - 1, &q);
        }
        if let Some(text) = supplied {
            if !text.trim().is_empty() {
                dsl.insert_str(dsl.len() - 1, &format!("<{}>", text.trim()));
            }
        }
        dsl
    }

    // ------------------------------------------------------------------
    // Words
    // ------------------------------------------------------------------

    /// The shorthand of a word-like element from its levels: an abbreviation,
    /// a normalized-only form, the first level with text, or its content.
    fn levels_dsl(&self, id: NodeId, lb: &mut bool) -> String {
        let facs_text = self.level_text(id, "facs", lb);
        let dipl_text = self.level_text(id, "dipl", lb);
        let norm_text = self.level_text(id, "norm", lb);
        if facs_text.is_none() && dipl_text.is_none() {
            if let Some(text) = &norm_text {
                return format!(".norm{{{}}}", text);
            }
        }
        if let Some(text) = facs_text.or(dipl_text).or(norm_text) {
            return text;
        }
        let mut dsl = String::new();
        match self.find_descendant(id, "facs") {
            Some(facs) => self.node_to_dsl(facs, &mut dsl, lb, true),
            None => self.node_to_dsl(id, &mut dsl, lb, true),
        }
        dsl
    }

    fn extract_word(&mut self, id: NodeId) -> Segment {
        let mut lb = false;
        let dsl_content = match self.abbr_expansion(id, &mut lb) {
            Some((abbr, expan)) => format!(".abbr[{}]{{{}}}", abbr, expan),
            None => self.levels_dsl(id, &mut lb),
        };
        Segment::Word {
            id: self.next_id(),
            original_xml: self.original_xml(id),
            attributes: self.attributes(id),
            dsl_content: join_written_words(&dsl_content),
            has_inline_lb: lb,
        }
    }

    fn extract_punctuation(&mut self, id: NodeId) -> Segment {
        let mut lb = false;
        let dsl_content = self.levels_dsl(id, &mut lb);
        Segment::Punctuation { id: self.next_id(), original_xml: self.original_xml(id), dsl_content }
    }

    fn extract_choice(&mut self, id: NodeId) -> Segment {
        let mut lb = false;
        let dsl_content = match self.choice_abbr(id) {
            Some((abbr, expan)) => format!(".abbr[{}]{{{}}}", abbr, expan),
            None => match self.abbr_expansion(id, &mut lb) {
                Some((abbr, expan)) => format!(".abbr[{}]{{{}}}", abbr, expan),
                None => {
                    let facs_text = self.level_text(id, "facs", &mut lb);
                    let dipl_text = self.level_text(id, "dipl", &mut lb);
                    let norm_text = self.level_text(id, "norm", &mut lb);
                    if facs_text.is_none() && dipl_text.is_none() && norm_text.is_some() {
                        format!(".norm{{{}}}", norm_text.unwrap_or_default())
                    } else if let Some(text) = facs_text.or(dipl_text).or(norm_text) {
                        text
                    } else {
                        // Only direct `facs` children count here
                        let facs = self
                            .tree
                            .children(id)
                            .iter()
                            .copied()
                            .rfind(|&c| self.is_element(c) && self.local(c) == "facs");
                        let mut dsl = String::new();
                        self.node_to_dsl(facs.unwrap_or(id), &mut dsl, &mut lb, true);
                        dsl
                    }
                }
            },
        };
        Segment::Word {
            id: self.next_id(),
            original_xml: self.original_xml(id),
            attributes: self.attributes(id),
            dsl_content,
            has_inline_lb: lb,
        }
    }

    /// `<choice><abbr>…</abbr><expan>…</expan></choice>`: the shorthand of
    /// both (each from all such children, in order).
    fn choice_abbr(&self, id: NodeId) -> Option<(String, String)> {
        let (mut abbr, mut expan) = (String::new(), String::new());
        let (mut found_abbr, mut found_expan) = (false, false);
        let mut lb = false;
        for &child in self.tree.children(id) {
            match self.local(child) {
                "abbr" => {
                    found_abbr = true;
                    self.node_to_dsl(child, &mut abbr, &mut lb, true);
                }
                "expan" => {
                    found_expan = true;
                    self.node_to_dsl(child, &mut expan, &mut lb, true);
                }
                _ => {}
            }
        }
        (found_abbr && found_expan).then_some((abbr, expan))
    }

    fn level_text(&self, id: NodeId, level: &str, lb: &mut bool) -> Option<String> {
        let child = self.find_descendant(id, level)?;
        let mut text = String::new();
        self.node_to_dsl(child, &mut text, lb, true);
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    /// MENOTA's `<am>` in facs or `<ex>` in dipl (or Saga-Scribe's
    /// `<abbr>`/`<expan>`): the facs and dipl shorthand.
    fn abbr_expansion(&self, id: NodeId, lb: &mut bool) -> Option<(String, String)> {
        let facs = self.find_descendant(id, "facs")?;
        let dipl = self.find_descendant(id, "dipl")?;
        let has_am = self.find_descendant(facs, "am").is_some() || self.find_descendant(facs, "abbr").is_some();
        let has_ex = self.find_descendant(dipl, "ex").is_some() || self.find_descendant(dipl, "expan").is_some();
        if !has_am && !has_ex {
            return None;
        }
        let mut abbr = String::new();
        self.node_to_dsl(facs, &mut abbr, lb, true);
        let mut expan = String::new();
        self.node_to_dsl(dipl, &mut expan, lb, true);
        let (abbr, expan) = (abbr.trim(), expan.trim());
        (!abbr.is_empty() && !expan.is_empty()).then(|| (abbr.to_string(), expan.to_string()))
    }

    fn dsl_of(&self, id: NodeId, allow_norm: bool) -> String {
        let mut out = String::new();
        let mut lb = false;
        self.node_to_dsl(id, &mut out, &mut lb, allow_norm);
        out
    }

    /// A text node's contribution: its words with single spaces, and a space
    /// before or after where it had whitespace there.
    fn push_text(&self, id: NodeId, content: &str, out: &mut String) {
        let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
        if content.starts_with(char::is_whitespace) && !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push_str(&normalized);
        if content.ends_with(char::is_whitespace) && self.next_sibling(id).is_some() && !out.ends_with(' ') {
            out.push(' ');
        }
    }

    /// The children of `id`, each with [`node_to_dsl`](Self::node_to_dsl)
    /// (which converts a child's own children), text directly.
    fn children_to_dsl(&self, id: NodeId, out: &mut String, lb: &mut bool, allow_norm: bool) {
        for &child in self.tree.children(id) {
            match self.tree.kind(child) {
                NodeKind::Text(content) => {
                    if content.split_whitespace().next().is_some() {
                        self.push_text(child, content, out);
                    }
                }
                _ => self.node_to_dsl(child, out, lb, allow_norm),
            }
        }
    }

    /// The shorthand of the children of `id`.
    fn node_to_dsl(&self, id: NodeId, out: &mut String, lb: &mut bool, allow_norm: bool) {
        let mut child = self.first_child(id);
        while let Some(c) = child {
            let mut next_child = self.next_sibling(c);
            match self.tree.kind(c) {
                NodeKind::Text(content) => {
                    if content.split_whitespace().next().is_none() {
                        // Pure whitespace is indentation, except between an
                        // entity and more text (`&et; &thorn;a`): a word space
                        let after_entity =
                            self.prev_sibling(c).is_some_and(|p| matches!(self.tree.kind(p), NodeKind::EntityRef(_)));
                        let before_text = next_child.is_some_and(|n| {
                            matches!(self.tree.kind(n), NodeKind::EntityRef(_) | NodeKind::Text(_))
                        });
                        if after_entity && before_text && !out.ends_with(' ') {
                            out.push(' ');
                        }
                    } else {
                        self.push_text(c, content, out);
                    }
                }
                NodeKind::EntityRef(name) => {
                    out.push(':');
                    out.push_str(name);
                    out.push(':');
                }
                NodeKind::Element(_) => match self.local(c) {
                    "choice" => self.choice_to_dsl(c, out, lb, allow_norm),
                    "am" => self.node_to_dsl(c, out, lb, allow_norm),
                    "w" => {
                        if !out.is_empty() && !out.ends_with(' ') {
                            out.push(' ');
                        }
                        match self.abbr_expansion(c, lb) {
                            Some((abbr, expan)) => out.push_str(&format!(".abbr[{}]{{{}}}", abbr, expan)),
                            None => self.word_levels_to_dsl(c, out, lb, allow_norm),
                        }
                    }
                    "pc" => self.word_levels_to_dsl(c, out, lb, allow_norm),
                    "c" => {
                        let mut inner = String::new();
                        self.node_to_dsl(c, &mut inner, lb, allow_norm);
                        if inner.is_empty() {
                            out.push_str(&self.text_content(c));
                        } else {
                            out.push_str(&inner);
                        }
                    }
                    name @ ("add" | "del" | "unclear" | "note") => {
                        let mut inner = String::new();
                        self.node_to_dsl(c, &mut inner, lb, allow_norm);
                        let trimmed = inner.trim();
                        if !trimmed.is_empty() {
                            let (open, close) = match name {
                                "add" => ("+{", "}+"),
                                "del" => ("-{", "}-"),
                                "unclear" => ("?{", "}?"),
                                _ => ("^{", "}"),
                            };
                            out.push_str(open);
                            out.push_str(trimmed);
                            out.push_str(close);
                        }
                    }
                    "supplied" => {
                        let has_children = self.has_element_children(c);
                        let mut inner = String::new();
                        if has_children {
                            self.node_to_dsl(c, &mut inner, lb, false);
                        } else {
                            self.children_to_dsl(c, &mut inner, lb, allow_norm);
                        }
                        let trimmed = inner.trim();
                        if !trimmed.is_empty() {
                            if has_children {
                                out.push_str(&format!(".supplied{{{}}}", trimmed));
                            } else {
                                out.push('<');
                                out.push_str(trimmed);
                                out.push('>');
                            }
                        }
                    }
                    "gap" => {
                        let mut supplied = None;
                        if let Some(next) = self.next_sibling(c) {
                            if self.local(next) == "supplied" {
                                let mut inner = String::new();
                                self.node_to_dsl(next, &mut inner, lb, allow_norm);
                                let trimmed = inner.trim();
                                if !trimmed.is_empty() {
                                    supplied = Some(trimmed.to_string());
                                    next_child = self.next_sibling(next);
                                }
                            }
                        }
                        out.push_str(&self.gap_dsl(c, supplied.as_deref()));
                    }
                    "lb" => {
                        *lb = true;
                        out.push_str("~//");
                        if let Some(n) = self.property(c, "n") {
                            out.push_str(&n);
                        }
                    }
                    "dipl" | "norm" => {}
                    _ => self.node_to_dsl(c, out, lb, allow_norm),
                },
                _ => {}
            }
            child = next_child;
        }
    }

    /// A `<w>` or `<pc>` inside other content, from its levels.
    fn word_levels_to_dsl(&self, id: NodeId, out: &mut String, lb: &mut bool, allow_norm: bool) {
        let facs_text = self.level_text(id, "facs", lb);
        let dipl_text = self.level_text(id, "dipl", lb);
        let norm_text = self.level_text(id, "norm", lb);
        if facs_text.is_none() && dipl_text.is_none() {
            if let Some(text) = &norm_text {
                if allow_norm {
                    out.push_str(&format!(".norm{{{}}}", text));
                } else {
                    out.push_str(text);
                }
                return;
            }
        }
        match facs_text.or(dipl_text).or(norm_text) {
            Some(text) => out.push_str(&text),
            None => self.children_to_dsl(id, out, lb, allow_norm),
        }
    }

    /// A `<choice>` inside other content.
    fn choice_to_dsl(&self, id: NodeId, out: &mut String, lb: &mut bool, allow_norm: bool) {
        let (mut abbr, mut expan) = (String::new(), String::new());
        let (mut found_abbr, mut found_expan) = (false, false);
        let mut facs = None;
        for &child in self.tree.children(id) {
            match self.local(child) {
                "abbr" => {
                    found_abbr = true;
                    self.node_to_dsl(child, &mut abbr, lb, allow_norm);
                }
                "expan" => {
                    found_expan = true;
                    self.node_to_dsl(child, &mut expan, lb, allow_norm);
                }
                "facs" => facs = Some(child),
                _ => {}
            }
        }
        if found_abbr && found_expan {
            out.push_str(&format!(".abbr[{}]{{{}}}", abbr, expan));
            return;
        }
        if let Some((abbr, expan)) = self.abbr_expansion(id, lb) {
            out.push_str(&format!(".abbr[{}]{{{}}}", abbr, expan));
            return;
        }
        let facs_text = self.level_text(id, "facs", lb);
        let dipl_text = self.level_text(id, "dipl", lb);
        let norm_text = self.level_text(id, "norm", lb);
        if facs_text.is_none() && dipl_text.is_none() {
            if let Some(text) = &norm_text {
                if allow_norm {
                    out.push_str(&format!(".norm{{{}}}", text));
                } else {
                    out.push_str(text);
                }
                return;
            }
        }
        if let Some(text) = facs_text.or(dipl_text).or(norm_text) {
            out.push_str(&text);
            return;
        }
        match facs {
            Some(facs) => self.node_to_dsl(facs, out, lb, allow_norm),
            None => {
                for &child in self.tree.children(id) {
                    if matches!(self.local(child), "dipl" | "norm") {
                        continue;
                    }
                    self.node_to_dsl(child, out, lb, allow_norm);
                }
            }
        }
    }
}
