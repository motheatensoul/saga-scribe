use super::ast::Node;
use super::lexer::Lexer;
use super::wordtokenizer::WordTokenizer;
use crate::entities::EntityRegistry;
use crate::normalizer::LevelDictionary;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for the compiler
#[derive(Debug, Clone, Default)]
pub struct CompilerConfig {
    pub word_wrap: bool,
    pub auto_line_numbers: bool,
    pub multi_level: bool,
}

/// A lemma mapping for a wordform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LemmaMapping {
    pub lemma: String,
    pub msa: String,
    #[serde(default)]
    pub normalized: Option<String>,
}

/// Compiles DSL input into TEI-XML
pub struct Compiler<'a> {
    entities: Option<&'a EntityRegistry>,
    dictionary: Option<&'a LevelDictionary>,
    /// Lemma mappings by word INDEX (for confirmed instances only)
    lemma_mappings: HashMap<u32, LemmaMapping>,
    config: CompilerConfig,
    line_number: u32,
    /// Current word index counter
    word_index: u32,
}

impl<'a> Compiler<'a> {
    pub fn new() -> Self {
        Self {
            entities: None,
            dictionary: None,
            lemma_mappings: HashMap::new(),
            config: CompilerConfig::default(),
            line_number: 0,
            word_index: 0,
        }
    }

    pub fn with_entities(mut self, registry: &'a EntityRegistry) -> Self {
        self.entities = Some(registry);
        self
    }

    pub fn with_dictionary(mut self, dictionary: &'a LevelDictionary) -> Self {
        self.dictionary = Some(dictionary);
        self
    }

    /// Set lemma mappings by word INDEX (for confirmed word instances)
    pub fn with_lemma_mappings(mut self, mappings: HashMap<u32, LemmaMapping>) -> Self {
        self.lemma_mappings = mappings;
        self
    }

    pub fn with_config(mut self, config: CompilerConfig) -> Self {
        self.config = config;
        self
    }

    pub fn compile(&mut self, input: &str) -> Result<String, String> {
        let mut lexer = Lexer::new(input);
        let doc = lexer.parse()?;

        let nodes = if self.config.word_wrap {
            let tokenizer = WordTokenizer::new();
            tokenizer.tokenize(doc.nodes)
        } else {
            doc.nodes
        };

        // Reset counters for each compilation
        self.line_number = 0;
        self.word_index = 0;

        let mut output = String::new();
        for node in &nodes {
            output.push_str(&self.node_to_xml(node));
        }
        Ok(output)
    }

    fn node_to_xml(&mut self, node: &Node) -> String {
        match node {
            Node::Text(text) => self.escape_xml(text),
            Node::LineBreak(n) => {
                self.line_number += 1;
                match n {
                    Some(num) => format!("<lb n=\"{}\"/>\n", self.escape_xml(num)),
                    None if self.config.auto_line_numbers => {
                        format!("<lb n=\"{}\"/>\n", self.line_number)
                    }
                    None => "<lb/>\n".to_string(),
                }
            }
            Node::PageBreak(n) => format!("<pb n=\"{}\"/>\n", self.escape_xml(n)),
            Node::Abbreviation { abbr, expansion } => {
                format!(
                    "<choice><abbr>{}</abbr><expan>{}</expan></choice>",
                    self.escape_xml(abbr),
                    self.escape_xml(expansion)
                )
            }
            Node::Gap { quantity, supplied } => {
                let gap_xml = match quantity {
                    Some(n) => {
                        format!("<gap reason=\"illegible\" quantity=\"{}\" unit=\"chars\"/>", n)
                    }
                    None => "<gap reason=\"illegible\"/>".to_string(),
                };
                // If there's supplied text, output both gap and supplied
                match supplied {
                    Some(text) => format!("{}<supplied>{}</supplied>", gap_xml, self.escape_xml(text)),
                    None => gap_xml,
                }
            }
            Node::Supplied(text) => format!("<supplied>{}</supplied>", self.escape_xml(text)),
            Node::Deletion(text) => format!("<del>{}</del>", self.escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", self.escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", self.escape_xml(text)),
            Node::Unclear(text) => format!("<unclear>{}</unclear>", self.escape_xml(text)),
            Node::Entity(name) => self.compile_entity(name),
            Node::WordContinuation => String::new(), // Consumed by word tokenizer
            Node::WordBoundary => String::new(),     // Consumed by word tokenizer
            Node::CompoundJoin => " ".to_string(),   // Space in single-level mode
            Node::Word(children) => self.compile_word(children),
            Node::Punctuation(children) => self.compile_punctuation(children),
        }
    }

    fn compile_entity(&self, name: &str) -> String {
        // Output as XML entity reference &name;
        // The entity must be defined in the TEI header or be a standard XML entity
        format!("&{};", name)
    }

    fn compile_word(&mut self, children: &[Node]) -> String {
        if self.config.multi_level {
            self.compile_word_multi_level(children)
        } else {
            self.compile_word_single(children)
        }
    }

    fn compile_word_single(&mut self, children: &[Node]) -> String {
        let mut content = String::new();
        for child in children {
            content.push_str(&self.node_to_xml(child));
        }
        if content.is_empty() {
            String::new()
        } else {
            // Get current word index and increment
            let current_index = self.word_index;
            self.word_index += 1;

            // Lookup by word INDEX (only confirmed instances have mappings)
            let attrs = self.get_lemma_attributes_by_index(current_index);
            format!("<w{}>{}</w>\n", attrs, content)
        }
    }

    fn compile_word_multi_level(&mut self, children: &[Node]) -> String {
        let facs = self.nodes_to_facs(children);
        let dipl = self.nodes_to_diplomatic(children);

        // Get current word index and increment
        let current_index = self.word_index;
        self.word_index += 1;

        // Check if we have a user-provided normalized form (by index)
        let norm = if let Some(stored_norm) = self.get_stored_normalized_by_index(current_index) {
            self.escape_xml(&stored_norm)
        } else {
            // Fall back to auto-generated normalization
            self.nodes_to_normalized(children)
        };

        if facs.is_empty() && dipl.is_empty() && norm.is_empty() {
            String::new()
        } else {
            // Lookup by word INDEX (only confirmed instances have mappings)
            let attrs = self.get_lemma_attributes_by_index(current_index);
            format!(
                "<w{}>\n  <me:facs>{}</me:facs>\n  <me:dipl>{}</me:dipl>\n  <me:norm>{}</me:norm>\n</w>\n",
                attrs, facs, dipl, norm
            )
        }
    }

    /// Get lemma and me:msa attributes for a word by INDEX
    fn get_lemma_attributes_by_index(&self, word_index: u32) -> String {
        if let Some(mapping) = self.lemma_mappings.get(&word_index) {
            format!(
                " lemma=\"{}\" me:msa=\"{}\"",
                self.escape_xml(&mapping.lemma),
                self.escape_xml(&mapping.msa)
            )
        } else {
            String::new()
        }
    }

    /// Get stored normalized form for a word by INDEX
    fn get_stored_normalized_by_index(&self, word_index: u32) -> Option<String> {
        self.lemma_mappings
            .get(&word_index)
            .and_then(|m| m.normalized.clone())
    }

    fn compile_punctuation(&mut self, children: &[Node]) -> String {
        if self.config.multi_level {
            self.compile_punctuation_multi_level(children)
        } else {
            self.compile_punctuation_single(children)
        }
    }

    fn compile_punctuation_single(&mut self, children: &[Node]) -> String {
        let mut content = String::new();
        for child in children {
            content.push_str(&self.node_to_xml(child));
        }
        if content.is_empty() {
            String::new()
        } else {
            format!("<pc>{}</pc>\n", content)
        }
    }

    fn compile_punctuation_multi_level(&mut self, children: &[Node]) -> String {
        let facs = self.nodes_to_facs(children);
        let dipl = self.nodes_to_diplomatic(children);
        let norm = self.nodes_to_normalized(children);

        if facs.is_empty() && dipl.is_empty() && norm.is_empty() {
            String::new()
        } else {
            format!(
                "<pc>\n  <me:facs>{}</me:facs>\n  <me:dipl>{}</me:dipl>\n  <me:norm>{}</me:norm>\n</pc>\n",
                facs, dipl, norm
            )
        }
    }

    /// Generate facsimile level content: entity refs, abbreviation form only
    fn nodes_to_facs(&self, nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            output.push_str(&self.node_to_facs(node));
        }
        output
    }

    fn node_to_facs(&self, node: &Node) -> String {
        match node {
            Node::Text(text) => self.escape_xml(text),
            Node::Entity(name) => format!("&{};", name),
            Node::Abbreviation { abbr, .. } => format!("<abbr>{}</abbr>", self.escape_xml(abbr)),
            Node::Unclear(text) => format!("<unclear>{}</unclear>", self.escape_xml(text)),
            Node::Gap { quantity, .. } => {
                // Facsimile shows gap only, not supplied
                match quantity {
                    Some(n) => format!("<gap reason=\"illegible\" quantity=\"{}\" unit=\"chars\"/>", n),
                    None => "<gap reason=\"illegible\"/>".to_string(),
                }
            }
            Node::Supplied(_) => String::new(), // Not shown in facsimile
            Node::Deletion(text) => format!("<del>{}</del>", self.escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", self.escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", self.escape_xml(text)),
            Node::CompoundJoin => " ".to_string(), // Space in facsimile
            Node::LineBreak(_) | Node::PageBreak(_) => String::new(), // Handled outside word
            _ => String::new(),
        }
    }

    /// Generate diplomatic level content: resolve entities, expand abbreviations, remove combining marks
    fn nodes_to_diplomatic(&self, nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            output.push_str(&self.node_to_diplomatic(node));
        }
        output
    }

    fn node_to_diplomatic(&self, node: &Node) -> String {
        match node {
            Node::Text(text) => self.escape_xml(text),
            Node::Entity(name) => {
                // Check if it's a combining mark (skip it)
                if let Some(dict) = self.dictionary {
                    if dict.is_combining_mark(name) {
                        return String::new();
                    }
                    // Check for entity → base letter mapping (diplomatic normalization)
                    if let Some(base_letter) = dict.get_entity_diplomatic(name) {
                        return self.escape_xml(base_letter);
                    }
                }
                // Fallback: resolve entity to character
                if let Some(registry) = self.entities {
                    if let Some(entity) = registry.get(name) {
                        return self.escape_xml(&entity.char);
                    }
                }
                // Final fallback to entity reference
                format!("&{};", name)
            }
            Node::Abbreviation { expansion, .. } => format!("<expan>{}</expan>", self.escape_xml(expansion)),
            Node::Unclear(text) => format!("<unclear>{}</unclear>", self.escape_xml(text)),
            // Clippy is cranky about this being two nested matches, its suggested fix does not work though.
            Node::Gap { supplied, .. } => {
                // Diplomatic shows supplied text if available
                match supplied {
                    Some(text) => format!("<supplied>{}</supplied>", self.escape_xml(text)),
                    None => String::new(),
                }
            }
            Node::Supplied(text) => format!("<supplied>{}</supplied>", self.escape_xml(text)),
            Node::Deletion(text) => format!("<del>{}</del>", self.escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", self.escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", self.escape_xml(text)),
            Node::CompoundJoin => " ".to_string(), // Space in diplomatic
            Node::LineBreak(_) | Node::PageBreak(_) => String::new(),
            _ => String::new(),
        }
    }

    /// Generate normalized level content: apply character normalization
    fn nodes_to_normalized(&self, nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            output.push_str(&self.node_to_normalized(node));
        }
        output
    }

    fn node_to_normalized(&self, node: &Node) -> String {
        match node {
            Node::Text(text) => {
                let normalized = self.normalize_text(text);
                self.escape_xml(&normalized)
            }
            Node::Entity(name) => {
                // Skip combining marks
                if let Some(dict) = self.dictionary {
                    if dict.is_combining_mark(name) {
                        return String::new();
                    }
                    // Check for entity → base letter mapping first
                    if let Some(base_letter) = dict.get_entity_diplomatic(name) {
                        // Apply character normalization to the base letter
                        let normalized = self.normalize_text(base_letter);
                        return self.escape_xml(&normalized);
                    }
                }
                // Fallback: resolve entity and normalize
                if let Some(registry) = self.entities {
                    if let Some(entity) = registry.get(name) {
                        let normalized = self.normalize_text(&entity.char);
                        return self.escape_xml(&normalized);
                    }
                }
                format!("&{};", name)
            }
            Node::Abbreviation { expansion, .. } => {
                let normalized = self.normalize_text(expansion);
                format!("<expan>{}</expan>", self.escape_xml(&normalized))
            }
            Node::Unclear(text) => {
                let normalized = self.normalize_text(text);
                format!("<unclear>{}</unclear>", self.escape_xml(&normalized))
            }
            //Same as above, Clippy is unhappy.
            Node::Gap { supplied, .. } => {
                match supplied {
                    Some(text) => {
                        let normalized = self.normalize_text(text);
                        format!("<supplied>{}</supplied>", self.escape_xml(&normalized))
                    }
                    None => String::new(),
                }
            }
            Node::Supplied(text) => {
                let normalized = self.normalize_text(text);
                format!("<supplied>{}</supplied>", self.escape_xml(&normalized))
            }
            Node::Deletion(text) => {
                let normalized = self.normalize_text(text);
                format!("<del>{}</del>", self.escape_xml(&normalized))
            }
            Node::Addition(text) => {
                let normalized = self.normalize_text(text);
                format!("<add>{}</add>", self.escape_xml(&normalized))
            }
            Node::Note(text) => {
                let normalized = self.normalize_text(text);
                format!("<note>{}</note>", self.escape_xml(&normalized))
            }
            Node::CompoundJoin => String::new(), // Join parts in normalized (no space)
            Node::LineBreak(_) | Node::PageBreak(_) => String::new(),
            _ => String::new(),
        }
    }

    fn normalize_text(&self, text: &str) -> String {
        if let Some(dict) = self.dictionary {
            dict.normalize_text(text)
        } else {
            text.to_string()
        }
    }

    fn escape_xml(&self, s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
}

impl Default for Compiler<'_> {
    fn default() -> Self {
        Self::new()
    }
}
