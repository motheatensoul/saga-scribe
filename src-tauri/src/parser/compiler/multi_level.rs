use crate::parser::ast::Node;
use crate::parser::compiler::base::Compiler;

impl Compiler<'_> {
    pub(crate) fn compile_word(&mut self, children: &[Node]) -> String {
        if self.config.multi_level {
            self.compile_word_multi_level(children)
        } else {
            self.compile_word_single(children)
        }
    }

    pub(crate) fn compile_word_multi_level(&mut self, children: &[Node]) -> String {
        let facs = self.nodes_to_facs(children);
        let dipl = self.nodes_to_diplomatic(children);

        // Get current word index and increment
        let current_index = self.word_index;
        self.word_index += 1;

        // Check if we have a user-provided normalized form (by index)
        let norm = if let Some(stored_norm) = self.get_stored_normalized_by_index(current_index) {
            crate::utils::escape_xml(&stored_norm)
        } else {
            // Fall back to auto-generated normalization
            self.nodes_to_normalized(children)
        };

        if facs.is_empty() && dipl.is_empty() && norm.is_empty() {
            String::new()
        } else {
            // Lookup by word INDEX (only confirmed instances have mappings)
            let lemma_attrs = self.get_lemma_attributes_by_index(current_index);
            let ann_attrs = self.get_annotation_attributes(current_index);
            let notes = self.get_note_elements(current_index);

            // Inject character annotations into facsimile level
            let facs_with_chars = self.inject_character_tags(&facs, current_index);

            format!(
                "<w{}{}>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>{}\n</w>\n",
                lemma_attrs, ann_attrs, facs_with_chars, dipl, norm, notes
            )
        }
    }

    pub(crate) fn compile_punctuation(&mut self, children: &[Node]) -> String {
        if self.config.multi_level {
            self.compile_punctuation_multi_level(children)
        } else {
            self.compile_punctuation_single(children)
        }
    }

    pub(crate) fn compile_punctuation_multi_level(&mut self, children: &[Node]) -> String {
        let facs = self.nodes_to_facs(children);
        let dipl = self.nodes_to_diplomatic(children);
        let norm = self.nodes_to_normalized(children);

        if facs.is_empty() && dipl.is_empty() && norm.is_empty() {
            String::new()
        } else {
            format!(
                "<pc>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</pc>\n",
                facs, dipl, norm
            )
        }
    }

    pub(crate) fn nodes_to_facs(&self, nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            output.push_str(&self.node_to_facs(node));
        }
        output
    }

    pub(crate) fn node_to_facs(&self, node: &Node) -> String {
        match node {
            Node::Text(text) => crate::utils::escape_xml(text),
            Node::Entity(name) => format!("&{};", name),
            Node::Abbreviation { abbr, .. } => format!("<abbr>{}</abbr>", crate::utils::escape_xml(abbr)),
            Node::Unclear(text) => format!("<unclear>{}</unclear>", crate::utils::escape_xml(text)),
            Node::Gap { quantity, .. } => {
                if let Some(n) = quantity {
                    format!("<gap reason=\"illegible\" quantity=\"{}\" unit=\"chars\"/>", n)
                } else {
                    "<gap reason=\"illegible\"/>".to_string()
                }
            }
            Node::Supplied(text) => format!("<supplied>{}</supplied>", crate::utils::escape_xml(text)),
            Node::Deletion(text) => format!("<del>{}</del>", crate::utils::escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", crate::utils::escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", crate::utils::escape_xml(text)),
            Node::CompoundJoin => " ".to_string(),
            _ => String::new(),
        }
    }

    pub(crate) fn nodes_to_diplomatic(&self, nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            output.push_str(&self.node_to_diplomatic(node));
        }
        output
    }

    pub(crate) fn node_to_diplomatic(&self, node: &Node) -> String {
        match node {
            Node::Text(text) => crate::utils::escape_xml(text),
            Node::Entity(name) => {
                if let Some(dict) = self.dictionary {
                    if dict.is_combining_mark(name) {
                        return String::new();
                    }
                    if let Some(base_letter) = dict.get_entity_diplomatic(name) {
                        return crate::utils::escape_xml(base_letter);
                    }
                }
                if let Some(registry) = self.entities {
                    if let Some(entity) = registry.get(name) {
                        return crate::utils::escape_xml(&entity.char);
                    }
                }
                format!("&{};", name)
            }
            Node::Abbreviation { expansion, .. } => format!("<expan>{}</expan>", crate::utils::escape_xml(expansion)),
            Node::Unclear(text) => format!("<unclear>{}</unclear>", crate::utils::escape_xml(text)),
            Node::Gap { supplied, .. } => {
                if let Some(text) = supplied {
                    format!("<supplied>{}</supplied>", crate::utils::escape_xml(text))
                } else {
                    String::new()
                }
            }
            Node::Supplied(text) => format!("<supplied>{}</supplied>", crate::utils::escape_xml(text)),
            Node::Deletion(text) => format!("<del>{}</del>", crate::utils::escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", crate::utils::escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", crate::utils::escape_xml(text)),
            Node::CompoundJoin => " ".to_string(),
            _ => String::new(),
        }
    }

    pub(crate) fn nodes_to_normalized(&self, nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            output.push_str(&self.node_to_normalized(node));
        }
        output
    }

    pub(crate) fn node_to_normalized(&self, node: &Node) -> String {
        match node {
            Node::Text(text) => {
                let normalized = self.normalize_text(text);
                crate::utils::escape_xml(&normalized)
            }
            Node::Entity(name) => {
                if let Some(dict) = self.dictionary {
                    if dict.is_combining_mark(name) {
                        return String::new();
                    }
                    if let Some(base_letter) = dict.get_entity_diplomatic(name) {
                        let normalized = self.normalize_text(base_letter);
                        return crate::utils::escape_xml(&normalized);
                    }
                }
                if let Some(registry) = self.entities {
                    if let Some(entity) = registry.get(name) {
                        let normalized = self.normalize_text(&entity.char);
                        return crate::utils::escape_xml(&normalized);
                    }
                }
                format!("&{};", name)
            }
            Node::Abbreviation { expansion, .. } => {
                let normalized = self.normalize_text(expansion);
                format!("<expan>{}</expan>", crate::utils::escape_xml(&normalized))
            }
            Node::Unclear(text) => {
                let normalized = self.normalize_text(text);
                format!("<unclear>{}</unclear>", crate::utils::escape_xml(&normalized))
            }
            Node::Gap { supplied, .. } => {
                if let Some(text) = supplied {
                    let normalized = self.normalize_text(text);
                    format!("<supplied>{}</supplied>", crate::utils::escape_xml(&normalized))
                } else {
                    String::new()
                }
            }
            Node::Supplied(text) => {
                let normalized = self.normalize_text(text);
                format!("<supplied>{}</supplied>", crate::utils::escape_xml(&normalized))
            }
            Node::Deletion(text) => {
                let normalized = self.normalize_text(text);
                format!("<del>{}</del>", crate::utils::escape_xml(&normalized))
            }
            Node::Addition(text) => {
                let normalized = self.normalize_text(text);
                format!("<add>{}</add>", crate::utils::escape_xml(&normalized))
            }
            Node::Note(text) => {
                let normalized = self.normalize_text(text);
                format!("<note>{}</note>", crate::utils::escape_xml(&normalized))
            }
            Node::CompoundJoin => String::new(),
            _ => String::new(),
        }
    }
}
