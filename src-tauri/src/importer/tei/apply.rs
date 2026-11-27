//! # Edits as operations on the tree
//!
//! Step 3b of `docs/DOCUMENT_MODEL.md`: the patcher's operations (keep,
//! modify, delete, insert a segment) are applied to the document's tree
//! rather than spliced into its text. The tree's writer then copies every
//! node the edits didn't touch from the original text, so the file changes
//! only where it was edited, and structure is changed as structure:
//!
//! - an edited word is replaced by the word with the edit merged in
//!   (`document/merge.rs`), a changed break number is set on the break;
//! - a deleted word goes with the elements it leaves empty
//!   (`<num><w>…</w></num>`), short of a block such as `<p>`;
//! - new words go after the token before them, in the same element;
//! - `.p` added splits its paragraph (the rest of it moves to a new `<p>`),
//!   `.p` removed merges the paragraph into the one before;
//! - markup around words (`.q{` … `}`) added wraps what is between its start
//!   and end in a new element, removed puts the element's content in its
//!   place, renamed (`.q{` to `.seg{`) puts its content in an element of the
//!   new name, with its attributes.
//!
//! Text outside words (a body without `<w>`) is split into text nodes where
//! a word in it is edited.

use super::patching::{
    inserted_xml, keeps_word_data, markup_segment, markup_token, render_insert, render_modify, token_kind, MarkupPart,
    Origin, Originals, ParagraphStructure, PatchOperation, RenderedBody, TokenKind,
};
use super::segments::Segment;
use crate::document::{find_body, Element, NodeId, NodeKind, Piece, Tree};
use crate::identity::PARAGRAPH;
use crate::parser::Compiler;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

/// Where a segment is in the tree.
#[derive(Debug, Clone)]
enum Place {
    /// The element (a word, a break, an inline element)
    Node(NodeId),
    /// The start tag of this element (a paragraph or other structure)
    Open(NodeId),
    /// The end tag of this element
    Close(NodeId),
    /// Characters `chars` of a text node (a word outside `<w>`)
    Text { node: NodeId, chars: Range<usize> },
}

/// An imported document as a tree, with the place of each segment.
#[derive(Debug)]
pub struct TreeSource {
    tree: Tree,
    body_start: usize,
    postamble_len: usize,
    /// By segment ID
    places: Vec<Option<Place>>,
    /// Where each segment is in the file's text, by segment ID
    spans: Vec<Option<(u32, u32)>>,
    /// Where each `<w>` of the body starts in the file, in order
    words: Vec<usize>,
    /// Words whose XML was set in the word panel: their XML, by segment,
    /// and their segment, by their new node (which has no span)
    replaced: HashMap<usize, String>,
    replaced_nodes: HashMap<NodeId, usize>,
    /// The tree as the last compile wrote it (the edits applied), if they
    /// changed it: where the word panel finds elements
    current: Option<Tree>,
    /// Attributes were set in the word panel: the body is no longer the file's
    attributes_set: bool,
}

/// An element of the file as the word panel shows it: found again by its
/// name and how many elements of that name come before it in the body.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementInfo {
    pub name: String,
    pub ordinal: usize,
    /// "around" (the word is in it), "inside" (in the word), "before"
    /// (between the word and the token before it: a break, a note)
    pub place: &'static str,
    pub attributes: Vec<(String, String)>,
    /// Attributes the shorthand writes (a break's `n`): set there
    pub fixed: Vec<String>,
}

/// Attributes of an element the shorthand writes, which the word panel
/// doesn't set.
fn fixed_attributes(name: &str) -> &'static [&'static str] {
    match local(name) {
        "lb" | "pb" | "cb" => &["n"],
        "rdg" => &["wit"],
        _ => &[],
    }
}

impl TreeSource {
    /// Parses the file (`preamble`, `body`, `postamble`) and finds each
    /// segment by its span (relative to the body).
    ///
    /// # Errors
    ///
    /// If the file has no spans, or a segment isn't where its span says.
    /// segment by its span (relative to the body). `None` if the file has
    /// no spans or a segment with a span isn't where it says.
    pub fn new(preamble: &str, body: &str, postamble: &str, segments: &[Segment], spans: &[Option<(usize, usize)>]) -> Result<Self, String> {
        let tree = Tree::parse(&format!("{preamble}{body}{postamble}"))?;
        Self::on(tree, preamble.len(), postamble.len(), segments, spans)
    }

    /// [`new`](Self::new) with the file already parsed (`tree`, whose body
    /// starts at `preamble_len`).
    pub fn on(tree: Tree, preamble_len: usize, postamble_len: usize, segments: &[Segment], spans: &[Option<(usize, usize)>]) -> Result<Self, String> {
        let source = tree.source().to_string();
        if !tree.has_spans() {
            return Err("the file's text and tree don't line up".into());
        }
        let body_node = find_body(&tree).ok_or("no <body>")?;
        let body_start = tree.span(body_node).ok_or("no <body> span")?.start;
        if body_start != preamble_len {
            return Err("the body isn't where the preamble ends".into());
        }

        let mut elements = HashMap::new();
        let mut opens = HashMap::new();
        let mut closes = HashMap::new();
        let mut texts = Vec::new();
        let mut words = Vec::new();
        for id in tree.descendants(body_node) {
            let Some(span) = tree.span(id) else { continue };
            match tree.kind(id) {
                NodeKind::Element(element) => {
                    if local(&element.name) == "w" {
                        words.push(span.start);
                    }
                    elements.insert((span.start, span.end), id);
                    if let Some((open, close)) = tree.tags(id) {
                        opens.insert((open.start, open.end), id);
                        // (empty for `<x/>`, at its end)
                        closes.insert((close.start, close.end), id);
                    }
                }
                NodeKind::Text(_) => texts.push((span.start, span.end, id)),
                _ => {}
            }
        }
        texts.sort_unstable();

        let mut places = vec![None; segments.len()];
        let mut located = vec![None; segments.len()];
        for segment in segments {
            let id = segment.id();
            let Some(&Some((start, end))) = spans.get(id) else { continue };
            let key = (start + body_start, end + body_start);
            let missing = || format!("segment {id} ({segment:?}) isn't at {}..{}", key.0, key.1);
            let place = match segment {
                Segment::Whitespace { .. } => continue,
                Segment::Paragraph { .. } => Place::Open(*opens.get(&key).ok_or_else(missing)?),
                Segment::Structural { xml, .. } if xml.starts_with("</") => Place::Close(*closes.get(&key).ok_or_else(missing)?),
                Segment::Structural { .. } => Place::Open(*opens.get(&key).ok_or_else(missing)?),
                _ => match elements.get(&key) {
                    Some(&node) => Place::Node(node),
                    None => (|| {
                        let i = texts.partition_point(|&(s, _, _)| s <= key.0).checked_sub(1)?;
                        let (text_start, text_end, node) = texts[i];
                        if key.1 > text_end {
                            return None;
                        }
                        let NodeKind::Text(content) = tree.kind(node) else { return None };
                        let mut cursor = super::source_map::SourceCursor::new(&source[text_start..text_end]);
                        let offsets = cursor.text(content)?;
                        let from = offsets.iter().position(|&o| o + text_start == key.0)?;
                        let to = offsets.iter().position(|&o| o + text_start == key.1)?;
                        Some(Place::Text { node, chars: from..to })
                    })()
                    .ok_or_else(missing)?,
                },
            };
            *places.get_mut(id).ok_or("segment IDs out of order")? = Some(place);
            let offset = |o: usize| u32::try_from(o).map_err(|_| "the file is too large".to_string());
            located[id] = Some((offset(key.0)?, offset(key.1)?));
        }
        Ok(Self {
            tree,
            body_start,
            postamble_len,
            places,
            spans: located,
            words,
            replaced: HashMap::new(),
            replaced_nodes: HashMap::new(),
            current: None,
            attributes_set: false,
        })
    }

    /// Segment `id` as written in the file (a word, punctuation, a tag), or
    /// as set since ([`replace_word`](Self::replace_word)).
    pub fn segment_xml(&self, id: usize) -> Option<&str> {
        if let Some(xml) = self.replaced.get(&id) {
            return Some(xml);
        }
        let (start, end) = (*self.spans.get(id)?)?;
        self.tree.source().get(start as usize..end as usize)
    }

    /// The attributes of the element segment `id` is, as read from the file.
    pub fn segment_attributes(&self, id: usize) -> Option<HashMap<String, String>> {
        let element = self.segment_element(id)?;
        Some(element.attributes.iter().map(|a| (a.name.clone(), a.value.clone())).collect())
    }

    /// Where each `<w>` element of the body starts, relative to the body:
    /// the words as the previews count them.
    pub fn word_offsets(&self) -> Vec<usize> {
        self.words.iter().map(|&w| w - self.body_start).collect()
    }

    /// Whether a word's XML was set since the file was read: the body is
    /// then no longer the file's.
    pub fn is_changed(&self) -> bool {
        !self.replaced.is_empty() || self.attributes_set
    }

    /// Keeps the tree a compile wrote (`None`: the file's, unedited).
    pub fn set_current(&mut self, tree: Option<Tree>) {
        self.current = tree;
    }

    /// The tree as the last compile wrote it.
    fn current(&self) -> &Tree {
        self.current.as_ref().unwrap_or(&self.tree)
    }

    /// The elements word `word` (as the previews count) is in, has in it,
    /// and has before it, with their attributes, as the last compile wrote
    /// them.
    pub fn word_elements(&self, word: usize) -> Result<Vec<ElementInfo>, String> {
        let tree = self.current();
        let body = find_body(tree).ok_or("no <body>")?;
        let order = tree.descendants(body);
        let w = order
            .iter()
            .copied()
            .filter(|&n| tree.element(n).is_some_and(|e| local(&e.name) == "w"))
            .nth(word)
            .ok_or("There is no such word")?;
        let is_token = |n: NodeId| tree.element(n).is_some_and(|e| matches!(local(&e.name), "w" | "pc" | "num"));
        let has_token = |n: NodeId| tree.descendants(n).into_iter().any(is_token);
        let mut wanted: Vec<(NodeId, &'static str)> = Vec::new();
        // Before it: breaks, notes and the like back to the token before
        let mut node = w;
        'back: loop {
            let parent = tree.parent(node).ok_or("no parent")?;
            let index = tree.index_in_parent(node).ok_or("not in its parent")?;
            let mut first = true;
            for &sibling in tree.children(parent)[..index].iter().rev() {
                if is_whitespace(tree, sibling) || tree.element(sibling).is_none() {
                    continue;
                }
                first = false;
                if is_token(sibling) || has_token(sibling) {
                    break 'back;
                }
                wanted.push((sibling, "before"));
            }
            let stop = parent == body || tree.element(parent).is_some_and(|e| BLOCKS.contains(&local(&e.name)));
            if !first || stop {
                break;
            }
            node = parent;
        }
        wanted.reverse();
        let mut ancestors = Vec::new();
        let mut current = tree.parent(w);
        while let Some(parent) = current.filter(|&p| p != body) {
            ancestors.push((parent, "around"));
            current = tree.parent(parent);
        }
        ancestors.reverse();
        let mut all = ancestors;
        all.extend(wanted);
        for node in tree.descendants(w).into_iter().skip(1) {
            let Some(e) = tree.element(node) else { continue };
            let levels = |n: NodeId| tree.children(n).iter().any(|&c| tree.element(c).is_some_and(|e| matches!(local(&e.name), "facs" | "dipl" | "norm")));
            if matches!(local(&e.name), "facs" | "dipl" | "norm") || (local(&e.name) == "choice" && levels(node)) {
                continue;
            }
            all.push((node, "inside"));
        }
        // Ordinals: elements of the same name before each in the body
        let mut counts: HashMap<&str, usize> = HashMap::new();
        let mut ordinal: HashMap<NodeId, usize> = HashMap::new();
        let looked_for: HashSet<NodeId> = all.iter().map(|(n, _)| *n).collect();
        for &n in &order {
            let Some(e) = tree.element(n) else { continue };
            let count = counts.entry(e.name.as_str()).or_default();
            if looked_for.contains(&n) {
                ordinal.insert(n, *count);
            }
            *count += 1;
        }
        Ok(all
            .into_iter()
            .filter_map(|(n, place)| {
                let e = tree.element(n)?;
                Some(ElementInfo {
                    name: e.name.clone(),
                    ordinal: *ordinal.get(&n)?,
                    place,
                    attributes: e.attributes.iter().map(|a| (a.name.clone(), a.value.clone())).collect(),
                    fixed: fixed_attributes(&e.name).iter().map(|s| s.to_string()).collect(),
                })
            })
            .collect())
    }

    /// Sets the attributes of the `ordinal`-th element named `name` in the
    /// body (as the last compile wrote it) to `attributes`, in the file. The
    /// element must be in the file, not added in the shorthand since it was
    /// opened, and the attributes the shorthand writes stay as they are.
    pub fn set_element_attributes(&mut self, name: &str, ordinal: usize, attributes: &[(String, String)]) -> Result<(), String> {
        let tree = self.current();
        let body = find_body(tree).ok_or("no <body>")?;
        let node = tree
            .descendants(body)
            .into_iter()
            .filter(|&n| tree.element(n).is_some_and(|e| e.name == name))
            .nth(ordinal)
            .ok_or_else(|| format!("There is no such <{name}>"))?;
        if !Tree::is_older(node, self.tree.node_count()) || self.tree.element(node).is_none_or(|e| e.name != name) {
            return Err("This element was added in the shorthand; its attributes can be set once the project is saved and opened again".into());
        }
        let before = self.tree.element(node).map(|e| e.attributes.clone()).unwrap_or_default();
        let valid = |n: &str| {
            let mut chars = n.chars();
            chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
                && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
        };
        if let Some((bad, _)) = attributes.iter().find(|(n, _)| !valid(n)) {
            return Err(format!("\"{bad}\" isn't an attribute name"));
        }
        for fixed in fixed_attributes(name) {
            let was = before.iter().find(|a| a.name == *fixed).map(|a| a.value.as_str());
            let now = attributes.iter().find(|(n, _)| n == fixed).map(|(_, v)| v.as_str());
            if was != now {
                return Err(format!("A <{name}>'s {fixed} is set in the shorthand"));
            }
        }
        let names: HashSet<&str> = attributes.iter().map(|(n, _)| n.as_str()).collect();
        for tree in std::iter::once(&mut self.tree).chain(self.current.as_mut()) {
            for a in before.iter().filter(|a| !names.contains(a.name.as_str())) {
                tree.set_attribute(node, &a.name, None)?;
            }
            for (n, v) in attributes {
                tree.set_attribute(node, n, Some(v))?;
            }
        }
        self.attributes_set = true;
        Ok(())
    }

    /// Sets the XML of the word that is segment `id` (an element, such as a
    /// `<w>`) to `xml`, a single element of the same name. The word is then
    /// written from the tree; everything else is still copied.
    pub fn replace_word(&mut self, id: usize, xml: &str) -> Result<(), String> {
        let Some(Place::Node(old)) = self.places.get(id).cloned().flatten() else {
            return Err("This word isn't an element of the file".into());
        };
        let old_name = self.tree.element(old).map(|e| e.name.clone()).ok_or("not an element")?;
        let (fragment, root) = parse_fragment(xml.trim()).ok_or("The XML is not well-formed")?;
        let elements: Vec<NodeId> = fragment.children(root).iter().copied().filter(|&c| !is_whitespace(&fragment, c)).collect();
        let [new] = elements.as_slice() else { return Err("The XML must be a single element".into()) };
        match fragment.element(*new) {
            Some(e) if local(&e.name) == local(&old_name) => {}
            _ => return Err(format!("The XML must be a single <{old_name}> element")),
        }
        let parent = self.tree.parent(old).ok_or("The word has no parent")?;
        let index = self.tree.index_in_parent(old).ok_or("The word isn't in its parent")?;
        let grafted = self.tree.graft(&fragment, *new, parent, index);
        self.tree.remove(old);
        self.places[id] = Some(Place::Node(grafted));
        self.replaced_nodes.insert(grafted, id);
        self.replaced.insert(id, self.tree.node_xml(grafted)?);
        Ok(())
    }

    /// The file's tree.
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// The segment that is the element `node`, if any.
    pub fn segment_of(&self, node: NodeId) -> Option<usize> {
        self.places.iter().position(|place| matches!(place, Some(Place::Node(n)) if *n == node))
    }

    /// The element segment `id` is (a word, punctuation, a break), as read
    /// from the file.
    pub fn segment_element(&self, id: usize) -> Option<&Element> {
        match self.places.get(id)?.as_ref()? {
            Place::Node(node) => self.tree.element(*node),
            _ => None,
        }
    }
}

/// Where to insert: after a node, at the start of an element, or after a
/// segment (found when needed, so text isn't split for nothing).
#[derive(Debug, Clone, Copy)]
enum Anchor {
    After(NodeId),
    Start(NodeId),
    Segment(usize),
}

/// Elements a paragraph can be started in (`.p` outside any paragraph).
const CONTAINERS: &[&str] = &["body", "div", "front", "back"];

/// Elements that a deleted word's emptied wrapper never is.
const BLOCKS: &[&str] = &["p", "div", "body", "front", "back", "text", "head", "lg", "l", "ab", "list", "item", "table", "row", "cell"];

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn is_whitespace(tree: &Tree, id: NodeId) -> bool {
    matches!(tree.kind(id), NodeKind::Text(text) if text.trim().is_empty())
}

/// The edits being applied to a copy of the tree.
struct Applier<'a> {
    source: &'a TreeSource,
    tree: Tree,
    /// The pieces of each text node split so far: (chars of the original, node)
    pieces: HashMap<NodeId, Vec<(Range<usize>, NodeId)>>,
    /// New nodes, by the edited token they come from
    created: HashMap<NodeId, Option<usize>>,
    /// Words edited in place, whose lemma and analysis were kept
    edited: Vec<NodeId>,
    /// Edited words with levels that couldn't follow the edit, by segment
    stale: Vec<(usize, Vec<String>)>,
    /// Markup the shorthand shows: never removed with the words in it
    shown: HashSet<NodeId>,
    /// Markup added, still open: what follows goes into it
    wraps: Vec<NodeId>,
    /// Markup open in the edited shorthand, innermost last: kept or added
    open: Vec<NodeId>,
    /// Kept markup whose end has passed: nothing after it may be in it
    closed: HashSet<NodeId>,
    /// A paragraph started outside any (`.p` in a new document's body):
    /// what follows goes into it, up to the next paragraph or structure
    paragraph: Option<NodeId>,
    /// Markup renamed: the element now in its place
    renamed: HashMap<NodeId, NodeId>,
}

impl Applier<'_> {
    /// The node of segment `id`, splitting text where it is part of one.
    fn node(&mut self, id: usize) -> Option<NodeId> {
        match self.source.places.get(id)?.clone()? {
            Place::Node(node) | Place::Open(node) | Place::Close(node) => Some(*self.renamed.get(&node).unwrap_or(&node)),
            Place::Text { node, chars } => self.split_text(node, chars),
        }
    }

    fn split_text(&mut self, node: NodeId, chars: Range<usize>) -> Option<NodeId> {
        let NodeKind::Text(content) = self.source.tree.kind(node) else { return None };
        let length = content.chars().count();
        let pieces = self.pieces.entry(node).or_insert_with(|| vec![(0..length, node)]);
        let k = pieces.iter().position(|(r, _)| r.start <= chars.start && chars.end <= r.end)?;
        let (range, piece) = pieces[k].clone();
        if range == chars {
            return Some(piece);
        }
        let NodeKind::Text(text) = self.tree.kind(piece).clone() else { return None };
        let part = |r: Range<usize>| -> String { text.chars().skip(r.start - range.start).take(r.len()).collect() };
        let (before, middle, after) = (range.start..chars.start, chars.clone(), chars.end..range.end);
        self.tree.set_text(piece, &part(middle.clone())).ok()?;
        let parent = self.tree.parent(piece)?;
        let mut replacement = Vec::new();
        if !before.is_empty() {
            let at = self.tree.index_in_parent(piece)?;
            let node = self.tree.insert(parent, at, NodeKind::Text(part(before.clone())));
            replacement.push((before, node));
        }
        replacement.push((middle, piece));
        if !after.is_empty() {
            let at = self.tree.index_in_parent(piece)? + 1;
            let node = self.tree.insert(parent, at, NodeKind::Text(part(after.clone())));
            replacement.push((after, node));
        }
        let pieces = self.pieces.get_mut(&node)?;
        pieces.splice(k..=k, replacement);
        Some(piece)
    }

    /// Where `anchor` is, as (parent, child index) to insert at.
    fn position(&mut self, anchor: Anchor) -> Option<(NodeId, usize)> {
        match anchor {
            Anchor::After(node) => Some((self.tree.parent(node)?, self.tree.index_in_parent(node)? + 1)),
            Anchor::Start(element) => Some((element, 0)),
            // After a paragraph's start tag is inside it
            Anchor::Segment(id) => match self.source.places.get(id)?.clone()? {
                Place::Open(element) => Some((*self.renamed.get(&element).unwrap_or(&element), 0)),
                _ => {
                    let node = self.node(id)?;
                    self.position(Anchor::After(node))
                }
            },
        }
    }

    /// The anchor for what was at `index` of `parent` before it was removed.
    fn anchor_at(&self, parent: NodeId, index: usize) -> Anchor {
        match index.checked_sub(1).and_then(|i| self.tree.children(parent).get(i)) {
            Some(&before) => Anchor::After(before),
            None => Anchor::Start(parent),
        }
    }

    /// Inserts compiled XML at `index` of `parent`; returns its top nodes.
    fn insert_xml(&mut self, xml: &str, parent: NodeId, index: usize, token: Option<usize>) -> Option<Vec<NodeId>> {
        let (fragment, root) = parse_fragment(xml.trim_end())?;
        let mut roots = Vec::new();
        for (i, &child) in fragment.children(root).iter().enumerate() {
            let copy = self.tree.graft(&fragment, child, parent, index + i);
            for node in self.tree.descendants(copy) {
                self.created.insert(node, token);
            }
            roots.push(copy);
        }
        Some(roots)
    }

    /// A moved word counts as edited: it keeps its lemma and analysis.
    fn mark_moved(&mut self, roots: &[NodeId], moved: Option<usize>) {
        if moved.is_some() {
            let word = roots.iter().copied().find(|&r| self.tree.element(r).is_some());
            self.edited.extend(word);
        }
    }

    fn insert_text(&mut self, text: &str, parent: NodeId, index: usize) -> NodeId {
        let node = self.tree.insert(parent, index, NodeKind::Text(text.to_string()));
        self.created.insert(node, None);
        node
    }

    /// Removes `node` with the elements it leaves empty; returns where it was.
    fn delete(&mut self, node: NodeId) -> Option<Anchor> {
        let mut gone = node;
        while let Some(parent) = self.tree.parent(gone) {
            let kept = self.shown.contains(&parent) || self.wraps.contains(&parent);
            let wrapper = !kept && self.tree.element(parent).is_some_and(|e| !BLOCKS.contains(&local(&e.name)));
            let alone = self.tree.children(parent).iter().all(|&c| c == gone || is_whitespace(&self.tree, c));
            if !(wrapper && alone) {
                break;
            }
            gone = parent;
        }
        if gone != node {
            log::info!("Deleting a word removes the elements it left empty");
        }
        let parent = self.tree.parent(gone)?;
        let index = self.tree.index_in_parent(gone)?;
        self.tree.remove(gone);
        Some(self.anchor_at(parent, index))
    }
}

impl Applier<'_> {
    /// The end of the innermost markup open in the shorthand (whichever end
    /// token it is: they are alike). Added markup takes what was written
    /// since its start; kept markup must end here in the file too. Returns
    /// the anchor after it.
    fn close(&mut self, anchor: Anchor) -> Option<Anchor> {
        let element = self.open.pop()?;
        if self.wraps.last() == Some(&element) {
            let last = self.anchor_node(anchor)?;
            self.absorb(last)?;
            self.wraps.pop();
        } else {
            let last = self.anchor_node(anchor)?;
            if last != element && !self.is_inside(last, element) {
                return None;
            }
            self.closed.insert(element);
        }
        if !self.wraps.is_empty() {
            self.absorb(element)?;
        }
        Some(Anchor::After(element))
    }

    /// Moves what was written since markup was added (up to `last`) into
    /// it. `None` if `last` isn't after it in the same element.
    fn absorb(&mut self, last: NodeId) -> Option<()> {
        // Into a paragraph started outside any, while it can take it
        if let Some(paragraph) = self.paragraph {
            if last != paragraph && !self.is_inside(last, paragraph) && self.absorb_into(paragraph, last).is_none() {
                self.paragraph = None;
            }
        }
        // Outermost first: what goes into nested markup goes into its outer
        for i in 0..self.wraps.len() {
            let wrap = self.wraps[i];
            if last != wrap && !self.is_inside(last, wrap) {
                self.absorb_into(wrap, last)?;
            }
        }
        Some(())
    }

    fn absorb_into(&mut self, wrap: NodeId, last: NodeId) -> Option<()> {
        let parent = self.tree.parent(wrap)?;
        if self.tree.parent(last) != Some(parent) {
            return None;
        }
        // Space before the first thing in it stays outside (`a <q><w>b`)
        if self.tree.children(wrap).is_empty() {
            let at = self.tree.index_in_parent(wrap)?;
            while let Some(&next) = self.tree.children(parent).get(at + 1) {
                if next == last || !is_whitespace(&self.tree, next) {
                    break;
                }
                self.tree.move_to(next, parent, at);
            }
        }
        let (from, to) = (self.tree.index_in_parent(wrap)? + 1, self.tree.index_in_parent(last)?);
        if to < from {
            return None;
        }
        let moving: Vec<NodeId> = self.tree.children(parent)[from..=to].to_vec();
        for node in moving {
            let at = self.tree.children(wrap).len();
            self.tree.move_to(node, wrap, at);
        }
        Some(())
    }

    /// The number of the last numbered line break before `lb` (in the body,
    /// in document order), on its page: none after a page break.
    fn line_number_before(&self, lb: NodeId) -> Option<u64> {
        let body = find_body(&self.tree)?;
        let mut last = None;
        for node in self.tree.descendants(body) {
            if node == lb {
                return last;
            }
            match self.tree.element(node).map(|e| local(&e.name)) {
                Some("lb") => {
                    if let Some(n) = self.tree.element(node).and_then(|e| e.attribute("n")).and_then(|n| n.trim().parse().ok()) {
                        last = Some(n);
                    }
                }
                Some("pb") => last = None,
                _ => {}
            }
        }
        None
    }

    /// Whether segment `id` is in kept markup whose end has passed.
    fn in_closed(&self, id: usize) -> bool {
        let node = match self.source.places.get(id).cloned().flatten() {
            Some(Place::Node(node) | Place::Open(node) | Place::Close(node) | Place::Text { node, .. }) => node,
            None => return false,
        };
        let mut current = self.tree.parent(node);
        while let Some(parent) = current {
            if self.closed.contains(&parent) {
                return true;
            }
            current = self.tree.parent(parent);
        }
        false
    }

    fn is_inside(&self, node: NodeId, ancestor: NodeId) -> bool {
        let mut current = self.tree.parent(node);
        while let Some(parent) = current {
            if parent == ancestor {
                return true;
            }
            current = self.tree.parent(parent);
        }
        false
    }

    /// The node an anchor stands after (or in), for [`absorb`](Self::absorb).
    fn anchor_node(&mut self, anchor: Anchor) -> Option<NodeId> {
        match anchor {
            Anchor::After(node) | Anchor::Start(node) => Some(node),
            Anchor::Segment(id) => self.node(id),
        }
    }

    /// Puts an element's content in its place (its markup was removed).
    fn unwrap(&mut self, element: NodeId) -> Option<Anchor> {
        let parent = self.tree.parent(element)?;
        let index = self.tree.index_in_parent(element)?;
        for (i, node) in self.tree.children(element).to_vec().into_iter().enumerate() {
            self.tree.move_to(node, parent, index + 1 + i);
        }
        self.tree.remove(element);
        Some(self.anchor_at(parent, index))
    }

    /// Puts an element's content in a new element `markup` (shorthand such
    /// as `.seg{`) stands for, with the old element's attributes.
    fn rename(&mut self, element: NodeId, markup: &str) -> Option<()> {
        let new = markup_element(markup)?;
        let old = self.tree.element(element)?.clone();
        if old.name == new.name && new.attributes.is_empty() {
            return Some(());
        }
        let mut attributes = old.attributes.clone();
        for attribute in new.attributes {
            match attributes.iter_mut().find(|a| a.name == attribute.name) {
                Some(a) => a.value = attribute.value,
                None => attributes.push(attribute),
            }
        }
        let parent = self.tree.parent(element)?;
        let index = self.tree.index_in_parent(element)?;
        let replacement = Element { name: new.name, namespaces: Vec::new(), attributes };
        let node = self.tree.insert(parent, index, NodeKind::Element(Box::new(replacement)));
        for (i, child) in self.tree.children(element).to_vec().into_iter().enumerate() {
            self.tree.move_to(child, node, i);
        }
        self.tree.remove(element);
        self.renamed.insert(element, node);
        self.shown.insert(node);
        Some(())
    }
}

/// The element markup's start (`.q{`, `*{`, `.rdg[A]{`) stands for.
fn markup_element(opener: &str) -> Option<Element> {
    let closer = if opener == "*{" { "}*" } else { "}" };
    let source = format!("{opener}x{closer}");
    let tree = crate::parser::syntax::parse(&source);
    let [node] = tree.nodes.as_slice() else { return None };
    let crate::parser::syntax::SyntaxKind::Markup { markup, .. } = &node.kind else { return None };
    if markup.pair.is_some() {
        return None;
    }
    let attributes = markup
        .attributes
        .iter()
        .map(|(name, value)| crate::document::Attribute { name: name.clone(), value: value.clone() })
        .collect();
    Some(Element { name: markup.name.clone(), namespaces: Vec::new(), attributes })
}

/// Parses compiled XML (any number of nodes) inside a wrapper declaring the
/// namespaces a TEI or MENOTA document uses.
fn parse_fragment(xml: &str) -> Option<(Tree, NodeId)> {
    let wrapped = format!(
        "<saga-scribe-fragment xmlns=\"http://www.tei-c.org/ns/1.0\" xmlns:me=\"http://www.menota.org/ns/1.0\">{xml}</saga-scribe-fragment>"
    );
    let tree = Tree::parse(&wrapped).ok()?;
    let root = tree.root_element()?;
    Some((tree, root))
}

/// Applies `patches` (as checked by `check_paragraph_edits`) to the tree and
/// writes the body. `None` if an operation doesn't fit the tree; the caller
/// then rebuilds the body from segments.
pub(super) fn apply_to_tree(
    source: &TreeSource,
    segments: &[Segment],
    patches: &[PatchOperation],
    compiler: &mut Compiler,
) -> Option<RenderedBody> {
    let by_id: HashMap<usize, &Segment> = segments.iter().map(|s| (s.id(), s)).collect();
    let originals = Originals::new(Some(source));
    let structure = ParagraphStructure::new(segments);
    let mut applier = Applier {
        source,
        tree: source.tree.clone(),
        pieces: HashMap::new(),
        created: HashMap::new(),
        edited: Vec::new(),
        stale: Vec::new(),
        shown: segments
            .iter()
            .filter(|s| markup_segment(s) == Some(MarkupPart::Open))
            .filter_map(|s| match source.places.get(s.id())?.as_ref()? {
                Place::Open(node) => Some(*node),
                _ => None,
            })
            .collect(),
        wraps: Vec::new(),
        open: Vec::new(),
        closed: HashSet::new(),
        paragraph: None,
        renamed: HashMap::new(),
    };
    let mut anchor: Option<Anchor> = None;
    let mut anchor_segment: Option<usize> = None;
    // Insertions before the first segment, waiting for a position
    let mut pending: Vec<(&str, Option<usize>, Option<usize>)> = Vec::new();

    for patch in patches {
        match patch {
            // Outside any paragraph (a new document's body): a paragraph
            // starts here, holding what follows up to the next
            PatchOperation::Insert { dsl, token, .. }
                if dsl == PARAGRAPH && anchor_segment.is_none_or(|after| structure.split_point_after(segments, after).is_err()) =>
            {
                applier.paragraph = None;
                let (parent, index) = match anchor {
                    Some(at) => applier.position(at)?,
                    None => (find_body(&applier.tree)?, 0),
                };
                if !applier.tree.element(parent).is_some_and(|e| CONTAINERS.contains(&local(&e.name))) {
                    return None;
                }
                let element = Element { name: "p".into(), namespaces: Vec::new(), attributes: Vec::new() };
                let node = applier.tree.insert(parent, index, NodeKind::Element(Box::new(element)));
                applier.created.insert(node, *token);
                applier.paragraph = Some(node);
                anchor = Some(Anchor::Start(node));
            }
            PatchOperation::Insert { dsl, token, .. } if dsl == PARAGRAPH => {
                // Split the paragraph after the point found for the token before
                let after = anchor_segment?;
                let split = structure.split_point_after(segments, after).ok()?;
                // Where in the paragraph: after the token (or right after
                // the paragraph's start), or after the element it ends
                let (paragraph, from) = if split == after {
                    applier.position(anchor?)?
                } else {
                    let element = applier.node(split)?;
                    (applier.tree.parent(element)?, applier.tree.index_in_parent(element)? + 1)
                };
                let name = applier.tree.element(paragraph).filter(|e| local(&e.name) == "p")?.name.clone();
                let container = applier.tree.parent(paragraph)?;
                let at = applier.tree.index_in_parent(paragraph)? + 1;
                applier.insert_text("\n", container, at);
                let element = Element { name, namespaces: Vec::new(), attributes: Vec::new() };
                let new = applier.tree.insert(container, at + 1, NodeKind::Element(Box::new(element)));
                applier.created.insert(new, *token);
                let rest: Vec<NodeId> = applier.tree.children(paragraph)[from..].to_vec();
                for (i, node) in rest.into_iter().enumerate() {
                    applier.tree.move_to(node, new, i);
                }
                anchor = Some(Anchor::Start(new));
            }
            PatchOperation::Insert { dsl, token, .. } if markup_token(dsl) == Some(MarkupPart::Open) => {
                let (parent, index) = applier.position(anchor?)?;
                let element = markup_element(dsl)?;
                let node = applier.tree.insert(parent, index, NodeKind::Element(Box::new(element)));
                applier.created.insert(node, *token);
                applier.wraps.push(node);
                applier.open.push(node);
                anchor = Some(Anchor::Start(node));
            }
            PatchOperation::Insert { dsl, .. } if markup_token(dsl) == Some(MarkupPart::Close) => {
                anchor = Some(applier.close(anchor?)?);
            }
            PatchOperation::Insert { dsl, token, moved } => match anchor {
                Some(at) => {
                    let (mut parent, mut index) = applier.position(at)?;
                    let kind = token_kind(dsl);
                    // Pages as paragraphs: a new page break ends the paragraph
                    // it is in, and the rest of it starts the page's
                    let mut page_paragraph = None;
                    if compiler.config().wrap_pages && moved.is_none() && matches!(kind, TokenKind::PageBreak(_)) {
                        if let Some(name) = applier.tree.element(parent).filter(|e| local(&e.name) == "p").map(|e| e.name.clone()) {
                            let container = applier.tree.parent(parent)?;
                            let at = applier.tree.index_in_parent(parent)? + 1;
                            let element = Element { name, namespaces: Vec::new(), attributes: Vec::new() };
                            let new = applier.tree.insert(container, at, NodeKind::Element(Box::new(element)));
                            applier.created.insert(new, *token);
                            let rest: Vec<NodeId> = applier.tree.children(parent)[index..].to_vec();
                            for (i, node) in rest.into_iter().enumerate() {
                                applier.tree.move_to(node, new, i);
                            }
                            (parent, index) = (container, at);
                            page_paragraph = Some(new);
                        }
                    }
                    let newline = applier.insert_text("\n", parent, index);
                    let xml = inserted_xml(dsl, *moved, segments, originals, compiler);
                    let roots = applier.insert_xml(&xml, parent, index + 1, *token)?;
                    applier.mark_moved(&roots, *moved);
                    // Automatic line numbers: one more than the line before
                    if compiler.config().auto_line_numbers && matches!(kind, TokenKind::LineBreak(None)) {
                        if let Some(&lb) = roots.iter().find(|&&r| applier.tree.element(r).is_some_and(|e| local(&e.name) == "lb")) {
                            let n = applier.line_number_before(lb).map_or(1, |n| n + 1);
                            applier.tree.set_attribute(lb, "n", Some(&n.to_string())).ok()?;
                        }
                    }
                    anchor = Some(match page_paragraph {
                        Some(new) => Anchor::Start(new),
                        None => Anchor::After(roots.last().copied().unwrap_or(newline)),
                    });
                }
                None if markup_token(dsl).is_some() => return None,
                None => pending.push((dsl, *token, *moved)),
            },
            PatchOperation::Keep { segment_id }
            | PatchOperation::Modify { segment_id, .. }
            | PatchOperation::Delete { segment_id } => {
                let segment = *by_id.get(segment_id)?;
                if !pending.is_empty() {
                    let node = applier.node(*segment_id)?;
                    let parent = applier.tree.parent(node)?;
                    let mut index = applier.tree.index_in_parent(node)?;
                    for (dsl, token, moved) in pending.drain(..) {
                        let xml = inserted_xml(dsl, moved, segments, originals, compiler);
                        let roots = applier.insert_xml(&xml, parent, index, token)?;
                        applier.mark_moved(&roots, moved);
                        index += roots.len();
                        applier.insert_text("\n", parent, index);
                        index += 1;
                    }
                }
                let part = markup_segment(segment);
                // Structure of the file ends a paragraph started outside any
                if matches!(segment, Segment::Paragraph { .. } | Segment::Structural { shown: None, .. }) {
                    applier.paragraph = None;
                }
                if part.is_none() && !applier.closed.is_empty() && applier.in_closed(*segment_id) {
                    // In kept markup whose end the shorthand has passed
                    return None;
                }
                anchor = Some(match patch {
                    // Markup: kept or renamed (its start), removed, or ended
                    PatchOperation::Keep { .. } | PatchOperation::Modify { .. } if part == Some(MarkupPart::Open) => {
                        let node = applier.node(*segment_id)?;
                        if let PatchOperation::Modify { new_dsl, .. } = patch {
                            applier.rename(node, new_dsl)?;
                        }
                        let node = applier.node(*segment_id)?;
                        if !applier.wraps.is_empty() {
                            applier.absorb(node)?;
                        }
                        applier.open.push(node);
                        Anchor::Segment(*segment_id)
                    }
                    PatchOperation::Keep { .. } | PatchOperation::Modify { .. } if part == Some(MarkupPart::Close) => {
                        anchor_segment = Some(*segment_id);
                        anchor = Some(applier.close(anchor?)?);
                        continue;
                    }
                    PatchOperation::Delete { .. } if part == Some(MarkupPart::Open) => {
                        let node = applier.node(*segment_id)?;
                        applier.shown.remove(&node);
                        applier.unwrap(node)?
                    }
                    // A pair's `=`, kept; or the end of markup removed (its
                    // element went with its start)
                    _ if part.is_some() => {
                        if matches!(patch, PatchOperation::Keep { .. }) && part == Some(MarkupPart::Middle) {
                            Anchor::Segment(*segment_id)
                        } else {
                            anchor?
                        }
                    }
                    PatchOperation::Modify { new_dsl, token, fresh, .. } => {
                        let node = applier.node(*segment_id)?;
                        match (segment, token_kind(new_dsl)) {
                            (Segment::LineBreak { .. }, TokenKind::LineBreak(n))
                            | (Segment::PageBreak { .. }, TokenKind::PageBreak(n)) => {
                                applier.tree.set_attribute(node, "n", n.as_deref()).ok()?;
                                Anchor::After(node)
                            }
                            _ => {
                                // Another word in its place keeps the
                                // transcription markup that follows the text
                                let xml = if *fresh && !keeps_word_data(segment, new_dsl, originals) {
                                    render_insert(new_dsl, compiler)
                                } else {
                                    let original = applier.tree.node_xml(node).ok()?;
                                    let (xml, stale) = render_modify(segment, originals, Some(&original), new_dsl, !fresh, compiler);
                                    if !stale.is_empty() {
                                        applier.stale.push((*segment_id, stale));
                                    }
                                    xml
                                };
                                let parent = applier.tree.parent(node)?;
                                let index = applier.tree.index_in_parent(node)?;
                                applier.tree.remove(node);
                                let roots = applier.insert_xml(&xml, parent, index, *token)?;
                                if !fresh && keeps_word_data(segment, new_dsl, originals) {
                                    let word = roots.iter().copied().find(|&r| applier.tree.element(r).is_some());
                                    applier.edited.extend(word);
                                }
                                match roots.last() {
                                    Some(&last) => Anchor::After(last),
                                    None => applier.anchor_at(parent, index),
                                }
                            }
                        }
                    }
                    PatchOperation::Delete { .. } if matches!(segment, Segment::Paragraph { .. }) => {
                        // Merge into the paragraph before, keeping what
                        // separates them (words must stay apart)
                        let paragraph = applier.node(*segment_id)?;
                        let close = structure.preceding_close(segments, *segment_id)?;
                        let previous = applier.node(close.id())?;
                        let container = applier.tree.parent(paragraph)?;
                        if applier.tree.parent(previous) != Some(container) {
                            return None;
                        }
                        let (from, to) = (applier.tree.index_in_parent(previous)? + 1, applier.tree.index_in_parent(paragraph)?);
                        let between: Vec<NodeId> = applier.tree.children(container).get(from..to)?.to_vec();
                        if !between.iter().all(|&n| is_whitespace(&applier.tree, n)) {
                            return None;
                        }
                        let mut separator = None;
                        for node in between {
                            let at = applier.tree.children(previous).len();
                            applier.tree.move_to(node, previous, at);
                            separator = Some(node);
                        }
                        let separator = match separator {
                            Some(node) => node,
                            None => {
                                let at = applier.tree.children(previous).len();
                                applier.insert_text("\n", previous, at)
                            }
                        };
                        for node in applier.tree.children(paragraph).to_vec() {
                            let at = applier.tree.children(previous).len();
                            applier.tree.move_to(node, previous, at);
                        }
                        applier.tree.remove(paragraph);
                        Anchor::After(separator)
                    }
                    PatchOperation::Delete { .. } => {
                        let node = applier.node(*segment_id)?;
                        applier.delete(node)?
                    }
                    _ => Anchor::Segment(*segment_id),
                });
                if !applier.wraps.is_empty() || applier.paragraph.is_some() {
                    let last = applier.anchor_node(anchor?)?;
                    applier.absorb(last)?;
                }
                anchor_segment = Some(*segment_id);
            }
        }
    }
    if !pending.is_empty() {
        // Nothing to anchor to (an empty body): at the end of the body
        let body = find_body(&applier.tree)?;
        for (dsl, token, moved) in pending.drain(..) {
            let at = applier.tree.children(body).len();
            applier.insert_text("\n", body, at);
            let xml = inserted_xml(dsl, moved, segments, originals, compiler);
            let roots = applier.insert_xml(&xml, body, at + 1, token)?;
            applier.mark_moved(&roots, moved);
        }
        let at = applier.tree.children(body).len();
        applier.insert_text("\n", body, at);
    }

    let is_word = |tree: &Tree, id: NodeId| tree.element(id).is_some_and(|e| local(&e.name) == "w");
    let output = match applier.tree.write_recording(&is_word) {
        Ok(output) => output,
        Err(e) => {
            log::warn!("The edited document can't be written: {e}");
            return None;
        }
    };
    let written = &output.text;
    let (start, end) = (source.body_start, written.len().checked_sub(source.postamble_len)?);
    let tree_source = source.tree.source();
    if end < start || written[..start] != tree_source[..start] || written[end..] != tree_source[tree_source.len() - source.postamble_len..] {
        return None;
    }
    // The words: those written anew, and the file's words in what was
    // copied (the body's are all after the unchanged preamble)
    let edited: std::collections::HashSet<NodeId> = applier.edited.iter().copied().collect();
    let mut words = Vec::with_capacity(source.words.len() + 16);
    let mut edited_words = Vec::new();
    for piece in output.pieces {
        match piece {
            Piece::Node(node, offset) => {
                if offset < start || offset >= end {
                    continue;
                }
                let origin = match applier.created.get(&node) {
                    Some(&token) => Origin::Token(token),
                    None => match applier.tree.span(node) {
                        Some(span) => Origin::Source(span.start - start),
                        None => match source.replaced_nodes.get(&node) {
                            Some(&segment) => Origin::Segment(segment),
                            None => Origin::Token(None),
                        },
                    },
                };
                if edited.contains(&node) {
                    edited_words.push(offset - start);
                }
                words.push((offset - start, origin));
            }
            Piece::Copied(range, offset) => {
                let first = source.words.partition_point(|&w| w < range.start);
                for &word in source.words[first..].iter().take_while(|&&w| w < range.end) {
                    words.push((offset + word - range.start - start, Origin::Source(word - start)));
                }
            }
        }
    }
    Some(RenderedBody { xml: written[start..end].to_string(), edited_words, words, stale: applier.stale, tree: Some(applier.tree) })
}
