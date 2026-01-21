use crate::importer::tei::extraction::segments_to_dsl;
use crate::importer::tei::helpers;
use crate::importer::tei::segments::Segment;
use crate::parser::{Compiler, Lexer, Node, WordTokenizer};
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PatchOperation {
    #[serde(rename = "keep")]
    Keep { segment_id: usize },
    #[serde(rename = "modify")]
    Modify { segment_id: usize, new_dsl: String },
    #[serde(rename = "insert")]
    Insert { dsl: String },
    #[serde(rename = "delete")]
    Delete { segment_id: usize },
}

pub fn compute_patches(segments: &[Segment], edited_dsl: &str) -> Vec<PatchOperation> {
    // Fast-path: if the edited DSL matches the original extraction, keep everything.
    let original_dsl = segments_to_dsl(segments);
    if original_dsl.trim() == edited_dsl.trim() {
        return Vec::new();
    }

    let original_tokens = extract_tokens_from_segments(segments);
    let edited_tokens = parse_dsl_to_tokens(edited_dsl);

    diff_tokens(&original_tokens, &edited_tokens)
}

#[derive(Debug, Clone)]
struct TokenInfo {
    content: String,
    segment_id: Option<usize>,
}

fn extract_tokens_from_segments(segments: &[Segment]) -> Vec<TokenInfo> {
    let mut tokens = Vec::new();
    for seg in segments {
        match seg {
            Segment::Word {
                id, dsl_content, ..
            } => {
                if !dsl_content.trim().is_empty() {
                    tokens.push(TokenInfo {
                        content: dsl_content.clone(),
                        segment_id: Some(*id),
                    });
                }
            }
            Segment::Punctuation {
                id, dsl_content, ..
            } => {
                if !dsl_content.trim().is_empty() {
                    tokens.push(TokenInfo {
                        content: dsl_content.clone(),
                        segment_id: Some(*id),
                    });
                }
            }
            Segment::LineBreak { id, attributes } => {
                let n = attributes.get("n");
                let content = if let Some(num) = n {
                    format!("//{}", num)
                } else {
                    "//".to_string()
                };
                tokens.push(TokenInfo {
                    content,
                    segment_id: Some(*id),
                });
            }
            Segment::PageBreak { id, attributes } => {
                let n = attributes.get("n");
                let content = if let Some(num) = n {
                    format!("///{}", num)
                } else {
                    "///".to_string()
                };
                tokens.push(TokenInfo {
                    content,
                    segment_id: Some(*id),
                });
            }
            _ => {}
        }
    }
    tokens
}

fn parse_dsl_to_tokens(dsl: &str) -> Vec<TokenInfo> {
    let mut lexer = Lexer::new(dsl);
    let doc = lexer.parse().unwrap_or_default();
    let tokenizer = WordTokenizer::new();
    let nodes = tokenizer.tokenize(doc.nodes);

    nodes
        .into_iter()
        .filter_map(|node| match &node {
            Node::Word(_)
            | Node::Punctuation(_)
            | Node::LineBreak(_)
            | Node::PageBreak(_)
            | Node::Head(_)
            | Node::SuppliedBlock(_)
            | Node::Norm(_) => Some(TokenInfo {
                content: node_to_dsl(&node),
                segment_id: None,
            }),
            _ => None,
        })
        .collect()
}

fn node_to_dsl(node: &Node) -> String {
    match node {
        Node::Text(t) => t.clone(),
        Node::LineBreak(n) => {
            if let Some(num) = n {
                format!("//{}", num)
            } else {
                "//".to_string()
            }
        }
        Node::PageBreak(n) => format!("///{}", n),
        Node::Abbreviation { abbr, expansion } => format!(".abbr[{}]{{{}}}", abbr, expansion),
        Node::Gap { quantity, supplied } => {
            let mut s = "[...".to_string();
            if let Some(q) = quantity {
                s.push_str(&q.to_string());
            }
            if let Some(ref supp) = supplied {
                s.push('<');
                s.push_str(supp);
                s.push('>');
            }
            s.push(']');
            s
        }
        Node::Supplied(t) => format!("<{}>", t),
        Node::SuppliedBlock(t) => format!(".supplied{{{}}}", t),
        Node::Norm(t) => format!(".norm{{{}}}", t),
        Node::Deletion(t) => format!("-{{{}}}-", t),
        Node::Addition(t) => format!("+{{{}}}+", t),
        Node::Note(t) => format!("^{{{}}}", t),
        Node::Head(t) => format!(".head{{{}}}", t),
        Node::Unclear(t) => format!("?{{{}}}?", t),
        Node::Entity(name) => format!(":{}:", name),
        Node::WordContinuation => "~".to_string(),
        Node::CompoundJoin => "~".to_string(),
        Node::WordBoundary => "|".to_string(),
        Node::Word(nodes) => {
            let mut s = String::new();
            for n in nodes {
                s.push_str(&node_to_dsl(n));
            }
            s
        }
        Node::Punctuation(nodes) => {
            let mut s = String::new();
            for n in nodes {
                s.push_str(&node_to_dsl(n));
            }
            s
        }
    }
}

fn diff_tokens(original: &[TokenInfo], edited: &[TokenInfo]) -> Vec<PatchOperation> {
    let mut start = 0;
    let m = original.len();
    let n = edited.len();

    // 1. Skip common prefix
    while start < m && start < n && original[start].content == edited[start].content {
        start += 1;
    }

    // 2. Skip common suffix
    let mut original_end = m;
    let mut edited_end = n;
    while original_end > start
        && edited_end > start
        && original[original_end - 1].content == edited[edited_end - 1].content
    {
        original_end -= 1;
        edited_end -= 1;
    }

    let mut patches = Vec::new();

    // Add Keep for common prefix
    for token in original.iter().take(start) {
        patches.push(PatchOperation::Keep {
            segment_id: token.segment_id.unwrap(),
        });
    }

    // Handle middle part with LCS
    if start < original_end || start < edited_end {
        let middle_patches =
            diff_tokens_lcs(&original[start..original_end], &edited[start..edited_end]);
        patches.extend(middle_patches);
    }

    // Add Keep for common suffix
    for token in original.iter().skip(original_end) {
        patches.push(PatchOperation::Keep {
            segment_id: token.segment_id.unwrap(),
        });
    }

    combine_to_modify(patches)
}

fn diff_tokens_lcs(original: &[TokenInfo], edited: &[TokenInfo]) -> Vec<PatchOperation> {
    let m = original.len();
    let n = edited.len();

    // Safety limit to avoid O(MN) explosion on very large unsynced docs
    // If it's too big, fallback to a linear diff that preserves structure
    if m > 1000 && n > 1000 {
        let mut fallback = Vec::new();
        let min_len = m.min(n);

        for (original_token, edited_token) in original
            .iter()
            .take(min_len)
            .zip(edited.iter().take(min_len))
        {
            let segment_id = original_token.segment_id.unwrap();

            if original_token.content == edited_token.content {
                fallback.push(PatchOperation::Keep { segment_id });
            } else {
                fallback.push(PatchOperation::Modify {
                    segment_id,
                    new_dsl: edited_token.content.clone(),
                });
            }
        }

        if m > n {
            for t in &original[min_len..] {
                fallback.push(PatchOperation::Delete {
                    segment_id: t.segment_id.unwrap(),
                });
            }
        } else if n > m {
            for t in &edited[min_len..] {
                fallback.push(PatchOperation::Insert {
                    dsl: t.content.clone(),
                });
            }
        }

        return fallback;
    }

    let mut dp = vec![vec![0; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if original[i - 1].content == edited[j - 1].content {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    let mut patches = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && original[i - 1].content == edited[j - 1].content {
            patches.push(PatchOperation::Keep {
                segment_id: original[i - 1].segment_id.unwrap(),
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            patches.push(PatchOperation::Insert {
                dsl: edited[j - 1].content.clone(),
            });
            j -= 1;
        } else {
            patches.push(PatchOperation::Delete {
                segment_id: original[i - 1].segment_id.unwrap(),
            });
            i -= 1;
        }
    }

    patches.reverse();
    patches
}

fn combine_to_modify(ops: Vec<PatchOperation>) -> Vec<PatchOperation> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        if i + 1 < ops.len() {
            if let (PatchOperation::Delete { segment_id }, PatchOperation::Insert { dsl }) =
                (&ops[i], &ops[i + 1])
            {
                result.push(PatchOperation::Modify {
                    segment_id: *segment_id,
                    new_dsl: dsl.clone(),
                });
                i += 2;
                continue;
            }
        }
        result.push(ops[i].clone());
        i += 1;
    }
    result
}

pub fn apply_patches_and_reconstruct(
    segments: &[Segment],
    patches: &[PatchOperation],
    compiler: &mut Compiler,
) -> String {
    reconstruct_from_segments_and_patches(segments, patches, compiler)
}

fn reconstruct_from_segments_and_patches(
    segments: &[Segment],
    patches: &[PatchOperation],
    compiler: &mut Compiler,
) -> String {
    let mut xml = String::new();
    let mut current_patch_idx = 0;

    // 1. Process segments in order
    for seg in segments {
        if matches!(
            seg,
            Segment::Word { .. }
                | Segment::Punctuation { .. }
                | Segment::LineBreak { .. }
                | Segment::PageBreak { .. }
        ) {
            let seg_id = seg.id();
            let mut handled = false;

            // Interleave leading insertions
            while current_patch_idx < patches.len() {
                match &patches[current_patch_idx] {
                    PatchOperation::Insert { dsl } => {
                        xml.push_str(&compiler.compile_fragment_from_dsl(dsl));
                        current_patch_idx += 1;
                    }
                    PatchOperation::Keep { segment_id }
                    | PatchOperation::Modify { segment_id, .. }
                    | PatchOperation::Delete { segment_id } => {
                        if *segment_id == seg_id {
                            // Apply patch for this segment
                            match &patches[current_patch_idx] {
                                PatchOperation::Keep { .. } => {
                                    xml.push_str(&serialize_original_segment(seg));
                                }
                                PatchOperation::Modify { new_dsl, .. } => match seg {
                                    Segment::Word { attributes, .. } => {
                                        if new_dsl.trim_start().starts_with(".head{") {
                                            xml.push_str(
                                                &compiler.compile_fragment_from_dsl(new_dsl),
                                            );
                                        } else {
                                            xml.push_str(
                                                &compiler
                                                    .compile_word_from_dsl(new_dsl, attributes),
                                            );
                                        }
                                    }
                                    Segment::Punctuation { .. } => {
                                        xml.push_str(
                                            &compiler.compile_punctuation_from_dsl(new_dsl),
                                        );
                                    }
                                    _ => {
                                        xml.push_str(&serialize_original_segment(seg));
                                    }
                                },
                                PatchOperation::Delete { .. } => {}
                                _ => unreachable!(),
                            }
                            current_patch_idx += 1;
                            handled = true;
                            break; // Finished this segment
                        } else {
                            // This patch belongs to a later segment.
                            break;
                        }
                    }
                }
            }

            if !handled {
                xml.push_str(&serialize_original_segment(seg));
            }
        } else {
            // Structural or whitespace segment
            xml.push_str(&serialize_original_segment(seg));
        }
    }

    // 2. Consume any trailing insertions
    while current_patch_idx < patches.len() {
        if let PatchOperation::Insert { dsl } = &patches[current_patch_idx] {
            xml.push_str(&compiler.compile_fragment_from_dsl(dsl));
        }
        current_patch_idx += 1;
    }

    xml
}

fn serialize_original_segment(seg: &Segment) -> String {
    match seg {
        Segment::Structural { xml, .. } => xml.clone(),
        Segment::Word { original_xml, .. } => original_xml.clone(),
        Segment::Punctuation { original_xml, .. } => original_xml.clone(),
        Segment::LineBreak { attributes, .. } => format_lb(attributes),
        Segment::PageBreak { attributes, .. } => format_pb(attributes),
        Segment::HandShift { attributes, .. } => format_tag("handShift", attributes),
        Segment::Whitespace { content, .. } => content.clone(),
    }
}

fn format_lb(attrs: &HashMap<String, String>) -> String {
    format_tag("lb", attrs)
}

fn format_pb(attrs: &HashMap<String, String>) -> String {
    format_tag("pb", attrs)
}

fn format_tag(name: &str, attrs: &HashMap<String, String>) -> String {
    let mut s = format!("<{}", name);
    let mut sorted_keys: Vec<_> = attrs.keys().collect();
    sorted_keys.sort();
    for k in sorted_keys {
        s.push_str(&format!(
            " {}=\"{}\"",
            k,
            helpers::escape_xml_attr(attrs.get(k).unwrap())
        ));
    }
    s.push_str("/>");
    s
}
