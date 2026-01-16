use super::ast::Node;
use super::lexer::Lexer;
use super::wordtokenizer::WordTokenizer;
use crate::annotations::AnnotationSet;
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
    /// Wrap page content in <p> tags (TEI requires content in structural elements)
    pub wrap_pages: bool,
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
    /// Full annotation set (for non-lemma annotations)
    annotations: Option<&'a AnnotationSet>,
    config: CompilerConfig,
    line_number: u32,
    /// Current word index counter
    word_index: u32,
    /// Whether we're currently inside a page <p> wrapper
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
            in_page_paragraph: false,
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

    /// Set full annotation set (for non-lemma annotations)
    pub fn with_annotations(mut self, annotations: &'a AnnotationSet) -> Self {
        self.annotations = Some(annotations);
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
        self.in_page_paragraph = false;

        let mut output = String::new();
        for node in &nodes {
            output.push_str(&self.node_to_xml(node));
        }

        // Close any open page paragraph at the end
        if self.config.wrap_pages && self.in_page_paragraph {
            output.push_str("</p>\n");
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
            let lemma_attrs = self.get_lemma_attributes_by_index(current_index);
            let ann_attrs = self.get_annotation_attributes(current_index);
            let notes = self.get_note_elements(current_index);

            format!("<w{}{}>{}{}</w>\n", lemma_attrs, ann_attrs, content, notes)
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

    /// Inject <c> tags for character annotations into an XML string
    /// Walks the XML string, skipping tags and treating entities as single chars
    /// to align with frontend visual indexing.
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
        unclosed.sort_by(|a, b| b.0.cmp(&a.0));

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
            Node::Supplied(_) | Node::SuppliedBlock(_) | Node::Norm(_) => String::new(),
            Node::Deletion(text) => format!("<del>{}</del>", self.escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", self.escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", self.escape_xml(text)),
            Node::Head(_) => String::new(),
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
            Node::SuppliedBlock(_) | Node::Norm(_) => String::new(),
            Node::Deletion(text) => format!("<del>{}</del>", self.escape_xml(text)),
            Node::Addition(text) => format!("<add>{}</add>", self.escape_xml(text)),
            Node::Note(text) => format!("<note>{}</note>", self.escape_xml(text)),
            Node::Head(_) => String::new(),
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
                let normalized = self.normalize_text(expansion);
                format!("<expan>{}</expan>", self.escape_xml(&normalized))
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
    pub fn compile_word_from_dsl(
        &mut self,
        dsl: &str,
        attributes: &std::collections::HashMap<String, String>,
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

        // Find the first Word node (there should only be one for a word fragment)
        for node in &nodes {
            if let Node::Word(children) = node {
                // Generate the three levels
                let facs = self.nodes_to_facs(children);
                let dipl = self.nodes_to_diplomatic(children);
                let norm = self.nodes_to_normalized(children);

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

                return format!(
                    "<w{}>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</w>\n",
                    attr_str, facs, dipl, norm
                );
            }
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

    /// Compile a DSL fragment (for insertions).
    /// Used by patching module to compile new content that isn't tied to an existing segment.
    /// Returns compiled XML for all words/punctuation in the fragment.
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
                Node::Word(children) => {
                    let facs = self.nodes_to_facs(children);
                    let dipl = self.nodes_to_diplomatic(children);
                    let norm = self.nodes_to_normalized(children);

                    if !facs.is_empty() || !dipl.is_empty() || !norm.is_empty() {
                        output.push_str(&format!(
                            "<w>\n  <choice>\n    <me:facs>{}</me:facs>\n    <me:dipl>{}</me:dipl>\n    <me:norm>{}</me:norm>\n  </choice>\n</w>\n",
                            facs, dipl, norm
                        ));
                    }
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
}

impl Default for Compiler<'_> {
    fn default() -> Self {
        Self::new()
    }
}
