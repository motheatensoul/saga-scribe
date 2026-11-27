use super::tei::parse;
use crate::importer::tei::helpers;
use crate::importer::tei::patching::{
    apply_patches_and_reconstruct, compiler_config_for, compute_patches, render_body, Originals,
    PatchOperation, Patcher, RenderedBody,
};
use crate::identity::{Guidance, TextChange};
use crate::importer::tei::segments::Segment;
use crate::parser::{Compiler, CompilerConfig, Lexer};
use libxml::tree::NodeType;

// ============================================================================
// BASIC IMPORT TESTS (XML → DSL)
// ============================================================================

#[test]
fn test_import_lb() {
    let xml = "<body>line 1<lb/>line 2<lb n=\"5\"/>line 3</body>";
    let result = parse(xml).unwrap();
    // lb adds a newline BEFORE the marker
    assert_eq!(result.dsl, "line 1\n// line 2\n//5 line 3");
}

#[test]
fn test_import_pb() {
    let xml = "<body>page 1<pb/>page 2<pb n=\"10v\"/>page 3</body>";
    let result = parse(xml).unwrap();
    // pb adds a newline before the marker, and the text after it starts a new
    // line (glued, "///10vpage" would read as the page number)
    assert_eq!(result.dsl, "page 1\n///\npage 2\n///10v\npage 3");
}

#[test]
fn test_import_choice() {
    let xml = "<body>word <choice><abbr>a</abbr><expan>abbr</expan></choice> end</body>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl, "word .abbr[a]{abbr} end");
}

#[test]
fn test_import_menota_am_ex_abbreviation() {
    let xml = "<body xmlns:me=\"http://www.menota.org/ns/1.0\"><w><choice><me:facs>kn<am>¯</am>gr</me:facs><me:dipl>k<ex>ono</ex>ngr</me:dipl><me:norm>konungr</me:norm></choice></w></body>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl, ".abbr[kn¯gr]{konongr}");
}

#[test]
fn test_import_norm_only_punctuation() {
    let xml = "<body xmlns:me=\"http://www.menota.org/ns/1.0\"><pc><choice><me:norm>,</me:norm></choice></pc></body>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl, ".norm{,}");
}

#[test]
fn test_import_supplied_block() {
    let xml = "<body><supplied><p>Missing text</p></supplied></body>";
    let result = parse(xml).unwrap();
    assert!(result.dsl.contains(".supplied{Missing text}"));
}

#[test]
fn test_import_gap() {
    let xml = "<body>start <gap/> end</body>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl, "start [...] end");
}

#[test]
fn test_import_gap_with_supplied_quantity() {
    let xml = "<body><w><gap quantity=\"3\"/><supplied>abc</supplied></w></body>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl, "[...3<abc>]");
}

#[test]
fn test_import_supplied() {
    let xml = "<body>start <supplied>missing</supplied> end</body>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl, "start <missing> end");
}

#[test]
fn test_import_head() {
    let xml = "<body><head>Title</head><p>text</p></body>";
    let result = parse(xml).unwrap();
    assert!(result.dsl.contains(".head{Title}"));
}

#[test]
fn test_import_choice_facs() {
    let xml = "<body xmlns:me=\"http://www.menota.org/ns/1.0\"><w><choice><me:facs>foo</me:facs><me:dipl>bar</me:dipl></choice></w></body>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl, "foo");
}

#[test]
fn test_import_preserves_msa_attribute() {
    let xml = "<body xmlns:me=\"http://www.menota.org/ns/1.0\"><w lemma=\"land\" me:msa=\"xNC\">land</w></body>";
    let result = parse(xml).unwrap();
    let doc = result.imported_document.expect("Expected imported document");
    let word_attrs = doc
        .segments
        .iter()
        .find_map(|segment| match segment {
            Segment::Word { attributes, .. } => Some(attributes),
            _ => None,
        })
        .expect("Expected word segment");
    assert_eq!(word_attrs.get("me:msa"), Some(&"xNC".to_string()));
}

#[test]
fn test_import_del_add() {
    let xml = "<body><del>deleted</del><add>added</add></body>";
    let result = parse(xml).unwrap();
    // Two elements, two tokens: `|` keeps them apart
    assert_eq!(result.dsl, "-{deleted}-|+{added}+");
}

#[test]
fn test_import_complex() {
    let xml = "<TEI><text><body><p>Line 1<lb/>Line 2</p></body></text></TEI>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl, ".p Line 1\n// Line 2");
}

#[test]
fn test_import_note() {
    let xml = "<body>text <note>margin note</note> more text</body>";
    let result = parse(xml).unwrap();
    // <note> should be converted to ^{text} DSL syntax
    assert_eq!(result.dsl, "text ^{margin note} more text");
}

#[test]
fn test_import_note_inline() {
    let xml = "<body>word<note>editorial comment</note>next</body>";
    let result = parse(xml).unwrap();
    // Glued to the word, the note would be part of it (as in `<w>word<note>`)
    assert_eq!(result.dsl, "word ^{editorial comment}|next");
}

// ============================================================================
// ROUNDTRIP TESTS (DSL → XML → DSL)
// These tests verify that information is preserved through the compile/import cycle
// ============================================================================

/// Helper function to compile DSL to XML (single-level, no word wrap)
fn compile_dsl(dsl: &str) -> String {
    let config = CompilerConfig {
        word_wrap: false,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
        levels: None,
    };
    Compiler::new().with_config(config).compile(dsl).expect("Compilation should succeed")
}

/// Helper function to compile DSL to XML with word wrapping
#[allow(dead_code)]
fn compile_dsl_with_word_wrap(dsl: &str) -> String {
    let config = CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
        levels: None,
    };
    Compiler::new().with_config(config).compile(dsl).expect("Compilation should succeed")
}

/// Helper to wrap XML body content for import
fn wrap_body(content: &str) -> String {
    format!("<body>{}</body>", content)
}

#[test]
fn test_roundtrip_simple_text() {
    let original_dsl = "hello world";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    // Normalize whitespace for comparison
    let normalized_original: String = original_dsl.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized_result: String = result.dsl.split_whitespace().collect::<Vec<_>>().join(" ");
    
    assert_eq!(normalized_result, normalized_original, 
        "Simple text should roundtrip: '{}' → XML → '{}'", original_dsl, result.dsl);
}

#[test]
fn test_roundtrip_line_breaks() {
    let original_dsl = "line one\n// line two\n//3 line three";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    // Both should have the same line break markers
    assert!(result.dsl.contains("//"), "Should preserve line breaks");
    assert!(result.dsl.contains("//3"), "Should preserve numbered line breaks");
}

#[test]
fn test_roundtrip_page_breaks() {
    let original_dsl = "page one\n///2rpage two";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    assert!(result.dsl.contains("///2r"), "Should preserve page break with folio number");
}

#[test]
fn test_roundtrip_abbreviation() {
    let original_dsl = "word .abbr[þ]{þat} end";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    assert!(result.dsl.contains(".abbr["), "Should preserve abbreviation syntax");
    assert!(result.dsl.contains("]{"), "Should preserve abbreviation expansion");
}

#[test]
fn test_roundtrip_supplied() {
    let original_dsl = "text <missing> more";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    assert!(result.dsl.contains("<missing>"), "Should preserve supplied text");
}

#[test]
fn test_roundtrip_deletion() {
    let original_dsl = "text -{deleted}- more";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    assert!(result.dsl.contains("-{deleted}-"), "Should preserve deletion");
}

#[test]
fn test_roundtrip_addition() {
    let original_dsl = "text +{added}+ more";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    assert!(result.dsl.contains("+{added}+"), "Should preserve addition");
}

#[test]
fn test_whitespace_before_add_element() {
    // This is the specific issue causing feedback loop instability:
    // Space before <add> is lost on import
    let xml = "<body>word <add>added</add> more</body>";
    let result = parse(xml).unwrap();
    
    // The space between "word" and "+{added}+" must be preserved
    assert!(result.dsl.contains("word +{added}+"), 
        "Space before <add> must be preserved! Got: '{}'", result.dsl);
    assert!(result.dsl.contains("+{added}+ more"), 
        "Space after <add> must be preserved! Got: '{}'", result.dsl);
}

#[test]
fn test_whitespace_before_del_element() {
    let xml = "<body>word <del>deleted</del> more</body>";
    let result = parse(xml).unwrap();
    
    assert!(result.dsl.contains("word -{deleted}-"), 
        "Space before <del> must be preserved! Got: '{}'", result.dsl);
}

#[test]
fn test_whitespace_before_supplied_element() {
    let xml = "<body>word <supplied>supplied</supplied> more</body>";
    let result = parse(xml).unwrap();
    
    assert!(result.dsl.contains("word <supplied>"), 
        "Space before <supplied> must be preserved! Got: '{}'", result.dsl);
}

#[test]
fn test_multiple_add_elements_with_spaces() {
    // This simulates the real-world case from HolmPerg-34-4to-MLL.xml
    let xml = "<body>logretto <add>men</add> logretto <add>menn</add> lgréttumenn</body>";
    let result = parse(xml).unwrap();
    
    // Each space before <add> must be preserved
    assert!(result.dsl.contains("logretto +{men}+"), 
        "First space before <add> must be preserved! Got: '{}'", result.dsl);
    assert!(result.dsl.contains("logretto +{menn}+"), 
        "Second space before <add> must be preserved! Got: '{}'", result.dsl);
}

#[test]
fn test_roundtrip_note() {
    let original_dsl = "text ^{marginal note} more";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    assert!(result.dsl.contains("^{marginal note}"), "Should preserve note");
}

#[test]
fn test_roundtrip_gap_basic() {
    let original_dsl = "text [...] more";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    assert!(result.dsl.contains("[...]"), "Should preserve gap marker");
}

#[test]
fn test_roundtrip_complex_document() {
    let original_dsl = r#"///1r
//1First line with .abbr[þ]{þat} abbreviation
//2Second line with <supplied> text
//3Third line with -{deletion}- and +{addition}+
//4Fourth line with ^{marginal note}
///1v
//5 New page content"#;
    
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    // Check key features are preserved
    assert!(result.dsl.contains("///1r"), "Should preserve page break 1r");
    assert!(result.dsl.contains("///1v"), "Should preserve page break 1v");
    assert!(result.dsl.contains(".abbr["), "Should preserve abbreviation");
    assert!(result.dsl.contains("<supplied>"), "Should preserve supplied");
    assert!(result.dsl.contains("-{deletion}-"), "Should preserve deletion");
    assert!(result.dsl.contains("+{addition}+"), "Should preserve addition");
    assert!(result.dsl.contains("^{marginal note}"), "Should preserve note");
}

// ============================================================================
// ADDITIONAL ROUNDTRIP TESTS
// ============================================================================

#[test]
fn test_roundtrip_gap_with_quantity() {
    // Gap quantity attribute is preserved through roundtrip
    let original_dsl = "text [...3] more";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();

    assert!(result.dsl.contains("[...3]"),
        "Gap quantity should be preserved: got '{}'", result.dsl);
}

#[test]
fn test_roundtrip_unclear() {
    let original_dsl = "text ?{unclear}? more";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();
    
    // This SHOULD contain ?{unclear}?
    assert!(result.dsl.contains("?{unclear}?"), 
        "Unclear text should be preserved but isn't: got '{}'", result.dsl);
}

#[test]
fn test_roundtrip_unicode_characters() {
    // Tests that Unicode characters (including those often written as entities)
    // are preserved through the compile/import roundtrip.
    let original_dsl = "text ð þ æ more";
    let xml = compile_dsl(original_dsl);
    let wrapped = wrap_body(&xml);
    let result = parse(&wrapped).unwrap();

    // All Unicode characters should be preserved
    assert!(result.dsl.contains("ð"),
        "Character ð should be preserved: got '{}'", result.dsl);
    assert!(result.dsl.contains("þ"),
        "Character þ should be preserved: got '{}'", result.dsl);
    assert!(result.dsl.contains("æ"),
        "Character æ should be preserved: got '{}'", result.dsl);
}

#[test]
fn test_entity_syntax_compiles_to_character() {
    // Entity syntax :name: should compile to the Unicode character when
    // the entity registry is available. Without a registry, it falls back
    // to an XML entity reference which may not survive XML parsing.
    //
    // This test documents the behavior: entity syntax is a convenience
    // for input, the output is the resolved character.
    use crate::entities::EntityRegistry;

    // Load a minimal entity registry
    let mut registry = EntityRegistry::new();
    let entities_json = r#"{
        "version": "1.0",
        "name": "test",
        "entities": {
            "eth": {"unicode": "U+00F0", "char": "ð", "description": "Latin small letter eth"}
        }
    }"#;
    registry.load_from_str(entities_json).unwrap();

    let config = CompilerConfig {
        word_wrap: false,
        auto_line_numbers: false,
        multi_level: false,
        wrap_pages: false,
        levels: None,
    };
    let xml = Compiler::new()
        .with_config(config)
        .with_entities(&registry)
        .compile("text :eth: more")
        .unwrap();

    // The compiled XML should contain the character ð, not &eth;
    assert!(xml.contains("ð"), "Entity should resolve to character: got '{}'", xml);
    assert!(!xml.contains("&eth;"), "Should not contain unresolved entity ref");
}

// ============================================================================
// WORD ELEMENT TESTS
// Tests for <w> elements with lemma/msa attributes
// ============================================================================

#[test]
fn test_import_word_with_lemma() {
    // Word elements with lemma/msa attributes: text content goes to DSL,
    // lemma attributes are preserved in the Segment's attributes map
    // (see test_import_preserves_msa_attribute for attribute preservation)
    let xml = r#"<body><w lemma="maðr" me:msa="ncmsn">maðr</w></body>"#;
    let result = parse(xml).unwrap();

    // DSL contains the word text
    assert_eq!(result.dsl.trim(), "maðr");

    // Lemma attributes are preserved in the segment (tested separately)
}

#[test]
fn test_import_menota_multi_level() {
    // Multi-level MENOTA structure: facs level is extracted for DSL editing,
    // full multi-level structure is preserved in Segment's original_xml for round-trip
    let xml = r#"<body xmlns:me="http://www.menota.org/ns/1.0">
        <w lemma="maðr" me:msa="ncmsn">
            <me:facs>maðꝛ</me:facs>
            <me:dipl>maðr</me:dipl>
            <me:norm>maðr</me:norm>
        </w>
    </body>"#;
    let result = parse(xml).unwrap();

    // DSL contains the facsimile level text (with archaic character)
    assert!(result.dsl.contains("maðꝛ"),
        "Should extract facs level: got '{}'", result.dsl);
}

// ============================================================================
// CHARACTER ANNOTATION TESTS
// ============================================================================

#[test]
fn test_import_character_annotation() {
    // Character elements (<c>) with annotations: text content is extracted,
    // the type attribute would be preserved in segment metadata for round-trip
    let xml = r#"<body><c type="initial">M</c>aðr</body>"#;
    let result = parse(xml).unwrap();

    // The full text, as the two segments it is read as
    assert_eq!(result.dsl.trim(), "M|aðr");
}

#[test]
fn test_import_word_with_character_annotations_preserves_lemma() {
    // Words with <c> elements inside <me:facs> should still preserve lemma/msa attributes
    // This is the structure used in HolmPerg for decorated initials
    let xml = r#"<body xmlns:me="http://www.menota.org/ns/1.0">
        <w lemma="Magnús" me:msa="xNP gM nS cN sI">
            <choice>
                <me:facs><c type="initial">M</c><c type="littNot">A</c>gnus</me:facs>
                <me:dipl><c type="initial">M</c><c type="littNot">A</c>gnus</me:dipl>
                <me:norm>Magnús</me:norm>
            </choice>
        </w>
    </body>"#;
    let result = parse(xml).unwrap();
    let doc = result.imported_document.expect("Expected imported document");

    // Find the word segment
    let word_segment = doc
        .segments
        .iter()
        .find_map(|segment| match segment {
            Segment::Word { attributes, dsl_content, .. } => Some((attributes, dsl_content)),
            _ => None,
        })
        .expect("Expected word segment");

    let (attrs, dsl) = word_segment;

    // Lemma and msa should be preserved in attributes
    assert_eq!(attrs.get("lemma"), Some(&"Magnús".to_string()),
        "Lemma attribute should be preserved");
    assert_eq!(attrs.get("me:msa"), Some(&"xNP gM nS cN sI".to_string()),
        "me:msa attribute should be preserved");

    // DSL content should have the facs level text
    assert!(dsl.contains("MAgnus") || dsl.contains("M") && dsl.contains("gnus"),
        "DSL should contain facs level text: got '{}'", dsl);
}

#[test]
fn test_holmperg_first_words_have_lemma() {
    // Test that the first few words from HolmPerg are extracted with their lemma attributes
    let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/fixtures/HolmPerg-34-4to-MLL.xml");

    if !test_file_path.exists() {
        println!("Test file not found at {:?}, skipping", test_file_path);
        return;
    }

    let xml_content = std::fs::read_to_string(&test_file_path)
        .expect("Should read test file");

    let result = parse(&xml_content).unwrap();
    let doc = result.imported_document.expect("Expected imported document");

    // Collect all word segments with their attributes
    let words_with_lemma: Vec<_> = doc
        .segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Word { attributes, dsl_content, original_xml, .. } => {
                if original_xml.trim_start().starts_with("<w") {
                    Some((attributes.clone(), dsl_content.clone(), original_xml.clone()))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();

    // Print first 10 words for debugging
    println!("First 10 word segments:");
    for (i, (attrs, dsl, xml)) in words_with_lemma.iter().take(10).enumerate() {
        let lemma = attrs.get("lemma").map(|s| s.as_str()).unwrap_or("<none>");
        let msa = attrs.get("me:msa").map(|s| s.as_str()).unwrap_or("<none>");
        println!("  {}: lemma='{}', msa='{}', dsl='{}'", i, lemma, msa, dsl);
        if lemma == "<none>" {
            println!("    xml preview: {}", &xml.chars().take(100).collect::<String>());
        }
    }

    // Count words with lemma attributes
    let words_with_lemma_count = words_with_lemma
        .iter()
        .filter(|(attrs, _, _)| attrs.contains_key("lemma"))
        .count();

    println!("Words with lemma: {} / {}", words_with_lemma_count, words_with_lemma.len());

    // The file has heavily annotated words - most should have lemma attributes
    assert!(words_with_lemma_count > 0, "Should have at least some words with lemma attributes");

    // Check that specific words we know exist have their lemma
    // Look for "Magnús" which appears early in the text
    let magnus_word = words_with_lemma
        .iter()
        .find(|(attrs, _, _)| attrs.get("lemma") == Some(&"Magnús".to_string()));

    assert!(magnus_word.is_some(),
        "Should find a word with lemma='Magnús'. First 5 lemmas: {:?}",
        words_with_lemma.iter().take(5).map(|(a, _, _)| a.get("lemma")).collect::<Vec<_>>());
}

// ============================================================================
// ABBREVIATION MARKER TESTS
// ============================================================================

#[test]
fn test_import_am_ex_markers() {
    let xml = r#"<body>
        <choice>
            <abbr>þ<am>̃</am></abbr>
            <expan>þ<ex>at</ex></expan>
        </choice>
    </body>"#;
    let result = parse(xml).unwrap();

    // Should produce .abbr[þ̃]{þat} preserving the combining character
    assert_eq!(result.dsl.trim(), ".abbr[þ̃]{þat}");
}

fn normalize_xml(xml: &str) -> String {
    // Entities declared as the importer does, so they survive on both sides
    let doc = crate::xml::parse(xml).expect("Should parse XML for normalization");
    let root = doc.get_root_element().expect("Should have root element");
    serialize_node_sorted(&root)
}

fn serialize_node_sorted(node: &libxml::tree::Node) -> String {
    match node.get_type() {
        Some(NodeType::ElementNode) => {
            let name = helpers::qualified_name(node);
            let mut output = String::new();
            output.push('<');
            output.push_str(&name);

            let mut attrs: Vec<_> = node.get_attributes().into_iter().collect();
            attrs.sort_by(|a, b| a.0.cmp(&b.0));
            for (key, value) in attrs {
                output.push(' ');
                output.push_str(&key);
                output.push_str("=\"");
                output.push_str(&helpers::escape_xml_attr(&value));
                output.push('"');
            }

            let first_child = node.get_first_child();
            if first_child.is_none() {
                output.push_str("/>");
            } else {
                output.push('>');
                let mut child = first_child;
                while let Some(c) = child {
                    output.push_str(&serialize_node_sorted(&c));
                    child = c.get_next_sibling();
                }
                output.push_str("</");
                output.push_str(&name);
                output.push('>');
            }

            output
        }
        Some(NodeType::TextNode) => {
            let content = node.get_content();
            if content.trim().is_empty() {
                String::new()
            } else {
                helpers::escape_xml_text(&content)
            }
        }
        Some(NodeType::CommentNode) => {
            format!("<!--{}-->", node.get_content())
        }
        Some(NodeType::CDataSectionNode) => {
            format!("<![CDATA[{}]]>", node.get_content())
        }
        Some(NodeType::EntityRefNode) => {
            let name = node.get_name();
            if !name.is_empty() {
                format!("&{};", name)
            } else {
                helpers::escape_xml_text(&node.get_content())
            }
        }
        _ => String::new(),
    }
}

// ============================================================================
// REAL MENOTA FILE INTEGRATION TEST
// ============================================================================

#[test]
fn test_menota_file_roundtrip_stability() {
    // Load the MENOTA test file
    let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/fixtures/HolmPerg-34-4to-MLL.xml");

    if !test_file_path.exists() {
        println!("Test file not found at {:?}, skipping", test_file_path);
        return;
    }

    let xml_content = std::fs::read_to_string(&test_file_path)
        .expect("Should read test file");

    // Import once
    let import_result = parse(&xml_content).unwrap();
    let imported_doc = import_result
        .imported_document
        .expect("Imported document manifest should exist");

    // Roundtrip using the imported patching path
    let patches = compute_patches(&imported_doc.segments, imported_doc.dsl_rules, &import_result.dsl).unwrap();
    let mut compiler = Compiler::new().with_config(CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: true,
        wrap_pages: false,
        levels: None,
    });
    let reconstructed_body =
        apply_patches_and_reconstruct(&imported_doc.segments, &patches, &mut compiler);
    let reconstructed_xml = format!(
        "{}{}{}",
        import_result.original_preamble.unwrap_or_default(),
        reconstructed_body,
        import_result.original_postamble.unwrap_or_default()
    );

    // Re-import and ensure the DSL is stable after one roundtrip
    let dsl2 = parse(&reconstructed_xml).unwrap().dsl;

    assert_eq!(
        import_result.dsl, dsl2,
        "Imported DSL should stabilize after one roundtrip cycle"
    );
}


#[test]
fn test_load_menota_test_file() {
    let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/fixtures/HolmPerg-34-4to-MLL.xml");

    if !test_file_path.exists() {
        println!("Test file not found at {:?}, skipping", test_file_path);
        return;
    }

    let xml_content = std::fs::read_to_string(&test_file_path)
        .expect("Should read test file");

    let result = parse(&xml_content);

    assert!(result.is_ok(), "Should successfully parse MENOTA file: {:?}", result.err());

    let import_result = result.unwrap();

    // Verify metadata was extracted
    assert!(import_result.metadata.is_some(), "Should extract metadata from teiHeader");
    let metadata = import_result.metadata.unwrap();

    // Check some expected metadata fields
    assert!(metadata.title_stmt.title.is_some(), "Should have a title");
    println!("Imported title: {:?}", metadata.title_stmt.title);

    // Verify DSL content was extracted
    assert!(!import_result.dsl.is_empty(), "Should extract DSL content from body");

    // The DSL should contain page breaks from the manuscript
    // (The test file should have <pb> elements)
    println!("DSL length: {} chars", import_result.dsl.len());
    println!("First 500 chars of DSL:\n{}", &import_result.dsl.chars().take(500).collect::<String>());
}


#[test]
fn test_menota_file_imported_roundtrip_xml() {
    // Verify imported patching reconstructs semantically identical XML
    let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/fixtures/HolmPerg-34-4to-MLL.xml");

    if !test_file_path.exists() {
        println!("Test file not found, skipping imported roundtrip test");
        return;
    }

    let xml_content = std::fs::read_to_string(&test_file_path)
        .expect("Should read test file");

    let import_result = parse(&xml_content).expect("Import should succeed");
    let imported_doc = import_result
        .imported_document
        .expect("Imported document manifest should exist");

    let preamble = import_result.original_preamble.unwrap_or_default();
    let postamble = import_result.original_postamble.unwrap_or_default();

    let mut lexer = Lexer::new(&import_result.dsl);
    lexer
        .parse()
        .expect("Imported DSL should parse without errors");

    let patches = compute_patches(&imported_doc.segments, imported_doc.dsl_rules, &import_result.dsl).unwrap();
    let mut compiler = Compiler::new().with_config(CompilerConfig {
        word_wrap: true,
        auto_line_numbers: false,
        multi_level: true,
        wrap_pages: false,
        levels: None,
    });

    let reconstructed_body =
        apply_patches_and_reconstruct(&imported_doc.segments, &patches, &mut compiler);
    let reconstructed_xml = format!("{}{}{}", preamble, reconstructed_body, postamble);

    let normalized_original = normalize_xml(&xml_content);
    let normalized_reconstructed = normalize_xml(&reconstructed_xml);

    if normalized_reconstructed != normalized_original {
        let left_bytes = normalized_reconstructed.as_bytes();
        let right_bytes = normalized_original.as_bytes();
        let mut diff_index = None;
        let min_len = left_bytes.len().min(right_bytes.len());
        for idx in 0..min_len {
            if left_bytes[idx] != right_bytes[idx] {
                diff_index = Some(idx);
                break;
            }
        }

        if diff_index.is_none() && left_bytes.len() != right_bytes.len() {
            diff_index = Some(min_len);
        }

        if let Some(idx) = diff_index {
            let start = idx.saturating_sub(100);
            let end = (idx + 100).min(min_len.max(idx + 1));
            let left_snippet = String::from_utf8_lossy(&left_bytes[start..end]);
            let right_snippet = String::from_utf8_lossy(&right_bytes[start..end]);
            panic!(
                "Round-trip XML mismatch at byte {}\nLeft: {}\nRight: {}",
                idx,
                left_snippet.escape_default(),
                right_snippet.escape_default()
            );
        }
    }

    assert_eq!(
        normalized_reconstructed, normalized_original,
        "Round-trip XML should be semantically identical"
    );
}

// ============================================================================
// IDEMPOTENCY TESTS
// Verify that compile(import(compile(import(x)))) == compile(import(x))
// ============================================================================

#[test]
fn test_compile_import_idempotent_simple() {
    let original_xml = "<body>Hello world</body>";
    
    // Import → DSL
    let dsl1 = parse(original_xml).unwrap().dsl;
    
    // DSL → XML → DSL
    let xml1 = compile_dsl(&dsl1);
    let dsl2 = parse(&wrap_body(&xml1)).unwrap().dsl;
    
    // Should be stable after one cycle
    let xml2 = compile_dsl(&dsl2);
    let dsl3 = parse(&wrap_body(&xml2)).unwrap().dsl;
    
    assert_eq!(dsl2.trim(), dsl3.trim(), "Simple text should be idempotent after first cycle");
}

#[test]
fn test_compile_import_idempotent_with_markup() {
    let original_xml = r#"<body>
        Text with <supplied>supplied</supplied> and 
        <choice><abbr>a</abbr><expan>abbr</expan></choice> and
        <del>deleted</del><add>added</add> and
        <note>a note</note>
    </body>"#;
    
    // Import → DSL
    let dsl1 = parse(original_xml).unwrap().dsl;
    
    // DSL → XML → DSL
    let xml1 = compile_dsl(&dsl1);
    let dsl2 = parse(&wrap_body(&xml1)).unwrap().dsl;
    
    // DSL → XML → DSL again
    let xml2 = compile_dsl(&dsl2);
    let dsl3 = parse(&wrap_body(&xml2)).unwrap().dsl;
    
    // Normalize for comparison
    let norm2: String = dsl2.split_whitespace().collect::<Vec<_>>().join(" ");
    let norm3: String = dsl3.split_whitespace().collect::<Vec<_>>().join(" ");
    
    assert_eq!(norm2, norm3, 
        "Markup should be idempotent after first cycle\nCycle 2: {}\nCycle 3: {}", norm2, norm3);
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_import_empty_body() {
    let xml = "<body></body>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl, "");
}

#[test]
fn test_import_whitespace_only() {
    let xml = "<body>   \n\t  </body>";
    let result = parse(xml).unwrap();
    assert_eq!(result.dsl.trim(), "");
}

#[test]
fn test_import_nested_elements() {
    let xml = "<body><div><p>Nested <supplied>text</supplied></p></div></body>";
    let result = parse(xml).unwrap();
    assert!(result.dsl.contains("<text>"), "Should handle nested elements");
}

#[test]
fn test_import_consecutive_line_breaks() {
    let xml = "<body>line1<lb/>line2<lb/><lb/>line4</body>";
    let result = parse(xml).unwrap();
    // Should have multiple // markers
    let lb_count = result.dsl.matches("//").count();
    assert_eq!(lb_count, 3, "Should preserve all line breaks, got: {}", result.dsl);
}

#[test]
fn test_import_mixed_whitespace() {
    let xml = "<body>word1  word2\n\nword3\tword4</body>";
    let result = parse(xml).unwrap();
    // Whitespace should be normalized to single spaces
    assert!(!result.dsl.contains("  "), "Should normalize multiple spaces");
    assert!(!result.dsl.contains("\t"), "Should normalize tabs");
}

#[test]
fn test_import_unicode_content() {
    let xml = "<body>Þórðr ok Njáll gengu til þings</body>";
    let result = parse(xml).unwrap();
    assert!(result.dsl.contains("Þórðr"), "Should preserve Icelandic characters");
    assert!(result.dsl.contains("Njáll"), "Should preserve Icelandic characters");
}



#[test]
fn test_roundtrip_space_before_newline() {
    // This reproduces the "þakalþo \n//7" vs "þakalþo\n//7" instability
    // using the exact context from the failure
    let original_dsl = "ppboetrrlogretto+{men}+þakalþo \n//7";
    
    // Cycle 1
    let xml1 = compile_dsl(original_dsl);
    let wrapped1 = wrap_body(&xml1);
    let dsl2 = parse(&wrapped1).unwrap().dsl;
    
    // Cycle 2
    let xml2 = compile_dsl(&dsl2);
    let wrapped2 = wrap_body(&xml2);
    let dsl3 = parse(&wrapped2).unwrap().dsl;
    
    assert_eq!(dsl3, dsl2, "Cycle 2 should be stable with Cycle 1");
}

#[test]
fn test_roundtrip_space_before_tag() {
    // Reproduces space loss before tags
    let original_dsl = "word +{added}+";
    
    let xml1 = compile_dsl(original_dsl);
    let wrapped1 = wrap_body(&xml1);
    let dsl2 = parse(&wrapped1).unwrap().dsl;
    
    assert_eq!(dsl2, "word +{added}+", "Cycle 1 should preserve space before tag");
}


// ============================================================================
// SOURCE-PRESERVING ROUND-TRIP TESTS
// The user's file is privileged: reading must never change it, and edits must
// only touch the bytes of what was edited.
// ============================================================================

const MENOTA_SAMPLE: &str = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body>
<p><lb n="3"/><w me:msa="xNC" lemma="maðr"><choice><me:facs>maþr</me:facs><me:dipl>maþr</me:dipl><me:norm>maðr</me:norm></choice></w> <w lemma="ganga"><choice><me:facs>geck</me:facs><me:dipl>gekk</me:dipl><me:norm>gekk</me:norm></choice></w><pc><choice><me:facs>.</me:facs><me:dipl>.</me:dipl><me:norm>.</me:norm></choice></pc></p>
<p><w><choice><me:facs>ok</me:facs><me:dipl>ok</me:dipl><me:norm>ok</me:norm></choice></w></p>
</body></text></TEI>"#;

/// Imports `xml`, applies `edit` to the DSL, and compiles back the way
/// `compile_imported` does. Returns (original body, new body).
fn imported_roundtrip(xml: &str, edit: impl Fn(&str) -> String) -> Result<(String, String), String> {
    imported_render(xml, edit).map(|(original, rendered)| (original, rendered.xml))
}

/// The file of an import as a tree, with its segments' places.
fn tree_source(
    preamble: &Option<String>,
    body: &str,
    postamble: &Option<String>,
    doc: &crate::importer::tei::segments::ImportedDocument,
) -> Option<crate::importer::tei::apply::TreeSource> {
    crate::importer::tei::apply::TreeSource::new(
        preamble.as_deref().unwrap_or_default(),
        body,
        postamble.as_deref().unwrap_or_default(),
        &doc.segments,
        doc.spans.as_deref()?,
    )
    .ok()
}

/// Like [`imported_roundtrip`], with the full render result.
fn imported_render(
    xml: &str,
    edit: impl Fn(&str) -> String,
) -> Result<(String, RenderedBody), String> {
    let import = parse(xml)?;
    let doc = import.imported_document.expect("manifest");
    let original_body = import.original_body_xml.expect("body");
    let patches = compute_patches(&doc.segments, doc.dsl_rules, &edit(&import.dsl))?;
    let tree = tree_source(&import.original_preamble, &original_body, &import.original_postamble, &doc);
    let mut compiler = Compiler::new().with_config(compiler_config_for(&doc, Originals::new(tree.as_ref())));
    // As a session holds it: without the copies of the XML the tree has
    let mut doc = doc;
    if let Some(tree) = &tree {
        doc.drop_copies(|id| tree.segment_xml(id), |id| tree.segment_attributes(id));
    }
    let body = render_body(&doc, Some(&original_body), tree.as_ref(), &patches, &mut compiler)?;
    Ok((original_body, body))
}

// ============================================================================
// EDITED WORDS KEEP THEIR MARKUP (document/merge.rs)
// ============================================================================

/// A MENOTA body with `words` in one paragraph.
fn menota_body(words: &str) -> String {
    format!(
        r#"<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body>
<p>{words}</p>
</body></text></TEI>"#
    )
}

#[test]
fn test_edit_keeps_expansion_in_a_diplomatic_only_word() {
    // DG 8: the expansion has no mark; the shorthand reads "hæn{=n}aR"
    let xml = menota_body(r#"<w xml:id="w28" lemma="hon"><me:dipl>hæn<ex>n</ex>aR</me:dipl></w> <w><me:dipl>ok</me:dipl></w>"#);
    let (_, body) = imported_roundtrip(&xml, |d| {
        assert!(d.contains("hæn{=n}aR"), "{d}");
        d.replace("hæn{=n}aR", "hæn{=n}aRR")
    })
    .unwrap();
    assert!(
        body.contains(r#"<w xml:id="w28" lemma="hon"><me:dipl>hæn<ex>n</ex>aRR</me:dipl></w>"#),
        "{body}"
    );
}

#[test]
fn test_edit_keeps_supplied_text_and_gap_attributes_and_adds_no_level() {
    // AM 242 fol: an initial left for the rubricator, supplied on the dipl level
    let xml = menota_body(
        r#"<w><me:facs><gap extent="1" unit="initial"/>erollꝺin</me:facs><me:dipl><supplied reason="empty space" resp="KGJ">V</supplied>erollꝺin</me:dipl></w> <w><me:facs>ok</me:facs><me:dipl>ok</me:dipl></w>"#,
    );
    let (_, body) = imported_roundtrip(&xml, |d| d.replace("erollꝺin", "erollꝺinn")).unwrap();
    assert!(body.contains(r#"<gap extent="1" unit="initial"/>erollꝺinn</me:facs>"#), "{body}");
    assert!(body.contains(r#"<supplied reason="empty space" resp="KGJ">V</supplied>erollꝺinn</me:dipl>"#), "{body}");
    assert!(!body.contains("me:norm"), "{body}");
}

#[test]
fn test_edit_keeps_abbreviation_marks_and_expansions_inside_the_word() {
    let xml = menota_body(
        r#"<w lemma="konungr"><choice><me:facs>k<am>&bar;</am>r</me:facs><me:dipl>k<ex>onung</ex>r</me:dipl><me:norm>konungr</me:norm></choice></w> <w><choice><me:facs>ok</me:facs><me:dipl>ok</me:dipl><me:norm>ok</me:norm></choice></w>"#,
    );
    let import = parse(&xml).unwrap();
    // The mark and its expansion where they stand (DSL rules 7)
    assert!(import.dsl.contains("k{:bar:=onung}r"), "{}", import.dsl);
    let (_, body) = imported_roundtrip(&xml, |d| d.replace("k{:bar:=onung}r", "k{:bar:=onung}rs")).unwrap();
    assert!(body.contains("<me:facs>k<am>&bar;</am>rs</me:facs>"), "{body}");
    assert!(body.contains("<me:dipl>k<ex>onung</ex>rs</me:dipl>"), "{body}");
    // The normalized form is the one the shorthand gives: it follows
    assert!(body.contains("<me:norm>konungrs</me:norm>"), "{body}");

}

#[test]
fn test_edit_keeps_comments_and_unknown_elements_in_the_word() {
    let xml = menota_body(
        r#"<w><me:facs>ſmi<lb ed="ms" n="9"/><!--Kontrollera--><hi rend="right"></hi>ðaðir</me:facs><me:dipl>ſmi<lb ed="ms" n="9"/>ðaðir</me:dipl></w> <w><me:facs>ok</me:facs><me:dipl>ok</me:dipl></w>"#,
    );
    let (_, body) = imported_roundtrip(&xml, |d| d.replace("ðaðir", "ðaðirr")).unwrap();
    assert!(body.contains(r#"ſmi<lb ed="ms" n="9"/><!--Kontrollera--><hi rend="right"/>ðaðirr</me:facs>"#), "{body}");
    assert!(body.contains(r#"<me:dipl>ſmi<lb ed="ms" n="9"/>ðaðirr</me:dipl>"#), "{body}");
}

#[test]
fn test_unchanged_import_is_byte_identical() {
    let (original, body) = imported_roundtrip(MENOTA_SAMPLE, |d| d.to_string()).unwrap();
    assert_eq!(original, body);
}

#[test]
fn test_whitespace_only_edit_is_byte_identical() {
    let (original, body) =
        imported_roundtrip(MENOTA_SAMPLE, |d| format!("\n\n{}\n\n", d.replace(' ', "   "))).unwrap();
    assert_eq!(original, body);
}

#[test]
fn test_dsl_parse_error_is_an_error_not_a_wipe() {
    let result = imported_roundtrip(MENOTA_SAMPLE, |d| format!("{} -{{broken", d));
    assert!(result.is_err(), "unparseable DSL must not produce output");
}

#[test]
fn test_edit_touches_only_the_edited_word() {
    let (original, body) = imported_roundtrip(MENOTA_SAMPLE, |d| d.replace("geck", "gekk")).unwrap();
    // Untouched word keeps its exact original bytes, including attribute order.
    assert!(body.contains(r#"<w me:msa="xNC" lemma="maðr"><choice><me:facs>maþr</me:facs>"#));
    // Edited word keeps its lemma.
    assert!(body.contains(r#"lemma="ganga""#));
    assert!(body.contains("<me:facs>gekk</me:facs>"));
    assert!(!body.contains("geck"));
    // Everything outside the edited <w> is unchanged.
    let start = original.find(r#"<w lemma="ganga">"#).unwrap();
    assert_eq!(&body[..start], &original[..start]);
    assert!(body.ends_with(&original[original.find("<pc>").unwrap()..]));
}

#[test]
fn test_line_break_edit_is_applied() {
    let (_, body) = imported_roundtrip(MENOTA_SAMPLE, |d| d.replace("//3", "//4")).unwrap();
    assert!(body.contains(r#"<lb n="4"/>"#), "{}", body);
    assert!(!body.contains(r#"<lb n="3"/>"#));
}

#[test]
fn test_insert_at_paragraph_end_stays_in_paragraph() {
    let (_, body) = imported_roundtrip(MENOTA_SAMPLE, |d| d.replace("geck.", "geck. hann")).unwrap();
    let first_p_end = body.find("</p>").unwrap();
    let inserted = body.find("<me:facs>hann</me:facs>").expect("inserted word");
    assert!(inserted < first_p_end, "insert must land in the first paragraph:\n{}", body);
}

#[test]
fn test_non_menota_insert_is_single_level() {
    let xml = "<TEI><text><body><p><w>alpha</w> <w>beta</w></p></body></text></TEI>";
    let (_, body) = imported_roundtrip(xml, |d| format!("{} gamma", d)).unwrap();
    assert!(body.contains("<w>gamma</w>"), "{}", body);
    assert!(!body.contains("me:facs"));
}

#[test]
fn test_plain_text_edit_stays_plain_text() {
    let xml = "<TEI><text><body><p>Hello brave world</p></body></text></TEI>";
    let (original, body) = imported_roundtrip(xml, |d| d.replace("brave", "bold")).unwrap();
    assert_eq!(body, original.replace("brave", "bold"));
}

#[test]
fn test_entities_and_comments_survive_untouched() {
    let xml = "<TEI><text><body><!-- keep me --><p>a &amp; b<lb n='1' />c</p></body></text></TEI>";
    let (original, body) = imported_roundtrip(xml, |d| d.replace('c', "cc")).unwrap();
    assert_eq!(body, original.replace(">c<", ">cc<"));
}

fn load_holmperg() -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("tests/fixtures/HolmPerg-34-4to-MLL.xml");
    std::fs::read_to_string(path).ok()
}

#[test]
fn test_menota_file_unchanged_is_byte_identical() {
    let Some(xml) = load_holmperg() else { return };
    let import = parse(&xml).unwrap();
    assert!(
        import.imported_document.as_ref().unwrap().spans.is_some(),
        "source spans should align for the reference MENOTA file"
    );
    let (original, body) = imported_roundtrip(&xml, |d| d.to_string()).unwrap();
    assert!(original == body, "unchanged import must reproduce the body byte-for-byte");
}

#[test]
fn test_menota_file_distant_edits_preserve_everything_else() {
    let Some(xml) = load_holmperg() else { return };
    let import = parse(&xml).unwrap();
    let doc = import.imported_document.unwrap();
    // First plain word that is a word of its own (a word inside `.head{...}`
    // is part of one opaque segment and would regenerate the whole head).
    let first = doc
        .segments
        .iter()
        .find_map(|s| match s {
            Segment::Word { dsl_content, .. }
                if dsl_content.len() > 3
                    && dsl_content.chars().all(char::is_alphabetic)
                    && import.dsl.contains(&format!(" {dsl_content} ")) =>
            {
                Some(dsl_content.clone())
            }
            _ => None,
        })
        .unwrap();
    // An edit of the word (one letter): it keeps its lemma. (Another word
    // in its place would be a new word, without the old word's lemma.)
    let typo: String = first.chars().take(first.chars().count() - 1).chain(['x']).collect();
    let edited = format!(
        "{} extra",
        import.dsl.replacen(&format!(" {} ", first), &format!(" {} ", typo), 1)
    );

    // Exactly two changes: the edited token's segment and the appended word.
    let patches = compute_patches(&doc.segments, doc.dsl_rules, &edited).unwrap();
    let changes: Vec<_> = patches
        .iter()
        .filter(|p| !matches!(p, PatchOperation::Keep { .. }))
        .collect();
    assert_eq!(changes.len(), 2, "unexpected changes: {:?}", changes);

    let (original, body) = imported_roundtrip(&xml, |_| edited.clone()).unwrap();
    assert_eq!(
        original.matches("lemma=").count(),
        body.matches("lemma=").count(),
        "no lemma attributes may be lost"
    );
    assert!(body.contains(&format!("<me:facs>{typo}</me:facs>")), "{typo}");
    assert!(body.contains("<me:facs>extra</me:facs>"));
}

#[test]
fn test_multi_word_w_edit_stays_one_word() {
    let xml = r#"<TEI xmlns:me="http://www.menota.org/ns/1.0"><text><body><p><w lemma="þó at"><choice><me:facs>þo at</me:facs><me:dipl>þo at</me:dipl><me:norm>þó at</me:norm></choice></w> <w><choice><me:facs>x</me:facs><me:dipl>x</me:dipl><me:norm>x</me:norm></choice></w></p></body></text></TEI>"#;
    // One <w> written as two words reads as one DSL word: "þo~at"
    let (_, body) = imported_roundtrip(xml, |d| d.replace("þo~at", "þa~at")).unwrap();
    assert!(body.contains("<me:facs>þa at</me:facs>"), "{}", body);
    assert_eq!(body.matches("<w").count(), 2, "{}", body);
    assert!(body.contains(r#"lemma="þó at""#));
}

// ============================================================================
// PARAGRAPH BREAKS (.p)
// ============================================================================

#[test]
fn test_paragraph_split_inserts_tags_in_place() {
    // DSL: ".p \n//3 maþr geck. .p ok"
    let (original, body) =
        imported_roundtrip(MENOTA_SAMPLE, |d| d.replace("maþr geck", "maþr .p geck")).unwrap();
    let expected = original.replacen("</w> <w lemma", "</w></p>\n<p> <w lemma", 1);
    assert_eq!(body, expected);
}

#[test]
fn test_paragraph_merge_removes_tags_keeps_separator() {
    let (original, body) =
        imported_roundtrip(MENOTA_SAMPLE, |d| d.replace("geck. .p ok", "geck. ok")).unwrap();
    let expected = original.replacen("</p>\n<p><w>", "\n<w>", 1);
    assert_eq!(body, expected);
}

#[test]
fn test_paragraph_whitespace_around_marker_changes_nothing() {
    let (original, body) =
        imported_roundtrip(MENOTA_SAMPLE, |d| d.replace(" .p ok", "\n\n  .p\n   ok")).unwrap();
    assert_eq!(original, body);
}

#[test]
fn test_paragraph_merge_of_plain_text_keeps_words_apart() {
    let xml = "<TEI><text><body><p>one two</p><p>three</p></body></text></TEI>";
    let (_, body) = imported_roundtrip(xml, |d| d.replace(" .p three", " three")).unwrap();
    assert_eq!(body, "<body><p>one two\nthree</p></body>");
}

#[test]
fn test_paragraph_split_inside_inline_element_is_an_error() {
    // Hidden markup: `<l>` (a verse line) isn't shown
    let xml = "<TEI><text><body><p>a <l>b c</l> d</p></body></text></TEI>";
    let result = imported_roundtrip(xml, |d| d.replace("b c", "b .p c"));
    let error = result.expect_err("a split inside <l> would break the XML");
    assert!(error.contains("inside <l>"), "{error}");
    // Shown markup: the shorthand can't have it
    let xml = "<TEI><text><body><p>a <hi>b c</hi> d</p></body></text></TEI>";
    let error = imported_roundtrip(xml, |d| d.replace("b c", "b .p c")).unwrap_err();
    assert!(error.contains("A paragraph can't start inside *{…}*"), "{error}");
}

#[test]
fn test_removing_first_paragraph_break_is_an_error() {
    let result = imported_roundtrip(MENOTA_SAMPLE, |d| d.replacen(".p", "", 1));
    let error = result.expect_err("the first paragraph has no paragraph to merge into");
    assert!(error.contains("first paragraph"), "{error}");
}

#[test]
fn test_menota_file_paragraph_edits_reimport_as_edited() {
    let Some(xml) = load_holmperg() else { return };
    let import = parse(&xml).unwrap();
    let dsl = &import.dsl;

    // Merge the 3rd paragraph into the 2nd, and split one later on
    let third = dsl.match_indices(" .p ").nth(2).unwrap().0;
    let edited = format!("{}{}", &dsl[..third], &dsl[third + 3..]);
    // Split after the first sentence end (`}` closes a `.s{…}`) that allows
    // it; a paragraph can't start inside a sentence
    let mut result = None;
    for (at, _) in edited.match_indices("} ").filter(|(at, _)| *at > third).skip(5).take(50) {
        let mut candidate = edited.clone();
        candidate.insert_str(at + 1, " .p");
        if let Ok(rendered) = imported_roundtrip(&xml, |_| candidate.clone()) {
            result = Some((candidate, rendered));
            break;
        }
    }
    let (edited, (_, body)) = result.expect("a sentence boundary that can be split");
    let start = xml.find("<body").unwrap();
    let end = xml.find("</body>").unwrap() + "</body>".len();
    let rewritten = format!("{}{}{}", &xml[..start], body, &xml[end..]);

    // The new file reads back as exactly the edited DSL (modulo whitespace)
    let reread = parse(&rewritten).unwrap().dsl;
    let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(words(&reread), words(&edited));
    // Nothing but paragraph tags (and whitespace) changed
    let without_paragraphs =
        |s: &str| s.replace("</p>", "").replace("<p>", "").split_whitespace().collect::<String>();
    assert_eq!(without_paragraphs(&rewritten), without_paragraphs(&xml));
    assert_ne!(rewritten, xml);
}

// ============================================================================
// EDITED WORDS KEEP THEIR CURATED DATA
// ============================================================================

#[test]
fn test_edited_word_keeps_lemma_msa_and_norm() {
    // DSL: ".p \n//3 maþr geck. .p ok"; "maþr" has lemma maðr and norm maðr
    let (_, rendered) = imported_render(MENOTA_SAMPLE, |d| d.replace("maþr", "maþꝛ")).unwrap();
    let xml = &rendered.xml;
    let start = xml.find("<w").unwrap();
    let word = &xml[start..start + xml[start..].find("</w>").unwrap()];
    assert!(word.contains(r#"lemma="maðr""#), "{word}");
    assert!(word.contains(r#"me:msa="xNC""#), "{word}");
    assert!(word.contains("<me:norm>maðr</me:norm>"), "curated norm kept: {word}");
    assert!(word.contains("<me:facs>maþꝛ</me:facs>"), "facs follows the edit: {word}");
    assert_eq!(rendered.edited_word_indices(), vec![0]);
}

#[test]
fn test_edited_word_numbers_follow_document_order() {
    let (_, rendered) = imported_render(MENOTA_SAMPLE, |d| d.replace("geck", "gekk").replace("ok", "oc")).unwrap();
    assert_eq!(rendered.edited_word_indices(), vec![1, 2]);
}

#[test]
fn test_unedited_and_inserted_words_are_not_flagged() {
    let (_, rendered) = imported_render(MENOTA_SAMPLE, |d| d.replace("geck.", "geck nyr.")).unwrap();
    assert!(rendered.edited_word_indices().is_empty(), "{:?}", rendered.edited_words);
}

#[test]
fn test_edited_word_with_break_moves_break_in_norm() {
    let xml = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body><p><w lemma="frostuþing"><choice><me:facs>fro<lb n="5"/>sto</me:facs><me:dipl>fro<lb n="5"/>sto</me:dipl><me:norm>Frostu<lb n="5"/></me:norm></choice></w></p></body></text></TEI>"#;
    let (_, rendered) = imported_render(xml, |d| d.replace("fro~//5sto", "frost~//5o")).unwrap();
    let out = &rendered.xml;
    assert!(out.contains("<me:facs>frost<lb n=\"5\"/>o</me:facs>"), "{out}");
    assert!(out.contains("<me:norm>Frostu<lb n=\"5\"/></me:norm>"), "{out}");
    assert_eq!(out.matches("<lb").count(), 3, "one break per level: {out}");
}

// ============================================================================
// ONE READING: the XML and the DSL generated from it agree token for token
// ============================================================================

#[test]
fn test_menota_file_xml_and_dsl_read_as_the_same_tokens() {
    let Some(xml) = load_holmperg() else { return };
    let import = parse(&xml).unwrap();
    let segments = import.imported_document.unwrap().segments;

    // Every element the XML reader found (word, punctuation, break,
    // paragraph) is exactly one token of the DSL reading, in order
    let from_xml: Vec<String> = segments
        .iter()
        .filter_map(crate::importer::tei::patching::segment_dsl_token)
        .collect();
    let from_dsl = crate::importer::tei::patching::dsl_tokens(&import.dsl).unwrap();
    assert_eq!(from_xml.len(), from_dsl.len());
    let first_difference = from_xml.iter().zip(&from_dsl).position(|(a, b)| a != b);
    assert_eq!(first_difference, None, "{:?}", first_difference.map(|i| (&from_xml[i], &from_dsl[i])));
}

#[test]
fn test_multi_word_w_reads_as_one_dsl_word() {
    let xml = r#"<TEI xmlns:me="http://www.menota.org/ns/1.0"><text><body><p><w><choice><me:facs>þo at</me:facs><me:dipl>þo at</me:dipl><me:norm>þó at</me:norm></choice></w></p></body></text></TEI>"#;
    assert_eq!(parse(xml).unwrap().dsl, ".p þo~at");
}

#[test]
fn test_entity_punctuation_is_set_off_by_spaces() {
    let xml = r#"<TEI xmlns:me="http://www.menota.org/ns/1.0"><text><body><p><w><choice><me:facs>þa</me:facs><me:dipl>þa</me:dipl><me:norm>þá</me:norm></choice></w><pc><choice><me:facs>&middot;</me:facs><me:dipl>&middot;</me:dipl><me:norm>,</me:norm></choice></pc><w><choice><me:facs>þo</me:facs><me:dipl>þo</me:dipl><me:norm>þó</me:norm></choice></w></p></body></text></TEI>"#;
    assert_eq!(parse(xml).unwrap().dsl, ".p þa :middot: þo");
}

#[test]
fn test_join_written_words() {
    use crate::importer::tei::extraction::join_written_words;
    assert_eq!(join_written_words("þo at"), "þo~at");
    assert_eq!(join_written_words(" frosto þings  logum "), "frosto~þings~logum");
    // Whitespace inside constructs is content, not a word boundary
    assert_eq!(join_written_words(".abbr[a b]{c d} e"), ".abbr[a b]{c d}~e");
    assert_eq!(join_written_words("a^{two words}"), "a^{two words}");
    // A continued break already joins
    assert_eq!(join_written_words("to~//5 þings logum"), "to~//5þings~logum");
}

#[test]
fn test_menota_file_whitespace_only_dsl_changes_nothing() {
    let Some(xml) = load_holmperg() else { return };
    let (original, body) =
        imported_roundtrip(&xml, |d| d.replace('\n', " \n\n  ").replace(" .p ", "\n.p\n")).unwrap();
    assert!(original == body, "a whitespace-only DSL change altered the XML");
}

#[test]
fn test_dsl_compiled_and_reimported_reads_the_same() {
    // The other direction: DSL → XML (MENOTA and plain word-wrapped) → DSL
    let dsl = ".p //1 Maþr het :thorn:orir þo~at .abbr[k:bar:]{konungr} :middot: \
               fro~//2sto þings , ok -{x}- +{y}+ .p //3 þa :middot: geck .";
    for multi_level in [true, false] {
        let config = CompilerConfig { word_wrap: true, auto_line_numbers: false, multi_level, wrap_pages: false, levels: None };
        let xml = Compiler::new().with_config(config).compile(dsl).unwrap();
        let ns = r#"xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0""#;
        let reread = parse(&format!("<TEI {ns}><text><body>{xml}</body></text></TEI>")).unwrap().dsl;
        let tokens = crate::importer::tei::patching::dsl_tokens;
        assert_eq!(
            tokens(&reread).unwrap(),
            tokens(dsl).unwrap(),
            "multi_level={multi_level}\nXML: {xml}\nDSL: {reread}"
        );
    }
}

#[test]
fn test_content_after_a_page_break_is_set_off_from_its_number() {
    // A note right after the page break (Holm A 49) became part of the number
    let xml = r#"<body><p><pb ed="ms" n="109r"/><note>Förekomst <hi>av</hi></note>
<w>jn</w>.</p></body>"#;
    let import = parse(xml).unwrap();
    assert!(import.dsl.contains("///109r\n^{"), "{}", import.dsl);
    let (original, body) = imported_roundtrip(xml, |d| d.to_string()).unwrap();
    assert_eq!(original, body);

    // Documents imported with rules 1 keep their glued DSL
    let doc = import.imported_document.unwrap();
    let stored = crate::importer::tei::extraction::segments_to_dsl(&doc.segments, 1);
    assert!(stored.contains("///109r^{"), "{}", stored);
}

#[test]
fn test_note_with_angle_bracket_imports_as_valid_dsl() {
    // AM 243 b α: "ð < t" records what a letter was corrected from
    let xml = r#"<body><p><w>seð</w><note>ð &lt; t</note> <w>annarra</w></p></body>"#;
    let import = parse(xml).unwrap();
    assert!(crate::parser::syntax::parse(&import.dsl).diagnostics.is_empty(), "{}", import.dsl);
    let (original, body) = imported_roundtrip(xml, |d| d.to_string()).unwrap();
    assert_eq!(original, body);
}

#[test]
fn test_documents_imported_before_rule_changes_keep_their_dsl() {
    let Some(xml) = load_holmperg() else { return };
    let import = parse(&xml).unwrap();
    let mut doc = import.imported_document.unwrap();

    // A manifest saved before rules were versioned reads as rules 0
    let mut json: serde_json::Value = serde_json::to_value(&doc).unwrap();
    json.as_object_mut().unwrap().remove("dsl_rules");
    let old = crate::importer::tei::segments::ImportedDocument::from_json(&json.to_string()).unwrap();
    assert_eq!(old.dsl_rules, 0);

    // Its stored DSL glues entity punctuation to words ("þa:middot:"); one
    // unrelated edit must still be one change, not one per middle dot.
    // (Before rules 10, no markup was shown.)
    doc.dsl_rules = 0;
    for segment in &mut doc.segments {
        if let Segment::Structural { shown, .. } = segment {
            *shown = None;
        }
    }
    let stored = crate::importer::tei::extraction::segments_to_dsl(&doc.segments, 0);
    assert!(!stored.contains(" :middot: "));
    let edited = stored.replacen(" logum ", " logvm ", 1);
    let patches = compute_patches(&doc.segments, 0, &edited).unwrap();
    let changes = patches.iter().filter(|p| !matches!(p, PatchOperation::Keep { .. })).count();
    assert_eq!(changes, 1);
}

// ============================================================================
// WORD IDENTITY ACROSS EDITS (editor-guided)
// ============================================================================

/// The single change from `old` to `new` (common prefix and suffix), in
/// UTF-16 units, as CodeMirror would report a one-place edit.
fn change_between(old: &str, new: &str) -> TextChange {
    let (a, b): (Vec<u16>, Vec<u16>) = (old.encode_utf16().collect(), new.encode_utf16().collect());
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let max_suffix = a.len().min(b.len()) - prefix;
    let suffix = a.iter().rev().zip(b.iter().rev()).take(max_suffix).take_while(|(x, y)| x == y).count();
    TextChange { from_a: prefix, to_a: a.len() - suffix, from_b: prefix, to_b: b.len() - suffix }
}

/// An imported document being edited the way the app does it: one Patcher,
/// each compile guided by the change since the previous one.
struct Session {
    doc: crate::importer::tei::segments::ImportedDocument,
    body: String,
    tree: Option<crate::importer::tei::apply::TreeSource>,
    patcher: Patcher,
    config: crate::parser::CompilerConfig,
    dsl: String,
}

impl Session {
    fn open(xml: &str) -> Self {
        let import = parse(xml).unwrap();
        let doc = import.imported_document.unwrap();
        let patcher = Patcher::new(&doc.segments, doc.dsl_rules);
        let body = import.original_body_xml.unwrap();
        let tree = tree_source(&import.original_preamble, &body, &import.original_postamble, &doc);
        assert!(tree.is_some(), "the file maps onto its segments");
        let config = compiler_config_for(&doc, Originals::new(tree.as_ref()));
        let mut doc = doc;
        if let Some(tree) = &tree {
            doc.drop_copies(|id| tree.segment_xml(id), |id| tree.segment_attributes(id));
        }
        let mut session = Session { doc, body, tree, patcher, config, dsl: import.dsl.clone() };
        session.compile(&import.dsl, false).unwrap();
        session
    }

    /// Compiles `dsl`, guided by the change from the previous DSL unless `blind`.
    fn compile(&mut self, dsl: &str, blind: bool) -> Result<String, String> {
        let change = change_between(&self.dsl, dsl);
        self.compile_with(dsl, change, blind)
    }

    /// Compiles `dsl`, reached from the previous DSL by `change`.
    fn compile_with(&mut self, dsl: &str, change: TextChange, blind: bool) -> Result<String, String> {
        let guidance = Guidance { base_revision: self.patcher.revision(), changes: vec![change] };
        let (patches, state) = self.patcher.compute(dsl, (!blind).then_some(&guidance))?;
        let mut compiler = Compiler::new().with_config(self.config.clone());
        let body = render_body(&self.doc, Some(&self.body), self.tree.as_ref(), &patches, &mut compiler)?;
        self.patcher.commit(state);
        self.dsl = dsl.to_string();
        Ok(body.xml)
    }
}

const TWO_OKS: &str = r#"<TEI xmlns:me="http://www.menota.org/ns/1.0"><text><body><p><w lemma="ok" me:msa="xCC"><choice><me:facs>ok</me:facs><me:dipl>ok</me:dipl><me:norm>ok</me:norm></choice></w> <w lemma="ok" me:msa="xAV"><choice><me:facs>ok</me:facs><me:dipl>ok</me:dipl><me:norm>ok</me:norm></choice></w> <w lemma="maðr"><choice><me:facs>maþr</me:facs><me:dipl>maþr</me:dipl><me:norm>maðr</me:norm></choice></w></p></body></text></TEI>"#;

#[test]
fn test_deleting_one_of_two_identical_words_deletes_that_one() {
    // DSL ".p ok ok maþr": select and delete the first "ok " (a conjunction, xCC)
    let delete_first = TextChange { from_a: 3, to_a: 6, from_b: 3, to_b: 3 };
    let mut session = Session::open(TWO_OKS);
    let xml = session.compile_with(".p ok maþr", delete_first, false).unwrap();
    assert!(xml.contains(r#"me:msa="xAV""#) && !xml.contains(r#"me:msa="xCC""#), "{xml}");

    // Comparing whole documents can't tell which "ok" went, and keeps the first
    let mut blind = Session::open(TWO_OKS);
    let xml = blind.compile(".p ok maþr", true).unwrap();
    assert!(xml.contains(r#"me:msa="xCC""#), "{xml}");
}

#[test]
fn test_editing_one_identical_word_and_deleting_the_other() {
    // Change the second "ok" to "ek" and delete the first, in two edits
    let mut session = Session::open(TWO_OKS);
    session.compile(".p ok ek maþr", false).unwrap();
    let delete_first = TextChange { from_a: 3, to_a: 6, from_b: 3, to_b: 3 };
    let xml = session.compile_with(".p ek maþr", delete_first, false).unwrap();
    assert!(!xml.contains(r#"me:msa="xCC""#), "the conjunction was deleted: {xml}");
    assert!(xml.contains(r#"me:msa="xAV""#), "the adverb was edited: {xml}");
    assert!(xml.contains("<me:facs>ek</me:facs>"), "{xml}");
}

#[test]
fn test_out_of_step_guidance_falls_back_to_comparison() {
    let session = Session::open(TWO_OKS);
    let guidance = Guidance { base_revision: 999, changes: vec![TextChange { from_a: 0, to_a: 9999, from_b: 0, to_b: 0 }] };
    let (patches, _) = session.patcher.compute(".p ok ok maður", Some(&guidance)).unwrap();
    let changed = patches.iter().filter(|p| !matches!(p, PatchOperation::Keep { .. })).count();
    assert_eq!(changed, 1);
}

#[test]
fn test_menota_file_guided_edits_reimport_as_edited() {
    let Some(xml) = load_holmperg() else { return };
    let mut session = Session::open(&xml);
    let start = xml.find("<body").unwrap();
    let end = xml.find("</body>").unwrap() + "</body>".len();

    // Deterministic pseudo-random word edits: delete, duplicate, change
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = |n: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 33) as usize % n
    };
    let mut dsl = session.dsl.clone();
    for round in 0..8 {
        let words: Vec<(usize, &str)> = dsl
            .match_indices(|c: char| c.is_whitespace())
            .map(|(i, _)| i + 1)
            .filter_map(|i| {
                let word = dsl[i..].split_whitespace().next()?;
                (word.chars().all(char::is_alphabetic) && dsl[i..].starts_with(word)).then_some((i, word))
            })
            .collect();
        let (at, word) = words[next(words.len())];
        let edited = match round % 3 {
            0 => format!("{}{}", &dsl[..at], &dsl[at + word.len()..]),
            1 => format!("{}{} {}", &dsl[..at], word, &dsl[at..]),
            _ => format!("{}{}x{}", &dsl[..at], word, &dsl[at + word.len()..]),
        };
        let body = session.compile(&edited, false).unwrap();
        let rewritten = format!("{}{}{}", &xml[..start], body, &xml[end..]);
        let reread = parse(&rewritten).unwrap().dsl;
        let tokens = crate::importer::tei::patching::dsl_tokens;
        assert_eq!(tokens(&reread).unwrap(), tokens(&edited).unwrap(), "round {round}");
        dsl = edited;
    }
}

#[test]
fn test_guided_character_edits_always_reimport_as_edited() {
    const PIECES: &[&str] = &["a", "þ", " ", ".", ",", "ok", ":middot:", "\n", "~", ".p "];
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = |n: usize| {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as usize % n
    };
    let mut session = Session::open(TWO_OKS);
    let (mut compiled, mut failed) = (0, 0);
    for _ in 0..400 {
        let dsl = session.dsl.clone();
        let chars: Vec<usize> = dsl.char_indices().map(|(i, _)| i).chain([dsl.len()]).collect();
        let at = chars[next(chars.len())];
        let edited = if next(2) == 0 && at < dsl.len() {
            // Delete one character
            let len = dsl[at..].chars().next().unwrap().len_utf8();
            format!("{}{}", &dsl[..at], &dsl[at + len..])
        } else {
            format!("{}{}{}", &dsl[..at], PIECES[next(PIECES.len())], &dsl[at..])
        };
        match session.compile(&edited, false) {
            Ok(body) => {
                compiled += 1;
                let reread = parse(&format!(r#"<TEI xmlns:me="http://www.menota.org/ns/1.0"><text>{body}</text></TEI>"#))
                    .unwrap()
                    .dsl;
                let tokens = crate::importer::tei::patching::dsl_tokens;
                let (got, want) = (tokens(&reread).unwrap(), tokens(&edited).unwrap());
                if got != want {
                    let i = got.iter().zip(&want).position(|(a, b)| a != b).unwrap_or(got.len().min(want.len()));
                    panic!(
                        "after editing to {edited:?}, token {i}: got {:?}, want {:?}",
                        &got[i.saturating_sub(2)..(i + 3).min(got.len())],
                        &want[i.saturating_sub(2)..(i + 3).min(want.len())],
                    );
                }
            }
            // Invalid DSL, or a paragraph edit that can't be placed: nothing changes
            Err(_) => {
                failed += 1;
                assert_eq!(session.dsl, dsl);
            }
        }
    }
    assert!(compiled > 300, "{compiled} compiled, {failed} rejected");
}
#[test]
fn test_word_space_between_entities_survives_import() {
    // One <w> whose written words meet at entities: `&et; &thorn;a`
    for dsl in ["a:thorn:~:middot:", ":et:~:thorn:a"] {
        let config = CompilerConfig { word_wrap: true, auto_line_numbers: false, multi_level: true, wrap_pages: false, levels: None };
        let xml = Compiler::new().with_config(config).compile(dsl).unwrap();
        let back = parse(&format!(r#"<TEI xmlns:me="http://www.menota.org/ns/1.0"><text><body>{xml}</body></text></TEI>"#)).unwrap().dsl;
        assert_eq!(back, dsl, "{xml}");
    }
}

#[test]
fn test_edit_keeps_a_level_with_only_markup() {
    // AM 36 fol: the word's only level holds a gap and no text
    let xml = menota_body(r#"<w><me:dipl><gap extent="1"/></me:dipl></w> <w><me:dipl>ok</me:dipl></w>"#);
    let (_, body) = imported_roundtrip(&xml, |d| d.replacen("[...]", "[...]x", 1)).unwrap();
    assert!(body.contains(r#"<w><me:dipl><gap extent="1"/>x</me:dipl></w>"#), "{body}");
}

#[test]
fn test_edit_word_of_escaped_symbols() {
    // `<w>~</w>` is `\~`, which reads as punctuation but is still the word
    let xml = menota_body(r#"<w><me:dipl>~</me:dipl></w> <w><me:dipl>ok</me:dipl></w>"#);
    assert_eq!(parse(&xml).unwrap().dsl, r".p \~ ok");
    let (_, body) = imported_roundtrip(&xml, |d| d.replacen(r"\~", r"\~x", 1)).unwrap();
    assert!(body.contains("<w><me:dipl>~x</me:dipl></w>"), "{body}");
}

#[test]
fn test_shorthand_reads_as_the_files_tokens() {
    let cases = [
        // A note after a word, with nothing between them, is set off by a space
        (r#"<w><me:dipl>skrims</me:dipl></w><note>sic</note> <w><me:dipl>ok</me:dipl></w>"#, ".p skrims ^{sic} ok"),
        // Two words written together
        (r#"<w><me:dipl>þꝩi</me:dipl></w><w><me:dipl>at</me:dipl></w>"#, ".p þꝩi|at"),
        // Punctuation inside a word, and a numeral
        (r#"<w><me:dipl>iðran;</me:dipl></w> <num><choice><me:facs>.x.</me:facs><me:dipl>.x.</me:dipl></choice></num>"#, r".p iðran\; #.x.#"),
        // Two commas in one <pc>, and a <pc> inside a word
        (r#"<w><me:dipl>sína</me:dipl></w><pc>,,</pc> <w><me:dipl><pc>.</pc>ij<pc>.</pc></me:dipl></w>"#, r".p sína,\, \.ij\."),
        // A word continued across a break, then another word right after
        (r#"<w><me:dipl>míſ<lb n="2"/></me:dipl></w> <w><me:dipl>kun</me:dipl></w>"#, ".p míſ~//2|kun"),
    ];
    for (words, expected) in cases {
        let xml = menota_body(words);
        let import = parse(&xml).unwrap();
        assert_eq!(import.dsl, expected);
        let segments = import.imported_document.unwrap().segments;
        let from_xml: Vec<String> =
            segments.iter().filter_map(crate::importer::tei::patching::segment_dsl_token).collect();
        assert_eq!(from_xml, crate::importer::tei::patching::dsl_tokens(&import.dsl).unwrap(), "{expected}");
    }
}

#[test]
fn test_break_numbers_and_supplied_markup_read_as_tokens() {
    let cases = [
        (r#"<w><me:dipl>a</me:dipl></w> <lb n="56r"/><w><me:dipl>ok</me:dipl></w>"#, ".p a\n//{56r} ok"),
        (r#"<w><me:dipl>a<lb n="-3"/>b</me:dipl></w>"#, ".p a~//{-3}b"),
        (r#"<w><me:dipl>sa<supplied>m<lb n="14"/>þ</supplied>yct</me:dipl></w>"#, ".p sa<m~//14þ>yct"),
        (r#"<w><me:dipl>sægír</me:dipl></w><pc>: –</pc>"#, r".p sægír:\ \–"),
    ];
    for (words, expected) in cases {
        let xml = menota_body(words);
        let import = parse(&xml).unwrap();
        assert_eq!(import.dsl, expected);
        let segments = import.imported_document.unwrap().segments;
        let from_xml: Vec<String> =
            segments.iter().filter_map(crate::importer::tei::patching::segment_dsl_token).collect();
        assert_eq!(from_xml, crate::importer::tei::patching::dsl_tokens(&import.dsl).unwrap(), "{expected}");
    }
    // An edited word keeps its supplied text with markup
    let xml = menota_body(r#"<w><me:dipl>sa<supplied resp="ed">m<lb n="14"/>þ</supplied>yct</me:dipl></w> <w><me:dipl>ok</me:dipl></w>"#);
    let (_, body) = imported_roundtrip(&xml, |d| d.replace("yct", "ycth")).unwrap();
    assert!(body.contains(r#"<me:dipl>sa<supplied resp="ed">m<lb n="14"/>þ</supplied>ycth</me:dipl>"#), "{body}");
}

#[test]
fn test_letter_typed_at_the_end_goes_inside_layout_whitespace() {
    // AM 619 4to, Holm A 80: levels laid out on lines of their own
    let xml = menota_body("<w>\n<me:dipl> <ex>ok</ex> </me:dipl>\n</w> <w><me:dipl>mælom</me:dipl></w>");
    let (_, body) = imported_roundtrip(&xml, |d| d.replacen("{=ok}", "{=ok}k", 1)).unwrap();
    assert!(body.contains("<me:dipl> <ex>ok</ex>k </me:dipl>"), "{body}");
    let (_, body) = imported_roundtrip(&xml, |d| d.replacen("{=ok}", "k{=ok}", 1)).unwrap();
    assert!(body.contains("<me:dipl> k<ex>ok</ex> </me:dipl>"), "{body}");
}

#[test]
fn test_deleting_the_only_word_of_a_wrapper_removes_the_wrapper() {
    // DN bind 4 nr 19: a numeral written as a word, `<num><w>…</w></num>`
    let xml = menota_body(
        r#"<w><me:dipl>alen</me:dipl></w> <num n="50"><w><me:dipl>fímtígí</me:dipl></w></num> <w><me:dipl>en</me:dipl></w>"#,
    );
    assert_eq!(parse(&xml).unwrap().dsl, ".p alen fímtígí en");
    let (original, body) = imported_roundtrip(&xml, |d| d.replace(" fímtígí", "")).unwrap();
    assert_eq!(body, original.replace(r#"<num n="50"><w><me:dipl>fímtígí</me:dipl></w></num>"#, ""));
    // A wrapper with something else left in it stays
    let xml = menota_body(r#"<w><me:dipl>a</me:dipl></w> <s><w><me:dipl>b</me:dipl></w> <w><me:dipl>c</me:dipl></w></s>"#);
    let (original, body) = imported_roundtrip(&xml, |d| d.replace(".s{b ", ".s{")).unwrap();
    assert_eq!(body, original.replace(r#"<w><me:dipl>b</me:dipl></w>"#, ""));
    // So does a paragraph or division, even if empty
    let xml = menota_body(r#"<w><me:dipl>a</me:dipl></w></p><p><w><me:dipl>b</me:dipl></w>"#);
    let (original, body) = imported_roundtrip(&xml, |d| d.replace(" b", "")).unwrap();
    assert_eq!(body, original.replace(r#"<w><me:dipl>b</me:dipl></w>"#, ""));
}

#[test]
fn test_deletions_and_unclear_readings_with_markup_read_as_tokens() {
    let cases = [
        (r#"<w><me:facs><del>ny<gap/></del>t</me:facs></w>"#, ".p -{ny[...]}-t"),
        (r#"<w><me:facs>þ<unclear>&er;</unclear></me:facs></w>"#, ".p þ?{:er:}?"),
        // Text that would read as shorthand is escaped
        (r#"<w><me:facs><del>a:b:c</del></me:facs></w>"#, r".p -{a\:b\:c}-"),
    ];
    for (words, expected) in cases {
        let xml = menota_body(words);
        let import = parse(&xml).unwrap();
        assert_eq!(import.dsl, expected);
        let segments = import.imported_document.unwrap().segments;
        let from_xml: Vec<String> =
            segments.iter().filter_map(crate::importer::tei::patching::segment_dsl_token).collect();
        assert_eq!(from_xml, crate::importer::tei::patching::dsl_tokens(&import.dsl).unwrap(), "{expected}");
    }
    // An edited word keeps the gap in its deletion (Holm D 4)
    let xml = menota_body(r#"<w><me:facs><del>ny<gap reason="damage"/></del>t</me:facs></w> <w><me:facs>ok</me:facs></w>"#);
    let (_, body) = imported_roundtrip(&xml, |d| d.replace("}-t", "}-tt")).unwrap();
    assert!(body.contains(r#"<me:facs><del>ny<gap reason="damage"/></del>tt</me:facs>"#), "{body}");
}

#[test]
fn test_edited_numeral_keeps_its_attributes_and_markup() {
    let xml = menota_body(
        r#"<w><me:facs>ok</me:facs></w> <num value="10"><me:facs>.<foreign rend="red">x</foreign>.</me:facs><me:dipl>.x.</me:dipl></num> <w><me:facs>ok</me:facs></w>"#,
    );
    let (_, body) = imported_roundtrip(&xml, |d| {
        assert!(d.contains("#.x.#"), "{d}");
        d.replace(".x.#", ".xv.#")
    })
    .unwrap();
    assert!(
        body.contains(r#"<num value="10"><me:facs>.<foreign rend="red">x</foreign>v.</me:facs><me:dipl>.xv.</me:dipl></num>"#),
        "{body}"
    );
}

const MOVABLE: &str = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body>
<p><w lemma="á"><me:dipl>a</me:dipl></w> <w lemma="bé" me:msa="xNC"><me:dipl>b<foreign rend="red">e</foreign></me:dipl></w> <w lemma="sé"><me:dipl>c</me:dipl></w></p>
</body></text></TEI>"#;
const MOVED_WORD: &str = r#"<w lemma="bé" me:msa="xNC"><me:dipl>b<foreign rend="red">e</foreign></me:dipl></w>"#;

#[test]
fn test_a_word_cut_and_pasted_elsewhere_is_moved_with_its_markup() {
    let mut session = Session::open(MOVABLE);
    assert_eq!(session.dsl, ".p a be c");
    // Cut "be " and paste it after "c"
    let cut = TextChange { from_a: 5, to_a: 8, from_b: 5, to_b: 5 };
    session.compile_with(".p a c", cut, false).unwrap();
    let paste = TextChange { from_a: 6, to_a: 6, from_b: 6, to_b: 9 };
    let xml = session.compile_with(".p a c be", paste, false).unwrap();
    let (moved, c) = (xml.find(MOVED_WORD), xml.find(r#"lemma="sé""#));
    assert!(moved.is_some() && moved > c, "the word, as written, after c: {xml}");

    // In one compile, and without the editor's guidance too
    for blind in [false, true] {
        let mut session = Session::open(MOVABLE);
        let xml = session.compile(".p a c be", blind).unwrap();
        assert!(xml.find(MOVED_WORD) > xml.find(r#"lemma="sé""#), "blind: {blind}: {xml}");
        // And moved back, it is the file again
        let xml = session.compile(".p a be c", blind).unwrap();
        assert!(MOVABLE.contains(&xml), "blind: {blind}: {xml}");
    }
}

#[test]
fn test_a_moved_word_keeps_its_identity() {
    let session = Session::open(MOVABLE);
    let (patches, state) = session.patcher.compute(".p a c be", None).unwrap();
    assert!(patches.iter().any(|p| matches!(p, PatchOperation::Insert { moved: Some(_), .. })), "{patches:?}");
    // "be" is still original token 2 (after .p and a)
    assert_eq!(state.ids, vec![0, 1, 3, 2]);
}

#[test]
fn test_words_moved_ambiguously_are_not_taken_for_moves() {
    // Two "ok" deleted, one inserted: which one moved can't be told
    let xml = menota_body(r#"<w lemma="ok"><me:dipl>ok</me:dipl></w> <w><me:dipl>x</me:dipl></w> <w lemma="ok"><me:dipl>ok</me:dipl></w> <w><me:dipl>y</me:dipl></w>"#);
    let session = Session::open(&xml);
    assert_eq!(session.dsl, ".p ok x ok y");
    let (patches, _) = session.patcher.compute(".p x y ok", None).unwrap();
    assert!(!patches.iter().any(|p| matches!(p, PatchOperation::Insert { moved: Some(_), .. })), "{patches:?}");
}

#[test]
fn test_a_project_saved_with_older_rules_is_brought_onto_the_tree() {
    use crate::importer::tei::extraction::segments_to_dsl;
    use crate::importer::tei::onto_tree;
    use crate::importer::tei::segments::DSL_RULES;
    let xml = menota_body(r#"<w><me:dipl>a</me:dipl></w><w><me:dipl>b</me:dipl></w> <w><me:dipl>c</me:dipl></w> <w><me:dipl>d</me:dipl></w>"#);
    let import = parse(&xml).unwrap();
    assert_eq!(import.dsl, ".p a|b c d");
    let body = import.original_body_xml.clone().unwrap();
    let (pre, post) = (import.original_preamble.clone().unwrap(), import.original_postamble.clone().unwrap());
    // The same file as imported with rules 2, before neighbours were set apart
    let mut stored = import.imported_document.clone().unwrap();
    stored.dsl_rules = 2;
    let old = segments_to_dsl(&stored.segments, 2);
    assert_eq!(old.trim(), ".p ab c d");

    // Unedited: rewritten to the current form
    let on_tree = onto_tree(&stored, old.trim(), &pre, &body, &post).unwrap();
    assert_eq!((on_tree.dsl.as_str(), on_tree.rewritten), (".p a|b c d", 1));
    assert_eq!(on_tree.document.dsl_rules, DSL_RULES);

    // An edit stays as typed, and so do the layout and the words around it
    let on_tree = onto_tree(&stored, ".p ab\nc dd", &pre, &body, &post).unwrap();
    assert_eq!(on_tree.dsl, ".p a|b\nc dd");
    // Edited where the rules differ: left as typed
    let on_tree = onto_tree(&stored, ".p abx c d", &pre, &body, &post).unwrap();
    assert_eq!((on_tree.dsl.as_str(), on_tree.rewritten), (".p abx c d", 0));

    // A project of this version is left as it is
    let current = import.imported_document.unwrap();
    let on_tree = onto_tree(&current, ".p a|b c dd", &pre, &body, &post).unwrap();
    assert_eq!((on_tree.dsl.as_str(), on_tree.rewritten), (".p a|b c dd", 0));
}

#[test]
fn test_a_new_word_gets_the_files_levels() {
    // A diplomatic-only charter: no <choice>, no facsimile or normalized level
    let xml = menota_body(r#"<w><me:dipl>a</me:dipl></w> <w><me:dipl>b</me:dipl></w>"#);
    let (_, body) = imported_roundtrip(&xml, |d| d.replace(" b", " b cc")).unwrap();
    assert!(body.contains("<w>\n  <me:dipl>cc</me:dipl>\n</w>"), "{body}");
    // Two levels in a <choice>
    let xml = menota_body(
        r#"<w><choice><me:facs>a</me:facs><me:dipl>a</me:dipl></choice></w> <w><choice><me:facs>b</me:facs><me:dipl>b</me:dipl></choice></w>"#,
    );
    let (_, body) = imported_roundtrip(&xml, |d| d.replace(" b", " b cc")).unwrap();
    assert!(body.contains("<choice>\n    <me:facs>cc</me:facs>\n    <me:dipl>cc</me:dipl>\n  </choice>"), "{body}");
    assert!(!body.contains("<me:norm>"), "{body}");
}

#[test]
fn test_a_plain_word_in_a_file_with_levels_is_edited() {
    // DN bind 13 nr 11: most words plain, the others diplomatic
    let xml = menota_body(r#"<w>dernest</w> <w><me:dipl>och</me:dipl></w> <w><me:dipl>att</me:dipl></w>"#);
    let (_, body) = imported_roundtrip(&xml, |d| d.replace("dernest", "dernestt")).unwrap();
    assert!(body.contains("<w>dernestt</w>"), "{body}");
}

#[test]
fn test_only_one_lineation_is_shown_and_older_projects_lose_the_other() {
    use crate::importer::tei::onto_tree;
    use crate::importer::tei::segments::DSL_RULES;
    // A DN charter: the manuscript's lines and the printed edition's
    let xml = menota_body(
        r#"<lb ed="ms" n="1"/><w><me:dipl>a</me:dipl></w> <lb ed="DN" n="7"/><w><me:dipl>b</me:dipl></w> <w><me:dipl>c<lb ed="DN" n="8"/>d</me:dipl></w> <lb ed="ms" n="2"/><w><me:dipl>e</me:dipl></w>"#,
    );
    let import = parse(&xml).unwrap();
    let profile = import.imported_document.as_ref().unwrap().profile.clone().unwrap();
    assert_eq!(profile.line_edition.as_deref(), Some("ms"));
    assert_eq!(import.dsl, ".p \n//1 a b cd\n//2 e");
    // Unedited, the file comes back as it was; edited around the hidden breaks too
    let (original, body) = imported_roundtrip(&xml, |d| d.to_string()).unwrap();
    assert_eq!(body, original);
    let (original, body) = imported_roundtrip(&xml, |d| d.replace(" b ", " bb ").replace("cd", "cdd")).unwrap();
    assert!(body.contains(r#"<lb ed="DN" n="7"/><w><me:dipl>bb</me:dipl></w>"#), "{body}");
    assert!(body.contains(r#"<me:dipl>c<lb ed="DN" n="8"/>dd</me:dipl>"#), "{body}");
    assert_ne!(body, original);
    // The other lineation can be chosen
    let dn = crate::importer::tei::parse_with(&xml, Some(Some("DN".into()))).unwrap();
    assert_eq!(dn.dsl, ".p a\n//7 b c~//8d e");

    // A project imported before, showing all line breaks, loses the other's
    let before = crate::importer::tei::parse_with(&xml, Some(None)).unwrap();
    let mut stored = before.imported_document.unwrap();
    (stored.dsl_rules, stored.profile) = (5, None);
    let (pre, body, post) = (before.original_preamble.unwrap(), before.original_body_xml.unwrap(), before.original_postamble.unwrap());
    let on_tree = onto_tree(&stored, &before.dsl, &pre, &body, &post).unwrap();
    assert_eq!(on_tree.document.dsl_rules, DSL_RULES);
    assert_eq!(crate::importer::tei::patching::dsl_tokens(&on_tree.dsl).unwrap(), crate::importer::tei::patching::dsl_tokens(&import.dsl).unwrap());
}

#[test]
fn test_stored_shorthand_is_kept_where_it_reads_as_the_file() {
    use crate::importer::tei::patching::keep_as_written;
    let projected = ".p a b c\n//2 d";
    // Layout kept
    assert_eq!(keep_as_written(".p a  b\n c //2   d", projected), (".p a  b\n c //2   d".to_string(), 0));
    // The XML wins where they differ
    assert_eq!(keep_as_written(".p a  bx\n c //2   d", projected), (".p a  b\n c //2   d".to_string(), 1));
    assert_eq!(keep_as_written(".p a  b //2 d", projected), (".p a  b c //2 d".to_string(), 1));
    assert_eq!(keep_as_written(".p a  b c e //2 d", projected), (".p a  b c  //2 d".to_string(), 1));
}

#[test]
fn test_abbreviations_are_read_where_they_stand() {
    let dsl = |words: &str| parse(&menota_body(words)).unwrap().dsl;
    // A letter with the mark fused into it
    assert_eq!(
        dsl(r#"<w><choice><me:facs><am>&kbar;</am>gs</me:facs><me:dipl>k<ex>onun</ex>gs</me:dipl><me:norm>konungs</me:norm></choice></w>"#),
        ".p {:kbar:=konun}gs"
    );
    // A whole word for a sign
    assert_eq!(
        dsl(r#"<w><choice><me:facs><am>&et;</am></me:facs><me:dipl><ex>ok</ex></me:dipl><me:norm>ok</me:norm></choice></w>"#),
        ".p {:et:=ok}"
    );
    // Facsimile only, with a mark (Holm A 49)
    assert_eq!(dsl(r#"<w><me:facs>k<am>&bar;</am>r</me:facs></w>"#), ".p k{:bar:=}r");
    // The levels differ in letterforms too: the facsimile's, with the expansion
    assert_eq!(
        dsl(r#"<w><choice><me:facs>ꝼ<am>&bar;</am>r</me:facs><me:dipl>f<ex>yri</ex>r</me:dipl><me:norm>fyrir</me:norm></choice></w>"#),
        ".p ꝼ{:bar:=yri}r"
    );
    // Marks and expansions that don't pair up: the whole word
    assert_eq!(
        dsl(r#"<w><choice><me:facs>ꝼ<am>&bar;</am>r</me:facs><me:dipl>fyrir</me:dipl><me:norm>fyrir</me:norm></choice></w>"#),
        ".p .abbr[ꝼ:bar:r]{fyrir}"
    );
}

#[test]
fn test_a_project_with_whole_word_abbreviations_is_brought_onto_rules_7() {
    use crate::importer::tei::onto_tree;
    let xml = menota_body(
        r#"<w lemma="konungr"><choice><me:facs>k<am>&bar;</am>r</me:facs><me:dipl>k<ex>onung</ex>r</me:dipl><me:norm>konungr</me:norm></choice></w> <w><choice><me:facs>ok</me:facs><me:dipl>ok</me:dipl><me:norm>ok</me:norm></choice></w>"#,
    );
    let import = parse(&xml).unwrap();
    assert_eq!(import.dsl, ".p k{:bar:=onung}r ok");
    // As imported with rules 6: the word as a whole
    let mut stored = import.imported_document.clone().unwrap();
    stored.dsl_rules = 6;
    stored.profile = None;
    for segment in &mut stored.segments {
        if let Segment::Word { dsl_content, .. } = segment {
            if dsl_content.contains('{') {
                *dsl_content = ".abbr[k:bar:r]{konungr}".into();
            }
        }
    }
    let (pre, body, post) = (import.original_preamble.unwrap(), import.original_body_xml.unwrap(), import.original_postamble.unwrap());
    let on_tree = onto_tree(&stored, ".p .abbr[k:bar:r]{konungr} ok", &pre, &body, &post).unwrap();
    assert_eq!((on_tree.dsl.as_str(), on_tree.rewritten), (".p k{:bar:=onung}r ok", 1));
}

#[test]
fn test_an_expansion_after_a_hyphen_is_not_a_deletion() {
    // DG 8: "k{=on}-{=ong}r", where `-{` would open a deletion
    let xml = menota_body(r#"<w><me:dipl>k<ex>on</ex>-<ex>ong</ex>r</me:dipl></w>"#);
    let import = parse(&xml).unwrap();
    assert_eq!(import.dsl, r".p k{=on}\-{=ong}r");
    let (original, body) = imported_roundtrip(&xml, |d| d.to_string()).unwrap();
    assert_eq!(body, original);
}

#[test]
fn test_an_expansion_after_an_unclear_reading_keeps_it_closed() {
    // AM 162 B: an unclear reading right before an expansion: `}?{` is its close
    let xml = menota_body(r#"<w><me:dipl><unclear>a</unclear><ex>n</ex>d</me:dipl></w>"#);
    let import = parse(&xml).unwrap();
    assert_eq!(import.dsl, ".p ?{a}?{=n}d");
    let (original, body) = imported_roundtrip(&xml, |d| d.to_string()).unwrap();
    assert_eq!(body, original);
}

/// Column breaks and changes of hand are shown (rules 11), and typed,
/// changed or deleted in an imported file like page breaks.
#[test]
fn test_column_breaks_and_hands_in_imported_files() {
    let xml = wrap_body(r#"<p><w>a</w><cb n="A"/><handShift new="h1"/> <w>b</w></p>"#);
    assert_eq!(parse(&xml).unwrap().dsl, ".p a\n///c=A .hs{h1} b");
    let body = |edit: &dyn Fn(&str) -> String| imported_render(&xml, |d| edit(d)).map(|(_, b)| b.xml);
    assert_eq!(body(&|d| d.replace("///c=A", "///c=B")).unwrap(), r#"<body><p><w>a</w><cb n="B"/><handShift new="h1"/> <w>b</w></p></body>"#);
    assert_eq!(body(&|d| d.replace(".hs{h1}", ".hs{h2}")).unwrap(), r#"<body><p><w>a</w><cb n="A"/><handShift new="h2"/> <w>b</w></p></body>"#);
    assert_eq!(body(&|d| d.replace(".hs{h1}", "")).unwrap(), r#"<body><p><w>a</w><cb n="A"/> <w>b</w></p></body>"#);
    let typed = body(&|d| d.replace("} b", "} .hs{h3} b ///c=C")).unwrap();
    assert!(typed.contains(r#"<handShift new="h3"/>"#) && typed.contains(r#"<cb n="C"/>"#), "{typed}");
    // Inside a word, markup is part of the word
    assert!(imported_render(&xml, |dsl| dsl.replace("a\n", "*{a}*x\n")).is_ok());
}

/// Markup typed around words in an imported file wraps them; removed, its
/// content stays in its place; renamed, its content and attributes go into
/// an element of the new name. The rest of the file is untouched.
#[test]
fn test_markup_around_words_is_edited_in_the_tree() {
    let body = |inner: &str| wrap_body(&format!("<p><w>a</w> <w>b</w> <w>c</w>{inner}</p>"));
    let xml = body(" <q who=\"A\"><w>d</w> <w>e</w></q> <w>f</w>");
    let render = |edit: &dyn Fn(&str) -> String| {
        imported_render(&xml, |dsl| edit(dsl)).map(|(_, body)| body.xml.trim_start_matches("<body>").trim_end_matches("</body>").to_string())
    };
    // Wrap
    assert_eq!(
        render(&|dsl| dsl.replace("a b c", "a .s{b c}")).unwrap(),
        "<p><w>a</w> <s><w>b</w> <w>c</w></s> <q who=\"A\"><w>d</w> <w>e</w></q> <w>f</w></p>"
    );
    // Wrap with a new word in it, and nested
    assert_eq!(
        render(&|dsl| dsl.replace("a b c", ".s{a .name{b} x} c")).unwrap().replace('\n', ""),
        "<p><s><w>a</w> <name><w>b</w></name><w>x</w></s> <w>c</w> <q who=\"A\"><w>d</w> <w>e</w></q> <w>f</w></p>"
    );
    // Unwrap
    assert_eq!(
        render(&|dsl| dsl.replace(".q{d e}", "d e")).unwrap(),
        "<p><w>a</w> <w>b</w> <w>c</w> <w>d</w> <w>e</w> <w>f</w></p>"
    );
    // Rename, keeping the attributes
    assert_eq!(
        render(&|dsl| dsl.replace(".q{d e}", "&{d e}")).unwrap(),
        "<p><w>a</w> <w>b</w> <w>c</w> <seg who=\"A\"><w>d</w> <w>e</w></seg> <w>f</w></p>"
    );
    // A word deleted from markup leaves the markup
    assert_eq!(
        render(&|dsl| dsl.replace(".q{d e}", ".q{e}")).unwrap(),
        "<p><w>a</w> <w>b</w> <w>c</w> <q who=\"A\"> <w>e</w></q> <w>f</w></p>"
    );
    // Across the end of existing markup: refused
    let error = render(&|dsl| dsl.replace("c .q{d e}", ".s{c .q{d} e}")).unwrap_err();
    assert!(error.contains("doesn't close") || error.contains("same element"), "{error}");

}

#[test]
fn test_markup_across_elements_is_refused() {
    let xml = wrap_body("<p><s><w>a</w> <w>b</w></s> <w>c</w></p>");
    let render = |from: &str, to: &str| imported_render(&xml, |dsl| dsl.replace(from, to)).map(|(_, b)| b.xml);
    // Inside `<s>`: fine
    assert_eq!(render(".s{a b} c", ".s{a .q{b}} c").unwrap(), "<body><p><s><w>a</w> <q><w>b</w></q></s> <w>c</w></p></body>");
    // A word moved out of it goes with its markup
    assert_eq!(render(".s{a b} c", ".s{a} .q{b c}").unwrap(), "<body><p><s><w>a</w> </s><q>\n<w>b</w> <w>c</w></q></p></body>");
    // Moving its end: refused
    for to in [".s{a .q{b} c}", ".s{a b c}"] {
        let error = render(".s{a b} c", to).unwrap_err();
        assert!(error.contains("same element"), "{to}: {error}");
    }
}

/// Markup around words is shown (DSL rules 10), and reads back as the same
/// tokens; unedited, the file comes back byte for byte.
#[test]
fn test_markup_around_words_is_shown() {
    let xml = wrap_body(
        "<p><q><w>Ok</w> <w>sagði</w></q> <persName><forename><w>Ari</w></forename> <w>fróði</w></persName><pc>.</pc>\
         <choice><sic><w>nu</w></choice> <hi rend=\"red\"><w>A</w></hi><seg type=\"nb\"><w>b</w></seg></p>"
            .replace("<choice><sic><w>nu</w></choice>", "<choice><sic><w>nu</w></sic><corr><w>nú</w></corr></choice>")
            .as_str(),
    );
    let import = parse(&xml).unwrap();
    assert_eq!(import.dsl, ".p .q{Ok sagði} .psname{.fname{Ari} fróði} . .corr{nu=nú} *{A}* &{b}");
    let doc = import.imported_document.as_ref().unwrap();
    let from_xml: Vec<String> = doc.segments.iter().filter_map(crate::importer::tei::patching::segment_dsl_token).collect();
    assert_eq!(from_xml, crate::importer::tei::patching::dsl_tokens(&import.dsl).unwrap());
    let (_, body) = imported_render(&xml, |dsl| dsl.to_string()).unwrap();
    assert_eq!(body.xml, xml);
}


/// A project from before markup was shown (rules 9) gets it in its
/// shorthand when opened, where it stands among the unedited words, so the
/// first compile doesn't take it out of the file.
#[test]
fn test_markup_goes_into_an_older_projects_shorthand() {
    use crate::importer::tei::extraction::segments_to_dsl;
    use crate::importer::tei::onto_tree;
    use crate::importer::tei::patching::keep_as_written;
    let xml = menota_body(
        r#"<w><me:dipl>a</me:dipl></w> <q><w><me:dipl>b</me:dipl></w> <persName><w><me:dipl>c</me:dipl></w></persName></q> <w><me:dipl>d</me:dipl></w>"#,
    );
    let import = parse(&xml).unwrap();
    assert_eq!(import.dsl, ".p a .q{b .psname{c}} d");
    let body = import.original_body_xml.clone().unwrap();
    let (pre, post) = (import.original_preamble.clone().unwrap(), import.original_postamble.clone().unwrap());
    let mut stored = import.imported_document.clone().unwrap();
    stored.dsl_rules = 9;
    for segment in &mut stored.segments {
        if let Segment::Structural { shown, .. } = segment {
            *shown = None;
        }
    }
    let old = segments_to_dsl(&stored.segments, 9);
    assert_eq!(old.trim(), ".p a b c d");
    let on_tree = onto_tree(&stored, old.trim(), &pre, &body, &post).unwrap();
    assert_eq!(on_tree.dsl, ".p a .q{b .psname{c}} d");
    // With edits around it, and a new word at its edge outside it
    let on_tree = onto_tree(&stored, ".p ax b\nc y d", &pre, &body, &post).unwrap();
    assert_eq!(on_tree.dsl, ".p ax .q{b\n.psname{c}} y d");
    let patches = compute_patches(&on_tree.document.segments, on_tree.document.dsl_rules, &on_tree.dsl).unwrap();
    assert!(!patches.iter().any(|p| matches!(p, PatchOperation::Delete { .. })), "{patches:?}");

    // A project saved as its file (2.0) keeps its shorthand, the markup put in
    let (dsl, _) = keep_as_written(".p a b\nc d", &import.dsl);
    let tokens = crate::importer::tei::patching::dsl_tokens;
    assert_eq!(tokens(&dsl).unwrap(), tokens(&import.dsl).unwrap(), "{dsl}");
}



/// Markup inside a word is shown where it stands (rules 10), unless it
/// would read as markup around the word; an edit of the word keeps it on the
/// levels that have it.
#[test]
fn test_markup_inside_words_is_shown() {
    let xml = menota_body(
        r#"<w><me:facs><hi rend="orncp">A</hi>nnat</me:facs><me:dipl>Annat</me:dipl></w> <w><me:facs><c type="littNot">S</c>tephan</me:facs><me:dipl>Stephan</me:dipl></w> <w><me:facs><hi>ok</hi></me:facs><me:dipl>ok</me:dipl></w> <w><me:dipl>mikill<choice><sic>mikit</sic><corr>mikit</corr></choice></me:dipl></w>"#,
    );
    let import = parse(&xml).unwrap();
    assert_eq!(import.dsl, ".p *{A}*nnat .c{S}tephan ok mikill.corr{mikit=mikit}");
    let (original, body) = imported_roundtrip(&xml, |d| d.replace("*{A}*nnat", "*{A}*nnatt")).unwrap();
    assert_eq!(body, original.replace("<hi rend=\"orncp\">A</hi>nnat</me:facs><me:dipl>Annat", "<hi rend=\"orncp\">A</hi>nnatt</me:facs><me:dipl>Annatt"));
}

#[test]
fn test_column_breaks_and_spaces_inside_words() {
    let xml = menota_body(r#"<w><me:facs>hel<cb n="B"/>lo</me:facs></w> <w><me:facs>a<space quantity="3"/>b</me:facs></w>"#);
    let import = parse(&xml).unwrap();
    assert_eq!(import.dsl, ".p hel~///c={B}lo a.space[3]b");
    let (original, body) = imported_roundtrip(&xml, |d| d.replace("lo ", "lox ")).unwrap();
    assert_eq!(body, original.replace("lo</me:facs>", "lox</me:facs>"));
}
