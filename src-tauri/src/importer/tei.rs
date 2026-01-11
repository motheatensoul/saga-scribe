use libxml::parser::Parser;
use libxml::tree::{Node, NodeType};

/// Parses TEI-XML content and converts it to Saga-Scribe DSL
pub fn parse(xml_content: &str) -> Result<String, String> {
    let parser = Parser::default();
    
    // Attempt to parse
    let doc = parser.parse_string(xml_content)
        .map_err(|e| format!("Failed to parse XML: {}", e))?;

    let root = doc.get_root_element()
        .ok_or("No root element found")?;

    // Find the <body> element specifically to avoid importing header metadata
    let body = find_body(&root).ok_or("No <body> element found in XML")?;

    let mut output = String::new();
    // Only process children of body
    process_children(&body, &mut output)?;

    // Trim output to avoid massive trailing/leading whitespace from XML structure
    let trimmed = output.trim();
    
    Ok(trimmed.to_string())
}

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
                },
                "pb" => {
                    output.push('\n'); // Start with newline so /// is at start of line
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
                     process_children(node, output)?;
                },
                _ => {
                    // Unknown element: Just traverse children
                    process_children(node, output)?;
                }
            }
        },
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
                
                let starts_ws = content.chars().next().map(|c| c.is_whitespace()).unwrap_or(false);
                let ends_ws = content.chars().last().map(|c| c.is_whitespace()).unwrap_or(false);
                
                if starts_ws { output.push(' '); }
                output.push_str(&normalized);
                if ends_ws { output.push(' '); }
            }
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
    // Simpler traversal just for text content inside abbr/expan
    // We also normalize this text because abbreviation expansions shouldn't have newlines usually
    match node.get_type() {
        Some(NodeType::TextNode) => {
            let content = node.get_content();
            let normalized: String = content.split_whitespace().collect::<Vec<&str>>().join(" ");
            output.push_str(&normalized);
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
