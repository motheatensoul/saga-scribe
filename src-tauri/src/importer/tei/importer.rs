use crate::metadata::{
    Availability, DateRange, History, Language, Metadata, MsContents, MsIdentifier, Person,
    PhysDesc, PublicationStmt, RespStmt, TitleStmt,
};
use crate::importer::tei::extraction::Extractor;
use crate::importer::tei::helpers;
use crate::importer::tei::segments::ImportedDocument;
use libxml::parser::Parser;
use libxml::tree::{Document, Node, NodeType};

/// Result of parsing a TEI-XML file
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    /// The DSL content extracted from the body
    pub dsl: String,
    /// Metadata extracted from teiHeader (if present)
    pub metadata: Option<Metadata>,
    /// Segment manifest for imported documents (enables round-trip fidelity)
    pub imported_document: Option<ImportedDocument>,
    /// Original body XML (for imported documents)
    pub original_body_xml: Option<String>,
    /// Original XML preamble (everything before <body>)
    pub original_preamble: Option<String>,
    /// Original XML postamble (everything after </body>)
    pub original_postamble: Option<String>,
    /// Whether this file was imported in "imported mode" (preserves structure)
    pub is_imported_mode: bool,
}

/// Parses TEI-XML content and converts it to Saga-Scribe DSL, also extracting metadata
pub fn parse(xml_content: &str) -> Result<ImportResult, String> {
    let parser = Parser::default();

    // Attempt to parse
    let doc = parser
        .parse_string(xml_content)
        .map_err(|e| format!("Failed to parse XML: {}", e))?;

    let root = doc
        .get_root_element()
        .ok_or("No root element found")?;

    // Extract metadata from teiHeader
    let metadata = extract_metadata(&doc, &root);

    // Find the <body> element specifically to avoid importing header metadata
    let body = find_body(&root).ok_or("No <body> element found in XML")?;

    // Check if this is a MENOTA file (has me:facs/me:dipl/me:norm structure)
    let is_menota = has_menota_structure(&body);

    // Split original XML into preamble/body/postamble for exact preservation
    let (original_preamble, original_body_xml, original_postamble) =
        match split_xml_sections(xml_content) {
            Some((preamble, body_xml, postamble)) => (preamble, body_xml, postamble),
            None => (
                String::new(),
                helpers::serialize_node(&body),
                String::new(),
            ),
        };

    // Extract segments using the new segment-based extractor
    let mut extractor = Extractor::new();
    let segments = extractor.extract_segments(&body);

    // Generate DSL from segments (this is the new, segment-aware path)
    let dsl = crate::importer::tei::extraction::segments_to_dsl(&segments);

    // Create the imported document manifest
    let imported_document = ImportedDocument {
        segments,
        is_menota,
    };

    // Trim output to avoid massive trailing/leading whitespace
    let trimmed = dsl.trim();

    Ok(ImportResult {
        dsl: trimmed.to_string(),
        metadata,
        imported_document: Some(imported_document),
        original_body_xml: Some(original_body_xml),
        original_preamble: Some(original_preamble),
        original_postamble: Some(original_postamble),
        is_imported_mode: true,
    })
}

/// Split original XML into preamble/body/postamble sections using string search.
/// Returns None if <body> tags cannot be found.
fn split_xml_sections(xml: &str) -> Option<(String, String, String)> {
    let body_open = xml.find("<body")?;
    let body_open_end = xml[body_open..].find('>')? + body_open + 1;
    let body_close = xml[body_open_end..].find("</body>")? + body_open_end;
    let body_end = body_close + "</body>".len();

    Some((
        xml[..body_open].to_string(),
        xml[body_open..body_end].to_string(),
        xml[body_end..].to_string(),
    ))
}

/// Check if the document has MENOTA multi-level structure
fn has_menota_structure(body: &Node) -> bool {
    // Look for any <w> element with me:facs child
    fn check_node(node: &Node) -> bool {
        if node.get_type() == Some(NodeType::ElementNode) {
            let name = node.get_name();
            if name == "w" {
                // Check for me:facs child
                let mut child = node.get_first_child();
                while let Some(c) = child {
                    if c.get_type() == Some(NodeType::ElementNode) {
                        let child_name = c.get_name();
                        if child_name == "me:facs" || child_name.ends_with(":facs") {
                            return true;
                        }
                        // Also check for <choice> containing me:facs
                        if child_name == "choice" {
                            let mut gc = c.get_first_child();
                            while let Some(g) = gc {
                                if g.get_type() == Some(NodeType::ElementNode) {
                                    let gc_name = g.get_name();
                                    if gc_name == "me:facs" || gc_name.ends_with(":facs") {
                                        return true;
                                    }
                                }
                                gc = g.get_next_sibling();
                            }
                        }
                    }
                    child = c.get_next_sibling();
                }
            }
            // Recurse into children
            let mut child = node.get_first_child();
            while let Some(c) = child {
                if check_node(&c) {
                    return true;
                }
                child = c.get_next_sibling();
            }
        }
        false
    }
    check_node(body)
}

/// Legacy function for backward compatibility - just returns DSL
#[allow(dead_code)]
pub fn parse_dsl_only(xml_content: &str) -> Result<String, String> {
    parse(xml_content).map(|r| r.dsl)
}

// ============================================================================
// Metadata Extraction
// ============================================================================

fn extract_metadata(_doc: &Document, root: &Node) -> Option<Metadata> {
    let header = find_element(root, "teiHeader")?;

    let mut metadata = Metadata::default();

    // Extract fileDesc
    if let Some(file_desc) = find_element(&header, "fileDesc") {
        // titleStmt
        if let Some(title_stmt) = find_element(&file_desc, "titleStmt") {
            metadata.title_stmt = extract_title_stmt(&title_stmt);
        }

        // publicationStmt
        if let Some(pub_stmt) = find_element(&file_desc, "publicationStmt") {
            metadata.publication_stmt = extract_publication_stmt(&pub_stmt);
        }

        // sourceDesc -> msDesc
        if let Some(source_desc) = find_element(&file_desc, "sourceDesc") {
            if let Some(ms_desc) = find_element(&source_desc, "msDesc") {
                // msIdentifier
                if let Some(ms_id) = find_element(&ms_desc, "msIdentifier") {
                    metadata.ms_identifier = extract_ms_identifier(&ms_id);
                }

                // msContents
                if let Some(ms_contents) = find_element(&ms_desc, "msContents") {
                    metadata.ms_contents = extract_ms_contents(&ms_contents);
                }

                // physDesc
                if let Some(phys_desc) = find_element(&ms_desc, "physDesc") {
                    metadata.phys_desc = extract_phys_desc(&phys_desc);
                }

                // history
                if let Some(history) = find_element(&ms_desc, "history") {
                    metadata.history = extract_history(&history);
                }
            }
        }
    }

    // Extract profileDesc -> langUsage
    if let Some(profile_desc) = find_element(&header, "profileDesc") {
        if let Some(lang_usage) = find_element(&profile_desc, "langUsage") {
            metadata.languages = extract_languages(&lang_usage);
        }
    }

    // Extract encodingDesc -> editorialDecl -> normalization
    if let Some(encoding_desc) = find_element(&header, "encodingDesc") {
        if let Some(editorial_decl) = find_element(&encoding_desc, "editorialDecl") {
            if let Some(normalization) = find_element(&editorial_decl, "normalization") {
                if let Some(p) = find_element(&normalization, "p") {
                    metadata.encoding_notes = Some(get_text_content(&p));
                }
            }
        }
    }

    // Only return metadata if we found something meaningful
    if metadata.title_stmt.title.is_some()
        || metadata.ms_identifier.repository.is_some()
        || metadata.ms_identifier.idno.is_some()
        || !metadata.languages.is_empty()
    {
        Some(metadata)
    } else {
        None
    }
}

fn extract_title_stmt(node: &Node) -> TitleStmt {
    let mut stmt = TitleStmt::default();

    // Title (first one without type attribute, or just first)
    for child in element_children(node) {
        if child.get_name() == "title" {
            let type_attr = child.get_property("type");
            if type_attr.is_none() || type_attr.as_deref() == Some("main") {
                if stmt.title.is_none() {
                    stmt.title = Some(get_text_content(&child));
                }
            } else if type_attr.as_deref() == Some("sub") {
                stmt.subtitle = Some(get_text_content(&child));
            }
        }
    }

    // Author
    if let Some(author) = find_element(node, "author") {
        stmt.author = Some(get_text_content(&author));
    }

    // Editors
    for child in element_children(node) {
        if child.get_name() == "editor" {
            let name = get_text_content(&child);
            if !name.is_empty() {
                let mut person = Person {
                    name,
                    identifier: None,
                    identifier_type: None,
                };
                // Check for ref attribute (e.g., "orcid:0000-0000-0000-0000")
                if let Some(ref_attr) = child.get_property("ref") {
                    if let Some((id_type, id)) = ref_attr.split_once(':') {
                        person.identifier_type = Some(id_type.to_string());
                        person.identifier = Some(id.to_string());
                    }
                }
                stmt.editors.push(person);
            }
        }
    }

    // respStmt
    for child in element_children(node) {
        if child.get_name() == "respStmt" {
            let resp = find_element(&child, "resp").map(|n| get_text_content(&n));
            let name = find_element(&child, "name").map(|n| get_text_content(&n));
            if let (Some(resp), Some(name)) = (resp, name) {
                if !resp.is_empty() && !name.is_empty() {
                    stmt.resp_stmts.push(RespStmt { resp, name });
                }
            }
        }
    }

    stmt
}

fn extract_publication_stmt(node: &Node) -> PublicationStmt {
    let mut stmt = PublicationStmt::default();

    if let Some(publisher) = find_element(node, "publisher") {
        stmt.publisher = Some(get_text_content(&publisher));
    }

    if let Some(pub_place) = find_element(node, "pubPlace") {
        stmt.pub_place = Some(get_text_content(&pub_place));
    }

    if let Some(date) = find_element(node, "date") {
        // Prefer @when attribute, fall back to text content
        stmt.date = date
            .get_property("when")
            .or_else(|| Some(get_text_content(&date)));
    }

    if let Some(distributor) = find_element(node, "distributor") {
        stmt.distributor = Some(get_text_content(&distributor));
    }

    // idno
    if let Some(idno) = find_element(node, "idno") {
        stmt.idno = Some(get_text_content(&idno));
        stmt.idno_type = idno.get_property("type");
    }

    // availability
    if let Some(avail) = find_element(node, "availability") {
        let availability = Availability {
            status: avail.get_property("status"),
            license: find_element(&avail, "licence").map(|l| get_text_content(&l)),
            license_url: find_element(&avail, "licence").and_then(|l| l.get_property("target")),
            note: find_element(&avail, "p").map(|p| get_text_content(&p)),
        };

        stmt.availability = Some(availability);
    }

    stmt
}

fn extract_ms_identifier(node: &Node) -> MsIdentifier {
    let mut id = MsIdentifier::default();

    if let Some(country) = find_element(node, "country") {
        id.country = Some(get_text_content(&country));
    }

    if let Some(settlement) = find_element(node, "settlement") {
        id.settlement = Some(get_text_content(&settlement));
    }

    if let Some(repository) = find_element(node, "repository") {
        id.repository = Some(get_text_content(&repository));
    }

    if let Some(collection) = find_element(node, "collection") {
        id.collection = Some(get_text_content(&collection));
    }

    if let Some(idno) = find_element(node, "idno") {
        id.idno = Some(get_text_content(&idno));
    }

    if let Some(ms_name) = find_element(node, "msName") {
        id.ms_name = Some(get_text_content(&ms_name));
    }

    // altIdentifier elements
    for child in element_children(node) {
        if child.get_name() == "altIdentifier" {
            if let Some(idno) = find_element(&child, "idno") {
                let id_type = child.get_property("type").unwrap_or_default();
                let idno_text = get_text_content(&idno);
                if !idno_text.is_empty() {
                    id.alt_identifiers.push(crate::metadata::AltIdentifier {
                        id_type,
                        idno: idno_text,
                    });
                }
            }
        }
    }

    id
}

fn extract_ms_contents(node: &Node) -> MsContents {
    let mut contents = MsContents::default();

    if let Some(summary) = find_element(node, "summary") {
        contents.summary = Some(get_text_content(&summary));
    }

    if let Some(text_lang) = find_element(node, "textLang") {
        contents.text_lang = text_lang.get_property("mainLang");
        let note = get_text_content(&text_lang);
        if !note.is_empty() && contents.text_lang.as_deref() != Some(&note) {
            contents.text_lang_note = Some(note);
        }
    }

    contents
}

fn extract_phys_desc(node: &Node) -> PhysDesc {
    let mut desc = PhysDesc::default();

    // objectDesc -> supportDesc
    if let Some(object_desc) = find_element(node, "objectDesc") {
        if let Some(support_desc) = find_element(&object_desc, "supportDesc") {
            if let Some(support) = find_element(&support_desc, "support") {
                desc.material = Some(get_text_content(&support));
            }
            if let Some(extent) = find_element(&support_desc, "extent") {
                desc.extent = Some(get_text_content(&extent));
            }
            if let Some(foliation) = find_element(&support_desc, "foliation") {
                desc.foliation = Some(get_text_content(&foliation));
            }
            if let Some(condition) = find_element(&support_desc, "condition") {
                desc.condition = Some(get_text_content(&condition));
            }
        }
        if let Some(layout_desc) = find_element(&object_desc, "layoutDesc") {
            if let Some(layout) = find_element(&layout_desc, "layout") {
                desc.layout = Some(get_text_content(&layout));
            }
        }
    }

    // handDesc
    if let Some(hand_desc) = find_element(node, "handDesc") {
        for child in element_children(&hand_desc) {
            if child.get_name() == "handNote" {
                let hand = crate::metadata::HandNote {
                    id: child.get_property("xml:id").or_else(|| child.get_property("id")),
                    script: child.get_property("script"),
                    scope: child.get_property("scope"),
                    description: Some(get_text_content(&child)),
                };
                desc.hands.push(hand);
            }
        }
    }

    desc
}

fn extract_history(node: &Node) -> History {
    let mut history = History::default();

    if let Some(origin) = find_element(node, "origin") {
        if let Some(orig_date) = find_element(&origin, "origDate") {
            let date_range = DateRange {
                display: Some(get_text_content(&orig_date)),
                when: orig_date.get_property("when"),
                not_before: orig_date.get_property("notBefore"),
                not_after: orig_date.get_property("notAfter"),
            };
            history.orig_date = Some(date_range);
        }

        if let Some(orig_place) = find_element(&origin, "origPlace") {
            history.orig_place = Some(get_text_content(&orig_place));
        }
    }

    if let Some(provenance) = find_element(node, "provenance") {
        history.provenance = Some(get_text_content(&provenance));
    }

    if let Some(acquisition) = find_element(node, "acquisition") {
        history.acquisition = Some(get_text_content(&acquisition));
    }

    history
}

fn extract_languages(node: &Node) -> Vec<Language> {
    let mut languages = Vec::new();

    for child in element_children(node) {
        if child.get_name() == "language" {
            if let Some(ident) = child.get_property("ident") {
                let usage = child
                    .get_property("usage")
                    .and_then(|s| s.parse::<u8>().ok());
                let name = get_text_content(&child);
                languages.push(Language {
                    ident,
                    usage,
                    name: if name.is_empty() { None } else { Some(name) },
                });
            }
        }
    }

    languages
}

// ============================================================================
// Helper Functions
// ============================================================================

fn find_element(node: &Node, name: &str) -> Option<Node> {
    let mut child = node.get_first_child();
    while let Some(c) = child {
        if c.get_type() == Some(NodeType::ElementNode) && c.get_name() == name {
            return Some(c);
        }
        child = c.get_next_sibling();
    }
    None
}

fn element_children(node: &Node) -> Vec<Node> {
    let mut children = Vec::new();
    let mut child = node.get_first_child();
    while let Some(c) = child {
        if c.get_type() == Some(NodeType::ElementNode) {
            children.push(c.clone());
        }
        child = c.get_next_sibling();
    }
    children
}

fn get_text_content(node: &Node) -> String {
    let content = node.get_content();
    // Normalize whitespace
    content.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ============================================================================
// DSL Extraction (existing code)
// ============================================================================

fn find_body(node: &Node) -> Option<Node> {
    if node.get_name() == "body" {
        return Some(node.clone());
    }

    let mut child = node.get_first_child();
    while let Some(c) = child {
        if c.get_type() == Some(NodeType::ElementNode) {
            if let Some(found) = find_body(&c) {
                return Some(found);
            }
        }
        child = c.get_next_sibling();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metadata_from_tei() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt>
        <title>Njáls saga</title>
        <author>Anonymous</author>
        <editor ref="orcid:0000-0001-2345-6789">Test Editor</editor>
      </titleStmt>
      <publicationStmt>
        <publisher>Test Publisher</publisher>
        <date when="2024">2024</date>
      </publicationStmt>
      <sourceDesc>
        <msDesc>
          <msIdentifier>
            <country>Iceland</country>
            <settlement>Reykjavík</settlement>
            <repository>Stofnun Árna Magnússonar</repository>
            <idno>GKS 2870 4to</idno>
          </msIdentifier>
        </msDesc>
      </sourceDesc>
    </fileDesc>
    <profileDesc>
      <langUsage>
        <language ident="non" usage="95">Old Norse</language>
        <language ident="lat" usage="5">Latin</language>
      </langUsage>
    </profileDesc>
  </teiHeader>
  <text>
    <body>
      <p>Test content</p>
    </body>
  </text>
</TEI>"#;

        let result = parse(xml).unwrap();
        assert_eq!(result.dsl.trim(), "Test content");

        let metadata = result.metadata.unwrap();
        assert_eq!(metadata.title_stmt.title, Some("Njáls saga".to_string()));
        assert_eq!(metadata.title_stmt.author, Some("Anonymous".to_string()));
        assert_eq!(metadata.title_stmt.editors.len(), 1);
        assert_eq!(metadata.title_stmt.editors[0].name, "Test Editor");
        assert_eq!(
            metadata.title_stmt.editors[0].identifier_type,
            Some("orcid".to_string())
        );

        assert_eq!(
            metadata.publication_stmt.publisher,
            Some("Test Publisher".to_string())
        );
        assert_eq!(metadata.publication_stmt.date, Some("2024".to_string()));

        assert_eq!(metadata.ms_identifier.country, Some("Iceland".to_string()));
        assert_eq!(
            metadata.ms_identifier.settlement,
            Some("Reykjavík".to_string())
        );
        assert_eq!(
            metadata.ms_identifier.idno,
            Some("GKS 2870 4to".to_string())
        );

        assert_eq!(metadata.languages.len(), 2);
        assert_eq!(metadata.languages[0].ident, "non");
        assert_eq!(metadata.languages[0].usage, Some(95));
    }

    #[test]
    fn test_parse_without_header() {
        let xml = r#"<?xml version="1.0"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <text>
    <body>
      <p>Just body content</p>
    </body>
  </text>
</TEI>"#;

        let result = parse(xml).unwrap();
        assert_eq!(result.dsl.trim(), "Just body content");
        assert!(result.metadata.is_none());
    }

    #[test]
    fn test_import_note_element() {
        let xml = r#"<?xml version="1.0"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <text>
    <body>
      <p>Some text<note>editorial comment</note> more text</p>
    </body>
  </text>
</TEI>"#;

        let result = parse(xml).unwrap();
        assert!(
            result.dsl.contains("^{editorial comment}"),
            "Expected note syntax ^{{...}} in: {}",
            result.dsl
        );
    }

    #[test]
    fn test_import_note_with_nested_elements() {
        let xml = r#"<?xml version="1.0"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <text>
    <body>
      <p>Text<note>A note with <supplied>supplied text</supplied> inside</note></p>
    </body>
  </text>
</TEI>"#;

        let result = parse(xml).unwrap();
        // Note should contain the nested supplied element converted to DSL
        assert!(
            result.dsl.contains("^{"),
            "Expected note syntax in: {}",
            result.dsl
        );
        assert!(
            result.dsl.contains("<supplied text>"),
            "Expected supplied syntax inside note in: {}",
            result.dsl
        );
    }
}
