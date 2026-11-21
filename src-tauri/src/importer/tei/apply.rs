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
//!   `.p` removed merges the paragraph into the one before.
//!
//! Text outside words (a body without `<w>`) is split into text nodes where
//! a word in it is edited.

use super::patching::{
    inserted_xml, keeps_word_data, render_insert, render_modify, token_kind, Origin, Originals, ParagraphStructure,
    PatchOperation, RenderedBody, TokenKind,
};
use super::segments::Segment;
use crate::document::{find_body, Element, NodeId, NodeKind, Piece, Tree};
use crate::identity::PARAGRAPH;
use crate::parser::Compiler;
use std::collections::HashMap;
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
        Ok(Self { tree, body_start, postamble_len, places, spans: located, words })
    }

    /// Segment `id` as written in the file (a word, punctuation, a tag).
    pub fn segment_xml(&self, id: usize) -> Option<&str> {
        let (start, end) = (*self.spans.get(id)?)?;
        self.tree.source().get(start as usize..end as usize)
    }

    /// The attributes of the element segment `id` is, as read from the file.
    pub fn segment_attributes(&self, id: usize) -> Option<HashMap<String, String>> {
        let element = self.segment_element(id)?;
        Some(element.attributes.iter().map(|a| (a.name.clone(), a.value.clone())).collect())
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
}

impl Applier<'_> {
    /// The node of segment `id`, splitting text where it is part of one.
    fn node(&mut self, id: usize) -> Option<NodeId> {
        match self.source.places.get(id)?.clone()? {
            Place::Node(node) | Place::Open(node) | Place::Close(node) => Some(node),
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
                Place::Open(element) => Some((element, 0)),
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
            let wrapper = self.tree.element(parent).is_some_and(|e| !BLOCKS.contains(&local(&e.name)));
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
    };
    let mut anchor: Option<Anchor> = None;
    let mut anchor_segment: Option<usize> = None;
    // Insertions before the first segment, waiting for a position
    let mut pending: Vec<(&str, Option<usize>, Option<usize>)> = Vec::new();

    for patch in patches {
        match patch {
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
            PatchOperation::Insert { dsl, token, moved } => match anchor {
                Some(at) => {
                    let (parent, index) = applier.position(at)?;
                    let newline = applier.insert_text("\n", parent, index);
                    let xml = inserted_xml(dsl, *moved, segments, originals, compiler);
                    let roots = applier.insert_xml(&xml, parent, index + 1, *token)?;
                    applier.mark_moved(&roots, *moved);
                    anchor = Some(Anchor::After(roots.last().copied().unwrap_or(newline)));
                }
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
                anchor = Some(match patch {
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
                        None => Origin::Token(None),
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
    Some(RenderedBody { xml: written[start..end].to_string(), edited_words, words, stale: applier.stale })
}
