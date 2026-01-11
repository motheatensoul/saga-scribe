use crate::metadata::{
    Availability, DateRange, History, Language, Metadata, MsContents, MsIdentifier, Person,
    PhysDesc, PublicationStmt, RespStmt, TitleStmt,
};
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

    let mut output = String::new();
    // Only process children of body
    process_children(&body, &mut output)?;

    // Trim output to avoid massive trailing/leading whitespace from XML structure
    let trimmed = output.trim();

    Ok(ImportResult {
        dsl: trimmed.to_string(),
        metadata,
    })
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

fn process_node(node: &Node, output: &mut String) -> Result<(), String> {
    match node.get_type() {
        Some(NodeType::ElementNode) => {
            let name = node.get_name();
            match name.as_str() {
                "lb" => {
                    output.push('\n'); // Start with newline so // is at start of line
                    output.push_str("//");
                    if let Some(n) = node.get_property("n") {
                        output.push_str(&n);
                    }
                }
                "pb" => {
                    output.push('\n'); // Start with newline so /// is at start of line
                    output.push_str("///");
                    if let Some(n) = node.get_property("n") {
                        output.push_str(&n);
                    }
                }
                "gap" => {
                    output.push_str("[...]");
                }
                "supplied" => {
                    output.push('<');
                    process_children(node, output)?;
                    output.push('>');
                }
                "del" => {
                    output.push_str("-{");
                    process_children(node, output)?;
                    output.push_str("}-");
                }
                "add" => {
                    output.push_str("+{");
                    process_children(node, output)?;
                    output.push_str("}+");
                }
                "choice" => {
                    // Specific handling for .abbr[a]{b} pattern: <choice><abbr>a</abbr><expan>b</expan></choice>
                    let mut abbr_text = String::new();
                    let mut expan_text = String::new();
                    let mut found_abbr = false;
                    let mut found_expan = false;

                    let mut child = node.get_first_child();
                    while let Some(c) = child {
                        if c.get_type() == Some(NodeType::ElementNode) {
                            let c_name = c.get_name();
                            if c_name == "abbr" {
                                found_abbr = true;
                                extract_text(&c, &mut abbr_text)?;
                            } else if c_name == "expan" {
                                found_expan = true;
                                extract_text(&c, &mut expan_text)?;
                            }
                        }
                        child = c.get_next_sibling();
                    }

                    if found_abbr && found_expan {
                        output.push_str(".abbr[");
                        output.push_str(&abbr_text);
                        output.push_str("]{");
                        output.push_str(&expan_text);
                        output.push('}');
                    } else {
                        // Fallback: just process children if it doesn't match our specific pattern
                        process_children(node, output)?;
                    }
                }
                "TEI" | "teiHeader" | "text" | "body" | "div" | "p" => {
                    // Structural elements we just traverse through without adding syntax
                    process_children(node, output)?;
                }
                _ => {
                    // Unknown element: Just traverse children
                    process_children(node, output)?;
                }
            }
        }
        Some(NodeType::TextNode) => {
            let content = node.get_content();
            // Normalize whitespace:
            // 1. If it contains newlines, it's likely structural XML formatting.
            // 2. We want to preserve single spaces between words but kill indentation.

            // Simple heuristic:
            // If the content is ONLY whitespace and contains a newline, it's probably indentation -> ignore.
            // If it has text, normalize spaces (replace \n\t with space).

            if content.trim().is_empty() {
                if !content.contains('\n') {
                    // Just spaces, might be significant?
                    // In XML, " " is usually significant unless schema says otherwise.
                    // But often between tags like </lb> <w> it's just formatting.
                    // Let's assume single space is significant, multiple spaces/newlines are not.
                    output.push(' ');
                }
                // If it contains newline and is empty, ignore it (indentation)
            } else {
                // Has content. Collapse internal whitespace to single spaces.
                let normalized: String = content.split_whitespace().collect::<Vec<&str>>().join(" ");
                // Check if we need leading/trailing space based on original content
                // (e.g. "word " + "next" should correspond)
                // This is hard to get perfect without complex logic.
                // For now, simply appending the normalized content is safer than raw.
                // BUT, split_whitespace removes leading/trailing.

                // Let's try a regex approach or manual char scan?
                // Actually, `split_whitespace().join(" ")` is standard "normalize-space" behavior in XSLT.
                // However, we need to know if we should prepend a space (if the original text started with one).

                let starts_ws = content
                    .chars()
                    .next()
                    .map(|c| c.is_whitespace())
                    .unwrap_or(false);
                let ends_ws = content
                    .chars()
                    .last()
                    .map(|c| c.is_whitespace())
                    .unwrap_or(false);

                if starts_ws {
                    output.push(' ');
                }
                output.push_str(&normalized);
                if ends_ws {
                    output.push(' ');
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn process_children(node: &Node, output: &mut String) -> Result<(), String> {
    let mut child = node.get_first_child();
    while let Some(c) = child {
        process_node(&c, output)?;
        child = c.get_next_sibling();
    }
    Ok(())
}

fn extract_text(node: &Node, output: &mut String) -> Result<(), String> {
    // Simpler traversal just for text content inside abbr/expan
    // We also normalize this text because abbreviation expansions shouldn't have newlines usually
    match node.get_type() {
        Some(NodeType::TextNode) => {
            let content = node.get_content();
            let normalized: String = content.split_whitespace().collect::<Vec<&str>>().join(" ");
            output.push_str(&normalized);
        }
        Some(NodeType::ElementNode) => {
            let mut child = node.get_first_child();
            while let Some(c) = child {
                extract_text(&c, output)?;
                child = c.get_next_sibling();
            }
        }
        _ => {}
    }
    Ok(())
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
}
