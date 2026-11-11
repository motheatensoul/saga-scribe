use super::*;

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
/// Run with `MENOTA_CORPUS=<dir> cargo test --release corpus_ -- --nocapture`.
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
