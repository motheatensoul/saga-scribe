use super::tei::parse;
use crate::importer::tei::helpers;
use crate::importer::tei::patching::{apply_patches_and_reconstruct, compute_patches};
use crate::importer::tei::segments::Segment;
use crate::parser::{Compiler, CompilerConfig, Lexer};
use libxml::parser::Parser;
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
    assert_eq!(result.dsl, "Line 1\n// Line 2");
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
    let parser = Parser::default();
    let doc = parser
        .parse_string(xml)
        .expect("Should parse XML for normalization");
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
        .join("static/tests/HolmPerg-34-4to-MLL.xml");

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
    let patches = compute_patches(&imported_doc.segments, &import_result.dsl);
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
        .join("static/tests/HolmPerg-34-4to-MLL.xml");

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
        .join("static/tests/HolmPerg-34-4to-MLL.xml");

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

    let patches = compute_patches(&imported_doc.segments, &import_result.dsl);
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

