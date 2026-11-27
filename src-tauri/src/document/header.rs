//! The metadata editor as a view of the file's `<teiHeader>`: a change made
//! there is written into the header as it is, and what the editor doesn't
//! show (other elements, attributes, comments, layout) stays as written.
//!
//! The header the metadata would generate (`Metadata::to_tei_header`) is the
//! editor's picture of the header. For each part of the metadata that
//! changed (the title statement, the manuscript identifier, …), that part of
//! the file's header is brought in line with the picture, element by element:
//! elements whose text is already right are left alone, elements the picture
//! has and the file doesn't are added, and elements the old picture had and
//! the new one doesn't are removed. Parts that didn't change aren't touched.

use super::tree::{NodeId, NodeKind, Tree};
use crate::metadata::Metadata;

/// Where each part of the metadata is in a header, from `<teiHeader>`.
const PARTS: &[&[&str]] = &[
    &["fileDesc", "titleStmt"],
    &["fileDesc", "publicationStmt"],
    &["fileDesc", "sourceDesc", "msDesc", "msIdentifier"],
    &["fileDesc", "sourceDesc", "msDesc", "msContents"],
    &["fileDesc", "sourceDesc", "msDesc", "physDesc"],
    &["fileDesc", "sourceDesc", "msDesc", "history"],
    &["encodingDesc"],
    &["profileDesc", "langUsage"],
];

/// Whether part `k` of [`PARTS`] differs between `a` and `b`.
fn part_changed(k: usize, a: &Metadata, b: &Metadata) -> bool {
    match k {
        0 => a.title_stmt != b.title_stmt,
        1 => a.publication_stmt != b.publication_stmt,
        2 => a.ms_identifier != b.ms_identifier,
        3 => a.ms_contents != b.ms_contents,
        4 => a.phys_desc != b.phys_desc,
        5 => a.history != b.history,
        6 => a.encoding_notes != b.encoding_notes,
        _ => a.languages != b.languages,
    }
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

const WRAPPER_START: &str = r#"<saga-scribe-header xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0">"#;
const WRAPPER_END: &str = "</saga-scribe-header>";

/// The `<teiHeader>` a metadata set generates, as a tree, and its node.
fn picture(metadata: &Metadata) -> Result<(Tree, NodeId), String> {
    let generated = metadata.to_tei_header(false);
    let start = generated.find("<teiHeader").ok_or("no header generated")?;
    let end = generated.find("</teiHeader>").ok_or("no header generated")? + "</teiHeader>".len();
    let tree = Tree::parse(&format!("{WRAPPER_START}{}{WRAPPER_END}", &generated[start..end]))?;
    let header = element_children(&tree, tree.root_element().ok_or("no root")?).into_iter().next().ok_or("no header")?;
    Ok((tree, header))
}

fn element_children(tree: &Tree, id: NodeId) -> Vec<NodeId> {
    tree.children(id).iter().copied().filter(|&c| tree.element(c).is_some()).collect()
}

fn child_named(tree: &Tree, id: NodeId, name: &str) -> Option<NodeId> {
    element_children(tree, id).into_iter().find(|&c| tree.element(c).is_some_and(|e| local(&e.name) == name))
}

fn text_content(tree: &Tree, id: NodeId) -> String {
    let mut out = String::new();
    for node in tree.descendants(id) {
        if let NodeKind::Text(t) | NodeKind::Cdata(t) = tree.kind(node) {
            out.push_str(t);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The preamble (everything before the body's content) with its header
/// brought in line with `new`, where it differs from `old` (what the
/// metadata editor read from it). A preamble without a header gets one.
pub fn with_metadata(preamble: &str, old: &Metadata, new: &Metadata) -> Result<String, String> {
    if old == new {
        return Ok(preamble.to_string());
    }
    let (new_tree, new_header) = picture(new)?;
    let (old_tree, old_header) = picture(old)?;
    let Some(start) = preamble.find("<teiHeader") else {
        // No header: the generated one, right after the TEI start tag
        let tei = preamble.find("<TEI").ok_or("The file has no <TEI> element for a header")?;
        let after = tei + preamble[tei..].find('>').ok_or("The <TEI> start tag isn't closed")? + 1;
        let header = new_tree.node_xml(new_header)?;
        return Ok(format!("{}\n  {header}{}", &preamble[..after], &preamble[after..]));
    };
    let end = preamble[start..].find("</teiHeader>").ok_or("The <teiHeader> isn't closed")? + start + "</teiHeader>".len();
    let mut file = Tree::parse(&format!("{WRAPPER_START}{}{WRAPPER_END}", &preamble[start..end]))?;
    let header = element_children(&file, file.root_element().ok_or("no root")?).into_iter().next().ok_or("no header")?;

    for (k, path) in PARTS.iter().enumerate() {
        if !part_changed(k, old, new) {
            continue;
        }
        let find = |tree: &Tree, from: NodeId| path.iter().try_fold(from, |at, name| child_named(tree, at, name));
        let wanted = find(&new_tree, new_header);
        let was = find(&old_tree, old_header);
        // Down the path in the file, adding what is missing
        let mut at = header;
        let mut picture_at = Some(new_header);
        let mut ok = true;
        for name in path.iter() {
            picture_at = picture_at.and_then(|p| child_named(&new_tree, p, name));
            match child_named(&file, at, name) {
                Some(child) => at = child,
                None => {
                    // Not in the file: the picture's, if it has it
                    if let Some(p) = picture_at {
                        let index = insert_index(&file, at, &new_tree, p);
                        file.graft(&new_tree, p, at, index);
                    }
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        match wanted {
            Some(wanted) => merge(&mut file, at, &new_tree, wanted, &old_tree, was),
            // Emptied: the elements the picture had go
            None => {
                if let Some(was) = was {
                    merge_children(&mut file, at, &new_tree, None, &old_tree, Some(was));
                }
            }
        }
    }
    let written = file.node_xml(header)?;
    Ok(format!("{}{written}{}", &preamble[..start], &preamble[end..]))
}

/// Where among `parent`'s children an element like `picture` (in `from`)
/// goes: before the first one the picture has after it, else at the end.
fn insert_index(file: &Tree, parent: NodeId, from: &Tree, picture: NodeId) -> usize {
    let Some(picture_parent) = from.parent(picture) else { return file.children(parent).len() };
    let order: Vec<String> = element_children(from, picture_parent)
        .into_iter()
        .filter_map(|c| from.element(c).map(|e| local(&e.name).to_string()))
        .collect();
    let name = from.element(picture).map(|e| local(&e.name).to_string()).unwrap_or_default();
    let later: Vec<&String> = order.iter().skip_while(|n| **n != name).skip(1).collect();
    file.children(parent)
        .iter()
        .position(|&c| file.element(c).is_some_and(|e| later.iter().any(|l| **l == local(&e.name))))
        .unwrap_or(file.children(parent).len())
}

/// Brings element `at` of the file in line with `wanted` (the new picture),
/// `was` being the old picture's counterpart.
fn merge(file: &mut Tree, at: NodeId, new: &Tree, wanted: NodeId, old: &Tree, was: Option<NodeId>) {
    // Attributes the picture sets
    if let Some(e) = new.element(wanted).cloned() {
        for attribute in &e.attributes {
            let _ = file.set_attribute(at, &attribute.name, Some(&attribute.value));
        }
        // Those the old picture set and the new one doesn't
        if let Some(old_e) = was.and_then(|w| old.element(w)) {
            for attribute in &old_e.attributes {
                if e.attribute(&attribute.name).is_none() {
                    let _ = file.set_attribute(at, &attribute.name, None);
                }
            }
        }
    }
    if element_children(new, wanted).is_empty() {
        // Text: replaced if it reads differently
        if text_content(file, at) != text_content(new, wanted) {
            for child in file.children(at).to_vec() {
                file.remove(child);
            }
            for (i, &child) in new.children(wanted).iter().enumerate() {
                file.graft(new, child, at, i);
            }
        }
        return;
    }
    merge_children(file, at, new, Some(wanted), old, was);
}

/// The child elements of `at`, name by name, in line with `wanted`'s.
fn merge_children(file: &mut Tree, at: NodeId, new: &Tree, wanted: Option<NodeId>, old: &Tree, was: Option<NodeId>) {
    let named = |tree: &Tree, id: Option<NodeId>| -> Vec<(String, NodeId)> {
        id.map(|id| element_children(tree, id))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|c| Some((local(&tree.element(c)?.name).to_string(), c)))
            .collect()
    };
    let wanted_children = named(new, wanted);
    let was_children = named(old, was);
    let mut names: Vec<String> = Vec::new();
    for (name, _) in wanted_children.iter().chain(&was_children) {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    for name in names {
        let ours: Vec<NodeId> = named(file, Some(at)).into_iter().filter(|(n, _)| *n == name).map(|(_, c)| c).collect();
        let theirs: Vec<NodeId> = wanted_children.iter().filter(|(n, _)| *n == name).map(|(_, c)| *c).collect();
        let before: Vec<NodeId> = was_children.iter().filter(|(n, _)| *n == name).map(|(_, c)| *c).collect();
        if ours.len() == theirs.len() {
            for (k, (&o, &t)) in ours.iter().zip(&theirs).enumerate() {
                merge(file, o, new, t, old, before.get(k).copied());
            }
            continue;
        }
        // A different number: the picture's in place of the file's
        let index = match ours.first() {
            Some(&first) => file.index_in_parent(first).unwrap_or(file.children(at).len()),
            None => theirs.first().map_or(file.children(at).len(), |&t| insert_index(file, at, new, t)),
        };
        for &o in &ours {
            file.remove(o);
        }
        for (i, &t) in theirs.iter().enumerate() {
            file.graft(new, t, at, index + i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{MsIdentifier, TitleStmt};

    const PREAMBLE: &str = r#"<?xml version="1.0"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt>
        <title>Old title</title>
        <sponsor>Someone</sponsor>
      </titleStmt>
      <publicationStmt><p>Unpublished</p></publicationStmt>
      <sourceDesc>
        <msDesc>
          <msIdentifier><repository>AM</repository><idno>AM 1</idno></msIdentifier>
          <!-- kept -->
        </msDesc>
      </sourceDesc>
    </fileDesc>
  </teiHeader>
  <text><body>"#;

    fn metadata(title: &str, idno: &str) -> Metadata {
        Metadata {
            title_stmt: TitleStmt { title: Some(title.into()), ..Default::default() },
            ms_identifier: MsIdentifier { repository: Some("AM".into()), idno: Some(idno.into()), ..Default::default() },
            ..Default::default()
        }
    }

    #[test]
    fn a_changed_field_is_written_into_the_header_and_the_rest_stays() {
        let old = metadata("Old title", "AM 1");
        let new = metadata("New title", "AM 1");
        let out = with_metadata(PREAMBLE, &old, &new).unwrap();
        assert_eq!(out, PREAMBLE.replace("Old title", "New title"));
        // Unchanged: as it was
        assert_eq!(with_metadata(PREAMBLE, &old, &old).unwrap(), PREAMBLE);
        // A field added and one removed in another part
        let mut more = metadata("Old title", "AM 2");
        more.ms_identifier.settlement = Some("Reykjavík".into());
        more.ms_identifier.repository = None;
        let out = with_metadata(PREAMBLE, &old, &more).unwrap();
        assert!(out.contains("<settlement>Reykjavík</settlement>"), "{out}");
        assert!(out.contains("<idno>AM 2</idno>") && !out.contains("<repository>"), "{out}");
        assert!(out.contains("<sponsor>Someone</sponsor>") && out.contains("<!-- kept -->"), "{out}");
        assert!(out.contains("<publicationStmt><p>Unpublished</p></publicationStmt>"), "{out}");
    }

    #[test]
    fn a_file_without_a_header_gets_one() {
        let preamble = "<TEI xmlns=\"http://www.tei-c.org/ns/1.0\"><text><body>";
        let out = with_metadata(preamble, &Metadata::default(), &metadata("T", "1")).unwrap();
        assert!(out.contains("<teiHeader>") && out.contains("<title>T</title>") && out.ends_with("<text><body>"), "{out}");
    }
}
