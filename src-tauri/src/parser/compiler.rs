use super::ast::Node;
use super::lexer::Lexer;
use super::wordtokenizer::WordTokenizer;
use crate::entities::EntityRegistry;
use crate::normalizer::LevelDictionary;

/// Configuration for the compiler
#[derive(Debug, Clone, Default)]
pub struct CompilerConfig {
    pub word_wrap: bool,
    pub auto_line_numbers: bool,
    pub multi_level: bool,
}

/// Compiles DSL input into TEI-XML
pub struct Compiler<'a> {
    entities: Option<&'a EntityRegistry>,
    dictionary: Option<&'a LevelDictionary>,
    config: CompilerConfig,
    line_number: u32,
}

impl<'a> Compiler<'a> {
    pub fn new() -> Self {
        Self {
            entities: None,
            dictionary: None,
            config: CompilerConfig::default(),
            line_number: 0,
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

        // Reset line counter for each compilation
        self.line_number = 0;

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
            format!("<w>{}</w>\n", content)
        }
    }

    fn compile_word_multi_level(&mut self, children: &[Node]) -> String {
        let facs = self.nodes_to_facs(children);
        let dipl = self.nodes_to_diplomatic(children);
        let norm = self.nodes_to_normalized(children);

        if facs.is_empty() && dipl.is_empty() && norm.is_empty() {
            String::new()
        } else {
            format!(
                "<w>\n  <me:facs>{}</me:facs>\n  <me:dipl>{}</me:dipl>\n  <me:norm>{}</me:norm>\n</w>\n",
                facs, dipl, norm
            )
        }
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
            Node::Abbreviation { abbr, .. } => self.escape_xml(abbr),
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
                }
                // Resolve entity to character
                if let Some(registry) = self.entities {
                    if let Some(entity) = registry.get(name) {
                        return self.escape_xml(&entity.char);
                    }
                }
                // Fallback to entity reference
                format!("&{};", name)
            }
            Node::Abbreviation { expansion, .. } => self.escape_xml(expansion),
            Node::Unclear(text) => format!("<unclear>{}</unclear>", self.escape_xml(text)),
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
                }
                // Resolve and normalize
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
                self.escape_xml(&normalized)
            }
            Node::Unclear(text) => {
                let normalized = self.normalize_text(text);
                format!("<unclear>{}</unclear>", self.escape_xml(&normalized))
            }
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
