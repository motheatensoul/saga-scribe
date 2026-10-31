//! # DSL Compiler
//!
//! This module transforms the parsed AST into TEI-XML output.
//!
//! ## Compilation Modes
//!
//! The compiler supports two output modes:
//!
//! ### Single-Level Mode (default)
//! Generates standard TEI-XML with `<w>` elements for words.
//! Suitable for simple transcriptions without diplomatic/normalized variants.
//!
//! ### Multi-Level Mode (`multi_level: true`)
//! Generates MENOTA-compliant three-level transcriptions:
//! - **Facsimile (`<me:facs>`)**: Exact manuscript representation with entity references
//! - **Diplomatic (`<me:dipl>`)**: Resolved entities, expanded abbreviations
//! - **Normalized (`<me:norm>`)**: Modern orthographic normalization
//!
//! ## Word Annotation Integration
//!
//! The compiler integrates with the annotation system to add:
//! - Lemma attributes (`lemma`, `me:msa`) from lemma mappings
//! - Semantic analysis (`@ana`) from annotations
//! - Character-level tags (`<c type="initial">`) for paleographic markup
//! - Inline notes from word annotations

use super::ast::Node;
use super::lexer::Lexer;
use super::wordtokenizer::WordTokenizer;
use crate::annotations::AnnotationSet;
use crate::entities::EntityRegistry;
use crate::normalizer::LevelDictionary;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A word of a compiled document: the ID of the DSL token it comes from (see
/// [`crate::identity`]) and its position among that token's words.
pub type WordKey = (u64, u32);

/// Configuration options for the DSL compiler.
///
/// Controls output format, word wrapping, and multi-level MENOTA support.
#[derive(Debug, Clone, Default)]
pub struct CompilerConfig {
    /// Wrap text content in `<w>` (word) and `<pc>` (punctuation) elements.
    /// Required for multi-level mode and lemmatization support.
    pub word_wrap: bool,
    /// Automatically number line breaks if no explicit number is provided.
    /// When true, `//` becomes `<lb n="1"/>`, `<lb n="2"/>`, etc.
    pub auto_line_numbers: bool,
    /// Generate MENOTA three-level transcription (`<me:facs>`, `<me:dipl>`, `<me:norm>`).
    /// Requires `word_wrap: true` to function correctly.
    pub multi_level: bool,
    /// Wrap page content in `<p>` tags for TEI structural compliance.
    /// TEI requires text to be inside structural elements like `<p>` or `<ab>`.
    pub wrap_pages: bool,
}

/// A lemmatization mapping for a word token.
///
/// Associates a word instance with its dictionary lemma and morphosyntactic analysis.
/// These mappings are stored by word index to support confirmed instance-level annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LemmaMapping {
    /// The dictionary headword (e.g., "maðr" for "manni")
    pub lemma: String,
    /// Morphosyntactic analysis tag (e.g., "nmsn" for noun-masculine-singular-nominative)
    pub msa: String,
    /// Optional user-provided normalized form (overrides auto-normalization)
    #[serde(default)]
    pub normalized: Option<String>,
}

/// Compiles DSL input into TEI-XML output.
///
/// The compiler processes an AST (optionally word-tokenized) and generates TEI-XML.
/// It supports single-level and multi-level (MENOTA) output modes.
///
/// # Builder Pattern
///
/// Use the builder methods to configure the compiler before calling [`compile`]:
///
/// ```rust,ignore
/// let mut compiler = Compiler::new()
///     .with_entities(&entity_registry)
///     .with_dictionary(&level_dictionary)
///     .with_lemma_mappings(lemmas)
///     .with_annotations(&annotations)
///     .with_config(config);
/// ```
pub struct Compiler<'a> {
    /// Entity registry for resolving `:entity:` references
    entities: Option<&'a EntityRegistry>,
    /// Level dictionary for character normalization and combining mark handling
    dictionary: Option<&'a LevelDictionary>,
    /// Lemma mappings indexed by word position (0-based)
    lemma_mappings: HashMap<u32, LemmaMapping>,
    /// Full annotation set for semantic, paleographic, and note annotations
    annotations: Option<&'a AnnotationSet>,
    /// Compiler configuration options
    config: CompilerConfig,
    /// Running line number counter (for auto_line_numbers)
    line_number: u32,
    /// Running word index counter (reset per compilation)
    word_index: u32,
    /// Token IDs (see [`crate::identity`]) of the DSL being compiled, by token
    token_ids: Option<Vec<u64>>,
    /// Each word's number in the previous compile, by word key: annotations
    /// (numbered by the previous compile) are looked up through it
    previous_numbers: Option<HashMap<WordKey, u32>>,
    /// The key of every `<w>` written, in document order
    word_keys: Vec<WordKey>,
    /// The token being compiled, and how many of its words were written
    current_token: u64,
    token_words: u32,
    /// Whether a `<p>` is open: opened by a page break (wrap_pages mode) or by `.p`
    in_page_paragraph: bool,
}

impl<'a> Compiler<'a> {
    pub fn new() -> Self {
        Self {
            entities: None,
            dictionary: None,
            lemma_mappings: HashMap::new(),
            annotations: None,
            config: CompilerConfig::default(),
            line_number: 0,
            word_index: 0,
            token_ids: None,
            previous_numbers: None,
            word_keys: Vec::new(),
            current_token: 0,
            token_words: 0,
            in_page_paragraph: false,
        }
    }

    /// Keys the words written by the token they come from (`token_ids`, one
    /// per token of the DSL, see [`crate::identity`]). With
    /// `previous_numbers`, annotations and lemma mappings are taken to be
    /// numbered by the previous compile and follow their words; without,
    /// they are numbered by this one.
    pub fn with_word_identity(
        mut self,
        token_ids: Vec<u64>,
        previous_numbers: Option<HashMap<WordKey, u32>>,
    ) -> Self {
        self.token_ids = Some(token_ids);
        self.previous_numbers = previous_numbers;
        self
    }

    /// The key of every `<w>` of the last compile, in document order.
    pub fn word_keys(&self) -> &[WordKey] {
        &self.word_keys
    }

    /// Numbers the next word written, returning the number its annotations
    /// are stored under (`u32::MAX` for a word that is new since the
    /// previous compile).
    fn claim_word(&mut self) -> u32 {
        let key = (self.current_token, self.token_words);
        self.token_words += 1;
        self.word_keys.push(key);
        let number = self.word_index;
        self.word_index += 1;
        match &self.previous_numbers {
            Some(previous) => previous.get(&key).copied().unwrap_or(u32::MAX),
            None => number,
        }
    }

    /// Takes back the last [`claim_word`](Self::claim_word), for a word that
    /// turned out empty and is not written.
    fn unclaim_word(&mut self) {
        self.word_keys.pop();
        self.word_index -= 1;
        self.token_words -= 1;
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

    /// Set full annotation set (for non-lemma annotations)
    pub fn with_annotations(mut self, annotations: &'a AnnotationSet) -> Self {
        self.annotations = Some(annotations);
        self
    }

    pub fn with_config(mut self, config: CompilerConfig) -> Self {
        self.config = config;
        self
    }

    /// The active compiler configuration.
    pub fn config(&self) -> &CompilerConfig {
        &self.config
    }

    /// Compiles DSL input to TEI-XML.
    ///
    /// This is the main entry point. The compilation pipeline:
    /// 1. Parse DSL to AST via [`Lexer`]
    /// 2. Optionally tokenize into words via [`WordTokenizer`] (if `word_wrap` enabled)
    /// 3. Transform each AST node to XML via [`node_to_xml`]
    ///
    /// # Errors
    ///
    /// Returns an error if the DSL parsing fails (unclosed brackets, etc.)
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
        self.word_keys.clear();
        self.in_page_paragraph = false;
        let mut token_ordinal = 0usize;

        let mut output = String::new();
        // Where the open paragraph's content starts, to spot an empty one
        let mut paragraph_start = 0;
        for node in &nodes {
            // Words are keyed by the token they come from (as tracked for
            // this DSL); only word-wrapped compiles tokenize the same way
            if crate::identity::is_token(node) {
                self.current_token = match (&self.token_ids, self.config.word_wrap) {
                    (Some(ids), true) => ids.get(token_ordinal).copied().unwrap_or(u64::MAX),
                    _ => token_ordinal as u64,
                };
                self.token_words = 0;
                token_ordinal += 1;
            }
            match node {
                Node::ParagraphBreak => {
                    if !self.in_page_paragraph {
                        output.push_str("<p>\n");
                        self.in_page_paragraph = true;
                    } else if !output[paragraph_start..].trim().is_empty() {
                        output.push_str("</p>\n<p>\n");
                    }
                    // else: a paragraph just opened (e.g. by a page break) is used as is
                    paragraph_start = output.len();
                }
                Node::PageBreak(_) => {
                    output.push_str(&self.node_to_xml(node));
                    paragraph_start = output.len();
                }
                _ => output.push_str(&self.node_to_xml(node)),
            }
        }

        // Close the paragraph still open at the end
        if self.in_page_paragraph {
            output.push_str("</p>\n");
        }

        Ok(output)
    }

    fn node_to_xml(&mut self, node: &Node) -> String {
        match node {
            Node::Text(text) => self.escape_xml(text),
            // Paragraphs are opened and closed by `compile`; inside fragments
            // (block content, inserted words) a break has no meaning
            Node::ParagraphBreak => String::new(),
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
            Node::PageBreak(n) => {
                let mut result = String::new();
                if self.config.wrap_pages {
                    // Close previous page paragraph if open
                    if self.in_page_paragraph {
                        result.push_str("</p>\n");
                    }
                    // Output page break
                    result.push_str(&format!("<pb n=\"{}\"/>\n", self.escape_xml(n)));
                    // Start new page paragraph
                    result.push_str("<p>\n");
                    self.in_page_paragraph = true;
                } else {
                    result.push_str(&format!("<pb n=\"{}\"/>\n", self.escape_xml(n)));
                }
                result
            }
            Node::Abbreviation { abbr, expansion } => {
                let abbr = self.nodes_to_xml(&inline_nodes(abbr));
                let expansion = self.nodes_to_xml(&inline_nodes(expansion));
                format!("<choice><abbr>{}</abbr><expan>{}</expan></choice>", abbr, expansion)
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
            Node::SuppliedBlock(text) => {
                let content = self.compile_fragment_from_dsl(text);
                format!("<supplied>{}</supplied>", content)
            }
            Node::Deletion(text) => format!("<del>{}</del>", self.escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", self.escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", self.escape_xml(text)),
            Node::Head(text) => {
                let content = self.compile_fragment_from_dsl(text);
                format!("<head>{}</head>", content)
            }
            Node::Norm(text) => self.compile_normalized_fragment(text),
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
        // Try to resolve entity to its character value if registry is available.
        // This produces valid XML without requiring entity definitions in a DTD.
        if let Some(registry) = self.entities {
            if let Some(entity) = registry.get(name) {
                return self.escape_xml(&entity.char);
            }
        }
        // Fallback to entity reference (requires DTD definition to be valid)
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
        let children = self.number_word_breaks(children);
        let mut content = String::new();
        for child in &children {
            match self.word_break_tag(child) {
                Some(tag) => content.push_str(&tag),
                None => content.push_str(&self.node_to_xml(child)),
            }
        }
        if content.is_empty() {
            String::new()
        } else {
            let current_index = self.claim_word();

            // Lookup by word INDEX (only confirmed instances have mappings)
            let lemma_attrs = self.get_lemma_attributes_by_index(current_index);
            let ann_attrs = self.get_annotation_attributes(current_index);
            let notes = self.get_note_elements(current_index);

            format!("<w{}{}>{}{}</w>\n", lemma_attrs, ann_attrs, content, notes)
        }
    }

    /// Compiles a word to MENOTA three-level format.
    ///
    /// Generates separate representations for each transcription level:
    /// - **Facsimile**: Entity references preserved, abbreviation forms only
    /// - **Diplomatic**: Entities resolved, abbreviations expanded, combining marks removed
    /// - **Normalized**: Full character normalization applied
    ///
    /// Also injects character-level `<c>` tags from paleographic annotations.
    fn compile_word_multi_level(&mut self, children: &[Node]) -> String {
        let children = &self.number_word_breaks(children);
        let facs = self.nodes_to_facs(children);
        let dipl = self.nodes_to_diplomatic(children);

        let current_index = self.claim_word();

        // Check if we have a user-provided normalized form (by index)
        let norm = if let Some(stored_norm) = self.get_stored_normalized_by_index(current_index) {
            self.escape_xml(&stored_norm) + &self.word_breaks_xml(children)
        } else {
            // Fall back to auto-generated normalization
            self.nodes_to_normalized(children)
        };

        if facs.is_empty() && dipl.is_empty() && norm.is_empty() {
            self.unclaim_word();
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

    /// Injects `<c>` tags for character annotations into compiled XML.
    ///
    /// This algorithm walks the XML string, maintaining a "visual index" that
    /// counts displayable characters while skipping over XML tags. Entity references
    /// like `&eth;` are treated as single characters to align with the frontend's
    /// visual character indexing.
    ///
    /// # Algorithm
    ///
    /// 1. Collect character annotations for the word (type=initial, capital, etc.)
    /// 2. Walk the XML string character by character:
    ///    - Skip `<...>` tag content (don't increment visual index)
    ///    - Treat `&...;` entities as single characters
    ///    - At each visual index, check if any annotation starts/ends
    ///    - Insert `<c type="...">` and `</c>` tags accordingly
    ///
    /// # Example
    ///
    /// For word "Maðr" with initial annotation on char 0:
    /// - Input: `M&eth;r`
    /// - Output: `<c type="initial">M</c>&eth;r`
    fn inject_character_tags(&self, xml: &str, word_index: u32) -> String {
        use crate::annotations::{AnnotationType, AnnotationValue, MenotaObservationType};
        
        let Some(ann_set) = self.annotations else {
            return xml.to_string();
        };

        // Collect character annotations for this word
        let mut char_anns = Vec::new();
        for ann in ann_set.for_word(word_index) {
            if let (
                AnnotationType::Paleographic,
                crate::annotations::AnnotationTarget::Character { char_start, char_end, .. },
                AnnotationValue::MenotaPaleographic { observation_type: MenotaObservationType::Character, char_type: Some(ctype), .. }
            ) = (&ann.annotation_type, &ann.target, &ann.value) {
                char_anns.push((*char_start, *char_end, ctype));
            }
        }

        if char_anns.is_empty() {
            return xml.to_string();
        }

        // Sort by start index
        char_anns.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));

        let mut result = String::with_capacity(xml.len() + char_anns.len() * 30);
        let mut text_idx = 0;
        let mut chars = xml.chars().peekable();
        
        while let Some(c) = chars.next() {
            // Check for tag start
            if c == '<' {
                result.push(c);
                // Consume until '>'
                for tc in chars.by_ref() {
                    result.push(tc);
                    if tc == '>' { break; }
                }
                continue;
            }

            // Check for entity start
            if c == '&' {
                // Collect entity
                let mut entity = String::from("&");
                while let Some(&ec) = chars.peek() {
                    chars.next();
                    entity.push(ec);
                    if ec == ';' { break; }
                }
                
                // Process entity as 1 text char
                self.process_char_injection(&mut result, &entity, text_idx, &char_anns);
                text_idx += 1;
                continue;
            }

            // Regular char
            self.process_char_injection(&mut result, &c.to_string(), text_idx, &char_anns);
            text_idx += 1;
        }

        // Close any tags that go beyond the text length (robustness)
        // We find annotations that were opened (start < text_idx) but not closed (end >= text_idx)
        let mut unclosed: Vec<_> = char_anns.iter()
            .filter(|(start, end, _)| *start < text_idx && *end >= text_idx)
            .collect();
        
        // Sort to close inner-most first (reverse of start)
        unclosed.sort_by_key(|a| std::cmp::Reverse(a.0));

        for _ in unclosed {
            result.push_str("</c>");
        }

        result
    }

    fn process_char_injection(
        &self, 
        result: &mut String, 
        content: &str, 
        text_idx: u32, 
        anns: &[(u32, u32, &crate::annotations::MenotaCharType)]
    ) {
        use crate::annotations::MenotaCharType;
        
        // Check for starts
        for (start, _end, ctype) in anns {
            if *start == text_idx {
                let type_str = match ctype {
                    MenotaCharType::Initial => "initial",
                    MenotaCharType::Capital => "capital",
                    MenotaCharType::Rubric => "rubric",
                    MenotaCharType::Colored => "colored",
                };
                result.push_str(&format!("<c type=\"{}\">", type_str));
            }
        }

        result.push_str(content);

        // Check for ends (inclusive end index)
        // Close in reverse order of opening to ensure proper XML nesting
        // (anns is sorted by start asc, so we iterate rev)
        for (_start, end, _) in anns.iter().rev() {
            if *end == text_idx {
                result.push_str("</c>");
            }
        }
    }
    fn get_lemma_attributes_by_index(&self, word_index: u32) -> String {
        // An empty lemma carries only a normalized form
        if let Some(mapping) = self.lemma_mappings.get(&word_index).filter(|m| !m.lemma.is_empty()) {
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

    /// Get additional attributes from annotations (semantic @ana, etc.)
    fn get_annotation_attributes(&self, word_index: u32) -> String {
        use crate::annotations::{AnnotationType, AnnotationValue, MenotaObservationType};

        let Some(ann_set) = self.annotations else {
            return String::new();
        };

        let mut attrs = String::new();
        let mut ana_values = Vec::new();

        for ann in ann_set.for_word(word_index) {
            match (&ann.annotation_type, &ann.value) {
                (AnnotationType::Semantic, AnnotationValue::Semantic { category, subcategory, .. }) => {
                    // Add semantic category to @ana attribute
                    let ana = if let Some(sub) = subcategory {
                        format!("#{}:{}", category, sub)
                    } else {
                        format!("#{}", category)
                    };
                    ana_values.push(ana);
                }
                (AnnotationType::Paleographic, AnnotationValue::Paleographic { observation_type, certainty, .. }) => {
                    use crate::annotations::PaleographicType;
                    // Add paleographic observation to @ana
                    let paleo_type = match observation_type {
                        PaleographicType::Unclear => "unclear",
                        PaleographicType::Damage => "damage",
                        PaleographicType::Erasure => "erasure",
                        PaleographicType::Letterform => "letterform",
                        PaleographicType::Abbreviation => "abbrev-mark",
                        PaleographicType::Correction => "correction",
                        PaleographicType::Addition => "addition",
                        PaleographicType::Decoration => "decoration",
                        PaleographicType::Other => "paleo",
                    };
                    ana_values.push(format!("#paleo:{}", paleo_type));

                    // Add certainty if specified
                    if let Some(cert) = certainty {
                        // TEI uses @cert with values "high", "medium", "low", or numeric
                        let cert_val = if *cert >= 0.8 {
                            "high"
                        } else if *cert >= 0.5 {
                            "medium"
                        } else {
                            "low"
                        };
                        attrs.push_str(&format!(" cert=\"{}\"", cert_val));
                    }
                }
                (AnnotationType::Paleographic, AnnotationValue::MenotaPaleographic { 
                    observation_type, 
                    unclear_reason,
                    add_place,
                    add_type,
                    hand,
                    del_rend,
                    supplied_reason,
                    resp,
                    char_type: _, // Ignored here, handled in nodes_to_facs via injection
                    certainty,
                    ..
                }) => {
                    // MENOTA-specific paleographic annotations with proper attributes
                    match observation_type {
                        MenotaObservationType::Unclear => {
                            ana_values.push("#unclear".to_string());
                            if let Some(reason) = unclear_reason {
                                attrs.push_str(&format!(" reason=\"{:?}\"", reason).to_lowercase());
                            }
                            if let Some(cert) = certainty {
                                let cert_val = if *cert >= 0.8 { "high" } else if *cert >= 0.5 { "medium" } else { "low" };
                                attrs.push_str(&format!(" cert=\"{}\"", cert_val));
                            }
                        }
                        MenotaObservationType::Addition => {
                            ana_values.push("#addition".to_string());
                            if let Some(place) = add_place {
                                attrs.push_str(&format!(" place=\"{:?}\"", place).to_lowercase().replace("_", "-"));
                            }
                            if let Some(add_t) = add_type {
                                attrs.push_str(&format!(" type=\"{:?}\"", add_t).to_lowercase());
                            }
                            if let Some(h) = hand {
                                attrs.push_str(&format!(" hand=\"{}\"", h));
                            }
                        }
                        MenotaObservationType::Deletion => {
                            ana_values.push("#deletion".to_string());
                            if let Some(rend) = del_rend {
                                attrs.push_str(&format!(" rend=\"{:?}\"", rend).to_lowercase());
                            }
                            if let Some(h) = hand {
                                attrs.push_str(&format!(" hand=\"{}\"", h));
                            }
                        }
                        MenotaObservationType::Supplied => {
                            ana_values.push("#supplied".to_string());
                            if let Some(reason) = supplied_reason {
                                attrs.push_str(&format!(" reason=\"{:?}\"", reason).to_lowercase());
                            }
                            if let Some(r) = resp {
                                attrs.push_str(&format!(" resp=\"{}\"", r));
                            }
                        }
                        MenotaObservationType::Character => {
                            // Handled in nodes_to_facs via injection of <c> tags
                        }
                    }
                }
                _ => {}
            }
        }

        if !ana_values.is_empty() {
            attrs.push_str(&format!(" ana=\"{}\"", ana_values.join(" ")));
        }

        attrs
    }

    /// Get note annotations as TEI <note> elements
    fn get_note_elements(&self, word_index: u32) -> String {
        use crate::annotations::{AnnotationType, AnnotationValue};

        let Some(ann_set) = self.annotations else {
            return String::new();
        };

        let mut notes = String::new();

        for ann in ann_set.for_word(word_index) {
            if let (AnnotationType::Note, AnnotationValue::Note { text, category }) =
                (&ann.annotation_type, &ann.value)
            {
                let type_attr = if let Some(cat) = category {
                    format!(" type=\"{}\"", self.escape_xml(cat))
                } else {
                    String::new()
                };
                notes.push_str(&format!(
                    "<note{}>{}</note>",
                    type_attr,
                    self.escape_xml(text)
                ));
            }
        }

        notes
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
                "<pc>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</pc>\n",
                facs, dipl, norm
            )
        }
    }

    /// Generates facsimile level content for MENOTA output.
    ///
    /// The facsimile level represents the manuscript exactly as it appears:
    /// - Entity references are preserved (e.g., `&eth;`)
    /// - Abbreviations show the abbreviated form only (e.g., `<abbr>w</abbr>`)
    /// - Supplied text is omitted (editor's additions not visible in facsimile)
    /// - Gaps show only the gap marker, not supplied readings
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
            Node::Abbreviation { abbr, .. } => {
                format!("<abbr>{}</abbr>", self.nodes_to_facs(&inline_nodes(abbr)))
            }
            Node::Unclear(text) => format!("<unclear>{}</unclear>", self.escape_xml(text)),
            Node::Gap { quantity, .. } => {
                // Facsimile shows gap only, not supplied
                match quantity {
                    Some(n) => format!("<gap reason=\"illegible\" quantity=\"{}\" unit=\"chars\"/>", n),
                    None => "<gap reason=\"illegible\"/>".to_string(),
                }
            }
            Node::Supplied(_) | Node::SuppliedBlock(_) | Node::Norm(_) => String::new(),
            Node::Deletion(text) => format!("<del>{}</del>", self.escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", self.escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", self.escape_xml(text)),
            Node::Head(_) => String::new(),
            Node::CompoundJoin => " ".to_string(), // Space in facsimile
            Node::LineBreak(_) | Node::PageBreak(_) => self.word_break_tag(node).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Generates diplomatic level content for MENOTA output.
    ///
    /// The diplomatic level is a readable interpretation of the manuscript:
    /// - Entities are resolved to their character values
    /// - Combining marks (abbreviation markers) are removed
    /// - Abbreviations are expanded (e.g., `<expan>world</expan>`)
    /// - Supplied text is shown with `<supplied>` tags
    fn nodes_to_diplomatic(&self, nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            output.push_str(&self.node_to_diplomatic(node));
        }
        output
    }

    // Telling clippy to shut up for now.
    #[allow(clippy::collapsible_match)]
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
            Node::Abbreviation { expansion, .. } => {
                format!("<expan>{}</expan>", self.nodes_to_diplomatic(&inline_nodes(expansion)))
            }
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
            Node::SuppliedBlock(_) | Node::Norm(_) => String::new(),
            Node::Deletion(text) => format!("<del>{}</del>", self.escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", self.escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", self.escape_xml(text)),
            Node::Head(_) => String::new(),
            Node::CompoundJoin => " ".to_string(), // Space in diplomatic
            Node::LineBreak(_) | Node::PageBreak(_) => self.word_break_tag(node).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Generates normalized level content for MENOTA output.
    ///
    /// The normalized level applies full orthographic normalization:
    /// - Character normalization (ð→d, þ→th, etc. per dictionary rules)
    /// - Entities resolved and normalized
    /// - Combining marks removed
    /// - Compound joins produce no space (upp~haf → upphaf)
    fn nodes_to_normalized(&self, nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            output.push_str(&self.node_to_normalized(node));
        }
        // Breaks inside a word go after the normalized form, as in MENOTA
        output.push_str(&self.word_breaks_xml(nodes));
        output
    }

    /// Gives unnumbered line breaks inside a word their automatic number, and
    /// advances the line counter past every line break, as `node_to_xml` does.
    fn number_word_breaks(&mut self, children: &[Node]) -> Vec<Node> {
        children
            .iter()
            .map(|child| match child {
                Node::LineBreak(n) => {
                    self.line_number += 1;
                    match n {
                        None if self.config.auto_line_numbers => {
                            Node::LineBreak(Some(self.line_number.to_string()))
                        }
                        _ => child.clone(),
                    }
                }
                _ => child.clone(),
            })
            .collect()
    }

    /// The tag for a line or page break inside a word: no trailing newline (it
    /// would become part of the word's text) and no paragraph wrapping.
    fn word_break_tag(&self, node: &Node) -> Option<String> {
        match node {
            Node::LineBreak(Some(n)) => Some(format!("<lb n=\"{}\"/>", self.escape_xml(n))),
            Node::LineBreak(None) => Some("<lb/>".to_string()),
            Node::PageBreak(n) => Some(format!("<pb n=\"{}\"/>", self.escape_xml(n))),
            _ => None,
        }
    }

    /// All breaks among a word's children, in order.
    fn word_breaks_xml(&self, children: &[Node]) -> String {
        children.iter().filter_map(|child| self.word_break_tag(child)).collect()
    }

    // Telling clippy to shut it
    #[allow(clippy::collapsible_match)]
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
                format!("<expan>{}</expan>", self.nodes_to_normalized(&inline_nodes(expansion)))
            }
            Node::Unclear(text) => {
                let normalized = self.normalize_text(text);
                format!("<unclear>{}</unclear>", self.escape_xml(&normalized))
            }
            Node::Gap { supplied, .. } => {
                let text = supplied.clone().unwrap_or_default();
                if text.is_empty() {
                    String::new()
                } else {
                    let normalized = self.normalize_text(&text);
                    format!("<supplied>{}</supplied>", self.escape_xml(&normalized))
                }
            }
            Node::Supplied(text) => {
                let normalized = self.normalize_text(text);
                format!("<supplied>{}</supplied>", self.escape_xml(&normalized))
            }
            Node::SuppliedBlock(_) | Node::Norm(_) => String::new(),
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
            Node::Head(_) => String::new(),
            Node::CompoundJoin => String::new(),
            // Appended after the word by `nodes_to_normalized`
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

    /// Compile a DSL fragment for a word element.
    /// Used by patching module to recompile modified words.
    /// The attributes parameter provides lemma, me:msa, etc. from the original word.
    ///
    /// `kept_norm` is the word's curated normalized form (XML content without
    /// breaks); when given, it is kept instead of being regenerated, and the
    /// edited word's breaks are appended to it.
    pub fn compile_word_from_dsl(
        &mut self,
        dsl: &str,
        attributes: &std::collections::HashMap<String, String>,
        kept_norm: Option<&str>,
    ) -> String {
        // Parse the DSL fragment
        let mut lexer = Lexer::new(dsl);
        let doc = match lexer.parse() {
            Ok(d) => d,
            Err(_) => return format!("<w><!-- parse error: {} --></w>\n", self.escape_xml(dsl)),
        };

        // Tokenize to get Word/Punctuation nodes
        let tokenizer = WordTokenizer::new();
        let nodes = tokenizer.tokenize(doc.nodes);

        // A single <w> may span several DSL words (e.g. "þo at"); each level
        // joins them with a space, mirroring how the importer reads them.
        let words: Vec<&Vec<Node>> = nodes
            .iter()
            .filter_map(|node| match node {
                Node::Word(children) => Some(children),
                _ => None,
            })
            .collect();

        if !words.is_empty() {
            // Format attributes from provided map
            let mut attr_str = String::new();
            // Sort keys for consistent output
            let mut keys: Vec<_> = attributes.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(value) = attributes.get(key) {
                    attr_str.push_str(&format!(" {}=\"{}\"", key, self.escape_xml(value)));
                }
            }

            if !self.config.multi_level {
                let content: Vec<String> = words.iter().map(|w| self.nodes_to_xml(w)).collect();
                return format!("<w{}>{}</w>\n", attr_str, content.join(" "));
            }

            // Generate the three levels
            let join = |level: Vec<String>| level.join(" ");
            let facs = join(words.iter().map(|w| self.nodes_to_facs(w)).collect());
            let dipl = join(words.iter().map(|w| self.nodes_to_diplomatic(w)).collect());
            let norm = match kept_norm {
                Some(norm) => {
                    let breaks: String = words.iter().map(|w| self.word_breaks_xml(w)).collect();
                    format!("{}{}", norm, breaks)
                }
                None => join(words.iter().map(|w| self.nodes_to_normalized(w)).collect()),
            };

            return format!(
                "<w{}>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</w>\n",
                attr_str, facs, dipl, norm
            );
        }

        // Fallback: if no Word node found, compile as plain text
        format!("<w><!-- no word content: {} --></w>\n", self.escape_xml(dsl))
    }

    /// Compile a DSL fragment for a punctuation element.
    /// Used by patching module to recompile modified punctuation.
    pub fn compile_punctuation_from_dsl(&mut self, dsl: &str) -> String {
        // Parse the DSL fragment
        let mut lexer = Lexer::new(dsl);
        let doc = match lexer.parse() {
            Ok(d) => d,
            Err(_) => return format!("<pc><!-- parse error: {} --></pc>\n", self.escape_xml(dsl)),
        };

        // Tokenize to get Word/Punctuation nodes
        let tokenizer = WordTokenizer::new();
        let nodes = tokenizer.tokenize(doc.nodes);

        // Find the first Punctuation node
        for node in &nodes {
            if let Node::Punctuation(children) = node {
                if !self.config.multi_level {
                    return format!("<pc>{}</pc>\n", self.nodes_to_xml(children));
                }

                // Generate the three levels for punctuation
                let facs = self.nodes_to_facs(children);
                let dipl = self.nodes_to_diplomatic(children);
                let norm = self.nodes_to_normalized(children);

                return format!(
                    "<pc>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</pc>\n",
                    facs, dipl, norm
                );
            }
        }

        // Fallback: compile as plain punctuation
        format!("<pc>{}</pc>\n", self.escape_xml(dsl))
    }

    fn compile_normalized_fragment(&mut self, dsl: &str) -> String {
        if !self.config.multi_level {
            return String::new();
        }

        let mut lexer = Lexer::new(dsl);
        let doc = match lexer.parse() {
            Ok(d) => d,
            Err(_) => return format!("<!-- parse error: {} -->", self.escape_xml(dsl)),
        };

        let tokenizer = WordTokenizer::new();
        let nodes = tokenizer.tokenize(doc.nodes);

        let mut output = String::new();
        for node in &nodes {
            match node {
                Node::Word(children) => {
                    let norm = self.nodes_to_normalized(children);
                    if !norm.is_empty() {
                        // Numbered like any other word (not annotated: there
                        // is only a normalized form)
                        self.claim_word();
                        output.push_str(&format!(
                            "<w>\n  <choice>\n    <me:facs></me:facs>\n    <me:dipl></me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</w>\n",
                            norm
                        ));
                    }
                }
                Node::Punctuation(children) => {
                    let norm = self.nodes_to_normalized(children);
                    if !norm.is_empty() {
                        output.push_str(&format!(
                            "<pc>\n  <choice>\n    <me:facs></me:facs>\n    <me:dipl></me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</pc>\n",
                            norm
                        ));
                    }
                }
                Node::LineBreak(n) => match n {
                    Some(num) => output.push_str(&format!("<lb n=\"{}\"/>\n", self.escape_xml(num))),
                    None => output.push_str("<lb/>\n"),
                },
                Node::PageBreak(n) => {
                    output.push_str(&format!("<pb n=\"{}\"/>\n", self.escape_xml(n)));
                }
                Node::SuppliedBlock(text) => {
                    let content = self.compile_normalized_fragment(text);
                    output.push_str(&format!("<supplied>{}</supplied>", content));
                }
                Node::Norm(text) => {
                    output.push_str(&self.compile_normalized_fragment(text));
                }
                _ => {
                    output.push_str(&self.node_to_normalized(node));
                }
            }
        }

        output
    }

    /// Compiles a DSL fragment to XML (for insertions and inline content).
    ///
    /// Used by:
    /// - The patching module to compile newly inserted content
    /// - Block elements like `.head{content}` and `.supplied{content}`
    ///
    /// Its words are numbered and annotated like the main `compile()`'s.
    pub fn compile_fragment_from_dsl(&mut self, dsl: &str) -> String {
        // Parse the DSL fragment
        let mut lexer = Lexer::new(dsl);
        let doc = match lexer.parse() {
            Ok(d) => d,
            Err(_) => return format!("<!-- parse error: {} -->", self.escape_xml(dsl)),
        };

        // Tokenize to get Word/Punctuation nodes
        let tokenizer = WordTokenizer::new();
        let nodes = tokenizer.tokenize(doc.nodes);

        // Compile all nodes
        let mut output = String::new();
        for node in &nodes {
            match node {
                // Words are numbered (and annotated) like any other word
                Node::Word(children) if !self.config.multi_level => {
                    output.push_str(&self.compile_word(children));
                }
                Node::Punctuation(children) if !self.config.multi_level => {
                    output.push_str(&format!("<pc>{}</pc>\n", self.nodes_to_xml(children)));
                }
                Node::Word(children) => {
                    output.push_str(&self.compile_word(children));
                }
                Node::Punctuation(children) => {
                    let facs = self.nodes_to_facs(children);
                    let dipl = self.nodes_to_diplomatic(children);
                    let norm = self.nodes_to_normalized(children);

                    output.push_str(&format!(
                        "<pc>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</pc>\n",
                        facs, dipl, norm
                    ));
                }
                Node::LineBreak(n) => {
                    match n {
                        Some(num) => output.push_str(&format!("<lb n=\"{}\"/>\n", self.escape_xml(num))),
                        None => output.push_str("<lb/>\n"),
                    }
                }
                Node::PageBreak(n) => {
                    output.push_str(&format!("<pb n=\"{}\"/>\n", self.escape_xml(n)));
                }
                _ => {
                    // Other nodes (text, etc.) - compile directly
                    output.push_str(&self.node_to_xml(node));
                }
            }
        }

        output
    }

    /// Compiles a DSL fragment without word wrapping, for documents whose text
    /// is not wrapped in `<w>` elements.
    pub fn compile_inline_from_dsl(&mut self, dsl: &str) -> String {
        match Lexer::new(dsl).parse() {
            Ok(doc) => self.nodes_to_xml(&doc.nodes),
            Err(_) => format!("<!-- parse error: {} -->", self.escape_xml(dsl)),
        }
    }

    /// Compiles child nodes in single-level form, without tracking word indices.
    fn nodes_to_xml(&mut self, nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            output.push_str(&self.node_to_xml(node));
        }
        output
    }
}

/// The DSL inside a construct (`.abbr[k:bar:]`) as nodes, so its entities are
/// compiled like anywhere else; unparseable content stays text.
fn inline_nodes(dsl: &str) -> Vec<Node> {
    Lexer::new(dsl)
        .parse()
        .map(|doc| doc.nodes)
        .unwrap_or_else(|_| vec![Node::Text(dsl.to_string())])
}

impl Default for Compiler<'_> {
    fn default() -> Self {
        Self::new()
    }
}
