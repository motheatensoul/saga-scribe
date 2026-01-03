use super::ast::Node;
use super::lexer::Lexer;
use super::wordtokenizer::WordTokenizer;
use crate::entities::EntityRegistry;

/// Configuration for the compiler
#[derive(Debug, Clone, Default)]
pub struct CompilerConfig {
    pub word_wrap: bool,
    pub auto_line_numbers: bool,
}

/// Compiles DSL input into TEI-XML
pub struct Compiler<'a> {
    entities: Option<&'a EntityRegistry>,
    config: CompilerConfig,
    line_number: u32,
}

impl<'a> Compiler<'a> {
    pub fn new() -> Self {
        Self {
            entities: None,
            config: CompilerConfig::default(),
            line_number: 0,
        }
    }

    pub fn with_entities(mut self, registry: &'a EntityRegistry) -> Self {
        self.entities = Some(registry);
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
            Node::Gap { quantity } => match quantity {
                Some(n) => {
                    format!("<gap reason=\"illegible\" quantity=\"{}\" unit=\"chars\"/>", n)
                }
                None => "<gap reason=\"illegible\"/>".to_string(),
            },
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

    fn compile_punctuation(&mut self, children: &[Node]) -> String {
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
