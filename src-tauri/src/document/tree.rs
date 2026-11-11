//! The arena tree: libxml2's parse of a document, copied node for node, with
//! the byte span of each node in the original text.

use crate::importer::tei::helpers::qualified_name;
use crate::importer::tei::source_map::SourceCursor;
use libxml::tree::{Node as XmlNode, NodeType};
use std::ffi::CStr;
use std::ops::Range;

/// A byte range in the original text.
pub type Span = Range<usize>;

/// A node in a [`Tree`]. Stays valid for the tree's lifetime, also after the
/// node is removed (it is then detached, not reused).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    fn index(self) -> usize {
        self.0 as usize
    }
}

/// An attribute: its name as written (`xml:id`, `me:msa`, `lemma`) and its
/// value with references resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

/// An element: its qualified name as written, the namespaces it declares
/// (prefix, empty for the default namespace; URI) and its attributes in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub namespaces: Vec<(String, String)>,
    pub attributes: Vec<Attribute>,
}

impl Element {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.iter().find(|a| a.name == name).map(|a| a.value.as_str())
    }
}

/// What a node is. Every kind of node libxml2 gives for a document has one,
/// so nothing in the document falls outside the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Document,
    Element(Element),
    /// Text with references resolved (`&amp;` is `&`, `&#xE6;` is `æ`)
    Text(String),
    /// A named entity reference (`&thorn;`), kept as a reference
    EntityRef(String),
    Comment(String),
    Cdata(String),
    ProcessingInstruction { target: String, data: String },
    /// The document type declaration, as written. It isn't edited.
    Doctype(String),
}

#[derive(Debug, Clone)]
struct NodeData {
    kind: NodeKind,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    /// Where the node is in the original text, if known
    span: Option<Span>,
    /// For elements: the spans of the start and end tag (empty for `<x/>`)
    tags: Option<(Span, Span)>,
    /// The element's attributes as read, kept once they are first changed, so
    /// the start tag can be patched in place
    original_attributes: Option<Vec<Attribute>>,
    /// The node's own data (text, attributes) changed
    changed: bool,
    /// Children were added or removed
    children_changed: bool,
    /// Something below it changed
    descendant_changed: bool,
}

impl NodeData {
    fn new(kind: NodeKind, parent: Option<NodeId>) -> Self {
        Self {
            kind,
            parent,
            children: Vec::new(),
            span: None,
            tags: None,
            original_attributes: None,
            changed: false,
            children_changed: false,
            descendant_changed: false,
        }
    }
}

/// A document: the tree, and the original text its spans refer to.
#[derive(Debug, Clone)]
pub struct Tree {
    nodes: Vec<NodeData>,
    source: String,
    /// Whether every node read from the source has its span. If the text and
    /// libxml2's tree ever disagree, no node has one (a partial map would be
    /// worse than none) and the document is written from the tree.
    spans: bool,
}

impl Tree {
    /// Parses `xml` with libxml2 ([`crate::xml::parse`]) and copies the result.
    ///
    /// # Errors
    ///
    /// libxml2's error, if the document isn't well-formed.
    pub fn parse(xml: &str) -> Result<Self, String> {
        let doc = crate::xml::parse(xml)?;
        let mut tree = Tree {
            nodes: vec![NodeData::new(NodeKind::Document, None)],
            source: xml.to_string(),
            spans: false,
        };
        let doctype = crate::xml::doctype_span(xml).map(|span| xml[span].to_string());
        let mut child = doc.as_node().get_first_child();
        while let Some(node) = child {
            tree.copy(&node, Tree::DOCUMENT, doctype.as_deref());
            child = node.get_next_sibling();
        }
        tree.spans = tree.locate_spans().is_some();
        if !tree.spans {
            for node in &mut tree.nodes {
                node.span = None;
                node.tags = None;
            }
        }
        Ok(tree)
    }

    /// The document node.
    pub const DOCUMENT: NodeId = NodeId(0);

    /// Whether every node read from the source knows its place in it.
    pub fn has_spans(&self) -> bool {
        self.spans
    }

    /// The original text.
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn kind(&self, id: NodeId) -> &NodeKind {
        &self.nodes[id.index()].kind
    }

    pub fn element(&self, id: NodeId) -> Option<&Element> {
        match self.kind(id) {
            NodeKind::Element(element) => Some(element),
            _ => None,
        }
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id.index()].children
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.index()].parent
    }

    pub fn span(&self, id: NodeId) -> Option<Span> {
        self.nodes[id.index()].span.clone()
    }

    /// The document's root element.
    pub fn root_element(&self) -> Option<NodeId> {
        self.children(Tree::DOCUMENT)
            .iter()
            .copied()
            .find(|&id| matches!(self.kind(id), NodeKind::Element(_)))
    }

    /// `id` and every node below it, in document order.
    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(next) = stack.pop() {
            out.push(next);
            stack.extend(self.children(next).iter().rev());
        }
        out
    }

    /// Drops every span, as when the text and the tree disagree.
    #[cfg(test)]
    pub(super) fn forget_spans(&mut self) {
        self.spans = false;
        for node in &mut self.nodes {
            node.span = None;
            node.tags = None;
        }
    }

    // ------------------------------------------------------------------
    // Editing
    // ------------------------------------------------------------------

    /// Replaces the content of a text, comment or CDATA node.
    ///
    /// # Errors
    ///
    /// If the node has no text content.
    pub fn set_text(&mut self, id: NodeId, text: &str) -> Result<(), String> {
        match &mut self.nodes[id.index()].kind {
            NodeKind::Text(t) | NodeKind::Comment(t) | NodeKind::Cdata(t) => {
                if t == text {
                    return Ok(());
                }
                *t = text.to_string();
            }
            other => return Err(format!("{other:?} has no text")),
        }
        self.mark_changed(id);
        Ok(())
    }

    /// Sets (`Some`) or removes (`None`) an attribute of an element. A new
    /// attribute is added after the others.
    ///
    /// # Errors
    ///
    /// If the node isn't an element.
    pub fn set_attribute(&mut self, id: NodeId, name: &str, value: Option<&str>) -> Result<(), String> {
        let node = &mut self.nodes[id.index()];
        let NodeKind::Element(element) = &mut node.kind else {
            return Err(format!("{:?} has no attributes", node.kind));
        };
        let position = element.attributes.iter().position(|a| a.name == name);
        if position.map(|i| element.attributes[i].value.as_str()) == value {
            return Ok(());
        }
        if node.original_attributes.is_none() {
            node.original_attributes = Some(element.attributes.clone());
        }
        match (position, value) {
            (Some(i), Some(v)) => element.attributes[i].value = v.to_string(),
            (Some(i), None) => {
                element.attributes.remove(i);
            }
            (None, Some(v)) => element.attributes.push(Attribute { name: name.to_string(), value: v.to_string() }),
            (None, None) => {}
        }
        self.mark_changed(id);
        Ok(())
    }

    /// Inserts a new node as the `index`-th child of `parent`.
    pub fn insert(&mut self, parent: NodeId, index: usize, kind: NodeKind) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        let mut data = NodeData::new(kind, Some(parent));
        data.changed = true;
        self.nodes.push(data);
        let children = &mut self.nodes[parent.index()].children;
        children.insert(index.min(children.len()), id);
        self.mark_children_changed(parent);
        id
    }

    /// Detaches a node (with everything below it) from its parent.
    pub fn remove(&mut self, id: NodeId) {
        let Some(parent) = self.nodes[id.index()].parent.take() else { return };
        self.nodes[parent.index()].children.retain(|&c| c != id);
        self.mark_children_changed(parent);
    }

    fn mark_changed(&mut self, id: NodeId) {
        self.nodes[id.index()].changed = true;
        self.mark_ancestors(id);
    }

    fn mark_children_changed(&mut self, id: NodeId) {
        self.nodes[id.index()].children_changed = true;
        self.mark_ancestors(id);
    }

    fn mark_ancestors(&mut self, id: NodeId) {
        let mut next = self.nodes[id.index()].parent;
        while let Some(parent) = next {
            let node = &mut self.nodes[parent.index()];
            if node.descendant_changed {
                break;
            }
            node.descendant_changed = true;
            next = node.parent;
        }
    }

    /// Whether the node and everything below it are as they were read.
    pub(super) fn is_unchanged(&self, id: NodeId) -> bool {
        let node = &self.nodes[id.index()];
        !(node.changed || node.children_changed || node.descendant_changed)
    }

    pub(super) fn tags(&self, id: NodeId) -> Option<(Span, Span)> {
        self.nodes[id.index()].tags.clone()
    }

    pub(super) fn own_data_changed(&self, id: NodeId) -> bool {
        self.nodes[id.index()].changed
    }

    pub(super) fn original_attributes(&self, id: NodeId) -> Option<&[Attribute]> {
        self.nodes[id.index()].original_attributes.as_deref()
    }

    // ------------------------------------------------------------------
    // Copying libxml2's tree
    // ------------------------------------------------------------------

    fn copy(&mut self, node: &XmlNode, parent: NodeId, doctype: Option<&str>) {
        let kind = match node.get_type() {
            Some(NodeType::ElementNode) => NodeKind::Element(Element {
                name: qualified_name(node),
                namespaces: node
                    .get_namespace_declarations()
                    .iter()
                    .map(|ns| (ns.get_prefix(), ns.get_href()))
                    .collect(),
                attributes: attributes(node),
            }),
            Some(NodeType::TextNode) => NodeKind::Text(node.get_content()),
            Some(NodeType::EntityRefNode) => NodeKind::EntityRef(node.get_name()),
            Some(NodeType::CommentNode) => NodeKind::Comment(node.get_content()),
            Some(NodeType::CDataSectionNode) => NodeKind::Cdata(node.get_content()),
            Some(NodeType::PiNode) => NodeKind::ProcessingInstruction {
                target: node.get_name(),
                data: node.get_content(),
            },
            // libxml2 gives a DTD node also when only crate::xml::parse added
            // one (to declare entities); only a declaration in the text counts
            Some(NodeType::DTDNode | NodeType::DocumentTypeNode) => match doctype {
                Some(text) => NodeKind::Doctype(text.to_string()),
                None => return,
            },
            // Nothing else occurs in a parsed document's tree
            _ => return,
        };
        let is_element = matches!(kind, NodeKind::Element(_));
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(NodeData::new(kind, Some(parent)));
        self.nodes[parent.index()].children.push(id);
        // An entity reference's children are the entity's own content, shared
        // by all its references: not part of this document's tree
        if is_element {
            let mut child = node.get_first_child();
            while let Some(c) = child {
                self.copy(&c, id, None);
                child = c.get_next_sibling();
            }
        }
    }

    // ------------------------------------------------------------------
    // Spans
    // ------------------------------------------------------------------

    /// Finds every node in the source, in document order. `None` as soon as
    /// the text and the tree disagree.
    fn locate_spans(&mut self) -> Option<()> {
        let mut cursor = SourceCursor::new(&self.source);
        cursor.advance(if self.source.starts_with('\u{FEFF}') { 3 } else { 0 });
        // The XML declaration isn't a node
        if cursor.at("<?xml") && self.source[cursor.pos() + 5..].starts_with(char::is_whitespace) {
            cursor.processing_instruction()?;
        }
        let top: Vec<NodeId> = self.children(Tree::DOCUMENT).to_vec();
        for id in top {
            cursor.skip_whitespace();
            self.locate(id, &mut cursor)?;
        }
        cursor.skip_whitespace();
        cursor.at_end().then_some(())
    }

    fn locate(&mut self, id: NodeId, cursor: &mut SourceCursor) -> Option<()> {
        let start = cursor.pos();
        match self.kind(id).clone() {
            NodeKind::Element(element) => {
                let start_tag = cursor.open_tag(&element.name)?;
                for child in self.children(id).to_vec() {
                    self.locate(child, cursor)?;
                }
                let end_tag = cursor.close_tag(&element.name)?;
                self.nodes[id.index()].tags = Some((start_tag, end_tag));
            }
            NodeKind::Text(text) => {
                cursor.text(&text)?;
            }
            NodeKind::EntityRef(name) => cursor.entity_ref(&name)?,
            NodeKind::Comment(_) => cursor.comment()?,
            NodeKind::Cdata(_) => cursor.cdata()?,
            NodeKind::ProcessingInstruction { .. } => cursor.processing_instruction()?,
            NodeKind::Doctype(text) => {
                if !cursor.at(&text) {
                    return None;
                }
                cursor.advance(text.len());
            }
            NodeKind::Document => return None,
        }
        self.nodes[id.index()].span = Some(start..cursor.pos());
        Some(())
    }
}

/// An element's attributes in the order they are written.
///
/// The binding only offers them as a map, which loses the order; libxml2
/// keeps them as a list, which is read here (names only; the values come
/// from the binding).
fn attributes(node: &XmlNode) -> Vec<Attribute> {
    let mut names: Vec<(String, String, Option<String>)> = Vec::new();
    // SAFETY: `node` is an element of a document that lives for this call.
    // Its attribute list and their namespaces are only read, and no pointer
    // is kept beyond this block.
    unsafe {
        let text = |p: *const u8| -> Option<String> {
            (!p.is_null()).then(|| CStr::from_ptr(p.cast()).to_string_lossy().into_owned())
        };
        let mut attr = (*node.node_ptr()).properties;
        while !attr.is_null() {
            let local = text((*attr).name).unwrap_or_default();
            let ns = (*attr).ns;
            let (name, href) = if ns.is_null() {
                (local.clone(), None)
            } else {
                let name = match text((*ns).prefix) {
                    Some(prefix) => format!("{prefix}:{local}"),
                    None => local.clone(),
                };
                (name, text((*ns).href))
            };
            names.push((name, local, href));
            attr = (*attr).next;
        }
    }
    names
        .into_iter()
        .map(|(name, local, href)| {
            let value = match &href {
                Some(href) => node.get_property_ns(&local, href),
                None => node.get_property_no_ns(&local),
            };
            Attribute { name, value: value.unwrap_or_default() }
        })
        .collect()
}
