use super::tei::parse;
use crate::importer::tei::helpers;
use crate::importer::tei::patching::{
    apply_patches_and_reconstruct, compiler_config_for, compute_patches, render_body,
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
    // pb adds a newline BEFORE the marker
    assert_eq!(result.dsl, "page 1\n///page 2\n///10vpage 3");
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
    assert_eq!(result.dsl, "-{deleted}-+{added}+");
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
    assert_eq!(result.dsl, "word^{editorial comment}next");
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

    // DSL contains the full text
    assert_eq!(result.dsl.trim(), "Maðr");
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

/// Like [`imported_roundtrip`], with the full render result.
fn imported_render(
    xml: &str,
    edit: impl Fn(&str) -> String,
) -> Result<(String, RenderedBody), String> {
    let import = parse(xml)?;
    let doc = import.imported_document.expect("manifest");
    let original_body = import.original_body_xml.expect("body");
    let patches = compute_patches(&doc.segments, doc.dsl_rules, &edit(&import.dsl))?;
    let mut compiler = Compiler::new().with_config(compiler_config_for(&doc));
    let body = render_body(&doc, Some(&original_body), &patches, &mut compiler)?;
    Ok((original_body, body))
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
    // First plain word outside any block construct (a word inside `.head{...}`
    // is part of one opaque segment and would regenerate the whole head).
    let mut depth = 0i32;
    let mut first = None;
    for token in import.dsl.split_whitespace() {
        if depth == 0 && token.len() > 3 && token.chars().all(char::is_alphabetic) {
            first = Some(token.to_string());
            break;
        }
        depth += token.matches('{').count() as i32 - token.matches('}').count() as i32;
    }
    let first = first.unwrap();
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
    let xml = "<TEI><text><body><p>a <hi>b c</hi> d</p></body></text></TEI>";
    let result = imported_roundtrip(xml, |d| d.replace("b c", "b .p c"));
    let error = result.expect_err("a split inside <hi> would break the XML");
    assert!(error.contains("inside <hi>"), "{error}");
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
    // Split after the first sentence end (". " closes an <s>) that allows it;
    // mid-sentence splits are refused, as <s> can't be split
    let mut result = None;
    for (at, _) in edited.match_indices(". ").skip(5).take(50) {
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
        let config = CompilerConfig { word_wrap: true, auto_line_numbers: false, multi_level, wrap_pages: false };
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
    // unrelated edit must still be one change, not one per middle dot
    doc.dsl_rules = 0;
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
    patcher: Patcher,
    dsl: String,
}

impl Session {
    fn open(xml: &str) -> Self {
        let import = parse(xml).unwrap();
        let doc = import.imported_document.unwrap();
        let patcher = Patcher::new(&doc.segments, doc.dsl_rules);
        let mut session = Session { doc, body: import.original_body_xml.unwrap(), patcher, dsl: import.dsl.clone() };
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
        let mut compiler = Compiler::new().with_config(compiler_config_for(&self.doc));
        let body = render_body(&self.doc, Some(&self.body), &patches, &mut compiler)?;
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
        let config = CompilerConfig { word_wrap: true, auto_line_numbers: false, multi_level: true, wrap_pages: false };
        let xml = Compiler::new().with_config(config).compile(dsl).unwrap();
        let back = parse(&format!(r#"<TEI xmlns:me="http://www.menota.org/ns/1.0"><text><body>{xml}</body></text></TEI>"#)).unwrap().dsl;
        assert_eq!(back, dsl, "{xml}");
    }
}
