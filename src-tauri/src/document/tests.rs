use super::project::{find_body, project_body};
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
        NodeKind::Element(Element {
            name: "ex".into(),
            namespaces: vec![],
            attributes: vec![Attribute { name: "resp".into(), value: "\"A&B\"".into() }],
        }),
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

/// The opening tag's name and attributes, in any order (the extractor wrote
/// attributes in hash order).
fn normalized_tag(xml: &str) -> Vec<String> {
    // Values are escaped, so `"` only delimits them: pieces alternate between
    // ` name=` (the first also holding the element name) and a value
    let inner = xml.trim_start_matches('<').trim_end_matches('>');
    let pieces: Vec<&str> = inner.split('"').collect();
    let mut parts = Vec::new();
    for pair in pieces.chunks(2) {
        let mut key = pair[0].trim();
        if parts.is_empty() {
            let (name, rest) = key.split_once(char::is_whitespace).unwrap_or((key, ""));
            parts.push(format!("<{name}"));
            key = rest.trim();
        }
        if let Some(value) = pair.get(1) {
            parts.push(format!("{key}{value}"));
        }
    }
    parts[1..].sort();
    parts
}

/// What the old extractor and the projection disagree on, if anything.
fn projection_difference(source: &str) -> Option<String> {
    use crate::importer::tei::segments::{Segment, DSL_RULES};
    use crate::importer::tei::extraction::{segments_to_dsl, Extractor};
    let tree = Tree::parse(source).ok()?;
    let body = find_body(&tree)?;
    let new = project_body(&tree, body);

    // The extractor, on libxml2's tree, with spans relative to the same body
    let doc = crate::xml::parse(source).ok()?;
    let mut stack = vec![doc.get_root_element()?];
    let xml_body = loop {
        let node = stack.pop()?;
        if node.get_name() == "body" {
            break node;
        }
        stack.extend(node.get_child_elements().into_iter().rev());
    };
    let body_text = &source[tree.span(body).unwrap_or(0..source.len())];
    let mut extractor = Extractor::with_source(body_text);
    let segments = extractor.extract_segments(&xml_body);
    let old_spans = extractor.take_spans();
    let old_dsl = segments_to_dsl(&segments, DSL_RULES);
    struct Old {
        segments: Vec<Segment>,
        spans: Option<crate::importer::tei::segments::SourceSpans>,
    }
    let old = Old { segments, spans: old_spans };

    let dsl = segments_to_dsl(&new.segments, DSL_RULES);
    let import_dsl = old_dsl.trim();
    if dsl.trim() != import_dsl {
        let at = dsl.trim().chars().zip(import_dsl.chars()).take_while(|(a, b)| a == b).count();
        let context: String = import_dsl.chars().skip(at.saturating_sub(40)).take(80).collect();
        return Some(format!("shorthand differs at char {at}: {context:?}"));
    }
    if old.segments.len() != new.segments.len() {
        return Some(format!("{} segments, then {}", old.segments.len(), new.segments.len()));
    }
    for (a, b) in old.segments.iter().zip(&new.segments) {
        let same = match (a, b) {
            (
                Segment::Word { id, attributes, dsl_content, has_inline_lb, .. },
                Segment::Word { id: id2, attributes: a2, dsl_content: d2, has_inline_lb: l2, .. },
            ) => id == id2 && attributes == a2 && dsl_content == d2 && has_inline_lb == l2,
            (Segment::Punctuation { id, dsl_content, .. }, Segment::Punctuation { id: id2, dsl_content: d2, .. }) => {
                id == id2 && dsl_content == d2
            }
            (Segment::Structural { id, xml }, Segment::Structural { id: id2, xml: x2 })
            | (Segment::Paragraph { id, xml }, Segment::Paragraph { id: id2, xml: x2 }) => {
                id == id2 && normalized_tag(xml) == normalized_tag(x2)
            }
            (Segment::LineBreak { id, attributes }, Segment::LineBreak { id: id2, attributes: a2 })
            | (Segment::PageBreak { id, attributes }, Segment::PageBreak { id: id2, attributes: a2 })
            | (Segment::HandShift { id, attributes }, Segment::HandShift { id: id2, attributes: a2 }) => {
                id == id2 && attributes == a2
            }
            (Segment::Whitespace { id, content }, Segment::Whitespace { id: id2, content: c2 }) => {
                id == id2 && content == c2
            }
            _ => false,
        };
        if !same {
            return Some(format!("segment differs: {a:?} became {b:?}"));
        }
    }
    if old.spans != new.spans {
        let first = old.spans.as_ref().zip(new.spans.as_ref()).and_then(|(a, b)| {
            a.iter().zip(b).position(|(x, y)| x != y)
        });
        return Some(format!(
            "spans differ (old {}, new {}, first at segment {first:?})",
            old.spans.is_some(),
            new.spans.is_some()
        ));
    }
    None
}

#[test]
fn test_projection_matches_the_importer_on_holmperg() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/HolmPerg-34-4to-MLL.xml");
    let Ok(source) = std::fs::read_to_string(path) else { return };
    assert_eq!(projection_difference(&source), None);
}

/// Gate of step 2: for every file, the projection gives the importer's
/// segments (but `original_xml`), spans and shorthand.
#[test]
fn corpus_projection_matches_the_importer() {
    let files = corpus_files();
    let mut failures = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(source) = std::fs::read_to_string(path) else { continue };
        match projection_difference(&source) {
            None => eprintln!("ok      {name}"),
            Some(difference) => {
                eprintln!("FAILED  {name}: {difference}");
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
    use crate::importer::tei::patching::{compiler_config_for, compute_patches, render_body};
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
            let mut compiler = crate::parser::Compiler::new().with_config(compiler_config_for(&doc));
            let body = render_body(&doc, Some(&original), &patches, &mut compiler)?;
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
