use super::ast::Node;
use super::compiler::{Compiler, CompilerConfig, LemmaMapping};
use super::lexer::Lexer;
use super::wordtokenizer::WordTokenizer;
use std::collections::HashMap;

// ============================================================================
// Lexer Tests
// ============================================================================

#[test]
fn test_lexer_plain_text() {
    let mut lexer = Lexer::new("hello world");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Text(t) if t == "hello world"));
}

#[test]
fn test_lexer_line_break_simple() {
    // Note: // consumes following non-whitespace as line number
    let mut lexer = Lexer::new("hello// world");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 3);
    assert!(matches!(&doc.nodes[0], Node::Text(t) if t == "hello"));
    assert!(matches!(&doc.nodes[1], Node::LineBreak(None)));
    assert!(matches!(&doc.nodes[2], Node::Text(t) if t == " world"));
}

#[test]
fn test_lexer_line_break_with_number() {
    let mut lexer = Lexer::new("hello//5 world");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 3);
    assert!(matches!(&doc.nodes[1], Node::LineBreak(Some(n)) if n == "5"));
}

#[test]
fn test_lexer_page_break() {
    let mut lexer = Lexer::new("hello///1r world");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 3);
    assert!(matches!(&doc.nodes[1], Node::PageBreak(n) if n == "1r"));
}

#[test]
fn test_lexer_abbreviation() {
    let mut lexer = Lexer::new(".abbr[dr]{doctor}");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Abbreviation { abbr, expansion } if abbr == "dr" && expansion == "doctor"));
}

#[test]
fn test_lexer_gap_without_quantity() {
    let mut lexer = Lexer::new("[...]");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Gap { quantity: None, supplied: None }));
}

#[test]
fn test_lexer_gap_with_quantity() {
    let mut lexer = Lexer::new("[...3]");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Gap { quantity: Some(3), supplied: None }));
}

#[test]
fn test_lexer_supplied() {
    let mut lexer = Lexer::new("<missing>");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Supplied(t) if t == "missing"));
}

#[test]
fn test_lexer_deletion() {
    let mut lexer = Lexer::new("-{removed}-");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Deletion(t) if t == "removed"));
}

#[test]
fn test_lexer_addition() {
    let mut lexer = Lexer::new("+{added}+");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Addition(t) if t == "added"));
}

#[test]
fn test_lexer_note() {
    let mut lexer = Lexer::new("^{margin note}");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Note(t) if t == "margin note"));
}

#[test]
fn test_lexer_unclear() {
    let mut lexer = Lexer::new("?{illegible}?");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Unclear(t) if t == "illegible"));
}

#[test]
fn test_lexer_entity() {
    let mut lexer = Lexer::new(":thorn:");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Entity(name) if name == "thorn"));
}

#[test]
fn test_lexer_word_continuation() {
    // Note: ~// consumes following non-whitespace as line number
    let mut lexer = Lexer::new("hel~// lo");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 4);
    assert!(matches!(&doc.nodes[0], Node::Text(t) if t == "hel"));
    assert!(matches!(&doc.nodes[1], Node::WordContinuation));
    assert!(matches!(&doc.nodes[2], Node::LineBreak(None)));
    assert!(matches!(&doc.nodes[3], Node::Text(t) if t == " lo"));
}

#[test]
fn test_lexer_word_boundary() {
    let mut lexer = Lexer::new("word1|word2");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 3);
    assert!(matches!(&doc.nodes[1], Node::WordBoundary));
}

#[test]
fn test_lexer_complex_input() {
    let input = "//1 :eth:is is .abbr[a]{abbrev} test// with [...] gaps";
    let mut lexer = Lexer::new(input);
    let doc = lexer.parse().unwrap();
    // Should have: LineBreak, Entity, Text, Abbreviation, Text, LineBreak, Text, Gap, Text
    assert!(doc.nodes.len() >= 5);
}

#[test]
fn test_lexer_utf8_characters() {
    // Test that multi-byte UTF-8 characters don't cause panics
    let mut lexer = Lexer::new("Þörður með öðrum");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Text(t) if t == "Þörður með öðrum"));
}

#[test]
fn test_lexer_utf8_with_constructs() {
    // Test UTF-8 mixed with DSL constructs
    let mut lexer = Lexer::new("hér er .abbr[skáld]{skáldskapur} og <ævintýri>");
    let doc = lexer.parse().unwrap();
    assert!(doc.nodes.len() >= 4); // Text, Abbreviation, Text, Supplied
    assert!(matches!(&doc.nodes[0], Node::Text(t) if t == "hér er "));
}

#[test]
fn test_lexer_utf8_in_brackets() {
    // Test UTF-8 inside bracketed content
    let mut lexer = Lexer::new(".abbr[þ]{þorn}");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Abbreviation { abbr, expansion } if abbr == "þ" && expansion == "þorn"));
}

// ============================================================================
// Word Tokenizer Tests
// ============================================================================

#[test]
fn test_word_tokenizer_simple_words() {
    let tokenizer = WordTokenizer::new();
    let nodes = vec![Node::Text("hello world".to_string())];
    let result = tokenizer.tokenize(nodes);

    // Should produce 2 Word nodes
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Node::Word(_)));
    assert!(matches!(&result[1], Node::Word(_)));
}

#[test]
fn test_word_tokenizer_punctuation() {
    let tokenizer = WordTokenizer::new();
    let nodes = vec![Node::Text("hello, world.".to_string())];
    let result = tokenizer.tokenize(nodes);

    // Should produce: Word(hello), Punctuation(,), Word(world), Punctuation(.)
    assert_eq!(result.len(), 4);
    assert!(matches!(&result[0], Node::Word(_)));
    assert!(matches!(&result[1], Node::Punctuation(_)));
    assert!(matches!(&result[2], Node::Word(_)));
    assert!(matches!(&result[3], Node::Punctuation(_)));
}

#[test]
fn test_word_tokenizer_with_line_break() {
    let tokenizer = WordTokenizer::new();
    let nodes = vec![
        Node::Text("hello".to_string()),
        Node::LineBreak(None),
        Node::Text(" world".to_string()),
    ];
    let result = tokenizer.tokenize(nodes);

    // "hello" + LineBreak in one word (heuristic), then "world" in another
    // Result: Word(hello, LineBreak), Word(world)
    assert_eq!(result.len(), 2);
}

#[test]
fn test_word_tokenizer_explicit_continuation() {
    let tokenizer = WordTokenizer::new();
    let nodes = vec![
        Node::Text("hel".to_string()),
        Node::WordContinuation,
        Node::LineBreak(None),
        Node::Text("lo".to_string()),
    ];
    let result = tokenizer.tokenize(nodes);

    // Should produce one word containing all parts
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], Node::Word(children) if children.len() >= 2));
}

// ============================================================================
// Compiler Tests
// ============================================================================

#[test]
fn test_compiler_plain_text() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("hello world").unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn test_compiler_line_break() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("hello// world").unwrap();
    assert!(result.contains("<lb/>\n"));
}

#[test]
fn test_compiler_line_break_with_number() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("hello//5 world").unwrap();
    assert!(result.contains("<lb n=\"5\"/>"));
}

#[test]
fn test_compiler_auto_line_numbers() {
    let config = CompilerConfig {
        word_wrap: false,
        auto_line_numbers: true,
        multi_level: false,
        wrap_pages: false,
    };
    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile("line1// line2// line3").unwrap();
    assert!(result.contains("<lb n=\"1\"/>"));
    assert!(result.contains("<lb n=\"2\"/>"));
}

#[test]
fn test_compiler_page_break() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("hello///1r world").unwrap();
    assert!(result.contains("<pb n=\"1r\"/>"));
}

#[test]
fn test_compiler_abbreviation() {
    let mut compiler = Compiler::new();
    let result = compiler.compile(".abbr[dr]{doctor}").unwrap();
    assert!(result.contains("<choice><abbr>dr</abbr><expan>doctor</expan></choice>"));
}

#[test]
fn test_compiler_gap() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("[...3]").unwrap();
    assert!(result.contains("<gap reason=\"illegible\" quantity=\"3\" unit=\"chars\"/>"));
}

#[test]
fn test_compiler_supplied() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("<missing>").unwrap();
    assert!(result.contains("<supplied>missing</supplied>"));
}

#[test]
fn test_compiler_deletion() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("-{removed}-").unwrap();
    assert!(result.contains("<del>removed</del>"));
}

#[test]
fn test_compiler_addition() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("+{added}+").unwrap();
    assert!(result.contains("<add>added</add>"));
}

#[test]
fn test_compiler_note() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("^{note}").unwrap();
    assert!(result.contains("<note>note</note>"));
}

#[test]
fn test_compiler_unclear() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("?{unclear}?").unwrap();
    assert!(result.contains("<unclear>unclear</unclear>"));
}

#[test]
fn test_compiler_entity() {
    let mut compiler = Compiler::new();
    let result = compiler.compile(":thorn:").unwrap();
    assert_eq!(result, "&thorn;");
}

#[test]
fn test_compiler_word_wrap() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
    };
    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile("hello world").unwrap();
    assert!(result.contains("<w>hello</w>"));
    assert!(result.contains("<w>world</w>"));
}

#[test]
fn test_compiler_punctuation_wrap() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
    };
    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile("hello, world.").unwrap();
    assert!(result.contains("<w>hello</w>"));
    assert!(result.contains("<pc>,</pc>"));
    assert!(result.contains("<w>world</w>"));
    assert!(result.contains("<pc>.</pc>"));
}

#[test]
fn test_compiler_xml_escaping() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("<a & b>").unwrap();
    // The <a & b> should be parsed as supplied text, so inner & is escaped
    assert!(result.contains("&amp;"));
}

#[test]
fn test_compiler_newlines_in_output() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
    };
    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile("hello world").unwrap();
    // Each <w> should be followed by newline
    assert!(result.contains("</w>\n"));
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_full_pipeline_menota_style() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: true,
        multi_level: false,
        wrap_pages: false,
    };
    let mut compiler = Compiler::new().with_config(config);

    let input = "//1 :eth:at is .abbr[go:d:]{good}// word";
    let result = compiler.compile(input).unwrap();

    // Should have line breaks with numbers
    assert!(result.contains("<lb n=\"1\"/>"));
    assert!(result.contains("<lb n=\"2\"/>"));
    // Should have entity reference
    assert!(result.contains("&eth;"));
    // Should have abbreviation
    assert!(result.contains("<choice>"));
    // Should have word wrapping
    assert!(result.contains("<w>"));
}

// ============================================================================
// Multi-Level Tests
// ============================================================================

#[test]
fn test_lexer_gap_with_supplied() {
    let mut lexer = Lexer::new("[...<missing>]");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Gap { quantity: None, supplied: Some(s) } if s == "missing"));
}

#[test]
fn test_lexer_gap_with_quantity_and_supplied() {
    let mut lexer = Lexer::new("[...3<abc>]");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 1);
    assert!(matches!(&doc.nodes[0], Node::Gap { quantity: Some(3), supplied: Some(s) } if s == "abc"));
}

#[test]
fn test_compiler_gap_with_supplied() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("[...<text>]").unwrap();
    assert!(result.contains("<gap reason=\"illegible\"/>"));
    assert!(result.contains("<supplied>text</supplied>"));
}

#[test]
fn test_compiler_gap_with_quantity_and_supplied() {
    let mut compiler = Compiler::new();
    let result = compiler.compile("[...5<lost>]").unwrap();
    assert!(result.contains("<gap reason=\"illegible\" quantity=\"5\" unit=\"chars\"/>"));
    assert!(result.contains("<supplied>lost</supplied>"));
}

#[test]
fn test_compiler_multi_level_word() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: true,
        wrap_pages: false,
    };
    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile("hello").unwrap();

    // Should have nested levels inside <w>
    assert!(result.contains("<w>"));
    assert!(result.contains("<me:facs>"));
    assert!(result.contains("<me:dipl>"));
    assert!(result.contains("<me:norm>"));
    assert!(result.contains("</w>"));
}

#[test]
fn test_compiler_multi_level_punctuation() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: true,
        wrap_pages: false,
    };
    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile(",").unwrap();

    // Should have nested levels inside <pc>
    assert!(result.contains("<pc>"));
    assert!(result.contains("<me:facs>"));
    assert!(result.contains("<me:dipl>"));
    assert!(result.contains("<me:norm>"));
    assert!(result.contains("</pc>"));
}

#[test]
fn test_compiler_multi_level_abbreviation() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: true,
        wrap_pages: false,
    };
    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile(".abbr[dr]{doctor}").unwrap();

    // Facsimile shows abbreviated form wrapped in <abbr>
    assert!(result.contains("<me:facs><abbr>dr</abbr></me:facs>"));
    // Diplomatic and normalized show expansion wrapped in <expan>
    assert!(result.contains("<me:dipl><expan>doctor</expan></me:dipl>"));
    assert!(result.contains("<me:norm><expan>doctor</expan></me:norm>"));
}

#[test]
fn test_compiler_multi_level_entity() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: true,
        wrap_pages: false,
    };
    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile(":eth:").unwrap();

    // Facsimile shows entity reference
    assert!(result.contains("<me:facs>&eth;</me:facs>"));
    // Without entities registry, diplomatic and normalized fall back to entity reference
    assert!(result.contains("<me:dipl>&eth;</me:dipl>"));
    assert!(result.contains("<me:norm>&eth;</me:norm>"));
}

// ============================================================================
// Lemma Attribute Tests
// ============================================================================

#[test]
fn test_compiler_lemma_attributes_single_level() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
    };
    let mut mappings = HashMap::new();
    // Now keyed by word INDEX (0 = first word)
    mappings.insert(
        0u32,
        LemmaMapping {
            lemma: "kona".to_string(),
            msa: "xNC cN nP gF".to_string(),
            normalized: None,
        },
    );

    let mut compiler = Compiler::new()
        .with_config(config)
        .with_lemma_mappings(mappings);
    let result = compiler.compile("konur").unwrap();

    // Should have lemma attributes
    assert!(result.contains(r#"lemma="kona""#));
    assert!(result.contains(r#"me:msa="xNC cN nP gF""#));
}

#[test]
fn test_compiler_lemma_attributes_multi_level() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: true,
        wrap_pages: false,
    };
    let mut mappings = HashMap::new();
    // Now keyed by word INDEX (0 = first word)
    mappings.insert(
        0u32,
        LemmaMapping {
            lemma: "vera".to_string(),
            msa: "xVB fF p3 nS tPT mIN vA".to_string(),
            normalized: None,
        },
    );

    let mut compiler = Compiler::new()
        .with_config(config)
        .with_lemma_mappings(mappings);
    let result = compiler.compile("var").unwrap();

    // Should have lemma/msa attributes on <w>
    assert!(result.contains(r#"lemma="vera""#));
    assert!(result.contains(r#"me:msa="xVB fF p3 nS tPT mIN vA""#));
    assert!(result.contains("<me:facs>var</me:facs>"));
}

#[test]
fn test_compiler_lemma_only_for_confirmed_index() {
    // Test that lemma is only applied to the SPECIFIC word index, not all matching wordforms
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
    };
    let mut mappings = HashMap::new();
    // Only confirm word at index 0 (first "kona")
    mappings.insert(
        0u32,
        LemmaMapping {
            lemma: "kona".to_string(),
            msa: "xNC cN nS gF".to_string(),
            normalized: None,
        },
    );

    let mut compiler = Compiler::new()
        .with_config(config)
        .with_lemma_mappings(mappings);
    // Two instances of "kona" - only first should have lemma
    let result = compiler.compile("kona kona").unwrap();

    // First word (index 0) should have lemma
    assert!(result.contains(r#"<w lemma="kona" me:msa="xNC cN nS gF">kona</w>"#));
    // Second word (index 1) should NOT have lemma
    assert!(result.contains("<w>kona</w>"));
}

#[test]
fn test_compiler_lemma_attributes_no_mapping() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
    };
    let mappings = HashMap::new(); // Empty mappings

    let mut compiler = Compiler::new()
        .with_config(config)
        .with_lemma_mappings(mappings);
    let result = compiler.compile("unknown").unwrap();

    // Should NOT have lemma or msa attributes
    assert!(result.contains("<w>unknown</w>"));
    assert!(!result.contains("lemma="));
    assert!(!result.contains("me:msa="));
}

#[test]
fn test_compiler_lemma_attributes_escaping() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
    };
    let mut mappings = HashMap::new();
    mappings.insert(
        0u32,
        LemmaMapping {
            lemma: "test & <special>".to_string(),
            msa: "xNC".to_string(),
            normalized: None,
        },
    );

    let mut compiler = Compiler::new()
        .with_config(config)
        .with_lemma_mappings(mappings);
    let result = compiler.compile("test").unwrap();

    // Special characters in lemma should be escaped
    assert!(result.contains("&amp;"));
    assert!(result.contains("&lt;"));
}

#[test]
fn test_lexer_compound_join() {
    let mut lexer = Lexer::new("upp~haf");
    let doc = lexer.parse().unwrap();
    assert_eq!(doc.nodes.len(), 3);
    assert!(matches!(&doc.nodes[0], Node::Text(t) if t == "upp"));
    assert!(matches!(&doc.nodes[1], Node::CompoundJoin));
    assert!(matches!(&doc.nodes[2], Node::Text(t) if t == "haf"));
}

#[test]
fn test_compiler_compound_join_single_level() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
    };

    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile("upp~haf").unwrap();

    // In single level, compound join shows space
    assert!(result.contains("<w>upp haf</w>"));
}

#[test]
fn test_compiler_compound_join_multi_level() {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: true,
        wrap_pages: false,
    };

    let mut compiler = Compiler::new().with_config(config);
    let result = compiler.compile("upp~haf").unwrap();

    // Facsimile and diplomatic show space, normalized joins
    assert!(result.contains("<me:facs>upp haf</me:facs>"));
    assert!(result.contains("<me:dipl>upp haf</me:dipl>"));
    assert!(result.contains("<me:norm>upphaf</me:norm>"));
}
