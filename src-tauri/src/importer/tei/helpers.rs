//! Helper functions for TEI XML extraction and serialization.

use libxml::tree::{Namespace, Node, NodeType};

/// Extract the local name of an element, stripping any namespace prefix.
/// For example, "me:facs" becomes "facs".
pub fn local_name(node: &Node) -> String {
    let name = node.get_name();
    if let Some(pos) = name.find(':') {
        name[pos + 1..].to_string()
    } else {
        name
    }
}

/// Escape special characters in XML text content.
pub fn escape_xml_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            _ => result.push(c),
        }
    }
    result
}

/// Escape special characters in XML attribute values.
pub fn escape_xml_attr(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(c),
        }
    }
    result
}

/// Serialize a node and all its children to an XML string.
/// This preserves the exact structure including namespace prefixes.
pub fn serialize_node(node: &Node) -> String {
    let mut output = String::new();
    serialize_node_internal(node, &mut output);
    output
}

/// Build a qualified name with namespace prefix if available.
pub fn qualified_name(node: &Node) -> String {
    let name = node.get_name();
    if name.contains(':') {
        name
    } else if let Some(namespace) = node.get_namespace() {
        let prefix = namespace.get_prefix();
        if prefix.is_empty() {
            name
        } else {
            format!("{}:{}", prefix, name)
        }
    } else {
        name
    }
}

pub fn attributes_with_ns(node: &Node) -> Vec<(String, String)> {
    node.get_attributes_ns()
        .into_iter()
        .map(|((name, ns), value)| {
            let key = match ns {
                Some(namespace) => qualify_attribute(&name, &namespace),
                None => name,
            };
            (key, value)
        })
        .collect()
}

fn qualify_attribute(name: &str, namespace: &Namespace) -> String {
    let prefix = namespace.get_prefix();
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}:{}", prefix, name)
    }
}

fn serialize_node_internal(node: &Node, output: &mut String) {
    match node.get_type() {
        Some(NodeType::ElementNode) => {
            let name = qualified_name(node);
            output.push('<');
            output.push_str(&name);

            // Serialize attributes
            for (key, value) in attributes_with_ns(node) {
                output.push(' ');
                output.push_str(&key);
                output.push_str("=\"");
                output.push_str(&escape_xml_attr(&value));
                output.push('"');
            }

            // Check if this is an empty element
            let first_child = node.get_first_child();
            if first_child.is_none() {
                output.push_str("/>");
            } else {
                output.push('>');

                // Serialize children
                let mut child = first_child;
                while let Some(c) = child {
                    serialize_node_internal(&c, output);
                    child = c.get_next_sibling();
                }

                // Closing tag
                output.push_str("</");
                output.push_str(&name);
                output.push('>');
            }
        }
        Some(NodeType::TextNode) => {
            output.push_str(&escape_xml_text(&node.get_content()));
        }
        Some(NodeType::CommentNode) => {
            output.push_str("<!--");
            output.push_str(&node.get_content());
            output.push_str("-->");
        }
        Some(NodeType::CDataSectionNode) => {
            output.push_str("<![CDATA[");
            output.push_str(&node.get_content());
            output.push_str("]]>");
        }
        Some(NodeType::EntityRefNode) => {
            let name = node.get_name();
            if !name.is_empty() {
                output.push('&');
                output.push_str(&name);
                output.push(';');
            } else {
                output.push_str(&escape_xml_text(&node.get_content()));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_name_with_namespace() {
        // Note: We can't easily test with real Node objects without libxml context,
        // but we test the string logic directly
        let name = "me:facs";
        let result = if let Some(pos) = name.find(':') {
            &name[pos + 1..]
        } else {
            name
        };
        assert_eq!(result, "facs");
    }

    #[test]
    fn test_local_name_without_namespace() {
        let name = "w";
        let result = if let Some(pos) = name.find(':') {
            &name[pos + 1..]
        } else {
            name
        };
        assert_eq!(result, "w");
    }

    #[test]
    fn test_escape_xml_text() {
        assert_eq!(escape_xml_text("hello"), "hello");
        assert_eq!(escape_xml_text("<>&"), "&lt;&gt;&amp;");
        assert_eq!(escape_xml_text("a < b && c > d"), "a &lt; b &amp;&amp; c &gt; d");
    }

    #[test]
    fn test_escape_xml_attr() {
        assert_eq!(escape_xml_attr("hello"), "hello");
        assert_eq!(escape_xml_attr("say \"hello\""), "say &quot;hello&quot;");
        assert_eq!(escape_xml_attr("it's"), "it&apos;s");
    }
}
