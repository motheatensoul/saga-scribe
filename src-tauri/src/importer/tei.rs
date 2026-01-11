use libxml::parser::Parser;
use libxml::tree::{Node, NodeType};

/// Parses TEI-XML content and converts it to Saga-Scribe DSL
pub fn parse(xml_content: &str) -> Result<String, String> {
    // libxml is picky about parsing fragments, so we wrap in a dummy root if needed
    // But usually we expect a full document or at least a root element.
    let parser = Parser::default();
    
    // Attempt to parse
    let doc = parser.parse_string(xml_content)
        .map_err(|e| format!("Failed to parse XML: {}", e))?;

    let root = doc.get_root_element()
        .ok_or("No root element found")?;

    let mut output = String::new();
    process_node(&root, &mut output)?;

    Ok(output)
}

fn process_node(node: &Node, output: &mut String) -> Result<(), String> {
    match node.get_type() {
        Some(NodeType::ElementNode) => {
            let name = node.get_name();
            match name.as_str() {
                "lb" => {
                    output.push_str("//");
                    if let Some(n) = node.get_property("n") {
                        output.push_str(&n);
                    }
                },
                "pb" => {
                    output.push_str("///");
                    if let Some(n) = node.get_property("n") {
                        output.push_str(&n);
                    }
                },
                "gap" => {
                    output.push_str("[...]");
                },
                "supplied" => {
                    output.push('<');
                    process_children(node, output)?;
                    output.push('>');
                },
                "del" => {
                    output.push_str("-{");
                    process_children(node, output)?;
                    output.push_str("}-");
                },
                "add" => {
                    output.push_str("+{");
                    process_children(node, output)?;
                    output.push_str("}+");
                },
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
                },
                "TEI" | "teiHeader" | "text" | "body" | "div" | "p" => {
                     // Structural elements we just traverse through without adding syntax
                     // (Assuming the user wants the content of the body/div)
                     process_children(node, output)?;
                },
                _ => {
                    // Unknown element: Just traverse children? 
                    // Or should we warn? For now, traverse children to extract text.
                    process_children(node, output)?;
                }
            }
        },
        Some(NodeType::TextNode) => {
            output.push_str(&node.get_content());
        },
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
    // Simpler traversal just for text content
    match node.get_type() {
        Some(NodeType::TextNode) => {
            output.push_str(&node.get_content());
        },
        Some(NodeType::ElementNode) => {
            let mut child = node.get_first_child();
            while let Some(c) = child {
                extract_text(&c, output)?;
                child = c.get_next_sibling();
            }
        },
        _ => {}
    }
    Ok(())
}
