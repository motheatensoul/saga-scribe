use super::tree::{Attribute, Element, NodeId, NodeKind, Tree};

/// The first element called `name`.
fn find(tree: &Tree, name: &str) -> NodeId {
    tree.descendants(Tree::DOCUMENT)
        .into_iter()
        .find(|&id| tree.element(id).is_some_and(|e| e.name == name))
        .unwrap_or_else(|| panic!("no <{name}>"))
}

/// The first text node whose text is `text`.
fn find_text(tree: &Tree, text: &str) -> NodeId {
    tree.descendants(Tree::DOCUMENT)
        .into_iter()
        .find(|&id| matches!(tree.kind(id), NodeKind::Text(t) if t == text))
        .unwrap_or_else(|| panic!("no text {text:?}"))
}

/// A document with what libxml2 doesn't keep as written: the XML declaration,
/// a DOCTYPE with an internal subset, quote styles, spacing in tags, `<x/>`
/// and `<x></x>`, character references, CRLF, entities, CDATA, comments and
/// processing instructions, namespaces.
const AWKWARD: &str = "<?xml version='1.0' encoding=\"UTF-8\"?>\r\n\
<!DOCTYPE TEI [\r\n<!ENTITY own \"x\">\r\n]>\r\n\
<!-- before -->\r\n\
<TEI xmlns=\"http://www.tei-c.org/ns/1.0\" xmlns:me='http://www.menota.org/ns/1.0'>\r\n\
<?pi some data?>\r\n\
  <w   lemma='konungr'  me:msa=\"xNC\"   xml:id=\"w1\" >k&#x00F6;n&amp;&thorn;&own;</w>\r\n\
  <lb n=\"1\" /><lb n=\"2\"></lb><![CDATA[<raw> & ]]>\r\n\
  <me:facs>a&lt;b&gt;c</me:facs>\r\n\
</TEI>\r\n<!-- after -->\r\n";

#[test]
fn test_unchanged_document_is_written_byte_for_byte() {
    let tree = Tree::parse(AWKWARD).unwrap();
    assert!(tree.has_spans());
    assert_eq!(tree.write().unwrap(), AWKWARD);
    assert_eq!(tree.write_checked().unwrap(), AWKWARD);
}

#[test]
fn test_tree_keeps_every_node_and_attribute_order() {
    let tree = Tree::parse(AWKWARD).unwrap();
    let w = tree.element(find(&tree, "w")).unwrap();
    let names: Vec<&str> = w.attributes.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["lemma", "me:msa", "xml:id"]);
    let kinds: Vec<String> = tree
        .children(find(&tree, "w"))
        .iter()
        .map(|&id| format!("{:?}", tree.kind(id)))
        .collect();
    assert_eq!(kinds, [r#"Text("kön&")"#, r#"EntityRef("thorn")"#, r#"EntityRef("own")"#]);
    let top: Vec<&str> = tree
        .children(Tree::DOCUMENT)
        .iter()
        .map(|&id| match tree.kind(id) {
            NodeKind::Doctype(_) => "doctype",
            NodeKind::Comment(_) => "comment",
            NodeKind::Element(_) => "element",
            _ => "other",
        })
        .collect();
    assert_eq!(top, ["doctype", "comment", "element", "comment"]);
    let root = tree.element(tree.root_element().unwrap()).unwrap();
    assert_eq!(root.namespaces.len(), 2);
}

#[test]
fn test_edited_text_changes_only_its_own_bytes() {
    let mut tree = Tree::parse(AWKWARD).unwrap();
    let text = find_text(&tree, "a<b>c");
    let span = tree.span(text).unwrap();
    tree.set_text(text, "a<b>c & d").unwrap();
    let out = tree.write_checked().unwrap();
    assert_eq!(&out[..span.start], &AWKWARD[..span.start]);
    assert_eq!(&out[out.len() - (AWKWARD.len() - span.end)..], &AWKWARD[span.end..]);
    assert!(out.contains("<me:facs>a&lt;b&gt;c &amp; d</me:facs>"));
}

#[test]
fn test_changed_attribute_keeps_the_rest_of_the_tag() {
    let mut tree = Tree::parse(AWKWARD).unwrap();
    let w = find(&tree, "w");
    tree.set_attribute(w, "me:msa", Some("xNC gM\tnS")).unwrap();
    tree.set_attribute(w, "lemma", None).unwrap();
    tree.set_attribute(w, "n", Some("1")).unwrap();
    let out = tree.write_checked().unwrap();
    assert!(
        // lemma goes with the whitespace before it; the rest stays as written
        out.contains("<w  me:msa=\"xNC gM&#9;nS\"   xml:id=\"w1\" n=\"1\">k&#x00F6;n"),
        "{out}"
    );
    let reread = Tree::parse(&out).unwrap();
    assert_eq!(reread.element(find(&reread, "w")).unwrap().attribute("me:msa"), Some("xNC gM\tnS"));
}

#[test]
fn test_inserted_and_removed_nodes() {
    let mut tree = Tree::parse(AWKWARD).unwrap();
    // An empty `<lb/>` gets content: it's opened and closed
    let lb = find(&tree, "lb");
    tree.insert(lb, 0, NodeKind::Text("x".into()));
    // A new element with an attribute that needs escaping
    let root = tree.root_element().unwrap();
    let ex = tree.insert(
        root,
        0,
        NodeKind::Element(Box::new(Element {
            name: "ex".into(),
            namespaces: vec![],
            attributes: vec![Attribute { name: "resp".into(), value: "\"A&B\"".into() }],
        })),
    );
    tree.insert(ex, 0, NodeKind::Text("onun".into()));
    // The CDATA section goes
    let cdata = tree
        .descendants(Tree::DOCUMENT)
        .into_iter()
        .find(|&id| matches!(tree.kind(id), NodeKind::Cdata(_)))
        .unwrap();
    tree.remove(cdata);
    let out = tree.write_checked().unwrap();
    assert!(out.contains("<lb n=\"1\" >x</lb><lb n=\"2\"></lb>\r\n"), "{out}");
    assert!(out.contains("<ex resp=\"&quot;A&amp;B&quot;\">onun</ex>"), "{out}");
    assert!(!out.contains("CDATA"));
}

#[test]
fn test_what_xml_cannot_hold_is_refused() {
    let mut tree = Tree::parse("<a><!--c--><b/></a>").unwrap();
    let comment = tree
        .descendants(Tree::DOCUMENT)
        .into_iter()
        .find(|&id| matches!(tree.kind(id), NodeKind::Comment(_)))
        .unwrap();
    tree.set_text(comment, "a -- b").unwrap();
    assert!(tree.write().is_err());

    let mut tree = Tree::parse("<a>x</a>").unwrap();
    let text = find_text(&tree, "x");
    tree.set_text(text, "bell \u{7}").unwrap();
    assert!(tree.write().is_err());

    let mut tree = Tree::parse("<a/>").unwrap();
    let a = find(&tree, "a");
    tree.set_attribute(a, "not a name", Some("x")).unwrap();
    assert!(tree.write_checked().is_err());
}

#[test]
fn test_without_spans_the_content_is_kept() {
    let mut tree = Tree::parse(AWKWARD).unwrap();
    tree.forget_spans();
    let out = tree.write_checked().unwrap();
    // The prolog is kept as written; the rest is written from the tree
    assert!(out.starts_with("<?xml version='1.0' encoding=\"UTF-8\"?>\r\n<!DOCTYPE TEI ["));
    assert!(out.contains("<!-- after -->"));
}

#[test]
fn test_entity_reference_content_is_not_copied() {
    // libxml2 hangs an entity's content below each of its references
    let tree = Tree::parse("<!DOCTYPE a [<!ENTITY e \"<b>x</b>\">]><a>&e;&e;</a>").unwrap();
    let a = find(&tree, "a");
    assert_eq!(tree.children(a).len(), 2);
    assert!(tree.children(a).iter().all(|&id| tree.children(id).is_empty()));
    assert!(tree.write_checked().is_ok());
}

// ============================================================================
// The Menota corpus (opt-in)
// ============================================================================

/// Every file of the Menota catalogue, from `scripts/fetch-menota-corpus.sh`.
/// Run with `MENOTA_CORPUS=<dir> cargo test --release corpus_ -- --test-threads=1 --nocapture`
/// (one at a time: each test loads the whole corpus).
fn corpus_files() -> Vec<std::path::PathBuf> {
    let Ok(dir) = std::env::var("MENOTA_CORPUS") else { return Vec::new() };
    let mut files = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from(dir)];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("MENOTA_CORPUS is a directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "xml") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// A deterministic choice of `n` of `items`, spread over them.
fn spread<T: Copy>(items: &[T], n: usize) -> Vec<T> {
    if items.is_empty() {
        return Vec::new();
    }
    (0..n.min(items.len())).map(|i| items[i * items.len() / n.min(items.len())]).collect()
}

/// Gate of step 1: each file is read and written back byte for byte, the
/// written file reads as the same tree, and an edit changes only the bytes of
/// what was edited.
#[test]
fn corpus_files_are_written_back_and_edits_stay_local() {
    let files = corpus_files();
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(source) = std::fs::read_to_string(path) else { continue };
        let result = (|| -> Result<(), String> {
            let tree = Tree::parse(&source)?;
            if !tree.has_spans() {
                return Err("no spans".into());
            }
            if tree.write()? != source {
                return Err("not written back byte for byte".into());
            }
            tree.write_checked()?;

            let texts: Vec<NodeId> = tree
                .descendants(Tree::DOCUMENT)
                .into_iter()
                .filter(|&id| matches!(tree.kind(id), NodeKind::Text(t) if !t.trim().is_empty()))
                .collect();
            let elements: Vec<NodeId> = tree
                .descendants(Tree::DOCUMENT)
                .into_iter()
                .filter(|&id| tree.element(id).is_some_and(|e| !e.attributes.is_empty()))
                .collect();
            for (i, id) in spread(&texts, 5).into_iter().chain(spread(&elements, 3)).enumerate() {
                let mut edited = tree.clone();
                let span = match edited.kind(id).clone() {
                    NodeKind::Text(text) => {
                        edited.set_text(id, &format!("{text}x<&"))?;
                        edited.span(id).unwrap()
                    }
                    NodeKind::Element(element) => {
                        let first = &element.attributes[0];
                        edited.set_attribute(id, &first.name, Some(&format!("{}\"{i}", first.value)))?;
                        edited.tags(id).unwrap().0
                    }
                    _ => unreachable!(),
                };
                let out = edited.write_checked()?;
                let tail = source.len() - span.end;
                if out[..span.start] != source[..span.start] || out[out.len() - tail..] != source[span.end..] {
                    return Err(format!("an edit of {:?} changed other bytes", edited.kind(id)));
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => eprintln!("ok      {name}"),
            Err(e) => {
                eprintln!("FAILED  {name}: {e}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "{} of {} files failed: {:?}", failures.len(), files.len(), failures);
}

/// Where the XML's tokens (each word, numeral, punctuation, break and
/// paragraph, read from the segments) and the shorthand's differ, if they do.
fn token_difference(source: &str) -> Result<Option<String>, String> {
    use crate::importer::tei::patching::{dsl_tokens, segment_dsl_token};
    let import = crate::importer::tei::parse(source)?;
    let doc = import.imported_document.ok_or("no manifest")?;
    let from_xml: Vec<String> = doc.segments.iter().filter_map(segment_dsl_token).collect();
    let from_dsl = dsl_tokens(&import.dsl)?;
    let Some(at) = (0..from_xml.len().max(from_dsl.len())).find(|&i| from_xml.get(i) != from_dsl.get(i)) else {
        return Ok(None);
    };
    let around = |tokens: &[String]| tokens[at.saturating_sub(2)..(at + 3).min(tokens.len())].join(" ");
    Ok(Some(format!(
        "token {at} of {}/{}: XML {:?}, shorthand {:?}",
        from_xml.len(),
        from_dsl.len(),
        around(&from_xml),
        around(&from_dsl)
    )))
}

#[test]
fn test_holmperg_xml_and_shorthand_read_as_the_same_tokens() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/HolmPerg-34-4to-MLL.xml");
    let Ok(source) = std::fs::read_to_string(path) else { return };
    assert_eq!(token_difference(&source), Ok(None));
}

/// The importer's promise, on every file: the shorthand reads as the XML's
/// words, numerals, punctuation and breaks, token for token (the patcher
/// pairs them with the file's elements by that).
#[test]
fn corpus_xml_and_shorthand_read_as_the_same_tokens() {
    let files = corpus_files();
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(source) = std::fs::read_to_string(path) else { continue };
        match token_difference(&source) {
            Ok(None) => eprintln!("ok      {name}"),
            Ok(Some(difference)) => {
                eprintln!("FAILED  {name}: {difference}");
                failures.push(name);
            }
            Err(e) => {
                eprintln!("FAILED  {name}: {e}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "{} of {} files failed: {:?}", failures.len(), files.len(), failures);
}

/// Every file imported and saved unedited through the patcher comes back
/// byte for byte (the importer works on the projection).
#[test]
fn corpus_unchanged_import_round_trips() {
    use crate::importer::tei::patching::{compiler_config_for, compute_patches, render_body, Originals};
    let files = corpus_files();
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(source) = std::fs::read_to_string(path) else { continue };
        let result = (|| -> Result<(), String> {
            let import = crate::importer::tei::parse(&source)?;
            let doc = import.imported_document.ok_or("no manifest")?;
            let original = import.original_body_xml.ok_or("no body")?;
            if doc.spans.is_none() {
                return Err("no spans".into());
            }
            let patches = compute_patches(&doc.segments, doc.dsl_rules, &import.dsl)?;
            let (doc, tree) = {
                let tree = gate_tree(&import.original_preamble, &original, &import.original_postamble, &doc)?;
                slim(doc, tree)
            };
            let mut compiler = crate::parser::Compiler::new().with_config(compiler_config_for(&doc, Originals::new(Some(&tree))));
            let body = render_body(&doc, Some(&original), Some(&tree), &patches, &mut compiler)?;
            if body.xml != original {
                return Err("the unedited body changed".into());
            }
            Ok(())
        })();
        match result {
            Ok(()) => eprintln!("ok      {name}"),
            Err(e) => {
                eprintln!("FAILED  {name}: {e}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "{} of {} files failed: {:?}", failures.len(), files.len(), failures);
}

/// Gate of step 3 (the merge): in every file, every 25th word gets a letter
/// added in the shorthand. The written file reads, and it has every element
/// it had: an edit doesn't lose markup the shorthand can't show.
#[test]
fn corpus_edited_words_keep_their_markup() {
    use crate::identity::{Guidance, TextChange};
    use crate::importer::tei::patching::{compiler_config_for, render_body, Originals, Patcher};
    use crate::importer::tei::segments::Segment;
    use std::collections::BTreeMap;

    fn element_counts(tree: &Tree) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for id in tree.descendants(Tree::DOCUMENT) {
            if let Some(element) = tree.element(id) {
                *counts.entry(element.name.clone()).or_default() += 1;
            }
        }
        counts
    }

    let files = corpus_files();
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(source) = std::fs::read_to_string(path) else { continue };
        let result = (|| -> Result<usize, String> {
            let import = crate::importer::tei::parse(&source)?;
            let doc = import.imported_document.ok_or("no manifest")?;
            let original = import.original_body_xml.clone().ok_or("no body")?;
            // Append "ꝸ" (a letter no Menota text has) to every 25th <w>, at
            // the end of its token (segments read one-to-one as tokens)
            let (_, spans) = crate::identity::spanned_dsl_tokens(&import.dsl)?;
            let token_segments: Vec<&Segment> = doc
                .segments
                .iter()
                .filter(|s| crate::importer::tei::patching::segment_dsl_token(s).is_some())
                .collect();
            // The patcher pairs tokens with segments (and
            // `corpus_xml_and_shorthand_read_as_the_same_tokens` checks more)
            if token_segments.len() != spans.len() {
                return Err(format!("{} segments read as {} tokens", token_segments.len(), spans.len()));
            }
            let mut ends = Vec::new();
            let mut words = 0;
            for (segment, span) in token_segments.iter().zip(&spans) {
                if let Segment::Word { original_xml, .. } = segment {
                    if original_xml.starts_with("<w") {
                        words += 1;
                        if words % 25 == 0 {
                            ends.push(span.end);
                        }
                    }
                }
            }
            let edited = ends.len();
            let mut dsl = import.dsl.clone();
            for &end in ends.iter().rev() {
                dsl.insert(end, 'ꝸ');
            }

            // As the app does: the editor's changes since the last compile
            // guide which word is which
            let mut patcher = Patcher::new(&doc.segments, doc.dsl_rules);
            let (_, state) = patcher.compute(&import.dsl, None)?;
            patcher.commit(state);
            let utf16 = |text: &str, byte: usize| text[..byte].encode_utf16().count();
            let changes = ends
                .iter()
                .enumerate()
                .map(|(k, &end)| {
                    let from_a = utf16(&import.dsl, end);
                    TextChange { from_a, to_a: from_a, from_b: from_a + k, to_b: from_a + k + 1 }
                })
                .collect();
            let guidance = Guidance { base_revision: patcher.revision(), changes };
            let (patches, _) = patcher.compute(&dsl, Some(&guidance))?;
            let (doc, tree) = {
                let tree = gate_tree(&import.original_preamble, &original, &import.original_postamble, &doc)?;
                slim(doc, tree)
            };
            let mut compiler = crate::parser::Compiler::new().with_config(compiler_config_for(&doc, Originals::new(Some(&tree))));
            let body = render_body(&doc, Some(&original), Some(&tree), &patches, &mut compiler)?;
            let written = format!(
                "{}{}{}",
                import.original_preamble.unwrap_or_default(),
                body.xml,
                import.original_postamble.unwrap_or_default()
            );
            let before = Tree::parse(&source)?;
            let after = Tree::parse(&written).map_err(|e| format!("doesn't read: {e}"))?;
            let (old_counts, new_counts) = (element_counts(&before), element_counts(&after));
            let lost: Vec<String> = old_counts
                .iter()
                .filter(|(name, n)| new_counts.get(*name).copied().unwrap_or(0) < **n)
                .map(|(name, n)| format!("{name} {n}→{}", new_counts.get(name).copied().unwrap_or(0)))
                .collect();
            if !lost.is_empty() {
                return Err(format!("elements lost: {}", lost.join(", ")));
            }
            let added = written.matches('ꝸ').count();
            if added < edited {
                return Err(format!("{edited} letters added, {added} written"));
            }
            Ok(edited)
        })();
        match result {
            Ok(n) => eprintln!("ok      {name} ({n} words edited)"),
            Err(e) => {
                eprintln!("FAILED  {name}: {e}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "{} of {} files failed: {:?}", failures.len(), files.len(), failures);
}

/// The document as a session holds it: without the copies of the file's
/// XML that the tree has (see `ImportedDocument::drop_copies`).
fn slim(
    mut doc: crate::importer::tei::segments::ImportedDocument,
    tree: crate::importer::tei::apply::TreeSource,
) -> (crate::importer::tei::segments::ImportedDocument, crate::importer::tei::apply::TreeSource) {
    doc.drop_copies(|id| tree.segment_xml(id), |id| tree.segment_attributes(id));
    (doc, tree)
}

/// The imported file as a tree, as the app edits it.
fn gate_tree(
    preamble: &Option<String>,
    body: &str,
    postamble: &Option<String>,
    doc: &crate::importer::tei::segments::ImportedDocument,
) -> Result<crate::importer::tei::apply::TreeSource, String> {
    crate::importer::tei::apply::TreeSource::new(
        preamble.as_deref().unwrap_or_default(),
        body,
        postamble.as_deref().unwrap_or_default(),
        &doc.segments,
        doc.spans.as_deref().ok_or("no spans")?,
    )
    .map_err(|e| format!("the file doesn't map onto its segments: {e}"))
}

/// One edit of the corpus gate below: `range` of the shorthand becomes
/// `text`, and `region` of the original body is all the XML it may change.
struct GateEdit {
    range: std::ops::Range<usize>,
    text: String,
    region: std::ops::Range<usize>,
}

/// Applies `edits` (sorted, apart) to the imported shorthand as the editor
/// would, with guidance, and returns the edited shorthand, the original body
/// and the written one.
fn put_edits(source: &str, edits: &[GateEdit]) -> Result<(String, String, String), String> {
    use crate::identity::{Guidance, TextChange};
    use crate::importer::tei::patching::{compiler_config_for, render_body, Originals, Patcher};
    let import = crate::importer::tei::parse(source)?;
    let doc = import.imported_document.ok_or("no manifest")?;
    let original = import.original_body_xml.clone().ok_or("no body")?;

    let utf16 = |text: &str| text.encode_utf16().count();
    let mut dsl = String::with_capacity(import.dsl.len());
    let mut changes = Vec::new();
    let mut copied = 0;
    for edit in edits {
        dsl.push_str(&import.dsl[copied..edit.range.start]);
        let from_b = utf16(&dsl);
        let from_a = utf16(&import.dsl[..edit.range.start]);
        let to_a = from_a + utf16(&import.dsl[edit.range.clone()]);
        dsl.push_str(&edit.text);
        changes.push(TextChange { from_a, to_a, from_b, to_b: utf16(&dsl) });
        copied = edit.range.end;
    }
    dsl.push_str(&import.dsl[copied..]);

    let mut patcher = Patcher::new(&doc.segments, doc.dsl_rules);
    let (_, state) = patcher.compute(&import.dsl, None)?;
    patcher.commit(state);
    let guidance = Guidance { base_revision: patcher.revision(), changes };
    let (patches, _) = patcher.compute(&dsl, Some(&guidance))?;
    let (doc, tree) = {
        let tree = gate_tree(&import.original_preamble, &original, &import.original_postamble, &doc)?;
        slim(doc, tree)
    };
    let mut compiler = crate::parser::Compiler::new().with_config(compiler_config_for(&doc, Originals::new(Some(&tree))));
    let body = render_body(&doc, Some(&original), Some(&tree), &patches, &mut compiler)?;
    Ok((dsl, original, body.xml))
}

/// Whether `out` is `original` with only `regions` (sorted, apart) replaced:
/// the text between them comes through, in order.
fn changed_only(original: &str, out: &str, regions: &[std::ops::Range<usize>]) -> Result<(), String> {
    let mut gaps = Vec::new();
    let mut from = 0;
    for region in regions {
        gaps.push(&original[from..region.start]);
        from = region.end;
    }
    let last = &original[from..];
    let mut at = 0;
    for (i, gap) in gaps.iter().enumerate() {
        let found = if i == 0 { out.starts_with(gap).then_some(0) } else { out[at..].find(gap).map(|k| at + k) };
        let Some(found) = found else {
            let shown: String = gap.chars().take(80).collect();
            return Err(format!("the XML between edits {} and {} changed: {shown:?}", i.max(1) - 1, i));
        };
        at = found + gap.len();
    }
    if !out.ends_with(last) || out.len() - last.len() < at {
        return Err("the XML after the last edit changed".into());
    }
    Ok(())
}

/// `region` of `body` with the elements around it that hold nothing else,
/// short of a paragraph.
fn with_emptied_wrappers(body: &str, mut region: std::ops::Range<usize>) -> std::ops::Range<usize> {
    loop {
        let before = body[..region.start].trim_end();
        let after = body[region.end..].trim_start();
        let Some(open) = before.rfind('<').filter(|_| before.ends_with('>')) else { return region };
        let tag = &before[open + 1..before.len() - 1];
        let name = tag.split(|c: char| c.is_whitespace()).next().unwrap_or_default();
        let close = format!("</{name}>");
        if name.is_empty() || tag.ends_with('/') || "/!?".contains(&name[..1]) || name == "p" || !after.starts_with(&close) {
            return region;
        }
        region = open..body.len() - after.len() + close.len();
    }
}

/// Gates of step 3b, PutGet and Locality, on every file: words are edited in
/// the shorthand in each way an editor does (a letter added or removed, a
/// word deleted, inserted or replaced by another, a line number changed).
/// The written file reads back as the edited shorthand, token for token, and
/// its XML differs from the original only where the edited words were.
#[test]
fn corpus_edits_are_put_back_locally() {
    use crate::importer::tei::patching::{dsl_tokens, segment_dsl_token};
    use crate::importer::tei::segments::Segment;
    let files = corpus_files();
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(source) = std::fs::read_to_string(path) else { continue };
        let result = (|| -> Result<usize, String> {
            let import = crate::importer::tei::parse(&source)?;
            let doc = import.imported_document.ok_or("no manifest")?;
            let segment_spans = doc.spans.clone().ok_or("no spans")?;
            let (_, token_spans) = crate::identity::spanned_dsl_tokens(&import.dsl)?;
            let tokens: Vec<&Segment> = doc.segments.iter().filter(|s| segment_dsl_token(s).is_some()).collect();
            if tokens.len() != token_spans.len() {
                return Err(format!("{} segments read as {} tokens", tokens.len(), token_spans.len()));
            }
            let dsl = &import.dsl;
            let body = import.original_body_xml.clone().ok_or("no body")?;
            let apart = |span: &std::ops::Range<usize>| {
                (span.start == 0 || dsl[..span.start].ends_with([' ', '\n']))
                    && (span.end == dsl.len() || dsl[span.end..].starts_with([' ', '\n']))
            };
            let xml_span = |segment: &Segment| -> Option<std::ops::Range<usize>> {
                let (start, end) = (*segment_spans.get(segment.id())?)?;
                Some(start..end)
            };

            // Candidates: plain words and numbered line breaks standing apart
            let mut edits = Vec::new();
            let mut kind = 0;
            let mut last_edited = None;
            for (i, (segment, span)) in tokens.iter().zip(&token_spans).enumerate() {
                if i % 40 != 39 || !apart(span) || last_edited.is_some_and(|l| i - l < 3) {
                    continue;
                }
                let text = &dsl[span.clone()];
                let Some(region) = xml_span(segment) else { continue };
                let edit = match segment {
                    Segment::Word { original_xml, .. }
                        if original_xml.starts_with("<w") && text.chars().all(char::is_alphabetic) =>
                    {
                        kind += 1;
                        match kind % 5 {
                            0 => GateEdit { range: span.end..span.end, text: "ꝸ".into(), region },
                            1 => {
                                let last = text.char_indices().last().map_or(0, |(k, _)| k);
                                if text.len() < 2 {
                                    continue;
                                }
                                GateEdit { range: span.start + last..span.end, text: String::new(), region }
                            }
                            // Deleted with the space before it, and with the
                            // elements it leaves empty
                            2 if span.start > 0 => GateEdit {
                                range: span.start - 1..span.end,
                                text: String::new(),
                                region: with_emptied_wrappers(&body, region),
                            },
                            3 => GateEdit { range: span.end..span.end, text: " ꝸꝸ".into(), region: region.end..region.end },
                            _ => GateEdit { range: span.clone(), text: "ꝸꝸꝸ".into(), region },
                        }
                    }
                    Segment::LineBreak { .. } if text.len() > 2 && text[2..].chars().all(|c| c.is_ascii_digit()) => {
                        GateEdit { range: span.clone(), text: "//{ꝸ1}".into(), region }
                    }
                    _ => continue,
                };
                last_edited = Some(i);
                edits.push(edit);
            }
            if edits.is_empty() {
                return Ok(0);
            }

            let (edited, original, written) = put_edits(&source, &edits)?;
            let regions: Vec<_> = edits.iter().map(|e| e.region.clone()).collect();
            changed_only(&original, &written, &regions)?;

            let file = format!(
                "{}{}{}",
                import.original_preamble.unwrap_or_default(),
                written,
                import.original_postamble.unwrap_or_default()
            );
            let reread = crate::importer::tei::parse(&file).map_err(|e| format!("doesn't read: {e}"))?;
            let (got, want) = (dsl_tokens(&reread.dsl)?, dsl_tokens(&edited)?);
            if got != want {
                let i = got.iter().zip(&want).position(|(a, b)| a != b).unwrap_or(got.len().min(want.len()));
                let around = |t: &[String]| t[i.saturating_sub(2)..(i + 3).min(t.len())].join(" ");
                return Err(format!("reads back differently at token {i}: {:?}, typed {:?}", around(&got), around(&want)));
            }
            Ok(edits.len())
        })();
        match result {
            Ok(n) => eprintln!("ok      {name} ({n} edits)"),
            Err(e) => {
                eprintln!("FAILED  {name}: {e}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "{} of {} files failed: {:?}", failures.len(), files.len(), failures);
}

/// A manuscript's tree has millions of nodes: a node's own data stays small
/// (an element's data and a changed node's old attributes are boxed).
#[test]
fn test_nodes_stay_small() {
    assert!(super::node_data_size() <= 112, "{} bytes", super::node_data_size());
}

/// Gate of step 3b (the tree replaces segments): a project saved with older
/// import rules (here: rules 1 and 2, the rules of 0.6 and 0.7, before
/// neighbours were set apart, and 5, before one lineation was chosen) is brought
/// onto the file's tree when opened. Unedited, its shorthand then reads as
/// the file imported now, token for token, and puts back the file
/// unchanged; with every 40th token edited, only the edits differ.
#[test]
fn corpus_older_projects_are_brought_onto_the_tree() {
    use crate::importer::tei::extraction::segments_to_dsl;
    use crate::importer::tei::patching::{compute_patches, dsl_tokens, PatchOperation};
    use crate::importer::tei::segments::DSL_RULES;
    let files = corpus_files();
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(source) = std::fs::read_to_string(path) else { continue };
        let result = (|| -> Result<(), String> {
            let import = crate::importer::tei::parse(&source)?;
            let doc = import.imported_document.ok_or("no manifest")?;
            let body = import.original_body_xml.ok_or("no body")?;
            let (pre, post) = (import.original_preamble.unwrap_or_default(), import.original_postamble.unwrap_or_default());
            let fresh_tokens = dsl_tokens(&import.dsl)?;
            // Rules 5: before one lineation was chosen, all line breaks shown
            let all_breaks = crate::importer::tei::parse_with(&source, Some(None))?.imported_document.ok_or("no manifest")?;
            for rules in [1, 2, 5] {
            let mut stored = if rules == 5 { all_breaks.clone() } else { doc.clone() };
            stored.dsl_rules = rules;
            stored.profile = None;
            let old = segments_to_dsl(&stored.segments, rules).trim().to_string();
            // Today's segments written with older rules can make shorthand
            // that no project had (today's lexer rejects it): not a case
            if rules < 5 && dsl_tokens(&old).is_err() {
                eprintln!("skip    {name}: rules {rules} make shorthand that doesn't parse");
                continue;
            }

            let on_tree = crate::importer::tei::onto_tree(&stored, &old, &pre, &body, &post).ok_or("not brought onto the tree")?;
            if dsl_tokens(&on_tree.dsl)? != fresh_tokens {
                let got = dsl_tokens(&on_tree.dsl)?;
                let at = got.iter().zip(&fresh_tokens).position(|(a, b)| a != b).unwrap_or(got.len().min(fresh_tokens.len()));
                let show = |t: &[String]| t[at.saturating_sub(2)..(at + 3).min(t.len())].join(" ");
                return Err(format!(
                    "rules {rules}: unedited, it doesn't read as the import ({} places rewritten): {:?} for {:?}",
                    on_tree.rewritten,
                    show(&got),
                    show(&fresh_tokens)
                ));
            }
            let patches = compute_patches(&on_tree.document.segments, DSL_RULES, &on_tree.dsl)?;
            if patches.iter().any(|p| !matches!(p, PatchOperation::Keep { .. })) {
                return Err(format!("rules {rules}: unedited, it changes the file"));
            }

            // Every 40th token of the old shorthand gets a letter
            let (tokens, spans) = crate::identity::spanned_dsl_tokens(&old)?;
            let mut edited = old.clone();
            let mut edits = 0;
            for (k, span) in spans.iter().enumerate().rev() {
                if k % 40 == 20 && tokens[k].chars().all(char::is_alphabetic) {
                    edited.insert(span.end, 'x');
                    edits += 1;
                }
            }
            let on_tree = crate::importer::tei::onto_tree(&stored, &edited, &pre, &body, &post).ok_or("edited: not brought onto the tree")?;
            // Every place that reads differently from the import has an edit
            let got = dsl_tokens(&on_tree.dsl)?;
            let a: Vec<&str> = fresh_tokens.iter().map(String::as_str).collect();
            let b: Vec<&str> = got.iter().map(String::as_str).collect();
            let mut diff = Vec::new();
            crate::identity::diff_range(&a, &b, 0, 0, &mut diff);
            use crate::identity::Edit;
            let (mut in_run, mut explained, mut unexplained) = (false, false, 0);
            for edit in diff.iter().chain([&Edit::Keep(usize::MAX)]) {
                match *edit {
                    Edit::Keep(_) => {
                        if in_run && !explained {
                            unexplained += 1;
                            if std::env::var("DEBUG_OLDER").is_ok() {
                                if let Edit::Keep(i) = *edit {
                                    let i = i.min(a.len());
                                    eprintln!("  before XML token {i}: {:?}", a[i.saturating_sub(4)..i].join(" "));
                                }
                            }
                        }
                        (in_run, explained) = (false, false);
                    }
                    Edit::Insert(j) | Edit::Replace(_, j) => {
                        in_run = true;
                        explained |= b[j].contains('x');
                    }
                    Edit::Delete(_) => in_run = true,
                }
            }
            // The comparison that explains differences can pair an edit with
            // its neighbour now and then; older rules are only approximated
            // here (today's segments), so one more is allowed for them
            if unexplained > edits / 500 + usize::from(rules < 5) {
                return Err(format!("rules {rules}: {unexplained} places changed that weren't edited ({edits} edits)"));
            }
            }
            Ok(())
        })();
        match result {
            Ok(()) => eprintln!("ok      {name}"),
            Err(e) => {
                eprintln!("FAILED  {name}: {e}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "{} of {} files failed: {:?}", failures.len(), files.len(), failures);
}

/// Gate of step 6 (`.teis` 2.0): in every file, every 40th word edited,
/// the project saved (the file as written, its profile and open records) and
/// opened again: the shorthand is as stored, the file the same, and the
/// records still open are back, on the same words.
#[test]
fn corpus_saved_projects_open_as_they_were() {
    use crate::commands::parse::ImportedSession;
    let files = corpus_files();
    let mut failures = Vec::new();
    let dir = std::env::temp_dir().join(format!("saga-scribe-gate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(source) = std::fs::read_to_string(path) else { continue };
        let result = (|| -> Result<(), String> {
            let session = ImportedSession::default();
            let reply = crate::commands::import::stage(crate::importer::tei::parse(&source)?, &session)?;
            let store = reply.imported.as_ref().map(|s| s.abbreviations.clone()).unwrap_or_default();
            session.load(reply.imported.map(|s| s.handle))?;
            let dsl = reply.dsl;
            let (tokens, spans) = crate::identity::spanned_dsl_tokens(&dsl)?;
            let mut edited = dsl.clone();
            for (k, span) in spans.iter().enumerate().rev() {
                if k % 40 == 20 && tokens[k].chars().all(char::is_alphabetic) {
                    edited.insert(span.end, 'x');
                }
            }
            let first = session.with_current_mut(|s| s.compile_plainly(&edited))?.ok_or("not loaded")??;
            let project = dir.join(format!("{name}.teis"));
            let fields = crate::commands::file::ProjectFields {
                source: edited.clone(),
                output: first.xml.clone(),
                confirmations_json: "{}".into(),
                template_id: "menota".into(),
                metadata_json: None,
                annotations_json: None,
                abbreviations_json: Some(serde_json::to_string(&store).map_err(|e| e.to_string())?),
            };
            crate::commands::file::save_project_at(&session, &project, fields, Some("{}".into()))?;
            let mut opened = crate::commands::file::read_project(&project.to_string_lossy())?;
            std::fs::remove_file(&project).ok();
            let (staged, rewritten) = crate::commands::file::stage_project(&mut opened, &session)?;
            if rewritten > 0 || opened.source != edited {
                return Err(format!("the shorthand changed ({rewritten} places)"));
            }
            session.load(staged.map(|s| s.handle))?;
            let again = session.with_current_mut(|s| s.compile_plainly(&opened.source))?.ok_or("not loaded")??;
            if again.xml != first.xml {
                return Err("the file isn't the same".into());
            }
            let words = |records: &[crate::importer::tei::patching::WordRecord]| -> Vec<(Option<usize>, String)> {
                records.iter().map(|r| (r.word, r.was.clone())).collect()
            };
            let (before, after) = (words(&first.records), words(&again.records));
            if before != after {
                let at = before.iter().zip(&after).position(|(a, b)| a != b).unwrap_or(before.len().min(after.len()));
                let only_before: Vec<_> = before.iter().filter(|x| !after.contains(x)).take(5).collect();
                let only_after: Vec<_> = after.iter().filter(|x| !before.contains(x)).take(5).collect();
                eprintln!("  only before {only_before:?}, only after {only_after:?}");
                return Err(format!(
                    "{} records before, {} after; at {at}: {:?} became {:?}",
                    before.len(),
                    after.len(),
                    before.get(at),
                    after.get(at)
                ));
            }
            Ok(())
        })();
        match result {
            Ok(()) => eprintln!("ok      {name}"),
            Err(e) => {
                eprintln!("FAILED  {name}: {e}");
                failures.push(name);
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(failures.is_empty(), "{} of {} files failed: {:?}", failures.len(), files.len(), failures);
}

/// Not a gate: with `MARKUP_SURVEY=<file>`, writes a CSV of the markup in the
/// corpus's bodies the shorthand can't show: elements it has no syntax for,
/// and attributes of the elements it shows that it doesn't write.
#[test]
fn corpus_zz_markup_survey() {
    use crate::document::anchor::body_words;
    use std::collections::BTreeMap;
    let Ok(out) = std::env::var("MARKUP_SURVEY") else { return };
    let local = |name: &str| name.rsplit(':').next().unwrap_or(name).to_string();
    // What the shorthand writes of the elements it shows
    let expressed = |element: &str, attribute: &str| {
        matches!(
            (element, attribute),
            ("lb" | "pb", "n") | ("w", "lemma" | "me:msa" | "msa") | ("gap", "quantity") | ("xml" | _, "xml:space")
        )
    };
    #[derive(Default)]
    struct Row {
        occurrences: usize,
        files: std::collections::BTreeSet<String>,
        texts: usize,
        charters: usize,
        example_file: String,
        example: String,
    }
    let mut rows: BTreeMap<(String, String), Row> = BTreeMap::new();
    let files = corpus_files();
    for path in &files {
        let Ok(source) = std::fs::read_to_string(path) else { continue };
        let Ok(tree) = super::Tree::parse(&source) else { continue };
        let Some(body) = super::find_body(&tree) else { continue };
        let _ = body_words;
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let charter = path.to_string_lossy().contains("diploma");
        for id in tree.descendants(body) {
            let Some(element) = tree.element(id) else { continue };
            let shown = super::profile::SHOWN.contains(&local(&element.name).as_str());
            let mut keys = Vec::new();
            if !shown {
                keys.push(("element".to_string(), element.name.clone()));
            } else {
                for attribute in &element.attributes {
                    if !expressed(&local(&element.name), &attribute.name) {
                        keys.push(("attribute".to_string(), format!("{}@{}", element.name, attribute.name)));
                    }
                }
            }
            for key in keys {
                let row = rows.entry(key).or_default();
                row.occurrences += 1;
                if row.files.insert(name.clone()) {
                    if charter { row.charters += 1 } else { row.texts += 1 }
                }
                if row.example.is_empty() {
                    row.example_file = name.clone();
                    let xml = tree.node_xml(id).unwrap_or_default();
                    row.example = xml.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(160).collect();
                }
            }
        }
    }
    let field = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut csv = String::from("kind,name,occurrences,files,texts,charters,example_file,example,shorthand\n");
    let mut sorted: Vec<_> = rows.into_iter().collect();
    sorted.sort_by(|a, b| b.1.occurrences.cmp(&a.1.occurrences).then_with(|| a.0.cmp(&b.0)));
    for ((kind, name), row) in sorted {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},\n",
            kind,
            field(&name),
            row.occurrences,
            row.files.len(),
            row.texts,
            row.charters,
            field(&row.example_file),
            field(&row.example)
        ));
    }
    std::fs::write(&out, csv).expect("the survey file is written");
    eprintln!("wrote {out} from {} files", files.len());
}
