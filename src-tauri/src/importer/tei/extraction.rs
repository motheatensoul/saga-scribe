use crate::importer::tei::helpers;
use crate::importer::tei::segments::Segment;
use libxml::tree::{Node, NodeType};
use std::collections::HashMap;

pub struct Extractor {
    next_id: usize,
}

impl Extractor {
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn extract_segments(&mut self, node: &Node) -> Vec<Segment> {
        let mut segments = Vec::new();
        self.process_node(node, &mut segments);
        segments
    }

    fn process_node(&mut self, node: &Node, segments: &mut Vec<Segment>) {
        match node.get_type() {
            Some(NodeType::ElementNode) => {
                let local_name = helpers::local_name(node);
                match local_name.as_str() {
                    "w" => {
                        segments.push(self.extract_word(node));
                    }
                    "pc" => {
                        segments.push(self.extract_punctuation(node));
                    }
                    "lb" => {
                        segments.push(Segment::LineBreak {
                            id: self.next_id(),
                            attributes: self.extract_attributes(node),
                        });
                    }
                    "pb" => {
                        segments.push(Segment::PageBreak {
                            id: self.next_id(),
                            attributes: self.extract_attributes(node),
                        });
                    }
                    "handShift" => {
                        segments.push(Segment::HandShift {
                            id: self.next_id(),
                            attributes: self.extract_attributes(node),
                        });
                    }
                    "choice" => {
                        segments.push(self.extract_choice(node));
                    }
                    // Handle TEI elements that should be converted to DSL notation
                    // even when they appear outside of <w> tags
                    "gap" => {
                        segments.push(self.extract_inline_element(node, "[...]"));
                    }
                    "supplied" => {
                        if self.has_element_children(node) {
                            self.emit_structural(node, segments);
                        } else {
                            let mut content = String::new();
                            let mut dummy = false;
                            Self::node_to_dsl(node, &mut content, &mut dummy);
                            let trimmed = content.trim();
                            if !trimmed.is_empty() {
                                let dsl = format!("<{}>", trimmed);
                                segments.push(self.extract_inline_element(node, &dsl));
                            }
                        }
                    }
                    "del" => {
                        if self.has_element_children(node) {
                            self.emit_structural(node, segments);
                        } else {
                            let mut content = String::new();
                            let mut dummy = false;
                            Self::node_to_dsl(node, &mut content, &mut dummy);
                            let trimmed = content.trim();
                            if !trimmed.is_empty() {
                                let dsl = format!("-{{{}}}-", trimmed);
                                segments.push(self.extract_inline_element(node, &dsl));
                            }
                        }
                    }
                    "add" => {
                        if self.has_element_children(node) {
                            self.emit_structural(node, segments);
                        } else {
                            let mut content = String::new();
                            let mut dummy = false;
                            Self::node_to_dsl(node, &mut content, &mut dummy);
                            let trimmed = content.trim();
                            if !trimmed.is_empty() {
                                let dsl = format!("+{{{}}}+", trimmed);
                                segments.push(self.extract_inline_element(node, &dsl));
                            }
                        }
                    }
                    "unclear" => {
                        if self.has_element_children(node) {
                            self.emit_structural(node, segments);
                        } else {
                            let mut content = String::new();
                            let mut dummy = false;
                            Self::node_to_dsl(node, &mut content, &mut dummy);
                            let trimmed = content.trim();
                            if !trimmed.is_empty() {
                                let dsl = format!("?{{{}}}?", trimmed);
                                segments.push(self.extract_inline_element(node, &dsl));
                            }
                        }
                    }
                    "note" => {
                        // For notes, recursively convert children to DSL notation
                        let mut note_content = String::new();
                        let mut dummy = false;
                        Self::node_to_dsl(node, &mut note_content, &mut dummy);
                        let trimmed = note_content.trim();
                        if !trimmed.is_empty() {
                            let dsl = format!("^{{{}}}", trimmed);
                            segments.push(self.extract_inline_element(node, &dsl));
                        }
                    }
                    "am" => {
                        if self.has_element_children(node) {
                            self.emit_structural(node, segments);
                        } else {
                            // Abbreviation marker - extract entity name
                            let mut content = String::new();
                            let mut dummy = false;
                            Self::node_to_dsl(node, &mut content, &mut dummy);
                            let trimmed = content.trim();
                            if !trimmed.is_empty() {
                                let dsl = format!(":{}:", trimmed);
                                segments.push(self.extract_inline_element(node, &dsl));
                            }
                        }
                    }
                    _ => {
                        // Structural element
                        segments.push(Segment::Structural {
                            id: self.next_id(),
                            xml: self.open_tag(node),
                        });

                        let mut child = node.get_first_child();
                        while let Some(c) = child {
                            self.process_node(&c, segments);
                            child = c.get_next_sibling();
                        }

                        segments.push(Segment::Structural {
                            id: self.next_id(),
                            xml: format!("</{}>", helpers::qualified_name(node)),
                        });
                    }
                }
            }
            Some(NodeType::TextNode) => {
                let content = node.get_content();
                if content.trim().is_empty() {
                    // Pure whitespace
                    segments.push(Segment::Whitespace {
                        id: self.next_id(),
                        content,
                    });
                } else {
                    // Split text into words separated by whitespace
                    // This handles cases like "word1  word2\n\nword3\tword4"
                    let mut current_word = String::new();
                    let mut current_ws = String::new();

                    for c in content.chars() {
                        if c.is_whitespace() {
                            if !current_word.is_empty() {
                                segments.push(Segment::Word {
                                    id: self.next_id(),
                                    original_xml: helpers::escape_xml_text(&current_word),
                                    attributes: HashMap::new(),
                                    dsl_content: current_word.clone(),
                                    has_inline_lb: false,
                                });
                                current_word.clear();
                            }
                            current_ws.push(c);
                        } else {
                            if !current_ws.is_empty() {
                                segments.push(Segment::Whitespace {
                                    id: self.next_id(),
                                    content: current_ws.clone(),
                                });
                                current_ws.clear();
                            }
                            current_word.push(c);
                        }
                    }

                    if !current_word.is_empty() {
                        segments.push(Segment::Word {
                            id: self.next_id(),
                            original_xml: helpers::escape_xml_text(&current_word),
                            attributes: HashMap::new(),
                            dsl_content: current_word,
                            has_inline_lb: false,
                        });
                    }

                    if !current_ws.is_empty() {
                        segments.push(Segment::Whitespace {
                            id: self.next_id(),
                            content: current_ws,
                        });
                    }
                }
            }
            Some(NodeType::CommentNode) => {
                segments.push(Segment::Structural {
                    id: self.next_id(),
                    xml: format!("<!--{}-->", node.get_content()),
                });
            }
            _ => {}
        }
    }

    fn open_tag(&self, node: &Node) -> String {
        let name = helpers::qualified_name(node);
        let mut tag = format!("<{}", name);
        for (key, value) in node.get_attributes() {
            tag.push_str(&format!(
                " {}=\"{}\"",
                key,
                helpers::escape_xml_attr(&value)
            ));
        }
        tag.push('>');
        tag
    }

    fn emit_structural(&mut self, node: &Node, segments: &mut Vec<Segment>) {
        segments.push(Segment::Structural {
            id: self.next_id(),
            xml: self.open_tag(node),
        });

        let mut child = node.get_first_child();
        while let Some(c) = child {
            self.process_node(&c, segments);
            child = c.get_next_sibling();
        }

        segments.push(Segment::Structural {
            id: self.next_id(),
            xml: format!("</{}>", helpers::qualified_name(node)),
        });
    }

    fn has_element_children(&self, node: &Node) -> bool {
        let mut child = node.get_first_child();
        while let Some(c) = child {
            if c.get_type() == Some(NodeType::ElementNode) {
                return true;
            }
            child = c.get_next_sibling();
        }
        false
    }

    fn extract_attributes(&self, node: &Node) -> HashMap<String, String> {
        node.get_attributes().into_iter().collect()
    }

    /// Extract an inline TEI element (gap, supplied, del, add, etc.) as a Word segment
    /// with the provided DSL content. Preserves the original XML for round-trip.
    fn extract_inline_element(&mut self, node: &Node, dsl: &str) -> Segment {
        Segment::Word {
            id: self.next_id(),
            original_xml: helpers::serialize_node(node),
            attributes: HashMap::new(),
            dsl_content: dsl.to_string(),
            has_inline_lb: false,
        }
    }

    fn extract_word(&mut self, node: &Node) -> Segment {
        let attributes = self.extract_attributes(node);
        let original_xml = helpers::serialize_node(node);

        let mut facs_node = None;
        let mut child = node.get_first_child();
        while let Some(c) = child {
            if c.get_type() == Some(NodeType::ElementNode) {
                let name = helpers::local_name(&c);
                if name == "facs" {
                    facs_node = Some(c);
                    break;
                }
            }
            child = c.get_next_sibling();
        }

        let mut dsl_content = String::new();
        let mut has_inline_lb = false;

        if let Some(facs) = facs_node {
            Self::node_to_dsl(&facs, &mut dsl_content, &mut has_inline_lb);
        } else {
            Self::node_to_dsl(node, &mut dsl_content, &mut has_inline_lb);
        }

        Segment::Word {
            id: self.next_id(),
            original_xml,
            attributes,
            dsl_content,
            has_inline_lb,
        }
    }

    fn extract_punctuation(&mut self, node: &Node) -> Segment {
        let original_xml = helpers::serialize_node(node);

        // Look for me:facs child (or direct content)
        let mut facs_node = None;
        let mut child = node.get_first_child();
        while let Some(c) = child {
            if c.get_type() == Some(NodeType::ElementNode) {
                let name = helpers::local_name(&c);
                if name == "facs" {
                    facs_node = Some(c);
                    break;
                }
            }
            child = c.get_next_sibling();
        }

        let mut dsl_content = String::new();
        let mut has_inline_lb = false;

        if let Some(facs) = facs_node {
            Self::node_to_dsl(&facs, &mut dsl_content, &mut has_inline_lb);
        } else {
            Self::node_to_dsl(node, &mut dsl_content, &mut has_inline_lb);
        }

        Segment::Punctuation {
            id: self.next_id(),
            original_xml,
            dsl_content,
        }
    }

    fn extract_choice(&mut self, node: &Node) -> Segment {
        let attributes = self.extract_attributes(node);
        let original_xml = helpers::serialize_node(node);

        // First check if this choice contains abbr/expan (abbreviation pattern)
        let mut abbr_text = String::new();
        let mut expan_text = String::new();
        let mut found_abbr = false;
        let mut found_expan = false;
        let mut facs_node = None;

        let mut child = node.get_first_child();
        while let Some(c) = child {
            if c.get_type() == Some(NodeType::ElementNode) {
                let name = helpers::local_name(&c);
                match name.as_str() {
                    "abbr" => {
                        found_abbr = true;
                        let mut dummy = false;
                        Self::node_to_dsl(&c, &mut abbr_text, &mut dummy);
                    }
                    "expan" => {
                        found_expan = true;
                        let mut dummy = false;
                        Self::node_to_dsl(&c, &mut expan_text, &mut dummy);
                    }
                    "facs" => {
                        facs_node = Some(c.clone());
                    }
                    _ => {}
                }
            }
            child = c.get_next_sibling();
        }

        let mut dsl_content = String::new();
        let mut has_inline_lb = false;

        if found_abbr && found_expan {
            // Abbreviation pattern: .abbr[abbr]{expan}
            dsl_content = format!(".abbr[{}]{{{}}}", abbr_text, expan_text);
        } else if let Some(facs) = facs_node {
            // MENOTA multi-level pattern: extract from facs
            Self::node_to_dsl(&facs, &mut dsl_content, &mut has_inline_lb);
        } else {
            // Fallback: process all children
            Self::node_to_dsl(node, &mut dsl_content, &mut has_inline_lb);
        }

        Segment::Word {
            id: self.next_id(),
            original_xml,
            attributes,
            dsl_content,
            has_inline_lb,
        }
    }

    fn node_to_dsl(node: &Node, output: &mut String, has_inline_lb: &mut bool) {
        let mut child = node.get_first_child();
        while let Some(c) = child {
            match c.get_type() {
                Some(NodeType::TextNode) => {
                    let content = c.get_content();
                    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
                    if normalized.is_empty() {
                        // Ignore pure whitespace
                    } else {
                        let has_leading = content
                            .chars()
                            .next()
                            .map(|ch| ch.is_whitespace())
                            .unwrap_or(false);
                        let has_trailing = content
                            .chars()
                            .last()
                            .map(|ch| ch.is_whitespace())
                            .unwrap_or(false);

                        if has_leading && !output.is_empty() && !output.ends_with(' ') {
                            output.push(' ');
                        }

                        output.push_str(&normalized);

                        if has_trailing {
                            let next = c.get_next_sibling();
                            if next.is_some() && !output.ends_with(' ') {
                                output.push(' ');
                            }
                        }
                    }
                }
                Some(NodeType::EntityRefNode) => {
                    let name = c.get_name();
                    if !name.is_empty() {
                        output.push(':');
                        output.push_str(&name);
                        output.push(':');
                    } else {
                        output.push_str(&c.get_content());
                    }
                }
                Some(NodeType::ElementNode) => {
                    let name = helpers::local_name(&c);
                    match name.as_str() {
                        "choice" => {
                            let mut abbr_text = String::new();
                            let mut expan_text = String::new();
                            let mut found_abbr = false;
                            let mut found_expan = false;

                            let mut gc = c.get_first_child();
                            while let Some(gcc) = gc {
                                if gcc.get_type() == Some(NodeType::ElementNode) {
                                    let gc_name = helpers::local_name(&gcc);
                                    if gc_name == "abbr" {
                                        found_abbr = true;
                                        Self::node_to_dsl(&gcc, &mut abbr_text, has_inline_lb);
                                    } else if gc_name == "expan" {
                                        found_expan = true;
                                        Self::node_to_dsl(&gcc, &mut expan_text, has_inline_lb);
                                    }
                                }
                                gc = gcc.get_next_sibling();
                            }

                            if found_abbr && found_expan {
                                output.push_str(&format!(".abbr[{}]{{{}}}", abbr_text, expan_text));
                            } else {
                                Self::node_to_dsl(&c, output, has_inline_lb);
                            }
                        }
                        "am" => {
                            output.push(':');
                            Self::node_to_dsl(&c, output, has_inline_lb);
                            output.push(':');
                        }
                        "c" => {
                            output.push_str(&c.get_content());
                        }
                        "add" => {
                            let mut inner = String::new();
                            Self::node_to_dsl(&c, &mut inner, has_inline_lb);
                            let trimmed = inner.trim();
                            if !trimmed.is_empty() {
                                output.push_str("+{");
                                output.push_str(trimmed);
                                output.push_str("}+");
                            }
                        }
                        "del" => {
                            let mut inner = String::new();
                            Self::node_to_dsl(&c, &mut inner, has_inline_lb);
                            let trimmed = inner.trim();
                            if !trimmed.is_empty() {
                                output.push_str("-{");
                                output.push_str(trimmed);
                                output.push_str("}-");
                            }
                        }
                        "unclear" => {
                            let mut inner = String::new();
                            Self::node_to_dsl(&c, &mut inner, has_inline_lb);
                            let trimmed = inner.trim();
                            if !trimmed.is_empty() {
                                output.push_str("?{");
                                output.push_str(trimmed);
                                output.push_str("}?");
                            }
                        }
                        "supplied" => {
                            let mut inner = String::new();
                            Self::node_to_dsl(&c, &mut inner, has_inline_lb);
                            let trimmed = inner.trim();
                            if !trimmed.is_empty() {
                                output.push('<');
                                output.push_str(trimmed);
                                output.push('>');
                            }
                        }
                        "gap" => {
                            output.push_str("[...]");
                        }
                        "note" => {
                            let mut inner = String::new();
                            Self::node_to_dsl(&c, &mut inner, has_inline_lb);
                            let trimmed = inner.trim();
                            if !trimmed.is_empty() {
                                output.push_str("^{");
                                output.push_str(trimmed);
                                output.push('}');
                            }
                        }
                        "lb" => {
                            *has_inline_lb = true;
                            output.push_str("~//");
                            if let Some(n) = c.get_property("n") {
                                output.push_str(&n);
                            }
                        }
                        "dipl" | "norm" => {
                            // Skip
                        }
                        _ => {
                            Self::node_to_dsl(&c, output, has_inline_lb);
                        }
                    }
                }
                _ => {}
            }
            child = c.get_next_sibling();
        }
    }
}

#[allow(dead_code)]
pub fn extract_segments(body_node: &Node) -> Vec<Segment> {
    let mut extractor = Extractor::new();
    extractor.extract_segments(body_node)
}

pub fn segments_to_dsl(segments: &[Segment]) -> String {
    let mut dsl = String::new();
    let mut last_was_linebreak = false;
    let mut pending_space = false;  // Track if we should add a space before next word

    for segment in segments {
        match segment {
            Segment::Word { dsl_content, .. } => {
                // Only add space if there was whitespace in the source XML
                if pending_space && !last_was_linebreak {
                    dsl.push(' ');
                }
                dsl.push_str(dsl_content);
                last_was_linebreak = false;
                pending_space = false;
            }
            Segment::Punctuation { dsl_content, .. } => {
                // Punctuation typically follows immediately after word (no space)
                dsl.push_str(dsl_content);
                last_was_linebreak = false;
                pending_space = false;
            }
            Segment::LineBreak { attributes, .. } => {
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
            }
            Segment::PageBreak { attributes, .. } => {
                // Page break: put on new line, no trailing newline (content follows)
                dsl.push('\n');
                if let Some(n) = attributes.get("n") {
                    dsl.push_str(&format!("///{}", n));
                } else {
                    dsl.push_str("///");
                }
                last_was_linebreak = true;
                pending_space = false;
            }
            Segment::HandShift { .. } => {
                // HandShift is preserved structurally, not shown in DSL
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
