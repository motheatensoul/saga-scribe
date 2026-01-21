use libxml::parser::Parser;
use saga_scribe::importer::tei::dsl::find_body;
use saga_scribe::importer::tei::extraction::Extractor;

#[test]
fn test_extractor_no_hang() {
    let xml = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0">
      <teiHeader><fileDesc><titleStmt><title>Test</title></titleStmt></fileDesc></teiHeader>
      <text><body>
        <p><w><facs>word</facs></w> <pc>.</pc></p>
      </body></text>
    </TEI>"#;
    let parser = Parser::default();
    let doc = parser.parse_string(xml).unwrap();
    let root = doc.get_root_element().unwrap();
    let body = find_body(&root).unwrap();
    let mut extractor = Extractor::new();
    let segments = extractor.extract_segments(&body);
    assert!(!segments.is_empty());
}
